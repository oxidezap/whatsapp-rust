//! Pure helpers for retry receipt protocol logic.
//!
//! Constants and utility functions for parsing retry receipts. These have no
//! runtime dependencies (`self`, `Client`, spawn, sleep). All orchestration
//! (session management, message resend, cache interaction) remains in
//! `whatsapp-rust/src/retry.rs`.

use crate::iq::prekeys::{OneTimePreKeyNode, SignedPreKeyNode};
use crate::libsignal::protocol::PublicKey;
use crate::protocol::ProtocolNode;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Node, NodeContent, NodeRef};

/// Maximum retry attempts we'll honor (matches WhatsApp Web's MAX_RETRY = 5).
/// We refuse to resend if the requester has already retried this many times.
pub const MAX_RETRY_COUNT: u8 = 5;

/// Minimum retry count before we include keys in retry receipts.
/// WhatsApp Web only includes keys when retryCount >= 2, giving the first
/// retry a chance to succeed without key exchange overhead.
pub const MIN_RETRY_COUNT_FOR_KEYS: u8 = 2;

/// Minimum retry count before we start tracking base keys.
/// WhatsApp Web saves base key on retry 2, checks on retry > 2.
pub const MIN_RETRY_FOR_BASE_KEY_CHECK: u8 = 2;

/// Retry reason codes matching WhatsApp Web's RetryReason enum.
/// These are included in the retry receipt to help the sender understand
/// why the message couldn't be decrypted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[allow(dead_code)] // All variants defined for WhatsApp Web compatibility
pub enum RetryReason {
    /// Unknown or unspecified error
    UnknownError = 0,
    /// No session exists with the sender (SessionNotFound)
    NoSession = 1,
    /// Invalid key in the message
    InvalidKey = 2,
    /// PreKey ID not found (InvalidPreKeyId)
    InvalidKeyId = 3,
    /// Invalid message format or content (InvalidMessage)
    InvalidMessage = 4,
    /// Invalid signature
    InvalidSignature = 5,
    /// Message from the future (timestamp issue)
    FutureMessage = 6,
    /// MAC verification failed (bad MAC)
    BadMac = 7,
    /// Invalid session state
    InvalidSession = 8,
    /// Invalid message key
    InvalidMsgKey = 9,
    /// Bad broadcast ephemeral setting
    BadBroadcastEphemeralSetting = 10,
    /// Unknown companion device, not in our device list
    UnknownCompanionNoPrekey = 11,
    /// ADV signature or device identity failure
    AdvFailure = 12,
    /// Status revoke delay exceeded
    StatusRevokeDelay = 13,
}

impl RetryReason {
    /// Stable, low-cardinality label for metrics and logs.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UnknownError => "unknown",
            Self::NoSession => "no_session",
            Self::InvalidKey => "invalid_key",
            Self::InvalidKeyId => "invalid_key_id",
            Self::InvalidMessage => "invalid_message",
            Self::InvalidSignature => "invalid_signature",
            Self::FutureMessage => "future_message",
            Self::BadMac => "bad_mac",
            Self::InvalidSession => "invalid_session",
            Self::InvalidMsgKey => "invalid_msg_key",
            Self::BadBroadcastEphemeralSetting => "bad_broadcast_ephemeral",
            Self::UnknownCompanionNoPrekey => "unknown_companion",
            Self::AdvFailure => "adv_failure",
            Self::StatusRevokeDelay => "status_revoke_delay",
        }
    }
}

/// Helper to extract bytes content from a Node.
pub fn get_bytes_content(node: &Node) -> Option<&[u8]> {
    match &node.content {
        Some(NodeContent::Bytes(b)) => Some(b.as_slice()),
        _ => None,
    }
}

/// Parses a `<registration>` payload as a big-endian `u32`. Registration IDs are
/// u32, so payloads longer than 4 bytes are rejected rather than silently
/// truncated; shorter payloads (1-3 bytes) are left-zero-padded.
fn parse_registration_id(bytes: &[u8]) -> Option<u32> {
    match bytes.len() {
        4 => Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])),
        // Reject oversized payloads rather than silently truncating.
        n if n > 4 => None,
        0 => None,
        _ => {
            let mut arr = [0u8; 4];
            let start = 4 - bytes.len();
            arr[start..].copy_from_slice(bytes);
            Some(u32::from_be_bytes(arr))
        }
    }
}

/// Helper to extract registration ID from a node (4 bytes big-endian).
///
/// Looks for a `<registration>` child node and parses its bytes content
/// as a big-endian `u32`. See [`parse_registration_id`] for the parse rules.
pub fn extract_registration_id_from_node(node: &Node) -> Option<u32> {
    let registration_node = node.get_optional_child("registration")?;
    parse_registration_id(get_bytes_content(registration_node)?)
}

/// `NodeRef` counterpart of [`extract_registration_id_from_node`]; shares the
/// same parse rules via [`parse_registration_id`].
pub fn extract_registration_id_from_node_ref(node: &NodeRef<'_>) -> Option<u32> {
    let registration_node = node.get_optional_child("registration")?;
    parse_registration_id(registration_node.content_bytes()?)
}

