//! LID-PN (Linked ID to Phone Number) Cache
//!
//! This module implements a cache for mapping between WhatsApp's Linked IDs (LIDs)
//! and phone numbers. The cache is used for Signal address resolution - WhatsApp Web
//! uses LID-based addresses for Signal sessions when available.
//!
//! The cache maintains bidirectional mappings:
//! - LID -> Entry (for getting phone number from LID)
//! - Phone Number -> Entry (for getting LID from phone number)
//!
//! When multiple LIDs exist for the same phone number (rare), the most recent one
//! (by `created_at` timestamp) is considered "current".
//!
//! Both maps are unbounded by default and never expire. WA Web
//! (`WAWebLidPnCache`) uses plain `Map`s, so a mapping learned at startup or
//! via usync stays available for every subsequent Signal-address resolution.
//! A custom `CacheEntryConfig` can impose a capacity bound if memory pressure
//! requires it, accepting the trade-off that capacity-LRU eviction silently
//! downgrades Signal addresses to `@c.us`.

use std::sync::Arc;

use wacore_binary::CompactString;

use crate::cache_config::{CacheConfig, CacheEntryConfig};
use crate::cache_store::TypedCache;
pub use wacore::types::{LearningSource, LidPnEntry};

/// Namespaces used in the custom store.
const NS_LID: &str = "lid_pn_by_lid";
const NS_PN: &str = "lid_pn_by_pn";

/// Cache for LID to Phone Number mappings.
///
/// This cache maintains bidirectional mappings between LIDs and phone numbers,
/// similar to WhatsApp Web's LidPnCache class. It provides fast lookups for
/// Signal address resolution.
///
/// The cache is thread-safe and can be shared across async tasks.
pub struct LidPnCache {
    /// LID -> Entry mapping. `Arc` so the hot `get_*` lookups clone a refcount,
    /// not the entry's strings; only the owned-`LidPnEntry` accessors deep-clone.
    /// Keys are `Arc<str>` sharing the entry's own allocations — each identifier
    /// is stored once per mapping, not once as key and again inside the entry
    /// (this cache is unbounded by design, so per-entry bytes compound).
    lid_to_entry: TypedCache<Arc<str>, Arc<LidPnEntry>>,
    /// Phone number -> Entry mapping (stores the most recent LID for that PN)
    pn_to_entry: TypedCache<Arc<str>, Arc<LidPnEntry>>,
    /// PN -> the LID this process durably persisted for it. Lets the learn hot
    /// path skip a re-persist without swallowing the first live persist of a
    /// mapping an offline replay only warmed in memory. Keyed by the pair so a
    /// remap (or a stale detached write that marks late) only matches its own
    /// LID, never a newer un-persisted one. `Arc<str>` value: the hot-path check
    /// clones a refcount and compares in place, no payload copy. In-memory only;
    /// cold after restart just replays the idempotent upsert.
    persisted: TypedCache<Arc<str>, Arc<str>>,
}

impl Default for LidPnCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LidPnCache {
    /// Create a new empty cache with default settings (no time-based expiry,
    /// effectively unbounded — matches `WAWebLidPnCache`).
    pub fn new() -> Self {
        Self::with_config(&CacheConfig::default().lid_pn_cache, None)
    }

    /// Create a new cache with custom configuration (uses time_to_idle semantics
    /// when a timeout is set; default config has none).
    ///
    /// When `store` is `Some`, both internal maps use the custom backend.
    /// When `store` is `None`, both maps use in-process moka caches.
    pub fn with_config(
        config: &CacheEntryConfig,
        store: Option<Arc<dyn wacore::store::CacheStore>>,
    ) -> Self {
        match store {
            Some(s) => Self {
                lid_to_entry: TypedCache::from_store(s.clone(), NS_LID, config.timeout),
                pn_to_entry: TypedCache::from_store(s, NS_PN, config.timeout),
                // Always in-memory: tracks per-process persist state, never the
                // mapping itself, so it must not go through the shared store.
                persisted: TypedCache::from_moka(config.build_with_tti()),
            },
            None => Self {
                lid_to_entry: TypedCache::from_moka(config.build_with_tti()),
                pn_to_entry: TypedCache::from_moka(config.build_with_tti()),
                persisted: TypedCache::from_moka(config.build_with_tti()),
            },
        }
    }

    /// Returns approximate entry counts for the LID and PN maps.
    #[cfg(feature = "debug-diagnostics")]
    pub fn entry_counts(&self) -> (u64, u64) {
        (
            self.lid_to_entry.entry_count(),
            self.pn_to_entry.entry_count(),
        )
    }

    /// Get the current LID for a phone number.
    ///
    /// Returns the LID user part if a mapping exists, None otherwise.
    /// The cache holds `Arc<LidPnEntry>`; return the LID user as an (inline for
    /// typical ~15-digit LIDs) `CompactString` instead of deep-cloning a `String`.
    pub async fn get_current_lid(&self, phone: &str) -> Option<CompactString> {
        self.pn_to_entry
            .get(phone)
            .await
            .map(|e| CompactString::from(&*e.lid))
    }

