//
// Copyright 2023 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

mod address;
// Not exporting the members because they have overly-generic names.
pub mod curve;

pub use address::{AddressBuf, DeviceId, ProtocolAddress};

/// A wire byte that names no variant of the enum it was converted into.
///
/// The `repr`-based `TryFrom` impls in this crate are hand-written rather than
/// derived, so their error type lives here instead of being a third-party one
/// leaked through the public API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no variant with discriminant {value}")]
pub struct UnknownDiscriminant<T: std::fmt::Debug + std::fmt::Display> {
    pub value: T,
}
