//! Portable cache implementation used by the client and its benchmarks.
//!
//! Concrete cache types are exposed only with `bench-harness` for repository benchmarks.

mod implementation;

pub(crate) use implementation::SyncTtlCache;

#[cfg(feature = "bench-harness")]
#[doc(hidden)]
pub use implementation::{PortableCache, PortableCacheBuilder, SeededCacheBuilder};

#[cfg(not(feature = "bench-harness"))]
pub(crate) use implementation::PortableCache;