/// Returns whether keys should be included in a retry receipt for the given
/// retry count and reason.
///
/// WhatsApp Web only includes keys when `retryCount >= 2`. As an optimization,
/// keys are included on retry #1 for `NoSession` errors to reduce round-trips
/// for skmsg-only message failures.
pub fn should_include_keys(retry_count: u8, reason: RetryReason) -> bool {
    let include_keys_early =
        reason == RetryReason::NoSession || reason == RetryReason::UnknownCompanionNoPrekey;
    retry_count >= MIN_RETRY_COUNT_FOR_KEYS || include_keys_early
}

/// Whether a retry from a device that is not in our device registry must be
/// dropped instead of resent.
///
/// WA Web (`WAWebHandleRetryRequest`) bails on an unknown device, but that is
/// safe only because it keeps participant device lists fresh via a pre-send
/// sync, so a legitimate requester is already known. When the retry carries a
/// `<keys>` bundle the requester's ADV-signed identity and prekeys are in hand,
/// so the session can be re-established and the message resent without a registry
/// entry. whatsmeow builds the session straight from the receipt bundle the same
/// way. Only drop when there is no bundle to recover from.
pub fn should_drop_unknown_device_retry(keys_present: bool, device_known: bool) -> bool {
    !keys_present && !device_known
}

