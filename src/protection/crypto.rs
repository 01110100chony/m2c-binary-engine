use hkdf::Hkdf;
#[allow(deprecated)]
use ml_kem::ExpandedKeyEncoding;
use ml_kem::kem::{Decapsulate, KeyExport, TryKeyInit};
use ml_kem::{B32, DecapsulationKey768, EncapsulationKey768, FromSeed, MlKem768, Seed};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::ProtectionError;
use super::codec::{KEM_CIPHERTEXT_BYTES, PUBLIC_KEY_BYTES, SECRET_KEY_BYTES};

const HKDF_INFO: &[u8] = b"M2C-M5-SUITE-0001-CONTENT-KEY";

pub(crate) struct KeyPairBytes {
    pub(crate) public: Vec<u8>,
    pub(crate) secret: Zeroizing<Vec<u8>>,
}

pub(crate) fn system_random(bytes: &mut [u8]) -> Result<(), ProtectionError> {
    map_entropy(getrandom::fill(bytes))
}

fn map_entropy(result: Result<(), getrandom::Error>) -> Result<(), ProtectionError> {
    result.map_err(|source| ProtectionError::EntropyUnavailable { source })
}

pub(crate) fn generate_keypair() -> Result<KeyPairBytes, ProtectionError> {
    let mut seed = Zeroizing::new([0_u8; 64]);
    system_random(seed.as_mut_slice())?;
    let seed_array = Seed::try_from(seed.as_slice()).map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "ML-KEM-768 seed has the wrong length",
    })?;
    let (decapsulation_key, encapsulation_key) = MlKem768::from_seed(&seed_array);
    serialize_keypair(&decapsulation_key, &encapsulation_key)
}

fn serialize_keypair(
    decapsulation_key: &DecapsulationKey768,
    encapsulation_key: &EncapsulationKey768,
) -> Result<KeyPairBytes, ProtectionError> {
    let public = encapsulation_key.to_bytes().as_slice().to_vec();
    #[allow(deprecated)]
    let expanded = decapsulation_key.to_expanded_bytes();
    let secret = Zeroizing::new(expanded.as_slice().to_vec());
    if public.len() != PUBLIC_KEY_BYTES || secret.len() != SECRET_KEY_BYTES {
        return Err(ProtectionError::InvalidKey {
            path: Default::default(),
            reason: "ML-KEM-768 implementation returned an unexpected key size",
        });
    }
    Ok(KeyPairBytes { public, secret })
}

#[cfg(test)]
pub(super) fn keypair_from_test_seed(seed: &[u8]) -> Result<KeyPairBytes, ProtectionError> {
    let seed = Seed::try_from(seed).map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "test seed has the wrong length",
    })?;
    let (secret, public) = MlKem768::from_seed(&seed);
    serialize_keypair(&secret, &public)
}

pub(crate) fn parse_public_key(bytes: &[u8]) -> Result<EncapsulationKey768, ProtectionError> {
    EncapsulationKey768::new_from_slice(bytes).map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "ML-KEM-768 public-key encoding failed validation",
    })
}

pub(crate) fn parse_secret_key(bytes: &[u8]) -> Result<DecapsulationKey768, ProtectionError> {
    let expanded = bytes.try_into().map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "ML-KEM-768 expanded secret-key encoding has the wrong length",
    })?;
    #[allow(deprecated)]
    DecapsulationKey768::from_expanded_bytes(&expanded).map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "ML-KEM-768 expanded secret-key encoding failed validation",
    })
}

pub(crate) fn public_key_bytes(secret: &DecapsulationKey768) -> Vec<u8> {
    secret.encapsulation_key().to_bytes().as_slice().to_vec()
}

pub(crate) fn fingerprint(public_key: &[u8]) -> [u8; 32] {
    Sha256::digest(public_key).into()
}

pub(crate) fn encapsulate(
    public_key: &EncapsulationKey768,
) -> Result<([u8; KEM_CIPHERTEXT_BYTES], Zeroizing<[u8; 32]>), ProtectionError> {
    let mut message = Zeroizing::new([0_u8; 32]);
    system_random(message.as_mut_slice())?;
    let message = B32::try_from(message.as_slice()).map_err(|_| ProtectionError::InvalidKey {
        path: Default::default(),
        reason: "ML-KEM-768 encapsulation randomness has the wrong length",
    })?;
    let (ciphertext, shared) = public_key.encapsulate_deterministic(&message);
    let ciphertext =
        ciphertext
            .as_slice()
            .try_into()
            .map_err(|_| ProtectionError::ArithmeticOverflow {
                operation: "ML-KEM-768 ciphertext serialization",
            })?;
    let mut shared_copy = Zeroizing::new([0_u8; 32]);
    shared_copy.copy_from_slice(shared.as_slice());
    Ok((ciphertext, shared_copy))
}

pub(crate) fn decapsulate(
    secret_key: &DecapsulationKey768,
    ciphertext: &[u8; KEM_CIPHERTEXT_BYTES],
) -> Zeroizing<[u8; 32]> {
    let ciphertext = ciphertext
        .as_slice()
        .try_into()
        .expect("fixed M5 ciphertext size matches ML-KEM-768");
    let shared = secret_key.decapsulate(&ciphertext);
    let mut shared_copy = Zeroizing::new([0_u8; 32]);
    shared_copy.copy_from_slice(shared.as_slice());
    shared_copy
}

