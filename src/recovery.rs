//! Local, synchronous M4 artifact lifecycle. Final receipts are the commit authority.

use crate::manifest::{self, Completion, Manifest, Receipt};
use crate::{CompiledCopybook, ConversionError, RecordDecoder, parquet_io, source};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::basic::Compression;
use parquet::errors::ParquetError;
use serde::Serialize;
use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryMode {
    Create,
    Resume,
}

/// A failed invocation never rolls back an already committed prefix.
#[derive(Debug)]
pub enum RecoveryError {
    Conversion(ConversionError),
    Io {
        operation: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    Metadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnsupportedVersion {
        version: u32,
    },
    UnsupportedProfile {
        profile: String,
    },
    IncompatibleResume {
        component: &'static str,
    },
    Busy {
        path: PathBuf,
    },
    InvalidState {
        path: PathBuf,
        reason: &'static str,
    },
    MissingCommittedPart {
        part_index: u64,
        path: PathBuf,
    },
    CorruptCommittedPart {
        part_index: u64,
        path: PathBuf,
        reason: &'static str,
        source: Option<Box<ParquetError>>,
    },
}

impl Display for RecoveryError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conversion(error) => Display::fmt(error, f),
            Self::Io {
                operation,
                path,
                source,
            } => write!(f, "{operation} {}: {source}", path.display()),
            Self::Metadata { path, source } => {
                write!(f, "invalid recovery metadata {}: {source}", path.display())
            }
            Self::UnsupportedVersion { version } => {
                write!(f, "unsupported recovery version {version}")
            }
            Self::UnsupportedProfile { profile } => {
                write!(f, "unsupported recovery profile {profile}")
            }
            Self::IncompatibleResume { component } => write!(f, "incompatible resume: {component}"),
            Self::Busy { path } => write!(f, "conversion is busy: {}", path.display()),
            Self::InvalidState { path, reason } => {
                write!(f, "invalid recovery state {}: {reason}", path.display())
            }
            Self::MissingCommittedPart { part_index, path } => {
                write!(f, "missing committed part {part_index}: {}", path.display())
            }
            Self::CorruptCommittedPart {
                part_index,
                path,
                reason,
                source,
            } => {
                write!(
                    f,
                    "corrupt committed part {part_index} {}: {reason}",
                    path.display()
                )?;
                if let Some(source) = source {
                    write!(f, ": {source}")?;
                }
                Ok(())
            }
        }
    }
}
impl Error for RecoveryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Conversion(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Metadata { source, .. } => Some(source),
            Self::CorruptCommittedPart {
                source: Some(source),
                ..
            } => Some(source.as_ref()),
            _ => None,
        }
    }
}
impl From<ConversionError> for RecoveryError {
    fn from(error: ConversionError) -> Self {
        Self::Conversion(error)
    }
}
impl From<ParquetError> for RecoveryError {
    fn from(error: ParquetError) -> Self {
        Self::Conversion(error.into())
    }
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> RecoveryError {
    RecoveryError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}
fn invalid(path: &Path, reason: &'static str) -> RecoveryError {
    RecoveryError::InvalidState {
        path: path.to_owned(),
        reason,
    }
}
fn contextualize(error: RecoveryError, path: &Path) -> RecoveryError {
    match error {
        RecoveryError::InvalidState { reason, .. } => invalid(path, reason),
        other => other,
    }
}

fn stat(path: &Path) -> Result<Option<Metadata>, RecoveryError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes() & 0x400 != 0 {
                    return Err(invalid(path, "reparse points are not allowed"));
                }
            }
            if metadata.file_type().is_symlink() {
                return Err(invalid(path, "symbolic links are not allowed"));
            }
            Ok(Some(metadata))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error("inspect artifact", path, error)),
    }
}
fn require_type(path: &Path, directory: bool) -> Result<(), RecoveryError> {
    match stat(path)? {
        Some(meta)
            if if directory {
                meta.is_dir()
            } else {
                meta.is_file()
            } =>
        {
            Ok(())
        }
        _ => Err(invalid(
            path,
            if directory {
                "expected a directory"
            } else {
                "expected a regular file"
            },
        )),
    }
}
fn entries(path: &Path) -> Result<fs::ReadDir, RecoveryError> {
    fs::read_dir(path).map_err(|error| io_error("read directory", path, error))
}
fn entry_path(entry: io::Result<fs::DirEntry>, dir: &Path) -> Result<PathBuf, RecoveryError> {
    entry
        .map(|entry| entry.path())
        .map_err(|error| io_error("read directory entry", dir, error))
}
fn basename(path: &Path) -> Result<&str, RecoveryError> {
    path.file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid(path, "unknown artifact name"))
}
fn part_name(index: u64, extension: &str) -> String {
    format!("part-{index:020}.{extension}")
}
fn parse_part_name(name: &str, extension: &str) -> Option<(u64, bool)> {
    let (name, temporary) = match name.strip_prefix('.') {
        Some(name) => (name.strip_suffix(".tmp")?, true),
        None => (name, false),
    };
    let digits = name
        .strip_prefix("part-")?
        .strip_suffix(extension)?
        .strip_suffix('.')?;
    if digits.len() != 20 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, temporary))
}

