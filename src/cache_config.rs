use std::fmt::Display;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::Cache;
use serde::{Serialize, de::DeserializeOwned};

use crate::cache_store::TypedCache;
pub use wacore::msg_secret::{MsgSecretPolicy, MsgSecretRetention, OriginalMessageResolver};
pub use wacore::store::cache::CacheStore;

/// Configuration for a single cache instance.
///
/// Controls the expiry timeout and maximum capacity of a moka cache.
/// The `timeout` field is used as either TTL (`build_with_ttl`) or TTI
/// (`build_with_tti`) depending on which builder method is called.
/// Set `timeout` to `None` to disable time-based expiry (entries stay until
/// evicted by capacity).
#[derive(Debug, Clone)]
pub struct CacheEntryConfig {
    /// Expiry timeout duration. `None` means no time-based expiry.
    /// Interpreted as TTL or TTI depending on the builder method used.
    pub timeout: Option<Duration>,
    /// Maximum number of entries.
    pub capacity: u64,
}

impl CacheEntryConfig {
    pub fn new(timeout: Option<Duration>, capacity: u64) -> Self {
        Self { timeout, capacity }
    }

    /// Build a Cache using time_to_live semantics.
    pub(crate) fn build_with_ttl<K, V>(&self) -> Cache<K, V>
    where
        K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let mut builder = Cache::builder().max_capacity(self.capacity);
        if let Some(timeout) = self.timeout {
            builder = builder.time_to_live(timeout);
        }
        builder.build()
    }

    /// Build a [`TypedCache`] with TTL semantics, using the custom store if
    /// provided or falling back to an in-process cache.
    pub(crate) fn build_typed_ttl<K, V>(
        &self,
        store: Option<Arc<dyn CacheStore>>,
        namespace: &'static str,
    ) -> TypedCache<K, V>
    where
        K: std::hash::Hash + Eq + Clone + Display + Send + Sync + 'static,
        V: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
    {
        match store {
            Some(s) => TypedCache::from_store(s, namespace, self.timeout),
            None => TypedCache::from_moka(self.build_with_ttl()),
        }
    }

    /// Build a Cache using time_to_idle semantics.
    pub(crate) fn build_with_tti<K, V>(&self) -> Cache<K, V>
    where
        K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        let mut builder = Cache::builder().max_capacity(self.capacity);
        if let Some(timeout) = self.timeout {
            builder = builder.time_to_idle(timeout);
        }
        builder.build()
    }
}

/// Per-cache custom store overrides.
///
/// Each field is an optional [`CacheStore`] for that specific cache. When
/// `None`, the default in-process moka cache is used.
///
/// # Example — group and device registry on Redis
///
/// ```rust,ignore
/// let redis = Arc::new(MyRedisCacheStore::new("redis://localhost:6379"));
/// let config = CacheConfig {
///     cache_stores: CacheStores {
///         group_cache: Some(redis.clone()),
///         device_registry_cache: Some(redis.clone()),
///         ..Default::default()
///     },
///     ..Default::default()
/// };
/// ```
#[derive(Default, Clone)]
pub struct CacheStores {
    /// Custom store for group metadata cache.
    pub group_cache: Option<Arc<dyn CacheStore>>,
    /// Custom store for device registry cache.
    pub device_registry_cache: Option<Arc<dyn CacheStore>>,
    /// Custom store for LID-PN bidirectional mapping cache.
    pub lid_pn_cache: Option<Arc<dyn CacheStore>>,
}

impl CacheStores {
    /// Set the same [`CacheStore`] for all pluggable caches at once.
    ///
    /// Coordination caches (`session_locks`, `chat_lanes`, etc.) and the
    /// signal write-behind cache always remain in-process regardless of this
    /// setting.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let stores = CacheStores::all(Arc::new(MyRedisCacheStore::new("redis://localhost:6379")));
    /// ```
    pub fn all(store: Arc<dyn CacheStore>) -> Self {
        Self {
            group_cache: Some(store.clone()),
            device_registry_cache: Some(store.clone()),
            lid_pn_cache: Some(store),
        }
    }
}