    /// Whether the learn fast path can skip re-recording `phone <-> lid`: the
    /// pair is durably persisted AND resolvable in BOTH cache directions.
    /// Requiring both directions means skipping never leaves the reverse
    /// (LID -> PN) lookup cold under a configured eviction; that lookup
    /// (`get_phone_number`, e.g. in PN->LID session migration) has no backend
    /// fallback. All checks compare in place over the Arc-shared values, no
    /// payload clone.
    pub(crate) async fn can_skip_relearn(&self, phone: &str, lid: &str) -> bool {
        self.is_persisted(phone, lid).await
            && self
                .pn_to_entry
                .get(phone)
                .await
                .is_some_and(|e| &*e.lid == lid)
            && self
                .lid_to_entry
                .get(lid)
                .await
                .is_some_and(|e| &*e.phone_number == phone)
    }

    /// Get the phone number for a LID.
    ///
    /// Returns the phone number user part if a mapping exists, None otherwise.
    pub async fn get_phone_number(&self, lid: &str) -> Option<String> {
        self.lid_to_entry
            .get(lid)
            .await
            .map(|e| e.phone_number.to_string())
    }

    /// Get the full entry for a LID.
    pub async fn get_entry_by_lid(&self, lid: &str) -> Option<LidPnEntry> {
        self.lid_to_entry.get(lid).await.map(|e| (*e).clone())
    }

    /// Get the full entry for a phone number.
    pub async fn get_entry_by_phone(&self, phone: &str) -> Option<LidPnEntry> {
        self.pn_to_entry.get(phone).await.map(|e| (*e).clone())
    }

    /// Add or update a mapping in the cache.
    ///
    /// For the LID -> Entry map, this always updates.
    /// For the PN -> Entry map, this only updates if the new entry has a
    /// newer or equal `created_at` timestamp (matching WhatsApp Web behavior).
    ///
    /// Note: the get-then-insert on the PN map is not atomic. With external
    /// backends (e.g., Redis), concurrent `add()` calls for the same phone
    /// number can race. This is acceptable because the cache is best-effort
    /// and backed by persistent storage for correctness.
    pub async fn add(&self, entry: &LidPnEntry) {
        let should_update_pn = match self.pn_to_entry.get(&*entry.phone_number).await {
            Some(existing) => existing.created_at <= entry.created_at,
            None => true,
        };

        // One shared copy of the entry; the keys clone the entry's own
        // Arc<str> allocations, so each identifier lives once per mapping.
        let shared = Arc::new(entry.clone());
        self.lid_to_entry
            .insert(shared.lid.clone(), Arc::clone(&shared))
            .await;

        // Update PN -> Entry map (only if newer or equal timestamp)
        if should_update_pn {
            self.pn_to_entry
                .insert(shared.phone_number.clone(), shared)
                .await;
        }
    }

    /// Whether this process has durably persisted exactly `phone -> lid`.
    /// Pair-keyed so a remap, or a stale detached write marking late, never
    /// reports a newer un-persisted LID as persisted. Clones no payload.
    pub(crate) async fn is_persisted(&self, phone: &str, lid: &str) -> bool {
        self.persisted
            .get(phone)
            .await
            .is_some_and(|stored| stored.as_ref() == lid)
    }

    pub(crate) async fn mark_persisted(&self, phone: &str, lid: &str) {
        self.persisted
            .insert(Arc::from(phone), Arc::from(lid))
            .await;
    }

    /// Warm up the cache with entries from persistent storage.
    ///
    /// This should be called during client initialization to populate
    /// the cache from the database.
    pub async fn warm_up(&self, entries: impl IntoIterator<Item = LidPnEntry>) {
        let start = wacore::time::Instant::now();
        let mut count = 0;

        for entry in entries {
            self.add(&entry).await;
            count += 1;
        }

        log::debug!(
            "LID-PN cache warmed up with {} entries in {:?}",
            count,
            start.elapsed()
        );
    }

    /// Clear all entries from the cache.
    ///
    /// Awaits the actual clear operation on custom backends (unlike
    /// `invalidate_all` which is fire-and-forget).
    pub async fn clear(&self) {
        self.lid_to_entry.clear().await;
        self.pn_to_entry.clear().await;
        self.persisted.clear().await;
    }

    /// Get the number of LID entries in the cache.
    pub async fn lid_count(&self) -> u64 {
        self.lid_to_entry.run_pending_tasks().await;
        self.lid_to_entry.entry_count()
    }

