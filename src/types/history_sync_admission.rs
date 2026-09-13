/// Metadata available to a history-sync admission policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySyncMetadata {
    /// Protocol history-sync type, using the core's stable numeric representation.
    pub sync_type: Option<i32>,
    pub chunk_order: Option<u32>,
    pub progress: Option<u32>,
    pub file_length: Option<u64>,
    pub has_inline_payload: bool,
    pub peer_data_request_session_id: Option<String>,
}

/// Result returned by [`HistorySyncAdmission::decide`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistorySyncDecision {
    Accept,
    Reject,
}

/// Synchronous, opt-in policy for admitting inbound history-sync notifications.
///
/// The policy runs before a notification creates history-sync activity or enters
/// the major-sync queue. It must make a fast local decision without I/O or
/// blocking. When no policy is registered, every notification is accepted and
/// the receive path only performs a single [`std::sync::OnceLock::get`].
pub trait HistorySyncAdmission: wacore::sync_marker::MaybeSendSync {
    /// Decide whether this notification should enter history-sync processing.
    fn decide(&self, metadata: &HistorySyncMetadata) -> HistorySyncDecision;
}