/// Configuration for all client caches and resource pools.
///
/// All fields default to WhatsApp Web behavior. Use `..Default::default()` to
/// override only specific settings.
///
/// # Example — tune TTL/capacity
///
/// ```rust,ignore
/// use whatsapp_rust::{CacheConfig, CacheEntryConfig};
/// use std::time::Duration;
///
/// let config = CacheConfig {
///     group_cache: CacheEntryConfig::new(None, 1_000), // no TTL
///     ..Default::default()
/// };
/// ```
///
/// # Example — Redis for group and device registry caches
///
/// ```rust,ignore
/// use std::sync::Arc;
/// use whatsapp_rust::{CacheConfig, CacheStores};
///
/// let redis = Arc::new(MyRedisCacheStore::new("redis://localhost:6379"));
/// let config = CacheConfig {
///     cache_stores: CacheStores {
///         group_cache: Some(redis.clone()),
///         device_registry_cache: Some(redis.clone()),
///         ..Default::default()
///     },
///     ..Default::default()
/// };
/// ```
#[derive(Clone)]
pub struct CacheConfig {
    /// Group metadata cache (time_to_live). Default: 1h TTL, 250 entries.
    pub group_cache: CacheEntryConfig,
    /// Device registry cache (time_to_live). Default: 1h TTL, 5000 entries
    /// (holds a large group's per-member device set; a near-max group is ~1024).
    pub device_registry_cache: CacheEntryConfig,
    /// LID-to-phone cache. WAWebLidPnCache uses plain Maps with no expiry
    /// and no size cap; evicting a still-valid mapping silently downgrades
    /// Signal addresses to `@c.us`. Default: no timeout, capacity u64::MAX
    /// (effectively unbounded — moka doesn't expose an `unbounded()` builder).
    pub lid_pn_cache: CacheEntryConfig,
    /// Optional L1 in-memory cache for sent messages (retry support).
    /// Default: capacity 0 (disabled — DB-only, matching WA Web).
    /// Set capacity > 0 to enable a fast in-memory cache in front of the DB.
    pub recent_messages: CacheEntryConfig,
    /// Message retry counts (time_to_live). Default: 1h TTL, 500 entries.
    /// Long enough that the MAX_DECRYPT_RETRIES cap survives spaced redeliveries.
    pub message_retry_counts: CacheEntryConfig,
    /// Dedup key for `UndecryptableMessage` dispatch so a server resend of
    /// the same id does not surface a second notification. Default: 5m TTL,
    /// 1000 entries.
    pub undecryptable_dispatched: CacheEntryConfig,
    /// PDO pending requests (time_to_live). Default: 30s TTL, 200 entries.
    pub pdo_pending_requests: CacheEntryConfig,
    /// Sender key device tracking cache (time_to_idle). Default: 1h TTI, 500 entries.
    /// Caches per-group SKDM distribution state to avoid DB reads on every group send.
    pub sender_key_devices_cache: CacheEntryConfig,
    /// Session-recreate throttle history (time_to_live). Default: 1h TTL, 256
    /// entries. Replaces a global `Mutex<HashMap>` scanned O(n) per retry receipt.
    pub session_recreate_history: CacheEntryConfig,

    // --- Coordination caches (capacity-only, no TTL) ---
    /// Per-device Signal session lock capacity. Default: 10000.
    pub session_locks_capacity: u64,
    /// Per-chat lane capacity (combined lock + queue). Default: 5000.
    pub chat_lanes_capacity: u64,

    // --- Sent message DB cleanup ---
    /// TTL in seconds for sent messages in DB before periodic cleanup. Must
    /// outlive retry receipts (which can arrive well after a send) or the retry
    /// is dropped as "not found in cache". The periodic sweep keeps the table
    /// bounded. 0 = no automatic cleanup. Default: 7200 (2 hours).
    pub sent_message_ttl_secs: u64,

    // --- MsgSecret retention ---
    /// How the per-message `messageSecret` store is managed (capture / seed /
    /// prune). Default [`MsgSecretPolicy::Managed`] bounds DB growth: it seeds
    /// only the still-relevant slice of history and prunes by a per-add-on-kind
    /// event-time horizon. Set [`MsgSecretPolicy::Full`] to keep everything
    /// forever, or [`MsgSecretPolicy::Disabled`] to persist nothing and delegate
    /// to [`original_message_resolver`].
    ///
    /// [`original_message_resolver`]: CacheConfig::original_message_resolver
    pub msg_secret_policy: MsgSecretPolicy,
    /// Per-add-on-kind retention horizons applied under `Managed`/`BotOnly`.
    pub msg_secret_retention: MsgSecretRetention,
    /// Whether to seed `messageSecret`s from history-sync blobs. Default `true`.
    ///
    /// Independent of live capture (which `msg_secret_policy` governs): seeding
    /// only matters for add-ons that arrive live after connect yet reference a
    /// parent delivered via history sync — edits of just-pre-pairing messages,
    /// add-options/edits on still-open polls, or replays to a reconnecting
    /// offline device. Headless consumers that only react to new messages can
    /// set this to `false` to skip the pairing-time seed entirely. When `true`,
    /// the policy still filters the seed (age/type under `Managed`, bot-only
    /// under `BotOnly`, everything under `Full`).
    pub seed_msg_secrets_from_history: bool,
    /// Optional app-supplied fallback consulted when an add-on's parent secret
    /// is absent from the store (and its LID/PN alternates). Lets an app that
    /// keeps its own message store own secret retention; required for the
    /// `Disabled` policy to decrypt anything beyond what it has seen live.
    pub original_message_resolver: Option<Arc<dyn OriginalMessageResolver>>,
    /// Bound on each [`original_message_resolver`] call. The resolver runs
    /// inside the per-chat receive lane, so a slow callback would stall that
    /// chat; on timeout the lookup degrades to a miss. Default: 5s.
    ///
    /// [`original_message_resolver`]: CacheConfig::original_message_resolver
    pub msg_secret_resolver_timeout: Duration,

