//! Public download contracts for native and browser hosts.
#[allow(dead_code)] // Compile-only future and platform contracts.
pub mod platform;

#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
mod native;
