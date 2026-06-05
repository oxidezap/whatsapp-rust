//! Post-decrypt dispatch: event emission, acks and delivery receipts.

use super::*;

impl Client {
    /// Dispatches a successfully parsed message to the event bus and sends a delivery receipt.
    pub(crate) async fn dispatch_parsed_message(
        self: &Arc<Self>,
        msg: wa::Message,
        info: &Arc<MessageInfo>,
    ) {
        use wacore::proto_helpers::MessageExt;

        let mut info = Arc::clone(info);
        if info.ephemeral_expiration.is_none()
            && msg.get_base_message().get_ephemeral_expiration().is_some()
        {
            Arc::make_mut(&mut info).ephemeral_expiration =
                msg.get_base_message().get_ephemeral_expiration();
        }

        // Keep this ordered with dispatch; add-on messages can immediately
        // reference the secret from the stanza just processed.
        self.maybe_capture_inbound_msg_secret(&msg, &info).await;
        let dispatch_msg = self
            .maybe_decrypt_secret_encrypted_message(&msg, &info)
            .await
            .unwrap_or(msg);
        self.ack_received_message(&info);

        self.core
            .event_bus
            .dispatch(Event::Message(Arc::new(dispatch_msg), info));
    }

    /// Acknowledge a received message so the server drops it from the offline
    /// queue: a delivery receipt when applicable (incl. the `type="sender"`
    /// receipt for own-account self-fanouts), else a transport ack. status is
    /// acked by the `should_ack` gate, newsletters/empty ids need nothing here.
    pub(crate) fn ack_received_message(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if info.id.is_empty() || info.source.chat.is_newsletter() {
            return;
        }
        // WA Web `sendAggregateReceipts`: for a DELIVERY where the chat is NOT
        // a bot but the author IS a bot (a bot reply inside a group), it emits
        // a bare `<ack class="message">` via `sendBotInvokeResponseAcks`, not a
        // `<receipt>`. A 1:1 bot chat keeps the normal receipt (chat.isBot() →
        // the branch's `v` is false). Our transport ack is that bare
        // `<ack class="message">` (group form carries `participant`).
        if info.source.is_bot_authored_non_bot_chat() {
            self.spawn_message_ack(info);
            return;
        }
        if Self::should_send_delivery_receipt(info) {
            self.spawn_delivery_receipt(info);
        } else if !info.source.chat.is_status_broadcast() {
            self.spawn_message_ack(info);
        }
    }

    /// Spawn a delivery receipt, tracked so `disconnect()` can flush it (issue #571).
    fn spawn_delivery_receipt(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        let client = self.clone();
        let info = Arc::clone(info);
        self.outbound_flush.spawn(&*self.runtime, async move {
            client.send_delivery_receipt(&info).await;
        });
    }
}
