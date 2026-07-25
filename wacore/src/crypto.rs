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

/// Legacy protocol fingerprint; this must not be used as a security hash.
pub fn md5_digest(input: &[u8]) -> [u8; 16] {
    md5::compute(input).into()
}

/// Suffix WA Web mixes into the contact hash (`WAWebApiContact`).
const CONTACT_NOTIFICATION_HASH_SUFFIX: &[u8] = b"WA_ADD_NOTIF";

/// The contact identifier carried by `<notification type="devices">
/// <update hash="..."/></notification>`, base64-encoded on the wire. `user` is
/// the contact's bare user part. Fed incrementally to avoid a concat buffer.
pub fn contact_notification_hash(user: &str) -> [u8; 3] {
    let mut context = md5::Context::new();
    context.consume(user.as_bytes());
    context.consume(CONTACT_NOTIFICATION_HASH_SUFFIX);
    let digest: [u8; 16] = context.finalize().into();
    [digest[0], digest[1], digest[2]]
}

/// Decode the wire form of [`contact_notification_hash`]. Rejects anything that
/// is not exactly one base64 group, so a malformed attribute cannot alias a
/// real contact.
pub fn parse_contact_notification_hash(wire: &str) -> Option<[u8; 3]> {
    use base64::Engine as _;

    let mut decoded = [0u8; 3];
    let written = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode_slice(wire.as_bytes(), &mut decoded)
        .ok()?;
    (written == decoded.len()).then_some(decoded)
}

/// Rejects output beyond HKDF's 255-block expansion limit before allocating.
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

/// Caller-owned output avoids an allocation in fixed-size derivation paths.
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

/// Uses the crate-wide secure random source so key generation follows one policy.
pub fn generate_curve_key_pair() -> KeyPair {
    KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
}

/// Uses the same secure random policy as key generation for randomized signatures.
pub fn calculate_curve_signature(
    private_key: &PrivateKey,
    message: &[u8],
) -> Result<[u8; 64], CurveError> {
    private_key.calculate_signature(message, &mut rand::make_rng::<rand::rngs::StdRng>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirmed against a live stream, then re-derived from fictitious LIDs.
    #[test]
    fn contact_notification_hash_matches_wire_vectors() {
        use base64::Engine as _;
        for (user, wire) in [
            ("100000000000001", "s7oK"),
            ("100000000000002", "P2DY"),
            ("100000000000003", "4ZhY"),
        ] {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(contact_notification_hash(user));
            assert_eq!(encoded, wire, "contact hash for {user}");
            assert_eq!(
                parse_contact_notification_hash(wire),
                Some(contact_notification_hash(user)),
                "wire hash for {user} must round-trip"
            );
        }
    }

    #[test]
    fn malformed_contact_hashes_are_rejected() {
        for wire in ["", "s7o", "s7oKX", "s7oK==", "!!!!", "s7oKs7oK"] {
            assert_eq!(
                parse_contact_notification_hash(wire),
                None,
                "{wire:?} is not a single base64 group"
            );
        }
    }

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
