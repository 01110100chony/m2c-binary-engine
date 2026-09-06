#![cfg(all(feature = "pqc", windows))]

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use m2c_pipeline::protection::{
    ProtectionError, ProtectionWarning, PublicationStatus, generate_keypair, protect_file,
    unprotect_file,
};

const CHUNK: usize = 1 << 20;
const HEADER: usize = 1179;
const TAG: usize = 16;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "m2c-m5-{label}-{}-{}",
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

fn generated_keys(root: &Path, name: &str) -> (PathBuf, PathBuf) {
    let directory = root.join(name);
    let outcome = generate_keypair(&directory).unwrap();
    assert_eq!(outcome.public_key, PublicationStatus::Published);
    assert_eq!(outcome.secret_key, PublicationStatus::Published);
    assert!(outcome.warnings.iter().all(|warning| matches!(
        warning,
        ProtectionWarning::PermissionRestrictionFailed { .. }
    )));
    (directory.join("public.key"), directory.join("secret.key"))
}

fn bytes(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| ((index.wrapping_mul(131).wrapping_add(17)) % 251) as u8)
        .collect()
}

fn no_staging_names(root: &Path) -> bool {
    fs::read_dir(root).unwrap().all(|entry| {
        let entry = entry.unwrap();
        !entry
            .file_name()
            .to_string_lossy()
            .starts_with(".m2c-m5-staging-")
            && (!entry.file_type().unwrap().is_dir() || no_staging_names(&entry.path()))
    })
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if entry.file_type().unwrap().is_dir() {
                result.insert(relative, b"<directory>".to_vec());
                visit(root, &path, result);
            } else {
                result.insert(relative, fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

#[test]
fn acceptance_round_trips_boundaries_and_uses_fresh_entropy() {
    let temp = TempDir::new("roundtrip");
    let (public, secret) = generated_keys(&temp.0, "keys");
    assert_eq!(fs::metadata(&public).unwrap().len(), 1200);
    assert_eq!(fs::metadata(&secret).unwrap().len(), 2416);
    assert_eq!(
        &fs::read(&public).unwrap()[..16],
        b"M2CM5PUB\0\x01\0\x01\0\0\x04\xa0"
    );
    assert_eq!(
        &fs::read(&secret).unwrap()[..16],
        b"M2CM5SEC\0\x01\0\x01\0\0\x09\x60"
    );

    for (index, plaintext) in [
        Vec::new(),
        bytes(1),
        bytes(CHUNK - 1),
        bytes(CHUNK),
        bytes(CHUNK + 1),
        bytes(3 * CHUNK + 17),
        include_bytes!("fixtures/sample_fixed.bin").to_vec(),
    ]
    .into_iter()
    .enumerate()
    {
        let input = temp.0.join(format!("input-{index}.bin"));
        let envelope = temp.0.join(format!("envelope-{index}.m5"));
        let recovered = temp.0.join(format!("recovered-{index}.bin"));
        fs::write(&input, &plaintext).unwrap();
        assert_eq!(
            protect_file(&input, &public, &envelope)
                .unwrap()
                .publication,
            PublicationStatus::Published
        );
        let frames = if plaintext.is_empty() {
            1
        } else {
            plaintext.len().div_ceil(CHUNK)
        };
        assert_eq!(
            fs::metadata(&envelope).unwrap().len() as usize,
            HEADER + plaintext.len() + TAG * frames
        );
        assert_eq!(
            unprotect_file(&envelope, &secret, &recovered)
                .unwrap()
                .publication,
            PublicationStatus::Published
        );
        assert_eq!(fs::read(recovered).unwrap(), plaintext);
    }

    let input = temp.0.join("repeat-input.bin");
    fs::write(&input, b"same plaintext").unwrap();
    let first = temp.0.join("first.m5");
    let second = temp.0.join("second.m5");
    protect_file(&input, &public, &first).unwrap();
    protect_file(&input, &public, &second).unwrap();
    assert_ne!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    for (index, envelope) in [first, second].into_iter().enumerate() {
        let recovered = temp.0.join(format!("repeat-recovered-{index}.bin"));
        unprotect_file(&envelope, &secret, &recovered).unwrap();
        assert_eq!(fs::read(recovered).unwrap(), b"same plaintext");
    }
    assert!(no_staging_names(&temp.0));

    let before_public = fs::read(&public).unwrap();
    let before_secret = fs::read(&secret).unwrap();
    assert!(matches!(
        generate_keypair(&temp.0.join("keys")),
        Err(ProtectionError::OutputAlreadyExists { .. })
    ));
    assert_eq!(fs::read(public).unwrap(), before_public);
    assert_eq!(fs::read(secret).unwrap(), before_secret);
}

fn assert_rejected_without_output(temp: &Path, secret: &Path, name: &str, envelope_bytes: &[u8]) {
    let envelope = temp.join(format!("bad-{name}.m5"));
    let output = temp.join(format!("bad-{name}.out"));
    fs::write(&envelope, envelope_bytes).unwrap();
    assert!(
        unprotect_file(&envelope, secret, &output).is_err(),
        "{name}"
    );
    assert!(!output.exists(), "{name}");
    assert!(no_staging_names(temp), "{name}");
}

#[test]
fn closeout_empty_envelope_requires_authenticated_final_tag() {
    let temp = TempDir::new("empty-auth");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let input = temp.0.join("empty");
    let envelope = temp.0.join("empty.m5");
    fs::write(&input, []).unwrap();
    protect_file(&input, &public, &envelope).unwrap();
    let original = fs::read(envelope).unwrap();
    assert_eq!(original.len(), HEADER + TAG);
    for offset in [0, 8, 10, 12, 20, 52, 84, 91, HEADER] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        assert_rejected_without_output(&temp.0, &secret, &format!("empty-{offset}"), &changed);
    }
    assert_rejected_without_output(
        &temp.0,
        &secret,
        "empty-short",
        &original[..HEADER + TAG - 1],
    );
    let mut trailing = original;
    trailing.push(0);
    assert_rejected_without_output(&temp.0, &secret, "empty-trailing", &trailing);
}

#[test]
fn closeout_huge_declared_length_rejected_before_key_or_payload_processing() {
    let temp = TempDir::new("huge-length");
    // A missing key makes validation order observable without a memory measurement.
    let secret = temp.0.join("nonexistent.key");
    for length in [1_u64 << 52, u64::MAX] {
        let mut envelope = vec![0; HEADER + TAG];
        envelope[..12].copy_from_slice(b"M2CM5ENC\0\x01\0\x01");
        envelope[12..20].copy_from_slice(&length.to_be_bytes());
        let input = temp.0.join(format!("huge-{length}.m5"));
        let output = temp.0.join(format!("huge-{length}.out"));
        fs::write(&input, envelope).unwrap();
        let error = std::panic::catch_unwind(|| unprotect_file(&input, &secret, &output))
            .expect("malformed length must not panic")
            .unwrap_err();
        if length == u64::MAX {
            assert!(matches!(error, ProtectionError::InputTooLarge { .. }));
        } else {
            assert!(matches!(
                error,
                ProtectionError::InvalidLength {
                    artifact: "M5 envelope",
                    actual: 1195,
                    ..
                }
            ));
        }
        assert!(!output.exists());
        assert!(no_staging_names(&temp.0));
    }
}

#[test]
fn closeout_late_frame_authentication_failure_never_publishes_prefix() {
    let temp = TempDir::new("late-auth");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let input = temp.0.join("input");
    let envelope = temp.0.join("valid.m5");
    let plaintext = bytes(CHUNK + 1000);
    fs::write(&input, &plaintext).unwrap();
    protect_file(&input, &public, &envelope).unwrap();
    let recovered = temp.0.join("valid.out");
    unprotect_file(&envelope, &secret, &recovered).unwrap();
    assert_eq!(fs::read(recovered).unwrap(), plaintext);
    let original = fs::read(envelope).unwrap();
    let second_frame = HEADER + CHUNK + TAG;
    let mut changed = original.clone();
    changed[second_frame] ^= 1;
    // Header and first authenticated frame are identical to the verified envelope.
    assert_eq!(&changed[..second_frame], &original[..second_frame]);
    let bad = temp.0.join("late.m5");
    let output = temp.0.join("late.out");
    fs::write(&bad, changed).unwrap();
    assert!(matches!(
        unprotect_file(&bad, &secret, &output),
        Err(ProtectionError::AuthenticationFailed)
    ));
    assert!(!output.exists());
    assert!(no_staging_names(&temp.0));
}

#[test]
fn header_body_wrong_key_and_existing_output_fail_closed() {
    let temp = TempDir::new("tamper");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let (_, wrong_secret) = generated_keys(&temp.0, "wrong-keys");
    let input = temp.0.join("input.bin");
    let envelope = temp.0.join("good.m5");
    fs::write(&input, b"authenticated plaintext").unwrap();
    protect_file(&input, &public, &envelope).unwrap();
    let original = fs::read(&envelope).unwrap();

    for (name, offset) in [
        ("magic", 0),
        ("version", 8),
        ("suite", 10),
        ("length", 12),
        ("fingerprint", 20),
        ("salt", 52),
        ("nonce", 84),
        ("kem", 91),
        ("ciphertext", HEADER),
        ("tag", original.len() - 1),
    ] {
        let mut changed = original.clone();
        changed[offset] ^= 1;
        assert_rejected_without_output(&temp.0, &secret, name, &changed);
    }
    assert_rejected_without_output(
        &temp.0,
        &secret,
        "truncated",
        &original[..original.len() - 1],
    );
    let mut trailing = original.clone();
    trailing.push(0);
    assert_rejected_without_output(&temp.0, &secret, "trailing", &trailing);

    let wrong_output = temp.0.join("wrong-key.out");
    assert!(matches!(
        unprotect_file(&envelope, &wrong_secret, &wrong_output),
        Err(ProtectionError::RecipientFingerprintMismatch)
    ));
    assert!(!wrong_output.exists());

    let incumbent = temp.0.join("incumbent.m5");
    fs::write(&incumbent, b"winner").unwrap();
    assert!(matches!(
        protect_file(&input, &public, &incumbent),
        Err(ProtectionError::OutputAlreadyExists { .. })
    ));
    assert_eq!(fs::read(incumbent).unwrap(), b"winner");
}

#[test]
fn frame_reordering_duplication_removal_and_truncation_are_rejected() {
    let temp = TempDir::new("frames");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let input = temp.0.join("input.bin");
    let envelope = temp.0.join("good.m5");
    fs::write(&input, bytes(CHUNK + 1)).unwrap();
    protect_file(&input, &public, &envelope).unwrap();
    let original = fs::read(envelope).unwrap();
    let split = HEADER + CHUNK + TAG;
    let first = &original[HEADER..split];
    let last = &original[split..];

    let mut reordered = original[..HEADER].to_vec();
    reordered.extend_from_slice(last);
    reordered.extend_from_slice(first);
    assert_rejected_without_output(&temp.0, &secret, "reordered", &reordered);

    let mut duplicated = original.clone();
    duplicated.extend_from_slice(last);
    assert_rejected_without_output(&temp.0, &secret, "duplicated", &duplicated);

    let mut removed = original[..HEADER].to_vec();
    removed.extend_from_slice(last);
    assert_rejected_without_output(&temp.0, &secret, "removed", &removed);
    assert_rejected_without_output(&temp.0, &secret, "header-only", &original[..HEADER]);
}

#[test]
fn every_m5_operation_preserves_m4_namespace_byte_for_byte() {
    let temp = TempDir::new("m4-isolation");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let input = temp.0.join("input.bin");
    fs::write(&input, b"outside M4").unwrap();
    let envelope = temp.0.join("outside.m5");
    protect_file(&input, &public, &envelope).unwrap();

    let root = temp.0.join("m4");
    fs::create_dir(&root).unwrap();
    fs::create_dir(root.join("parts")).unwrap();
    fs::create_dir(root.join("commits")).unwrap();
    fs::create_dir(root.join("parts").join("nested")).unwrap();
    fs::write(root.join(".m4.lock"), b"active or persistent lock").unwrap();
    fs::write(
        root.join("manifest.json"),
        br#"{"format":"m2c-m4","version":1,"corrupt":true}"#,
    )
    .unwrap();
    fs::write(root.join("parts").join("part.bin"), b"M4 part bytes").unwrap();
    fs::write(root.join("commits").join("part.json"), b"M4 receipt").unwrap();
    let before = snapshot(&root);

    for (index, directory) in [
        root.clone(),
        root.join("parts"),
        root.join("commits"),
        root.join("parts").join("nested"),
    ]
    .into_iter()
    .enumerate()
    {
        let protected = directory.join(format!("forbidden-{index}.m5"));
        let plaintext = directory.join(format!("forbidden-{index}.out"));
        assert!(matches!(
            protect_file(&input, &public, &protected),
            Err(ProtectionError::DestinationInM4Namespace { .. })
        ));
        assert!(matches!(
            unprotect_file(&envelope, &secret, &plaintext),
            Err(ProtectionError::DestinationInM4Namespace { .. })
        ));
    }
    assert!(matches!(
        generate_keypair(&root.join("forbidden-keys")),
        Err(ProtectionError::DestinationInM4Namespace { .. })
    ));
    assert_eq!(snapshot(&root), before);
    assert!(no_staging_names(&root));

    let adjacent = temp.0.join("adjacent");
    fs::create_dir(&adjacent).unwrap();
    let allowed = adjacent.join("allowed.m5");
    protect_file(&input, &public, &allowed).unwrap();
    assert!(allowed.exists());
    assert_eq!(snapshot(&root), before);
}

#[test]
fn malformed_corpus_and_invalid_keys_never_panic_or_publish() {
    let temp = TempDir::new("malformed");
    let (public, secret) = generated_keys(&temp.0, "keys");
    let input = temp.0.join("input.bin");
    fs::write(&input, b"input").unwrap();

    for length in [0, 1, 7, 8, 11, 20, HEADER - 1, HEADER, HEADER + TAG] {
        let envelope = temp.0.join(format!("corpus-{length}.m5"));
        let output = temp.0.join(format!("corpus-{length}.out"));
        fs::write(&envelope, bytes(length)).unwrap();
        let result = std::panic::catch_unwind(|| unprotect_file(&envelope, &secret, &output));
        assert!(result.is_ok(), "length {length}");
        assert!(result.unwrap().is_err(), "length {length}");
        assert!(!output.exists());
    }

    let mut bad_public = fs::read(&public).unwrap();
    bad_public[16..].fill(0xff);
    let bad_public_path = temp.0.join("bad-public.key");
    fs::write(&bad_public_path, bad_public).unwrap();
    let output = temp.0.join("bad-key.m5");
    assert!(matches!(
        protect_file(&input, &bad_public_path, &output),
        Err(ProtectionError::InvalidKey { .. })
    ));
    assert!(!output.exists());

    let mut bad_secret = fs::read(&secret).unwrap();
    bad_secret[2376] ^= 1;
    let bad_secret_path = temp.0.join("bad-secret.key");
    fs::write(&bad_secret_path, bad_secret).unwrap();
    let valid_envelope = temp.0.join("valid.m5");
    protect_file(&input, &public, &valid_envelope).unwrap();
    let output = temp.0.join("bad-secret.out");
    assert!(matches!(
        unprotect_file(&valid_envelope, &bad_secret_path, &output),
        Err(ProtectionError::InvalidKey { .. })
    ));
    assert!(!output.exists());
    assert!(no_staging_names(&temp.0));
}

#[test]
fn cli_keygen_protect_unprotect_and_argument_errors_are_visible() {
    let temp = TempDir::new("cli");
    let binary = env!("CARGO_BIN_EXE_m2c-pipeline");
    let keys = temp.0.join("keys with spaces");
    let keygen = Command::new(binary)
        .args([OsStr::new("keygen"), OsStr::new("--output-dir")])
        .arg(&keys)
        .output()
        .unwrap();
    assert!(
        keygen.status.success(),
        "{}",
        String::from_utf8_lossy(&keygen.stderr)
    );

    let input = temp.0.join("input with spaces.bin");
    let envelope = temp.0.join("protected with spaces.m5");
    let output = temp.0.join("recovered with spaces.bin");
    fs::write(&input, b"CLI round trip").unwrap();
    let protected = Command::new(binary)
        .args([OsStr::new("protect"), OsStr::new("--input")])
        .arg(&input)
        .arg("--public-key")
        .arg(keys.join("public.key"))
        .arg("--output")
        .arg(&envelope)
        .output()
        .unwrap();
    assert!(
        protected.status.success(),
        "{}",
        String::from_utf8_lossy(&protected.stderr)
    );
    let recovered = Command::new(binary)
        .args([OsStr::new("unprotect"), OsStr::new("--input")])
        .arg(&envelope)
        .arg("--secret-key")
        .arg(keys.join("secret.key"))
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        recovered.status.success(),
        "{}",
        String::from_utf8_lossy(&recovered.stderr)
    );
    assert_eq!(fs::read(output).unwrap(), b"CLI round trip");

    for arguments in [
        vec!["keygen"],
        vec!["protect", "--input", "x"],
        vec!["unprotect", "--unknown", "x"],
    ] {
        let invalid = Command::new(binary).args(arguments).output().unwrap();
        assert!(!invalid.status.success());
        assert!(String::from_utf8_lossy(&invalid.stderr).contains("usage:"));
    }
}
