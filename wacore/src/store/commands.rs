use crate::client_profile::ClientProfile;
use crate::libsignal::protocol::KeyPair;
use crate::store::Device;
use crate::store::device::{CachedServerCertChain, DevicePropsOverride};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

// Debug is hand-written below: `KeyPair` deliberately omits `Debug` (its
// private key must never format into logs), so the enum cannot derive it.
#[derive(Clone)]
pub enum DeviceCommand {
    SetId(Option<Jid>),
    SetLid(Option<Jid>),
    SetPushName(String),
    SetAccount(Option<wa::ADVSignedDeviceIdentity>),
    SetAppVersion((u32, u32, u32)),
    SetDeviceProps(DevicePropsOverride),
    SetClientProfile(ClientProfile),
    SetPropsHash(Option<String>),
    /// Update both prekey watermarks in one command, so a generation-time
    /// NEXT advance and a FIRST init/advance can never be observed split.
    /// The watermarks have no single-field setter on purpose: split updates
    /// are how the pre-watermark model lost track of generated keys.
    SetPreKeyWatermarks {
        next_pre_key_id: u32,
        first_unupload_pre_key_id: u32,
    },
    SetAdvSecretKey([u8; 32]),
    SetNctSalt(Option<Vec<u8>>),
    SetNctSaltFromHistorySync(Vec<u8>),
    /// Cache the server cert chain extracted from a successful XX (or
    /// XX-fallback) handshake. Enables Noise IK on the next connect.
    SetServerCertChain(CachedServerCertChain),
    /// Drop the cached server cert chain (e.g. after IK fails with a
    /// crypto-fatal error, signalling that the cached `leaf.key` is stale).
    /// Forces XX on the next connect.
    ClearServerCertChain,
    /// Bump the persisted `lc` (login counter) ahead of a login payload.
    IncrementLoginCounter,
    /// Set the account's 1:1-LID-migrated state. Runtime paths only ever set
    /// `true`, like WA Web's `WAIsAccountLidFieldMigrated` pref; `false` is
    /// reserved for pair-success, where a fresh pairing must not inherit the
    /// previous account's migration state.
    SetLidMigrated(bool),
    /// Install a freshly rotated signed pre-key (WA Web `RotateKeyJob`). Sets
    /// the current key trio and stamps the rotation clock in one command so the
    /// key and its cadence baseline can never be observed split.
    SetSignedPreKey {
        key_pair: KeyPair,
        id: u32,
        signature: [u8; 64],
        rotation_ms: i64,
    },
    /// Seed the rotation cadence baseline without touching the key. Used once
    /// for devices upgraded in with `last_signed_pre_key_rotation_ms == 0`, so
    /// the first rotation is scheduled a full interval out rather than firing
    /// immediately on the next connect.
    SetSignedPreKeyRotationBaseline(i64),
}

impl std::fmt::Debug for DeviceCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SetId(v) => f.debug_tuple("SetId").field(v).finish(),
            Self::SetLid(v) => f.debug_tuple("SetLid").field(v).finish(),
            Self::SetPushName(v) => f.debug_tuple("SetPushName").field(v).finish(),
            Self::SetAccount(v) => f.debug_tuple("SetAccount").field(v).finish(),
            Self::SetAppVersion(v) => f.debug_tuple("SetAppVersion").field(v).finish(),
            Self::SetDeviceProps(v) => f.debug_tuple("SetDeviceProps").field(v).finish(),
            Self::SetClientProfile(v) => f.debug_tuple("SetClientProfile").field(v).finish(),
            Self::SetPropsHash(v) => f.debug_tuple("SetPropsHash").field(v).finish(),
            Self::SetPreKeyWatermarks {
                next_pre_key_id,
                first_unupload_pre_key_id,
            } => f
                .debug_struct("SetPreKeyWatermarks")
                .field("next_pre_key_id", next_pre_key_id)
                .field("first_unupload_pre_key_id", first_unupload_pre_key_id)
                .finish(),
            Self::SetAdvSecretKey(_) => f.write_str("SetAdvSecretKey(..)"),
            Self::SetNctSalt(v) => f.debug_tuple("SetNctSalt").field(v).finish(),
            Self::SetNctSaltFromHistorySync(v) => {
                f.debug_tuple("SetNctSaltFromHistorySync").field(v).finish()
            }
            Self::SetServerCertChain(v) => f.debug_tuple("SetServerCertChain").field(v).finish(),
            Self::ClearServerCertChain => f.write_str("ClearServerCertChain"),
            Self::IncrementLoginCounter => f.write_str("IncrementLoginCounter"),
            Self::SetLidMigrated(v) => f.debug_tuple("SetLidMigrated").field(v).finish(),
            // Redact key material; only the id and cadence stamp are logged.
            Self::SetSignedPreKey {
                id, rotation_ms, ..
            } => f
                .debug_struct("SetSignedPreKey")
                .field("id", id)
                .field("rotation_ms", rotation_ms)
                .finish_non_exhaustive(),
            Self::SetSignedPreKeyRotationBaseline(v) => f
                .debug_tuple("SetSignedPreKeyRotationBaseline")
                .field(v)
                .finish(),
        }
    }
}

