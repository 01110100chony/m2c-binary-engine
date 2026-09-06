use std::io::{self, Read, Write};
use std::path::Path;

use aead_stream::{DecryptorBE32, EncryptorBE32};
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::KeyInit;
use zeroize::Zeroizing;

use super::codec::{CHUNK_BYTES, STREAM_NONCE_PREFIX_BYTES, TAG_BYTES, frame_count};
use super::{ProtectionError, io_error};

fn cipher(key: &[u8; 32]) -> Result<Aes256Gcm, ProtectionError> {
    Aes256Gcm::new_from_slice(key).map_err(|_| ProtectionError::ArithmeticOverflow {
        operation: "AES-256-GCM key initialization",
    })
}

fn read_exact(
    reader: &mut impl Read,
    mut target: &mut [u8],
    path: &Path,
) -> Result<(), ProtectionError> {
    while !target.is_empty() {
        match reader.read(target) {
            Ok(0) => {
                return Err(ProtectionError::InvalidFrameSequence {
                    reason: "premature end of input",
                });
            }
            Ok(read) => target = &mut target[read..],
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => return Err(io_error("read M5 input", path, source)),
        }
    }
    Ok(())
}

fn ensure_eof(reader: &mut impl Read, path: &Path) -> Result<(), ProtectionError> {
    let mut byte = [0_u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => return Ok(()),
            Ok(_) => {
                return Err(ProtectionError::InvalidFrameSequence {
                    reason: "input contains trailing bytes or changed during processing",
                });
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(source) => return Err(io_error("read M5 input", path, source)),
        }
    }
}

fn frame_plaintext_bytes(remaining: u64) -> Result<usize, ProtectionError> {
    usize::try_from(remaining.min(CHUNK_BYTES)).map_err(|_| ProtectionError::ArithmeticOverflow {
        operation: "M5 frame buffer size",
    })
}

pub(crate) struct StreamParameters<'a> {
    pub(crate) input_path: &'a Path,
    pub(crate) output_path: &'a Path,
    pub(crate) plaintext_length: u64,
    pub(crate) header: &'a [u8],
    pub(crate) content_key: &'a [u8; 32],
    pub(crate) nonce_prefix: &'a [u8; STREAM_NONCE_PREFIX_BYTES],
}

pub(crate) fn encrypt_payload(
    reader: &mut impl Read,
    writer: &mut impl Write,
    parameters: StreamParameters<'_>,
) -> Result<(), ProtectionError> {
    let StreamParameters {
        input_path,
        output_path,
        plaintext_length,
        header,
        content_key,
        nonce_prefix,
    } = parameters;
    let frames = frame_count(plaintext_length)?;
    let nonce =
        nonce_prefix
            .as_slice()
            .try_into()
            .map_err(|_| ProtectionError::ArithmeticOverflow {
                operation: "STREAM-BE32 nonce prefix",
            })?;
    let mut encryptor = Some(EncryptorBE32::from_aead(cipher(content_key)?, &nonce));
    let mut remaining = plaintext_length;
    for frame in 0..frames {
        let plaintext_bytes = frame_plaintext_bytes(remaining)?;
        let capacity = plaintext_bytes.checked_add(TAG_BYTES as usize).ok_or(
            ProtectionError::ArithmeticOverflow {
                operation: "M5 ciphertext frame buffer",
            },
        )?;
        let mut buffer = Vec::with_capacity(capacity);
        buffer.resize(plaintext_bytes, 0);
        read_exact(reader, &mut buffer, input_path)?;
        let last = frame
            .checked_add(1)
            .ok_or(ProtectionError::ArithmeticOverflow {
                operation: "M5 frame index",
            })?
            == frames;
        if last {
            encryptor
                .take()
                .ok_or(ProtectionError::InvalidFrameSequence {
                    reason: "missing STREAM-BE32 encryptor",
                })?
                .encrypt_last_in_place(header, &mut buffer)
                .map_err(|_| ProtectionError::AuthenticationFailed)?;
        } else {
            encryptor
                .as_mut()
                .ok_or(ProtectionError::InvalidFrameSequence {
                    reason: "missing STREAM-BE32 encryptor",
                })?
                .encrypt_next_in_place(header, &mut buffer)
                .map_err(|_| ProtectionError::InvalidFrameSequence {
                    reason: "STREAM-BE32 counter exhausted before final frame",
                })?;
        }
        writer
            .write_all(&buffer)
            .map_err(|source| io_error("write M5 staging", output_path, source))?;
        remaining = remaining.checked_sub(plaintext_bytes as u64).ok_or(
            ProtectionError::ArithmeticOverflow {
                operation: "remaining M5 plaintext bytes",
            },
        )?;
    }
    if remaining != 0 || encryptor.is_some() {
        return Err(ProtectionError::InvalidFrameSequence {
            reason: "plaintext length did not end on the final frame",
        });
    }
    ensure_eof(reader, input_path)
}

