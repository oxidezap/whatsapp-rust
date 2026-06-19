//! The client's in-process cache type.
//!
//! Backed by [`PortableCache`](crate::portable_cache::PortableCache): a
//! runtime-agnostic cache (capacity + TTL/TTI eviction, single-flight
//! `get_with`) that builds on every target, including wasm32.

pub use crate::portable_cache::PortableCache as Cache;
