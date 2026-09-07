use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::crypto::system_random;
use super::windows;
use super::{ProtectionError, ProtectionWarning, PublicationStatus, io_error};

const STAGING_PREFIX: &str = ".m2c-m5-staging-";
const MAX_M4_CONTROL_BYTES: u64 = 4096;
const STAGING_ATTEMPTS: usize = 32;

#[derive(Debug)]
pub(crate) struct PreparedDestination {
    parent: PathBuf,
    final_path: PathBuf,
}

pub(crate) struct StagingName(String);

fn symlink_metadata(path: &Path) -> Result<Option<Metadata>, ProtectionError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(io_error("inspect M5 path", path, source)),
    }
}

#[cfg(windows)]
fn is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes()
        & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
        != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn absolute_without_parent_components(path: &Path) -> Result<PathBuf, ProtectionError> {
    if path.components().any(|part| part == Component::ParentDir) {
        return Err(ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "parent-directory components are not allowed",
        });
    }
    if path.is_absolute() {
        Ok(path.to_owned())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| io_error("resolve current directory", path, source))
    }
}

fn validate_ancestors_no_reparse(path: &Path) -> Result<(), ProtectionError> {
    let mut cursor = Some(path);
    while let Some(current) = cursor {
        let metadata = symlink_metadata(current)?.ok_or_else(|| ProtectionError::UnsafePath {
            path: current.to_owned(),
            reason: "output ancestor does not exist",
        })?;
        if is_reparse(&metadata) {
            return Err(ProtectionError::UnsafePath {
                path: current.to_owned(),
                reason: "reparse points and symlinks are not allowed in output paths",
            });
        }
        cursor = current.parent();
    }
    Ok(())
}

fn named_entry_exists(directory: &Path, name: &str) -> Result<bool, ProtectionError> {
    Ok(symlink_metadata(&directory.join(name))?.is_some())
}

fn is_directory(path: &Path) -> Result<bool, ProtectionError> {
    Ok(symlink_metadata(path)?.is_some_and(|metadata| metadata.is_dir()))
}

fn read_control(path: &Path) -> Result<Value, ProtectionError> {
    let file =
        File::open(path).map_err(|source| io_error("open possible M4 marker", path, source))?;
    let mut bytes = Vec::with_capacity((MAX_M4_CONTROL_BYTES + 1) as usize);
    file.take(MAX_M4_CONTROL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| io_error("read possible M4 marker", path, source))?;
    if bytes.len() as u64 > MAX_M4_CONTROL_BYTES {
        return Err(ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "possible M4 control file exceeds its bounded classification size",
        });
    }
    serde_json::from_slice(&bytes).map_err(|_| ProtectionError::UnsafePath {
        path: path.to_owned(),
        reason: "possible M4 control file cannot be classified safely",
    })
}

fn declares_m4(directory: &Path, name: &str) -> Result<bool, ProtectionError> {
    let path = directory.join(name);
    if symlink_metadata(&path)?.is_none() {
        return Ok(false);
    }
    let value = read_control(&path)?;
    let Some(object) = value.as_object() else {
        return Err(ProtectionError::UnsafePath {
            path,
            reason: "possible M4 control file is not an object",
        });
    };
    if name == "manifest.json" {
        return Ok(
            object.get("format").and_then(Value::as_str) == Some("m2c-m4")
                || (object.get("version").and_then(Value::as_u64) == Some(1)
                    && object.get("profile").and_then(Value::as_str)
                        == Some("m2c-v0.1-cp037-parquet53-uncompressed-v1")),
        );
    }
    Ok(object.get("version").and_then(Value::as_u64) == Some(1))
}

fn m4_root(parent: &Path) -> Result<Option<PathBuf>, ProtectionError> {
    for directory in parent.ancestors() {
        for marker in [".m4.lock", ".manifest.json.tmp", ".complete.json.tmp"] {
            if named_entry_exists(directory, marker)? {
                return Ok(Some(directory.to_owned()));
            }
        }
        let manifest_exists = named_entry_exists(directory, "manifest.json")?;
        let complete_exists = named_entry_exists(directory, "complete.json")?;
        if is_directory(&directory.join("parts"))?
            && is_directory(&directory.join("commits"))?
            && (manifest_exists || complete_exists)
        {
            return Ok(Some(directory.to_owned()));
        }
        if (manifest_exists && declares_m4(directory, "manifest.json")?)
            || (complete_exists && declares_m4(directory, "complete.json")?)
        {
            return Ok(Some(directory.to_owned()));
        }
    }
    Ok(None)
}

