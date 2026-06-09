//! ADV (Advanced Device Verification) key index utilities.
//!
//! Decodes `ADVSignedKeyIndexList` protobuf from `key-index-list` elements
//! and filters device lists by `valid_indexes`.
//!
//! Reference: WAWebHandleAdvDeviceNotificationUtils.decodeSignedKeyIndexBytes()

use crate::libsignal::protocol::PublicKey;
use crate::store::traits::DeviceInfo;
use prost::Message;

// ADV signature prefixes (WAWebAdvSignatureConstants). The hosted ([6,5]/[6,6])
// variants apply to business-hosted companion devices.
const ADV_PREFIX_ACCOUNT_SIGNATURE: &[u8] = &[6, 0];
const ADV_PREFIX_DEVICE_SIGNATURE: &[u8] = &[6, 1];
const ADV_HOSTED_PREFIX_ACCOUNT_SIGNATURE: &[u8] = &[6, 5];
const ADV_HOSTED_PREFIX_DEVICE_SIGNATURE: &[u8] = &[6, 6];

/// Outcome of validating a fetched companion device's `ADVSignedDeviceIdentity`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvValidation {
    /// Both signatures verified against an available account key — trusted.
    Valid,
    /// The blob is malformed, or the signatures failed to verify against an
    /// available account key. A relay swapping in a forged identity lands here;
    /// the caller must reject the bundle.
    Invalid,
    /// No account key was available: the blob omits `account_signature_key` and
    /// no trusted account identity was passed as a fallback, so the chain can't
    /// be checked at all. The caller decides what to do (we log and proceed,
    /// matching the existing "device-identity absent" handling).
    NoAccountKey,
}

