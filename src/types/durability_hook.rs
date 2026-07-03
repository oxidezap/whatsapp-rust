use crate::client::Client;
use anyhow::Result;
use std::sync::Arc;
pub use wacore::types::events::InboundMessage;

/// Hook invoked for every decrypted inbound user message before it is
/// acknowledged to the server, turning the consumer from at-most-once into
/// at-least-once delivery.
///
/// The transport ack tells the server to drop the message from its offline
/// queue. By default the SDK acks as soon as a message is decrypted, so a crash
/// (or a failed DB write) before the consumer persists the message loses it for
/// good. When a hook is registered, the ack is deferred until the hook returns
/// `Ok`: the decrypted messages are buffered durably first, the hook runs, and
/// only on success are their acks sent and the buffer cleared. On `Err` (or a
/// crash) the messages stay unacked and the server redelivers them on the next
/// connect, where the hook runs again from the buffered copies.
///
/// This is at-least-once, not exactly-once: a crash after the consumer commits
/// but before the ack lands replays the message, so the hook MUST be idempotent.
/// A failed batch is redelivered whole, so a partially-applied batch commit
/// must also be safe to re-run. Deduplicate by the message source AND id —
/// `(info.source.chat, info.source.sender, info.id)` — not `info.id` alone:
/// stanza ids are only unique within a `(chat, sender)`, so two chats can
/// reuse the same id.
///
/// Durable replay across process crashes requires a backend that implements the
/// `ProtocolStore` pending-inbound methods (the bundled `SqliteStore` does).
/// With a backend that does not, the hook still runs and still gates the ack for
/// the live attempt, but a crash mid-commit cannot be replayed.
///
/// The hook is awaited inside the receive pipeline, so a slow hook backpressures
/// inbound processing (the same trade-off as whatsmeow's synchronous ack). Do
/// not perform blocking client operations for a sender present in the batch
/// (e.g. a synchronous reply) — that can deadlock against the per-sender Signal
/// lock held while a 1:1 message is processed; persist and return, and spawn
/// any reply.
///
/// Scope and known limitations:
/// - Covers end-to-end encrypted messages (1:1 and group). Newsletter / broadcast
///   channel messages are not encrypted and are acked on their own path, so the
///   hook does not gate them.
/// - If the durable buffer write itself fails (e.g. disk full, after retries),
///   the acks are suppressed, but if the process does not crash the Signal
///   ratchet still advances and those messages degrade to at-most-once on their
///   next redelivery (they can no longer be decrypted, and there is no buffered
///   copy to replay). The guarantee holds whenever the buffer write succeeds.
/// - On a redelivery replay the `info` is re-parsed from the stanza, so a few
///   fields derived during the first dispatch (the ephemeral timer, encrypted
///   comment threading) may be absent. The `message` body is always the original.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait InboundDurabilityHook: wacore::sync_marker::MaybeSendSync {
    /// Durably commit the whole batch, all-or-nothing, in slice order (e.g. one
    /// multi-row INSERT transaction). Return `Ok(())` only after the commit is
    /// durable; the SDK then acks every message in the batch. Return `Err` to
    /// suppress all their acks and have the server redeliver them.
    ///
    /// Live messages arrive as batches of one. During the offline drain the
    /// SDK accumulates and commits per batch (WA Web's MessageProcessorCache
    /// granularity), so one round-trip covers the lot.
    /// [`Event::Messages`](wacore::types::events::Event::Messages) then
    /// carries the exact same items: what this method committed is what event
    /// consumers observe.
    async fn on_messages(&self, client: Arc<Client>, batch: &[InboundMessage]) -> Result<()>;
}