pub(crate) fn derive_content_key(
    salt: &[u8; 32],
    shared_secret: &[u8; 32],
) -> Result<Zeroizing<[u8; 32]>, ProtectionError> {
    let hkdf = Hkdf::<Sha256>::new(Some(salt), shared_secret);
    let mut key = Zeroizing::new([0_u8; 32]);
    hkdf.expand(HKDF_INFO, key.as_mut_slice()).map_err(|_| {
        ProtectionError::ArithmeticOverflow {
            operation: "HKDF-SHA-256 content-key derivation",
        }
    })?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_hex(value: &str) -> Vec<u8> {
        fn nibble(byte: u8) -> u8 {
            match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("invalid lowercase hex fixture"),
            }
        }
        assert_eq!(value.len() % 2, 0);
        value
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
            .collect()
    }

    fn fixture_field(name: &str) -> Vec<u8> {
        let fixture = include_str!("../../tests/fixtures/m5_mlkem768_openssl.txt");
        let prefix = format!("{name}=");
        let value = fixture
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .unwrap();
        decode_hex(value)
    }

    fn encapsulate_with_test_message(
        public_key: &EncapsulationKey768,
        message: &[u8],
    ) -> Result<([u8; KEM_CIPHERTEXT_BYTES], Zeroizing<[u8; 32]>), ProtectionError> {
        let message = B32::try_from(message).map_err(|_| ProtectionError::InvalidKey {
            path: Default::default(),
            reason: "test message has the wrong length",
        })?;
        let (ciphertext, shared) = public_key.encapsulate_deterministic(&message);
        let ciphertext = ciphertext.as_slice().try_into().unwrap();
        let mut shared_copy = Zeroizing::new([0_u8; 32]);
        shared_copy.copy_from_slice(shared.as_slice());
        Ok((ciphertext, shared_copy))
    }

    #[test]
    fn ml_kem_adapter_round_trips_and_validates_key_encodings() {
        let seed: Vec<u8> = (0_u8..64).collect();
        let pair = keypair_from_test_seed(&seed).unwrap();
        assert_eq!(pair.public.len(), PUBLIC_KEY_BYTES);
        assert_eq!(pair.secret.len(), SECRET_KEY_BYTES);
        let public = parse_public_key(&pair.public).unwrap();
        let secret = parse_secret_key(&pair.secret).unwrap();
        assert_eq!(public_key_bytes(&secret), pair.public);
        let message: Vec<u8> = (0_u8..32).rev().collect();
        let (ciphertext, sent) = encapsulate_with_test_message(&public, &message).unwrap();
        let received = decapsulate(&secret, &ciphertext);
        assert_eq!(*sent, *received);

        let mut bad_public = pair.public.clone();
        bad_public.fill(0xff);
        assert!(parse_public_key(&bad_public).is_err());
        let mut bad_secret = pair.secret.to_vec();
        bad_secret[SECRET_KEY_BYTES - 40] ^= 1;
        assert!(parse_secret_key(&bad_secret).is_err());
    }

    #[test]
    fn ml_kem_matches_openssl_3_6_interoperability_fixture() {
        let seed = fixture_field("seed");
        let pair = keypair_from_test_seed(&seed).unwrap();
        assert_eq!(
            fingerprint(&pair.public),
            fixture_field("public_sha256").as_slice()
        );
        let secret = parse_secret_key(&pair.secret).unwrap();
        let ciphertext: [u8; KEM_CIPHERTEXT_BYTES] =
            fixture_field("ciphertext").try_into().unwrap();
        let shared = decapsulate(&secret, &ciphertext);
        assert_eq!(shared.as_slice(), fixture_field("shared"));
    }

    #[test]
    fn hkdf_matches_rfc_5869_sha256_case_one() {
        let ikm = [0x0b; 22];
        let salt: [u8; 13] = (0_u8..13).collect::<Vec<_>>().try_into().unwrap();
        let hkdf = Hkdf::<Sha256>::new(Some(&salt), &ikm);
        let mut output = [0_u8; 42];
        hkdf.expand(b"\xf0\xf1\xf2\xf3\xf4\xf5\xf6\xf7\xf8\xf9", &mut output)
            .unwrap();
        assert_eq!(
            output,
            [
                0x3c, 0xb2, 0x5f, 0x25, 0xfa, 0xac, 0xd5, 0x7a, 0x90, 0x43, 0x4f, 0x64, 0xd0, 0x36,
                0x2f, 0x2a, 0x2d, 0x2d, 0x0a, 0x90, 0xcf, 0x1a, 0x5a, 0x4c, 0x5d, 0xb0, 0x2d, 0x56,
                0xec, 0xc4, 0xc5, 0xbf, 0x34, 0x00, 0x72, 0x08, 0xd5, 0xb8, 0x87, 0x18, 0x58, 0x65,
            ]
        );
    }

    #[test]
    fn m5_hkdf_context_is_stable() {
        let key = derive_content_key(&[0x01; 32], &[0x02; 32]).unwrap();
        assert_eq!(
            *key,
            [
                0x56, 0x66, 0x7d, 0x24, 0x29, 0xef, 0x7d, 0x17, 0xe6, 0xeb, 0xd2, 0x22, 0x73, 0x60,
                0x5c, 0xc9, 0x20, 0xf3, 0xec, 0xcd, 0x23, 0x64, 0x0d, 0xfb, 0xb5, 0x57, 0xb2, 0x37,
                0xd2, 0x35, 0xad, 0x01,
            ]
        );
    }

    #[test]
    fn entropy_failure_is_typed_without_fallback() {
        assert!(matches!(
            map_entropy(Err(getrandom::Error::new_custom(7))),
            Err(ProtectionError::EntropyUnavailable { .. })
        ));
    }
}
