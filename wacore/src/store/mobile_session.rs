//! Persistent role binding, retained even when mobile support is disabled.
use super::error::{Result, StoreError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredMobilePlatform {
    Android,
    AndroidBusiness,
    Ios,
    IosBusiness,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredMobileRole {
    Primary,
}

/// Registration context may contain credentials; its Debug representation is redacted.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "BindingWire")]
pub struct MobileSessionBinding {
    pub schema_version: u32,
    pub environment_hash: [u8; 32],
    pub identity_hash: [u8; 32],
    pub platform: StoredMobilePlatform,
    pub role: StoredMobileRole,
    pub registration_context: Vec<u8>,
    pub registration_checkpoint: Option<Vec<u8>>,
    #[serde(default)]
    pub primary_state: Option<Vec<u8>>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingWire {
    schema_version: u32,
    environment_hash: [u8; 32],
    identity_hash: [u8; 32],
    platform: StoredMobilePlatform,
    role: StoredMobileRole,
    registration_context: Vec<u8>,
    registration_checkpoint: Option<Vec<u8>>,
    #[serde(default)]
    primary_state: Option<Vec<u8>>,
}
impl MobileSessionBinding {
    pub fn new(
        environment_hash: [u8; 32],
        identity_hash: [u8; 32],
        platform: StoredMobilePlatform,
        registration_context: Vec<u8>,
        registration_checkpoint: Option<Vec<u8>>,
    ) -> Result<Self> {
        let value = Self {
            schema_version: 1,
            environment_hash,
            identity_hash,
            platform,
            role: StoredMobileRole::Primary,
            registration_context,
            registration_checkpoint,
            primary_state: None,
        };
        value.validate()?;
        Ok(value)
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .primary_state
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 65536)
            || self.schema_version != 1
            || self.registration_context.len() > 65536
            || self
                .registration_checkpoint
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > 65536)
        {
            return Err(StoreError::Validation(
                "invalid mobile session binding".into(),
            ));
        }
        Ok(())
    }
}
impl Drop for MobileSessionBinding {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.registration_context.zeroize();
        if let Some(value) = &mut self.registration_checkpoint {
            value.zeroize();
        }
        if let Some(value) = &mut self.primary_state {
            value.zeroize();
        }
    }
}

impl std::fmt::Debug for MobileSessionBinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MobileSessionBinding")
            .field("schema_version", &self.schema_version)
            .field("platform", &self.platform)
            .field("role", &self.role)
            .finish_non_exhaustive()
    }
}
impl TryFrom<BindingWire> for MobileSessionBinding {
    type Error = StoreError;
    fn try_from(v: BindingWire) -> Result<Self> {
        let value = Self {
            schema_version: v.schema_version,
            environment_hash: v.environment_hash,
            identity_hash: v.identity_hash,
            platform: v.platform,
            role: v.role,
            registration_context: v.registration_context,
            registration_checkpoint: v.registration_checkpoint,
            primary_state: v.primary_state,
        };
        value.validate()?;
        Ok(value)
    }
}
/// Validate the canonical E.164 primary-account JID used by mobile login.
/// No minimum phone length is assumed here; registration providers own that policy.
pub fn validate_mobile_primary_pn(pn: &wacore_binary::Jid) -> Result<()> {
    if pn.server != wacore_binary::Server::Pn
        || pn.device != 0
        || pn.agent != 0
        || pn.integrator != 0
        || pn.user.is_empty()
        || pn.user.len() > 15
        || !pn.user.bytes().all(|v| v.is_ascii_digit())
        || !pn
            .user
            .as_bytes()
            .first()
            .is_some_and(|v| (b'1'..=b'9').contains(v))
    {
        return Err(StoreError::Validation(
            "invalid canonical mobile primary identity".into(),
        ));
    }
    Ok(())
}

