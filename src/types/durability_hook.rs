use crate::client::Client;
use crate::types::message::MessageInfo;
use anyhow::Result;
use std::sync::Arc;
use waproto::whatsapp as wa;

/// Hook invoked for every decrypted inbound user message before it is
/// acknowledged to the server, turning the consumer from at-most-once into
/// at-least-once delivery.
///
/// The transport ack tells the server to drop the message from its offline
/// queue. By default the SDK acks as soon as a message is decrypted, so a crash
/// (or a failed DB write) before the consumer persists the message loses it for
/// good. When a hook is registered, the ack is deferred until the hook returns
/// `Ok`: the decrypted message is buffered durably first, the hook runs, and
/// only on success is the ack sent and the buffer cleared. On `Err` (or a
/// crash) the message stays unacked and the server redelivers it on the next
/// connect, where the hook runs again from the buffered copy.
///
/// This is at-least-once, not exactly-once: a crash after the consumer commits
/// but before the ack lands replays the message, so the hook MUST be idempotent.
/// Deduplicate by the message source AND id — `(info.source.chat,
/// info.source.sender, info.id)` — not `info.id` alone: stanza ids are only
/// unique within a `(chat, sender)`, so two chats can reuse the same id.
///
/// Durable replay across process crashes requires a backend that implements the
/// `ProtocolStore` pending-inbound methods (the bundled `SqliteStore` does).
/// With a backend that does not, the hook still runs and still gates the ack for
/// the live attempt, but a crash mid-commit cannot be replayed.
///
/// The hook is awaited inside the receive pipeline, so a slow hook backpressures
/// inbound processing (the same trade-off as whatsmeow's synchronous ack). Do
/// not perform blocking client operations for the same sender inside it (e.g. a
/// synchronous reply) — that can deadlock against the per-sender Signal lock held
/// while a 1:1 message is processed; persist and return, and spawn any reply.
///
/// Scope and known limitations:
/// - Covers end-to-end encrypted messages (1:1 and group). Newsletter / broadcast
///   channel messages are not encrypted and are acked on their own path, so the
///   hook does not gate them.
/// - If the durable buffer write itself fails (e.g. disk full, after retries),
///   the ack is suppressed, but if the process does not crash the Signal ratchet
///   still advances and that one message degrades to at-most-once on its next
///   redelivery (it can no longer be decrypted, and there is no buffered copy to
///   replay). The guarantee holds whenever the buffer write succeeds.
/// - On a redelivery replay the `info` is re-parsed from the stanza, so a few
///   fields derived during the first dispatch (the ephemeral timer, encrypted
///   comment threading) may be absent. The `message` body is always the original.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait InboundDurabilityHook: wacore::sync_marker::MaybeSendSync {
    /// Durably commit `message` (e.g. INSERT into your DB, enqueue to a broker).
    /// Return `Ok(())` only after the commit is durable; the SDK then acks the
    /// message. Return `Err` to suppress the ack and have the server redeliver.
    async fn on_message(
        &self,
        client: Arc<Client>,
        info: &MessageInfo,
        message: &wa::Message,
    ) -> Result<()>;
}
