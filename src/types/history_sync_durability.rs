use crate::HistorySyncMetadata;
use anyhow::Result;

/// Identity of one self-sent history notification, scoped to the Client account.
/// Stanza IDs alone are not unique: deduplicate on all three fields, and keep
/// each account's records separate. Device components are preserved verbatim.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySyncKey {
    pub chat: String,
    pub sender: String,
    pub id: String,
}

impl HistorySyncKey {
    pub fn new(chat: impl Into<String>, sender: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            chat: chat.into(),
            sender: sender.into(),
            id: id.into(),
        }
    }
}

/// Opt-in durable notification recovery, independent of [`crate::HistorySyncCaptureHook`].
///
/// The SDK first stores the decrypted protocol-message carrier in its bounded
/// pending-inbound journal, then awaits `on_notification` and admission to the
/// bounded history work queue before acknowledging the message carrier. Queue
/// saturation or capture failure keeps the journal for server redelivery without
/// repeating Signal decryption. Downloads run on the history worker, sharing its
/// two-task limit with explicit replay; the `hist_sync` receipt additionally
/// waits for `on_compressed_chunk` and successful parsing.
///
/// After the carrier ACK, the host owns recovery if the worker fails or the
/// process stops. Keep the notification and schedule retries through
/// [`crate::Client::replay_history_sync_notification`] until the chunk is durable;
/// do not depend on server redelivery or the SDK journal surviving its cleanup.
///
/// Implementations must commit before returning `Ok`, be idempotent, and reject
/// an existing key with different bytes. Callbacks can repeat after failure,
/// cancellation, reconnect or a lost receipt. Store metadata together with the
/// bytes; fields are untrusted sender claims, not validated file properties.
/// Notifications contain media keys and possibly inline history: protect them
/// as account data, and do not log their contents.
///
/// Only self-sent notifications are captured. Skip/admission rejection still
/// intentionally acknowledges without calling either hook. Registration enables
/// the same bounded inbound journal/admission used by [`crate::InboundDurabilityHook`]
/// for encrypted messages, even without that hook; unsupported backends fail
/// construction. No live-message consumer hook is installed implicitly.
///
/// `on_notification` backpressures the receive commit pipeline; chunk callbacks
/// run on the history worker or a host replay. Do not call blocking client
/// operations from a hook. Failed pending writes remain in
/// bounded resident retention for this Client; surviving process termination
/// requires either the journal or the host notification write to have committed.
/// Durability strength depends on the selected backend/storage configuration.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait HistorySyncDurabilityHook: wacore::sync_marker::MaybeSendSync {
    /// Retain the complete encoded `HistorySyncNotification`, including inline
    /// payload, before attempting download, processing, or a positive receipt.
    async fn on_notification(
        &self,
        key: &HistorySyncKey,
        metadata: &HistorySyncMetadata<'_>,
        notification: &[u8],
    ) -> Result<()>;

    /// Retain the decrypted, zlib-compressed chunk before its receipt. Parsing
    /// follows this callback; malformed chunks remain unacknowledged.
    async fn on_compressed_chunk(
        &self,
        key: &HistorySyncKey,
        metadata: &HistorySyncMetadata<'_>,
        compressed: &[u8],
    ) -> Result<()>;
}
