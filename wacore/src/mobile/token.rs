//! Android registration-token primitive, reconstructed from pinned Android references.
//!
//! The host must supply version-bound APK material and derivation parameters. This
//! module neither authenticates an APK nor establishes that a server accepts a token.
//!
//! Reference provenance: whalibmob `AndroidApk.js` (`deriveSecretKey`, `computeToken`),
//! commit `8f50db4c586534dc0af4d13b26bdd09ef42660c0`; Cobalt
//! `WhatsAppAndroidClientInfo.java` (`getSecretKey`, `computeRegistrationToken`),
//! commit `cb9fbf172507626948fbc3611d0a6fb00bb3a0f1`. Neither is official-client
//! differential evidence; APK qualification remains a separate host responsibility.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use zeroize::Zeroizing;

/// Invalid or excessive token material.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid Android registration-token material")]
pub struct AndroidTokenError;

/// Secret APK-derived token material. Debug output deliberately omits all material.
pub struct AndroidTokenMaterial {
    secret: Zeroizing<[u8; 64]>,
    dex_md5: [u8; 16],
    certificates: Vec<Vec<u8>>,
}

impl std::fmt::Debug for AndroidTokenMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AndroidTokenMaterial([REDACTED])")
    }
}

impl AndroidTokenMaterial {
    /// Import material obtained from a separately authenticated, pinned APK.
    /// Certificate order is significant; no sorting or deduplication is performed.
    pub fn from_derived(
        secret: [u8; 64],
        dex_md5: [u8; 16],
        certificates: Vec<Vec<u8>>,
    ) -> Result<Self, AndroidTokenError> {
        let secret = Zeroizing::new(secret);
        if certificates.is_empty()
            || certificates.len() > 32
            || certificates
                .iter()
                .any(|c| c.is_empty() || c.len() > 65_536)
            || certificates.iter().map(Vec::len).sum::<usize>() > 1_048_576
        {
            return Err(AndroidTokenError);
        }
        Ok(Self {
            secret,
            dex_md5,
            certificates,
        })
    }

    /// Derive the binary PBKDF2-HMAC-SHA1 key using explicit version-bound inputs.
    /// No current production salt, package, logo path or iteration count is assumed.
    /// Cost is four PBKDF2 blocks: roughly four times `rounds` HMAC evaluations.
    pub fn derive_secret(
        package: &str,
        logo: &[u8],
        salt: &[u8],
        rounds: u32,
    ) -> Result<Zeroizing<[u8; 64]>, AndroidTokenError> {
        if !matches!(package, "com.whatsapp" | "com.whatsapp.w4b")
            || logo.is_empty()
            || logo.len() > 8 * 1024 * 1024
            || salt.is_empty()
            || salt.len() > 1024
            || !(1..=1_000_000).contains(&rounds)
        {
            return Err(AndroidTokenError);
        }
        let mut password = Zeroizing::new(Vec::with_capacity(package.len() + logo.len()));
        password.extend_from_slice(package.as_bytes());
        password.extend_from_slice(logo);
        let mut key = Zeroizing::new([0; 64]);
        pbkdf2(&password, salt, rounds, &mut *key)?;
        Ok(key)
    }

    /// Explicitly copy sensitive material for a host-managed private cache.
    /// Never log or persist this output without private-file protections.
    pub fn export_private_parts(&self) -> (Zeroizing<[u8; 64]>, [u8; 16], Vec<Vec<u8>>) {
        (
            Zeroizing::new(*self.secret),
            self.dex_md5,
            self.certificates.clone(),
        )
    }

    /// Generate raw base64 for ASCII national digits, retaining leading zeroes.
    /// The caller owns validation of country-specific telephone-number semantics.
    pub fn token_base64(&self, national: &str) -> Result<String, AndroidTokenError> {
        if national.is_empty()
            || national.len() > 32
            || !national.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(AndroidTokenError);
        }
        let mut mac = <Hmac<Sha1> as hmac::KeyInit>::new_from_slice(&self.secret[..])
            .map_err(|_| AndroidTokenError)?;
        for certificate in &self.certificates {
            mac.update(certificate);
        }
        mac.update(&self.dex_md5);
        mac.update(national.as_bytes());
        Ok(STANDARD.encode(mac.finalize().into_bytes()))
    }

    /// Generate the percent-encoded form value used by the pinned references.
    /// Supply this to a raw query builder exactly once to avoid double encoding.
    pub fn token_form_encoded(&self, national: &str) -> Result<String, AndroidTokenError> {
        Ok(self
            .token_base64(national)?
            .replace('+', "%2B")
            .replace('/', "%2F")
            .replace('=', "%3D"))
    }
}