pub fn apply_command_to_device(device: &mut Device, command: DeviceCommand) {
    match command {
        DeviceCommand::SetId(id) => {
            device.pn = id;
        }
        DeviceCommand::SetLid(lid) => {
            device.lid = lid;
        }
        DeviceCommand::SetPushName(name) => {
            device.push_name = name;
        }
        DeviceCommand::SetAccount(account) => {
            device.account = account.map(std::sync::Arc::new);
        }
        DeviceCommand::SetAppVersion((p, s, t)) => {
            device.app_version_primary = p;
            device.app_version_secondary = s;
            device.app_version_tertiary = t;
            device.app_version_last_fetched_ms = crate::time::now_millis();
        }
        DeviceCommand::SetDeviceProps(override_) => {
            device.set_device_props(override_);
        }
        DeviceCommand::SetClientProfile(profile) => {
            device.set_client_profile(profile);
        }
        DeviceCommand::SetPropsHash(hash) => {
            device.props_hash = hash;
        }
        DeviceCommand::SetPreKeyWatermarks {
            next_pre_key_id,
            first_unupload_pre_key_id,
        } => {
            device.next_pre_key_id = next_pre_key_id;
            device.first_unupload_pre_key_id = first_unupload_pre_key_id;
        }
        DeviceCommand::SetAdvSecretKey(key) => {
            device.adv_secret_key = key;
        }
        DeviceCommand::SetNctSalt(salt) => {
            device.nct_salt = salt;
            device.nct_salt_sync_seen = true;
        }
        DeviceCommand::SetNctSaltFromHistorySync(salt) => {
            if !salt.is_empty() && !device.nct_salt_sync_seen && device.nct_salt.is_none() {
                device.nct_salt = Some(salt);
            }
        }
        DeviceCommand::SetServerCertChain(chain) => {
            device.server_cert_chain = Some(chain);
        }
        DeviceCommand::ClearServerCertChain => {
            device.server_cert_chain = None;
        }
        DeviceCommand::IncrementLoginCounter => {
            device.login_counter = device.login_counter.saturating_add(1);
        }
        DeviceCommand::SetLidMigrated(migrated) => {
            device.lid_migrated = migrated;
        }
        DeviceCommand::SetSignedPreKey {
            key_pair,
            id,
            signature,
            rotation_ms,
        } => {
            device.signed_pre_key = key_pair;
            device.signed_pre_key_id = id;
            device.signed_pre_key_signature = signature;
            device.last_signed_pre_key_rotation_ms = rotation_ms;
        }
        DeviceCommand::SetSignedPreKeyRotationBaseline(rotation_ms) => {
            device.last_signed_pre_key_rotation_ms = rotation_ms;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceCommand, apply_command_to_device};
    use crate::store::Device;
    use crate::store::device::{CachedNoiseCert, CachedServerCertChain};

    fn dummy_chain() -> CachedServerCertChain {
        CachedServerCertChain {
            intermediate: CachedNoiseCert {
                key: [0x11; 32],
                not_before: 1_700_000_000,
                not_after: 1_900_000_000,
            },
            leaf: CachedNoiseCert {
                key: [0x22; 32],
                not_before: 1_700_000_100,
                not_after: 1_899_999_900,
            },
        }
    }

    #[test]
    fn set_server_cert_chain_populates_field() {
        let mut device = Device::new();
        assert!(device.server_cert_chain.is_none());

        let chain = dummy_chain();
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetServerCertChain(chain.clone()),
        );
        assert_eq!(device.server_cert_chain, Some(chain));
    }

    #[test]
    fn clear_server_cert_chain_drops_field() {
        // Seed via the command path rather than mutating Device directly,
        // so that the test exercises the same single mutation surface used
        // in production (PersistenceManager::process_command -> apply_*).
        let mut device = Device::new();
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetServerCertChain(dummy_chain()),
        );
        assert!(device.server_cert_chain.is_some(), "seed precondition");

        apply_command_to_device(&mut device, DeviceCommand::ClearServerCertChain);
        assert!(device.server_cert_chain.is_none());
    }

    #[test]
    fn set_then_clear_roundtrips() {
        let mut device = Device::new();
        let chain = dummy_chain();
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetServerCertChain(chain.clone()),
        );
        assert_eq!(device.server_cert_chain.as_ref(), Some(&chain));
        apply_command_to_device(&mut device, DeviceCommand::ClearServerCertChain);
        assert!(device.server_cert_chain.is_none());
    }

    #[test]
    fn test_history_sync_salt_backfills_when_no_syncd_mutation_was_seen() {
        let mut device = Device::new();
        let salt = vec![1, 2, 3, 4];

        apply_command_to_device(
            &mut device,
            DeviceCommand::SetNctSaltFromHistorySync(salt.clone()),
        );

        assert_eq!(device.nct_salt, Some(salt));
        assert!(!device.nct_salt_sync_seen);
    }

    #[test]
    fn test_history_sync_salt_does_not_resurrect_after_remove() {
        let mut device = Device::new();

        apply_command_to_device(&mut device, DeviceCommand::SetNctSalt(None));
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetNctSaltFromHistorySync(vec![9, 9, 9]),
        );

        assert_eq!(device.nct_salt, None);
        assert!(device.nct_salt_sync_seen);
    }

    #[test]
    fn test_history_sync_salt_does_not_overwrite_syncd_value() {
        let mut device = Device::new();
        let syncd_salt = vec![7, 8, 9];

        apply_command_to_device(
            &mut device,
            DeviceCommand::SetNctSalt(Some(syncd_salt.clone())),
        );
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetNctSaltFromHistorySync(vec![1, 2, 3]),
        );

        assert_eq!(device.nct_salt, Some(syncd_salt));
        assert!(device.nct_salt_sync_seen);
    }

    #[test]
    fn increment_login_counter_bumps_and_saturates() {
        let mut device = Device::new();
        assert_eq!(device.login_counter, 0);

        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        assert_eq!(device.login_counter, 2);

        device.login_counter = i32::MAX;
        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        assert_eq!(device.login_counter, i32::MAX);
    }

    #[test]
    fn set_lid_migrated_roundtrips() {
        let mut device = Device::new();
        assert!(!device.lid_migrated);

        apply_command_to_device(&mut device, DeviceCommand::SetLidMigrated(true));
        assert!(device.lid_migrated);

        // Reset happens only at pair time, so a fresh pairing does not
        // inherit the previous account's migration state.
        apply_command_to_device(&mut device, DeviceCommand::SetLidMigrated(false));
        assert!(!device.lid_migrated);
    }

    #[test]
    fn set_signed_pre_key_rotates_trio_and_stamps_clock() {
        use crate::libsignal::protocol::KeyPair;

        let mut device = Device::new();
        let old_kp_bytes = device.signed_pre_key.public_key.public_key_bytes().to_vec();
        let old_id = device.signed_pre_key_id;

        let new_kp = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let new_pub = new_kp.public_key.public_key_bytes().to_vec();
        let sig = [7u8; 64];

        apply_command_to_device(
            &mut device,
            DeviceCommand::SetSignedPreKey {
                key_pair: new_kp,
                id: old_id + 1,
                signature: sig,
                rotation_ms: 1_234,
            },
        );

        assert_ne!(
            device.signed_pre_key.public_key.public_key_bytes().to_vec(),
            old_kp_bytes
        );
        assert_eq!(
            device.signed_pre_key.public_key.public_key_bytes(),
            &new_pub[..]
        );
        assert_eq!(device.signed_pre_key_id, old_id + 1);
        assert_eq!(device.signed_pre_key_signature, sig);
        assert_eq!(device.last_signed_pre_key_rotation_ms, 1_234);
    }

    #[test]
    fn set_rotation_baseline_leaves_key_untouched() {
        let mut device = Device::new();
        let key_before = device.signed_pre_key.public_key.public_key_bytes().to_vec();
        let id_before = device.signed_pre_key_id;
        let sig_before = device.signed_pre_key_signature;

        apply_command_to_device(
            &mut device,
            DeviceCommand::SetSignedPreKeyRotationBaseline(9_999),
        );

        assert_eq!(device.last_signed_pre_key_rotation_ms, 9_999);
        assert_eq!(
            device.signed_pre_key.public_key.public_key_bytes().to_vec(),
            key_before
        );
        assert_eq!(device.signed_pre_key_id, id_before);
        assert_eq!(device.signed_pre_key_signature, sig_before);
    }

    #[test]
    fn test_history_sync_empty_salt_is_ignored() {
        let mut device = Device::new();

        apply_command_to_device(
            &mut device,
            DeviceCommand::SetNctSaltFromHistorySync(vec![]),
        );

        assert_eq!(device.nct_salt, None);
        assert!(!device.nct_salt_sync_seen);
    }
}
