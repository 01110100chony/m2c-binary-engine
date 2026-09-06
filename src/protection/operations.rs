use std::fs::{File, Metadata};
use std::io::{self, Read, Write};
use std::path::Path;

use zeroize::Zeroizing;

use super::codec::{
    self, EnvelopeHeader, HEADER_BYTES, HKDF_SALT_BYTES, PUBLIC_KEY_FILE_BYTES,
    SECRET_KEY_FILE_BYTES, STREAM_NONCE_PREFIX_BYTES,
};
use super::crypto;
use super::publication::{StagedOutput, create_output_directory, random_staging_name};
use super::stream::{StreamParameters, decrypt_payload, encrypt_payload};
use super::{KeyGenerationOutcome, ProtectionError, ProtectionOutcome, io_error};

pub(crate) const PUBLIC_KEY_FILENAME: &str = "public.key";
pub(crate) const SECRET_KEY_FILENAME: &str = "secret.key";

fn open_regular(path: &Path, operation: &'static str) -> Result<(File, Metadata), ProtectionError> {
    let file = File::open(path).map_err(|source| io_error(operation, path, source))?;
    let metadata = file
        .metadata()
        .map_err(|source| io_error("inspect M5 input", path, source))?;
    if !metadata.is_file() {
        return Err(ProtectionError::UnsafePath {
            path: path.to_owned(),
            reason: "M5 input must be a regular file",
        });
    }
    Ok((file, metadata))
}

fn read_exact_sized(
    path: &Path,
    expected: usize,
    operation: &'static str,
) -> Result<Vec<u8>, ProtectionError> {
    let (mut file, metadata) = open_regular(path, operation)?;
    if metadata.len() != expected as u64 {
        return Err(ProtectionError::InvalidLength {
            artifact: "M5 key file",
            expected: expected as u64,
            actual: metadata.len(),
        });
    }
    let mut bytes = vec![0_u8; expected];
    finish_sized_read(&mut file, &mut bytes, path, expected)?;
    Ok(bytes)
}

