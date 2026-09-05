//! ADV (Advanced Device Verification) key index utilities.
//!
//! Decodes `ADVSignedKeyIndexList` protobuf from `key-index-list` elements
//! and filters device lists by `valid_indexes`.
//!
//! Reference: WAWebHandleAdvDeviceNotificationUtils.decodeSignedKeyIndexBytes()

use buffa::view::MessageView as _;
use smallvec::SmallVec;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp::ADVEncryptionType;

use crate::libsignal::protocol::PublicKey;
use crate::store::traits::DeviceInfo;

const ADV_PREFIX_LEN: usize = 2;

// WAWebAdvSignatureConstants.
pub(crate) const ADV_PREFIX_ACCOUNT_SIGNATURE: [u8; ADV_PREFIX_LEN] = [6, 0];
const ADV_PREFIX_DEVICE_SIGNATURE: [u8; ADV_PREFIX_LEN] = [6, 1];
pub(crate) const ADV_HOSTED_PREFIX_ACCOUNT_SIGNATURE: [u8; ADV_PREFIX_LEN] = [6, 5];
const ADV_HOSTED_PREFIX_DEVICE_SIGNATURE: [u8; ADV_PREFIX_LEN] = [6, 6];

fn account_signature_prefix(
    device_type: Option<ADVEncryptionType>,
) -> &'static [u8; ADV_PREFIX_LEN] {
    // WAWebAdvSignatureApi uses the signed deviceType, independently of accountType.
    if device_type == Some(ADVEncryptionType::HOSTED) {
        &ADV_HOSTED_PREFIX_ACCOUNT_SIGNATURE
    } else {
        &ADV_PREFIX_ACCOUNT_SIGNATURE
    }
}

type AdvSigBuffer = SmallVec<[u8; 256]>;

#[derive(Clone, Copy)]
pub(crate) enum DeviceSignatureKind {
    Companion,
    Hosted,
}

pub(crate) struct AccountSignatureMessage(AdvSigBuffer);

impl AccountSignatureMessage {
    pub(crate) fn new(
        details: &[u8],
        identity: &PublicKey,
        device_type: Option<ADVEncryptionType>,
    ) -> Self {
        let identity = identity.public_key_bytes();
        let mut message =
            AdvSigBuffer::with_capacity(ADV_PREFIX_LEN + details.len() + identity.len());
        message.extend_from_slice(account_signature_prefix(device_type));
        message.extend_from_slice(details);
        message.extend_from_slice(identity);
        Self(message)
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn for_device(
        &self,
        account_key: &PublicKey,
        kind: DeviceSignatureKind,
    ) -> AdvSigBuffer {
        let account_key = account_key.public_key_bytes();
        let mut message = AdvSigBuffer::with_capacity(self.0.len() + account_key.len());
        message.extend_from_slice(&self.0);
        message[..ADV_PREFIX_LEN].copy_from_slice(match kind {
            DeviceSignatureKind::Companion => &ADV_PREFIX_DEVICE_SIGNATURE,
            DeviceSignatureKind::Hosted => &ADV_HOSTED_PREFIX_DEVICE_SIGNATURE,
        });
        message.extend_from_slice(account_key);
        message
    }
}

#[cfg(any(test, feature = "test-util"))]
#[doc(hidden)]
pub mod test_util;

/// Outcome of validating a fetched companion device's `ADVSignedDeviceIdentity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvValidation {
    /// Both signatures verified against an available account key — trusted.
    Valid,
    /// The blob is malformed, or the signatures failed to verify against an
    /// available account key. A relay swapping in a forged identity lands here;
    /// the caller must reject the bundle.
    Invalid,
    /// The blob is structurally well-formed, but no account key is available.
    /// Neither the blob nor the fallback supplies one, so the signatures cannot
    /// be verified. The caller decides whether to proceed without verification.
    NoAccountKey,
}

