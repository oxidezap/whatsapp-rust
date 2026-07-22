//! The client's in-process cache type.
//!
//! Backed by [`PortableCache`](crate::portable_cache::PortableCache): a
//! runtime-agnostic cache (capacity + TTL/TTI eviction, single-flight
//! `get_with`) that builds on every target, including wasm32.

pub use crate::portable_cache::PortableCache as Cache;

/// Selects whether an operation may use an existing snapshot or must refresh it
/// from its authoritative source before returning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Freshness {
    /// Return a cached snapshot when available and consult the source on a miss.
    #[default]
    CachePreferred,
    /// Consult the source and publish the resulting snapshot without clearing the
    /// previous one first.
    Refresh,
}
