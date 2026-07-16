//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

//! Traits defining several stores used throughout the Signal Protocol.

use crate::protocol::error::Result;
use crate::protocol::sender_keys::SenderKeyRecord;
use crate::protocol::state::{
    PreKeyId, PreKeyRecord, SessionRecord, SignedPreKeyId, SignedPreKeyRecord,
};
use crate::protocol::{IdentityKey, IdentityKeyPair, ProtocolAddress, SignalProtocolError};
use crate::store::sender_key_name::SenderKeyName;

/// Each Signal message can be considered to have exactly two participants, a sender and receiver.
///
/// [IdentityKeyStore::is_trusted_identity] uses this to ensure the identity provided is configured
/// for the appropriate role.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Direction {
    /// We are in the context of sending a message.
    Sending,
    /// We are in the context of receiving a message.
    Receiving,
}

/// The result of saving a new identity key for a protocol address.
#[derive(Copy, Clone, Debug, Eq, PartialEq, derive_more::TryFrom)]
#[repr(C)]
#[try_from(repr)]
pub enum IdentityChange {
    /// The protocol address didn't have an identity key or had the same key.
    NewOrUnchanged,
    /// The new identity key replaced a different key for the protocol address.
    ReplacedExisting,
}

#[cfg(not(target_arch = "wasm32"))]
pub trait ThreadSafe: Send + Sync {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + Sync> ThreadSafe for T {}

#[cfg(target_arch = "wasm32")]
pub trait ThreadSafe {}
#[cfg(target_arch = "wasm32")]
impl<T> ThreadSafe for T {}

/// Interface defining the identity store, which may be in-memory, on-disk, etc.
///
/// Signal clients usually use the identity store in a [TOFU] manner, but this is not required.
///
/// [TOFU]: https://en.wikipedia.org/wiki/Trust_on_first_use
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait IdentityKeyStore: ThreadSafe {
    /// Return the single specific identity the store is assumed to represent, with private key.
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair>;

    /// Return a [u32] specific to this store instance.
    ///
    /// This local registration id is separate from the per-device identifier used in
    /// [ProtocolAddress] and should not change run over run.
    ///
    /// If the same *device* is unregistered, then registers again, the [ProtocolAddress::device_id]
    /// may be the same, but the store registration id returned by this method should
    /// be regenerated.
    async fn get_local_registration_id(&self) -> Result<u32>;

    /// Record an identity into the store. The identity is then considered "trusted".
    ///
    /// The return value represents whether an existing identity was replaced.
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Result<IdentityChange>;

    /// Return whether an identity is trusted for the role specified by `direction`.
    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        direction: Direction,
    ) -> Result<bool>;

    /// Return the public identity for the given `address`, if known.
    async fn get_identity(&self, address: &ProtocolAddress) -> Result<Option<IdentityKey>>;
}

/// Interface for storing pre-keys downloaded from a server.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait PreKeyStore: ThreadSafe {
    /// Look up the pre-key corresponding to `prekey_id`.
    async fn get_pre_key(&self, prekey_id: PreKeyId) -> Result<PreKeyRecord>;

    /// Set the entry for `prekey_id` to the value of `record`.
    async fn save_pre_key(&mut self, prekey_id: PreKeyId, record: &PreKeyRecord) -> Result<()>;

    /// Remove the entry for `prekey_id`.
    async fn remove_pre_key(&mut self, prekey_id: PreKeyId) -> Result<()>;
}

/// Interface for storing signed pre-keys downloaded from a server.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SignedPreKeyStore: ThreadSafe {
    /// Look up the signed pre-key corresponding to `signed_prekey_id`.
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> Result<SignedPreKeyRecord>;

    /// Set the entry for `signed_prekey_id` to the value of `record`.
    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> Result<()>;
}