/// No descriptor means no persisted job identity, and only an empty bootstrap is recoverable.
fn validate_bootstrap(root: &Path) -> Result<(), RecoveryError> {
    for entry in entries(root)? {
        let path = entry_path(entry, root)?;
        match basename(&path)? {
            ".m4.lock" | ".manifest.json.tmp" => require_type(&path, false)?,
            "parts" | "commits" => {
                require_type(&path, true)?;
                if let Some(entry) = entries(&path)?.next() {
                    entry_path(entry, &path)?;
                    return Err(invalid(&path, "bootstrap directory must be empty"));
                }
            }
            _ => return Err(invalid(&path, "unexpected bootstrap artifact")),
        }
    }
    Ok(())
}

fn lock(root: &Path) -> Result<File, RecoveryError> {
    let path = root.join(".m4.lock");
    if stat(&path)?.is_none() {
        // Never recreate a missing lock in an initialized job. The check also
        // prevents writing a lock into an unrelated directory on rejected resume.
        validate_bootstrap(root)?;
    } else {
        require_type(&path, false)?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| io_error("open lock", &path, error))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(RecoveryError::Busy { path }),
        Err(TryLockError::Error(error)) => Err(io_error("lock conversion", &path, error)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Artifact {
    Bootstrap,
    Manifest,
    Part,
    Receipt,
    Completion,
    Cleanup,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    AfterCreate,
    BeforeStaging,
    Write,
    BeforeFinish,
    AfterFinish,
    BeforeSync,
    AfterSync,
    BeforePublish,
    AfterPublish,
    BeforeNext,
    BeforeRemove,
    AfterRemove,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Point {
    artifact: Artifact,
    stage: Stage,
    index: Option<u64>,
}
impl Point {
    fn new(artifact: Artifact, stage: Stage, index: Option<u64>) -> Self {
        Self {
            artifact,
            stage,
            index,
        }
    }
}

// Fault selection exists only in the unit-test executable, never in the CLI.
#[derive(Default)]
struct Faults {
    #[cfg(test)]
    config: Option<tests::FaultConfig>,
}
impl Faults {
    fn hit(&mut self, point: Point) -> io::Result<()> {
        #[cfg(test)]
        if let Some(config) = self.config.as_mut() {
            return config.hit(point, None);
        }
        let _ = point;
        Ok(())
    }
    fn write_limit(&self, point: Point, written: u64, requested: usize) -> usize {
        #[cfg(test)]
        if let Some(config) = self.config.as_ref() {
            return config.write_limit(point, written, requested);
        }
        let _ = (point, written);
        requested
    }
    fn wrote(&mut self, point: Point, written: u64) -> io::Result<()> {
        #[cfg(test)]
        if let Some(config) = self.config.as_mut() {
            return config.hit(point, Some(written));
        }
        let _ = (point, written);
        Ok(())
    }
}
struct ObservedWriter<'a> {
    file: &'a mut File,
    faults: &'a mut Faults,
    point: Point,
    written: u64,
}
impl Write for ObservedWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self
            .faults
            .write_limit(self.point, self.written, bytes.len());
        let count = self.file.write(&bytes[..count])?;
        self.written = self
            .written
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("write offset overflow"))?;
        self.faults.wrote(self.point, self.written)?;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
fn hit(
    faults: &mut Faults,
    artifact: Artifact,
    stage: Stage,
    index: Option<u64>,
    path: &Path,
) -> Result<(), RecoveryError> {
    faults
        .hit(Point::new(artifact, stage, index))
        .map_err(|error| io_error("artifact lifecycle", path, error))
}
fn create_stage(path: &Path) -> Result<File, RecoveryError> {
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create staging", path, error))
}
fn publish(
    from: &Path,
    to: &Path,
    artifact: Artifact,
    index: Option<u64>,
    faults: &mut Faults,
) -> Result<(), RecoveryError> {
    hit(faults, artifact, Stage::BeforePublish, index, to)?;
    if stat(to)?.is_some() {
        return Err(invalid(to, "publication destination already exists"));
    }
    // A held OS lock serializes cooperating writers; external mutation is outside
    // the contract. Both names are in the same directory and filesystem.
    fs::rename(from, to).map_err(|error| io_error("publish artifact", to, error))?;
    hit(faults, artifact, Stage::AfterPublish, index, to)
}
fn publish_json<T: Serialize>(
    root: &Path,
    name: &str,
    value: &T,
    artifact: Artifact,
    index: Option<u64>,
    faults: &mut Faults,
) -> Result<(), RecoveryError> {
    let bytes = manifest::json_bytes(value)?;
    let staged = root.join(format!(".{name}.tmp"));
    let final_path = root.join(name);
    hit(faults, artifact, Stage::BeforeStaging, index, &staged)?;
    let mut file = create_stage(&staged)?;
    let mut sink = ObservedWriter {
        file: &mut file,
        faults,
        point: Point::new(artifact, Stage::Write, index),
        written: 0,
    };
    sink.write_all(&bytes)
        .map_err(|error| io_error("write recovery metadata", &staged, error))?;
    hit(faults, artifact, Stage::BeforeSync, index, &staged)?;
    file.sync_all()
        .map_err(|error| io_error("sync recovery metadata", &staged, error))?;
    hit(faults, artifact, Stage::AfterSync, index, &staged)?;
    drop(file);
    publish(&staged, &final_path, artifact, index, faults)
}

fn verify_parquet(
    path: &Path,
    layout: &CompiledCopybook,
    rows: u64,
    index: u64,
) -> Result<(), RecoveryError> {
    let corrupt = |reason| RecoveryError::CorruptCommittedPart {
        part_index: index,
        path: path.to_owned(),
        reason,
        source: None,
    };
    let file = File::open(path).map_err(|error| io_error("open Parquet part", path, error))?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|source| {
        RecoveryError::CorruptCommittedPart {
            part_index: index,
            path: path.to_owned(),
            reason: "invalid Parquet footer",
            source: Some(Box::new(source)),
        }
    })?;
    let metadata = reader.metadata();
    if reader.schema().as_ref() != &layout.arrow_schema {
        return Err(corrupt("schema differs from compiled layout"));
    }
    if u64::try_from(metadata.file_metadata().num_rows()).ok() != Some(rows)
        || metadata.num_row_groups() != usize::from(rows != 0)
        || metadata.row_groups().iter().any(|group| {
            u64::try_from(group.num_rows()).ok() != Some(rows)
                || group
                    .columns()
                    .iter()
                    .any(|column| column.compression() != Compression::UNCOMPRESSED)
        })
    {
        return Err(corrupt(
            "row count, row groups or compression differs from profile",
        ));
    }
    Ok(())
}
fn part_hash(path: &Path) -> Result<(u64, String), RecoveryError> {
    let mut file =
        File::open(path).map_err(|error| io_error("open part for hashing", path, error))?;
    manifest::hash_reader(&mut file).map_err(|error| io_error("hash part", path, error))
}

