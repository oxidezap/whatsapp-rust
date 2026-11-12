// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! libsignal protocol stores over a byte-level [`SignalStore`].
//!
//! Records use the same for-store encodings as wacore's own direct-write
//! adapters, so a scope written here reads back in any wacore consumer.

use crate::device::DeviceKeys;
use async_lock::Mutex;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use wacore::libsignal::protocol::{
    Direction, GenericSignedPreKey, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore,
    PreKeyId, PreKeyRecord, PreKeyStore, ProtocolAddress, PublicKey, SenderKeyRecord,
    SenderKeyStore, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore,
};
use wacore::libsignal::store::record_helpers;
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use wacore::store::traits::SignalStore;

type SignalResult<T> = Result<T, SignalProtocolError>;

/// One per process. Session and sender-key records carry the incarnation that
/// wrote them; a record read back under a different incarnation (another
/// process, or this one after a restart) is treated as possibly crashed, so
/// libsignal skips past chain positions it may already have used.
pub(crate) fn incarnation() -> &'static [u8; 16] {
    static INCARNATION: OnceLock<[u8; 16]> = OnceLock::new();
    INCARNATION.get_or_init(|| {
        use rand::Rng;
        let mut bytes = [0u8; 16];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut bytes);
        bytes
    })
}

fn backend<E>(context: &'static str) -> impl FnOnce(E) -> SignalProtocolError
where
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    move |e| SignalProtocolError::BackendError(context, e.into())
}

pub(crate) struct Stores<S> {
    pub store: Arc<S>,
    pub keys: Arc<DeviceKeys>,
    pub sender_key_locks: Arc<KeyedLocks>,
}

impl<S> Clone for Stores<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            keys: self.keys.clone(),
            sender_key_locks: self.sender_key_locks.clone(),
        }
    }
}

/// Lazily created per-key async mutexes. Entries are never removed; a node
/// serves a bounded set of chats per scope.
#[derive(Default)]
pub(crate) struct KeyedLocks {
    locks: std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl KeyedLocks {
    pub fn get(&self, key: &str) -> Arc<Mutex<()>> {
        let mut locks = self
            .locks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        locks
            .entry(key.to_owned())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalStore + 'static> SessionStore for Stores<S> {
    async fn load_session(&self, address: &ProtocolAddress) -> SignalResult<Option<SessionRecord>> {
        match self
            .store
            .get_session(address.as_str())
            .await
            .map_err(backend("load_session"))?
        {
            Some(bytes) => Ok(Some(SessionRecord::deserialize_for_store(
                &bytes,
                incarnation(),
            )?)),
            None => Ok(None),
        }
    }

    async fn has_session(&self, address: &ProtocolAddress) -> SignalResult<bool> {
        self.store
            .has_session(address.as_str())
            .await
            .map_err(backend("has_session"))
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> SignalResult<()> {
        let mut bytes = Vec::new();
        record.serialize_into_for_store(&mut bytes, incarnation());
        self.store
            .put_session(address.as_str(), &bytes)
            .await
            .map_err(backend("store_session"))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalStore + 'static> IdentityKeyStore for Stores<S> {
    async fn get_identity_key_pair(&self) -> SignalResult<IdentityKeyPair> {
        Ok(self.keys.identity_pair())
    }

    async fn get_local_registration_id(&self) -> SignalResult<u32> {
        Ok(self.keys.registration_id)
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> SignalResult<IdentityChange> {
        let existing = self.get_identity(address).await?;
        let key: [u8; 32] = *identity.public_key().public_key_bytes();
        self.store
            .put_identity(address.as_str(), key)
            .await
            .map_err(backend("save_identity"))?;
        Ok(match existing {
            Some(previous) if &previous != identity => IdentityChange::ReplacedExisting,
            _ => IdentityChange::NewOrUnchanged,
        })
    }

    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> SignalResult<bool> {
        // Same policy as wacore's own adapters and WA Web: identity changes
        // surface through save_identity, never by refusing the message.
        Ok(true)
    }

    async fn get_identity(&self, address: &ProtocolAddress) -> SignalResult<Option<IdentityKey>> {
        match self
            .store
            .load_identity(address.as_str())
            .await
            .map_err(backend("get_identity"))?
        {
            Some(bytes) => Ok(Some(IdentityKey::new(
                PublicKey::from_djb_public_key_bytes(&bytes)?,
            ))),
            None => Ok(None),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalStore + 'static> PreKeyStore for Stores<S> {
    async fn get_pre_key(&self, prekey_id: PreKeyId) -> SignalResult<PreKeyRecord> {
        let bytes = self
            .store
            .load_prekey(prekey_id.into())
            .await
            .map_err(backend("get_pre_key"))?
            .ok_or(SignalProtocolError::InvalidPreKeyId)?;
        let structure = waproto::codec::pre_key_record_decode(&bytes)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        record_helpers::prekey_structure_to_record(structure)
    }

    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> SignalResult<()> {
        let structure = record_helpers::prekey_record_to_structure(record)?;
        self.store
            .store_prekey(
                prekey_id.into(),
                &waproto::codec::pre_key_record_to_vec(&structure),
                false,
            )
            .await
            .map_err(backend("save_pre_key"))
    }

    async fn remove_pre_key(&mut self, prekey_id: PreKeyId) -> SignalResult<()> {
        self.store
            .remove_prekey(prekey_id.into())
            .await
            .map_err(backend("remove_pre_key"))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalStore + 'static> SignedPreKeyStore for Stores<S> {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> SignalResult<SignedPreKeyRecord> {
        let id: u32 = id.into();
        let structure = if id == self.keys.signed_prekey_id {
            record_helpers::new_signed_pre_key_record(
                id,
                &self.keys.signed_prekey,
                self.keys.signed_prekey_signature,
                wacore::time::now_utc(),
            )?
        } else {
            // A prekey message minted against a rotated-out signed prekey
            // still names its old ID.
            let bytes = self
                .store
                .load_signed_prekey(id)
                .await
                .map_err(backend("get_signed_pre_key"))?
                .ok_or(SignalProtocolError::InvalidSignedPreKeyId)?;
            waproto::codec::signed_pre_key_record_decode(&bytes)
                .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?
        };
        record_helpers::signed_prekey_structure_to_record(structure)
    }

    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> SignalResult<()> {
        self.store
            .store_signed_prekey(id.into(), &GenericSignedPreKey::serialize(record)?)
            .await
            .map_err(backend("save_signed_pre_key"))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalStore + 'static> SenderKeyStore for Stores<S> {
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> SignalResult<()> {
        let bytes = record.serialize_for_store(incarnation())?;
        self.store
            .put_sender_key(name.cache_key(), &bytes)
            .await
            .map_err(backend("store_sender_key"))
    }

    async fn load_sender_key(&self, name: &SenderKeyName) -> SignalResult<Option<SenderKeyRecord>> {
        match self
            .store
            .get_sender_key(name.cache_key())
            .await
            .map_err(backend("load_sender_key"))?
        {
            Some(bytes) => Ok(Some(SenderKeyRecord::deserialize_for_store(
                &bytes,
                incarnation(),
            )?)),
            None => Ok(None),
        }
    }

    async fn sender_key_lock(&self, name: &SenderKeyName) -> Arc<Mutex<()>> {
        self.sender_key_locks.get(name.cache_key())
    }
}