/// Verify a fetched companion device's `ADVSignedDeviceIdentity` binds the
/// fetched identity key to the account's ADV chain, mirroring WA Web
/// `WAWebAdvSignatureApi.validateADVwithIdentityKey`.
///
/// The account prefix follows the signed `ADVDeviceIdentity.device_type`; the
/// device prefix follows `device_jid.is_hosted()`. Both signatures must verify
/// with those exact prefixes, including mixed hosted-account/regular-device chains.
/// An in-blob account key takes precedence over `account_identity_fallback`.
/// Callers must supply the fetched device's JID, preserving its hosting identity.
pub fn validate_adv_with_identity_key(
    device_identity_bytes: &[u8],
    fetched_identity_key: &[u8; 32],
    account_identity_fallback: Option<&[u8; 32]>,
    device_jid: &Jid,
) -> AdvValidation {
    let Ok(signed) =
        waproto::whatsapp::ADVSignedDeviceIdentityView::decode_view(device_identity_bytes)
    else {
        return AdvValidation::Invalid;
    };
    let (Some(details), Some(account_sig), Some(device_sig)) = (
        signed.details,
        signed.account_signature,
        signed.device_signature,
    ) else {
        return AdvValidation::Invalid;
    };
    let (Ok(account_sig), Ok(device_sig)) = (
        <&[u8; 64]>::try_from(account_sig),
        <&[u8; 64]>::try_from(device_sig),
    ) else {
        return AdvValidation::Invalid;
    };
    let Ok(identity_details) = waproto::whatsapp::ADVDeviceIdentityView::decode_view(details)
    else {
        return AdvValidation::Invalid;
    };
    // WA Web `e.accountSignatureKey || t`: prefer the in-blob key, else the
    // caller-supplied trusted identity. An empty field counts as absent.
    let account_key: &[u8] = match signed.account_signature_key {
        Some(k) if !k.is_empty() => k,
        _ => match account_identity_fallback {
            Some(f) => f.as_slice(),
            None => return AdvValidation::NoAccountKey,
        },
    };
    let (Ok(account_pub), Ok(device_pub)) = (
        PublicKey::from_djb_public_key_bytes(account_key),
        PublicKey::from_djb_public_key_bytes(fetched_identity_key),
    ) else {
        return AdvValidation::Invalid;
    };

    let account_msg =
        AccountSignatureMessage::new(details, &device_pub, identity_details.device_type);
    if !account_pub.verify_signature(account_msg.as_bytes(), account_sig) {
        return AdvValidation::Invalid;
    }

    let kind = if device_jid.is_hosted() {
        DeviceSignatureKind::Hosted
    } else {
        DeviceSignatureKind::Companion
    };
    let device_msg = account_msg.for_device(&account_pub, kind);
    let verified = device_pub.verify_signature(&device_msg, device_sig);

    if verified {
        AdvValidation::Valid
    } else {
        AdvValidation::Invalid
    }
}

/// Decoded fields from `ADVKeyIndexList` protobuf.
#[derive(Debug, Clone)]
pub struct DecodedKeyIndex {
    pub raw_id: u32,
    pub timestamp: u64,
    pub current_index: u32,
    pub valid_indexes: Vec<u32>,
}

/// Decode signed key index bytes from a `key-index-list` element.
///
/// The bytes are an `ADVSignedKeyIndexList` protobuf whose `details` field
/// contains a serialized `ADVKeyIndexList`. Signature verification is deferred
/// (the notification arrives over a Noise-encrypted connection, so content is
/// already authenticated).
pub fn decode_key_index_list(signed_bytes: &[u8]) -> Option<DecodedKeyIndex> {
    let signed = waproto::codec::adv_signed_key_index_list_decode(signed_bytes).ok()?;
    let details_bytes = signed.details.as_ref()?;
    let key_index = waproto::codec::adv_key_index_list_decode(details_bytes.as_slice()).ok()?;

    let raw_id = key_index.raw_id?;
    let timestamp = key_index.timestamp?;
    let current_index = key_index.current_index.unwrap_or(0);

    Some(DecodedKeyIndex {
        raw_id,
        timestamp,
        current_index,
        valid_indexes: key_index.valid_indexes,
    })
}

