use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use zeroize::Zeroizing;

use super::{PrimaryLinkError, bounded_text};

/// A companion-generated QR carries a one-time ADV secret. It is deliberately
/// not printable or serializable; hosts must not put it in logs or plain journals.
pub struct CompanionQr {
    reference: String,
    noise_public_key: [u8; 32],
    identity_public_key: [u8; 32],
    adv_secret_key: Zeroizing<[u8; 32]>,
    platform: String,
}

impl CompanionQr {
    /// Read the five-field format emitted by `PairUtils::make_qr_data`.
    /// The four trailing fields are fixed; a reference can contain commas.
    pub fn parse(input: &str) -> Result<Self, PrimaryLinkError> {
        if input.len() > 8192 {
            return Err(PrimaryLinkError::InvalidQr("size limit"));
        }
        let input = input
            .strip_prefix(crate::companion_reg::NATIVE_CAMERA_DEEP_LINK_PREFIX)
            .unwrap_or(input);
        let mut fields = input.rsplitn(5, ',');
        let platform = fields
            .next()
            .ok_or(PrimaryLinkError::InvalidQr("platform"))?;
        let secret = fields.next().ok_or(PrimaryLinkError::InvalidQr("secret"))?;
        let identity = fields
            .next()
            .ok_or(PrimaryLinkError::InvalidQr("identity"))?;
        let noise = fields.next().ok_or(PrimaryLinkError::InvalidQr("noise"))?;
        let reference = fields
            .next()
            .ok_or(PrimaryLinkError::InvalidQr("reference"))?;
        if !bounded_text(reference, 4096) || !bounded_text(platform, 64) {
            return Err(PrimaryLinkError::InvalidQr("reference or platform"));
        }
        let noise_public_key = decode_key(noise)?;
        let identity_public_key = decode_key(identity)?;
        if noise_public_key == [0; 32] || identity_public_key == [0; 32] {
            return Err(PrimaryLinkError::InvalidQr("zero public key"));
        }
        Ok(Self {
            reference: reference.to_owned(),
            noise_public_key,
            identity_public_key,
            adv_secret_key: Zeroizing::new(decode_key(secret)?),
            platform: platform.to_owned(),
        })
    }

    /// Construct from an authenticated code exchange without serializing its secret.
    pub fn from_parts(
        reference: &str,
        noise_public_key: [u8; 32],
        identity_public_key: [u8; 32],
        adv_secret_key: [u8; 32],
        platform: &str,
    ) -> Result<Self, PrimaryLinkError> {
        if !bounded_text(reference, 4096)
            || !bounded_text(platform, 64)
            || noise_public_key == [0; 32]
            || identity_public_key == [0; 32]
        {
            return Err(PrimaryLinkError::InvalidQr(
                "reference, platform or public key",
            ));
        }
        Ok(Self {
            reference: reference.to_owned(),
            noise_public_key,
            identity_public_key,
            adv_secret_key: Zeroizing::new(adv_secret_key),
            platform: platform.to_owned(),
        })
    }

    pub fn reference(&self) -> &str {
        &self.reference
    }
    pub fn noise_public_key(&self) -> &[u8; 32] {
        &self.noise_public_key
    }
    pub fn identity_public_key(&self) -> &[u8; 32] {
        &self.identity_public_key
    }
    pub fn platform(&self) -> &str {
        &self.platform
    }

    /// One-time secret used only to authenticate the companion's ADV envelope.
    pub fn adv_secret_key(&self) -> &[u8; 32] {
        &self.adv_secret_key
    }
}

fn decode_key(value: &str) -> Result<[u8; 32], PrimaryLinkError> {
    if !(43..=44).contains(&value.len()) {
        return Err(PrimaryLinkError::InvalidQr("key length"));
    }
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        let mut output = [0; 33];
        if let Ok(length) = engine.decode_slice(value, &mut output)
            && length == 32
        {
            let mut key = [0; 32];
            key.copy_from_slice(&output[..32]);
            return Ok(key);
        }
    }
    Err(PrimaryLinkError::InvalidQr("base64 key"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::companion_reg::CompanionWebClientType;
    use crate::libsignal::protocol::KeyPair;
    use crate::pair::{DeviceState, PairUtils};
    use rand::SeedableRng;

    #[test]
    fn parses_actual_companion_encoder_and_native_camera_prefix() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(101);
        let state = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [7; 32],
        };
        let raw = PairUtils::make_qr_data(
            &state,
            "synthetic,reference",
            CompanionWebClientType::Chrome,
        );
        for raw in [
            raw.clone(),
            format!(
                "{}{raw}",
                crate::companion_reg::NATIVE_CAMERA_DEEP_LINK_PREFIX
            ),
        ] {
            let parsed = CompanionQr::parse(&raw).unwrap();
            assert_eq!(parsed.reference(), "synthetic,reference");
            assert_eq!(
                parsed.noise_public_key().as_slice(),
                state.noise_key.public_key.public_key_bytes()
            );
            assert_eq!(
                parsed.identity_public_key().as_slice(),
                state.identity_key.public_key.public_key_bytes()
            );
            assert_eq!(parsed.adv_secret_key(), &[7; 32]);
            assert_eq!(parsed.platform(), "1");
        }
    }

    #[test]
    fn rejects_missing_and_malformed_fields_without_secret_in_error() {
        for raw in [
            "a,b,c",
            ",a,b,c,1",
            "r,not_base64,not_base64,private-secret,1",
        ] {
            let error = CompanionQr::parse(raw).err().unwrap();
            assert!(!error.to_string().contains("private-secret"));
        }
    }
}
