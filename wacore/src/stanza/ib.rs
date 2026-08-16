//! `<ib>` (info bulletin) child vocabulary.
//!
//! Reference: WhatsApp Web `WAWebHandleInfoBulletinTypes.INFO_TYPE`, the frozen
//! list its `<ib>` parser branches on.
//!
//! Hand-written rather than generated: whatspec's `notif` document describes the
//! tags a *stanza* arrives under, not the children inside one, and its `srvreq`
//! document carries a parser for `client_expiration` alone. The rest of this
//! vocabulary exists only in WA Web's dispatcher, so there is nothing to bind
//! against -- this is the `schemas_unlisted.rs` situation, not a gap in the
//! emitter.

/// A child of `<ib>`, which is what says what the bulletin is about.
///
/// Open: the server adds bulletin kinds without warning, and one this client
/// does not know is logged and skipped rather than treated as a parse failure.
#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
pub enum InfoBulletinType {
    /// One or more `<dirty>` markers: a cached protocol domain went stale.
    #[wire = "dirty"]
    Dirty,
    /// Edge routing info for the next connect's pre-intro.
    #[wire = "edge_routing"]
    EdgeRouting,
    /// The offline backlog finished draining.
    #[wire = "offline"]
    Offline,
    /// How much backlog is coming, ahead of it being delivered.
    #[wire = "offline_preview"]
    OfflinePreview,
    /// Terms-of-service notice ids.
    #[wire = "tos"]
    Tos,
    /// Per-chat offline watermarks.
    #[wire = "thread_metadata"]
    ThreadMetadata,
    /// The date the server expects to stop accepting this client build.
    #[wire = "client_expiration"]
    ClientExpiration,
    /// The priority slice of the offline backlog finished.
    #[wire = "priority_offline_complete"]
    PriorityOfflineComplete,
    /// A nonce recovered out of band, carrying the use case it belongs to.
    #[wire = "recovery_nonce"]
    RecoveryNonce,
    #[wire_default]
    #[wire = "unknown"]
    Unknown,
}