/// Validate the entire namespace and confirmed prefix before any cleanup.
fn validate_state(
    root: &Path,
    job: &Manifest,
    layout: &CompiledCopybook,
) -> Result<(u64, bool), RecoveryError> {
    for entry in entries(root)? {
        let path = entry_path(entry, root)?;
        match basename(&path)? {
            ".m4.lock" | "manifest.json" | ".manifest.json.tmp" | "complete.json"
            | ".complete.json.tmp" => require_type(&path, false)?,
            "parts" | "commits" => require_type(&path, true)?,
            _ => return Err(invalid(&path, "unknown artifact")),
        }
    }
    let commits = root.join("commits");
    let parts = root.join("parts");
    require_type(&commits, true)?;
    require_type(&parts, true)?;
    let (mut count, mut max) = (0_u64, None::<u64>);
    for entry in entries(&commits)? {
        let path = entry_path(entry, &commits)?;
        require_type(&path, false)?;
        let (index, temporary) = parse_part_name(basename(&path)?, "json")
            .ok_or_else(|| invalid(&path, "unknown receipt name"))?;
        if index >= job.part_count() {
            return Err(invalid(&path, "receipt index outside input"));
        }
        if !temporary {
            count = count
                .checked_add(1)
                .ok_or(ConversionError::CapacityExceeded)?;
            max = Some(max.map_or(index, |previous| previous.max(index)));
        }
    }
    if max.and_then(|index| index.checked_add(1)).unwrap_or(0) != count {
        return Err(invalid(&commits, "committed receipt sequence has a gap"));
    }
    for entry in entries(&parts)? {
        let path = entry_path(entry, &parts)?;
        require_type(&path, false)?;
        let (index, temporary) = parse_part_name(basename(&path)?, "parquet")
            .ok_or_else(|| invalid(&path, "unknown part name"))?;
        if index >= job.part_count() || (!temporary && index > count) {
            return Err(invalid(&path, "unexpected part beyond committed prefix"));
        }
    }
    for index in 0..count {
        let receipt_path = commits.join(part_name(index, "json"));
        let receipt: Receipt = manifest::read_json(&receipt_path)?;
        receipt
            .validate(job, index)
            .map_err(|error| contextualize(error, &receipt_path))?;
        let path = parts.join(part_name(index, "parquet"));
        if stat(&path)?.is_none() {
            return Err(RecoveryError::MissingCommittedPart {
                part_index: index,
                path,
            });
        }
        let (length, digest) = part_hash(&path)?;
        if length != receipt.parquet_bytes || digest != receipt.parquet_sha256 {
            return Err(RecoveryError::CorruptCommittedPart {
                part_index: index,
                path,
                reason: "size or SHA-256 differs from receipt",
                source: None,
            });
        }
        verify_parquet(&path, layout, receipt.record_count, index)?;
    }
    let complete = root.join("complete.json");
    let completed = stat(&complete)?.is_some();
    if completed {
        let marker: Completion = manifest::read_json(&complete)?;
        marker
            .validate(job)
            .map_err(|error| contextualize(error, &complete))?;
        if count != job.part_count() {
            return Err(invalid(&complete, "completion precedes all part commits"));
        }
    }
    Ok((count, completed))
}