/// Interface for a Signal client instance to store a session associated with another particular
/// separate Signal client instance.
///
/// This [SessionRecord] object between a pair of Signal clients is used to drive the state for the
/// forward-secret message chain in the [Double Ratchet] protocol.
///
/// [Double Ratchet]: https://signal.org/docs/specifications/doubleratchet/
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SessionStore: ThreadSafe {
    /// Loads the session for `address` without removing it from the store.
    /// Destructive cache checkouts belong in [`load_session_for_update`].
    async fn load_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>>;

    /// Loads a mutation copy and optionally marks it as destructively checked out.
    ///
    /// `None` is an assertion that [`load_session`] was non-destructive. A store
    /// returning a generation must also implement
    /// [`try_store_session_from_checkout`] so cancellation can restore the record.
    #[doc(hidden)]
    async fn load_session_for_update(
        &self,
        address: &ProtocolAddress,
    ) -> Result<(Option<SessionRecord>, Option<u64>)> {
        Ok((self.load_session(address).await?, None))
    }

    /// Non-destructive existence check (must not consume the cached entry).
    async fn has_session(&self, address: &ProtocolAddress) -> Result<bool>;

    /// Set the entry for `address` to the value of `record`.
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> Result<()>;

    /// Gives destructive stores a synchronous recovery path before an async
    /// owner can be cancelled again.
    #[doc(hidden)]
    fn try_store_session_from_checkout(
        &mut self,
        _address: &ProtocolAddress,
        record: SessionRecord,
        _generation: Option<u64>,
        _had_session: bool,
    ) -> SessionCheckoutStoreResult {
        SessionCheckoutStoreResult::Unhandled(record)
    }

    /// Releases an empty reservation made by a destructive update load.
    #[doc(hidden)]
    fn cancel_session_checkout(&mut self, _address: &ProtocolAddress, _generation: Option<u64>) {}

    /// Drives a queued synchronous return once the contended store is available.
    #[doc(hidden)]
    async fn complete_session_checkout(&mut self) {}
}

/// Result of returning a record from a cancellation-safe checkout.
#[doc(hidden)]
pub enum SessionCheckoutStoreResult {
    Stored,
    Rejected,
    Pending(std::sync::Arc<portable_atomic::AtomicBool>),
    Unhandled(SessionRecord),
}

/// Keeps an owned session recoverable while an async protocol operation can be cancelled.
pub struct SessionCheckout<'a> {
    store: &'a mut dyn SessionStore,
    address: &'a ProtocolAddress,
    record: Option<SessionRecord>,
    generation: Option<u64>,
    had_session: bool,
}

impl<'a> SessionCheckout<'a> {
    /// Loads an existing session and arms cancellation recovery.
    pub async fn load(
        store: &'a mut dyn SessionStore,
        address: &'a ProtocolAddress,
    ) -> Result<Option<Self>> {
        let (record, generation) = store.load_session_for_update(address).await?;
        let Some(record) = record else {
            store.cancel_session_checkout(address, generation);
            return Ok(None);
        };
        Ok(Some(Self {
            store,
            address,
            record: Some(record),
            generation,
            had_session: true,
        }))
    }

    /// Creates an empty working record if the store has no session yet.
    pub async fn load_or_create(
        store: &'a mut dyn SessionStore,
        address: &'a ProtocolAddress,
    ) -> Result<Self> {
        let (record, generation) = store.load_session_for_update(address).await?;
        let had_session = record.is_some();
        Ok(Self {
            store,
            address,
            record: Some(record.unwrap_or_else(SessionRecord::new_fresh)),
            generation,
            had_session,
        })
    }

    /// Reports whether the store supplied the working record.
    pub fn had_session(&self) -> bool {
        self.had_session
    }

    /// Borrows the working record.
    pub fn record(&self) -> &SessionRecord {
        self.record
            .as_ref()
            .expect("a live checkout always owns its record")
    }

    /// Mutably borrows the working record.
    pub fn record_mut(&mut self) -> &mut SessionRecord {
        self.record
            .as_mut()
            .expect("a live checkout always owns its record")
    }

    /// Preserves recovery while replacing a rejected mutation with its snapshot.
    pub fn replace(&mut self, record: SessionRecord) {
        self.record = Some(record);
    }

