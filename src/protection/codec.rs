use super::ProtectionError;

pub(crate) const VERSION: u16 = 1;
pub(crate) const ALGORITHM_ID: u16 = 1;
pub(crate) const SUITE_ID: u16 = 1;
pub(crate) const PUBLIC_KEY_BYTES: usize = 1184;
pub(crate) const SECRET_KEY_BYTES: usize = 2400;
pub(crate) const KEM_CIPHERTEXT_BYTES: usize = 1088;
pub(crate) const PUBLIC_KEY_FILE_BYTES: usize = 1200;
pub(crate) const SECRET_KEY_FILE_BYTES: usize = 2416;
pub(crate) const HEADER_BYTES: usize = 1179;
pub(crate) const HKDF_SALT_BYTES: usize = 32;
pub(crate) const STREAM_NONCE_PREFIX_BYTES: usize = 7;
pub(crate) const TAG_BYTES: u64 = 16;
pub(crate) const CHUNK_BYTES: u64 = 1 << 20;
pub(crate) const FRAME_MAX: u64 = 1_u64 << 32;
pub(crate) const PLAINTEXT_MAX: u64 = CHUNK_BYTES * FRAME_MAX;

const PUBLIC_MAGIC: &[u8; 8] = b"M2CM5PUB";
const SECRET_MAGIC: &[u8; 8] = b"M2CM5SEC";
const ENVELOPE_MAGIC: &[u8; 8] = b"M2CM5ENC";

#[derive(Clone)]
pub(crate) struct EnvelopeHeader {
    pub(crate) plaintext_length: u64,
    pub(crate) recipient_public_key_sha256: [u8; 32],
    pub(crate) hkdf_salt: [u8; HKDF_SALT_BYTES],
    pub(crate) stream_nonce_prefix: [u8; STREAM_NONCE_PREFIX_BYTES],
    pub(crate) kem_ciphertext: [u8; KEM_CIPHERTEXT_BYTES],
}

impl EnvelopeHeader {
    pub(crate) fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut bytes = [0_u8; HEADER_BYTES];
        bytes[0..8].copy_from_slice(ENVELOPE_MAGIC);
        bytes[8..10].copy_from_slice(&VERSION.to_be_bytes());
        bytes[10..12].copy_from_slice(&SUITE_ID.to_be_bytes());
        bytes[12..20].copy_from_slice(&self.plaintext_length.to_be_bytes());
        bytes[20..52].copy_from_slice(&self.recipient_public_key_sha256);
        bytes[52..84].copy_from_slice(&self.hkdf_salt);
        bytes[84..91].copy_from_slice(&self.stream_nonce_prefix);
        bytes[91..1179].copy_from_slice(&self.kem_ciphertext);
        bytes
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, ProtectionError> {
        if bytes.len() != HEADER_BYTES {
            return Err(ProtectionError::InvalidLength {
                artifact: "M5 envelope header",
                expected: HEADER_BYTES as u64,
                actual: bytes.len() as u64,
            });
        }
        if &bytes[0..8] != ENVELOPE_MAGIC {
            return Err(ProtectionError::InvalidMagic {
                artifact: "M5 envelope",
            });
        }
        let version = u16::from_be_bytes([bytes[8], bytes[9]]);
        if version != VERSION {
            return Err(ProtectionError::UnsupportedVersion {
                artifact: "M5 envelope",
                version,
            });
        }
        let suite = u16::from_be_bytes([bytes[10], bytes[11]]);
        if suite != SUITE_ID {
            return Err(ProtectionError::UnsupportedSuite { suite });
        }
        let mut length = [0_u8; 8];
        length.copy_from_slice(&bytes[12..20]);
        let plaintext_length = u64::from_be_bytes(length);
        frame_count(plaintext_length)?;
        Ok(Self {
            plaintext_length,
            recipient_public_key_sha256: bytes[20..52].try_into().map_err(|_| {
                ProtectionError::ArithmeticOverflow {
                    operation: "recipient fingerprint",
                }
            })?,
            hkdf_salt: bytes[52..84].try_into().map_err(|_| {
                ProtectionError::ArithmeticOverflow {
                    operation: "HKDF salt",
                }
            })?,
            stream_nonce_prefix: bytes[84..91].try_into().map_err(|_| {
                ProtectionError::ArithmeticOverflow {
                    operation: "STREAM nonce prefix",
                }
            })?,
            kem_ciphertext: bytes[91..1179].try_into().map_err(|_| {
                ProtectionError::ArithmeticOverflow {
                    operation: "ML-KEM ciphertext",
                }
            })?,
        })
    }
}

