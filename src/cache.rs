//! Unified cache type backed by moka or [`PortableCache`](crate::portable_cache::PortableCache).
//!
//! The selection below is target-gated, not just feature-gated, because moka is
//! thread-based (crossbeam/uuid) and can't build on wasm32: a defaults-based wasm32
//! build must fall back to PortableCache instead of failing deep inside moka's deps.

#[cfg(all(feature = "moka-cache", not(target_arch = "wasm32")))]
mod inner {
    pub type Cache<K, V> = moka::future::Cache<K, V>;
}

#[cfg(any(not(feature = "moka-cache"), target_arch = "wasm32"))]
mod inner {
    pub type Cache<K, V> = crate::portable_cache::PortableCache<K, V>;
}

pub use inner::Cache;
