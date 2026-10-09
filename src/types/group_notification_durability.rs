use anyhow::Result;
use std::sync::Arc;
use wacore_binary::OwnedNodeRef;

/// A durable gate for each complete inbound `notification type="w:gp2"` stanza.
///
/// Awaited once, before stanza interceptors, group cache changes, sender-key
/// rotation, typed group events, and the generic transport ACK. `Ok(())` means the entire original
/// notification (all actions) has committed. `Err` prevents those effects and
/// cancels the ACK; the stanza remains handled, without an unknown-stanza NACK.
/// No hook preserves existing processing. This also gates `groups_dirty` and
/// malformed/unknown group actions so the raw envelope is not lost by parsing.
/// With a hook, transport admissions for the same group share its inbound
/// message lane through capture and effects. Other groups and transport
/// responses remain independent. Do not wait inside the hook for a later
/// inbound message or notification on that same group lane.
///
/// Persist `node.backing_bytes()` verbatim: unpacked decoded node bytes, not a
/// network frame, JSON, re-encoded node, or serialized `GroupUpdate`. Compute a
/// provenance hash over these exact bytes. The `id` attribute is optional and
/// only scoped by the authenticated account/session and source group; never
/// invent an ID or use parsed timestamp defaults (`now`) for identity. A repeat
/// scoped ID with changed bytes requires conflict handling rather than silent
/// deduplication. Without an ID, identical bytes cannot distinguish redelivery
/// from a distinct identical notification; retain this ambiguity explicitly.
///
/// Withholding ACK permits possible redelivery but does not guarantee it.
/// A crash after commit and before ACK can repeat this callback: implementations
/// must atomically persist raw evidence and an outbox, be idempotent, and return
/// only after durable commit. This hook does not replay local effects or make
/// detached/ordered event delivery durable; drive downstream work from outbox.
/// If the connection is retired while capture is pending, the completed
/// commit is retained by the host but stale effects and ACK are withheld.
/// Shutdown may cancel the callback; treat cancellation as an indeterminate
/// commit and reconcile persisted evidence before retrying.
/// RawNode diagnostic observers may run before this gate. Do not treat those
/// observers or the typed event stream as the durable source.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait GroupNotificationDurabilityHook: wacore::sync_marker::MaybeSendSync {
    /// Commit the full envelope before returning `Ok(())` to admit processing.
    ///
    /// Return an error when the host cannot establish a durable commit. A
    /// successful return does not promise effects or an ACK: the connection
    /// may have retired while this future was pending. Implementations must
    /// therefore publish downstream work atomically with their durable record.
    async fn on_notification(&self, node: Arc<OwnedNodeRef>) -> Result<()>;
}
