//! Conditional `Send + Sync` bound.
//!
//! Native builds drive the client across multi-threaded receive lanes, so any
//! trait object stored on it must be `Send + Sync`. wasm32 is single-threaded
//! and its callbacks may hold `!Send` JS handles (see
//! [`crate::msg_secret::OriginalMessageResolver`]), so the bound is dropped
//! there. The blanket impls keep this transparent: on native every
//! `Send + Sync` type already satisfies it, on wasm every type does.

#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSendSync: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync + ?Sized> MaybeSendSync for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSendSync {}
#[cfg(target_arch = "wasm32")]
impl<T: ?Sized> MaybeSendSync for T {}