    /// Prevents a deliberately rejected fresh session from being recovered.
    pub fn discard(mut self) {
        debug_assert!(!self.had_session, "only a fresh checkout can be discarded");
        if self.had_session {
            return;
        }
        self.record.take();
        self.store
            .cancel_session_checkout(self.address, self.generation);
    }

    /// Returns the record by move without a cancellation gap on take-style stores.
    pub async fn commit(mut self) -> Result<()> {
        let record = self
            .record
            .take()
            .expect("a live checkout always owns its record");
        match self.store.try_store_session_from_checkout(
            self.address,
            record,
            self.generation,
            self.had_session,
        ) {
            SessionCheckoutStoreResult::Stored => Ok(()),
            SessionCheckoutStoreResult::Rejected => Err(checkout_rejected()),
            SessionCheckoutStoreResult::Pending(completion) => {
                self.store.complete_session_checkout().await;
                if completion.load(portable_atomic::Ordering::Acquire) {
                    Ok(())
                } else {
                    Err(checkout_rejected())
                }
            }
            SessionCheckoutStoreResult::Unhandled(record) => {
                self.store.store_session(self.address, record).await
            }
        }
    }
}

fn checkout_rejected() -> SignalProtocolError {
    SignalProtocolError::InvalidState(
        "SessionCheckout::commit",
        "session changed during mutation".to_string(),
    )
}

impl Drop for SessionCheckout<'_> {
    fn drop(&mut self) {
        let Some(record) = self.record.take() else {
            return;
        };
        if self.had_session || record.session_state().is_some() {
            match self.store.try_store_session_from_checkout(
                self.address,
                record,
                self.generation,
                self.had_session,
            ) {
                SessionCheckoutStoreResult::Stored
                | SessionCheckoutStoreResult::Rejected
                | SessionCheckoutStoreResult::Pending(_) => {}
                SessionCheckoutStoreResult::Unhandled(_) => {}
            }
        } else {
            self.store
                .cancel_session_checkout(self.address, self.generation);
        }
    }
}

/// Interface for storing sender key records, allowing multiple keys per user.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SenderKeyStore: ThreadSafe {
    /// Assign `record` to the entry for `(sender, distribution_id)`.
    async fn store_sender_key(
        &mut self,
        sender_key_name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> Result<()>;

    /// Look up the entry corresponding to `(sender, distribution_id)`.
    async fn load_sender_key(
        &self,
        sender_key_name: &SenderKeyName,
    ) -> Result<Option<SenderKeyRecord>>;

    /// Serializes every load/mutate/store of one sender-key chain so concurrent
    /// encrypt, decrypt, distribution and rotation operations cannot overwrite
    /// each other. Default is uncontended; stores over shared state override it.
    async fn sender_key_lock(
        &self,
        _sender_key_name: &SenderKeyName,
    ) -> std::sync::Arc<async_lock::Mutex<()>> {
        std::sync::Arc::new(async_lock::Mutex::new(()))
    }

    /// Serializes per-group session setup (prekey fetch + X3DH) so concurrent
    /// cold sends to the same group can't race writes to the same per-device
    /// sessions. Distinct from [`sender_key_lock`](Self::sender_key_lock):
    /// held only across setup — never the chain-advancing critical section —
    /// so it may span network I/O without blocking warm sends. Default is
    /// uncontended; stores over shared state override it.
    async fn session_setup_lock(
        &self,
        _sender_key_name: &SenderKeyName,
    ) -> std::sync::Arc<async_lock::Mutex<()>> {
        std::sync::Arc::new(async_lock::Mutex::new(()))
    }
}

/// Mixes in all the store interfaces defined in this module.
pub trait ProtocolStore: SessionStore + PreKeyStore + SignedPreKeyStore + IdentityKeyStore {}

impl IdentityChange {
    /// Convenience constructor from a boolean `changed` flag.
    ///
    /// Returns [`IdentityChange::ReplacedExisting`] if `changed` is `true`,
    /// otherwise [`IdentityChange::NewOrUnchanged`].
    pub fn from_changed(changed: bool) -> Self {
        if changed {
            Self::ReplacedExisting
        } else {
            Self::NewOrUnchanged
        }
    }
}