fn require_supported_publication_target(
    path: &Path,
    supported: bool,
) -> Result<(), ProtectionError> {
    if supported {
        Ok(())
    } else {
        Err(ProtectionError::UnsupportedPublicationPlatform {
            path: path.to_owned(),
            reason: "M5 v1 requires Windows/MSVC on a local NTFS volume",
        })
    }
}

pub(crate) fn prepare_destination(path: &Path) -> Result<PreparedDestination, ProtectionError> {
    let absolute = absolute_without_parent_components(path)?;
    let file_name = absolute
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "output must have a final filename",
        })?;
    let filename = file_name.to_string_lossy();
    if filename.contains(':')
        || filename.ends_with(['.', ' '])
        || filename.to_ascii_lowercase().starts_with(STAGING_PREFIX)
    {
        return Err(ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "output filename is ambiguous or reserved on Windows",
        });
    }
    let lexical_parent = absolute
        .parent()
        .ok_or_else(|| ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "output must have an existing parent directory",
        })?;
    validate_ancestors_no_reparse(lexical_parent)?;
    let parent = fs::canonicalize(lexical_parent)
        .map_err(|source| io_error("resolve M5 output parent", lexical_parent, source))?;
    let parent_metadata =
        symlink_metadata(&parent)?.ok_or_else(|| ProtectionError::UnsafePath {
            path: parent.clone(),
            reason: "output parent disappeared",
        })?;
    if !parent_metadata.is_dir() {
        return Err(ProtectionError::UnsafePath {
            path: parent,
            reason: "output parent is not a directory",
        });
    }
    require_supported_publication_target(&parent, windows::is_supported_ntfs(&parent))?;
    if let Some(root) = m4_root(&parent)? {
        return Err(ProtectionError::DestinationInM4Namespace {
            path: path.to_owned(),
            root,
        });
    }
    let final_path = parent.join(file_name);
    if let Some(metadata) = symlink_metadata(&final_path)? {
        if is_reparse(&metadata) {
            return Err(ProtectionError::UnsafePath {
                path: final_path,
                reason: "the output name is a reparse point or symlink",
            });
        }
        return Err(ProtectionError::OutputAlreadyExists { path: final_path });
    }
    Ok(PreparedDestination { parent, final_path })
}

pub(crate) fn random_staging_name() -> Result<StagingName, ProtectionError> {
    let mut random = [0_u8; 16];
    system_random(&mut random)?;
    let mut name = String::with_capacity(STAGING_PREFIX.len() + random.len() * 2);
    name.push_str(STAGING_PREFIX);
    for byte in random {
        use std::fmt::Write as _;
        write!(name, "{byte:02x}").map_err(|_| ProtectionError::ArithmeticOverflow {
            operation: "M5 staging filename",
        })?;
    }
    Ok(StagingName(name))
}

pub(crate) struct StagedOutput {
    prepared: PreparedDestination,
    path: PathBuf,
    file: Option<File>,
    committed: bool,
    secret: bool,
    warnings: Vec<ProtectionWarning>,
}

impl StagedOutput {
    pub(crate) fn create(path: &Path, secret: bool) -> Result<Self, ProtectionError> {
        let initial = random_staging_name()?;
        Self::create_with_initial(path, secret, initial)
    }

