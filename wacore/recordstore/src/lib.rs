// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Fenced, sealed record storage for wacore Signal state.
//!
//! A database adapter implements two small traits, [`RecordStore`] and
//! [`LeaseStore`], over opaque byte records. [`RecordSignalStore`] builds the
//! full [`wacore::store::traits::SignalStore`] on top of them, so Signal
//! crypto keeps using unmodified wacore code.
//!
//! Two guarantees come from this layer rather than from each adapter:
//!
//! - **Fencing.** Every write carries the [`Fence`] of the lease that
//!   authorised it. The adapter applies the batch only while that lease
//!   generation is still current, in the same transaction. A node that lost
//!   its lease can never overwrite state written by its successor.
//! - **Sealing.** Every value is sealed by a [`Sealer`] before it reaches the
//!   adapter. The authenticated data binds scope, namespace and key, so a
//!   record cannot be moved to another slot. Record keys (Signal addresses and
//!   prekey IDs) stay readable so adapters can index and scan them.
//!
//! Sealing does not detect rollback: an adapter that serves an older sealed
//! value for the same slot passes authentication.

mod memory;
mod record;
mod seal;
mod signal;

#[cfg(any(test, feature = "conformance"))]
pub mod conformance;

pub use memory::MemoryRecordStore;
pub use record::{
    Fence, FenceLost, Lease, LeaseStore, Namespace, RecordExists, RecordStore, WriteOp,
    is_fence_lost, is_record_exists,
};
pub use seal::{AesGcmSealer, SealError, Sealer};
pub use signal::RecordSignalStore;