/// Filter a device list using `valid_indexes` and `current_index` from an
/// `ADVKeyIndexList`, matching WA Web's filtering algorithm.
///
/// Retention rules (from `AdvDeviceNotificationApi` and `AdvKeyIndexResultApi`):
/// - Primary device (id=0): **always kept**
/// - Device with `key_index ∈ valid_indexes`: kept
/// - Device with `key_index > current_index`: kept (newer than server knows)
/// - Everything else: removed
pub fn filter_devices_by_key_index(
    devices: &[DeviceInfo],
    decoded: &DecodedKeyIndex,
) -> Vec<DeviceInfo> {
    devices
        .iter()
        .filter(|device| should_retain_device(device, decoded))
        .cloned()
        .collect()
}

/// Filter a device list in place using the same ADV key-index rules as
/// [`filter_devices_by_key_index`]. This avoids allocating and copying a second
/// list when the caller already owns its device snapshot.
pub fn retain_devices_by_key_index(devices: &mut Vec<DeviceInfo>, decoded: &DecodedKeyIndex) {
    devices.retain(|device| should_retain_device(device, decoded));
}

fn should_retain_device(device: &DeviceInfo, decoded: &DecodedKeyIndex) -> bool {
    device.device_id() == 0 || is_key_index_valid(device.key_index(), decoded)
}

