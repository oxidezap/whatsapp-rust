use crate::client::Client;
use anyhow::Result;
use std::sync::Arc;
pub use wacore::types::events::InboundMessage;
use waproto::whatsapp as wa;

/// Consumer commit barrier for supported decrypted inbound messages.
///
/// For ordinary encrypted 1:1 and group messages, the SDK collects every
/// dispatched payload of a stanza, stores a pending copy, awaits this hook,
/// and attempts its delivery receipt only after `Ok(())`. The message event
/// follows the receipt attempt. During an offline drain, pending writes and
/// Signal persistence are batched before the hook. Live Signal writes remain
/// coalesced independently; this is not a transaction spanning Signal and the
/// consumer's store.
///
/// Buffer-write and hook failures retain the admitted plaintext in this Client
/// across connection resets. Resident failures are retried while the client is
/// running. A duplicate cannot bypass a resident copy just because its pending
/// database row is absent. Corrupt or conflicting pending records are preserved
/// and withhold receipts until repaired; uncommitted rows never expire
/// automatically. Multipart records use a versioned envelope; the reader also
/// accepts legacy single-message records, but older SDKs cannot read the new
/// multipart format. Existing rows from a previous process are replayed when a
/// corresponding inbound delivery reaches the replay path, not scanned at startup.
/// Group replay reads all original sender keys for that chat/id and filters the
/// established device-less participant identity without merging PN and LID.
/// If one recorded payload sequence contains all matching rows, replay reuses
/// that sequence and its repeated parts. Corrupt or conflicting matching rows
/// fail closed. Partial retries whose ciphertexts cannot prove the retained
/// occurrence order withhold commit until a complete sequence proves it.
/// Fresh parts waiting for that proof remain in this Client; after a restart,
/// another fully decoded delivery may be needed. Receipt suppression does not
/// guarantee that the server provides one.
/// Only rows read for the
/// successful commit (and rows it wrote) are removed; unrelated participants,
/// chats, message ids and backend devices are preserved.
///
/// Receipt suppression does not guarantee another server delivery. If the
/// process stops or the Client is dropped before a pending copy is persisted,
/// memory retention cannot provide crash durability. A consumer must make its
/// own commit durable before returning `Ok(())`.
///
/// The builder checks the backend's individual `ProtocolStore` pending-inbound
/// store/read/delete and participant lookup operations, rejecting unsupported backends with
/// [`ClientBuilderError::UnsupportedDurabilityBackend`](crate::ClientBuilderError::UnsupportedDurabilityBackend).
/// Custom batched overrides must preserve those operations' semantics; the
/// construction probe does not certify an arbitrary batch implementation.
///
/// History capture is configured independently through [`HistorySyncCaptureHook`].
/// Registering this hook alone does not change history receipt ordering.
///
/// Retries can repeat a successful consumer commit whose receipt or cleanup
/// failed, and batch boundaries can change. Make commits idempotent by source
/// and id — `(info.source.chat, info.source.sender, info.id)` — while preserving
/// the ordered payloads and their multiplicity: one stanza can contain several
/// equal parts with the same id. For group/broadcast authors, device-qualified
/// and device-less spellings identify the same message, as in the event dispatch
/// gate; PN and LID remain separate namespaces. Original metadata and stored
/// keys are preserved. A partial batch commit must be safe to retry.
///
/// Admission counts queued, processing and retained stanzas together: at most
/// 400 stanzas and a 4 MiB budget of original decoded-frame lengths. One larger
/// supported frame can occupy an otherwise empty budget. These are SDK limits,
/// not exact decoded-heap measurements. Exhaustion ends the connection without
/// decrypting or acknowledging the rejected stanza; the read loop never waits
/// for a hook to release capacity. Each connection retains the existing 64-slot
/// live limit and per-chat receive serialization. Retries may follow newer
/// deliveries, and an entered hook can outlive its connection. Persistent storage
/// capacity across process restarts is the backend/operator's responsibility;
/// the store interface does not scan pending rows at startup or impose a quota.
///
/// The builder probes pending-inbound store, exact read, participant-candidate
/// read and delete operations. Custom backends must implement
/// [`ProtocolStore::get_pending_inbound_for_message`](crate::store::traits::ProtocolStore::get_pending_inbound_for_message)
/// in addition to the existing buffer methods. Unsupported backends are rejected with
/// [`ClientBuilderError::UnsupportedDurabilityBackend`](crate::ClientBuilderError::UnsupportedDurabilityBackend).
/// Custom batch overrides must preserve their semantics. History capture is
/// configured independently through [`HistorySyncCaptureHook`]; this hook alone
/// does not change history receipt ordering.
///
/// A slow hook occupies a processing slot. Do not wait for inbound work from the
/// same chat inside the hook. Persist and return, then schedule dependent work.
/// Shutdown stops scheduling recovery retries but does not abort or join an
/// already-entered hook. Use [`Client::shutdown_signal`] if the hook needs to
/// observe shutdown, and make partial writes safe against cancellation.
///
/// Scope: newsletter/channel messages and PDO placeholder recoveries use their
/// existing separate paths and are not gated by this hook. On replay from a
/// persisted row, stanza metadata is re-parsed; derived fields such as ephemeral
/// timers or encrypted-comment threading may be absent. The stored message
/// payloads are reused without a parallel metadata schema.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait InboundDurabilityHook: wacore::sync_marker::MaybeSendSync {
    /// Commit the whole batch durably before returning `Ok(())`. An error
    /// suppresses its receipts and leaves the pending copies available to retry.
    /// Live calls contain all dispatched parts of one stanza; offline calls can
    /// contain several stanzas. Successful commits feed the same items to
    /// [`Event::Messages`](wacore::types::events::Event::Messages).
    async fn on_messages(&self, client: Arc<Client>, batch: &[InboundMessage]) -> Result<()>;
}

