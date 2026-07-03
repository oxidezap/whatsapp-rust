//! Typed cache wrapper that dispatches to either the in-process
//! [`Cache`](crate::cache::Cache) or a custom [`CacheStore`] backend (e.g., Redis).
//!
//! [`TypedCache`] presents the same interface regardless of the backing store.
//! Keys are serialised via [`Display`]; values are serialised with `serde_json`
//! only on the custom-store path — the in-process path has zero extra overhead.

use std::borrow::Borrow;
use std::fmt::Display;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use crate::cache::Cache;
use serde::{Serialize, de::DeserializeOwned};

pub use wacore::store::cache::CacheStore;

// ── Internal storage variant ──────────────────────────────────────────────────

enum Inner<K, V> {
    Local(Cache<K, V>),
    Custom {
        store: Arc<dyn CacheStore>,
        namespace: &'static str,
        ttl: Option<Duration>,
        _marker: PhantomData<fn(K, V)>,
    },
}

// ── TypedCache ─────────────────────────────────────────────────────────────────

/// A cache over `K → V` backed by either the in-process cache or any [`CacheStore`].
///
/// The in-process path has **zero extra overhead** — values are stored in
/// memory without any serialisation.  The custom-store path serialises values
/// with `serde_json` and keys via [`Display`].
pub struct TypedCache<K, V> {
    inner: Inner<K, V>,
}

impl<K, V> TypedCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    /// Wrap an in-process [`Cache`] (zero overhead vs. using the cache directly).
    pub fn from_local(cache: Cache<K, V>) -> Self {
        Self {
            inner: Inner::Local(cache),
        }
    }
}

