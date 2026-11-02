//! WhatsApp Web multi-device numeric security codes.
//!
//! These are snapshots for an out-of-band comparison, not a trust decision or
//! proof of possession. Include every non-hosted device identity exactly once.

use sha2::{Digest, Sha512};
use thiserror::Error;

use crate::libsignal::protocol::IdentityKey;
use wacore_binary::Jid;

/// Invalid inputs to numeric security-code generation.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SecurityCodeError {
    #[error("security codes require a numeric phone-number or LID user JID")]
    InvalidIdentifier,
    #[error("security codes require at least one public identity key per account")]
    MissingIdentity,
}

/// Numeric identifier convention. These versions must not be compared to each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityCodeVersion {
    /// WhatsApp's country-code plus phone-number suffix convention.
    V3,
    /// WhatsApp's bare LID user identifier convention.
    V4,
}

/// Compute the 60 ASCII digits shown by WhatsApp Web for the supplied accounts.
///
/// Keys use Signal's public [`IdentityKey`] type. The algorithm hashes their
/// 33-byte type-prefixed encodings, sorted by bytes, retaining multiplicity.
/// Callers must supply the complete device sets, including their own device.
/// V3 accepts PN JIDs, V4 accepts LID JIDs. Device suffixes do not affect the
/// account identifier. No network or trust-store mutation occurs here.
///
/// Static reference: whatspec ae3880925bacd23863cc30ec480c9f82bfd869f1,
/// WA 2.3000.1052559453, WAWebIdentityVerificationApi (`p`, `_`, `v`, `R`)
/// and WAWebIdentityApiUtils (`identityKeysToBinary`).
pub fn numeric_security_code(
    version: SecurityCodeVersion,
    local: &Jid,
    local_keys: &[IdentityKey],
    remote: &Jid,
    remote_keys: &[IdentityKey],
) -> Result<String, SecurityCodeError> {
    let local_id = identifier(version, local)?;
    let remote_id = identifier(version, remote)?;
    let mut halves = [half(&local_id, local_keys)?, half(&remote_id, remote_keys)?];
    halves.sort();
    Ok(halves.concat())
}

fn identifier(version: SecurityCodeVersion, jid: &Jid) -> Result<String, SecurityCodeError> {
    if jid.user.is_empty() || !jid.user.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SecurityCodeError::InvalidIdentifier);
    }
    match version {
        SecurityCodeVersion::V4 if jid.is_lid() => Ok(jid.user.to_string()),
        SecurityCodeVersion::V3 if jid.is_pn() => {
            let user = jid.user.as_str();
            // Preserve the official regexp's ordered alternatives, including
            // its three-digit fallback and full-JID fallback for short users.
            let prefixes = [
                "1", "7", "20", "27", "30", "31", "32", "33", "34", "36", "39", "40", "41", "43",
                "44", "45", "46", "47", "48", "49", "51", "52", "53", "54", "55", "56", "57", "58",
                "60", "61", "62", "63", "64", "65", "66", "81", "82", "84", "86", "90", "91", "92",
                "93", "94", "95", "98",
            ];
            let prefix_len = prefixes
                .iter()
                .find(|p| user.starts_with(**p) && user.len() >= p.len() + 4)
                .map(|p| p.len())
                .or_else(|| (user.len() >= 7).then_some(3));
            Ok(match prefix_len {
                Some(n) => format!(
                    "{}{}",
                    &user[..n],
                    &user[user.len() - (user.len() - n).min(6)..]
                ),
                None => jid.to_non_ad().to_string(),
            })
        }
        _ => Err(SecurityCodeError::InvalidIdentifier),
    }
}

