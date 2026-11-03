// SPDX-License-Identifier: MIT
//! Async host adapters around the synchronous Signal engine. Returned outbound
//! messages remain subject to the host's durable reservation publication gate.
pub use crate::core::*;
pub use crate::store::sender_key_name::SenderKeyName;
mod components;
pub mod error;
pub use components::*;
#[cfg(feature = "legacy-session-interop")]
mod legacy;
#[cfg(feature = "legacy-session-interop")]
pub use legacy::*;
mod messages;
mod operations;
mod records;
mod stores;
pub use error::SignalProtocolError;
pub use messages::*;
pub use operations::*;
pub use records::*;
pub use stores::*;
pub mod consts {
    pub const SENDER_CHAIN_RESERVATION_BATCH: u32 = 64;
    pub const MAX_FORWARD_JUMPS: u32 = 2000;
    pub const MAX_RESERVATION_FAST_FORWARD: u32 = 2000;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Sending,
    Receiving,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IdentityChange {
    #[default]
    NewOrUnchanged,
    ReplacedExisting,
    ReplacedExistingWithKeys {
        previous: IdentityKey,
        current: IdentityKey,
    },
}
impl IdentityChange {
    pub const fn from_changed(changed: bool) -> Self {
        if changed {
            Self::ReplacedExisting
        } else {
            Self::NewOrUnchanged
        }
    }
    pub fn keys(&self) -> Option<(IdentityKey, IdentityKey)> {
        match self {
            Self::ReplacedExistingWithKeys { previous, current } => Some((*previous, *current)),
            _ => None,
        }
    }
    pub fn is_changed(&self) -> bool {
        !matches!(self, Self::NewOrUnchanged)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsePQRatchet {
    Yes,
    No,
}
macro_rules! key_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub u32);
        impl From<u32> for $name {
            fn from(v: u32) -> Self {
                Self(v)
            }
        }
        impl From<$name> for u32 {
            fn from(v: $name) -> Self {
                v.0
            }
        }
        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}
key_id!(PreKeyId);
key_id!(SignedPreKeyId);

/// An opaque millisecond count supplied by the caller, with no clock access.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);
impl Timestamp {
    pub const fn from_epoch_millis(value: u64) -> Self {
        Self(value)
    }
    pub const fn epoch_millis(self) -> u64 {
        self.0
    }
}
impl From<Timestamp> for u64 {
    fn from(value: Timestamp) -> Self {
        value.0
    }
}

#[cfg(test)]
mod tests;