fn remove_staged(
    path: &Path,
    index: Option<u64>,
    faults: &mut Faults,
) -> Result<(), RecoveryError> {
    require_type(path, false)?;
    hit(faults, Artifact::Cleanup, Stage::BeforeRemove, index, path)?;
    fs::remove_file(path).map_err(|error| io_error("remove uncommitted artifact", path, error))?;
    hit(faults, Artifact::Cleanup, Stage::AfterRemove, index, path)
}
fn cleanup(root: &Path, count: u64, faults: &mut Faults) -> Result<(), RecoveryError> {
    for name in [".manifest.json.tmp", ".complete.json.tmp"] {
        let path = root.join(name);
        if stat(&path)?.is_some() {
            remove_staged(&path, None, faults)?;
        }
    }
    for (directory, extension) in [("parts", "parquet"), ("commits", "json")] {
        let dir = root.join(directory);
        for entry in entries(&dir)? {
            let path = entry_path(entry, &dir)?;
            let (index, temporary) = parse_part_name(basename(&path)?, extension)
                .ok_or_else(|| invalid(&path, "unknown artifact during cleanup"))?;
            if temporary || (directory == "parts" && index == count) {
                remove_staged(&path, Some(index), faults)?;
            }
        }
    }
    Ok(())
}

/// Convert to deterministic Parquet parts, or resume a validated committed prefix.
///
/// The input must remain immutable during an invocation. Recovery covers process
/// interruption on a local filesystem with atomic same-directory rename; it does
/// not promise recovery from power loss or external mutation of the output tree.
/// Existing M3 single-file conversion remains a separate operation.
pub fn convert_parts(
    layout: &CompiledCopybook,
    input: &Path,
    output_dir: &Path,
    batch_records: usize,
    mode: RecoveryMode,
) -> Result<(), RecoveryError> {
    convert_with_faults(
        layout,
        input,
        output_dir,
        batch_records,
        mode,
        &mut Faults::default(),
    )
}

