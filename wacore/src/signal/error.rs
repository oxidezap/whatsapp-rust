// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use crate::libsignal::protocol::SignalProtocolError;
use crate::store::error::StoreError;

#[cfg(feature = "signal-operations")]
#[derive(Debug, thiserror::Error)]
#[error("a Signal operation was interrupted; reconcile state and open a fresh authority")]
struct InterruptedOperation;

#[cfg(feature = "signal-operations")]
pub(crate) fn interrupted_operation() -> OpsError {
    StoreError::Database(Box::new(InterruptedOperation)).into()
}

#[cfg(feature = "signal-operations")]
fn state_is_refused(error: &StoreError) -> bool {
    crate::store::transaction::is_authority_lost(error)
        || matches!(error, StoreError::Database(source) if source.is::<InterruptedOperation>())
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OpsError {
    #[error("Signal write authority is no longer valid")]
    AuthorityLost,
    #[error("signal protocol error")]
    Signal(#[from] SignalProtocolError),
    #[error("store error")]
    Store(#[from] StoreError),
    /// The caller sent something this operation cannot accept.
    #[error("invalid input: {0}")]
    InvalidInput(String),
    /// Pairing was refused. `code` and `text` are the values WhatsApp Web
    /// answers the server with for the same failure.
    #[error("pairing refused: {code} {text}")]
    Pairing { code: u16, text: &'static str },
    /// Building an outbound stanza failed, including resolver (device list,
    /// prekey fetch) failures reported by the WhatsApp client.
    #[error("send failed: {0}")]
    Send(String),
    /// A peer's prekey bundle did not verify against its pinned account
    /// identity; see [`crate::signal::BundleRejection`].
    #[error("prekey bundle refused: {0}")]
    BundleRefused(BundleRejection),
    /// The scope has no device keys yet; call `SignalOps::create`.
    #[error("device keys are not initialised for this scope")]
    NotInitialised,
}

impl OpsError {
    /// The writer was invalidated or cancellation left a write outcome unknown.
    /// Reconcile persisted state and obtain a fresh authority before resuming.
    pub fn is_authority_lost(&self) -> bool {
        match self {
            OpsError::AuthorityLost => true,
            #[cfg(feature = "signal-operations")]
            OpsError::Store(e) => state_is_refused(e),
            #[cfg(feature = "signal-operations")]
            OpsError::Signal(SignalProtocolError::BackendError(_, source)) => source
                .downcast_ref::<StoreError>()
                .is_some_and(state_is_refused),
            _ => false,
        }
    }
}

/// Why a bundle was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BundleRejection {
    #[error("the account's primary identity changed and was not accepted")]
    IdentityChanged,
    #[error("a companion bundle carries no device identity")]
    MissingDeviceIdentity,
    #[error("no pinned account identity to verify the companion against")]
    NoAccountIdentity,
    #[error("the companion's device identity does not verify against the account")]
    InvalidDeviceIdentity,
}
