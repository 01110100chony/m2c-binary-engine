use super::*;
use crate::m6_campaign as campaign;
use std::{collections::BTreeMap, fs, io::Cursor, path::Path};
fn field(name: &str) -> Vec<u8> {
    let prefix = format!("{name}=");
    let text = include_str!("../../tests/fixtures/m5_mlkem768_openssl.txt")
        .lines()
        .find_map(|s| s.strip_prefix(&prefix))
        .unwrap();
    text.as_bytes()
        .chunks_exact(2)
        .map(|b| u8::from_str_radix(std::str::from_utf8(b).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn m6_structured_protection_mutations() {
    let pair = crypto::keypair_from_test_seed(&field("seed")).unwrap();
    let secret = codec::encode_secret_key(&pair.secret).unwrap();
    // Test-only deterministic envelope, based on independent OpenSSL KEM material.
    let mut envelopes = Vec::new();
    for length in [0, 1, (1 << 20) - 1, 1 << 20, (1 << 20) + 1, (2 << 20) + 17] {
        let header = codec::EnvelopeHeader {
            plaintext_length: length,
            recipient_public_key_sha256: crypto::fingerprint(&pair.public),
            hkdf_salt: [7; 32],
            stream_nonce_prefix: [3; 7],
            kem_ciphertext: field("ciphertext").try_into().unwrap(),
        };
        let key =
            crypto::derive_content_key(&header.hkdf_salt, &field("shared").try_into().unwrap())
                .unwrap();
        let encoded = header.encode();
        let mut envelope = encoded.to_vec();
        stream::encrypt_payload(
            &mut Cursor::new(vec![0x42; length as usize]),
            &mut envelope,
            stream::StreamParameters {
                input_path: Path::new("test"),
                output_path: Path::new("test"),
                plaintext_length: length,
                header: &encoded,
                content_key: &key,
                nonce_prefix: &header.stream_nonce_prefix,
            },
        )
        .unwrap();
        envelopes.push(envelope);
    }
    campaign::run(
        "m5",
        |bytes| {
            let mut envelope = envelopes[usize::from(bytes[0]) % envelopes.len()].clone();
            let mut key = secret.clone();
            match bytes[0] % 4 {
                0 => {
                    let offset = usize::from(*bytes.get(1).unwrap_or(&0)) % 1179;
                    envelope[offset] ^= 1;
                }
                1 => {
                    let last = envelope.len() - 1;
                    envelope[last] ^= 1;
                }
                2 => {
                    envelope.truncate(
                        envelope.len()
                            - 1
                            - usize::from(*bytes.get(1).unwrap_or(&0)).min(envelope.len() - 1),
                    );
                }
                _ => {
                    key.truncate(usize::from(*bytes.get(1).unwrap_or(&0)) * 9);
                }
            }
            campaign::Case {
                kind: "m5".into(),
                files: BTreeMap::from([("envelope".into(), envelope), ("secret.key".into(), key)]),
            }
        },
        |_, root| {
            let result = unprotect_file(
                &root.join("envelope"),
                &root.join("secret.key"),
                &root.join("output"),
            );
            assert!(result.is_err());
            assert!(!root.join("output").exists());
            assert_eq!(
                fs::read_dir(root).unwrap().count(),
                2,
                "staging residue on normal error"
            );
        },
    );
}