    pub(crate) fn create_with_initial(
        path: &Path,
        secret: bool,
        initial: StagingName,
    ) -> Result<Self, ProtectionError> {
        let prepared = prepare_destination(path)?;
        let mut name = Some(initial);
        for _ in 0..STAGING_ATTEMPTS {
            let stage_path = prepared
                .parent
                .join(name.take().map_or_else(random_staging_name, Ok)?.0);
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&stage_path)
            {
                Ok(file) => {
                    let mut warnings = Vec::new();
                    if secret {
                        restrict_or_warn(&stage_path, &mut warnings);
                    }
                    return Ok(Self {
                        prepared,
                        path: stage_path,
                        file: Some(file),
                        committed: false,
                        secret,
                        warnings,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(source) => return Err(io_error("create M5 staging", &stage_path, source)),
            }
        }
        Err(ProtectionError::UnsafePath {
            path: prepared.parent,
            reason: "could not create a unique M5 staging name",
        })
    }

    pub(crate) fn file_mut(&mut self) -> Result<&mut File, ProtectionError> {
        self.file
            .as_mut()
            .ok_or(ProtectionError::InvalidFrameSequence {
                reason: "M5 staging file is already closed",
            })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn finish(
        self,
    ) -> Result<(PublicationStatus, Vec<ProtectionWarning>), ProtectionError> {
        self.finish_with(
            || Ok(()),
            || Ok(()),
            |staging, final_path| fs::hard_link(staging, final_path),
            |path| fs::remove_file(path),
        )
    }

    fn finish_with<B, L, C, R>(
        mut self,
        before_validation: B,
        before_link: L,
        commit_link: C,
        remove_stage: R,
    ) -> Result<(PublicationStatus, Vec<ProtectionWarning>), ProtectionError>
    where
        B: FnOnce() -> io::Result<()>,
        L: FnOnce() -> io::Result<()>,
        C: FnOnce(&Path, &Path) -> io::Result<()>,
        R: FnOnce(&Path) -> io::Result<()>,
    {
        let file = self
            .file
            .take()
            .ok_or(ProtectionError::InvalidFrameSequence {
                reason: "M5 staging file is already closed",
            })?;
        file.sync_all()
            .map_err(|source| io_error("sync M5 staging", &self.path, source))?;
        drop(file);
        before_validation()
            .map_err(|source| io_error("M5 pre-commit hook", &self.prepared.final_path, source))?;
        let validated = prepare_destination(&self.prepared.final_path)?;
        if validated.parent != self.prepared.parent {
            return Err(ProtectionError::UnsafePath {
                path: self.prepared.final_path.clone(),
                reason: "output parent changed before commit",
            });
        }
        before_link()
            .map_err(|source| io_error("M5 pre-link hook", &self.prepared.final_path, source))?;
        match commit_link(&self.path, &self.prepared.final_path) {
            Ok(()) => self.committed = true,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(ProtectionError::OutputAlreadyExists {
                    path: self.prepared.final_path.clone(),
                });
            }
            Err(error) if matches!(error.raw_os_error(), Some(1 | 17 | 50)) => {
                return Err(ProtectionError::UnsupportedPublicationPlatform {
                    path: self.prepared.final_path.clone(),
                    reason: "NTFS hard-link no-clobber commit is unavailable",
                });
            }
            Err(source) => {
                return Err(io_error(
                    "commit M5 output with hard link",
                    &self.prepared.final_path,
                    source,
                ));
            }
        }
        if self.secret {
            restrict_or_warn(&self.prepared.final_path, &mut self.warnings);
        }
        let status = match remove_stage(&self.path) {
            Ok(()) => PublicationStatus::Published,
            Err(_) => PublicationStatus::PublishedWithStagingResidue(self.path.clone()),
        };
        Ok((status, std::mem::take(&mut self.warnings)))
    }
}

pub(crate) fn create_output_directory(
    path: &Path,
) -> Result<(PathBuf, Vec<ProtectionWarning>), ProtectionError> {
    let prepared = prepare_destination(path)?;
    match fs::create_dir(&prepared.final_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            return Err(ProtectionError::OutputAlreadyExists {
                path: prepared.final_path,
            });
        }
        Err(source) => {
            return Err(io_error(
                "create M5 key directory",
                &prepared.final_path,
                source,
            ));
        }
    }
    let mut warnings = Vec::new();
    restrict_or_warn(&prepared.final_path, &mut warnings);
    Ok((prepared.final_path, warnings))
}

impl Drop for StagedOutput {
    fn drop(&mut self) {
        if !self.committed {
            self.file.take();
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub(crate) fn restrict_or_warn(path: &Path, warnings: &mut Vec<ProtectionWarning>) {
    restrict_or_warn_result(path, warnings, windows::restrict_permissions(path));
}

fn restrict_or_warn_result(
    path: &Path,
    warnings: &mut Vec<ProtectionWarning>,
    result: io::Result<()>,
) {
    if let Err(error) = result {
        warnings.push(ProtectionWarning::PermissionRestrictionFailed {
            path: path.to_owned(),
            reason: error.to_string(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "m2c-m5-publication-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn supported() -> bool {
        windows::is_supported_ntfs(&std::env::temp_dir())
    }

    fn skip_without_supported_ntfs(test: &str) -> bool {
        if supported() {
            false
        } else {
            eprintln!("skipped {test}: requires Windows/MSVC on local NTFS");
            true
        }
    }

    #[test]
    fn hard_link_commit_is_no_clobber_and_cleanup_residue_is_explicit() {
        if skip_without_supported_ntfs("hard-link no-clobber publication") {
            return;
        }
        let temp = TempDir::new();
        let output = temp.0.join("output.bin");
        let mut staged = StagedOutput::create(&output, false).unwrap();
        staged.file_mut().unwrap().write_all(b"payload").unwrap();
        let (status, _) = staged
            .finish_with(
                || Ok(()),
                || Ok(()),
                |staging, final_path| fs::hard_link(staging, final_path),
                |_| Err(io::Error::other("injected")),
            )
            .unwrap();
        let residue = match status {
            PublicationStatus::PublishedWithStagingResidue(path) => path,
            other => panic!("unexpected publication status: {other:?}"),
        };
        assert_eq!(fs::read(&output).unwrap(), b"payload");
        assert!(residue.exists());

        let raced = temp.0.join("raced.bin");
        let mut contender = StagedOutput::create(&raced, false).unwrap();
        contender.file_mut().unwrap().write_all(b"loser").unwrap();
        assert!(matches!(
            contender.finish_with(
                || Ok(()),
                || fs::write(&raced, b"winner"),
                |staging, final_path| fs::hard_link(staging, final_path),
                |path| fs::remove_file(path),
            ),
            Err(ProtectionError::OutputAlreadyExists { .. })
        ));
        assert_eq!(fs::read(&raced).unwrap(), b"winner");
    }

    #[test]
    fn every_frozen_m4_marker_rejects_staging_in_descendants() {
        if skip_without_supported_ntfs("M4 marker isolation") {
            return;
        }
        for marker in [".m4.lock", ".manifest.json.tmp", ".complete.json.tmp"] {
            let temp = TempDir::new();
            fs::write(temp.0.join(marker), b"marker").unwrap();
            let child = temp.0.join("child");
            fs::create_dir(&child).unwrap();
            let output = child.join("output.bin");
            assert!(matches!(
                StagedOutput::create(&output, false),
                Err(ProtectionError::DestinationInM4Namespace { .. })
            ));
            assert!(fs::read_dir(&child).unwrap().next().is_none());
        }
    }

    #[test]
    fn m4_shape_corrupt_manifest_and_late_marker_fail_closed() {
        if skip_without_supported_ntfs("M4 shape and late-marker isolation") {
            return;
        }
        let temp = TempDir::new();
        fs::create_dir(temp.0.join("parts")).unwrap();
        fs::create_dir(temp.0.join("commits")).unwrap();
        fs::write(temp.0.join("manifest.json"), b"{corrupt").unwrap();
        let output = temp.0.join("parts").join("forbidden.bin");
        assert!(matches!(
            StagedOutput::create(&output, false),
            Err(ProtectionError::DestinationInM4Namespace { .. })
        ));

        let adjacent = temp.0.parent().unwrap().join(format!(
            "m2c-m5-adjacent-{}-{}",
            std::process::id(),
            STAGING_ATTEMPTS
        ));
        fs::create_dir(&adjacent).unwrap();
        let adjacent_output = adjacent.join("allowed.bin");
        let mut staged = StagedOutput::create(&adjacent_output, false).unwrap();
        staged.file_mut().unwrap().write_all(b"safe").unwrap();
        let parent = adjacent.clone();
        assert!(matches!(
            staged.finish_with(
                || fs::write(parent.join(".m4.lock"), b"late"),
                || Ok(()),
                |staging, final_path| fs::hard_link(staging, final_path),
                |path| fs::remove_file(path)
            ),
            Err(ProtectionError::DestinationInM4Namespace { .. })
        ));
        assert!(!adjacent_output.exists());
        assert_eq!(
            fs::read_dir(&adjacent)
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>(),
            vec![std::ffi::OsString::from(".m4.lock")]
        );
        fs::remove_dir_all(adjacent).unwrap();
    }

    #[test]
    fn recognizable_m4_documents_reject_root_and_descendants() {
        if skip_without_supported_ntfs("recognizable M4 document isolation") {
            return;
        }
        for (name, contents) in [
            (
                "manifest.json",
                br#"{"format":"m2c-m4","version":999}"#.as_slice(),
            ),
            ("complete.json", br#"{"version":1}"#.as_slice()),
        ] {
            let temp = TempDir::new();
            fs::write(temp.0.join(name), contents).unwrap();
            for directory in [temp.0.clone(), temp.0.join("nested")] {
                if directory != temp.0 {
                    fs::create_dir(&directory).unwrap();
                }
                assert!(matches!(
                    prepare_destination(&directory.join("output")),
                    Err(ProtectionError::DestinationInM4Namespace { .. })
                ));
            }
        }
    }

    #[test]
    fn permission_failure_is_a_structured_warning() {
        let mut warnings = Vec::new();
        restrict_or_warn_result(
            Path::new("secret.key"),
            &mut warnings,
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected")),
        );
        assert!(matches!(
            warnings.as_slice(),
            [ProtectionWarning::PermissionRestrictionFailed { path, reason }]
                if path == Path::new("secret.key") && reason.contains("injected")
        ));
    }

    #[test]
    fn unsupported_filesystem_and_hard_link_conditions_fail_closed() {
        let unsupported = require_supported_publication_target(Path::new("not-ntfs"), false);
        assert!(matches!(
            unsupported,
            Err(ProtectionError::UnsupportedPublicationPlatform { .. })
        ));

        if skip_without_supported_ntfs("hard-link failure injection") {
            return;
        }
        let temp = TempDir::new();
        let canonical_parent = fs::canonicalize(&temp.0).unwrap();
        for error_code in [1, 17, 50] {
            let output = temp.0.join(format!("unsupported-{error_code}.bin"));
            let mut staged = StagedOutput::create(&output, false).unwrap();
            assert_eq!(staged.path().parent(), Some(canonical_parent.as_path()));
            staged.file_mut().unwrap().write_all(b"payload").unwrap();
            let staging = staged.path().to_owned();
            let result = staged.finish_with(
                || Ok(()),
                || Ok(()),
                move |_, _| Err(io::Error::from_raw_os_error(error_code)),
                |path| fs::remove_file(path),
            );
            assert!(matches!(
                result,
                Err(ProtectionError::UnsupportedPublicationPlatform { .. })
            ));
            assert!(!output.exists());
            assert!(!staging.exists());
        }

        let secret = temp.0.join("secret.key");
        fs::write(&secret, b"incumbent secret").unwrap();
        assert!(matches!(
            StagedOutput::create(&secret, true),
            Err(ProtectionError::OutputAlreadyExists { .. })
        ));
        assert_eq!(fs::read(secret).unwrap(), b"incumbent secret");
    }

    #[test]
    #[cfg(windows)]
    fn reparse_point_in_write_path_fails_closed() {
        use std::os::windows::fs::symlink_dir;

        if skip_without_supported_ntfs("reparse-point rejection") {
            return;
        }
        let temp = TempDir::new();
        let real = temp.0.join("real");
        let link = temp.0.join("reparse");
        fs::create_dir(&real).unwrap();
        if let Err(error) = symlink_dir(&real, &link) {
            eprintln!(
                "skipped reparse-point rejection: creating a test symlink is unavailable: {error}"
            );
            return;
        }
        assert!(matches!(
            prepare_destination(&link.join("output.bin")),
            Err(ProtectionError::UnsafePath { .. })
        ));
        assert!(fs::read_dir(&real).unwrap().next().is_none());
        fs::remove_dir(&link).unwrap();
    }
}
