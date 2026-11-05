// SPDX-License-Identifier: MIT
#[path = "core/address.rs"]
mod address;
#[path = "core/keys.rs"]
mod keys;
pub use address::*;
pub use keys::*;

/// Native backends may be shared by executor workers. Browser WASM backends
/// may own local JS handles; threaded WASM is not covered by this contract.
#[cfg(not(target_arch = "wasm32"))]
pub trait ThreadSafe: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> ThreadSafe for T {}
#[cfg(target_arch = "wasm32")]
pub trait ThreadSafe {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> ThreadSafe for T {}