pub(crate) fn decrypt_payload(
    reader: &mut impl Read,
    writer: &mut impl Write,
    parameters: StreamParameters<'_>,
) -> Result<(), ProtectionError> {
    let StreamParameters {
        input_path,
        output_path,
        plaintext_length,
        header,
        content_key,
        nonce_prefix,
    } = parameters;
    let frames = frame_count(plaintext_length)?;
    let nonce =
        nonce_prefix
            .as_slice()
            .try_into()
            .map_err(|_| ProtectionError::ArithmeticOverflow {
                operation: "STREAM-BE32 nonce prefix",
            })?;
    let mut decryptor = Some(DecryptorBE32::from_aead(cipher(content_key)?, &nonce));
    let mut remaining = plaintext_length;
    for frame in 0..frames {
        let plaintext_bytes = frame_plaintext_bytes(remaining)?;
        let ciphertext_bytes = plaintext_bytes.checked_add(TAG_BYTES as usize).ok_or(
            ProtectionError::ArithmeticOverflow {
                operation: "M5 ciphertext frame buffer",
            },
        )?;
        let mut buffer = Zeroizing::new(Vec::with_capacity(ciphertext_bytes));
        buffer.resize(ciphertext_bytes, 0);
        read_exact(reader, &mut buffer, input_path)?;
        let last = frame
            .checked_add(1)
            .ok_or(ProtectionError::ArithmeticOverflow {
                operation: "M5 frame index",
            })?
            == frames;
        let result = if last {
            decryptor
                .take()
                .ok_or(ProtectionError::InvalidFrameSequence {
                    reason: "missing STREAM-BE32 decryptor",
                })?
                .decrypt_last_in_place(header, &mut *buffer)
        } else {
            decryptor
                .as_mut()
                .ok_or(ProtectionError::InvalidFrameSequence {
                    reason: "missing STREAM-BE32 decryptor",
                })?
                .decrypt_next_in_place(header, &mut *buffer)
        };
        result.map_err(|_| ProtectionError::AuthenticationFailed)?;
        if buffer.len() != plaintext_bytes {
            return Err(ProtectionError::InvalidFrameSequence {
                reason: "authenticated frame has an unexpected plaintext length",
            });
        }
        writer
            .write_all(&buffer)
            .map_err(|source| io_error("write M5 plaintext staging", output_path, source))?;
        remaining = remaining.checked_sub(plaintext_bytes as u64).ok_or(
            ProtectionError::ArithmeticOverflow {
                operation: "remaining M5 plaintext bytes",
            },
        )?;
    }
    if remaining != 0 || decryptor.is_some() {
        return Err(ProtectionError::InvalidFrameSequence {
            reason: "plaintext length did not end on the final frame",
        });
    }
    ensure_eof(reader, input_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aead_stream::aead::{Aead, Payload};
    use aes_gcm::Nonce;
    use std::io::Cursor;

    #[test]
    fn aes_256_gcm_matches_nist_empty_plaintext_vector() {
        let cipher = cipher(&[0_u8; 32]).unwrap();
        let nonce = Nonce::try_from(&[0_u8; 12][..]).unwrap();
        let actual = cipher.encrypt(&nonce, b"".as_slice()).unwrap();
        assert_eq!(
            actual,
            [
                0x53, 0x0f, 0x8a, 0xfb, 0xc7, 0x45, 0x36, 0xb9, 0xa9, 0x63, 0xb4, 0xf1, 0xc4, 0xcb,
                0x73, 0x8b,
            ]
        );
    }

    #[test]
    fn stream_be32_frame_matches_independent_nonce_construction() {
        let key = [0x42; 32];
        let prefix = [0x24; 7];
        let aad = b"authenticated M5 header";
        let plaintext = b"one final frame";
        let mut encrypted = Vec::new();
        encrypt_payload(
            &mut Cursor::new(plaintext),
            &mut encrypted,
            StreamParameters {
                input_path: Path::new("input"),
                output_path: Path::new("output"),
                plaintext_length: plaintext.len() as u64,
                header: aad,
                content_key: &key,
                nonce_prefix: &prefix,
            },
        )
        .unwrap();

        let mut full_nonce = [0_u8; 12];
        full_nonce[..7].copy_from_slice(&prefix);
        full_nonce[7..11].copy_from_slice(&0_u32.to_be_bytes());
        full_nonce[11] = 1;
        let full_nonce = Nonce::try_from(full_nonce.as_slice()).unwrap();
        let independent = cipher(&key)
            .unwrap()
            .encrypt(
                &full_nonce,
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .unwrap();
        assert_eq!(encrypted, independent);
    }

    #[test]
    fn stream_be32_two_frames_match_independent_counter_and_final_flag() {
        let key = [0x43; 32];
        let prefix = [0x25; 7];
        let aad = b"authenticated M5 header";
        let plaintext = vec![0x5c; CHUNK_BYTES as usize + 1];
        let mut encrypted = Vec::new();
        encrypt_payload(
            &mut Cursor::new(&plaintext),
            &mut encrypted,
            StreamParameters {
                input_path: Path::new("input"),
                output_path: Path::new("output"),
                plaintext_length: plaintext.len() as u64,
                header: aad,
                content_key: &key,
                nonce_prefix: &prefix,
            },
        )
        .unwrap();

        let independent_frame = |counter: u32, last: bool, plaintext: &[u8]| {
            let mut nonce = [0_u8; 12];
            nonce[..7].copy_from_slice(&prefix);
            nonce[7..11].copy_from_slice(&counter.to_be_bytes());
            nonce[11] = u8::from(last);
            cipher(&key)
                .unwrap()
                .encrypt(
                    &Nonce::try_from(nonce.as_slice()).unwrap(),
                    Payload {
                        msg: plaintext,
                        aad,
                    },
                )
                .unwrap()
        };
        let split = CHUNK_BYTES as usize + TAG_BYTES as usize;
        assert_eq!(
            &encrypted[..split],
            independent_frame(0, false, &plaintext[..CHUNK_BYTES as usize])
        );
        assert_eq!(
            &encrypted[split..],
            independent_frame(1, true, &plaintext[CHUNK_BYTES as usize..])
        );
    }

    #[test]
    fn stream_round_trip_covers_empty_and_chunk_boundary() {
        for plaintext in [Vec::new(), vec![0x5a; CHUNK_BYTES as usize + 1]] {
            let mut encrypted = Vec::new();
            encrypt_payload(
                &mut Cursor::new(&plaintext),
                &mut encrypted,
                StreamParameters {
                    input_path: Path::new("input"),
                    output_path: Path::new("output"),
                    plaintext_length: plaintext.len() as u64,
                    header: b"header",
                    content_key: &[7; 32],
                    nonce_prefix: &[9; 7],
                },
            )
            .unwrap();
            let mut decrypted = Vec::new();
            decrypt_payload(
                &mut Cursor::new(&encrypted),
                &mut decrypted,
                StreamParameters {
                    input_path: Path::new("input"),
                    output_path: Path::new("output"),
                    plaintext_length: plaintext.len() as u64,
                    header: b"header",
                    content_key: &[7; 32],
                    nonce_prefix: &[9; 7],
                },
            )
            .unwrap();
            assert_eq!(decrypted, plaintext);
        }
    }

    #[test]
    fn tampering_and_length_changes_fail_closed() {
        let plaintext = b"secret";
        let mut encrypted = Vec::new();
        encrypt_payload(
            &mut Cursor::new(plaintext),
            &mut encrypted,
            StreamParameters {
                input_path: Path::new("input"),
                output_path: Path::new("output"),
                plaintext_length: plaintext.len() as u64,
                header: b"header",
                content_key: &[7; 32],
                nonce_prefix: &[9; 7],
            },
        )
        .unwrap();
        for changed in [
            {
                let mut value = encrypted.clone();
                value[0] ^= 1;
                value
            },
            encrypted[..encrypted.len() - 1].to_vec(),
            {
                let mut value = encrypted.clone();
                value.push(0);
                value
            },
        ] {
            let mut output = Vec::new();
            assert!(
                decrypt_payload(
                    &mut Cursor::new(changed),
                    &mut output,
                    StreamParameters {
                        input_path: Path::new("input"),
                        output_path: Path::new("output"),
                        plaintext_length: plaintext.len() as u64,
                        header: b"header",
                        content_key: &[7; 32],
                        nonce_prefix: &[9; 7],
                    },
                )
                .is_err()
            );
        }
    }

    #[test]
    fn injected_reader_and_writer_failures_are_typed() {
        struct FailingReader;

        impl Read for FailingReader {
            fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("injected read failure"))
            }
        }

        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::other("injected write failure"))
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let parameters = || StreamParameters {
            input_path: Path::new("input"),
            output_path: Path::new("output"),
            plaintext_length: 1,
            header: b"header",
            content_key: &[7; 32],
            nonce_prefix: &[9; 7],
        };
        let mut output = Vec::new();
        assert!(matches!(
            encrypt_payload(&mut FailingReader, &mut output, parameters()),
            Err(ProtectionError::Io {
                operation: "read M5 input",
                ..
            })
        ));
        assert!(matches!(
            encrypt_payload(&mut Cursor::new([0x42]), &mut FailingWriter, parameters()),
            Err(ProtectionError::Io {
                operation: "write M5 staging",
                ..
            })
        ));
    }
}