pub(crate) fn encode_public_key(payload: &[u8]) -> Result<Vec<u8>, ProtectionError> {
    encode_key(PUBLIC_MAGIC, PUBLIC_KEY_BYTES, payload)
}

pub(crate) fn encode_secret_key(payload: &[u8]) -> Result<Vec<u8>, ProtectionError> {
    encode_key(SECRET_MAGIC, SECRET_KEY_BYTES, payload)
}

fn encode_key(
    magic: &[u8; 8],
    expected_payload: usize,
    payload: &[u8],
) -> Result<Vec<u8>, ProtectionError> {
    if payload.len() != expected_payload {
        return Err(ProtectionError::InvalidLength {
            artifact: "M5 key payload",
            expected: expected_payload as u64,
            actual: payload.len() as u64,
        });
    }
    let length =
        u32::try_from(expected_payload).map_err(|_| ProtectionError::ArithmeticOverflow {
            operation: "M5 key payload length",
        })?;
    let capacity =
        16_usize
            .checked_add(expected_payload)
            .ok_or(ProtectionError::ArithmeticOverflow {
                operation: "M5 key file length",
            })?;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(magic);
    bytes.extend_from_slice(&VERSION.to_be_bytes());
    bytes.extend_from_slice(&ALGORITHM_ID.to_be_bytes());
    bytes.extend_from_slice(&length.to_be_bytes());
    bytes.extend_from_slice(payload);
    Ok(bytes)
}

pub(crate) fn decode_public_key(bytes: &[u8]) -> Result<&[u8], ProtectionError> {
    decode_key(
        bytes,
        PUBLIC_MAGIC,
        PUBLIC_KEY_BYTES,
        PUBLIC_KEY_FILE_BYTES,
        "M5 public key",
    )
}

pub(crate) fn decode_secret_key(bytes: &[u8]) -> Result<&[u8], ProtectionError> {
    decode_key(
        bytes,
        SECRET_MAGIC,
        SECRET_KEY_BYTES,
        SECRET_KEY_FILE_BYTES,
        "M5 secret key",
    )
}

fn decode_key<'a>(
    bytes: &'a [u8],
    magic: &[u8; 8],
    payload_bytes: usize,
    file_bytes: usize,
    artifact: &'static str,
) -> Result<&'a [u8], ProtectionError> {
    if bytes.len() != file_bytes {
        return Err(ProtectionError::InvalidLength {
            artifact,
            expected: file_bytes as u64,
            actual: bytes.len() as u64,
        });
    }
    if &bytes[0..8] != magic {
        return Err(ProtectionError::InvalidMagic { artifact });
    }
    let version = u16::from_be_bytes([bytes[8], bytes[9]]);
    if version != VERSION {
        return Err(ProtectionError::UnsupportedVersion { artifact, version });
    }
    let algorithm = u16::from_be_bytes([bytes[10], bytes[11]]);
    if algorithm != ALGORITHM_ID {
        return Err(ProtectionError::UnsupportedAlgorithm { algorithm });
    }
    let declared = u32::from_be_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if declared != payload_bytes as u32 {
        return Err(ProtectionError::InvalidLength {
            artifact: "declared M5 key payload",
            expected: payload_bytes as u64,
            actual: u64::from(declared),
        });
    }
    Ok(&bytes[16..])
}

pub(crate) fn frame_count(plaintext_length: u64) -> Result<u64, ProtectionError> {
    if plaintext_length > PLAINTEXT_MAX {
        return Err(ProtectionError::InputTooLarge {
            length: plaintext_length,
            maximum: PLAINTEXT_MAX,
        });
    }
    if plaintext_length == 0 {
        return Ok(1);
    }
    plaintext_length
        .checked_sub(1)
        .and_then(|value| value.checked_div(CHUNK_BYTES))
        .and_then(|value| value.checked_add(1))
        .filter(|&frames| frames <= FRAME_MAX)
        .ok_or(ProtectionError::ArithmeticOverflow {
            operation: "STREAM-BE32 frame count",
        })
}

pub(crate) fn envelope_size(plaintext_length: u64) -> Result<u64, ProtectionError> {
    let frames = frame_count(plaintext_length)?;
    checked_envelope_size(plaintext_length, frames)
}

