// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use async_trait::async_trait;
use std::sync::Arc;
use wacore::store::error::{Result, StoreError};

/// Record families. The wire tag is persisted by adapters; never renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Namespace {
    Identity,
    Session,
    PreKey,
    SignedPreKey,
    SenderKey,
    /// The local device's own keys (identity, registration ID, ADV secret,
    /// current signed prekey). Read and written whole by the Signal service.
    Device,
    /// Decrypted results kept until the app confirms delivery, so a
    /// redelivered ciphertext returns the same result instead of failing.
    DecryptBuffer,
    /// Messages this device sent, kept so a retry receipt can re-encrypt them.
    SentMessage,
    /// Group member devices confirmed to hold this device's sender key.
    SenderKeyDevices,
}

impl Namespace {
    pub const ALL: [Namespace; 9] = [
        Namespace::Identity,
        Namespace::Session,
        Namespace::PreKey,
        Namespace::SignedPreKey,
        Namespace::SenderKey,
        Namespace::Device,
        Namespace::DecryptBuffer,
        Namespace::SentMessage,
        Namespace::SenderKeyDevices,
    ];

    /// Stable persisted name.
    pub const fn tag(self) -> &'static str {
        match self {
            Namespace::Identity => "identity",
            Namespace::Session => "session",
            Namespace::PreKey => "prekey",
            Namespace::SignedPreKey => "signed_prekey",
            Namespace::SenderKey => "sender_key",
            Namespace::Device => "device",
            Namespace::DecryptBuffer => "decrypt_buffer",
            Namespace::SentMessage => "sent_message",
            Namespace::SenderKeyDevices => "sender_key_devices",
        }
    }
}

/// Authority to write one scope. A scope is one linked device's Signal state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fence {
    pub scope: Arc<str>,
    pub generation: u64,
}

/// A held lease. `fence` stays constant across renewals by the same holder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lease {
    pub fence: Fence,
    pub holder: Arc<str>,
    pub expires_at_ms: u64,
}

/// One change inside an atomic [`RecordStore::write`] batch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOp {
    /// Insert or replace.
    Put {
        ns: Namespace,
        key: String,
        value: Vec<u8>,
    },
    /// Replace only if the key exists; a missing key is a no-op, never an
    /// insert. Used where resurrecting a deleted record would be a bug.
    Update {
        ns: Namespace,
        key: String,
        value: Vec<u8>,
    },
    /// Remove; a missing key is a no-op.
    Delete { ns: Namespace, key: String },
    /// Insert only if the key is absent. If it exists, the whole batch fails
    /// with [`RecordExists`] and nothing is applied.
    Insert {
        ns: Namespace,
        key: String,
        value: Vec<u8>,
    },
}

/// The write was refused because `fence` no longer matches the scope's lease.
#[derive(Debug, Clone, thiserror::Error)]
#[error("lease for scope {scope} moved past generation {generation}")]
pub struct FenceLost {
    pub scope: Arc<str>,
    pub generation: u64,
}

impl FenceLost {
    pub fn into_store_error(self) -> StoreError {
        StoreError::Database(Box::new(self))
    }
}

/// An [`WriteOp::Insert`] found its key already present; nothing was applied.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{ns:?} record {key} already exists")]
pub struct RecordExists {
    pub ns: Namespace,
    pub key: String,
}

impl RecordExists {
    pub fn into_store_error(self) -> StoreError {
        StoreError::Database(Box::new(self))
    }
}

/// Whether `error` (or any error it wraps) is a [`RecordExists`].
pub fn is_record_exists(error: &StoreError) -> bool {
    let mut layer: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = layer {
        if current.is::<RecordExists>() {
            return true;
        }
        layer = current.source();
    }
    false
}

/// Whether `error` (or any error it wraps) is a [`FenceLost`]. A node that
/// sees this must stop serving the scope; retrying cannot succeed.
pub fn is_fence_lost(error: &StoreError) -> bool {
    let mut layer: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = layer {
        if current.is::<FenceLost>() {
            return true;
        }
        layer = current.source();
    }
    false
}

/// Byte-level storage an adapter implements. Values are already sealed.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait RecordStore: Send + Sync {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>>;

    /// Returns only keys that exist, in any order.
    async fn get_many(
        &self,
        scope: &str,
        ns: Namespace,
        keys: &[&str],
    ) -> Result<Vec<(String, Vec<u8>)>> {
        let mut found = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(value) = self.get(scope, ns, key).await? {
                found.push(((*key).to_owned(), value));
            }
        }
        Ok(found)
    }

    /// Keys starting with `prefix`, ascending by byte order, at most `limit`.
    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>>;

    /// Apply every op or none, and only while `fence.generation` is the
    /// scope's current lease generation. Otherwise return
    /// [`FenceLost::into_store_error`] and apply nothing. An `Insert` whose key
    /// exists fails the batch with [`RecordExists::into_store_error`].
    /// An `Insert` followed by a `Delete` of the same key checks absence
    /// atomically without retaining a record after a successful batch.
    ///
    /// The returned future may be dropped after the backend applied the batch.
    /// A caller that did not see the result must re-read the affected records
    /// before retrying, because a blind retry of an `Insert` fails with
    /// [`RecordExists`].
    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()>;
}

/// Per-scope single-writer leases. Time is supplied by the caller so the
/// contract holds on targets without a system clock.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait LeaseStore: Send + Sync {
    /// Take the scope if it is free, expired, or already held by `holder`.
    /// A new holder gets a strictly higher generation; the same holder keeps
    /// its generation. Returns `None` while another holder's lease is live.
    async fn acquire(
        &self,
        scope: &str,
        holder: &str,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<Option<Lease>>;

    /// Extend `lease`. Returns `None` if the scope moved to another
    /// generation, in which case the caller lost it.
    async fn renew(&self, lease: &Lease, now_ms: u64, ttl_ms: u64) -> Result<Option<Lease>>;

    /// Give the scope up early. Releasing a lease that was already lost is a
    /// no-op. The generation is kept so a later holder still advances it.
    async fn release(&self, lease: &Lease) -> Result<()>;
}