    /// Get the number of phone number entries in the cache.
    pub async fn pn_count(&self) -> u64 {
        self.pn_to_entry.run_pending_tasks().await;
        self.pn_to_entry.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_operations() {
        let cache = LidPnCache::new();

        // Initially empty
        assert!(cache.get_current_lid("559980000001").await.is_none());
        assert!(cache.get_phone_number("100000012345678").await.is_none());

        // Add a mapping
        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(&entry).await;

        // Should be retrievable both ways
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678")
        );
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_timestamp_conflict_resolution() {
        let cache = LidPnCache::new();

        // Add old mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(&old_entry).await;

        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678")
        );

        // Add newer mapping for same phone (different LID)
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(&new_entry).await;

        // Should return the newer LID for PN lookup
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000087654321")
        );

        // Both LIDs should still be in the LID -> Entry map
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
        assert_eq!(
            cache.get_phone_number("100000087654321").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_older_entry_does_not_override() {
        let cache = LidPnCache::new();

        // Add new mapping first
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(&new_entry).await;

        // Try to add older mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(&old_entry).await;

        // PN -> LID should still return the newer one
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000087654321")
        );
    }

    #[tokio::test]
    async fn test_warm_up() {
        let cache = LidPnCache::new();

        let entries = vec![
            LidPnEntry::with_timestamp(
                "lid1".to_string(),
                "pn1".to_string(),
                1,
                LearningSource::Other,
            ),
            LidPnEntry::with_timestamp(
                "lid2".to_string(),
                "pn2".to_string(),
                2,
                LearningSource::Usync,
            ),
            LidPnEntry::with_timestamp(
                "lid3".to_string(),
                "pn3".to_string(),
                3,
                LearningSource::PeerPnMessage,
            ),
        ];

        cache.warm_up(entries).await;

        assert_eq!(cache.lid_count().await, 3);
        assert_eq!(cache.pn_count().await, 3);

        assert_eq!(cache.get_current_lid("pn1").await.as_deref(), Some("lid1"));
        assert_eq!(cache.get_current_lid("pn2").await.as_deref(), Some("lid2"));
        assert_eq!(cache.get_current_lid("pn3").await.as_deref(), Some("lid3"));
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = LidPnCache::new();

        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(&entry).await;

        assert_eq!(cache.lid_count().await, 1);
        assert_eq!(cache.pn_count().await, 1);

        cache.clear().await;
        assert_eq!(cache.lid_count().await, 0);
        assert_eq!(cache.pn_count().await, 0);
        assert!(cache.get_current_lid("559980000001").await.is_none());
    }

    #[tokio::test]
    async fn persisted_marker_is_pair_specific() {
        let cache = LidPnCache::new();
        let pn = "559980000099";
        let (lid_a, lid_b) = ("100000000000001", "100000000000002");
        assert!(!cache.is_persisted(pn, lid_a).await);
        cache.mark_persisted(pn, lid_a).await;
        assert!(cache.is_persisted(pn, lid_a).await);
        // A remap to a new LID is not persisted (even a stale mark of the old
        // LID never satisfies the new pair), so it re-persists.
        assert!(!cache.is_persisted(pn, lid_b).await);
    }

    #[tokio::test]
    async fn skip_requires_both_cache_directions() {
        let cache = LidPnCache::new();
        let (pn, lid) = ("559980000099", "100000000000001");
        cache
            .add(&LidPnEntry::new(
                lid.to_string(),
                pn.to_string(),
                LearningSource::PeerPnMessage,
            ))
            .await;
        cache.mark_persisted(pn, lid).await;
        assert!(cache.can_skip_relearn(pn, lid).await);

        // Evict the reverse (LID -> PN) entry: the fast path must stop skipping
        // so the next live message re-warms it (get_phone_number has no fallback).
        cache.lid_to_entry.invalidate(lid).await;
        assert!(
            !cache.can_skip_relearn(pn, lid).await,
            "skip must require the reverse map, not just PN -> LID"
        );
    }

    #[test]
    fn test_learning_source_serialization() {
        let sources = [
            (LearningSource::Usync, "usync"),
            (LearningSource::PeerPnMessage, "peer_pn_message"),
            (LearningSource::PeerLidMessage, "peer_lid_message"),
            (LearningSource::RecipientLatestLid, "recipient_latest_lid"),
            (LearningSource::MigrationSyncLatest, "migration_sync_latest"),
            (LearningSource::MigrationSyncOld, "migration_sync_old"),
            (LearningSource::BlocklistActive, "blocklist_active"),
            (LearningSource::BlocklistInactive, "blocklist_inactive"),
            (LearningSource::Pairing, "pairing"),
            (LearningSource::DeviceNotification, "device_notification"),
            (LearningSource::Other, "other"),
        ];

        for (source, expected_str) in sources {
            assert_eq!(source.as_str(), expected_str);
            assert_eq!(LearningSource::parse(expected_str), source);
        }

        // Unknown string should map to Other
        assert_eq!(LearningSource::parse("unknown"), LearningSource::Other);
    }
}