fn finish_sized_read(
    file: &mut File,
    bytes: &mut [u8],
    path: &Path,
    expected: usize,
) -> Result<(), ProtectionError> {
    file.read_exact(bytes)
        .map_err(|source| io_error("read M5 key", path, source))?;
    let mut trailing = [0_u8; 1];
    loop {
        match file.read(&mut trailing) {
            Ok(0) => break,
            Ok(_) => {
                return Err(ProtectionError::InvalidLength {
                    artifact: "M5 key file",
                    expected: expected as u64,
                    actual: expected as u64 + 1,
                });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => return Err(io_error("read M5 key", path, source)),
        }
    }
    Ok(())
}

fn contextualize_key(error: ProtectionError, path: &Path) -> ProtectionError {
    match error {
        ProtectionError::InvalidKey { reason, .. } => ProtectionError::InvalidKey {
            path: path.to_owned(),
            reason,
        },
        other => other,
    }
}

fn read_public_key(path: &Path) -> Result<(Vec<u8>, ml_kem::EncapsulationKey768), ProtectionError> {
    let bytes = read_exact_sized(path, PUBLIC_KEY_FILE_BYTES, "open M5 public key")?;
    let payload = codec::decode_public_key(&bytes)?;
    let parsed =
        crypto::parse_public_key(payload).map_err(|error| contextualize_key(error, path))?;
    Ok((payload.to_vec(), parsed))
}

fn read_secret_key(
    path: &Path,
) -> Result<(Zeroizing<Vec<u8>>, ml_kem::DecapsulationKey768), ProtectionError> {
    let (mut file, metadata) = open_regular(path, "open M5 secret key")?;
    if metadata.len() != SECRET_KEY_FILE_BYTES as u64 {
        return Err(ProtectionError::InvalidLength {
            artifact: "M5 key file",
            expected: SECRET_KEY_FILE_BYTES as u64,
            actual: metadata.len(),
        });
    }
    // RAII is active before any secret bytes are read, including short-read
    // and trailing-byte error paths.
    let mut bytes = Zeroizing::new(vec![0_u8; SECRET_KEY_FILE_BYTES]);
    finish_sized_read(&mut file, &mut bytes, path, SECRET_KEY_FILE_BYTES)?;
    let payload = codec::decode_secret_key(&bytes)?;
    let parsed =
        crypto::parse_secret_key(payload).map_err(|error| contextualize_key(error, path))?;
    Ok((bytes, parsed))
}

fn write_all(
    staged: &mut StagedOutput,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), ProtectionError> {
    let path = staged.path().to_owned();
    staged
        .file_mut()?
        .write_all(bytes)
        .map_err(|source| io_error(operation, &path, source))
}

fn verify_staging_size(
    staged: &mut StagedOutput,
    expected: u64,
    artifact: &'static str,
) -> Result<(), ProtectionError> {
    let path = staged.path().to_owned();
    let actual = staged
        .file_mut()?
        .metadata()
        .map_err(|source| io_error("inspect M5 staging", &path, source))?
        .len();
    if actual != expected {
        return Err(ProtectionError::InvalidLength {
            artifact,
            expected,
            actual,
        });
    }
    Ok(())
}

/// Generate one ML-KEM-768 key pair in a new directory.
///
/// The directory must not exist. Its fixed files are `public.key` and
/// `secret.key`; no existing path is overwritten.
pub fn generate_keypair(output_dir: &Path) -> Result<KeyGenerationOutcome, ProtectionError> {
    // Generate all normal-case randomness before making the output directory visible.
    let pair = crypto::generate_keypair()?;
    let public_stage_name = random_staging_name()?;
    let secret_stage_name = random_staging_name()?;
    let public_file = codec::encode_public_key(&pair.public)?;
    let secret_file = Zeroizing::new(codec::encode_secret_key(&pair.secret)?);

    let (directory, mut warnings) = create_output_directory(output_dir)?;
    let public_path = directory.join(PUBLIC_KEY_FILENAME);
    let secret_path = directory.join(SECRET_KEY_FILENAME);
    let mut public_stage =
        StagedOutput::create_with_initial(&public_path, false, public_stage_name)?;
    let mut secret_stage =
        StagedOutput::create_with_initial(&secret_path, true, secret_stage_name)?;
    write_all(
        &mut public_stage,
        &public_file,
        "write M5 public-key staging",
    )?;
    write_all(
        &mut secret_stage,
        &secret_file,
        "write M5 secret-key staging",
    )?;
    verify_staging_size(
        &mut public_stage,
        PUBLIC_KEY_FILE_BYTES as u64,
        "M5 public key staging",
    )?;
    verify_staging_size(
        &mut secret_stage,
        SECRET_KEY_FILE_BYTES as u64,
        "M5 secret key staging",
    )?;
    let (public_key, public_warnings) = public_stage.finish()?;
    warnings.extend(public_warnings);
    let (secret_key, secret_warnings) = secret_stage.finish()?;
    warnings.extend(secret_warnings);
    Ok(KeyGenerationOutcome {
        public_key,
        secret_key,
        warnings,
    })
}

/// Protect `input` for the single ML-KEM-768 recipient in `public_key`.
///
/// `output` must be absent and outside every M4-managed namespace.
pub fn protect_file(
    input: &Path,
    public_key: &Path,
    output: &Path,
) -> Result<ProtectionOutcome, ProtectionError> {
    let (mut input_file, input_metadata) = open_regular(input, "open M5 plaintext input")?;
    let plaintext_length = input_metadata.len();
    let expected_envelope = codec::envelope_size(plaintext_length)?;
    let (public_bytes, public_key) = read_public_key(public_key)?;

    let mut salt = [0_u8; HKDF_SALT_BYTES];
    let mut nonce_prefix = [0_u8; STREAM_NONCE_PREFIX_BYTES];
    crypto::system_random(&mut salt)?;
    crypto::system_random(&mut nonce_prefix)?;
    let (kem_ciphertext, shared_secret) = crypto::encapsulate(&public_key)?;
    let content_key = crypto::derive_content_key(&salt, &shared_secret)?;
    drop(shared_secret);
    let header = EnvelopeHeader {
        plaintext_length,
        recipient_public_key_sha256: crypto::fingerprint(&public_bytes),
        hkdf_salt: salt,
        stream_nonce_prefix: nonce_prefix,
        kem_ciphertext,
    }
    .encode();

    let mut staged = StagedOutput::create(output, false)?;
    write_all(&mut staged, &header, "write M5 envelope header")?;
    let stage_path = staged.path().to_owned();
    encrypt_payload(
        &mut input_file,
        staged.file_mut()?,
        StreamParameters {
            input_path: input,
            output_path: &stage_path,
            plaintext_length,
            header: &header,
            content_key: &content_key,
            nonce_prefix: &nonce_prefix,
        },
    )?;
    verify_staging_size(&mut staged, expected_envelope, "M5 envelope staging")?;
    let (publication, warnings) = staged.finish()?;
    Ok(ProtectionOutcome {
        publication,
        warnings,
    })
}

/// Authenticate and recover one M5 v1 envelope with `secret_key`.
///
/// Plaintext is written only to private staging until every frame validates.
pub fn unprotect_file(
    input: &Path,
    secret_key: &Path,
    output: &Path,
) -> Result<ProtectionOutcome, ProtectionError> {
    let (mut input_file, input_metadata) = open_regular(input, "open M5 envelope")?;
    if input_metadata.len() < HEADER_BYTES as u64 {
        return Err(ProtectionError::InvalidLength {
            artifact: "M5 envelope",
            expected: HEADER_BYTES as u64,
            actual: input_metadata.len(),
        });
    }
    let mut header_bytes = [0_u8; HEADER_BYTES];
    input_file
        .read_exact(&mut header_bytes)
        .map_err(|source| io_error("read M5 envelope header", input, source))?;
    let header = EnvelopeHeader::decode(&header_bytes)?;
    let expected_envelope = codec::envelope_size(header.plaintext_length)?;
    if input_metadata.len() != expected_envelope {
        return Err(ProtectionError::InvalidLength {
            artifact: "M5 envelope",
            expected: expected_envelope,
            actual: input_metadata.len(),
        });
    }

    let (_secret_bytes, secret_key) = read_secret_key(secret_key)?;
    let public_bytes = crypto::public_key_bytes(&secret_key);
    if crypto::fingerprint(&public_bytes) != header.recipient_public_key_sha256 {
        return Err(ProtectionError::RecipientFingerprintMismatch);
    }
    let shared_secret = crypto::decapsulate(&secret_key, &header.kem_ciphertext);
    let content_key = crypto::derive_content_key(&header.hkdf_salt, &shared_secret)?;
    drop(shared_secret);

    let mut staged = StagedOutput::create(output, false)?;
    let stage_path = staged.path().to_owned();
    decrypt_payload(
        &mut input_file,
        staged.file_mut()?,
        StreamParameters {
            input_path: input,
            output_path: &stage_path,
            plaintext_length: header.plaintext_length,
            header: &header_bytes,
            content_key: &content_key,
            nonce_prefix: &header.stream_nonce_prefix,
        },
    )?;
    verify_staging_size(&mut staged, header.plaintext_length, "M5 plaintext staging")?;
    let (publication, warnings) = staged.finish()?;
    Ok(ProtectionOutcome {
        publication,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn decode_hex(value: &str) -> Vec<u8> {
        fn nibble(byte: u8) -> u8 {
            match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("invalid lowercase hex fixture"),
            }
        }
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
            .collect()
    }

    fn fixture_field(name: &str) -> Vec<u8> {
        let fixture = include_str!("../../tests/fixtures/m5_mlkem768_openssl.txt");
        let prefix = format!("{name}=");
        decode_hex(
            fixture
                .lines()
                .find_map(|line| line.strip_prefix(&prefix))
                .unwrap(),
        )
    }

    #[test]
    #[cfg(windows)]
    fn decoder_accepts_independently_generated_complete_envelope() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "m2c-m5-independent-envelope-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        if !super::super::windows::is_supported_ntfs(&root) {
            eprintln!("skipped: independent envelope publication requires local NTFS");
            fs::remove_dir(&root).unwrap();
            return;
        }
        let pair = crypto::keypair_from_test_seed(&fixture_field("seed")).unwrap();
        let secret_file = codec::encode_secret_key(&pair.secret).unwrap();
        let secret = root.join("secret.key");
        let envelope = root.join("independent.m5");
        let output = root.join("plaintext.bin");
        fs::write(&secret, secret_file).unwrap();
        fs::write(&envelope, fixture_field("envelope")).unwrap();
        let outcome = unprotect_file(&envelope, &secret, &output).unwrap();
        assert_eq!(
            outcome.publication,
            super::super::PublicationStatus::Published
        );
        assert_eq!(fs::read(&output).unwrap(), fixture_field("plaintext"));
        fs::remove_dir_all(root).unwrap();
    }
}