/// Caller-supplied mobile key material. Secret fields never implement Debug.
#[derive(Clone)]
pub struct MobileDeviceKeys {
    pub registration_id: u32,
    pub noise_key: crate::libsignal::protocol::KeyPair,
    pub identity_key: crate::libsignal::protocol::KeyPair,
    pub signed_pre_key: crate::libsignal::protocol::KeyPair,
    pub signed_pre_key_id: u32,
    pub signed_pre_key_signature: [u8; 64],
    pub adv_secret_key: [u8; 32],
}
impl MobileDeviceKeys {
    pub fn into_device(self, binding: MobileSessionBinding) -> Result<super::Device> {
        super::Device::from_mobile_keys(
            binding,
            self.registration_id,
            self.noise_key,
            self.identity_key,
            self.signed_pre_key,
            self.signed_pre_key_id,
            self.signed_pre_key_signature,
            self.adv_secret_key,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn primary_pn_is_canonical_e164_without_an_invented_minimum() {
        for value in ["1@s.whatsapp.net", "123456789012345@s.whatsapp.net"] {
            validate_mobile_primary_pn(&value.parse().unwrap()).unwrap();
        }
        for value in [
            "0@s.whatsapp.net",
            "012345@s.whatsapp.net",
            "1234567890123456@s.whatsapp.net",
            "123:1@s.whatsapp.net",
            "123@lid",
            "123@g.us",
        ] {
            assert!(validate_mobile_primary_pn(&value.parse().unwrap()).is_err());
        }
    }

    #[test]
    fn imported_key_material_rejects_signature_and_identity_drift() {
        let device = super::super::Device::new();
        let binding = MobileSessionBinding::new(
            [1; 32],
            device.mobile_identity_hash(),
            StoredMobilePlatform::Android,
            b"retained-registration-secret".to_vec(),
            None,
        )
        .unwrap();
        let keys = MobileDeviceKeys {
            registration_id: device.registration_id,
            noise_key: device.noise_key.clone(),
            identity_key: device.identity_key.clone(),
            signed_pre_key: device.signed_pre_key.clone(),
            signed_pre_key_id: device.signed_pre_key_id,
            signed_pre_key_signature: device.signed_pre_key_signature,
            adv_secret_key: device.adv_secret_key,
        };
        let imported = keys.clone().into_device(binding.clone()).unwrap();
        let snapshot = imported.clone();
        assert!(std::sync::Arc::ptr_eq(
            imported.mobile_session.as_ref().unwrap(),
            snapshot.mobile_session.as_ref().unwrap()
        ));
        drop(imported);
        assert_eq!(
            snapshot
                .mobile_session
                .as_ref()
                .unwrap()
                .registration_context
                .as_slice(),
            b"retained-registration-secret"
        );
        let mut unsigned = keys.clone();
        unsigned.signed_pre_key_signature = [0; 64];
        assert!(unsigned.into_device(binding.clone()).is_err());
        let mut drifted = binding;
        drifted.identity_hash = [0; 32];
        assert!(keys.into_device(drifted).is_err());
    }

    #[test]
    fn strict_binding_deserialization_and_redacted_debug() {
        let binding = MobileSessionBinding::new(
            [1; 32],
            [2; 32],
            StoredMobilePlatform::Android,
            b"credential-secret".to_vec(),
            Some(vec![7]),
        )
        .unwrap();
        let mut value = serde_json::to_value(&binding).unwrap();
        assert!(!format!("{binding:?}").contains("credential-secret"));
        assert_eq!(
            serde_json::from_value::<MobileSessionBinding>(value.clone()).unwrap(),
            binding
        );
        value["schema_version"] = 2.into();
        assert!(serde_json::from_value::<MobileSessionBinding>(value).is_err());
        assert!(
            MobileSessionBinding::new(
                [1; 32],
                [2; 32],
                StoredMobilePlatform::Android,
                vec![0; 65537],
                None
            )
            .is_err()
        );
    }
}