/// Verify a fetched companion device's `ADVSignedDeviceIdentity` binds the
/// fetched identity key to the account's ADV chain, mirroring WA Web
/// `WAWebAdvSignatureApi.validateADVwithIdentityKey`.
///
/// Two checks must both hold under one prefix family: the account signature over
/// `prefix || details || identity`, and the device signature (made with the
/// fetched identity key) over `prefix || details || identity || accountKey`. The
/// device signature is what binds the fetched identity to the account, so a relay
/// that substitutes a fabricated identity key is rejected. We try the E2EE then
/// the hosted prefix set rather than replicating WA Web's `bizHostedDevicesEnabled`
/// gating: the prefix is only a domain separator, so accepting whichever the
/// signer used stays sound (an attacker still can't forge the account signature).
///
/// The account key is `blob.account_signature_key || account_identity_fallback`,
/// exactly as WA Web's `P`/`w` helpers (`e.accountSignatureKey || t`): the server
/// legitimately omits `account_signature_key` for a contact's companion device
/// because it's the contact's primary (device 0) identity the client already
/// holds, so the caller passes that stored identity as the fallback. When neither
/// source has a key the chain is unverifiable and we return `NoAccountKey` rather
/// than rejecting a bundle we simply lack the material to check.
pub fn validate_adv_with_identity_key(
    device_identity_bytes: &[u8],
    fetched_identity_key: &[u8; 32],
    account_identity_fallback: Option<&[u8; 32]>,
) -> AdvValidation {
    let Ok(signed) = waproto::whatsapp::AdvSignedDeviceIdentity::decode(device_identity_bytes)
    else {
        return AdvValidation::Invalid;
    };
    let (Some(details), Some(account_sig), Some(device_sig)) = (
        signed.details.as_deref(),
        signed.account_signature.as_deref(),
        signed.device_signature.as_deref(),
    ) else {
        return AdvValidation::Invalid;
    };
    // WA Web `e.accountSignatureKey || t`: prefer the in-blob key, else the
    // caller-supplied trusted identity. An empty field counts as absent.
    let account_key: &[u8] = match signed.account_signature_key.as_deref() {
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

    let verified = [
        (ADV_PREFIX_ACCOUNT_SIGNATURE, ADV_PREFIX_DEVICE_SIGNATURE),
        (
            ADV_HOSTED_PREFIX_ACCOUNT_SIGNATURE,
            ADV_HOSTED_PREFIX_DEVICE_SIGNATURE,
        ),
    ]
    .into_iter()
    .any(|(account_prefix, device_prefix)| {
        let account_msg = [account_prefix, details, fetched_identity_key].concat();
        let device_msg = [device_prefix, details, fetched_identity_key, account_key].concat();
        account_pub.verify_signature(&account_msg, account_sig)
            && device_pub.verify_signature(&device_msg, device_sig)
    });

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
    let signed = waproto::whatsapp::AdvSignedKeyIndexList::decode(signed_bytes).ok()?;
    let details_bytes = signed.details.as_ref()?;
    let key_index = waproto::whatsapp::AdvKeyIndexList::decode(details_bytes.as_slice()).ok()?;

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
    let valid_set: std::collections::HashSet<u32> = decoded.valid_indexes.iter().copied().collect();

    devices
        .iter()
        .filter(|d| {
            // Primary device always kept
            if d.device_id == 0 {
                return true;
            }
            match d.key_index {
                Some(ki) => valid_set.contains(&ki) || ki > decoded.current_index,
                // WA Web: h.has(null) → false, null > y → false → device removed
                None => false,
            }
        })
        .cloned()
        .collect()
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
mod tests {
    use super::*;

    fn dev(id: u32, key_index: Option<u32>) -> DeviceInfo {
        DeviceInfo {
            device_id: id,
            key_index,
        }
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
        assert_eq!(result[0].device_id, 0);
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
        assert!(result.iter().any(|d| d.device_id == 0));
        assert!(result.iter().any(|d| d.device_id == 12));
        assert!(!result.iter().any(|d| d.device_id == 11));
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
        assert_eq!(result[0].device_id, 0);
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
        use prost::Message;

        let key_index = waproto::whatsapp::AdvKeyIndexList {
            raw_id: Some(42),
            timestamp: Some(1000),
            current_index: Some(5),
            valid_indexes: vec![3, 5, 7],
            account_type: None,
        };
        let details = key_index.encode_to_vec();

        let signed = waproto::whatsapp::AdvSignedKeyIndexList {
            details: Some(details),
            account_signature: None,
            account_signature_key: None,
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
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity = device.public_key.public_key_bytes();
        let account_key = account.public_key.public_key_bytes();
        let (acct_prefix, dev_prefix): (&[u8], &[u8]) = if hosted {
            (
                ADV_HOSTED_PREFIX_ACCOUNT_SIGNATURE,
                ADV_HOSTED_PREFIX_DEVICE_SIGNATURE,
            )
        } else {
            (ADV_PREFIX_ACCOUNT_SIGNATURE, ADV_PREFIX_DEVICE_SIGNATURE)
        };
        let account_sig = account
            .private_key
            .calculate_signature(&[acct_prefix, details, identity].concat(), &mut rng)
            .unwrap()
            .to_vec();
        let device_sig = device
            .private_key
            .calculate_signature(
                &[dev_prefix, details, identity, account_key].concat(),
                &mut rng,
            )
            .unwrap()
            .to_vec();
        waproto::whatsapp::AdvSignedDeviceIdentity {
            details: Some(details.to_vec()),
            account_signature_key: include_account_key.then(|| account_key.to_vec()),
            account_signature: Some(account_sig),
            device_signature: Some(device_sig),
        }
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
    fn adv_chain_valid_accepted() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"details", false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), None),
            AdvValidation::Valid
        );
    }

    #[test]
    fn adv_chain_hosted_prefix_accepted() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let bytes = signed_identity(&account, &device, b"hosted-details", true);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), None),
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
        let bytes = signed_identity(&account, &device, b"details", false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&attacker), None),
            AdvValidation::Invalid
        );
    }

    #[test]
    fn adv_chain_rejects_missing_device_signature() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let no_dev_sig = waproto::whatsapp::AdvSignedDeviceIdentity {
            details: Some(b"details".to_vec()),
            account_signature_key: Some(account.public_key.public_key_bytes().to_vec()),
            account_signature: Some(vec![0u8; 64]),
            device_signature: None,
        }
        .encode_to_vec();
        assert_eq!(
            validate_adv_with_identity_key(&no_dev_sig, &id32(&device), None),
            AdvValidation::Invalid
        );
    }

    #[test]
    fn adv_chain_rejects_garbage() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let device = KeyPair::generate(&mut rng);
        assert_eq!(
            validate_adv_with_identity_key(&[1, 2, 3, 4], &id32(&device), None),
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
        let bytes = signed_identity_opts(&account, &device, b"details", false, false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), Some(&id32(&account))),
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
        let bytes = signed_identity_opts(&account, &device, b"details", false, false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), None),
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
        let bytes = signed_identity_opts(&account, &device, b"details", false, false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), Some(&id32(&attacker))),
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
        let bytes = signed_identity(&account, &device, b"details", false);
        assert_eq!(
            validate_adv_with_identity_key(&bytes, &id32(&device), Some(&id32(&unrelated))),
            AdvValidation::Valid
        );
    }
}
