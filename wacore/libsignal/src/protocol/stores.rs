// SPDX-License-Identifier: MIT
use super::error::Result;
use super::*;
use async_trait::async_trait;
use portable_atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionCheckoutKey {
    generation: u64,
    token: std::num::NonZeroU64,
}
impl SessionCheckoutKey {
    pub fn new(generation: u64, token: std::num::NonZeroU64) -> Self {
        Self { generation, token }
    }
    pub const fn generation(&self) -> u64 {
        self.generation
    }
    pub const fn token(&self) -> std::num::NonZeroU64 {
        self.token
    }
}
// An unhandled handoff returns ownership without allocating a box per checkout.
#[allow(clippy::large_enum_variant)]
pub enum SessionCheckoutStoreResult {
    Stored,
    Pending(Arc<AtomicBool>),
    Rejected,
    Unhandled(SessionRecord),
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SessionStore: ThreadSafe {
    async fn load_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>>;
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> Result<()>;
    async fn prefetch_sessions(&self, _addresses: &[ProtocolAddress]) -> Result<()> {
        Ok(())
    }
    async fn load_session_for_update(
        &self,
        address: &ProtocolAddress,
    ) -> Result<(Option<SessionRecord>, Option<SessionCheckoutKey>)> {
        Ok((self.load_session(address).await?, None))
    }
    fn try_load_session_for_update(
        &self,
        _address: &ProtocolAddress,
    ) -> Option<Result<(Option<SessionRecord>, Option<SessionCheckoutKey>)>> {
        None
    }
    fn try_store_session_from_checkout(
        &mut self,
        _address: &ProtocolAddress,
        record: SessionRecord,
        _checkout: Option<SessionCheckoutKey>,
        _had_session: bool,
    ) -> SessionCheckoutStoreResult {
        SessionCheckoutStoreResult::Unhandled(record)
    }
    /// Couple consumed-prekey bookkeeping to the received session's ownership
    /// transfer. Hosts with deferred deletion override this to preserve the id
    /// even if completion is cancelled. The default preserves the existing
    /// store hook; those hosts still receive the id in `DecryptionResult`.
    fn try_store_received_session_from_checkout(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
        checkout: Option<SessionCheckoutKey>,
        had_session: bool,
        _consumed_prekey: Option<PreKeyId>,
    ) -> SessionCheckoutStoreResult {
        self.try_store_session_from_checkout(address, record, checkout, had_session)
    }
    fn cancel_session_checkout(
        &mut self,
        _address: &ProtocolAddress,
        _checkout: Option<SessionCheckoutKey>,
    ) {
    }
    async fn complete_session_checkout(&mut self) {}
    fn try_has_session(&self, _address: &ProtocolAddress) -> Option<Result<bool>> {
        None
    }
    async fn has_session(&self, address: &ProtocolAddress) -> Result<bool> {
        Ok(self
            .load_session(address)
            .await?
            .is_some_and(|record| record.session_state().is_some()))
    }
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait IdentityKeyStore: ThreadSafe {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair>;
    async fn get_local_registration_id(&self) -> Result<u32>;
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Result<IdentityChange>;
    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        direction: Direction,
    ) -> Result<bool>;
    async fn get_identity(&self, address: &ProtocolAddress) -> Result<Option<IdentityKey>>;
    fn try_is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Option<Result<bool>> {
        None
    }
    fn try_save_identity(
        &mut self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
    ) -> Option<Result<IdentityChange>> {
        None
    }
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait PreKeyStore: ThreadSafe {
    async fn get_pre_key(&self, id: PreKeyId) -> Result<PreKeyRecord>;
    async fn save_pre_key(&mut self, id: PreKeyId, record: &PreKeyRecord) -> Result<()>;
    async fn remove_pre_key(&mut self, id: PreKeyId) -> Result<()>;
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignedPreKeyStore: ThreadSafe {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> Result<SignedPreKeyRecord>;
    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> Result<()>;
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SenderKeyStore: ThreadSafe {
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> Result<()>;
    async fn load_sender_key(&self, name: &SenderKeyName) -> Result<Option<SenderKeyRecord>>;
    async fn sender_key_lock(&self, _name: &SenderKeyName) -> Arc<async_lock::Mutex<()>> {
        Arc::new(async_lock::Mutex::new(()))
    }
    async fn session_setup_lock(&self, name: &SenderKeyName) -> Arc<async_lock::Mutex<()>> {
        self.sender_key_lock(name).await
    }
}
/// A checked-out slot is restored synchronously on drop. The host must implement
/// the checkout hooks when load transfers cache ownership; a borrowed-copy store
/// may use the default hooks because dropping that copy cannot hide its record.
pub struct SessionCheckout<'a, S: SessionStore + ?Sized> {
    store: &'a mut S,
    address: ProtocolAddress,
    record: Option<SessionRecord>,
    checkout: Option<SessionCheckoutKey>,
    had_session: bool,
}
impl<'a, S: SessionStore + ?Sized> SessionCheckout<'a, S> {
    pub async fn load(store: &'a mut S, address: &ProtocolAddress) -> Result<Option<Self>> {
        let (record, checkout) = if let Some(answer) = store.try_load_session_for_update(address) {
            answer?
        } else {
            store.load_session_for_update(address).await?
        };
        if record.is_none() {
            store.cancel_session_checkout(address, checkout);
            return Ok(None);
        }
        Ok(Some(Self {
            store,
            address: address.clone(),
            had_session: true,
            record,
            checkout,
        }))
    }
    pub(crate) async fn load_or_fresh(store: &'a mut S, address: &ProtocolAddress) -> Result<Self> {
        let (record, checkout) = if let Some(answer) = store.try_load_session_for_update(address) {
            answer?
        } else {
            store.load_session_for_update(address).await?
        };
        let had_session = record.is_some();
        Ok(Self {
            store,
            address: address.clone(),
            record: Some(record.unwrap_or_else(SessionRecord::new_fresh)),
            checkout,
            had_session,
        })
    }
    pub fn record(&self) -> &SessionRecord {
        self.record
            .as_ref()
            .expect("checkout record is present until consumed")
    }
    pub fn record_mut(&mut self) -> &mut SessionRecord {
        self.record
            .as_mut()
            .expect("checkout record is present until consumed")
    }
    pub async fn commit(self) -> Result<()> {
        self.commit_received(None).await
    }
    pub async fn commit_received(mut self, consumed_prekey: Option<PreKeyId>) -> Result<()> {
        let record = self.record.take().ok_or_else(|| {
            SignalProtocolError::InvalidState("checkout", "record already consumed".into())
        })?;
        match self.store.try_store_received_session_from_checkout(
            &self.address,
            record,
            self.checkout,
            self.had_session,
            consumed_prekey,
        ) {
            SessionCheckoutStoreResult::Stored => {
                self.checkout = None;
            }
            SessionCheckoutStoreResult::Rejected => {
                self.checkout = None;
                return Err(SignalProtocolError::InvalidState(
                    "SessionCheckout::commit",
                    "stale checkout".into(),
                ));
            }
            SessionCheckoutStoreResult::Pending(completion) => {
                self.checkout = None;
                if !completion.load(portable_atomic::Ordering::Acquire) {
                    self.store.complete_session_checkout().await;
                    if !completion.load(portable_atomic::Ordering::Acquire) {
                        return Err(SignalProtocolError::InvalidState(
                            "checkout",
                            "host did not complete queued restoration".into(),
                        ));
                    }
                }
            }
            SessionCheckoutStoreResult::Unhandled(record) => {
                if self.checkout.is_some() {
                    self.record = Some(record);
                    return Err(SignalProtocolError::InvalidState(
                        "checkout",
                        "owned checkout must implement synchronous restoration".into(),
                    ));
                }
                self.store.store_session(&self.address, record).await?;
            }
        }
        self.checkout = None;
        Ok(())
    }
}
impl<S: SessionStore + ?Sized> Drop for SessionCheckout<'_, S> {
    fn drop(&mut self) {
        if let Some(record) = self.record.take() {
            if !self.had_session
                && record.session_state().is_none()
                && record.inner.previous.is_empty()
            {
                self.store
                    .cancel_session_checkout(&self.address, self.checkout);
                return;
            }
            if let SessionCheckoutStoreResult::Unhandled(_) =
                self.store.try_store_session_from_checkout(
                    &self.address,
                    record,
                    self.checkout,
                    self.had_session,
                )
            {
                self.store
                    .cancel_session_checkout(&self.address, self.checkout);
            }
        } else if self.checkout.is_some() {
            self.store
                .cancel_session_checkout(&self.address, self.checkout);
        }
    }
}
pub async fn has_session<S: SessionStore + ?Sized>(
    store: &S,
    address: &ProtocolAddress,
) -> Result<bool> {
    if let Some(answer) = store.try_has_session(address) {
        answer
    } else {
        store.has_session(address).await
    }
}
pub async fn is_trusted_identity<I: IdentityKeyStore + ?Sized>(
    store: &I,
    address: &ProtocolAddress,
    identity: &IdentityKey,
    direction: Direction,
) -> Result<bool> {
    if let Some(answer) = store.try_is_trusted_identity(address, identity, direction) {
        answer
    } else {
        store
            .is_trusted_identity(address, identity, direction)
            .await
    }
}
pub(crate) async fn save_identity<I: IdentityKeyStore + ?Sized>(
    store: &mut I,
    address: &ProtocolAddress,
    identity: &IdentityKey,
) -> Result<IdentityChange> {
    if let Some(answer) = store.try_save_identity(address, identity) {
        answer
    } else {
        store.save_identity(address, identity).await
    }
}
