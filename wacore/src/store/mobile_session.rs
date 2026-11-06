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
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BindingWire {
    schema_version: u32,
    environment_hash: [u8; 32],
    identity_hash: [u8; 32],
    platform: StoredMobilePlatform,
    role: StoredMobileRole,
    #[serde(deserialize_with = "deserialize_secret_bytes")]
    registration_context: zeroize::Zeroizing<Vec<u8>>,
    #[serde(default, deserialize_with = "deserialize_optional_secret_bytes")]
    registration_checkpoint: Option<zeroize::Zeroizing<Vec<u8>>>,
    #[serde(default, deserialize_with = "deserialize_optional_secret_bytes")]
    primary_state: Option<zeroize::Zeroizing<Vec<u8>>>,
}

const MAX_BINDING_FIELD_BYTES: usize = 65536;

/// Decode into protected storage so malformed later fields also clear earlier
/// credentials. Enforce bounds before allocation or each sequence append.
fn deserialize_secret_bytes<'de, D>(
    deserializer: D,
) -> std::result::Result<zeroize::Zeroizing<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct SecretBytes;
    impl<'de> serde::de::Visitor<'de> for SecretBytes {
        type Value = zeroize::Zeroizing<Vec<u8>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("at most 65536 credential bytes")
        }

        fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let size_hint = sequence.size_hint();
            if size_hint.is_some_and(|size| size > MAX_BINDING_FIELD_BYTES) {
                return Err(serde::de::Error::custom(
                    "mobile binding field exceeds limit",
                ));
            }
            let capacity = size_hint.unwrap_or(0).min(MAX_BINDING_FIELD_BYTES);
            let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(capacity));
            while let Some(byte) = sequence.next_element::<u8>()? {
                if bytes.len() == MAX_BINDING_FIELD_BYTES {
                    return Err(serde::de::Error::custom(
                        "mobile binding field exceeds limit",
                    ));
                }
                if bytes.len() == bytes.capacity() {
                    grow_secret_bytes(&mut bytes);
                }
                bytes.push(byte);
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_seq(SecretBytes)
}

/// Keep the retired allocation protected while its replacement copies the
/// bytes. Its owner wipes it on drop before the decoder appends again.
fn grow_secret_bytes(bytes: &mut zeroize::Zeroizing<Vec<u8>>) {
    let capacity = bytes
        .capacity()
        .saturating_mul(2)
        .clamp(8, MAX_BINDING_FIELD_BYTES);
    debug_assert!(bytes.len() == bytes.capacity() && bytes.len() < capacity);
    let replacement = zeroize::Zeroizing::new(Vec::with_capacity(capacity));
    let retired = std::mem::replace(bytes, replacement);
    bytes.extend_from_slice(retired.as_slice());
}