fn convert_with_faults(
    layout: &CompiledCopybook,
    input: &Path,
    output_dir: &Path,
    batch_records: usize,
    mode: RecoveryMode,
    faults: &mut Faults,
) -> Result<(), RecoveryError> {
    let decoder = RecordDecoder::try_new(layout).map_err(|source| ConversionError::Decode {
        batch_offset: 0,
        source,
    })?;
    if layout.arrow_schema.fields().is_empty() {
        return Err(ConversionError::EmptySchema.into());
    }
    let mut buffer = source::batch_buffer(layout.record_length, batch_records)?;
    let input_path =
        fs::canonicalize(input).map_err(|error| io_error("resolve input", input, error))?;
    let root = match mode {
        RecoveryMode::Resume => {
            require_type(output_dir, true)?;
            fs::canonicalize(output_dir)
                .map_err(|error| io_error("resolve output directory", output_dir, error))?
        }
        RecoveryMode::Create => {
            let parent = output_dir
                .parent()
                .filter(|path| !path.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let parent = fs::canonicalize(parent)
                .map_err(|error| io_error("resolve output parent", parent, error))?;
            let name = output_dir
                .file_name()
                .ok_or_else(|| invalid(output_dir, "output must name a new directory"))?;
            parent.join(name)
        }
    };
    if input_path.starts_with(&root) {
        return Err(invalid(input, "input must be outside the output directory"));
    }
    // Resume holds the lock throughout identity validation, inspection and cleanup.
    let mut guard = if mode == RecoveryMode::Resume {
        Some(lock(&root)?)
    } else {
        None
    };
    let mut file = File::open(&input_path).map_err(|error| io_error("open input", input, error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect input", input, error))?;
    if !metadata.is_file() {
        return Err(invalid(input, "input must be a regular file"));
    }
    let (input_bytes, input_sha256) =
        manifest::hash_reader(&mut file).map_err(|error| io_error("hash input", input, error))?;
    if input_bytes != metadata.len()
        || file
            .metadata()
            .map_err(|error| io_error("inspect input", input, error))?
            .len()
            != input_bytes
    {
        return Err(invalid(input, "input changed during identity validation"));
    }
    let record_length =
        u64::try_from(layout.record_length).map_err(|_| ConversionError::CapacityExceeded)?;
    let remainder = input_bytes % record_length;
    if remainder != 0 {
        return Err(ConversionError::TruncatedRecord {
            byte_offset: input_bytes - remainder,
            actual: usize::try_from(remainder).map_err(|_| ConversionError::CapacityExceeded)?,
            record_length: layout.record_length,
        }
        .into());
    }
    let expected = Manifest::new(
        input_bytes,
        input_sha256,
        manifest::layout_hash(layout)?,
        record_length,
        u64::try_from(batch_records).map_err(|_| ConversionError::CapacityExceeded)?,
    )?;
    if mode == RecoveryMode::Create {
        fs::create_dir(&root).map_err(|error| io_error("create output directory", &root, error))?;
        hit(faults, Artifact::Bootstrap, Stage::AfterCreate, None, &root)?;
        guard = Some(lock(&root)?);
    }
    let manifest_path = root.join("manifest.json");
    let job = if stat(&manifest_path)?.is_some() {
        require_type(&manifest_path, false)?;
        let job: Manifest = manifest::read_json(&manifest_path)?;
        job.validate()
            .map_err(|error| contextualize(error, &manifest_path))?;
        for (matches, component) in [
            (
                job.input_bytes == expected.input_bytes
                    && job.input_sha256 == expected.input_sha256,
                "input",
            ),
            (
                job.layout_sha256 == expected.layout_sha256
                    && job.record_length == expected.record_length,
                "layout/schema",
            ),
            (job.batch_records == expected.batch_records, "batch-records"),
        ] {
            if !matches {
                return Err(RecoveryError::IncompatibleResume { component });
            }
        }
        job
    } else {
        validate_bootstrap(&root)?;
        let temporary = root.join(".manifest.json.tmp");
        if stat(&temporary)?.is_some() {
            remove_staged(&temporary, None, faults)?;
        }
        for name in ["parts", "commits"] {
            let path = root.join(name);
            if stat(&path)?.is_none() {
                fs::create_dir(&path)
                    .map_err(|error| io_error("create artifact directory", &path, error))?;
            }
        }
        publish_json(
            &root,
            "manifest.json",
            &expected,
            Artifact::Manifest,
            None,
            faults,
        )?;
        expected
    };
    let (count, completed) = validate_state(&root, &job, layout)?;
    cleanup(&root, count, faults)?;
    if completed {
        return Ok(());
    }
    let cursor = if count == job.part_count() {
        input_bytes
    } else {
        job.range(count)?
            .0
            .checked_mul(record_length)
            .ok_or(ConversionError::CapacityExceeded)?
    };
    file.seek(SeekFrom::Start(cursor))
        .map_err(|error| io_error("seek input", input, error))?;
    for index in count..job.part_count() {
        let (start_record, rows) = job.range(index)?;
        let byte_offset = start_record
            .checked_mul(record_length)
            .ok_or(ConversionError::CapacityExceeded)?;
        let expected_bytes = rows
            .checked_mul(record_length)
            .ok_or(ConversionError::CapacityExceeded)?;
        let length = source::read_batch(&mut file, &mut buffer)
            .map_err(|error| io_error("read input", input, error))?;
        if u64::try_from(length).map_err(|_| ConversionError::CapacityExceeded)? != expected_bytes {
            return Err(invalid(
                input,
                "input length changed after identity validation",
            ));
        }
        let batch = match decoder.decode_batch(&buffer[..length]) {
            Ok(batch) => batch,
            Err(mut source) => {
                if let Some(context) = source.context.as_mut() {
                    let first = usize::try_from(start_record)
                        .map_err(|_| ConversionError::CapacityExceeded)?;
                    context.record_index = first
                        .checked_add(context.record_index)
                        .ok_or(ConversionError::CapacityExceeded)?;
                }
                return Err(ConversionError::Decode {
                    batch_offset: byte_offset,
                    source,
                }
                .into());
            }
        };
        let name = part_name(index, "parquet");
        let parts = root.join("parts");
        let staged = parts.join(format!(".{name}.tmp"));
        hit(
            faults,
            Artifact::Part,
            Stage::BeforeStaging,
            Some(index),
            &staged,
        )?;
        let mut part_file = create_stage(&staged)?;
        {
            let sink = ObservedWriter {
                file: &mut part_file,
                faults,
                point: Point::new(Artifact::Part, Stage::Write, Some(index)),
                written: 0,
            };
            let mut writer = parquet_io::writer_from(
                sink,
                Arc::new(layout.arrow_schema.clone()),
                batch_records,
            )?;
            if rows != 0 {
                writer.write(&batch)?;
                writer.flush()?;
            }
            writer
                .inner_mut()
                .faults
                .hit(Point::new(Artifact::Part, Stage::BeforeFinish, Some(index)))
                .map_err(|error| io_error("finalize part", &staged, error))?;
            writer.close()?;
        }
        hit(
            faults,
            Artifact::Part,
            Stage::AfterFinish,
            Some(index),
            &staged,
        )?;
        hit(
            faults,
            Artifact::Part,
            Stage::BeforeSync,
            Some(index),
            &staged,
        )?;
        part_file
            .sync_all()
            .map_err(|error| io_error("sync Parquet part", &staged, error))?;
        hit(
            faults,
            Artifact::Part,
            Stage::AfterSync,
            Some(index),
            &staged,
        )?;
        drop(part_file);
        let (parquet_bytes, parquet_sha256) = part_hash(&staged)?;
        verify_parquet(&staged, layout, rows, index)?;
        publish(
            &staged,
            &parts.join(name),
            Artifact::Part,
            Some(index),
            faults,
        )?;
        let receipt = Receipt {
            version: manifest::VERSION,
            job_id: job.job_id.clone(),
            part_index: index,
            start_record,
            record_count: rows,
            parquet_bytes,
            parquet_sha256,
        };
        publish_json(
            &root.join("commits"),
            &part_name(index, "json"),
            &receipt,
            Artifact::Receipt,
            Some(index),
            faults,
        )?;
        hit(
            faults,
            Artifact::Part,
            Stage::BeforeNext,
            Some(index),
            &root,
        )?;
    }
    if source::read_batch(&mut file, &mut [0_u8; 1])
        .map_err(|error| io_error("check input EOF", input, error))?
        != 0
    {
        return Err(invalid(input, "input grew after identity validation"));
    }
    let complete = Completion {
        version: manifest::VERSION,
        job_id: job.job_id.clone(),
        part_count: job.part_count(),
        total_records: job.total_records(),
    };
    publish_json(
        &root,
        "complete.json",
        &complete,
        Artifact::Completion,
        None,
        faults,
    )?;
    drop(guard);
    Ok(())
}

#[cfg(test)]
#[path = "m6_recovery_tests.rs"]
mod m6_tests;
#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
