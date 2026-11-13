//! Atomic storage for operations that update several Signal records together.
//!
//! Implementations choose their database, encryption and writer coordination.
//! Existing [`SignalStore`] implementations need not implement this extension.
//! A staged view must provide read-your-writes and commit all changes atomically.
//! Concurrent views must serialize overlapping records or detect conflicts.
//! Dropping a commit future may leave its outcome unknown. The caller must
//! reconcile stored state before resuming mutations under a fresh authority.

use super::error::Result as StoreResult;
use super::traits::SignalStore;
use async_trait::async_trait;
use std::sync::Arc;

/// Record families used by the optional Signal operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SignalRecord {
    Identity,
    Session,
    PreKey,
    SignedPreKey,
    SenderKey,
    /// The local device's own keys (identity, registration ID, ADV secret,
    /// current signed prekey). Read and written together.
    Device,
    /// Decrypted results kept until the app confirms delivery, so a
    /// redelivered ciphertext returns the same result instead of failing.
    DecryptBuffer,
    /// Messages this device sent, kept so a retry receipt can re-encrypt them.
    SentMessage,
    /// Group member devices confirmed to hold this device's sender key.
    SenderKeyDevices,
    /// Pinned identity keys of peer accounts, the anchor that companion
    /// devices' ADV signatures are verified against.
    AccountIdentity,
}

impl SignalRecord {
    pub const ALL: [SignalRecord; 10] = [
        SignalRecord::Identity,
        SignalRecord::Session,
        SignalRecord::PreKey,
        SignalRecord::SignedPreKey,
        SignalRecord::SenderKey,
        SignalRecord::Device,
        SignalRecord::DecryptBuffer,
        SignalRecord::SentMessage,
        SignalRecord::SenderKeyDevices,
        SignalRecord::AccountIdentity,
    ];

    /// Stable persisted name.
    pub const fn tag(self) -> &'static str {
        match self {
            SignalRecord::Identity => "identity",
            SignalRecord::Session => "session",
            SignalRecord::PreKey => "prekey",
            SignalRecord::SignedPreKey => "signed_prekey",
            SignalRecord::SenderKey => "sender_key",
            SignalRecord::Device => "device",
            SignalRecord::DecryptBuffer => "decrypt_buffer",
            SignalRecord::SentMessage => "sent_message",
            SignalRecord::SenderKeyDevices => "sender_key_devices",
            SignalRecord::AccountIdentity => "account_identity",
        }
    }
}

/// Storage Signal operations need beyond [`SignalStore`]: the device key
/// blob, its own records (decrypt buffer, sent messages), and staged views
/// whose writes commit as one batch.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignalOperationStore: SignalStore + Sized + 'static {
    async fn load_device_keys(&self) -> StoreResult<Option<Vec<u8>>>;
    async fn save_device_keys(&self, blob: &[u8]) -> StoreResult<()>;
    /// Write the first device key blob; fails with `RecordExists` if one is
    /// already stored.
    async fn create_device_keys(&self, blob: &[u8]) -> StoreResult<()>;
    /// A view whose writes are held until [`Self::commit`].
    fn staged(&self) -> Self;
    /// Apply a staged view's writes as one atomic batch.
    async fn commit(&self) -> StoreResult<()>;
    async fn load_aux(&self, ns: SignalRecord, key: &str) -> StoreResult<Option<Vec<u8>>>;
    async fn put_aux(&self, ns: SignalRecord, key: &str, value: &[u8]) -> StoreResult<()>;
    /// Exclusive inserts used to publish durable prekey uploads atomically.
    async fn insert_aux(&self, ns: SignalRecord, key: &str, value: &[u8]) -> StoreResult<()>;
    async fn insert_prekeys_batch(&self, keys: &[(u32, bytes::Bytes)]) -> StoreResult<()>;
    async fn delete_aux(&self, ns: SignalRecord, keys: &[&str]) -> StoreResult<()>;
    async fn scan_aux(&self, ns: SignalRecord, prefix: &str) -> StoreResult<Vec<String>>;
    /// Addresses with a stored session ([`SignalRecord::Session`]) or identity
    /// ([`SignalRecord::Identity`]) starting with `prefix`.
    async fn scan_peer_addresses(&self, ns: SignalRecord, prefix: &str)
    -> StoreResult<Vec<String>>;
    /// Identifies the backend and write authority this store writes under.
    /// Instances with equal keys in one process share operation locks and caches.
    fn authority_key(&self) -> AuthorityKey;
}

/// Identity of one backend, device scope and writer incarnation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AuthorityKey {
    backend: usize,
    scope: Arc<str>,
    generation: u64,
}

impl AuthorityKey {
    /// `backend` must be the shared handle that every store over the same
    /// records holds; it stays alive while an instance using it is open.
    pub fn new<B: ?Sized>(backend: &Arc<B>, scope: impl Into<Arc<str>>, generation: u64) -> Self {
        Self {
            backend: Arc::as_ptr(backend).cast::<()>() as usize,
            scope: scope.into(),
            generation,
        }
    }
}

/// A conditional insert found an existing record. No staged write was applied.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{ns:?} record {key} already exists")]
pub struct RecordExists {
    pub ns: SignalRecord,
    pub key: String,
}

/// The writer can no longer commit under its current authority.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Signal write authority is no longer valid")]
pub struct AuthorityLost;

impl AuthorityLost {
    pub fn into_store_error(self) -> super::error::StoreError {
        super::error::StoreError::Database(Box::new(self))
    }
}

impl RecordExists {
    pub fn into_store_error(self) -> super::error::StoreError {
        super::error::StoreError::Database(Box::new(self))
    }
}

pub fn is_record_exists(error: &super::error::StoreError) -> bool {
    has_source::<RecordExists>(error)
}

pub fn is_authority_lost(error: &super::error::StoreError) -> bool {
    has_source::<AuthorityLost>(error)
}

fn has_source<E: std::error::Error + 'static>(error: &super::error::StoreError) -> bool {
    let mut current: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(error) = current {
        if error.is::<E>() {
            return true;
        }
        current = error.source();
    }
    false
}