fn half(identifier: &str, keys: &[IdentityKey]) -> Result<String, SecurityCodeError> {
    if keys.is_empty() {
        return Err(SecurityCodeError::MissingIdentity);
    }
    let mut keys: Vec<_> = keys.iter().map(IdentityKey::serialize).collect();
    keys.sort();
    let mut initial = Sha512::new();
    initial.update([0, 0]);
    for key in &keys {
        initial.update(key);
    }
    initial.update(identifier.as_bytes());
    // The first round hashes the entire seed followed by the keys too.
    for key in &keys {
        initial.update(key);
    }
    let mut digest = initial.finalize();
    for _ in 1..5200 {
        let mut h = Sha512::new();
        h.update(digest);
        for key in &keys {
            h.update(key);
        }
        digest = h.finalize();
    }
    let mut result = String::with_capacity(30);
    use std::fmt::Write;
    for chunk in digest[..30].chunks_exact(5) {
        let value = chunk.iter().fold(0u64, |acc, b| (acc << 8) | u64::from(*b));
        // Writing to String is infallible.
        let _ = write!(result, "{:05}", value % 100_000);
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libsignal::protocol::PublicKey;
    fn key(v: u8) -> IdentityKey {
        IdentityKey::new(PublicKey::from_djb_public_key_bytes(&[v; 32]).unwrap())
    }
    // Synthetic calls to AST-extracted official S/p/_ and WABinary, with
    // native WebCrypto and an async-generator runtime adapter. No authenticated-client execution.
    // SHA256 03d897d9def554de4b712469aa7ce39f9e7861cf176bd36d99f33270c6fca901,
    // WAWebIdentityVerificationApi byte range 9916..18195.
    #[test]
    fn matches_official_web_numeric_vectors() {
        for (version, local, remote, expected) in [
            (
                SecurityCodeVersion::V3,
                Jid::pn("15551234567"),
                Jid::pn("442079460001"),
                "694840143725290095540559872187863669591573112226601946543520",
            ),
            (
                SecurityCodeVersion::V4,
                Jid::lid("100000000000001"),
                Jid::lid("100000000000002"),
                "158788750905376650263214847873716600833018264479947487142264",
            ),
        ] {
            assert_eq!(
                numeric_security_code(version, &local, &[key(2), key(1)], &remote, &[key(3)])
                    .unwrap(),
                expected
            );
            assert_eq!(
                numeric_security_code(
                    version,
                    &remote.with_device(7),
                    &[key(3)],
                    &local.with_device(2),
                    &[key(1), key(2)]
                )
                .unwrap(),
                expected
            );
            assert_ne!(
                numeric_security_code(version, &local, &[key(1)], &remote, &[key(3)]).unwrap(),
                expected
            );
            assert_ne!(
                numeric_security_code(version, &local, &[key(2), key(1)], &remote, &[key(4)])
                    .unwrap(),
                expected
            );
        }
    }
    #[test]
    fn rejects_missing_keys_and_wrong_identifiers() {
        assert_eq!(
            numeric_security_code(
                SecurityCodeVersion::V3,
                &Jid::pn("15551234567"),
                &[],
                &Jid::pn("442079460001"),
                &[key(3)]
            ),
            Err(SecurityCodeError::MissingIdentity)
        );
        for jid in [
            Jid::pn("15551234567"),
            Jid::lid("100000000000001"),
            Jid::pn("+15551234567"),
            Jid::pn(""),
        ] {
            let version = if jid.is_lid() {
                SecurityCodeVersion::V3
            } else {
                SecurityCodeVersion::V4
            };
            assert_eq!(
                numeric_security_code(version, &jid, &[key(1)], &jid, &[key(2)]),
                Err(SecurityCodeError::InvalidIdentifier)
            );
        }
        assert!(IdentityKey::try_from(&[5u8; 32][..]).is_err());
        assert!(IdentityKey::try_from(&[6u8; 33][..]).is_err());
    }
    #[test]
    fn phone_normalization() {
        for (input, expected) in [
            ("15551234567", "1234567"),
            ("442079460001", "44460001"),
            ("9991234567", "999234567"),
            ("12345", "12345"),
            ("1234", "1234@s.whatsapp.net"),
        ] {
            assert_eq!(
                identifier(SecurityCodeVersion::V3, &Jid::pn(input)).unwrap(),
                expected
            );
        }
    }
}
