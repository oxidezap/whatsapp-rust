//! Pure helpers for receipt stanza logic.
//!
//! These functions contain no runtime dependencies (`self`, `Client`, spawn, sleep).
//! Orchestration and dispatch remain in `whatsapp-rust/src/receipt.rs`.

use crate::types::message::{MessageCategory, MessageInfo};
use crate::types::presence::ReceiptType;
use wacore_binary::NodeRef;
use wacore_binary::{Jid, JidExt as _, STATUS_BROADCAST_USER};

/// Parsed `<user>` entry inside `<receipt><participants>`.
///
/// Mirrors `WAWebHandleMsgReceiptParser` (`d`/`m` branches): each user has a
/// device JID and per-user timestamp; for `aggregated_by_message` shapes the
/// user also carries its own type, otherwise the receipt-level type applies.
///
/// `timestamp` is `None` when the `<user t>` attr is missing (malformed
/// stanza). The handler falls back to the stanza-level `t` in that case.
#[derive(Debug, Clone)]
pub struct ReceiptUser {
    pub jid: Jid,
    pub timestamp: Option<u64>,
    /// Per-user `type` attr (only on aggregated_by_message shape).
    pub r#type: Option<String>,
    pub participant_pn: Option<Jid>,
    pub participant_username: Option<String>,
}

