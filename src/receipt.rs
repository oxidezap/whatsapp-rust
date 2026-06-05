use crate::client::Client;
use crate::types::events::{Event, Receipt};
use crate::types::message::MessageInfo;
use crate::types::presence::ReceiptType;
use log::debug;
use std::sync::Arc;
use wacore::protocol::nack::NackReason;
use wacore::types::message::MessageCategory;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, JidExt as _};

use wacore_binary::OwnedNodeRef;

/// Pure builder for the delivery `<receipt>` node. Extracted so unit tests
/// can assert wire shape without spinning a transport. Mirrors WA Web's
/// `Send/DeliveryReceiptJob.js` — the participant gate there is
/// `(t.isGroup() || t.isBroadcast()) && r ? DEVICE_JID(r) : DROP_ATTR`, so
/// status broadcasts (isBroadcast = true) also carry the original poster's
/// JID. Without it the server can't map the ack back to the status owner.
/// `active=false` sends `type="inactive"` (not rendered as ticks), matching
/// whatsmeow's background companion. Peer/status keep their own type/context.
///
/// Self-fanout (`is_from_me` + a `recipient`) gets `type="sender"` + the
/// `recipient`, matching WA Web (`isMeAccount` author => SENDER) and whatsmeow.
/// The server's offline queue only drops a self-fanout on this sender receipt;
/// a bare transport `<ack>` is ignored and the stanza is replayed until a
/// ~50min GC closes the stream.
fn build_delivery_receipt_node(
    info: &crate::types::message::MessageInfo,
    active: bool,
) -> wacore_binary::Node {
    let is_status = info.source.chat.is_status_broadcast();
    // A peer-synced message takes `type="peer_msg"` and carries NO recipient
    // (WA Web `!l` guard), so the sender-receipt shape applies only off the
    // peer path.
    let sender_receipt = info.source.is_self_fanout() && info.category != MessageCategory::Peer;
    // Mirror whatsmeow `buildBaseReceipt` / WA Web `JID(extractJidFromJidWithType)`:
    // echo `from` verbatim so the device survives. `chat` strips it via to_non_ad,
    // which the LID server rejects for multi-device DMs.
    let to = if info.source.is_group || is_status {
        &info.source.chat
    } else {
        &info.source.sender
    };
    let mut builder = NodeBuilder::new("receipt")
        .attr("id", &info.id)
        .attr("to", to);

    if info.category == MessageCategory::Peer {
        builder = builder.attr("type", ReceiptType::PeerMsg.as_wire_str());
    } else if sender_receipt {
        builder = builder.attr("type", ReceiptType::Sender.as_wire_str());
    } else if !active && !is_status {
        builder = builder.attr("type", ReceiptType::Inactive.as_wire_str());
    }

    // Device-stripped recipient (WA Web `USER_JID`) so the server can route it.
    if sender_receipt && let Some(recipient) = &info.source.recipient {
        builder = builder.attr("recipient", recipient.to_non_ad());
    }

    if info.source.is_group || is_status {
        builder = builder.attr("participant", &info.source.sender);
    }

    if is_status {
        builder = builder.attr("context", "status");
    }

    builder.build()
}

/// `<ack class="message" error=N>` builder, mirrors WA Web's
/// `Handle/MsgSendAck.js::sendNack`. `failure_reason` is only emitted
/// for `InvalidProtobuf` (as `<meta failure_reason=N>` child).
fn build_nack_node(
    info: &MessageInfo,
    own_pn: &Jid,
    reason: NackReason,
    failure_reason: Option<i32>,
) -> wacore_binary::Node {
    let mut builder = NodeBuilder::new("ack")
        .attr("class", "message")
        .attr("id", &info.id)
        .attr("from", own_pn)
        .attr("to", &info.source.chat)
        .attr("error", reason.code().to_string());

    let is_status = info.source.chat.is_status_broadcast();
    if info.source.is_group || is_status {
        builder = builder.attr("participant", &info.source.sender);
    }

    if !info.r#type.is_empty() {
        builder = builder.attr("type", &info.r#type);
    }

    if reason == NackReason::InvalidProtobuf
        && let Some(code) = failure_reason
    {
        let meta = NodeBuilder::new("meta")
            .attr("failure_reason", code.to_string())
            .build();
        builder = builder.children(vec![meta]);
    }

    builder.build()
}

impl Client {
    pub(crate) fn should_send_delivery_receipt(info: &crate::types::message::MessageInfo) -> bool {
        if info.id.is_empty() || info.source.chat.is_newsletter() {
            return false;
        }

        // WA Web sends type="peer_msg" delivery receipts for self-synced
        // messages (category="peer").  These tell the primary phone that
        // this companion device received the message.
        // For all other messages, skip receipts for our own messages.
        //
        // status@broadcast: WA Web sends `<receipt context="status">`
        // (`Send/DeliveryReceiptJob.js` + `Handle/MsgSendReceipt.js` —
        // `C = y && isStatusStanzaReceiveEnabled() ? "status" : void 0`).
        // The context attribute is added in send_delivery_receipt below.
        //
        // Self-fanout (own message echoed back, carrying a `recipient`) needs a
        // sender receipt to drain the offline queue; without it the server
        // replays it until a ~50min GC closes the stream. A recipient-less own
        // message (self-note) stays skipped. See `build_delivery_receipt_node`.
        info.category == MessageCategory::Peer
            || !info.source.is_from_me
            || info.source.is_self_fanout()
    }