/// Optional capture of history-sync bytes before attempting their receipt.
///
/// Register with [`crate::ClientBuilder::with_history_sync_capture_hook`] or
/// [`crate::bot::BotBuilder::with_history_sync_capture_hook`]. This hook does not
/// enable message durability or require pending-inbound storage. Consumers
/// migrating from `InboundDurabilityHook::on_history_sync` must implement this
/// trait and register it separately, even when one object implements both hooks.
///
/// ```
/// use std::sync::Arc;
/// use whatsapp_rust::{ClientBuilder, HistorySyncCaptureHook};
///
/// fn capture_history(builder: ClientBuilder, capture: Arc<dyn HistorySyncCaptureHook>) -> ClientBuilder {
///     builder.with_history_sync_capture_hook_arc(capture)
/// }
/// ```
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait HistorySyncCaptureHook: wacore::sync_marker::MaybeSendSync {
    /// Durably capture one history-sync chunk before its `hist_sync` receipt
    /// is sent. `compressed` is the chunk exactly as the phone uploaded it
    /// (zlib-compressed `HistorySync`), whether it arrived inline or as a
    /// downloaded blob.
    ///
    /// Return `Ok(())` only after the capture is durable. The SDK awaits this
    /// method before attempting the receipt. Download failures, hook errors,
    /// and cancellation before this method returns leave the chunk without a
    /// `hist_sync` receipt. The SDK does not retry the hook or persist a replay
    /// buffer for history chunks; withholding a receipt does not itself
    /// guarantee that the phone will redeliver the chunk.
    ///
    /// Duplicate notifications can invoke this hook again, including after a
    /// successful capture whose receipt failed to send. Make capture idempotent
    /// by `message_id`. Downloaded bytes are decrypted but still compressed;
    /// capture runs before decompression and protobuf validation.
    ///
    /// History tasks can call this hook concurrently. A slow hook holds a history
    /// worker slot. Shutdown does not join these detached tasks or cancel an
    /// already-running hook; use [`Client::shutdown_signal`] if the capture needs
    /// to observe shutdown, and make partial writes safe against cancellation.
    ///
    /// Chunks the SDK does not process (history sync skipped, or rejected by the
    /// admission policy) are acknowledged without calling the hook. Without a
    /// capture hook the receipt is attempted before download, including when only
    /// an [`InboundDurabilityHook`] is registered. There is no default capture.
    async fn on_history_sync(
        &self,
        client: Arc<Client>,
        message_id: &str,
        sync_type: Option<wa::message::HistorySyncType>,
        compressed: &[u8],
    ) -> Result<()>;
}
