use crate::client_profile::ClientProfile;
use crate::libsignal::protocol::{IdentityKeyPair, KeyPair};
use buffa::Message;
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::sync::{Arc, LazyLock};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Protobuf-bytes serde for `ADVSignedDeviceIdentity` (the generated types lack `Deserialize`).
pub mod account_serde {
    use buffa::Message;
    use waproto::whatsapp as wa;

    pub fn to_bytes(account: &wa::ADVSignedDeviceIdentity) -> Vec<u8> {
        account.encode_to_vec()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<wa::ADVSignedDeviceIdentity, buffa::DecodeError> {
        wa::ADVSignedDeviceIdentity::decode_from_slice(bytes)
    }

    pub fn serialize<S: serde::Serializer>(
        val: &Option<std::sync::Arc<wa::ADVSignedDeviceIdentity>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match val {
            Some(v) => s.serialize_some(&to_bytes(v)),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        d: D,
    ) -> Result<Option<std::sync::Arc<wa::ADVSignedDeviceIdentity>>, D::Error> {
        let bytes: Option<Vec<u8>> = serde::Deserialize::deserialize(d)?;
        match bytes {
            Some(b) => from_bytes(&b)
                .map(|a| Some(std::sync::Arc::new(a)))
                .map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

pub mod key_pair_serde {
    use super::KeyPair;
    use crate::libsignal::protocol::{PrivateKey, PublicKey};
    use serde::{self, Deserializer, Serializer};

    pub fn serialize<S>(key_pair: &KeyPair, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let bytes: Vec<u8> = key_pair
            .private_key
            .serialize()
            .iter()
            .copied()
            .chain(key_pair.public_key.public_key_bytes().iter().copied())
            .collect();
        serializer.serialize_bytes(&bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<KeyPair, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes: Vec<u8> = serde::Deserialize::deserialize(deserializer)?;
        if bytes.len() != 64 {
            return Err(serde::de::Error::invalid_length(bytes.len(), &"64"));
        }
        // reason: serde::de::Error::custom flattens to a String at the boundary —
        // serde's error model has no source-chain preservation.
        let private_key = PrivateKey::deserialize(&bytes[0..32])
            .map_err(|e| serde::de::Error::custom(e.to_string()))?;
        let public_key = PublicKey::from_djb_public_key_bytes(&bytes[32..64])
            .map_err(|e| serde::de::Error::custom(e.to_string()))?;
        Ok(KeyPair::new(public_key, private_key))
    }
}

fn build_base_client_payload(
    app_version: wa::client_payload::user_agent::AppVersion,
    profile: &ClientProfile,
) -> wa::ClientPayload {
    // WA Web (`Client/Payload.js`) never sets `UserAgent.phoneId`; a previous
    // audit auto-generated a UUID per build, which the server flagged as a
    // rotating device fingerprint and silently invalidated the session.
    wa::ClientPayload {
        user_agent: buffa::MessageField::some(wa::client_payload::UserAgent {
            platform: Some(profile.user_agent_platform),
            release_channel: Some(wa::client_payload::user_agent::ReleaseChannel::RELEASE),
            app_version: buffa::MessageField::some(app_version),
            mcc: Some("000".to_string()),
            mnc: Some("000".to_string()),
            os_version: Some(profile.os_version.clone()),
            manufacturer: Some(profile.manufacturer.clone()),
            device: Some(profile.device.clone()),
            os_build_number: Some(profile.os_version.clone()),
            locale_language_iso6391: Some(profile.locale_language.clone()),
            locale_country_iso31661_alpha2: Some(profile.locale_country.clone()),
            phone_id: profile.phone_id.clone(),
            ..Default::default()
        }),
        web_info: if profile.include_web_info {
            buffa::MessageField::some(wa::client_payload::WebInfo {
                web_sub_platform: Some(wa::client_payload::web_info::WebSubPlatform::WEB_BROWSER),
                ..Default::default()
            })
        } else {
            buffa::MessageField::default()
        },
        connect_type: Some(wa::client_payload::ConnectType::WIFI_UNKNOWN),
        connect_reason: Some(wa::client_payload::ConnectReason::USER_ACTIVATED),
        ..Default::default()
    }
}

/// Override for selected `DeviceProps` fields before pairing. `None` fields
/// preserve the current value on the device.
#[derive(Debug, Clone, Default)]
pub struct DevicePropsOverride {
    pub os: Option<String>,
    pub version: Option<wa::device_props::AppVersion>,
    pub platform_type: Option<wa::device_props::PlatformType>,
    pub history_sync_config: Option<wa::device_props::HistorySyncConfig>,
}

impl DevicePropsOverride {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_os(mut self, os: impl Into<String>) -> Self {
        self.os = Some(os.into());
        self
    }

    pub fn with_version(mut self, version: wa::device_props::AppVersion) -> Self {
        self.version = Some(version);
        self
    }

    pub fn with_platform_type(mut self, platform_type: wa::device_props::PlatformType) -> Self {
        self.platform_type = Some(platform_type);
        self
    }

    /// Replaces the entire `HistorySyncConfig`. Spread [`default_history_sync_config`]
    /// into the literal to patch only specific fields while keeping sane defaults.
    pub fn with_history_sync_config(
        mut self,
        history_sync_config: wa::device_props::HistorySyncConfig,
    ) -> Self {
        self.history_sync_config = Some(history_sync_config);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.os.is_none()
            && self.version.is_none()
            && self.platform_type.is_none()
            && self.history_sync_config.is_none()
    }
}

/// Default `HistorySyncConfig` aligned with WA Web's static claims
/// (`Payload.js` in `WAWebClientPayload`). Runtime-derived fields like
/// `storage_quota_mb`, `on_demand_ready`, and the justknobx-gated numeric
/// limits are left unset so callers can populate them through
/// [`DevicePropsOverride::with_history_sync_config`] without fighting stale
/// hardcoded values.
///
/// `support_*` capability flags are advertised as `true`: they tell the
/// server which history payload variants the client can ingest, and the
/// library either handles them or treats them as opaque (no harm). The
/// platform-gated `support_call_log_history` is `false` because the call
/// log history payload is bound to the Windows desktop client.
pub fn default_history_sync_config() -> wa::device_props::HistorySyncConfig {
    wa::device_props::HistorySyncConfig {
        full_sync_days_limit: Some(30),
        inline_initial_payload_in_e2_ee_msg: Some(true),
        support_bot_user_agent_chat_history: Some(true),
        support_cag_reactions_and_polls: Some(true),
        support_recent_sync_chunk_message_count_tuning: Some(true),
        support_hosted_group_msg: Some(true),
        support_biz_hosted_msg: Some(true),
        support_fbid_bot_chat_history: Some(true),
        support_message_association: Some(true),
        support_call_log_history: Some(false),
        support_group_history: Some(true),
        support_manus_history: Some(true),
        support_hatch_history: Some(true),
        ..Default::default()
    }
}

pub static DEVICE_PROPS: LazyLock<wa::DeviceProps> = LazyLock::new(|| wa::DeviceProps {
    os: Some("rust".to_string()),
    version: buffa::MessageField::some(wa::device_props::AppVersion {
        primary: Some(0),
        secondary: Some(1),
        tertiary: Some(0),
        ..Default::default()
    }),
    platform_type: Some(wa::device_props::PlatformType::UNKNOWN),
    require_full_sync: Some(true),
    history_sync_config: buffa::MessageField::some(default_history_sync_config()),
});

#[derive(Clone, Serialize, Deserialize)]
pub struct Device {
    pub pn: Option<Jid>,
    pub lid: Option<Jid>,
    pub registration_id: u32,
    #[serde(with = "key_pair_serde")]
    pub noise_key: KeyPair,
    #[serde(with = "key_pair_serde")]
    pub identity_key: KeyPair,
    #[serde(with = "key_pair_serde")]
    pub signed_pre_key: KeyPair,
    pub signed_pre_key_id: u32,
    #[serde(with = "BigArray")]
    pub signed_pre_key_signature: [u8; 64],
    pub adv_secret_key: [u8; 32],
    // Arc: immutable after pairing, so per-snapshot clones bump a refcount
    // instead of deep-copying its four Vec<u8> fields.
    #[serde(with = "account_serde", default)]
    pub account: Option<Arc<wa::ADVSignedDeviceIdentity>>,
    pub push_name: String,
    pub app_version_primary: u32,
    pub app_version_secondary: u32,
    pub app_version_tertiary: u32,
    pub app_version_last_fetched_ms: i64,
    // Arc: set once at setup then read-only, so snapshot clones bump a refcount;
    // the rare mutations go through `Arc::make_mut`.
    #[serde(skip)]
    pub device_props: Arc<wa::DeviceProps>,
    /// Runtime-only. Set before `connect()` on every process start.
    #[serde(skip)]
    pub client_profile: ClientProfile,
    /// Edge routing info received from server, used for optimized reconnection.
    /// When present, this should be sent as a pre-intro before the Noise handshake.
    #[serde(default)]
    pub edge_routing_info: Option<Vec<u8>>,
    /// Hash from the last props (A/B experiment config) fetch.
    /// Sent on subsequent connects to enable delta updates instead of full fetches.
    #[serde(default)]
    pub props_hash: Option<String>,
    /// Monotonically increasing counter for one-time pre-key ID generation.
    /// Matches WhatsApp Web's `NEXT_PK_ID` pattern: only increases, never resets.
    /// Advances at GENERATION time (WA Web `savePreKeys`), so it covers every
    /// key that exists in the store, uploaded or not.
    #[serde(default)]
    pub next_pre_key_id: u32,
    /// Watermark of the first generated-but-not-yet-uploaded one-time prekey,
    /// matching WA Web's `FIRST_UNUPLOAD_PK_ID`. `next_pre_key_id - this` is
    /// the pool of leftover keys an upload re-offers before generating new
    /// ones. `0` = unset (legacy device); initialised on the first upload.
    #[serde(default)]
    pub first_unupload_pre_key_id: u32,
    /// Persisted flag matching WA Web's `signal_sever_has_pre_keys` metadata.
    #[serde(default)]
    pub server_has_prekeys: bool,
    /// NCT salt provisioned by the server via app state sync or history sync.
    #[serde(default)]
    pub nct_salt: Option<Vec<u8>>,
    /// Runtime-only marker that an authoritative nct_salt_sync mutation was seen.
    /// This prevents stale history sync data from resurrecting a cleared salt.
    #[serde(skip)]
    pub nct_salt_sync_seen: bool,
    /// Server cert chain cached from the last successful XX (or XX-fallback)
    /// handshake. Enables Noise IK on the next connect by exposing
    /// `leaf.key` as the server's static public key, and lets us reject
    /// stale entries via `not_after` before even attempting IK.
    /// `None` forces XX on the next connect.
    #[serde(default)]
    pub server_cert_chain: Option<CachedServerCertChain>,
    /// Login counter sent as `ClientPayload.lc` on every login. WA Web's
    /// `WAWebUserPrefsGeneral.getLoginCounter()` reads (and bumps) this from
    /// localStorage on each connect; the server uses it as an anti-abuse
    /// signal. Persisted so it survives restarts.
    #[serde(default)]
    pub login_counter: i32,
    /// WA Web's `WAIsAccountLidFieldMigrated` pref: whether the account is
    /// 1:1-LID-migrated. Set from `ClientPairingProps.isChatDbLidMigrated` at
    /// pair time or when the primary pushes migration mappings. Gates outbound
    /// DM wire addressing (LID vs PN); the Signal session layer stays LID-first
    /// regardless, mirroring WAWebSignalAddress. Once set it never reverts,
    /// like the WA Web pref.
    #[serde(default)]
    pub lid_migrated: bool,
    /// Wall-clock ms of the last signed-pre-key rotation, driving WA Web's
    /// `RotateKeyJob` cadence. Fresh devices baseline off creation; devices
    /// persisted before this field existed deserialize to `0`, which the
    /// rotation path treats as "seed the baseline, don't rotate yet".
    #[serde(default)]
    pub last_signed_pre_key_rotation_ms: i64,
}

/// Minimal cached form of a Noise certificate. Mirrors the JSON shape WA Web
/// persists in `waNoiseInfo.certificateChainBuffer` (only `key` plus the
/// validity window — signatures and issuer_serial are intentionally dropped).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedNoiseCert {
    /// 32-byte X25519 public key from `NoiseCertificate.Details.key`.
    pub key: [u8; 32],
    /// Unix epoch seconds. Validation window from `NoiseCertificate.Details`.
    pub not_before: i64,
    pub not_after: i64,
}

/// Cached form of the server's two-cert chain. `leaf.key` is the server
/// static public key consumed by Noise IK; the intermediate is kept solely
/// to mirror WA Web's expiry checks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedServerCertChain {
    pub intermediate: CachedNoiseCert,
    pub leaf: CachedNoiseCert,
}

impl From<wacore_noise::VerifiedServerCertChain> for CachedServerCertChain {
    fn from(v: wacore_noise::VerifiedServerCertChain) -> Self {
        Self {
            intermediate: CachedNoiseCert {
                key: v.intermediate_key,
                not_before: v.intermediate_not_before,
                not_after: v.intermediate_not_after,
            },
            leaf: CachedNoiseCert {
                key: v.leaf_key,
                not_before: v.leaf_not_before,
                not_after: v.leaf_not_after,
            },
        }
    }
}

impl Default for Device {
    fn default() -> Self {
        Self::new()
    }
}

impl Device {
    pub fn new() -> Self {
        use rand::{Rng, RngExt};

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity_key_pair = IdentityKeyPair::generate(&mut rng);

        let identity_key: KeyPair = KeyPair::new(
            *identity_key_pair.public_key(),
            identity_key_pair.private_key().clone(),
        );
        let signed_pre_key = KeyPair::generate(&mut rng);
        let signature_box = identity_key_pair
            .private_key()
            .calculate_signature(&signed_pre_key.public_key.serialize(), &mut rng)
            .expect("signing with valid Ed25519 key should succeed");
        let signed_pre_key_signature: [u8; 64] = signature_box
            .as_ref()
            .try_into()
            .expect("Ed25519 signature is always 64 bytes");
        let mut adv_secret_key = [0u8; 32];
        rng.fill_bytes(&mut adv_secret_key);

        Self {
            pn: None,
            lid: None,
            registration_id: rng.random_range(1..=2147483647),
            noise_key: KeyPair::generate(&mut rng),
            identity_key,
            signed_pre_key,
            signed_pre_key_id: 1,
            signed_pre_key_signature,
            adv_secret_key,
            account: None,
            push_name: String::new(),
            app_version_primary: 2,
            app_version_secondary: 3000,
            app_version_tertiary: 1040878135,
            app_version_last_fetched_ms: 0,
            device_props: Arc::new(DEVICE_PROPS.clone()),
            client_profile: ClientProfile::web(),
            edge_routing_info: None,
            props_hash: None,
            next_pre_key_id: 1,
            first_unupload_pre_key_id: 0,
            server_has_prekeys: false,
            nct_salt: None,
            nct_salt_sync_seen: false,
            server_cert_chain: None,
            login_counter: 0,
            lid_migrated: false,
            last_signed_pre_key_rotation_ms: crate::time::now_millis(),
        }
    }

    /// Returns the default OS string used for device props
    pub fn default_os() -> &'static str {
        "rust"
    }

    /// Returns the default device props version
    pub fn default_device_props_version() -> wa::device_props::AppVersion {
        wa::device_props::AppVersion {
            primary: Some(0),
            secondary: Some(1),
            tertiary: Some(0),
            ..Default::default()
        }
    }

    pub fn is_ready_for_presence(&self) -> bool {
        self.pn.is_some() && !self.push_name.is_empty()
    }

    /// Mirrors WA Web `WAWebUserPrefsMultiDevice.isRegistered()`:
    /// `!!(m() && getMaybeMeDevicePn())`.
    pub fn is_registered(&self) -> bool {
        self.pn.is_some()
    }

    pub fn set_device_props(&mut self, o: DevicePropsOverride) {
        let props = Arc::make_mut(&mut self.device_props);
        if let Some(os) = o.os {
            props.os = Some(os);
        }
        if let Some(version) = o.version {
            props.version = buffa::MessageField::some(version);
        }
        if let Some(platform_type) = o.platform_type {
            props.platform_type = Some(platform_type);
        }
        if let Some(history_sync_config) = o.history_sync_config {
            props.history_sync_config = buffa::MessageField::some(history_sync_config);
        }
    }

    pub fn set_client_profile(&mut self, profile: ClientProfile) {
        self.client_profile = profile;
    }

    pub fn get_client_payload(&self) -> wa::ClientPayload {
        match &self.pn {
            Some(jid) => self.get_login_payload(jid),
            None => self.get_registration_payload(),
        }
    }

    fn get_login_payload(&self, jid: &Jid) -> wa::ClientPayload {
        let app_version = wa::client_payload::user_agent::AppVersion {
            primary: Some(self.app_version_primary),
            secondary: Some(self.app_version_secondary),
            tertiary: Some(self.app_version_tertiary),
            ..Default::default()
        };
        let mut payload = build_base_client_payload(app_version, &self.client_profile);
        payload.username = jid.user.parse::<u64>().ok();
        payload.device = Some(jid.device as u32);
        payload.passive = Some(self.client_profile.passive_login);
        // WA Web's `Get/ClientPayloadForLogin.js` hardcodes `pull: true` on
        // the login wrapper; only `passive` is dynamic.
        payload.pull = Some(true);
        payload.lc = Some(self.login_counter);
        // Hardcoded false: no LID migration path here. WA Web sends this on
        // every login so the server can branch on it.
        payload.lid_db_migrated = Some(false);
        payload
    }

    fn get_registration_payload(&self) -> wa::ClientPayload {
        let app_version = wa::client_payload::user_agent::AppVersion {
            primary: Some(self.app_version_primary),
            secondary: Some(self.app_version_secondary),
            tertiary: Some(self.app_version_tertiary),
            ..Default::default()
        };
        let mut payload = build_base_client_payload(app_version, &self.client_profile);

        let device_props_bytes = self.device_props.encode_to_vec();

        let version = &payload.user_agent.app_version;
        let version_str = format!(
            "{}.{}.{}",
            version.primary.unwrap_or(0),
            version.secondary.unwrap_or(0),
            version.tertiary.unwrap_or(0)
        );
        let build_hash: [u8; 16] = md5::compute(version_str.as_bytes()).into();

        let reg_data = wa::client_payload::DevicePairingRegistrationData {
            e_regid: Some(self.registration_id.to_be_bytes().to_vec()),
            e_keytype: Some(vec![5]),
            e_ident: Some(self.identity_key.public_key.public_key_bytes().to_vec()),
            e_skey_id: Some(self.signed_pre_key_id.to_be_bytes()[1..].to_vec()),
            e_skey_val: Some(self.signed_pre_key.public_key.public_key_bytes().to_vec()),
            e_skey_sig: Some(self.signed_pre_key_signature.to_vec()),
            build_hash: Some(build_hash.to_vec()),
            device_props: Some(device_props_bytes),
        };

        payload.device_pairing_data = buffa::MessageField::some(reg_data);
        payload.passive = Some(false);
        payload.pull = Some(false);

        // Include push_name if set — enables deterministic phone assignment in mock server
        if !self.push_name.is_empty() {
            payload.push_name = Some(self.push_name.clone());
        }

        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registration_id_range() {
        for _ in 0..1000 {
            let device = Device::new();
            assert!(device.registration_id >= 1);
            assert!(device.registration_id <= 2147483647);
        }
    }

    #[test]
    fn test_device_serde_roundtrip() {
        // Regression test: key_pair_serde::serialize uses serialize_bytes which
        // produces a JSON integer array. deserialize must use Vec<u8> (not &[u8])
        // to accept a sequence from serde_json; &[u8] would fail with
        // "invalid type: sequence, expected a borrowed byte array".
        let device = Device::new();
        let json = serde_json::to_string(&device).expect("serialize should succeed");
        let restored: Device = serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(device.registration_id, restored.registration_id);
        assert_eq!(
            device.noise_key.public_key.public_key_bytes(),
            restored.noise_key.public_key.public_key_bytes()
        );
        assert_eq!(
            device.identity_key.public_key.public_key_bytes(),
            restored.identity_key.public_key.public_key_bytes()
        );
    }

    #[test]
    fn test_device_server_cert_chain_serde_roundtrip() {
        let mut device = Device::new();
        device.server_cert_chain = Some(CachedServerCertChain {
            intermediate: CachedNoiseCert {
                key: [0xAA; 32],
                not_before: 1_700_000_000,
                not_after: 1_900_000_000,
            },
            leaf: CachedNoiseCert {
                key: [0xBB; 32],
                not_before: 1_700_000_500,
                not_after: 1_899_999_500,
            },
        });

        let json = serde_json::to_string(&device).expect("serialize should succeed");
        let restored: Device = serde_json::from_str(&json).expect("deserialize should succeed");
        assert_eq!(device.server_cert_chain, restored.server_cert_chain);
    }

    #[test]
    fn test_device_legacy_record_without_cert_chain_deserializes() {
        // Devices serialized before this field existed must still load — the
        // #[serde(default)] attribute is what makes that work.
        let mut device = Device::new();
        device.server_cert_chain = None;
        let json = serde_json::to_string(&device).expect("serialize should succeed");
        // Strip the field as if a legacy file lacked it entirely.
        let stripped = json.replace(",\"server_cert_chain\":null", "");
        assert_ne!(stripped, json, "field was expected to be present in JSON");

        let restored: Device =
            serde_json::from_str(&stripped).expect("legacy record should deserialize");
        assert!(restored.server_cert_chain.is_none());
    }

    /// Regression: #403
    #[test]
    fn test_device_serde_preserves_account() {
        let mut device = Device::new();
        device.account = Some(Arc::new(wa::ADVSignedDeviceIdentity {
            details: Some(b"test-details".to_vec()),
            account_signature_key: Some(vec![1; 32]),
            account_signature: Some(vec![2; 64]),
            device_signature: Some(vec![3; 64]),
        }));

        let json = serde_json::to_string(&device).expect("serialize should succeed");
        let restored: Device = serde_json::from_str(&json).expect("deserialize should succeed");

        assert!(
            restored.account.is_some(),
            "account must survive serde roundtrip"
        );
        let acc = restored.account.unwrap();
        assert_eq!(acc.details.as_deref(), Some(b"test-details".as_slice()));
        assert_eq!(
            acc.account_signature_key.as_deref(),
            Some([1u8; 32].as_slice())
        );
        assert_eq!(acc.account_signature.as_deref(), Some([2u8; 64].as_slice()));
        assert_eq!(acc.device_signature.as_deref(), Some([3u8; 64].as_slice()));
    }

    /// Override survives the ClientPayload → bytes → DeviceProps round-trip;
    /// `None` fields preserve the prior value.
    #[test]
    fn set_device_props_override_reaches_registration_payload() {
        let mut device = Device::new();
        assert!(device.pn.is_none());

        device.set_device_props(
            DevicePropsOverride::new()
                .with_os("Android 14")
                .with_platform_type(wa::device_props::PlatformType::ANDROID_PHONE),
        );

        let payload = device.get_client_payload();
        let reg = payload
            .device_pairing_data
            .into_option()
            .expect("device_pairing_data");
        let bytes = reg.device_props.expect("device_props bytes");
        let props =
            wa::DeviceProps::decode_from_slice(bytes.as_slice()).expect("decode DeviceProps");

        assert_eq!(props.os.as_deref(), Some("Android 14"));
        assert_eq!(
            props.platform_type,
            Some(wa::device_props::PlatformType::ANDROID_PHONE)
        );
        // None preserves the default version.
        assert_eq!(
            props.version.as_option(),
            Some(&Device::default_device_props_version())
        );
    }

    /// `HistorySyncConfig` override is delivered whole — users patch by
    /// spreading [`default_history_sync_config`] into the literal.
    #[test]
    fn history_sync_config_override_reaches_registration_payload() {
        let mut device = Device::new();
        device.set_device_props(DevicePropsOverride::new().with_history_sync_config(
            wa::device_props::HistorySyncConfig {
                full_sync_days_limit: Some(365),
                support_group_history: Some(true),
                ..default_history_sync_config()
            },
        ));

        let payload = device.get_client_payload();
        let bytes = payload
            .device_pairing_data
            .into_option()
            .expect("device_pairing_data")
            .device_props
            .expect("device_props bytes");
        let props =
            wa::DeviceProps::decode_from_slice(bytes.as_slice()).expect("decode DeviceProps");
        let hsc = props
            .history_sync_config
            .into_option()
            .expect("history_sync_config");

        assert_eq!(hsc.full_sync_days_limit, Some(365));
        assert_eq!(hsc.support_group_history, Some(true));
        // Defaults spread in via default_history_sync_config() survive.
        assert_eq!(hsc.support_message_association, Some(true));
        assert_eq!(hsc.inline_initial_payload_in_e2_ee_msg, Some(true));
    }

    /// After pairing, `device_props` must not leak into the login payload —
    /// WA Web only sends it during registration.
    #[test]
    fn login_payload_has_no_device_props() {
        let mut device = Device::new();
        device.pn = Some("12345@s.whatsapp.net".parse().unwrap());
        device.set_device_props(
            DevicePropsOverride::new()
                .with_platform_type(wa::device_props::PlatformType::ANDROID_PHONE),
        );

        let payload = device.get_client_payload();
        assert!(
            payload.device_pairing_data.is_unset(),
            "login payload must not carry device_pairing_data"
        );
    }

    #[test]
    fn default_profile_emits_legacy_web_payload() {
        let device = Device::new();
        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert_eq!(
            ua.platform,
            Some(wa::client_payload::user_agent::Platform::WEB)
        );
        assert_eq!(ua.device.as_deref(), Some("Desktop"));
        assert_eq!(ua.os_version.as_deref(), Some("0.1.0"));
        assert_eq!(ua.os_build_number.as_deref(), Some("0.1.0"));
        assert_eq!(ua.manufacturer.as_deref(), Some(""));
        let web_info = payload
            .web_info
            .as_option()
            .expect("web profile must include web_info");
        assert_eq!(
            web_info.web_sub_platform,
            Some(wa::client_payload::web_info::WebSubPlatform::WEB_BROWSER)
        );
    }

    #[test]
    fn android_profile_emits_android_payload_without_web_info() {
        let mut device = Device::new();
        device.set_client_profile(ClientProfile::android("13"));

        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert_eq!(
            ua.platform,
            Some(wa::client_payload::user_agent::Platform::ANDROID)
        );
        assert_eq!(ua.device.as_deref(), Some("Smartphone"));
        assert_eq!(ua.os_version.as_deref(), Some("13"));
        assert_eq!(ua.os_build_number.as_deref(), Some("13"));
        assert!(
            payload.web_info.is_unset(),
            "android profile must omit web_info"
        );
    }

    #[test]
    fn android_profile_survives_login_payload_path() {
        let mut device = Device::new();
        device.set_client_profile(ClientProfile::android("13"));
        device.pn = Some("12345@s.whatsapp.net".parse().unwrap());

        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert_eq!(
            ua.platform,
            Some(wa::client_payload::user_agent::Platform::ANDROID)
        );
        assert!(payload.web_info.is_unset());
        assert!(
            payload.device_pairing_data.is_unset(),
            "login payload still must not carry device_pairing_data"
        );
    }

    #[test]
    fn client_profile_independent_of_device_props_platform_type() {
        let mut device = Device::new();
        device.set_device_props(
            DevicePropsOverride::new()
                .with_platform_type(wa::device_props::PlatformType::ANDROID_PHONE),
        );

        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert_eq!(
            ua.platform,
            Some(wa::client_payload::user_agent::Platform::WEB)
        );
        assert!(payload.web_info.is_set());
    }

    #[test]
    fn every_native_profile_drops_web_info_in_payload() {
        for profile in [
            ClientProfile::android("13"),
            ClientProfile::smb_android("13"),
            ClientProfile::ios("17.4"),
            ClientProfile::macos("14.4"),
            ClientProfile::windows("10.0.22631"),
        ] {
            let mut device = Device::new();
            let platform = profile.user_agent_platform;
            device.set_client_profile(profile);

            let payload = device.get_client_payload();
            let ua = payload.user_agent.as_option().expect("user_agent");
            assert_eq!(ua.platform, Some(platform));
            assert!(
                payload.web_info.is_unset(),
                "{platform:?} must omit web_info"
            );
        }
    }

    /// Per-connect `phone_id` UUID is flagged by the server as a rotating
    /// fingerprint and silently kills the session. Must stay omitted.
    #[test]
    fn phone_id_default_is_omitted_and_payload_is_deterministic() {
        let device = Device::new();
        let payload_a = device.get_client_payload();
        let payload_b = device.get_client_payload();
        let ua_a = payload_a.user_agent.as_option().expect("user_agent");
        let ua_b = payload_b.user_agent.as_option().expect("user_agent");
        assert!(
            ua_a.phone_id.is_none(),
            "default ClientProfile must leave UserAgent.phoneId unset (got {:?})",
            ua_a.phone_id
        );
        assert_eq!(
            ua_a.phone_id, ua_b.phone_id,
            "phoneId must not change between payload builds"
        );
        // Wire-level determinism: encoded bytes must match across builds.
        let bytes_a = payload_a.encode_to_vec();
        let bytes_b = payload_b.encode_to_vec();
        assert_eq!(
            bytes_a, bytes_b,
            "get_client_payload() must be deterministic across calls"
        );
    }

    #[test]
    fn phone_id_passes_through_from_profile_when_set() {
        let mut profile = ClientProfile::web();
        profile.phone_id = Some("fixed-test-id".to_string());
        let mut device = Device::new();
        device.set_client_profile(profile);

        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert_eq!(ua.phone_id.as_deref(), Some("fixed-test-id"));
    }

    #[test]
    fn login_payload_phone_id_is_omitted_by_default() {
        let mut device = Device::new();
        device.pn = Some("12345:0@s.whatsapp.net".parse().unwrap());
        let payload = device.get_client_payload();
        let ua = payload.user_agent.as_option().expect("user_agent");
        assert!(
            ua.phone_id.is_none(),
            "login payload phoneId must be omitted (WA Web compliance)"
        );
    }

    /// `lc` must be bumped per successful login, not stuck at 0.
    #[test]
    fn login_payload_lc_reflects_login_counter() {
        use crate::store::commands::{DeviceCommand, apply_command_to_device};

        let mut device = Device::new();
        device.pn = Some("12345:0@s.whatsapp.net".parse().unwrap());

        // Fresh device: lc = 0.
        assert_eq!(device.get_client_payload().lc, Some(0));

        // After one successful login (one IncrementLoginCounter dispatch),
        // the NEXT payload's lc must be 1.
        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        assert_eq!(device.get_client_payload().lc, Some(1));

        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        assert_eq!(device.get_client_payload().lc, Some(3));
    }

    /// `Get/ClientPayloadForLogin.js` hardcodes `pull: true` on login wrapper.
    #[test]
    fn login_payload_pull_is_true() {
        let mut device = Device::new();
        device.pn = Some("12345:0@s.whatsapp.net".parse().unwrap());
        let payload = device.get_client_payload();
        assert_eq!(
            payload.pull,
            Some(true),
            "login payload must send pull=true (WA Web compliance)"
        );
    }

    #[test]
    fn registration_payload_pull_is_false() {
        // Fresh device without `pn` exercises the registration path.
        let device = Device::new();
        assert!(device.pn.is_none());
        let payload = device.get_client_payload();
        assert_eq!(
            payload.pull,
            Some(false),
            "registration payload must send pull=false"
        );
    }

    /// `lc` is part of the LOGIN payload only. Registration payloads use a
    /// different protobuf path (`get_registration_payload`) that doesn't read
    /// it; the field MUST stay None on the wire there, matching WA Web's
    /// `getClientPayloadForRegistration` which omits the field.
    #[test]
    fn registration_payload_does_not_carry_lc() {
        let device = Device::new();
        assert!(device.pn.is_none());
        let payload = device.get_client_payload();
        assert!(payload.lc.is_none(), "registration payload must omit lc");
    }

    /// `lc` must survive process restarts (WA Web persists in IndexedDB).
    #[test]
    fn login_counter_survives_serde_roundtrip() {
        use crate::store::commands::{DeviceCommand, apply_command_to_device};

        let mut device = Device::new();
        for _ in 0..5 {
            apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        }
        assert_eq!(device.login_counter, 5);

        let json = serde_json::to_string(&device).expect("serialize");
        let restored: Device = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(
            restored.login_counter, 5,
            "login_counter must survive Device serde roundtrip"
        );
    }

    /// Backward compat: missing `account` field deserializes as `None`.
    #[test]
    fn test_device_serde_account_none_and_missing() {
        // None roundtrip
        let device = Device::new();
        assert!(device.account.is_none());
        let json = serde_json::to_string(&device).expect("serialize should succeed");
        let restored: Device = serde_json::from_str(&json).expect("deserialize should succeed");
        assert!(restored.account.is_none());

        // Missing field in JSON (backward compat with old data)
        let mut val: serde_json::Value = serde_json::from_str(&json).expect("parse as Value");
        val.as_object_mut().unwrap().remove("account");
        let restored: Device =
            serde_json::from_value(val).expect("deserialize without account field");
        assert!(restored.account.is_none());
    }
}
