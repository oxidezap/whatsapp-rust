//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

//! Interfaces in [traits] and reference implementations in [inmem] for various mutable stores.

#![warn(missing_docs)]

mod traits;
#[cfg(test)]
mod traits_hook_tests;

pub use traits::{
    Direction, IdentityChange, IdentityKeyStore, PreKeyStore, ProtocolStore, SenderKeyStore,
    SessionCheckout, SessionCheckoutKey, SessionCheckoutStoreResult, SessionStore,
    SignedPreKeyStore, has_session, is_trusted_identity, save_identity,
};