/// Parses `<participants>` child of `<receipt>`. Returns `(message_id, key, users)`.
/// `message_id` is set when `<participants message_id="...">` is present
/// (aggregated_by_message); `key` is the legacy aggregated_by_type identifier.
pub fn parse_participants(
    node: &NodeRef<'_>,
) -> (Option<String>, Option<String>, Vec<ReceiptUser>) {
    let mut attrs = node.attrs();
    let message_id = attrs.optional_string("message_id").map(|s| s.into_owned());
    let key = attrs.optional_string("key").map(|s| s.into_owned());

    let users = node
        .children()
        .map(|children| {
            children
                .iter()
                .filter(|c| c.tag == "user")
                .filter_map(|c| {
                    let mut a = c.attrs();
                    let jid = a.optional_jid("jid")?;
                    let timestamp = a.optional_u64("t");
                    let r#type = a.optional_string("type").map(|s| s.into_owned());
                    let participant_pn = a.optional_jid("participant_pn");
                    let participant_username = a
                        .optional_string("participant_username")
                        .map(|s| s.into_owned());
                    Some(ReceiptUser {
                        jid,
                        timestamp,
                        r#type,
                        participant_pn,
                        participant_username,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    (message_id, key, users)
}

/// Collects message IDs from a simple `<receipt>` (no `<participants>` child).
///
/// Mirrors `WAWebHandleMsgReceiptParser` p() branch: read `<list><item id=.../>`
/// when present, then append the stanza id for non-view receipts (for view
/// receipts the items use `server_id` and the stanza id is NOT appended).
pub fn collect_simple_message_ids(
    node: &NodeRef<'_>,
    stanza_id: &str,
    is_view: bool,
) -> Vec<String> {
    let id_attr = if is_view { "server_id" } else { "id" };
    let mut ids: Vec<String> = node
        .get_optional_child("list")
        .and_then(|list| {
            list.children().map(|items| {
                items
                    .iter()
                    .filter(|c| c.tag == "item")
                    .filter_map(|c| c.attrs().optional_string(id_attr).map(|s| s.into_owned()))
                    .collect()
            })
        })
        .unwrap_or_default();

    if !is_view {
        ids.push(stanza_id.to_string());
    }
    ids
}

/// Determines whether a delivery receipt should be sent for this message.
///
/// Returns `false` for:
/// - Messages with an empty ID
/// - Status broadcasts (`status@broadcast`)
/// - Newsletter messages
/// - Own outgoing messages, EXCEPT category `"peer"` (self-synced) and
///   self-fanouts (`is_from_me` with a `recipient`), which need a
///   `<receipt type="sender">`.
///
/// WA Web sends `type="peer_msg"` for self-synced and `type="sender"` for
/// own-account fanouts (`isMeAccount(author)`). For all other own messages,
/// receipts are skipped.
///
/// NOTE: the message-dispatch hot path uses
/// `crate::client::Client::should_send_delivery_receipt` (in the
/// `whatsapp-rust` crate), which is authoritative and intentionally diverges
/// here (it also sends `<receipt context="status">` for status broadcasts,
/// which this copy still skips). The self-fanout rule is shared via
/// [`MessageSource::is_self_fanout`].
pub fn should_send_delivery_receipt(info: &MessageInfo) -> bool {
    if info.id.is_empty()
        || info.source.chat.user == STATUS_BROADCAST_USER
        || info.source.chat.is_newsletter()
    {
        return false;
    }

    // WA Web sends type="peer_msg" for self-synced (category="peer") and
    // type="sender" for own-account fanouts. Other own messages are skipped.
    info.category == MessageCategory::Peer
        || !info.source.is_from_me
        || info.source.is_self_fanout()
}

/// WA Web's receipt parser downgrades a delivery ack to "sent" (not delivered) when the
/// `<receipt>` carries `<error reason="lid" type="feature-incapable">`: the LID peer is
/// feature-incapable and never received the message. Returns the effective receipt type.
///
/// Scoped to `Delivered` (the only type that carries this error in practice), which also
/// keeps the downgrade from rerouting retry / enc-rekey receipts.
pub fn downgrade_for_feature_incapable(
    node: &NodeRef<'_>,
    parsed_type: ReceiptType,
) -> ReceiptType {
    if parsed_type != ReceiptType::Delivered {
        return parsed_type;
    }
    let Some(err) = node.get_optional_child("error") else {
        return parsed_type;
    };
    let mut a = err.attrs();
    let reason = a.optional_string("reason");
    let err_type = a.optional_string("type");
    if reason.as_deref() == Some("lid") && err_type.as_deref() == Some("feature-incapable") {
        ReceiptType::Sent
    } else {
        parsed_type
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::message::{MessageCategory, MessageInfo, MessageSource};

    #[test]
    fn feature_incapable_error_downgrades_delivery_to_sent() {
        use wacore_binary::builder::NodeBuilder;

        let with_error = NodeBuilder::new("receipt")
            .children([NodeBuilder::new("error")
                .attr("reason", "lid")
                .attr("type", "feature-incapable")
                .build()])
            .build();
        assert_eq!(
            downgrade_for_feature_incapable(&with_error.as_node_ref(), ReceiptType::Delivered),
            ReceiptType::Sent,
            "lid/feature-incapable error downgrades delivery to sent"
        );

        // No <error> child: unchanged.
        let plain = NodeBuilder::new("receipt").build();
        assert_eq!(
            downgrade_for_feature_incapable(&plain.as_node_ref(), ReceiptType::Delivered),
            ReceiptType::Delivered
        );

        // Different error type: unchanged.
        let other = NodeBuilder::new("receipt")
            .children([NodeBuilder::new("error")
                .attr("reason", "lid")
                .attr("type", "other")
                .build()])
            .build();
        assert_eq!(
            downgrade_for_feature_incapable(&other.as_node_ref(), ReceiptType::Delivered),
            ReceiptType::Delivered
        );

        // Non-delivery type with the same error: not downgraded (scoped to Delivered).
        assert_eq!(
            downgrade_for_feature_incapable(&with_error.as_node_ref(), ReceiptType::Read),
            ReceiptType::Read
        );
    }

    #[test]
    fn skip_empty_id() {
        let info = MessageInfo {
            id: "".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net".parse().unwrap(),
                sender: "12345@s.whatsapp.net".parse().unwrap(),
                is_from_me: false,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&info));
    }

    #[test]
    fn skip_status_broadcast() {
        let info = MessageInfo {
            id: "MSG1".to_string(),
            source: MessageSource {
                chat: "status@broadcast".parse().unwrap(),
                sender: "12345@s.whatsapp.net".parse().unwrap(),
                is_from_me: false,
                is_group: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&info));
    }

    #[test]
    fn skip_newsletter() {
        let info = MessageInfo {
            id: "NL1".to_string(),
            source: MessageSource {
                chat: "120363173003902460@newsletter".parse().unwrap(),
                sender: "120363173003902460@newsletter".parse().unwrap(),
                is_from_me: false,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&info));
    }

    #[test]
    fn skip_own_non_peer_messages() {
        let info = MessageInfo {
            id: "OWN1".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net".parse().unwrap(),
                sender: "12345@s.whatsapp.net".parse().unwrap(),
                is_from_me: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&info));
    }

    #[test]
    fn allow_peer_self_synced_messages() {
        let info = MessageInfo {
            id: "PEER1".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net".parse().unwrap(),
                sender: "12345@s.whatsapp.net".parse().unwrap(),
                is_from_me: true,
                ..Default::default()
            },
            category: MessageCategory::Peer,
            ..Default::default()
        };
        assert!(should_send_delivery_receipt(&info));
    }

    #[test]
    fn allow_self_fanout_with_recipient() {
        // Own outgoing message echoed back (is_from_me + recipient): needs a
        // sender receipt. A recipient-less own message (skip_own_non_peer_*)
        // stays skipped. Mirrors the hot-path copy in the whatsapp-rust crate.
        let info = MessageInfo {
            id: "FANOUT1".to_string(),
            source: MessageSource {
                chat: "200000000000002@bot".parse().unwrap(),
                sender: "100000000000001@lid".parse().unwrap(),
                recipient: Some("200000000000002@bot".parse().unwrap()),
                is_from_me: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(should_send_delivery_receipt(&info));
    }

    #[test]
    fn skip_own_status_and_group_even_with_recipient() {
        // Negative parity with Client::should_send_delivery_receipt: the
        // self-fanout allowance must NOT leak into own status broadcasts or
        // group messages, even when a recipient is present.
        let own_status = MessageInfo {
            id: "OWN_STATUS".to_string(),
            source: MessageSource {
                chat: "status@broadcast".parse().unwrap(),
                sender: "100000000000001@lid".parse().unwrap(),
                recipient: Some("100000000000001@lid".parse().unwrap()),
                is_from_me: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&own_status));

        let own_group = MessageInfo {
            id: "OWN_GROUP".to_string(),
            source: MessageSource {
                chat: "120363021033254949@g.us".parse().unwrap(),
                sender: "100000000000001@lid".parse().unwrap(),
                recipient: Some("100000000000001@lid".parse().unwrap()),
                is_from_me: true,
                is_group: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_send_delivery_receipt(&own_group));
    }

    #[test]
    fn allow_incoming_dm() {
        let info = MessageInfo {
            id: "DM1".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net".parse().unwrap(),
                sender: "12345@s.whatsapp.net".parse().unwrap(),
                is_from_me: false,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(should_send_delivery_receipt(&info));
    }

    // -- Aggregated and list receipt parsing -------------------------------

    use wacore_binary::builder::NodeBuilder;

    /// `<receipt><participants message_id="...">` shape with per-user types.
    /// Mirrors `WAWebHandleMsgReceiptParser` m() branch.
    #[test]
    fn participants_aggregated_by_message() {
        let node = NodeBuilder::new("receipt")
            .attr("id", "STANZA-AGG")
            .children([NodeBuilder::new("participants")
                .attr("message_id", "REAL-MSG-ID")
                .children([
                    NodeBuilder::new("user")
                        .attr("jid", "11111@lid")
                        .attr("t", "1700000001")
                        .attr("type", "delivery")
                        .build(),
                    NodeBuilder::new("user")
                        .attr("jid", "22222@lid")
                        .attr("t", "1700000002")
                        .attr("type", "read")
                        .build(),
                ])
                .build()])
            .build();
        let part = node.get_optional_child("participants").unwrap();
        let (message_id, key, users) = parse_participants(&part.as_node_ref());

        assert_eq!(message_id.as_deref(), Some("REAL-MSG-ID"));
        assert!(key.is_none());
        assert_eq!(users.len(), 2);
        assert_eq!(users[0].jid.user, "11111");
        assert_eq!(users[0].timestamp, Some(1700000001));
        assert_eq!(users[0].r#type.as_deref(), Some("delivery"));
        assert_eq!(users[1].r#type.as_deref(), Some("read"));
    }

    /// Missing `<user t>` attr leaves `timestamp` as `None` so the handler
    /// can fall back to the stanza-level `t`. Previous code defaulted to 0,
    /// which silently produced an epoch-zero `DateTime` instead of the
    /// stanza ts.
    #[test]
    fn participants_user_missing_t_yields_none_timestamp() {
        let node = NodeBuilder::new("receipt")
            .attr("id", "STANZA-NOT")
            .children([NodeBuilder::new("participants")
                .attr("message_id", "MSG-NOT")
                .children([NodeBuilder::new("user")
                    .attr("jid", "99000000000001@lid")
                    .attr("type", "delivery")
                    .build()])
                .build()])
            .build();
        let part = node.get_optional_child("participants").unwrap();
        let (_, _, users) = parse_participants(&part.as_node_ref());
        assert_eq!(users.len(), 1);
        assert!(
            users[0].timestamp.is_none(),
            "missing <user t> must yield None"
        );
    }

    /// `<participants key="...">` without `message_id` — aggregated_by_type.
    #[test]
    fn participants_aggregated_by_type() {
        let node = NodeBuilder::new("receipt")
            .attr("id", "STANZA-AGG2")
            .children([NodeBuilder::new("participants")
                .attr("key", "AGG-KEY")
                .children([NodeBuilder::new("user")
                    .attr("jid", "33333@lid")
                    .attr("t", "1700000003")
                    .build()])
                .build()])
            .build();
        let part = node.get_optional_child("participants").unwrap();
        let (message_id, key, users) = parse_participants(&part.as_node_ref());

        assert!(message_id.is_none());
        assert_eq!(key.as_deref(), Some("AGG-KEY"));
        assert_eq!(users.len(), 1);
        assert!(
            users[0].r#type.is_none(),
            "no per-user type on aggregated_by_type"
        );
    }

    /// `<list><item id=.../></list>` items are collected and the stanza id is
    /// appended LAST for non-view receipts. Mirrors p() branch.
    #[test]
    fn simple_message_ids_with_list() {
        let node = NodeBuilder::new("receipt")
            .attr("id", "STANZA-Z")
            .children([NodeBuilder::new("list")
                .children([
                    NodeBuilder::new("item").attr("id", "MSG-A").build(),
                    NodeBuilder::new("item").attr("id", "MSG-B").build(),
                ])
                .build()])
            .build();
        let ids = collect_simple_message_ids(&node.as_node_ref(), "STANZA-Z", false);
        assert_eq!(ids, vec!["MSG-A", "MSG-B", "STANZA-Z"]);
    }

    /// No `<list>` child: only the stanza id ends up in message_ids.
    #[test]
    fn simple_message_ids_without_list() {
        let node = NodeBuilder::new("receipt").attr("id", "SOLO").build();
        let ids = collect_simple_message_ids(&node.as_node_ref(), "SOLO", false);
        assert_eq!(ids, vec!["SOLO"]);
    }

    /// View receipts use `server_id` instead of `id` and DON'T append the
    /// stanza id. Mirrors `t.maybeAttrString("type")==="view"` branch.
    #[test]
    fn simple_message_ids_view_uses_server_id_no_stanza_append() {
        let node = NodeBuilder::new("receipt")
            .attr("id", "VIEW-STANZA")
            .children([NodeBuilder::new("list")
                .children([
                    NodeBuilder::new("item").attr("server_id", "100").build(),
                    NodeBuilder::new("item").attr("server_id", "101").build(),
                ])
                .build()])
            .build();
        let ids = collect_simple_message_ids(&node.as_node_ref(), "VIEW-STANZA", true);
        assert_eq!(ids, vec!["100", "101"]);
    }
}
