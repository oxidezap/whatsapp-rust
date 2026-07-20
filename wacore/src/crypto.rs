//! Runtime-agnostic cryptographic primitives shared across protocol features.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::libsignal::protocol::{CurveError, KeyPair, PrivateKey};

const HKDF_SHA256_MAX_OUTPUT_LENGTH: usize = 255 * 32;

/// Errors returned by the shared cryptographic helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// The requested HKDF output exceeds the SHA-256 expansion limit.
    #[error("HKDF-SHA256 output length is invalid")]
    InvalidHkdfLength,
}

/// Computes an MD5 digest.
pub fn md5_digest(input: &[u8]) -> [u8; 16] {
    md5::compute(input).into()
}

/// Derives `expanded_length` bytes using HKDF-SHA256.
pub fn hkdf_sha256(
    input_key_material: &[u8],
    expanded_length: usize,
    salt: Option<&[u8]>,
    info: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if expanded_length > HKDF_SHA256_MAX_OUTPUT_LENGTH {
        return Err(CryptoError::InvalidHkdfLength);
    }
    let mut output = vec![0; expanded_length];
    hkdf_sha256_into(input_key_material, salt, info, &mut output)?;
    Ok(output)
}

/// Derives HKDF-SHA256 output directly into a caller-provided buffer.
pub fn hkdf_sha256_into(
    input_key_material: &[u8],
    salt: Option<&[u8]>,
    info: &[u8],
    output: &mut [u8],
) -> Result<(), CryptoError> {
    Hkdf::<Sha256>::new(salt, input_key_material)
        .expand(info, output)
        .map_err(|_| CryptoError::InvalidHkdfLength)
}

/// Generates a Curve25519 key pair with the configured secure random source.
pub fn generate_curve_key_pair() -> KeyPair {
    KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
}

/// Signs `message` with the supplied Curve25519 private key.
pub fn calculate_curve_signature(
    private_key: &PrivateKey,
    message: &[u8],
) -> Result<[u8; 64], CurveError> {
    private_key.calculate_signature(message, &mut rand::make_rng::<rand::rngs::StdRng>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_expands_known_vectors() {
        assert_eq!(
            hex::encode(md5_digest(b"abc")),
            "900150983cd24fb0d6963f7d28e17f72"
        );

        let ikm = [0x0bu8; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let output = hkdf_sha256(&ikm, 42, Some(&salt), &info).unwrap();
        assert_eq!(
            hex::encode(output),
            "3cb25f25faacd57a90434f64d0362f2a\
             2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865"
                .replace(char::is_whitespace, "")
        );
    }

    #[test]
    fn writes_into_a_caller_owned_buffer() {
        let mut output = [0u8; 32];
        hkdf_sha256_into(b"input", None, b"info", &mut output).unwrap();
        assert_eq!(
            output.as_slice(),
            hkdf_sha256(b"input", 32, None, b"info").unwrap()
        );
    }

    #[test]
    fn rejects_output_beyond_sha256_limit() {
        assert_eq!(
            hkdf_sha256(b"input", HKDF_SHA256_MAX_OUTPUT_LENGTH + 1, None, b"info"),
            Err(CryptoError::InvalidHkdfLength)
        );

        let mut output = vec![0; HKDF_SHA256_MAX_OUTPUT_LENGTH + 1];
        assert_eq!(
            hkdf_sha256_into(b"input", None, b"info", &mut output),
            Err(CryptoError::InvalidHkdfLength)
        );
    }

    #[test]
    fn generated_keys_sign_with_the_shared_signal_implementation() {
        let pair = generate_curve_key_pair();
        let signature = calculate_curve_signature(&pair.private_key, b"message").unwrap();
        assert!(pair.public_key.verify_signature(b"message", &signature));
        assert!(!pair.public_key.verify_signature(b"tampered", &signature));
    }
}
