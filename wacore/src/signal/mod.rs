//! Signal backend contracts, enabled by `signal-backend`.
//!
//! Consumers implement [`SignalBackend`] over their own key custody, storage
//! and transport. Native implementations are thread-safe; WASM implementations
//! may own JavaScript handles directly. [`ReceiveOutcome::DeliveredExternally`]
//! represents completed delivery without returning plaintext to the caller.
//!
//! `signal-operations` additionally enables `SignalOps`, a stored implementation
//! using the existing Signal and stanza-building primitives. Its storage
//! extension requires atomic writes but prescribes no database, encryption at
//! rest or lease implementation. Both features are disabled by default.
//!
//! The stored implementation pins account identities on first use, verifies
//! companion ADV identities and requires explicit approval for identity changes.
//! It still trusts the caller's device lists, group membership and PN/LID maps.
//! It does not check device revocation against signed key-index lists. Dropping
//! a mutation retires its authority until stored state is reconciled.
//!
//! These contracts do not configure the Tokio Client's internal Signal paths.

mod backend;
mod error;
mod resolver;
mod types;

pub use backend::{ReceiveOutcome, SignalBackend};
pub use error::{BundleRejection, OpsError};
pub use resolver::{FetchedBundle, SignalResolver, VerifiablePrekeys};
pub use types::*;

#[cfg(feature = "signal-operations")]
mod device;
#[cfg(feature = "signal-operations")]
mod guard;
#[cfg(feature = "signal-operations")]
mod inbound;
#[cfg(feature = "signal-operations")]
mod ops;
#[cfg(feature = "signal-operations")]
mod outbound;
#[cfg(feature = "signal-operations")]
mod stores;
#[cfg(feature = "signal-operations")]
mod trust;
#[cfg(feature = "signal-operations")]
pub use device::DeviceKeys;
#[cfg(feature = "signal-operations")]
pub use ops::{AuthorityKey, SignalOperationStore, SignalOps};