    pub(crate) async fn handle_receipt(self: &Arc<Self>, node: Arc<OwnedNodeRef>) {
        let nr = node.get();
        let mut attrs = nr.attrs();
        let from = attrs.jid("from");
        let stanza_id = match attrs.optional_string("id") {
            Some(id) => id.to_string(),
            None => {
                log::warn!("Receipt stanza missing required 'id' attribute");
                return;
            }
        };
        let receipt_type_cow = attrs.optional_string("type");
        let receipt_type_str = receipt_type_cow.as_deref().unwrap_or("delivery");
        let participant = attrs.optional_jid("participant");
        // participant_pn -> sender_alt so the LID-PN cache warms from receipts too.
        let participant_pn = attrs.optional_jid("participant_pn");
        let stanza_ts = attrs
            .optional_u64("t")
            .and_then(|t| i64::try_from(t).ok())
            .and_then(wacore::time::from_secs)
            .unwrap_or_else(wacore::time::now_utc);

        let receipt_type = ReceiptType::parse(receipt_type_str);
        // WA Web downgrades a delivery ack to "sent" (not delivered) when the receipt carries
        // <error reason="lid" type="feature-incapable"> (the LID peer can't receive it).
        let receipt_type =
            wacore::stanza::receipt::downgrade_for_feature_incapable(nr, receipt_type);
        let is_view = receipt_type_str == "view";
        let is_group = from.is_group();
        let default_sender = if is_group {
            participant.unwrap_or_else(|| from.clone())
        } else {
            from.clone()
        };

        // Aggregated shape (`<participants>` child): WAWebHandleMsgReceiptParser
        // produces one entry per `<user>`. Fan out into one Receipt event per
        // user so per-user type/timestamp/sender are not lost. Retries and
        // enc_rekey_retry never use the aggregated shape, so this short-circuits
        // before the retry pipeline below.
        if let Some(part_node) = nr.get_optional_child("participants") {
            let (agg_msg_id, agg_key, users) =
                wacore::stanza::receipt::parse_participants(part_node);
            let fan_out_id = agg_msg_id
                .clone()
                .or_else(|| agg_key.clone())
                .unwrap_or_else(|| stanza_id.clone());
            debug!(
                "Aggregated receipt from {from}: stanza={stanza_id} \
                 message_id={agg_msg_id:?} key={agg_key:?} users={}",
                users.len()
            );
            for user in users {
                // Missing `<user t>` means the server didn't disambiguate the
                // per-user time; fall back to the stanza-level `t`.
                let user_ts = user
                    .timestamp
                    .and_then(|t| i64::try_from(t).ok())
                    .and_then(wacore::time::from_secs)
                    .unwrap_or(stanza_ts);
                // aggregated_by_message: each <user> carries its own type;
                // aggregated_by_type: all users share the receipt-level type.
                let effective_type = match user.r#type.as_deref() {
                    // Apply the receipt-level feature-incapable downgrade to the per-user type
                    // too, so an aggregated delivery receipt with a feature-incapable LID
                    // participant doesn't re-emit a delivered tick for it.
                    Some(t) => wacore::stanza::receipt::downgrade_for_feature_incapable(
                        nr,
                        ReceiptType::parse(t),
                    ),
                    None => receipt_type.clone(),
                };
                let r = Receipt {
                    message_ids: vec![fan_out_id.clone()],
                    source: crate::types::message::MessageSource {
                        chat: from.clone(),
                        sender: user.jid,
                        sender_alt: user.participant_pn,
                        ..Default::default()
                    },
                    timestamp: user_ts,
                    r#type: effective_type,
                };
                self.core.event_bus.dispatch(Event::Receipt(r));
            }
            return;
        }

        // Simple receipt: collect `<list><item id=.../>` items plus the stanza
        // id (for non-view receipts), matching the JS p() branch.
        let message_ids =
            wacore::stanza::receipt::collect_simple_message_ids(nr, &stanza_id, is_view);

        debug!(
            "Received receipt type '{receipt_type:?}' for {} message(s) from {from}",
            message_ids.len()
        );

        let receipt = Receipt {
            message_ids,
            source: crate::types::message::MessageSource {
                chat: from,
                sender: default_sender,
                sender_alt: participant_pn,
                ..Default::default()
            },
            timestamp: stanza_ts,
            r#type: receipt_type,
        };

