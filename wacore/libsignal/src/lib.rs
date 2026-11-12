// SPDX-License-Identifier: MIT
//! Runtime adapters for independently implemented Signal transitions.
#![forbid(unsafe_code)]
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod bench_support;
pub mod core;
pub mod crypto;
mod curve;
pub mod engine;
pub mod protocol;
pub mod store;