/// iOS registration-token primitive with explicit version and variant-specific
/// static token supplied by the host. No current consumer/business secret is assumed.
/// Reference: pinned whalibmob Registration.js computeToken and Cobalt
/// WhatsAppIosClientInfo.computeRegistrationToken (same commits as Android above).
/// Synthetic validation does not qualify behavior against an official IPA/APK.
pub fn ios_registration_token(
    exact_version: &str,
    static_token: &str,
    national: &str,
) -> Result<String, AndroidTokenError> {
    if exact_version.is_empty()
        || exact_version.len() > 128
        || !exact_version.is_ascii()
        || static_token.is_empty()
        || static_token.len() > 1024
        || !static_token.is_ascii()
        || national.is_empty()
        || national.len() > 32
        || !national.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(AndroidTokenError);
    }
    let version = hex::encode(crate::crypto::md5_digest(exact_version.as_bytes()));
    let mut hash = crate::crypto::Md5::new();
    hash.consume(static_token.as_bytes());
    hash.consume(version.as_bytes());
    hash.consume(national.as_bytes());
    Ok(hex::encode(hash.finalize()))
}

fn pbkdf2(
    password: &[u8],
    salt: &[u8],
    rounds: u32,
    output: &mut [u8],
) -> Result<(), AndroidTokenError> {
    let keyed =
        <Hmac<Sha1> as hmac::KeyInit>::new_from_slice(password).map_err(|_| AndroidTokenError)?;
    for (index, chunk) in output.chunks_mut(20).enumerate() {
        let mut mac = keyed.clone();
        mac.update(salt);
        mac.update(&((index as u32 + 1).to_be_bytes()));
        let mut u = Zeroizing::new(<[u8; 20]>::from(mac.finalize().into_bytes()));
        chunk.copy_from_slice(&u[..chunk.len()]);
        for _ in 1..rounds {
            let mut mac = keyed.clone();
            mac.update(&u[..]);
            *u = mac.finalize().into_bytes().into();
            for (target, byte) in chunk.iter_mut().zip(u.iter()) {
                *target ^= *byte;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rfc6070_vectors_and_binary_password() {
        for (password, salt, rounds, expected) in [
            (
                &b"password"[..],
                &b"salt"[..],
                1,
                "0c60c80f961f0e71f3a9b524af6012062fe037a6",
            ),
            (
                &b"password"[..],
                &b"salt"[..],
                2,
                "ea6c014dc72d6f8ccd1ed92ace1d41f0d8de8957",
            ),
            (
                &b"pass\0word"[..],
                &b"sa\0lt"[..],
                4096,
                "56fa6aa75548099dcc37d7f03425e0c3",
            ),
        ] {
            let mut output = vec![0; expected.len() / 2];
            pbkdf2(password, salt, rounds, &mut output).unwrap();
            assert_eq!(hex::encode(output), expected);
        }
    }
    #[test]
    fn ios_synthetic_independent_standard_crypto_vector() {
        assert_eq!(
            ios_registration_token("2.0.1", "synthetic-static-token", "01234").unwrap(),
            "a767d4d0a67308a3122bbb255c77fc37"
        );
        assert_ne!(
            ios_registration_token("2.0.1", "synthetic-static-token", "01234").unwrap(),
            ios_registration_token("2.0.1", "synthetic-business-token", "01234").unwrap()
        );
        assert!(ios_registration_token("2.0.1", "synthetic", "+1234").is_err());
    }
    /// Synthetic inputs independently evaluated with Node's standard crypto API.
    #[test]
    fn binary_apk_material_token_vector() {
        let secret =
            AndroidTokenMaterial::derive_secret("com.whatsapp", &[0, 255, 7], &[9, 0, 2], 128)
                .unwrap();
        assert_eq!(
            hex::encode(*secret),
            "f7e0acbaa441b10300b7b1bb3deb79633cf38799bc1f206f09ffdf3951b6955608abc37eaac90ae2313d868277d08103f5c0e1960d4435797fec613253558335"
        );
        let material =
            AndroidTokenMaterial::from_derived(*secret, [3; 16], vec![vec![1], vec![2]]).unwrap();
        assert_eq!(
            material.token_base64("01234").unwrap(),
            "ynDf0WRRrRVjhKogPZpT9Q/ofOE="
        );
        assert_eq!(
            material.token_form_encoded("01234").unwrap(),
            "ynDf0WRRrRVjhKogPZpT9Q%2FofOE%3D"
        );
    }
    #[test]
    fn validation_and_order() {
        let a =
            AndroidTokenMaterial::from_derived([7; 64], [3; 16], vec![vec![1], vec![2]]).unwrap();
        let b =
            AndroidTokenMaterial::from_derived([7; 64], [3; 16], vec![vec![2], vec![1]]).unwrap();
        assert_ne!(
            a.token_base64("01234").unwrap(),
            b.token_base64("01234").unwrap()
        );
        assert_ne!(
            a.token_base64("01234").unwrap(),
            a.token_base64("1234").unwrap()
        );
        assert!(a.token_base64("+1234").is_err());
        assert_eq!(format!("{a:?}"), "AndroidTokenMaterial([REDACTED])");
        assert!(AndroidTokenMaterial::derive_secret("other", b"logo", b"salt", 128).is_err());
    }
}