        if receipt.r#type == ReceiptType::Retry {
            let client_clone = Arc::clone(self);
            let node_clone = Arc::clone(&node);
            self.runtime
                .spawn(Box::pin(async move {
                    if let Err(e) = client_clone
                        .handle_retry_receipt(&receipt, &node_clone)
                        .await
                    {
                        log::warn!(
                            "Failed to handle retry receipt for {}: {:?}",
                            receipt.message_ids[0],
                            e
                        );
                    }
                }))
                .detach();
        } else if receipt.r#type == ReceiptType::EncRekeyRetry {
            // WA Web: both "retry" and "enc_rekey_retry" route through
            // handleMessageRetryRequest, but enc_rekey_retry branches to the
            // VoIP stack's resendEncRekeyRetry(peerJid, retryCount).
            // Since we don't have a VoIP stack yet, log and dispatch as a
            // Receipt event so consumers can observe it. When VoIP is
            // implemented (#345), this will route to the VoIP re-key handler.
            if let Some(child) = nr.get_optional_child("enc_rekey") {
                let mut child_attrs = child.attrs();
                log::debug!(
                    "Received enc_rekey_retry receipt for call-id={} from {} \
                     (call-creator={}, count={}). VoIP not implemented, forwarding as event.",
                    child_attrs
                        .optional_string("call-id")
                        .as_deref()
                        .unwrap_or_default(),
                    receipt.source.chat,
                    child_attrs
                        .optional_string("call-creator")
                        .as_deref()
                        .unwrap_or_default(),
                    child_attrs
                        .optional_string("count")
                        .and_then(|s| s.parse::<u8>().ok())
                        .unwrap_or(1),
                );
            }
            self.core.event_bus.dispatch(Event::Receipt(receipt));
        } else {
            self.core.event_bus.dispatch(Event::Receipt(receipt));
        }
    }

    /// Sends a delivery receipt to the sender of a message.
    ///
    /// Eligibility lives in [`Self::should_send_delivery_receipt`]; the wire
    /// shape is assembled by [`build_delivery_receipt_node`]. Coverage:
    ///
    /// - Direct messages (DMs) — `<receipt>` to the sender's JID.
    /// - Group messages — `<receipt participant=...>` to the group JID.
    /// - Peer device messages (`category="peer"`) — `<receipt type="peer_msg">`
    ///   to acknowledge self-synced messages from the primary phone.
    /// - Status broadcasts — `<receipt context="status">` (WA Web's
    ///   `Send/DeliveryReceiptJob.js`); these are NOT skipped anymore.
    /// - Newsletters and messages without an ID are skipped (newsletters are
    ///   handled by the ack gate, not here).
    pub(crate) async fn send_delivery_receipt(&self, info: &crate::types::message::MessageInfo) {
        if !Self::should_send_delivery_receipt(info) {
            return;
        }

        let receipt_node = build_delivery_receipt_node(info, self.receipts_are_active());

        // Mirror build_delivery_receipt_node's type selection so the log is
        // accurate (a passive companion emits `inactive`, not `delivery`).
        let receipt_kind = if info.category == MessageCategory::Peer {
            ReceiptType::PeerMsg
        } else if info.source.is_self_fanout() {
            ReceiptType::Sender
        } else if !self.receipts_are_active() && !info.source.chat.is_status_broadcast() {
            ReceiptType::Inactive
        } else {
            ReceiptType::Delivered
        };
        debug!(target: "Client/Receipt", "Sending {} receipt for message {} to {}",
            receipt_kind.as_wire_str(), info.id, info.source.sender);

        if let Err(e) = self.send_node(receipt_node).await
            && !matches!(e, crate::client::ClientError::NotConnected)
        {
            log::warn!(target: "Client/Receipt", "Failed to send delivery receipt for message {}: {:?}", info.id, e);
        }
    }

    /// Spawn an async nack so the caller doesn't await network I/O while
    /// holding a session lock. Mirrors `spawn_retry_receipt`.
    pub(crate) fn spawn_nack(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        reason: NackReason,
        failure_reason: Option<i32>,
    ) {
        let client = Arc::clone(self);
        let info = Arc::clone(info);
        self.runtime
            .spawn(Box::pin(async move {
                client.send_nack(&info, reason, failure_reason).await;
            }))
            .detach();
    }

    /// Emits a nack so the server stops retransmitting an unrecoverable
    /// failure. Prefer [`Client::send_retry_receipt`] for recoverable
    /// errors (BadMac, NoSession, etc).
    pub(crate) async fn send_nack(
        &self,
        info: &MessageInfo,
        reason: NackReason,
        failure_reason: Option<i32>,
    ) {
        if info.id.is_empty() {
            return;
        }
        let Some(own_pn) = self.get_pn().await else {
            log::debug!(
                "[msg:{}] Skipping nack ({:?}): own PN not yet set",
                info.id,
                reason
            );
            return;
        };

        let nack = build_nack_node(info, &own_pn, reason, failure_reason);
        debug!(target: "Client/Receipt",
            "Sending nack (reason={:?}, code={}) for message {} from {}",
            reason, reason.code(), info.id, info.source.sender);

        if let Err(e) = self.send_node(nack).await
            && !matches!(e, crate::client::ClientError::NotConnected)
        {
            log::warn!(target: "Client/Receipt",
                "Failed to send nack for message {}: {:?}", info.id, e);
        }
    }

    /// Sends read receipts for one or more messages.
    ///
    /// For group messages, pass the message sender as `sender`.
    pub async fn mark_as_read(
        &self,
        chat: &Jid,
        sender: Option<&Jid>,
        message_ids: Vec<String>,
    ) -> Result<(), anyhow::Error> {
        if message_ids.is_empty() {
            return Ok(());
        }

        let timestamp = wacore::time::now_secs_u64().to_string();

        let mut builder = NodeBuilder::new("receipt")
            .attr("to", chat)
            .attr("type", "read")
            .attr("id", &message_ids[0])
            .attr("t", &timestamp);

        if let Some(sender) = sender {
            builder = builder.attr("participant", sender);
        }

        // Additional message IDs go into <list><item id="..."/></list>
        if message_ids.len() > 1 {
            let items: Vec<wacore_binary::Node> = message_ids[1..]
                .iter()
                .map(|id| NodeBuilder::new("item").attr("id", id).build())
                .collect();
            builder = builder.children(vec![NodeBuilder::new("list").children(items).build()]);
        }

        let node = builder.build();

        debug!(target: "Client/Receipt", "Sending read receipt for {} message(s) to {}", message_ids.len(), chat);

        self.send_node(node)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to send read receipt: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::test_utils::{MockHttpClient, TestEventCollector};
    use crate::types::message::{MessageInfo, MessageSource};

    fn node_to_arc(node: wacore_binary::Node) -> Arc<OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(&node)
    }

    fn info_with(chat: &str, sender: &str, is_group: bool) -> MessageInfo {
        MessageInfo {
            id: "MID".to_string(),
            source: MessageSource {
                chat: chat.parse().expect("test chat JID"),
                sender: sender.parse().expect("test sender JID"),
                is_from_me: false,
                is_group,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn delivery_receipt_for_status_broadcast_carries_context_status_and_participant() {
        // WA Web's gate is `(isGroup || isBroadcast) && participant` for the
        // participant attr, and `isStatus && gating` for context — see
        // `Send/DeliveryReceiptJob.js`. Status broadcasts must carry BOTH so
        // the server can map the ack back to the status owner.
        let info = info_with("status@broadcast", "12345@s.whatsapp.net", false);
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(node.tag, "receipt");
        assert_eq!(
            node.attrs.get("context").map(|v| v.as_str()).as_deref(),
            Some("status")
        );
        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("12345@s.whatsapp.net")
        );
    }

    #[test]
    fn delivery_receipt_for_dm_has_no_context_no_participant() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let node = build_delivery_receipt_node(&info, true);
        assert!(node.attrs.get("context").is_none());
        assert!(node.attrs.get("participant").is_none());
        assert!(node.attrs.get("type").is_none());
    }

    #[test]
    fn delivery_receipt_for_self_fanout_to_bot_is_sender_with_recipient() {
        // Own prompt to a @bot, echoed back: <receipt type="sender" to=ourLID
        // recipient=@bot>, `to` preserving the sender's device. Mirrors WA Web
        // DeliveryReceiptJob (SENDER + USER_JID(recipient)) and whatsmeow.
        let info = MessageInfo {
            id: "FANOUT_BOT".to_string(),
            source: MessageSource {
                sender: "100000000000001:11@lid".parse().expect("sender"),
                chat: "200000000000002@bot".parse().expect("chat"),
                recipient: Some("200000000000002@bot".parse().expect("recipient")),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(node.tag, "receipt");
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("sender")
        );
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("100000000000001:11@lid"),
            "`to` must preserve the own device or the LID server rejects it"
        );
        assert_eq!(
            node.attrs.get("recipient").map(|v| v.as_str()).as_deref(),
            Some("200000000000002@bot")
        );
        assert!(node.attrs.get("participant").is_none());
        assert!(node.attrs.get("context").is_none());
    }

    #[test]
    fn delivery_receipt_for_self_fanout_strips_recipient_device() {
        // WA Web's `USER_JID` strips the device from `recipient`; a fanout to a
        // multi-device user echoes the non-AD recipient.
        let info = MessageInfo {
            id: "FANOUT_DEV".to_string(),
            source: MessageSource {
                sender: "100000000000001:5@lid".parse().expect("sender"),
                chat: "300000000000003@lid".parse().expect("chat"),
                recipient: Some("300000000000003:7@lid".parse().expect("recipient")),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("recipient").map(|v| v.as_str()).as_deref(),
            Some("300000000000003@lid"),
            "recipient device must be stripped (USER_JID semantics)"
        );
    }

    #[test]
    fn peer_self_fanout_is_peer_msg_without_recipient() {
        // A peer-synced message that also looks like a self-fanout (is_from_me +
        // recipient) must keep `type="peer_msg"` and carry NO recipient (WA Web
        // `!l` guard), never `type="sender"`.
        let info = MessageInfo {
            id: "PEER_FANOUT".to_string(),
            source: MessageSource {
                sender: "100000000000001@lid".parse().expect("sender"),
                chat: "300000000000003@lid".parse().expect("chat"),
                recipient: Some("300000000000003@lid".parse().expect("recipient")),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            category: MessageCategory::Peer,
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("peer_msg")
        );
        assert!(
            node.attrs.get("recipient").is_none(),
            "a peer_msg receipt must not carry a recipient"
        );
    }

    #[test]
    fn self_fanout_is_sender_even_when_inactive() {
        // type=sender takes precedence over the inactive (passive companion)
        // branch: a self-fanout is always acknowledged as sender.
        let info = MessageInfo {
            id: "FANOUT_INACTIVE".to_string(),
            source: MessageSource {
                sender: "100000000000001@lid".parse().expect("sender"),
                chat: "200000000000002@bot".parse().expect("chat"),
                recipient: Some("200000000000002@bot".parse().expect("recipient")),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, false);
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("sender"),
            "self-fanout must stay type=sender, not become inactive"
        );
        assert_eq!(
            node.attrs.get("recipient").map(|v| v.as_str()).as_deref(),
            Some("200000000000002@bot")
        );
    }

    #[test]
    fn delivery_receipt_is_inactive_when_not_active() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let inactive = build_delivery_receipt_node(&info, false);
        assert_eq!(
            inactive.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("inactive"),
            "a passive companion sends inactive delivery receipts"
        );
        let active = build_delivery_receipt_node(&info, true);
        assert!(active.attrs.get("type").is_none());
    }

    #[test]
    fn status_and_peer_receipts_ignore_inactive() {
        let status = info_with("status@broadcast", "12345@s.whatsapp.net", false);
        let node = build_delivery_receipt_node(&status, false);
        // status keeps context, never type=inactive
        assert!(node.attrs.get("type").is_none());
        assert_eq!(
            node.attrs.get("context").map(|v| v.as_str()).as_deref(),
            Some("status")
        );

        let mut peer = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        peer.category = MessageCategory::Peer;
        let node = build_delivery_receipt_node(&peer, false);
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("peer_msg")
        );
    }

    #[test]
    fn delivery_receipt_for_group_carries_participant() {
        let info = info_with(
            "120363021033254949@g.us",
            "15551234567@s.whatsapp.net",
            true,
        );
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("15551234567@s.whatsapp.net")
        );
        assert!(node.attrs.get("context").is_none());
    }

    #[test]
    fn should_send_delivery_receipt_allows_status_broadcast() {
        let info = info_with("status@broadcast", "12345@s.whatsapp.net", false);
        assert!(Client::should_send_delivery_receipt(&info));
    }

    /// Regression: LID DM with explicit device must echo the device in `to`
    /// (matches whatsmeow buildBaseReceipt + WA Web JID encoding). Stripping
    /// the device caused <stream:error><ack/> for multi-device LID senders.
    #[test]
    fn delivery_receipt_for_lid_dm_preserves_device_in_to() {
        let info = MessageInfo {
            id: "LID_DEV_RECEIPT".to_string(),
            source: MessageSource {
                // chat is the non-AD form (matches parse_message_info's
                // chat = from.to_non_ad()).
                chat: "156535032389744@lid".parse().expect("chat"),
                // sender preserves device (matches parse_message_info's
                // sender = from.clone()).
                sender: "156535032389744:7@lid".parse().expect("sender"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("156535032389744:7@lid"),
            "LID DM receipt must preserve the device or the server rejects the ack"
        );
        assert!(node.attrs.get("participant").is_none());
    }

    /// LID DM without device stays as-is (no-op for the common case).
    #[test]
    fn delivery_receipt_for_lid_dm_no_device_unchanged() {
        let info = MessageInfo {
            id: "LID_NO_DEV".to_string(),
            source: MessageSource {
                chat: "185323896221943@lid".parse().expect("chat"),
                sender: "185323896221943@lid".parse().expect("sender"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("185323896221943@lid")
        );
    }

    /// Group: `to` must remain the group JID, participant carries the device.
    #[test]
    fn delivery_receipt_for_group_to_is_group_not_sender() {
        let info = MessageInfo {
            id: "GRP_RECEIPT".to_string(),
            source: MessageSource {
                chat: "120363021033254949@g.us".parse().expect("group"),
                sender: "156535032389744:7@lid".parse().expect("sender"),
                is_from_me: false,
                is_group: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("120363021033254949@g.us")
        );
        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("156535032389744:7@lid")
        );
    }

    /// peer_msg: `to` echoes from (us with device), no participant.
    #[test]
    fn delivery_receipt_for_peer_dm_to_preserves_device() {
        let mut info = MessageInfo {
            id: "PEER_DEV".to_string(),
            source: MessageSource {
                chat: "9999999999@lid".parse().expect("chat"),
                sender: "9999999999:3@lid".parse().expect("sender"),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        info.category = MessageCategory::Peer;
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("9999999999:3@lid")
        );
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("peer_msg")
        );
        assert!(node.attrs.get("participant").is_none());
    }

    /// status@broadcast: `to` must stay status@broadcast (chat), participant
    /// carries the original sender device.
    #[test]
    fn delivery_receipt_for_status_to_is_status_not_sender() {
        let info = MessageInfo {
            id: "STATUS_RECEIPT".to_string(),
            source: MessageSource {
                chat: "status@broadcast".parse().expect("status"),
                sender: "156535032389744:7@lid".parse().expect("sender"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("to").map(|v| v.as_str()).as_deref(),
            Some("status@broadcast")
        );
        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("156535032389744:7@lid")
        );
        assert_eq!(
            node.attrs.get("context").map(|v| v.as_str()).as_deref(),
            Some("status")
        );
    }

    #[test]
    fn delivery_receipt_for_peer_dm_carries_type_peer_msg() {
        // category=Peer + DM (self device sync) → type="peer_msg", no
        // participant, no context. Matches WA Web's DROP_ATTR gating.
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.category = MessageCategory::Peer;
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("peer_msg")
        );
        assert!(node.attrs.get("participant").is_none());
        assert!(node.attrs.get("context").is_none());
    }

    #[test]
    fn delivery_receipt_for_status_broadcast_keeps_participant_even_with_peer_type() {
        // Defensive: if a status broadcast ever surfaces with category=Peer,
        // the participant attr must still be there — server identifies the
        // status owner from it regardless of the peer_msg type.
        let mut info = info_with("status@broadcast", "12345@s.whatsapp.net", false);
        info.category = MessageCategory::Peer;
        let node = build_delivery_receipt_node(&info, true);
        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("12345@s.whatsapp.net")
        );
        assert_eq!(
            node.attrs.get("context").map(|v| v.as_str()).as_deref(),
            Some("status")
        );
    }

    fn own_pn() -> Jid {
        "5511000000001:0@s.whatsapp.net"
            .parse()
            .expect("own PN should parse")
    }

    #[test]
    fn nack_for_dm_carries_class_message_and_error_code() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let node = build_nack_node(&info, &own_pn(), NackReason::ParsingError, None);

        assert_eq!(node.tag, "ack");
        assert_eq!(
            node.attrs.get("class").map(|v| v.as_str()).as_deref(),
            Some("message")
        );
        assert_eq!(
            node.attrs.get("error").map(|v| v.as_str()).as_deref(),
            Some("487")
        );
        assert_eq!(
            node.attrs.get("id").map(|v| v.as_str()).as_deref(),
            Some("MID")
        );
        assert!(node.attrs.get("from").is_some());
        assert!(node.attrs.get("to").is_some());
        assert!(node.attrs.get("participant").is_none());
    }

    #[test]
    fn nack_for_group_carries_participant() {
        let info = info_with(
            "120363021033254949@g.us",
            "15551234567@s.whatsapp.net",
            true,
        );
        let node = build_nack_node(&info, &own_pn(), NackReason::UnhandledError, None);

        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("15551234567@s.whatsapp.net")
        );
        assert_eq!(
            node.attrs.get("error").map(|v| v.as_str()).as_deref(),
            Some("500")
        );
    }

    #[test]
    fn nack_for_status_broadcast_carries_participant() {
        let info = info_with("status@broadcast", "12345@s.whatsapp.net", false);
        let node = build_nack_node(&info, &own_pn(), NackReason::ParsingError, None);

        assert_eq!(
            node.attrs.get("participant").map(|v| v.as_str()).as_deref(),
            Some("12345@s.whatsapp.net")
        );
    }

    #[test]
    fn nack_invalid_protobuf_includes_meta_failure_reason() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let node = build_nack_node(&info, &own_pn(), NackReason::InvalidProtobuf, Some(42));

        assert_eq!(
            node.attrs.get("error").map(|v| v.as_str()).as_deref(),
            Some("491")
        );
        let meta = node
            .get_optional_child("meta")
            .expect("InvalidProtobuf nack must have <meta> child");
        assert_eq!(
            meta.attrs
                .get("failure_reason")
                .map(|v| v.as_str())
                .as_deref(),
            Some("42")
        );
    }

    #[test]
    fn nack_invalid_protobuf_without_failure_reason_omits_meta() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let node = build_nack_node(&info, &own_pn(), NackReason::InvalidProtobuf, None);
        assert!(node.get_optional_child("meta").is_none());
    }

    /// failure_reason only applies to InvalidProtobuf.
    #[test]
    fn nack_omits_meta_for_non_invalid_protobuf_even_with_failure_reason() {
        let info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        let node = build_nack_node(&info, &own_pn(), NackReason::ParsingError, Some(99));
        assert!(node.get_optional_child("meta").is_none());
    }

    #[test]
    fn nack_includes_type_when_present() {
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.r#type = "text".to_string();
        let node = build_nack_node(&info, &own_pn(), NackReason::ParsingError, None);
        assert_eq!(
            node.attrs.get("type").map(|v| v.as_str()).as_deref(),
            Some("text")
        );
    }

    #[test]
    fn nack_omits_type_when_empty() {
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.r#type = String::new();
        let node = build_nack_node(&info, &own_pn(), NackReason::ParsingError, None);
        assert!(node.attrs.get("type").is_none());
    }

    #[test]
    fn should_send_delivery_receipt_skips_newsletter() {
        let info = info_with(
            "120363298765432100@newsletter",
            "120363298765432100@newsletter",
            false,
        );
        assert!(!Client::should_send_delivery_receipt(&info));
    }

    #[test]
    fn should_send_delivery_receipt_skips_empty_id() {
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.id = String::new();
        assert!(!Client::should_send_delivery_receipt(&info));
    }

    #[test]
    fn should_send_delivery_receipt_skips_own_dm() {
        // Self-sent message with NO `recipient` (a self-note where from==to):
        // not a fanout, so no receipt. Peer-category self-sync and self-fanouts
        // (which carry a `recipient`) are handled by the cases below.
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.source.is_from_me = true;
        assert!(info.source.recipient.is_none());
        assert!(!Client::should_send_delivery_receipt(&info));
    }

    #[test]
    fn should_send_delivery_receipt_allows_self_fanout_to_user() {
        // Own outgoing DM to another user, echoed back to this device
        // (is_from_me + recipient). WA Web emits a `<receipt type="sender">`.
        let mut info = info_with("300000000000003@lid", "100000000000001@lid", false);
        info.source.is_from_me = true;
        info.source.recipient = Some("300000000000003@lid".parse().expect("recipient"));
        assert!(Client::should_send_delivery_receipt(&info));
    }

    #[test]
    fn should_send_delivery_receipt_allows_self_fanout_to_bot() {
        // The reported disconnect-loop case: our own prompt to a @bot, echoed
        // back. Must get a sender receipt or the server replays it forever.
        let mut info = info_with("200000000000002@bot", "100000000000001@lid", false);
        info.source.is_from_me = true;
        info.source.recipient = Some("200000000000002@bot".parse().expect("recipient"));
        assert!(Client::should_send_delivery_receipt(&info));
    }

    #[test]
    fn should_send_delivery_receipt_skips_own_status_and_group_fanout() {
        // Regression guard: the self-fanout allowance must NOT leak into our own
        // status broadcasts or group messages (WA Web does not send a DM-style
        // sender receipt there).
        let mut own_status = info_with("status@broadcast", "100000000000001@lid", false);
        own_status.source.is_from_me = true;
        own_status.source.recipient = Some("100000000000001@lid".parse().expect("recipient"));
        assert!(!Client::should_send_delivery_receipt(&own_status));

        let mut own_group = info_with("120363021033254949@g.us", "100000000000001@lid", true);
        own_group.source.is_from_me = true;
        own_group.source.recipient = Some("100000000000001@lid".parse().expect("recipient"));
        assert!(!Client::should_send_delivery_receipt(&own_group));
    }

    #[test]
    fn should_send_delivery_receipt_allows_own_peer_msg() {
        // Self-synced messages from the primary phone (category=Peer) DO need
        // a receipt with type="peer_msg", per the WA Web `OUR_OWN_DEVICE` ack.
        let mut info = info_with("12345@s.whatsapp.net", "12345@s.whatsapp.net", false);
        info.source.is_from_me = true;
        info.category = MessageCategory::Peer;
        assert!(Client::should_send_delivery_receipt(&info));
    }

    #[tokio::test]
    async fn test_send_delivery_receipt_dm() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let info = MessageInfo {
            id: "TEST-ID-123".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                sender: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };

        // This should complete without panicking. The actual node sending
        // would fail since we're not connected, but the function should
        // handle that gracefully and log a warning.
        client.send_delivery_receipt(&info).await;

        // If we got here, the function executed successfully.
        // In a real scenario, we'd need to mock the transport to verify
        // the exact node sent, but basic functionality testing confirms
        // the method doesn't panic and logs appropriately.
    }

    #[tokio::test]
    async fn test_send_delivery_receipt_group() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let info = MessageInfo {
            id: "GROUP-MSG-ID".to_string(),
            source: MessageSource {
                chat: "120363021033254949@g.us"
                    .parse()
                    .expect("test JID should be valid"),
                sender: "15551234567@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                is_from_me: false,
                is_group: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should complete without panicking for group messages too.
        client.send_delivery_receipt(&info).await;
    }

    #[tokio::test]
    async fn test_skip_delivery_receipt_for_own_messages() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let info = MessageInfo {
            id: "OWN-MSG-ID".to_string(),
            source: MessageSource {
                chat: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                sender: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                is_from_me: true, // Own message
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should return early without attempting to send.
        // We can't easily assert that send_node was not called without
        // refactoring, but at least verify the function completes.
        client.send_delivery_receipt(&info).await;
    }

    #[tokio::test]
    async fn test_skip_delivery_receipt_for_empty_id() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let info = MessageInfo {
            id: "".to_string(), // Empty ID
            source: MessageSource {
                chat: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                sender: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should return early without attempting to send.
        client.send_delivery_receipt(&info).await;
    }

    #[tokio::test]
    async fn test_skip_delivery_receipt_for_status_broadcast() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let info = MessageInfo {
            id: "STATUS-MSG-ID".to_string(),
            source: MessageSource {
                chat: "status@broadcast"
                    .parse()
                    .expect("test JID should be valid"), // Status broadcast
                sender: "12345@s.whatsapp.net"
                    .parse()
                    .expect("test JID should be valid"),
                is_from_me: false,
                is_group: true,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should return early without attempting to send for status broadcasts.
        client.send_delivery_receipt(&info).await;
    }

    #[test]
    fn test_should_skip_delivery_receipt_for_newsletter() {
        let info = MessageInfo {
            id: "NEWSLETTER-MSG-ID".to_string(),
            source: MessageSource {
                chat: "120363173003902460@newsletter"
                    .parse()
                    .expect("newsletter JID should be valid"),
                sender: "120363173003902460@newsletter"
                    .parse()
                    .expect("newsletter JID should be valid"),
                is_from_me: false,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(
            !Client::should_send_delivery_receipt(&info),
            "generic delivery receipts must be skipped for newsletters"
        );
    }

    #[test]
    fn test_should_send_peer_msg_receipt_for_self_synced_messages() {
        // Self-synced messages (category="peer") should get delivery receipts
        // even though is_from_me is true.  WA Web sends type="peer_msg" for these.
        let info = MessageInfo {
            id: "PEER-MSG-ID".to_string(),
            source: MessageSource {
                chat: "155500012345@s.whatsapp.net"
                    .parse()
                    .expect("own PN JID should be valid"),
                sender: "155500012345@s.whatsapp.net"
                    .parse()
                    .expect("own PN JID should be valid"),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            category: MessageCategory::Peer,
            ..Default::default()
        };

        assert!(
            Client::should_send_delivery_receipt(&info),
            "peer device messages must get delivery receipts even when is_from_me"
        );
    }

    /// Create a test client with an event collector registered.
    async fn setup_client_with_collector() -> (Arc<Client>, Arc<TestEventCollector>) {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());
        (client, collector)
    }

    /// Verify that enc_rekey_retry receipt is dispatched as a Receipt event
    /// with EncRekeyRetry type so consumers can observe it.
    #[tokio::test]
    async fn test_enc_rekey_retry_receipt_dispatches_event() {
        let (client, collector) = setup_client_with_collector().await;

        // Build an enc_rekey_retry receipt node matching WA Web structure
        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "5511999999999@s.whatsapp.net")
                .attr("id", "3EB0AABBCCDD")
                .attr("type", "enc_rekey_retry")
                .children([
                    NodeBuilder::new("enc_rekey")
                        .attr("call-creator", "5511888888888@s.whatsapp.net")
                        .attr("call-id", "CALL-123")
                        .attr("count", "1")
                        .build(),
                    NodeBuilder::new("registration")
                        .bytes(12345u32.to_be_bytes().to_vec())
                        .build(),
                ])
                .build(),
        );

        client.handle_receipt(node).await;

        // Must dispatch exactly one Receipt event with EncRekeyRetry type
        let events = collector.events();
        let receipt_events: Vec<_> = events
            .iter()
            .filter_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(
            receipt_events.len(),
            1,
            "enc_rekey_retry must dispatch exactly one Receipt event"
        );
        assert_eq!(
            receipt_events[0].r#type,
            ReceiptType::EncRekeyRetry,
            "dispatched receipt must have EncRekeyRetry type"
        );
        assert_eq!(receipt_events[0].message_ids, vec!["3EB0AABBCCDD"]);
    }

    /// Verify that enc_rekey_retry without <enc_rekey> child still dispatches
    /// the Receipt event (graceful degradation, no crash).
    #[tokio::test]
    async fn test_enc_rekey_retry_receipt_without_child_still_dispatches() {
        let (client, collector) = setup_client_with_collector().await;

        // Malformed: no <enc_rekey> child
        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "5511999999999@s.whatsapp.net")
                .attr("id", "3EB0AABBCCDD")
                .attr("type", "enc_rekey_retry")
                .build(),
        );

        client.handle_receipt(node).await;

        // Should still dispatch the Receipt event even without <enc_rekey> child
        let events = collector.events();
        let receipt_events: Vec<_> = events
            .iter()
            .filter_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(
            receipt_events.len(),
            1,
            "malformed enc_rekey_retry must still dispatch Receipt event"
        );
        assert_eq!(receipt_events[0].r#type, ReceiptType::EncRekeyRetry);
    }

    #[test]
    fn test_should_skip_non_peer_self_messages() {
        // Normal self messages (no category) should still be skipped.
        let info = MessageInfo {
            id: "SELF-MSG-ID".to_string(),
            source: MessageSource {
                chat: "155500012345@s.whatsapp.net"
                    .parse()
                    .expect("own PN JID should be valid"),
                sender: "155500012345@s.whatsapp.net"
                    .parse()
                    .expect("own PN JID should be valid"),
                is_from_me: true,
                is_group: false,
                ..Default::default()
            },
            ..Default::default()
        };

        assert!(
            !Client::should_send_delivery_receipt(&info),
            "non-peer self messages must not get delivery receipts"
        );
    }

    /// Aggregated-by-message receipt: fan out one Receipt event per `<user>`
    /// with that user's type, and use the `message_id` attr (not the stanza
    /// id) as the message id. Matches `WAWebHandleMsgReceiptParser` m() branch.
    #[tokio::test]
    async fn test_aggregated_by_message_receipt_fans_out_per_user() {
        let (client, collector) = setup_client_with_collector().await;

        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "120363000000000001@g.us")
                .attr("id", "STANZA-AGG-XYZ")
                .attr("t", "1700000000")
                .children([NodeBuilder::new("participants")
                    .attr("message_id", "REAL-MSG-ID")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", "99000000000001@lid")
                            .attr("t", "1700000001")
                            .attr("type", "delivery")
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", "99000000000002@lid")
                            .attr("t", "1700000002")
                            .attr("type", "read")
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", "99000000000003@lid")
                            .attr("t", "1700000003")
                            .attr("type", "inactive")
                            .build(),
                    ])
                    .build()])
                .build(),
        );
        client.handle_receipt(node).await;

        let events = collector.events();
        let receipts: Vec<_> = events
            .iter()
            .filter_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(receipts.len(), 3, "must dispatch one event per <user>");
        for r in &receipts {
            assert_eq!(
                r.message_ids,
                vec!["REAL-MSG-ID"],
                "fan-out events must use participants.message_id, not stanza id"
            );
            assert_eq!(r.source.chat.user, "120363000000000001");
        }
        assert_eq!(receipts[0].r#type, ReceiptType::Delivered);
        assert_eq!(receipts[0].source.sender.user, "99000000000001");
        assert_eq!(receipts[1].r#type, ReceiptType::Read);
        assert_eq!(receipts[2].r#type, ReceiptType::Inactive);
    }

    /// participant_pn must land in the Receipt event's sender_alt on both shapes.
    #[tokio::test]
    async fn test_receipt_threads_participant_pn_into_sender_alt() {
        let (client, collector) = setup_client_with_collector().await;

        // Aggregated shape: per-user participant_pn.
        client
            .handle_receipt(node_to_arc(
                NodeBuilder::new("receipt")
                    .attr("from", "120363000000000001@g.us")
                    .attr("id", "STANZA-PPN")
                    .attr("t", "1700000000")
                    .children([NodeBuilder::new("participants")
                        .attr("message_id", "MSG-PPN")
                        .children([NodeBuilder::new("user")
                            .attr("jid", "99000000000001@lid")
                            .attr("participant_pn", "15551234567@s.whatsapp.net")
                            .attr("type", "read")
                            .build()])
                        .build()])
                    .build(),
            ))
            .await;

        // Simple shape: receipt-level participant_pn.
        client
            .handle_receipt(node_to_arc(
                NodeBuilder::new("receipt")
                    .attr("from", "99000000000002@lid")
                    .attr("id", "STANZA-PPN-SIMPLE")
                    .attr("participant_pn", "15557654321@s.whatsapp.net")
                    .attr("t", "1700000000")
                    .build(),
            ))
            .await;

        let events = collector.events();
        let receipts: Vec<_> = events
            .iter()
            .filter_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .collect();

        let agg = receipts
            .iter()
            .find(|r| r.message_ids.iter().any(|id| id == "MSG-PPN"))
            .expect("aggregated receipt dispatched");
        assert_eq!(
            agg.source.sender_alt.as_ref().expect("sender_alt set").user,
            "15551234567",
            "aggregated receipt must thread per-user participant_pn into sender_alt"
        );

        let simple = receipts
            .iter()
            .find(|r| r.message_ids.iter().any(|id| id == "STANZA-PPN-SIMPLE"))
            .expect("simple receipt dispatched");
        assert_eq!(
            simple
                .source
                .sender_alt
                .as_ref()
                .expect("sender_alt set")
                .user,
            "15557654321",
            "simple receipt must thread receipt-level participant_pn into sender_alt"
        );
    }

    /// Missing per-user `t`: the fan-out event's timestamp falls back to
    /// the stanza-level `t` rather than collapsing to epoch zero (which
    /// was the previous behavior).
    #[tokio::test]
    async fn test_aggregated_user_missing_t_uses_stanza_timestamp() {
        let (client, collector) = setup_client_with_collector().await;

        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "120363000000000001@g.us")
                .attr("id", "STANZA-AGG-NOT")
                .attr("t", "1700000000")
                .children([NodeBuilder::new("participants")
                    .attr("message_id", "REAL-MSG-NOT")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "99000000000001@lid")
                        .attr("type", "delivery")
                        .build()])
                    .build()])
                .build(),
        );
        client.handle_receipt(node).await;

        let events = collector.events();
        let r = events
            .iter()
            .find_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .expect("expected Receipt");
        let expected = wacore::time::from_secs(1700000000).expect("valid ts");
        assert_eq!(r.timestamp, expected);
    }

    /// Aggregated-by-type receipt: `<participants key="...">` without
    /// `message_id`. All users inherit the receipt-level type. Mirrors d() branch.
    #[tokio::test]
    async fn test_aggregated_by_type_receipt_uses_receipt_level_type() {
        let (client, collector) = setup_client_with_collector().await;

        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "120363000000000001@g.us")
                .attr("id", "STANZA-KEY")
                .attr("type", "read")
                .attr("t", "1700000000")
                .children([NodeBuilder::new("participants")
                    .attr("key", "AGG-KEY")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "99000000000001@lid")
                        .attr("t", "1700000001")
                        .build()])
                    .build()])
                .build(),
        );
        client.handle_receipt(node).await;

        let events = collector.events();
        let receipts: Vec<_> = events
            .iter()
            .filter_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(receipts.len(), 1);
        assert_eq!(receipts[0].r#type, ReceiptType::Read);
        assert_eq!(receipts[0].message_ids, vec!["AGG-KEY"]);
    }

    /// `<list><item id=.../>` batched read receipt: all items plus the stanza
    /// id (appended last) must end up in `message_ids`. Pre-fix only the
    /// stanza id was kept.
    #[tokio::test]
    async fn test_simple_receipt_with_list_collects_all_ids() {
        let (client, collector) = setup_client_with_collector().await;

        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "99000000000001@s.whatsapp.net")
                .attr("id", "MSG-A")
                .attr("type", "read")
                .attr("t", "1700000000")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("item").attr("id", "MSG-B").build(),
                        NodeBuilder::new("item").attr("id", "MSG-C").build(),
                    ])
                    .build()])
                .build(),
        );
        client.handle_receipt(node).await;

        let events = collector.events();
        let r = events
            .iter()
            .find_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .expect("expected Receipt");
        // Stanza id is appended LAST per WAWebHandleMsgReceiptParser.
        assert_eq!(r.message_ids, vec!["MSG-B", "MSG-C", "MSG-A"]);
        assert_eq!(r.r#type, ReceiptType::Read);
    }

    /// Simple receipt without `<list>`: only the stanza id is in message_ids.
    #[tokio::test]
    async fn test_simple_receipt_without_list_uses_stanza_id() {
        let (client, collector) = setup_client_with_collector().await;

        let node = node_to_arc(
            NodeBuilder::new("receipt")
                .attr("from", "99000000000001@s.whatsapp.net")
                .attr("id", "SOLO-MSG")
                .attr("t", "1700000000")
                .build(),
        );
        client.handle_receipt(node).await;

        let events = collector.events();
        let r = events
            .iter()
            .find_map(|e| match &**e {
                Event::Receipt(r) => Some(r),
                _ => None,
            })
            .expect("expected Receipt");
        assert_eq!(r.message_ids, vec!["SOLO-MSG"]);
        assert_eq!(r.r#type, ReceiptType::Delivered);
    }

    /// Verify that receipt nodes use JID-typed attrs for `to` and `participant`,
    /// ensuring the NodeValue::Jid optimization is not accidentally regressed to to_string.
    #[test]
    fn test_receipt_node_uses_jid_attrs() {
        use wacore_binary::NodeValue;

        let chat_jid: Jid = "120363021033254949@g.us"
            .parse()
            .expect("test JID should be valid");
        let sender_jid: Jid = "15551234567@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");

        // Build a group receipt node using the same pattern as send_delivery_receipt
        let node = NodeBuilder::new("receipt")
            .attr("id", "MSG-123")
            .attr("to", chat_jid.clone())
            .attr("participant", sender_jid.clone())
            .build();

        // "to" must be stored as NodeValue::Jid, not NodeValue::String
        let to_attr = node.attrs.get("to").expect("receipt must have 'to' attr");
        assert!(
            matches!(to_attr, NodeValue::Jid(_)),
            "'to' attr should be JID-typed, got: {:?}",
            to_attr
        );
        assert_eq!(to_attr.to_jid().unwrap(), chat_jid);

        // "participant" must also be JID-typed
        let participant_attr = node
            .attrs
            .get("participant")
            .expect("group receipt must have 'participant' attr");
        assert!(
            matches!(participant_attr, NodeValue::Jid(_)),
            "'participant' attr should be JID-typed, got: {:?}",
            participant_attr
        );
        assert_eq!(participant_attr.to_jid().unwrap(), sender_jid);
    }
}
