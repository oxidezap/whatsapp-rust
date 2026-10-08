//! LID/phone identity records and the client's internal cache.
//!
//! Identity records remain public. The cache implementation is exposed only with
//! `bench-harness` for repository benchmarks.

mod implementation;

pub(crate) use implementation::LidPnMutationGuard;

pub use wacore::types::{LearningSource, LidPnEntry};

#[cfg(feature = "bench-harness")]
#[doc(hidden)]
pub use implementation::{LidPnCache, LidPnMemory};

#[cfg(not(feature = "bench-harness"))]
pub(crate) use implementation::LidPnCache;