fn deserialize_optional_secret_bytes<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<zeroize::Zeroizing<Vec<u8>>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct OptionalSecretBytes;
    impl<'de> serde::de::Visitor<'de> for OptionalSecretBytes {
        type Value = Option<zeroize::Zeroizing<Vec<u8>>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("an optional credential byte buffer")
        }

        fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserialize_secret_bytes(deserializer).map(Some)
        }
    }
    deserializer.deserialize_option(OptionalSecretBytes)
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
            .is_some_and(|v| v.is_empty() || v.len() > MAX_BINDING_FIELD_BYTES)
            || self.schema_version != 1
            || self.registration_context.len() > MAX_BINDING_FIELD_BYTES
            || self
                .registration_checkpoint
                .as_ref()
                .is_some_and(|v| v.is_empty() || v.len() > MAX_BINDING_FIELD_BYTES)
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
    fn try_from(mut v: BindingWire) -> Result<Self> {
        let value = Self {
            schema_version: v.schema_version,
            environment_hash: v.environment_hash,
            identity_hash: v.identity_hash,
            platform: v.platform,
            role: v.role,
            registration_context: std::mem::take(&mut *v.registration_context),
            registration_checkpoint: v
                .registration_checkpoint
                .map(|mut bytes| std::mem::take(&mut *bytes)),
            primary_state: v.primary_state.map(|mut bytes| std::mem::take(&mut *bytes)),
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
    #[test]
    fn credential_fields_reject_oversized_streams_before_reading_to_the_end() {
        let reads = std::cell::Cell::new(0);
        let bytes = std::iter::from_fn(|| {
            let count = reads.get() + 1;
            reads.set(count);
            assert!(count <= MAX_BINDING_FIELD_BYTES + 1);
            Some(7_u8)
        });
        let deserializer =
            serde::de::value::SeqDeserializer::<_, serde::de::value::Error>::new(bytes);
        assert!(deserialize_secret_bytes(deserializer).is_err());
        assert_eq!(reads.get(), MAX_BINDING_FIELD_BYTES + 1);
    }

    #[test]
    fn credential_fields_keep_optional_defaults_and_reject_partial_records() {
        let binding = MobileSessionBinding::new(
            [1; 32],
            [2; 32],
            StoredMobilePlatform::Android,
            vec![7; MAX_BINDING_FIELD_BYTES],
            Some(vec![8; MAX_BINDING_FIELD_BYTES]),
        )
        .unwrap();
        let value = serde_json::to_value(&binding).unwrap();
        for field in [
            "registration_context",
            "registration_checkpoint",
            "primary_state",
        ] {
            let mut oversized = value.clone();
            oversized[field] = serde_json::json!(vec![9_u8; MAX_BINDING_FIELD_BYTES + 1]);
            assert!(serde_json::from_value::<MobileSessionBinding>(oversized).is_err());
        }
        let mut missing_optional = value.clone();
        let fields = missing_optional.as_object_mut().unwrap();
        fields.remove("registration_checkpoint");
        fields.remove("primary_state");
        let restored: MobileSessionBinding = serde_json::from_value(missing_optional).unwrap();
        assert!(restored.registration_checkpoint.is_none());
        assert!(restored.primary_state.is_none());

        let mut malformed = value;
        malformed["primary_state"] = serde_json::json!("malformed after credential fields");
        assert!(serde_json::from_value::<MobileSessionBinding>(malformed).is_err());
        assert_eq!(
            serde_json::from_slice::<MobileSessionBinding>(&serde_json::to_vec(&binding).unwrap())
                .unwrap(),
            binding
        );
    }
    #[test]
    fn secret_growth_preserves_bytes_and_reserves_capacity_before_append() {
        for capacity in [1, 8, 15, MAX_BINDING_FIELD_BYTES / 2] {
            let mut bytes = zeroize::Zeroizing::new(vec![0xA5; capacity]);
            grow_secret_bytes(&mut bytes);

            assert_eq!(bytes.len(), capacity);
            assert!(bytes.iter().all(|byte| *byte == 0xA5));
            assert!(bytes.capacity() > capacity);
            assert!(bytes.capacity() <= MAX_BINDING_FIELD_BYTES);

            let replacement_pointer = bytes.as_ptr();
            bytes.push(0x7C);
            assert_eq!(bytes.as_ptr(), replacement_pointer);
            assert_eq!(bytes.len(), capacity + 1);
            assert_eq!(bytes[capacity], 0x7C);
        }
    }

    #[test]
    fn json_credentials_keep_geometric_capacity_and_move_without_reallocation() {
        for len in [0, 1, 7, 8, 9, 31, 32, 33, MAX_BINDING_FIELD_BYTES] {
            let mut binding = MobileSessionBinding::new(
                [1; 32],
                [2; 32],
                StoredMobilePlatform::Android,
                vec![7; len],
                (len != 0).then(|| vec![8; len]),
            )
            .unwrap();
            binding.primary_state = (len != 0).then(|| vec![9; len]);
            let encoded = serde_json::to_vec(&binding).unwrap();
            let wire: BindingWire = serde_json::from_slice(&encoded).unwrap();
            let context_pointer = wire.registration_context.as_ptr();
            let checkpoint_pointer = wire
                .registration_checkpoint
                .as_ref()
                .map(|bytes| bytes.as_ptr());
            let primary_pointer = wire.primary_state.as_ref().map(|bytes| bytes.as_ptr());
            let decoded = MobileSessionBinding::try_from(wire).unwrap();
            assert_eq!(decoded.registration_context.as_ptr(), context_pointer);
            assert_eq!(
                decoded
                    .registration_checkpoint
                    .as_ref()
                    .map(|bytes| bytes.as_ptr()),
                checkpoint_pointer
            );
            assert_eq!(
                decoded.primary_state.as_ref().map(|bytes| bytes.as_ptr()),
                primary_pointer
            );
            assert_eq!(decoded, binding);
            for bytes in std::iter::once(&decoded.registration_context)
                .chain(decoded.registration_checkpoint.iter())
                .chain(decoded.primary_state.iter())
            {
                let geometric_limit = len.next_power_of_two().max(8);
                assert!(bytes.capacity() <= geometric_limit);
                assert!(bytes.capacity() <= MAX_BINDING_FIELD_BYTES);
            }

            let mut malformed = serde_json::to_value(&binding).unwrap();
            malformed["primary_state"] = serde_json::json!("malformed after credentials");
            assert!(
                serde_json::from_slice::<MobileSessionBinding>(
                    &serde_json::to_vec(&malformed).unwrap()
                )
                .is_err()
            );
        }
    }
}