impl<K, V> TypedCache<K, V>
where
    K: std::hash::Hash + Eq + Clone + Display + Send + Sync + 'static,
    V: Clone + Serialize + DeserializeOwned + Send + Sync + 'static,
{
    /// Create a cache backed by a custom store.
    ///
    /// - `namespace` — unique string for this cache (e.g., `"group"`)
    /// - `ttl` — forwarded to [`CacheStore::set`]; `None` means no expiry
    pub fn from_store(
        store: Arc<dyn CacheStore>,
        namespace: &'static str,
        ttl: Option<Duration>,
    ) -> Self {
        Self {
            inner: Inner::Custom {
                store,
                namespace,
                ttl,
                _marker: PhantomData,
            },
        }
    }

    /// Look up a value.
    ///
    /// Accepts borrowed keys (`&str` for `String`, `&Jid` for `Jid`, etc.)
    /// following the same pattern as [`std::collections::HashMap::get`].
    ///
    /// Cache misses and deserialisation failures both return `None`; the
    /// caller re-fetches from the authoritative source.
    pub async fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq + Display + ?Sized,
    {
        match &self.inner {
            Inner::Local(cache) => cache.get(key).await,
            Inner::Custom {
                store, namespace, ..
            } => {
                let key_str = key.to_string();
                match store.get(namespace, &key_str).await {
                    Ok(Some(bytes)) => serde_json::from_slice(&bytes)
                        .inspect_err(|e| {
                            log::warn!(
                                "TypedCache[{namespace}]: deserialise failed for {key_str}: {e}"
                            );
                        })
                        .ok(),
                    Ok(None) => None,
                    Err(e) => {
                        log::warn!("TypedCache[{namespace}]: get({key_str}) error: {e}");
                        None
                    }
                }
            }
        }
    }

    /// Insert or update a value (takes ownership of key and value).
    pub async fn insert(&self, key: K, value: V) {
        match &self.inner {
            Inner::Local(cache) => cache.insert(key, value).await,
            Inner::Custom {
                store,
                namespace,
                ttl,
                ..
            } => {
                let key_str = key.to_string();
                match serde_json::to_vec(&value) {
                    Ok(bytes) => {
                        if let Err(e) = store.set(namespace, &key_str, &bytes, *ttl).await {
                            log::warn!("TypedCache[{namespace}]: set({key_str}) error: {e}");
                        }
                    }
                    Err(e) => {
                        log::warn!("TypedCache[{namespace}]: serialise failed for {key_str}: {e}");
                    }
                }
            }
        }
    }

    /// Remove a single key.
    ///
    /// Accepts borrowed keys following the same pattern as `get`.
    pub async fn invalidate<Q>(&self, key: &Q)
    where
        K: Borrow<Q>,
        Q: std::hash::Hash + Eq + Display + ?Sized,
    {
        match &self.inner {
            Inner::Local(cache) => cache.invalidate(key).await,
            Inner::Custom {
                store, namespace, ..
            } => {
                let key_str = key.to_string();
                if let Err(e) = store.delete(namespace, &key_str).await {
                    log::warn!("TypedCache[{namespace}]: delete({key_str}) error: {e}");
                }
            }
        }
    }

    /// Remove all entries.
    ///
    /// For the in-process backend this is synchronous.
    /// For the custom backend this spawns a fire-and-forget task via
    /// [`tokio::runtime::Handle::try_current`] (requires `tokio-runtime`
    /// feature) to avoid panicking if called outside a Tokio runtime.
    /// Without `tokio-runtime`, the clear is skipped with a warning.
    pub fn invalidate_all(&self) {
        match &self.inner {
            Inner::Local(cache) => cache.invalidate_all(),
            Inner::Custom {
                store, namespace, ..
            } => {
                let _store = store.clone();
                let _ns = *namespace;
                #[cfg(all(not(target_arch = "wasm32"), feature = "tokio-runtime"))]
                match tokio::runtime::Handle::try_current() {
                    Ok(handle) => {
                        handle.spawn(async move {
                            if let Err(e) = _store.clear(_ns).await {
                                log::warn!("TypedCache[{_ns}]: clear() error: {e}");
                            }
                        });
                    }
                    Err(_) => {
                        log::warn!("TypedCache[{_ns}]: clear() skipped: no runtime");
                    }
                }
                #[cfg(all(not(target_arch = "wasm32"), not(feature = "tokio-runtime")))]
                log::warn!("TypedCache[{_ns}]: clear() skipped: tokio-runtime feature not enabled");
            }
        }
    }

    /// Remove all entries, awaiting completion for custom backends.
    pub async fn clear(&self) {
        match &self.inner {
            Inner::Local(cache) => cache.clear().await,
            Inner::Custom {
                store, namespace, ..
            } => {
                if let Err(e) = store.clear(namespace).await {
                    log::warn!("TypedCache[{namespace}]: clear() error: {e}");
                }
            }
        }
    }

    /// Run any pending internal housekeeping tasks (in-process backend only).
    ///
    /// For the in-process backend this evicts expired entries so a subsequent
    /// [`entry_count`](Self::entry_count) reflects them. For custom backends
    /// this is a no-op.
    pub async fn run_pending_tasks(&self) {
        if let Inner::Local(cache) = &self.inner {
            cache.run_pending_tasks().await;
        }
    }

    /// Iterate the in-process backend's entries. `None` for custom stores,
    /// whose entries live outside this process (memory reports treat them as
    /// zero retained bytes for the same reason).
    pub fn iter_local(&self) -> Option<std::vec::IntoIter<(Arc<K>, V)>> {
        match &self.inner {
            Inner::Local(cache) => Some(cache.iter()),
            Inner::Custom { .. } => None,
        }
    }

    /// Entry count plus estimated retained bytes, summing `per_entry` over the
    /// in-process backend (a reliable by-reference walk — no clones, cannot
    /// degrade to an empty snapshot under write contention). Custom stores
    /// report zero: their entries live outside this process.
    pub async fn memory_stats(
        &self,
        per_entry: impl FnMut(&K, &V) -> usize,
    ) -> wacore::stats::CollectionStats {
        match &self.inner {
            Inner::Local(cache) => cache.memory_stats(per_entry).await,
            Inner::Custom { .. } => wacore::stats::CollectionStats::default(),
        }
    }

    /// Approximate entry count (sync). Returns `0` for custom backends.
    ///
    /// For diagnostics that need custom backend counts, use
    /// [`entry_count_async`](Self::entry_count_async) instead.
    pub fn entry_count(&self) -> u64 {
        match &self.inner {
            Inner::Local(cache) => cache.entry_count(),
            Inner::Custom { .. } => 0,
        }
    }

    /// Approximate entry count, delegating to the custom backend if available.
    pub async fn entry_count_async(&self) -> u64 {
        match &self.inner {
            Inner::Local(cache) => cache.entry_count(),
            Inner::Custom {
                store, namespace, ..
            } => store.entry_count(namespace).await.unwrap_or(0),
        }
    }
}
