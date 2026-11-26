//! Pluggable cache storage trait.
//!
//! Implementations of [`CacheStore`] can back any of the client's data caches
//! (group metadata, device lists, LID-PN mappings, etc.). The default behaviour
//! uses in-process caches; a Redis, Memcached, or any other implementation
//! can be plugged in via the client crate's `CacheConfig`.

use async_trait::async_trait;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct CacheError(#[source] Box<dyn std::error::Error + Send + Sync>);

impl CacheError {
    pub fn new(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self(Box::new(error))
    }

    pub fn from_boxed(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self(error)
    }
}

impl From<std::io::Error> for CacheError {
    fn from(error: std::io::Error) -> Self {
        Self::new(error)
    }
}

pub type CacheResult<T> = Result<T, CacheError>;

/// Backend trait for pluggable cache storage.
///
/// Keys and values are opaque strings / bytes — the typed cache wrapper in
/// `whatsapp-rust` handles serialization via serde.
///
/// # Namespaces
///
/// Each logical cache uses a unique namespace string (e.g., `"group"`,
/// `"device"`, `"lid_pn_by_lid"`). Implementations should use this to
/// partition keys — for example, a Redis implementation might prefix keys
/// as `{namespace}:{key}`.
///
/// # Error handling
///
/// Cache operations are best-effort. The client falls back gracefully when
/// cache reads fail (treats as miss) and logs warnings on write failures.
/// Implementations should still return errors for observability.
///
/// Identity replacement additionally requires confirmation that stale device
/// registry entries were removed. A deletion or verification-read failure can
/// reject that operation before new trust or device lists are published.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait CacheStore: Send + Sync + 'static {
    /// Retrieve a cached value by namespace and key.
    async fn get(&self, namespace: &str, key: &str) -> CacheResult<Option<Vec<u8>>>;

    /// Store a value with an optional TTL.
    ///
    /// When `ttl` is `None`, the entry should persist until explicitly deleted
    /// or evicted by the implementation's own policy.
    async fn set(
        &self,
        namespace: &str,
        key: &str,
        value: &[u8],
        ttl: Option<Duration>,
    ) -> CacheResult<()>;

    /// Delete a single key from the given namespace.
    async fn delete(&self, namespace: &str, key: &str) -> CacheResult<()>;

    /// Delete all keys in a namespace.
    async fn clear(&self, namespace: &str) -> CacheResult<()>;

    /// Return the approximate number of entries in a namespace.
    ///
    /// Used only for diagnostics. Implementations that cannot cheaply
    /// report counts should return `Ok(0)`.
    async fn entry_count(&self, _namespace: &str) -> CacheResult<u64> {
        Ok(0)
    }
}
