//! Unified cache type that dispatches to moka or the portable implementation.
//!
//! Selection is both feature- and target-aware: [`Cache`] is `moka::future::Cache`
//! only when the `moka-cache` feature is on AND the target is not wasm32. On wasm32
//! (or with `moka-cache` off) it is [`PortableCache`](crate::portable_cache::PortableCache).
//! moka is thread-based (it pulls crossbeam/uuid) and does not compile on
//! wasm32-unknown-unknown, so a defaults-based wasm32 build falls back here rather than
//! failing deep inside moka's dependency tree.

#[cfg(all(feature = "moka-cache", not(target_arch = "wasm32")))]
mod inner {
    pub type Cache<K, V> = moka::future::Cache<K, V>;
}

#[cfg(any(not(feature = "moka-cache"), target_arch = "wasm32"))]
mod inner {
    pub type Cache<K, V> = crate::portable_cache::PortableCache<K, V>;
}

pub use inner::Cache;
