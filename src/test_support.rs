//! Opt-in native fixtures. No connection or call readiness flags are exposed.
#![doc = include_str!("../agent_docs/call_test_support.md")]

mod session;
pub(crate) use session::seed_peer_session;

#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
mod call;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub use call::{CallFixture, PendingOffer};