/// Builds the `<keys>` bundle embedded in a retry receipt (type, identity, one-time prekey,
/// signed prekey, device identity) so a peer can re-establish the Signal session.
///
/// Every public key `<value>` is the raw 32-byte curve key (`public_key_bytes`), not the
/// 33-byte Signal-serialized form: this matches WA Web's xmppPreKey/xmppSignedPreKey and the
/// prekey upload path, and is what a peer's prekey parser expects.
pub fn build_retry_keys_node(
    identity_pub: &PublicKey,
    prekey_id: u32,
    prekey_pub: &PublicKey,
    signed_prekey_id: u32,
    signed_prekey_pub: &PublicKey,
    signed_prekey_signature: Vec<u8>,
    device_identity: Vec<u8>,
) -> Node {
    NodeBuilder::new("keys")
        .children([
            NodeBuilder::new("type").bytes(vec![5u8]).build(),
            NodeBuilder::new("identity")
                .bytes(identity_pub.public_key_bytes().to_vec())
                .build(),
            OneTimePreKeyNode::new(prekey_id, prekey_pub.public_key_bytes().to_vec()).into_node(),
            SignedPreKeyNode::new(
                signed_prekey_id,
                signed_prekey_pub.public_key_bytes().to_vec(),
                signed_prekey_signature,
            )
            .into_node(),
            NodeBuilder::new("device-identity")
                .bytes(device_identity)
                .build(),
        ])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use wacore_binary::Attrs;

    #[test]
    fn get_bytes_content_extracts_bytes() {
        let node = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![1, 2, 3, 4])),
        };
        assert_eq!(get_bytes_content(&node), Some(&[1, 2, 3, 4][..]));
    }

    #[test]
    fn get_bytes_content_returns_none_for_string() {
        let node = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: Some(NodeContent::String("hello".into())),
        };
        assert_eq!(get_bytes_content(&node), None);
    }

    #[test]
    fn get_bytes_content_returns_none_for_empty() {
        let node = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: None,
        };
        assert_eq!(get_bytes_content(&node), None);
    }

    #[test]
    fn extract_registration_id_4_bytes() {
        let reg_node = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![0x00, 0x01, 0x02, 0x03])),
        };
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node])),
        };
        assert_eq!(extract_registration_id_from_node(&parent), Some(0x00010203));
    }

    #[test]
    fn extract_registration_id_3_bytes() {
        let reg_node = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![0x01, 0x02, 0x03])),
        };
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node])),
        };
        assert_eq!(extract_registration_id_from_node(&parent), Some(0x00010203));
    }

    #[test]
    fn extract_registration_id_missing() {
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![])),
        };
        assert_eq!(extract_registration_id_from_node(&parent), None);
    }

    #[test]
    fn extract_registration_id_empty_bytes() {
        let reg_node = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![])),
        };
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node])),
        };
        assert_eq!(extract_registration_id_from_node(&parent), None);
    }

    // Registration IDs are u32; an oversized payload must be rejected, not truncated
    // to its first 4 bytes.
    #[test]
    fn extract_registration_id_rejects_oversized() {
        let reg_node = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![0x01, 0x02, 0x03, 0x04, 0x05])),
        };
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node])),
        };
        assert_eq!(extract_registration_id_from_node(&parent), None);
        assert_eq!(
            extract_registration_id_from_node_ref(&parent.as_node_ref()),
            None
        );
    }

    #[test]
    fn extract_registration_id_from_node_ref_matches_owned() {
        let reg_node = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![0x00, 0x01, 0x02, 0x03])),
        };
        let parent = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node])),
        };
        assert_eq!(
            extract_registration_id_from_node_ref(&parent.as_node_ref()),
            Some(0x00010203)
        );

        // Variable-length (3-byte) payloads zero-pad on the left, same as the owned path.
        let reg_node_short = Node {
            tag: Cow::Borrowed("registration"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![0x01, 0x02, 0x03])),
        };
        let parent_short = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![reg_node_short])),
        };
        assert_eq!(
            extract_registration_id_from_node_ref(&parent_short.as_node_ref()),
            Some(0x00010203)
        );
    }

    #[test]
    fn should_include_keys_no_session_retry_1() {
        assert!(
            should_include_keys(1, RetryReason::NoSession),
            "NoSession at retry#1 should include keys (optimization)"
        );
    }

    #[test]
    fn should_include_keys_unknown_companion_retry_1() {
        assert!(
            should_include_keys(1, RetryReason::UnknownCompanionNoPrekey),
            "UnknownCompanionNoPrekey at retry#1 should include keys"
        );
    }

    #[test]
    fn should_include_keys_invalid_message_retry_1() {
        assert!(
            !should_include_keys(1, RetryReason::InvalidMessage),
            "InvalidMessage at retry#1 should NOT include keys"
        );
    }

    #[test]
    fn should_include_keys_retry_2_any_reason() {
        assert!(should_include_keys(2, RetryReason::InvalidMessage));
        assert!(should_include_keys(2, RetryReason::UnknownError));
        assert!(should_include_keys(2, RetryReason::BadMac));
        assert!(should_include_keys(2, RetryReason::NoSession));
    }

    #[test]
    fn should_include_keys_retry_3_any_reason() {
        assert!(should_include_keys(3, RetryReason::InvalidMessage));
        assert!(should_include_keys(3, RetryReason::UnknownError));
    }

    #[test]
    fn drop_unknown_device_retry_only_without_bundle() {
        // The regression this guards: a retry that carries a <keys> bundle from a
        // device missing from our registry must be recovered, not dropped.
        assert!(!should_drop_unknown_device_retry(true, false));
        // No bundle and unknown device: nothing to recover from, so drop.
        assert!(should_drop_unknown_device_retry(false, false));
        // A known device is always handled, bundle or not.
        assert!(!should_drop_unknown_device_retry(true, true));
        assert!(!should_drop_unknown_device_retry(false, true));
    }

    #[test]
    fn constants_match_wa_web() {
        assert_eq!(MAX_RETRY_COUNT, 5);
        assert_eq!(MIN_RETRY_COUNT_FOR_KEYS, 2);
        assert_eq!(MIN_RETRY_FOR_BASE_KEY_CHECK, 2);
    }

    #[test]
    fn retry_keys_bundle_emits_raw_32_byte_curve_values() {
        // WhatsApp Web's xmppPreKey / xmppSignedPreKey carry the raw 32-byte curve public key in
        // <value>; a peer parses the retry <keys> bundle expecting exactly that, so the prekey and
        // signed prekey values must parse with the prekey parsers (which require 32 bytes).
        use crate::libsignal::protocol::KeyPair;

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity = KeyPair::generate(&mut rng);
        let prekey = KeyPair::generate(&mut rng);
        let signed_prekey = KeyPair::generate(&mut rng);

        let keys = build_retry_keys_node(
            &identity.public_key,
            201,
            &prekey.public_key,
            100,
            &signed_prekey.public_key,
            vec![7u8; 64],
            vec![9u8; 16],
        );

        let key_node = keys.get_optional_child("key").expect("<key> child present");
        let parsed_prekey =
            OneTimePreKeyNode::try_from_node(key_node).expect("one-time prekey should parse");
        assert_eq!(
            parsed_prekey.public_bytes.as_slice(),
            prekey.public_key.public_key_bytes()
        );

        let skey_node = keys
            .get_optional_child("skey")
            .expect("<skey> child present");
        let parsed_skey =
            SignedPreKeyNode::try_from_node(skey_node).expect("signed prekey should parse");
        assert_eq!(
            parsed_skey.public_bytes.as_slice(),
            signed_prekey.public_key.public_key_bytes()
        );
    }

    // These strings are metric label values; drift silently breaks dashboards.
    #[test]
    fn retry_reason_as_str_is_stable() {
        let cases = [
            (RetryReason::UnknownError, "unknown"),
            (RetryReason::NoSession, "no_session"),
            (RetryReason::InvalidKey, "invalid_key"),
            (RetryReason::InvalidKeyId, "invalid_key_id"),
            (RetryReason::InvalidMessage, "invalid_message"),
            (RetryReason::InvalidSignature, "invalid_signature"),
            (RetryReason::FutureMessage, "future_message"),
            (RetryReason::BadMac, "bad_mac"),
            (RetryReason::InvalidSession, "invalid_session"),
            (RetryReason::InvalidMsgKey, "invalid_msg_key"),
            (
                RetryReason::BadBroadcastEphemeralSetting,
                "bad_broadcast_ephemeral",
            ),
            (RetryReason::UnknownCompanionNoPrekey, "unknown_companion"),
            (RetryReason::AdvFailure, "adv_failure"),
            (RetryReason::StatusRevokeDelay, "status_revoke_delay"),
        ];
        for (reason, expected) in cases {
            assert_eq!(reason.as_str(), expected);
        }
    }
}
