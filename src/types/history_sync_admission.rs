/// Metadata available to a history-sync admission policy.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistorySyncMetadata<'a> {
    /// Protocol history-sync type, using the core's stable numeric representation.
    pub sync_type: Option<i32>,
    pub chunk_order: Option<u32>,
    pub progress: Option<u32>,
    /// Sender-declared file length. The value is not validated by the core.
    pub file_length: Option<u64>,
    pub inline_payload_len: Option<usize>,
    pub peer_data_request_session_id: Option<&'a str>,
}

/// Result returned by [`HistorySyncAdmission::decide`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistorySyncDecision {
    Accept,
    /// Send a `hist_sync` receipt that permanently acknowledges the chunk and
    /// prevents retry. Do not use this for transient load shedding.
    RejectAndAcknowledge,
}

/// Synchronous, opt-in policy for admitting inbound history-sync notifications.
///
/// The policy runs before a notification creates history-sync activity or enters
/// the major-sync queue. It must make a fast local decision without I/O or
/// blocking. When no policy is registered, every notification is accepted and
/// the receive path only checks an immutable optional policy.
pub trait HistorySyncAdmission: wacore::sync_marker::MaybeSendSync {
    /// Decide whether this notification should enter history-sync processing.
    fn decide(&self, metadata: &HistorySyncMetadata<'_>) -> HistorySyncDecision;
}