/// Check if a key_index is accepted by the decoded ADV list.
/// Used to validate a newly-notified device before adding it to the registry.
///
/// WA Web `AdvDeviceNotificationApi`: device added only if
/// `keyIndex != null && (validIndexes.has(keyIndex) || keyIndex > currentIndex)`
pub fn is_key_index_valid(key_index: Option<u32>, decoded: &DecodedKeyIndex) -> bool {
    match key_index {
        Some(ki) => decoded.valid_indexes.contains(&ki) || ki > decoded.current_index,
        None => false,
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::test_util::{ENCRYPTION_TYPES, TEST_PN, device_cases};
    use super::*;
    use buffa::Message;

    fn dev(id: u16, key_index: Option<u32>) -> DeviceInfo {
        DeviceInfo::new(id, key_index)
    }

    #[test]
    fn primary_device_always_kept() {
        let devices = vec![dev(0, None), dev(5, Some(3))];
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 10,
            valid_indexes: vec![], // empty — nothing valid
        };
        let result = filter_devices_by_key_index(&devices, &decoded);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].device_id(), 0);
    }

    #[test]
    fn valid_index_kept_invalid_removed() {
        let devices = vec![dev(0, None), dev(11, Some(5)), dev(12, Some(7))];
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 10,
            valid_indexes: vec![7], // only key_index=7 is valid
        };
        let result = filter_devices_by_key_index(&devices, &decoded);
        assert_eq!(result.len(), 2); // device 0 + device 12
        assert!(result.iter().any(|d| d.device_id() == 0));
        assert!(result.iter().any(|d| d.device_id() == 12));
        assert!(!result.iter().any(|d| d.device_id() == 11));
    }

    #[test]
    fn device_newer_than_current_index_kept() {
        let devices = vec![dev(0, None), dev(15, Some(20))];
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 10,
            valid_indexes: vec![7],
        };
        let result = filter_devices_by_key_index(&devices, &decoded);
        assert_eq!(result.len(), 2); // device 0 + device 15 (key_index 20 > current 10)
    }

    #[test]
    fn device_without_key_index_removed() {
        // WA Web: h.has(null) → false, null > y → false → device removed
        let devices = vec![dev(0, None), dev(5, None)];
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 10,
            valid_indexes: vec![7],
        };
        let result = filter_devices_by_key_index(&devices, &decoded);
        assert_eq!(result.len(), 1); // only primary device kept
        assert_eq!(result[0].device_id(), 0);
    }

    #[test]
    fn is_key_index_valid_in_valid_set() {
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 5,
            valid_indexes: vec![3, 7],
        };
        assert!(is_key_index_valid(Some(3), &decoded));
        assert!(is_key_index_valid(Some(7), &decoded));
    }

    #[test]
    fn is_key_index_valid_not_in_valid_set() {
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 5,
            valid_indexes: vec![3, 7],
        };
        assert!(!is_key_index_valid(Some(4), &decoded));
    }

    #[test]
    fn is_key_index_valid_newer_than_current() {
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 5,
            valid_indexes: vec![3],
        };
        assert!(is_key_index_valid(Some(10), &decoded));
    }

    #[test]
    fn is_key_index_valid_none_rejected() {
        let decoded = DecodedKeyIndex {
            raw_id: 1,
            timestamp: 100,
            current_index: 5,
            valid_indexes: vec![3, 7],
        };
        assert!(!is_key_index_valid(None, &decoded));
    }

    #[test]
    fn decode_roundtrip() {
        use buffa::Message;

        let key_index = waproto::whatsapp::ADVKeyIndexList {
            raw_id: Some(42),
            timestamp: Some(1000),
            current_index: Some(5),
            valid_indexes: vec![3, 5, 7],
            ..Default::default()
        };
        let details = key_index.encode_to_vec();

        let signed = waproto::whatsapp::ADVSignedKeyIndexList {
            details: Some(details),
            ..Default::default()
        };
        let bytes = signed.encode_to_vec();

        let decoded = decode_key_index_list(&bytes).unwrap();
        assert_eq!(decoded.raw_id, 42);
        assert_eq!(decoded.timestamp, 1000);
        assert_eq!(decoded.current_index, 5);
        assert_eq!(decoded.valid_indexes, vec![3, 5, 7]);
    }

    use crate::libsignal::protocol::KeyPair;

    /// Build a signed device-identity. When `include_account_key` is false the
    /// `account_signature_key` field is omitted (the trimmed shape the real
    /// server sends for a contact's companion device); the signatures are still
    /// made with the account key so a fallback can verify them.
    fn signed_identity_opts(
        account: &KeyPair,
        device: &KeyPair,
        details: &[u8],
        hosted: bool,
        include_account_key: bool,
    ) -> Vec<u8> {
        signed_identity_with_prefixes(
            account,
            device,
            details,
            if hosted { &[6, 5] } else { &[6, 0] },
            if hosted { &[6, 6] } else { &[6, 1] },
            include_account_key,
        )
    }

    fn signed_identity_with_prefixes(
        account: &KeyPair,
        device: &KeyPair,
        details: &[u8],
        acct_prefix: &[u8; 2],
        dev_prefix: &[u8; 2],
        include_account_key: bool,
    ) -> Vec<u8> {
        test_util::signed_identity(
            account,
            device,
            details,
            acct_prefix,
            Some(dev_prefix),
            include_account_key,
        )
        .encode_to_vec()
    }

    fn signed_identity(
        account: &KeyPair,
        device: &KeyPair,
        details: &[u8],
        hosted: bool,
    ) -> Vec<u8> {
        signed_identity_opts(account, device, details, hosted, true)
    }

    fn id32(kp: &KeyPair) -> [u8; 32] {
        kp.public_key.public_key_bytes().try_into().unwrap()
    }

    #[test]
    fn adv_chain_hosted_account_signature_with_regular_device_signature() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let details = &[0x28, 0x01];
        let bytes = signed_identity(&account, &device, details, true);
        let mut signed = waproto::codec::adv_signed_device_identity_decode(&bytes).unwrap();
        signed.device_signature = Some(
            device
                .private_key
                .calculate_signature(
                    &[
                        &[6, 1],
                        details.as_slice(),
                        device.public_key.public_key_bytes(),
                        account.public_key.public_key_bytes(),
                    ]
                    .concat(),
                    &mut rng,
                )
                .unwrap()
                .to_vec(),
        );
        assert_eq!(
            validate_adv_with_identity_key(
                &signed.encode_to_vec(),
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Valid
        );
    }

    #[test]
    fn adv_prefixes_follow_signed_device_type_and_address_independently() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let types = ENCRYPTION_TYPES;
        let addresses = device_cases();
        for account_type in types {
            for device_type in types {
                let details = waproto::whatsapp::ADVDeviceIdentity {
                    key_index: Some(0),
                    account_type,
                    device_type,
                    ..Default::default()
                }
                .encode_to_vec();
                let account_prefix = test_util::account_prefix(device_type);
                let wrong_account_prefix = if device_type == Some(ADVEncryptionType::HOSTED) {
                    &[6, 0]
                } else {
                    &[6, 5]
                };
                for (jid, hosted) in &addresses {
                    let device_prefix = if *hosted { &[6, 6] } else { &[6, 1] };
                    let wrong_device_prefix = if *hosted { &[6, 1] } else { &[6, 6] };
                    for include_account_key in [false, true] {
                        let fallback = (!include_account_key).then(|| id32(&account));
                        for (acct, dev, expected) in [
                            (account_prefix, device_prefix, AdvValidation::Valid),
                            (wrong_account_prefix, device_prefix, AdvValidation::Invalid),
                            (account_prefix, wrong_device_prefix, AdvValidation::Invalid),
                        ] {
                            let bytes = signed_identity_with_prefixes(
                                &account,
                                &device,
                                &details,
                                acct,
                                dev,
                                include_account_key,
                            );
                            assert_eq!(
                                validate_adv_with_identity_key(
                                    &bytes,
                                    &id32(&device),
                                    fallback.as_ref(),
                                    jid
                                ),
                                expected,
                                "account={account_type:?} device={device_type:?} jid={jid} in_blob={include_account_key} prefixes={acct:?}/{dev:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn adv_rejects_invalid_signature_lengths_without_account_key() {
        let jid = Jid::pn_device(TEST_PN, 1);
        for account_len in [0, 63, 64, 65] {
            for device_len in [0, 63, 64, 65] {
                let signed = waproto::whatsapp::ADVSignedDeviceIdentity {
                    details: Some(vec![0x18, 0]),
                    account_signature: Some(vec![0; account_len]),
                    device_signature: Some(vec![0; device_len]),
                    ..Default::default()
                };
                let expected = if account_len == 64 && device_len == 64 {
                    AdvValidation::NoAccountKey
                } else {
                    AdvValidation::Invalid
                };
                assert_eq!(
                    validate_adv_with_identity_key(&signed.encode_to_vec(), &[0; 32], None, &jid),
                    expected,
                    "account_len={account_len} device_len={device_len}"
                );
            }
        }
    }

    #[test]
    fn adv_rejects_signed_malformed_details_even_without_account_key() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        for include_key in [false, true] {
            let bytes = signed_identity_with_prefixes(
                &account,
                &device,
                &[0xff],
                &[6, 0],
                &[6, 1],
                include_key,
            );
            assert_eq!(
                validate_adv_with_identity_key(
                    &bytes,
                    &id32(&device),
                    None,
                    &Jid::pn_device(TEST_PN, 1)
                ),
                AdvValidation::Invalid
            );
        }
    }

    #[test]
    fn adv_empty_account_key_uses_fallback_and_rejects_tampering() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        for hosted in [false, true] {
            let jid = Jid::pn_device(TEST_PN, if hosted { 99 } else { 1 });
            let device_prefix = if hosted { &[6, 6] } else { &[6, 1] };
            let bytes = signed_identity_with_prefixes(
                &account,
                &device,
                &[0x28, 1],
                &[6, 5],
                device_prefix,
                false,
            );
            let mut signed = waproto::codec::adv_signed_device_identity_decode(&bytes).unwrap();
            signed.account_signature_key = Some(vec![]);
            assert_eq!(
                validate_adv_with_identity_key(
                    &signed.encode_to_vec(),
                    &id32(&device),
                    Some(&id32(&account)),
                    &jid
                ),
                AdvValidation::Valid
            );
            assert_eq!(
                validate_adv_with_identity_key(&signed.encode_to_vec(), &id32(&device), None, &jid),
                AdvValidation::NoAccountKey
            );
            for field in 0..3 {
                let mut corrupt = signed.clone();
                match field {
                    0 => corrupt.account_signature.as_mut().unwrap()[0] ^= 1,
                    1 => corrupt.device_signature.as_mut().unwrap()[0] ^= 1,
                    2 => corrupt.details = Some(vec![0x28, 0]),
                    _ => unreachable!(),
                }
                assert_eq!(
                    validate_adv_with_identity_key(
                        &corrupt.encode_to_vec(),
                        &id32(&device),
                        Some(&id32(&account)),
                        &jid
                    ),
                    AdvValidation::Invalid
                );
            }
        }
    }

    #[test]
    fn adv_chain_valid_accepted() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"\x18\x01", false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Valid
        );
    }

    #[test]
    fn adv_chain_hosted_prefix_accepted() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"\x28\x01", true);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 99)
            ),
            AdvValidation::Valid
        );
    }

    #[test]
    fn adv_chain_rejects_substituted_identity() {
        // A relay swaps the bundle's <identity> to its own key, but the signed
        // device-identity still binds the real one: validation must fail.
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let attacker = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"\x18\x01", false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&attacker),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Invalid
        );
    }

    #[test]
    fn adv_chain_rejects_missing_device_signature() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let no_dev_sig = waproto::whatsapp::ADVSignedDeviceIdentity {
            details: Some(b"\x18\x01".to_vec()),
            account_signature_key: Some(account.public_key.public_key_bytes().to_vec()),
            account_signature: Some(vec![0u8; 64]),
            device_signature: None,
        }
        .encode_to_vec();
        assert_eq!(
            validate_adv_with_identity_key(
                &no_dev_sig,
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Invalid
        );
    }

    #[test]
    fn adv_chain_rejects_garbage() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let device = KeyPair::generate(&mut rng);
        assert_eq!(
            validate_adv_with_identity_key(
                &[1, 2, 3, 4],
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Invalid
        );
    }

    // The real server omits account_signature_key for a contact's companion
    // device. With the contact's primary identity supplied as the fallback, the
    // chain verifies — this is the regression #772 broke (it rejected outright).
    #[test]
    fn adv_chain_missing_account_key_verifies_with_fallback() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity_opts(&account, &device, b"\x18\x01", false, false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                Some(&id32(&account)),
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Valid
        );
    }

    // No in-blob key and no fallback: the chain is unverifiable, so we must NOT
    // reject (that would drop a legitimate device); the caller proceeds.
    #[test]
    fn adv_chain_missing_account_key_no_fallback_is_no_account_key() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity_opts(&account, &device, b"\x18\x01", false, false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                None,
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::NoAccountKey
        );
    }

    // A wrong fallback (relay-supplied account identity) must NOT verify: the
    // account signature was made by the real account key, so a mismatched
    // fallback yields Invalid, preserving the forgery protection.
    #[test]
    fn adv_chain_missing_account_key_wrong_fallback_is_invalid() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let attacker = KeyPair::generate(&mut rng);
        let bytes = signed_identity_opts(&account, &device, b"\x18\x01", false, false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                Some(&id32(&attacker)),
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Invalid
        );
    }

    // The in-blob key takes precedence over the fallback (WA Web `|| t`): a valid
    // full blob still verifies even if an unrelated fallback is passed.
    #[test]
    fn adv_chain_in_blob_key_takes_precedence_over_fallback() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let unrelated = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"\x18\x01", false);
        assert_eq!(
            validate_adv_with_identity_key(
                &bytes,
                &id32(&device),
                Some(&id32(&unrelated)),
                &Jid::pn_device(TEST_PN, 1)
            ),
            AdvValidation::Valid
        );
    }
}