    // --- Custom store overrides ---
    /// Per-cache custom store overrides.
    ///
    /// For each field set to `Some(store)`, the corresponding cache uses that
    /// backend instead of the default in-process moka cache. Fields left as
    /// `None` keep the default moka behaviour.
    ///
    /// Coordination caches (`session_locks`, `chat_lanes`), the signal write-behind
    /// cache, and `pdo_pending_requests` always stay in-process — they hold live Rust
    /// objects (mutexes, channel senders, oneshot senders) that cannot be
    /// serialised to an external store.
    pub cache_stores: CacheStores,
}

impl std::fmt::Debug for CacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheConfig")
            .field("group_cache", &self.group_cache)
            .field("device_registry_cache", &self.device_registry_cache)
            .field("lid_pn_cache", &self.lid_pn_cache)
            .field("recent_messages", &self.recent_messages)
            .field("message_retry_counts", &self.message_retry_counts)
            .field("undecryptable_dispatched", &self.undecryptable_dispatched)
            .field("pdo_pending_requests", &self.pdo_pending_requests)
            .field("sender_key_devices_cache", &self.sender_key_devices_cache)
            .field("session_recreate_history", &self.session_recreate_history)
            .field("session_locks_capacity", &self.session_locks_capacity)
            .field("chat_lanes_capacity", &self.chat_lanes_capacity)
            .field("sent_message_ttl_secs", &self.sent_message_ttl_secs)
            .field("msg_secret_policy", &self.msg_secret_policy)
            .field("msg_secret_retention", &self.msg_secret_retention)
            .field(
                "seed_msg_secrets_from_history",
                &self.seed_msg_secrets_from_history,
            )
            .field(
                "original_message_resolver",
                &self.original_message_resolver.is_some(),
            )
            .field(
                "msg_secret_resolver_timeout",
                &self.msg_secret_resolver_timeout,
            )
            .field(
                "cache_stores.group_cache",
                &self.cache_stores.group_cache.is_some(),
            )
            .field(
                "cache_stores.device_registry_cache",
                &self.cache_stores.device_registry_cache.is_some(),
            )
            .field(
                "cache_stores.lid_pn_cache",
                &self.cache_stores.lid_pn_cache.is_some(),
            )
            .finish()
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        let one_hour = Some(Duration::from_secs(3600));
        let five_min = Some(Duration::from_secs(300));

        Self {
            group_cache: CacheEntryConfig::new(one_hour, 250),
            // One entry per group member; 1000 was below a near-max (~1024) group,
            // so large-group warm sends thrashed to the serial per-user DB path.
            device_registry_cache: CacheEntryConfig::new(one_hour, 5_000),
            lid_pn_cache: CacheEntryConfig::new(None, u64::MAX),
            recent_messages: CacheEntryConfig::new(five_min, 0),
            // 1h so the MAX_DECRYPT_RETRIES cap survives spaced redeliveries; a
            // 5m TTL expired between reconnects so the count never reached the cap.
            message_retry_counts: CacheEntryConfig::new(one_hour, 500),
            undecryptable_dispatched: CacheEntryConfig::new(five_min, 1_000),
            pdo_pending_requests: CacheEntryConfig::new(Some(Duration::from_secs(30)), 200),
            sender_key_devices_cache: CacheEntryConfig::new(one_hour, 500),
            session_recreate_history: CacheEntryConfig::new(one_hour, 256),
            // Coordination caches hold live mutexes/senders; capacity eviction
            // while a reference is held creates a second lock for the same key,
            // breaking serialization. Size generously to avoid eviction pressure.
            session_locks_capacity: 10_000,
            chat_lanes_capacity: 5_000,
            sent_message_ttl_secs: 7200,
            // Bounded by default: seed only the still-relevant slice of history
            // and prune by per-add-on-kind event-time horizons, so the store no
            // longer accumulates a secret for every message forever.
            msg_secret_policy: MsgSecretPolicy::default(),
            msg_secret_retention: MsgSecretRetention::default(),
            seed_msg_secrets_from_history: true,
            original_message_resolver: None,
            msg_secret_resolver_timeout: Duration::from_secs(5),
            cache_stores: CacheStores::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lid_pn_cache_default_is_effectively_unbounded() {
        let cfg = CacheConfig::default();
        assert_eq!(
            cfg.lid_pn_cache.timeout, None,
            "lid_pn_cache must not expire entries by time; WAWebLidPnCache uses plain Maps"
        );
        assert_eq!(
            cfg.lid_pn_cache.capacity,
            u64::MAX,
            "lid_pn_cache must be effectively unbounded; capacity-LRU re-introduces the eviction bug at higher thresholds"
        );
    }
}