fn checked_envelope_size(plaintext_length: u64, frames: u64) -> Result<u64, ProtectionError> {
    let tags = TAG_BYTES
        .checked_mul(frames)
        .ok_or(ProtectionError::ArithmeticOverflow {
            operation: "M5 authentication tag bytes",
        })?;
    (HEADER_BYTES as u64)
        .checked_add(plaintext_length)
        .and_then(|value| value.checked_add(tags))
        .ok_or(ProtectionError::ArithmeticOverflow {
            operation: "M5 envelope length",
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(length: u64) -> EnvelopeHeader {
        EnvelopeHeader {
            plaintext_length: length,
            recipient_public_key_sha256: [0x11; 32],
            hkdf_salt: [0x22; 32],
            stream_nonce_prefix: [0x33; 7],
            kem_ciphertext: [0x44; KEM_CIPHERTEXT_BYTES],
        }
    }

    #[test]
    fn key_codecs_have_exact_big_endian_layout_and_reject_mutations() {
        for (secret, payload_len, file_len, magic) in [
            (false, PUBLIC_KEY_BYTES, PUBLIC_KEY_FILE_BYTES, b"M2CM5PUB"),
            (true, SECRET_KEY_BYTES, SECRET_KEY_FILE_BYTES, b"M2CM5SEC"),
        ] {
            let payload = vec![0x5a; payload_len];
            let bytes = if secret {
                encode_secret_key(&payload).unwrap()
            } else {
                encode_public_key(&payload).unwrap()
            };
            assert_eq!(bytes.len(), file_len);
            assert_eq!(&bytes[..8], magic);
            assert_eq!(&bytes[8..10], &[0, 1]);
            assert_eq!(&bytes[10..12], &[0, 1]);
            assert_eq!(
                u32::from_be_bytes(bytes[12..16].try_into().unwrap()),
                payload_len as u32
            );
            let decoded = if secret {
                decode_secret_key(&bytes)
            } else {
                decode_public_key(&bytes)
            }
            .unwrap();
            assert_eq!(decoded, payload);

            for index in [0, 8, 10, 12] {
                let mut changed = bytes.clone();
                changed[index] ^= 1;
                assert!(
                    (if secret {
                        decode_secret_key(&changed)
                    } else {
                        decode_public_key(&changed)
                    })
                    .is_err()
                );
            }
            let mut trailing = bytes.clone();
            trailing.push(0);
            assert!(
                (if secret {
                    decode_secret_key(&trailing)
                } else {
                    decode_public_key(&trailing)
                })
                .is_err()
            );
            assert!(
                (if secret {
                    decode_secret_key(&bytes[..bytes.len() - 1])
                } else {
                    decode_public_key(&bytes[..bytes.len() - 1])
                })
                .is_err()
            );
        }
    }

    #[test]
    fn envelope_header_has_exact_offsets_and_rejects_control_mutations() {
        let bytes = header(0x0000_0102_0304_0506).encode();
        assert_eq!(bytes.len(), HEADER_BYTES);
        assert_eq!(&bytes[0..8], b"M2CM5ENC");
        assert_eq!(&bytes[8..12], &[0, 1, 0, 1]);
        assert_eq!(&bytes[12..20], &0x0000_0102_0304_0506_u64.to_be_bytes());
        assert_eq!(&bytes[20..52], &[0x11; 32]);
        assert_eq!(&bytes[52..84], &[0x22; 32]);
        assert_eq!(&bytes[84..91], &[0x33; 7]);
        assert_eq!(&bytes[91..], &[0x44; KEM_CIPHERTEXT_BYTES]);
        let decoded = EnvelopeHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.encode(), bytes);

        for index in [0, 8, 10] {
            let mut changed = bytes;
            changed[index] ^= 1;
            assert!(EnvelopeHeader::decode(&changed).is_err());
        }
        assert!(EnvelopeHeader::decode(&bytes[..HEADER_BYTES - 1]).is_err());
    }

    #[test]
    fn stream_limits_are_exact_and_checked_without_allocating_payloads() {
        assert_eq!(frame_count(0).unwrap(), 1);
        assert_eq!(frame_count(1).unwrap(), 1);
        assert_eq!(frame_count(CHUNK_BYTES).unwrap(), 1);
        assert_eq!(frame_count(CHUNK_BYTES + 1).unwrap(), 2);
        assert_eq!(frame_count(PLAINTEXT_MAX).unwrap(), FRAME_MAX);
        assert!(matches!(
            frame_count(PLAINTEXT_MAX + 1),
            Err(ProtectionError::InputTooLarge { .. })
        ));
        assert_eq!(envelope_size(0).unwrap(), HEADER_BYTES as u64 + TAG_BYTES);
        assert_eq!(
            envelope_size(CHUNK_BYTES + 1).unwrap(),
            HEADER_BYTES as u64 + CHUNK_BYTES + 1 + 2 * TAG_BYTES
        );
        assert!(checked_envelope_size(0, u64::MAX).is_err());
        assert!(checked_envelope_size(u64::MAX, 1).is_err());
    }
}
