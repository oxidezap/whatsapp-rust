use crate::store::Device;
use crate::store::persistence_manager::PersistenceManager;
use crate::store::signal_cache::SignalStoreCache;
use async_trait::async_trait;
use std::sync::Arc;
use wacore::libsignal::protocol::{
    Direction, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore, PreKeyId,
    PreKeyRecord, PreKeyStore, ProtocolAddress, SessionCheckoutKey, SessionCheckoutStoreResult,
    SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId, SignedPreKeyRecord,
    SignedPreKeyStore,
};

use wacore::libsignal::store::record_helpers as wacore_record;
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use wacore::libsignal::store::{
    PreKeyStore as WacorePreKeyStore, SignedPreKeyStore as WacoreSignedPreKeyStore,
};

fn signal_err<E>(context: &'static str) -> impl FnOnce(E) -> SignalProtocolError
where
    E: Into<Box<dyn std::error::Error + Send + Sync + 'static>>,
{
    move |e| SignalProtocolError::BackendError(context, e.into())
}

/// Boxed future with the exact shape `#[async_trait]` expects, so the hot
/// methods below can be hand-desugared: a cache hit completes synchronously
/// and boxes only a tiny `Ready` instead of the full async state machine.
#[cfg(not(target_arch = "wasm32"))]
type BoxFut<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
type BoxFut<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + 'a>>;

use std::future::{Future, ready};

/// Snapshots per call instead of holding the device lock: `get_device_snapshot`
/// is a refcount bump, so a backend round-trip no longer keeps a read guard
/// that every later Signal read would queue behind once a write arrives.
///
/// Per call, not per adapter: `signed_pre_key_id` is promoted by rotation and
/// its staged backend row is then dropped, so an adapter pinned to a
/// pre-rotation snapshot would fail to load the very key a peer just used.
#[derive(Clone)]
struct SharedDevice {
    persistence_manager: Arc<PersistenceManager>,
    cache: Arc<SignalStoreCache>,
}

impl SharedDevice {
    fn device(&self) -> Arc<Device> {
        self.persistence_manager.get_device_snapshot()
    }
}

#[derive(Clone)]
pub struct SessionAdapter(SharedDevice);
#[derive(Clone)]
pub struct IdentityAdapter(SharedDevice);
#[derive(Clone)]
pub struct PreKeyAdapter(SharedDevice);
#[derive(Clone)]
pub struct SignedPreKeyAdapter(SharedDevice);

#[derive(Clone)]
pub struct SenderKeyAdapter(SharedDevice);

impl SenderKeyAdapter {
    /// Build a standalone sender-key store without constructing the full
    /// five-store [`SignalProtocolStoreAdapter`]. Used on the SKDM-processing
    /// path, which only needs the sender-key store.
    pub fn new(persistence_manager: Arc<PersistenceManager>, cache: Arc<SignalStoreCache>) -> Self {
        Self(SharedDevice {
            persistence_manager,
            cache,
        })
    }
}

#[derive(Clone)]
pub struct SignalProtocolStoreAdapter {
    pub session_store: SessionAdapter,
    pub identity_store: IdentityAdapter,
    pub pre_key_store: PreKeyAdapter,
    pub signed_pre_key_store: SignedPreKeyAdapter,
    pub sender_key_store: SenderKeyAdapter,
}

impl SignalProtocolStoreAdapter {
    pub fn new(persistence_manager: Arc<PersistenceManager>, cache: Arc<SignalStoreCache>) -> Self {
        let shared = SharedDevice {
            persistence_manager,
            cache,
        };
        Self {
            session_store: SessionAdapter(shared.clone()),
            identity_store: IdentityAdapter(shared.clone()),
            pre_key_store: PreKeyAdapter(shared.clone()),
            signed_pre_key_store: SignedPreKeyAdapter(shared.clone()),
            sender_key_store: SenderKeyAdapter(shared),
        }
    }

    pub fn as_signal_stores(&mut self) -> wacore::send::SignalStores<'_> {
        wacore::send::SignalStores {
            session_store: &mut self.session_store,
            identity_store: &mut self.identity_store,
            prekey_store: &mut self.pre_key_store,
            signed_prekey_store: &self.signed_pre_key_store,
            sender_key_store: &mut self.sender_key_store,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SessionStore for SessionAdapter {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> Result<Option<SessionRecord>, SignalProtocolError> {
        let device = self.0.device();
        self.0
            .cache
            .peek_session(address, &*device.backend)
            .await
            .map(|record| record.map(|record| (*record).clone()))
            .map_err(signal_err("backend"))
    }

    async fn load_session_for_update(
        &self,
        address: &ProtocolAddress,
    ) -> Result<(Option<SessionRecord>, Option<SessionCheckoutKey>), SignalProtocolError> {
        let device = self.0.device();
        self.0
            .cache
            .checkout_session(address, &*device.backend)
            .await
            .map(|(record, checkout)| (record, Some(checkout)))
            .map_err(signal_err("backend"))
    }

    fn try_load_session_for_update(
        &self,
        address: &ProtocolAddress,
    ) -> Option<Result<(Option<SessionRecord>, Option<SessionCheckoutKey>), SignalProtocolError>>
    {
        self.0.cache.try_checkout_session(address).map(|result| {
            result
                .map(|(record, checkout)| (record, Some(checkout)))
                .map_err(signal_err("backend"))
        })
    }

    fn try_store_session_from_checkout(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
        checkout: Option<SessionCheckoutKey>,
        had_session: bool,
    ) -> SessionCheckoutStoreResult {
        let Some(checkout) = checkout else {
            return SessionCheckoutStoreResult::Unhandled(record);
        };
        self.0
            .cache
            .restore_session_from_checkout(address, record, checkout, had_session)
    }

    fn cancel_session_checkout(
        &mut self,
        address: &ProtocolAddress,
        checkout: Option<SessionCheckoutKey>,
    ) {
        if let Some(checkout) = checkout {
            self.0.cache.cancel_session_checkout(address, checkout);
        }
    }

    async fn complete_session_checkout(&mut self) {
        self.0.cache.complete_session_checkout().await;
    }

    fn try_has_session(
        &self,
        address: &ProtocolAddress,
    ) -> Option<Result<bool, SignalProtocolError>> {
        self.0.cache.try_has_session(address).map(Ok)
    }

    // Hand-desugared (see `BoxFut`): a cached answer skips the device lock
    // and returns a ready future.
    fn has_session<'life0, 'life1, 'async_trait>(
        &'life0 self,
        address: &'life1 ProtocolAddress,
    ) -> BoxFut<'async_trait, Result<bool, SignalProtocolError>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        if let Some(answer) = SessionStore::try_has_session(self, address) {
            return Box::pin(ready(answer));
        }
        Box::pin(async move {
            let device = self.0.device();
            self.0
                .cache
                .has_session(address, &*device.backend)
                .await
                .map_err(signal_err("backend"))
        })
    }

    // Hand-desugared (see `BoxFut`): the record moves into the cache before
    // the future is built, so the hot path never boxes the record-sized
    // state machine. Contention (a flush commit) falls back to the async put.
    fn store_session<'life0, 'life1, 'async_trait>(
        &'life0 mut self,
        address: &'life1 ProtocolAddress,
        record: SessionRecord,
    ) -> BoxFut<'async_trait, Result<(), SignalProtocolError>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        match self.0.cache.try_put_session(address, record) {
            Ok(()) => Box::pin(ready(Ok(()))),
            Err(record) => Box::pin(async move {
                self.0.cache.put_session(address, record).await;
                Ok(())
            }),
        }
    }
}

impl IdentityAdapter {
    /// The cache-hit half of [`IdentityKeyStore::save_identity`], shared with
    /// its sync hook so the two cannot drift.
    ///
    /// `None` means the caller must take the async path: either the previous
    /// value was not cached, or a concurrent flush owns the entry. A parse
    /// failure of the cached bytes is reported here rather than swallowed,
    /// matching the async path in erroring BEFORE any write.
    fn save_identity_cached(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Option<Result<IdentityChange, SignalProtocolError>> {
        let prev = self.0.cache.try_get_identity(address)?;
        let change = match parse_cached_identity(prev) {
            Ok(None) => IdentityChange::NewOrUnchanged,
            Ok(Some(existing)) if &existing == identity => IdentityChange::NewOrUnchanged,
            Ok(Some(_)) => IdentityChange::ReplacedExisting,
            Err(e) => return Some(Err(e)),
        };
        self.0
            .cache
            .try_put_identity(address, identity.public_key().public_key_bytes())
            .then_some(Ok(change))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl IdentityKeyStore for IdentityAdapter {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
        let device = self.0.device();
        IdentityKeyStore::get_identity_key_pair(device.as_ref())
            .await
            .map_err(signal_err("get_identity_key_pair"))
    }

    async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
        let device = self.0.device();
        IdentityKeyStore::get_local_registration_id(device.as_ref())
            .await
            .map_err(signal_err("get_local_registration_id"))
    }

    // Hand-desugared (see `BoxFut`): with the previous value cached, the
    // read+compare+write completes synchronously. A parse failure of the
    // cached bytes errors BEFORE the write, like the async path.
    fn save_identity<'life0, 'life1, 'life2, 'async_trait>(
        &'life0 mut self,
        address: &'life1 ProtocolAddress,
        identity: &'life2 IdentityKey,
    ) -> BoxFut<'async_trait, Result<IdentityChange, SignalProtocolError>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        Self: 'async_trait,
    {
        if let Some(answer) = self.save_identity_cached(address, identity) {
            return Box::pin(ready(answer));
        }
        Box::pin(async move {
            let existing_identity = self.get_identity(address).await?;

            // Cache-first: write to cache only. The cache flushes to the backend
            // during flush_signal_cache(). This avoids a synchronous backend write
            // on every encrypt/decrypt. is_trusted_identity always returns true
            // (matching WA Web), so the Device-level save is redundant.
            self.0
                .cache
                .put_identity(address, identity.public_key().public_key_bytes())
                .await;

            match existing_identity {
                None => Ok(IdentityChange::NewOrUnchanged),
                Some(existing) if &existing == identity => Ok(IdentityChange::NewOrUnchanged),
                Some(_) => Ok(IdentityChange::ReplacedExisting),
            }
        })
    }

    // Boxing a future whose whole body is `Ok(true)` costs an allocation per
    // encrypt and per decrypt; the hook lets callers skip it entirely.
    fn try_is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Option<Result<bool, SignalProtocolError>> {
        Some(Ok(true))
    }

    fn try_save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Option<Result<IdentityChange, SignalProtocolError>> {
        self.save_identity_cached(address, identity)
    }

    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Result<bool, SignalProtocolError> {
        // WAWebProtocolStoreUnifiedApi.isTrustedIdentity always returns true;
        // identity changes surface via save_identity. Avoid acquiring the
        // device RwLock just to delegate to a stub — the read is acquired N
        // times per group send (once per recipient device) and adds
        // contention pressure under any future parallel encrypt path.
        Ok(true)
    }

    // Hand-desugared (see `BoxFut`): a cached entry (present OR known-absent)
    // skips the device lock and returns a ready future.
    fn get_identity<'life0, 'life1, 'async_trait>(
        &'life0 self,
        address: &'life1 ProtocolAddress,
    ) -> BoxFut<'async_trait, Result<Option<IdentityKey>, SignalProtocolError>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        if let Some(cached) = self.0.cache.try_get_identity(address) {
            return Box::pin(ready(parse_cached_identity(cached)));
        }
        Box::pin(async move {
            let device = self.0.device();
            let data = self
                .0
                .cache
                .get_identity(address, &*device.backend)
                .await
                .map_err(signal_err("get_identity"))?;
            parse_cached_identity(data)
        })
    }
}

/// Decode the cache's raw 32-byte DJB public key bytes; empty/absent = no
/// identity (mirrors the previous inline match in `get_identity`).
fn parse_cached_identity(
    data: Option<Arc<[u8]>>,
) -> Result<Option<IdentityKey>, SignalProtocolError> {
    match data {
        Some(data) if !data.is_empty() => {
            let public_key =
                wacore::libsignal::protocol::PublicKey::from_djb_public_key_bytes(&data)?;
            Ok(Some(IdentityKey::new(public_key)))
        }
        _ => Ok(None),
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PreKeyStore for PreKeyAdapter {
    async fn get_pre_key(&self, prekey_id: PreKeyId) -> Result<PreKeyRecord, SignalProtocolError> {
        let device = self.0.device();
        WacorePreKeyStore::load_prekey(device.as_ref(), prekey_id.into())
            .await
            .map_err(signal_err("backend"))?
            .ok_or(SignalProtocolError::InvalidPreKeyId)
            .and_then(wacore_record::prekey_structure_to_record)
    }
    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> Result<(), SignalProtocolError> {
        let device = self.0.device();
        let structure = wacore_record::prekey_record_to_structure(record)?;
        WacorePreKeyStore::store_prekey(device.as_ref(), prekey_id.into(), structure, false)
            .await
            .map_err(signal_err("backend"))
    }
    async fn remove_pre_key(&mut self, prekey_id: PreKeyId) -> Result<(), SignalProtocolError> {
        // Plain immediate-removal primitive. The inbound pkmsg path does NOT route
        // through here: message_decrypt reports the consumed prekey and the receive
        // path buffers it via buffer_consumed_prekey so the durable delete is
        // atomic with the session flush (matching WAWebSignalProtocolStoreUnifiedApi).
        let device = self.0.device();
        device
            .backend
            .remove_prekey(prekey_id.into())
            .await
            .map_err(signal_err("backend"))
    }
}

impl PreKeyAdapter {
    /// Buffer a consumed one-time prekey for deletion on the next cache flush,
    /// keyed by the session address whose pkmsg promotion consumed it. Called by
    /// the inbound receive path after `message_decrypt` reports the consumed
    /// prekey: the promoted session is still volatile in the cache, so the prekey
    /// must only be deleted once that session is durably flushed.
    pub async fn buffer_consumed_prekey(&self, prekey_id: PreKeyId, address: &ProtocolAddress) {
        self.0
            .cache
            .remove_prekey(prekey_id.into(), address.as_str())
            .await;
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignedPreKeyStore for SignedPreKeyAdapter {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> Result<SignedPreKeyRecord, SignalProtocolError> {
        let id = signed_prekey_id.into();
        let mut record = WacoreSignedPreKeyStore::load_signed_prekey(self.0.device().as_ref(), id)
            .await
            .map_err(signal_err("backend"))?;
        if record.is_none() {
            // Rotation promotes an id into the device field and only then drops
            // its staged row, so a snapshot taken just before the promotion
            // resolves it in neither place. Re-read before calling it unknown:
            // once the row is gone the field definitely holds it.
            record = WacoreSignedPreKeyStore::load_signed_prekey(self.0.device().as_ref(), id)
                .await
                .map_err(signal_err("backend"))?;
        }
        record
            .ok_or(SignalProtocolError::InvalidSignedPreKeyId)
            .and_then(wacore_record::signed_prekey_structure_to_record)
    }
    async fn save_signed_pre_key(
        &mut self,
        _id: SignedPreKeyId,
        _record: &SignedPreKeyRecord,
    ) -> Result<(), SignalProtocolError> {
        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl wacore::libsignal::protocol::SenderKeyStore for SenderKeyAdapter {
    async fn store_sender_key(
        &mut self,
        sender_key_name: &SenderKeyName,
        record: wacore::libsignal::protocol::SenderKeyRecord,
    ) -> wacore::libsignal::protocol::error::Result<()> {
        self.0.cache.put_sender_key(sender_key_name, record).await;
        Ok(())
    }

    async fn load_sender_key(
        &self,
        sender_key_name: &SenderKeyName,
    ) -> wacore::libsignal::protocol::error::Result<
        Option<wacore::libsignal::protocol::SenderKeyRecord>,
    > {
        let device = self.0.device();
        // group_decrypt mutates the loaded record (catch-up + ratchet) and stores
        // it back, so the trait needs an owned copy. The cache keeps its `Arc`, so
        // this clones the inner record (unchanged from the prior behavior).
        self.0
            .cache
            .get_sender_key(sender_key_name, &*device.backend)
            .await
            .map(|opt| opt.map(Arc::unwrap_or_clone))
            .map_err(signal_err("backend"))
    }

    async fn sender_key_lock(&self, sender_key_name: &SenderKeyName) -> Arc<async_lock::Mutex<()>> {
        self.0.cache.sender_key_lock(sender_key_name).await
    }

    async fn session_setup_lock(
        &self,
        sender_key_name: &SenderKeyName,
    ) -> Arc<async_lock::Mutex<()>> {
        self.0.cache.session_setup_lock(sender_key_name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::store::in_memory::InMemoryBackend;

    const PREKEY_ID: u32 = 7777;

    async fn test_persistence_manager(
        backend: Arc<dyn crate::store::Backend>,
    ) -> Arc<PersistenceManager> {
        Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("in-memory persistence manager"),
        )
    }

    /// Rotation promotes the new id into the device field and only then drops
    /// its staged backend row, so a snapshot taken before the promotion
    /// resolves that id in neither place. This is why the adapter re-reads the
    /// snapshot before reporting an id unknown: a fresh one resolves what a
    /// stale one cannot, and a decrypt that raced the promotion would otherwise
    /// reject a valid pre-key message.
    #[tokio::test]
    async fn a_promoted_signed_pre_key_is_only_visible_to_a_fresh_snapshot() {
        use wacore::libsignal::protocol::KeyPair;
        use wacore::store::commands::DeviceCommand;

        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let pm = test_persistence_manager(backend).await;

        let stale = pm.get_device_snapshot();
        let promoted_id = stale.signed_pre_key_id + 1;

        // Nothing staged for the new id, and it is not the snapshot's current
        // one: the pre-promotion view cannot resolve it.
        assert!(
            WacoreSignedPreKeyStore::load_signed_prekey(stale.as_ref(), promoted_id)
                .await
                .expect("load")
                .is_none(),
            "the new id must be unresolvable before promotion"
        );

        let key_pair = KeyPair::generate(&mut rand::rng());
        pm.process_command(DeviceCommand::SetSignedPreKey {
            key_pair,
            id: promoted_id,
            signature: [0u8; 64],
            rotation_ms: 0,
        })
        .await;

        // The stale snapshot still cannot resolve it, which is exactly the
        // failure a pinned snapshot would produce.
        assert!(
            WacoreSignedPreKeyStore::load_signed_prekey(stale.as_ref(), promoted_id)
                .await
                .expect("load")
                .is_none(),
            "a stale snapshot must not resolve the promoted id"
        );

        let adapter = SignalProtocolStoreAdapter::new(pm, Arc::new(SignalStoreCache::new()));
        assert!(
            adapter
                .signed_pre_key_store
                .get_signed_pre_key(promoted_id.into())
                .await
                .is_ok(),
            "the adapter must resolve the promoted id from a fresh snapshot"
        );
    }

    /// The window the re-read closes is intra-call: the promotion can land
    /// after the first snapshot is taken and before its backend lookup
    /// resolves. Parks that lookup, promotes, then releases, so the retry is
    /// the only thing that can produce an answer.
    #[tokio::test]
    async fn a_promotion_racing_a_lookup_is_resolved_by_the_retry() {
        use wacore::libsignal::protocol::KeyPair;
        use wacore::store::commands::DeviceCommand;

        let backend = Arc::new(InMemoryBackend::new());
        let pm = test_persistence_manager(backend.clone()).await;
        let promoted_id = pm.get_device_snapshot().signed_pre_key_id + 1;

        // Built before the promotion, so its first snapshot is the stale one.
        let adapter =
            SignalProtocolStoreAdapter::new(pm.clone(), Arc::new(SignalStoreCache::new()));

        let gate = Arc::new(async_lock::Barrier::new(2));
        backend.gate_next_signed_prekey_read(gate.clone());

        let lookup = tokio::spawn(async move {
            adapter
                .signed_pre_key_store
                .get_signed_pre_key(promoted_id.into())
                .await
        });

        // The first load is now parked inside the backend, holding a snapshot
        // that predates everything below.
        gate.wait().await;
        let key_pair = KeyPair::generate(&mut rand::rng());
        pm.process_command(DeviceCommand::SetSignedPreKey {
            key_pair,
            id: promoted_id,
            signature: [0u8; 64],
            rotation_ms: 0,
        })
        .await;
        gate.wait().await;

        assert!(
            lookup.await.expect("lookup task").is_ok(),
            "the retry must resolve an id promoted mid-lookup"
        );
    }

    /// The inbound decrypt path consumes a one-time prekey and buffers it via
    /// `buffer_consumed_prekey`. It must NOT delete the prekey from the backend
    /// synchronously: the promoted session is still volatile at that point, so an
    /// eager backend delete would lose both on a crash. The removal must only be
    /// committed during the session-bearing cache flush.
    #[tokio::test]
    async fn buffer_consumed_prekey_defers_backend_delete_to_flush() {
        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        backend
            .store_prekey(PREKEY_ID, b"durable-prekey", false)
            .await
            .unwrap();

        let device = test_persistence_manager(backend.clone()).await;
        let cache = Arc::new(SignalStoreCache::new());
        let adapter = SignalProtocolStoreAdapter::new(device.clone(), cache.clone());

        let addr = ProtocolAddress::new("bob", 1.into());
        // The real path stores the promoted session before buffering the prekey.
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        adapter
            .pre_key_store
            .buffer_consumed_prekey(PREKEY_ID.into(), &addr)
            .await;

        // Still durable: the removal was only buffered, not written to the backend.
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "buffer_consumed_prekey must not delete from the backend before flush"
        );

        // The flush commits the session AND the buffered prekey removal together.
        cache.flush(backend.as_ref()).await.unwrap();
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "flush must commit the buffered prekey removal"
        );
    }

    /// The plain `remove_pre_key` primitive (not used by the inbound consume path)
    /// removes immediately from the backend.
    #[tokio::test]
    async fn remove_pre_key_deletes_immediately() {
        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        backend
            .store_prekey(PREKEY_ID, b"durable-prekey", false)
            .await
            .unwrap();

        let device = test_persistence_manager(backend.clone()).await;
        let cache = Arc::new(SignalStoreCache::new());
        let mut adapter = SignalProtocolStoreAdapter::new(device.clone(), cache.clone());

        adapter
            .pre_key_store
            .remove_pre_key(PREKEY_ID.into())
            .await
            .unwrap();

        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "remove_pre_key must delete from the backend immediately"
        );
    }

    async fn test_adapter() -> SignalProtocolStoreAdapter {
        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let device = test_persistence_manager(backend).await;
        SignalProtocolStoreAdapter::new(device.clone(), Arc::new(SignalStoreCache::new()))
    }

    struct BlockingIdentityStore {
        pair: IdentityKeyPair,
        entered: Arc<async_lock::Barrier>,
    }

    #[async_trait]
    impl IdentityKeyStore for BlockingIdentityStore {
        async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
            Ok(self.pair.clone())
        }

        async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
            Ok(1)
        }

        async fn save_identity(
            &mut self,
            _address: &ProtocolAddress,
            _identity: &IdentityKey,
        ) -> Result<IdentityChange, SignalProtocolError> {
            Ok(IdentityChange::NewOrUnchanged)
        }

        async fn is_trusted_identity(
            &self,
            _address: &ProtocolAddress,
            _identity: &IdentityKey,
            _direction: Direction,
        ) -> Result<bool, SignalProtocolError> {
            self.entered.wait().await;
            futures::future::pending().await
        }

        async fn get_identity(
            &self,
            _address: &ProtocolAddress,
        ) -> Result<Option<IdentityKey>, SignalProtocolError> {
            Ok(None)
        }
    }

    fn outbound_session() -> (SessionRecord, IdentityKeyPair) {
        use wacore::libsignal::protocol::{
            AliceSignalProtocolParameters, KeyPair, UsePQRatchet, initialize_alice_session_record,
        };

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let local_identity = IdentityKeyPair::generate(&mut rng);
        let remote_identity = IdentityKeyPair::generate(&mut rng);
        let remote_signed_prekey = KeyPair::generate(&mut rng);
        let parameters = AliceSignalProtocolParameters::new(
            local_identity.clone(),
            KeyPair::generate(&mut rng),
            *remote_identity.identity_key(),
            remote_signed_prekey.public_key,
            remote_signed_prekey.public_key,
            UsePQRatchet::No,
        );
        (
            initialize_alice_session_record(&parameters, &mut rng).expect("valid session"),
            local_identity,
        )
    }

    async fn cancel_encrypt_after_checkout(seed_durable: bool) {
        use wacore::libsignal::protocol::message_encrypt;

        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let cache = Arc::new(SignalStoreCache::new());
        let address = ProtocolAddress::new("15550006666", 1.into());
        let (record, identity_pair) = outbound_session();
        let expected = record.serialize().expect("serialize session");
        cache.put_session(&address, record).await;
        if seed_durable {
            cache.flush(backend.as_ref()).await.expect("seed flush");
        }
        assert_eq!(
            wacore::store::traits::SignalStore::get_session(backend.as_ref(), address.as_str())
                .await
                .expect("backend read")
                .is_some(),
            seed_durable,
        );

        let device = test_persistence_manager(backend.clone()).await;
        let mut session_store =
            SignalProtocolStoreAdapter::new(device.clone(), cache.clone()).session_store;
        let entered = Arc::new(async_lock::Barrier::new(2));
        let mut identity_store = BlockingIdentityStore {
            pair: identity_pair,
            entered: entered.clone(),
        };
        let task_address = address.clone();
        let task = tokio::spawn(async move {
            message_encrypt(
                b"cancelled ciphertext",
                &task_address,
                &mut session_store,
                &mut identity_store,
            )
            .await
        });

        tokio::time::timeout(std::time::Duration::from_secs(5), entered.wait())
            .await
            .expect("encrypt reached identity check");
        task.abort();
        assert!(
            task.await
                .expect_err("task must be cancelled")
                .is_cancelled()
        );

        let recovered = cache
            .get_session(&address, backend.as_ref())
            .await
            .expect("cache read")
            .expect("cancelled checkout restored");
        assert_eq!(
            recovered.serialize().expect("serialize recovered session"),
            expected
        );
        cache.put_session(&address, recovered).await;
        cache.flush(backend.as_ref()).await.expect("recovery flush");
        assert!(
            wacore::store::traits::SignalStore::get_session(backend.as_ref(), address.as_str())
                .await
                .expect("backend read")
                .is_some(),
            "the recovered checkout must remain durably flushable"
        );
    }

    #[tokio::test]
    async fn cancelled_encrypt_restores_a_clean_checkout() {
        cancel_encrypt_after_checkout(true).await;
    }

    #[tokio::test]
    async fn cancelled_encrypt_preserves_a_new_dirty_session() {
        cancel_encrypt_after_checkout(false).await;
    }

    #[tokio::test]
    async fn checkout_commit_fails_closed_across_a_lossy_clear() {
        use wacore::libsignal::protocol::SessionCheckout;

        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let cache = Arc::new(SignalStoreCache::new());
        let device = test_persistence_manager(backend).await;
        let mut adapter = SignalProtocolStoreAdapter::new(device.clone(), cache.clone());
        let address = ProtocolAddress::new("15550005555", 1.into());
        cache
            .put_session(&address, SessionRecord::new_fresh())
            .await;

        let checkout = SessionCheckout::load(&mut adapter.session_store, &address)
            .await
            .expect("checkout")
            .expect("session");
        cache.clear().await;
        assert!(matches!(
            checkout.commit().await,
            Err(SignalProtocolError::InvalidState(
                "SessionCheckout::commit",
                _
            ))
        ));
        assert_eq!(cache.try_has_session(&address), None);
    }

    /// The hand-desugared session methods must behave exactly like the trait's
    /// async path: store → visible to has/load, both on cold and warm cache.
    #[tokio::test]
    async fn session_store_fast_paths_round_trip() {
        use wacore::libsignal::protocol::SessionStore as _;
        let mut adapter = test_adapter().await;
        let addr = ProtocolAddress::new("15550002222", 1.into());

        // Cold cache: goes through the async fallback (backend consult).
        assert!(!adapter.session_store.has_session(&addr).await.unwrap());

        adapter
            .session_store
            .store_session(&addr, SessionRecord::new_fresh())
            .await
            .unwrap();

        // Warm cache: answered by the sync fast path.
        assert!(adapter.session_store.has_session(&addr).await.unwrap());
        assert!(
            adapter
                .session_store
                .load_session(&addr)
                .await
                .unwrap()
                .is_some(),
            "stored session must be loadable"
        );
        assert!(
            adapter
                .session_store
                .load_session(&addr)
                .await
                .unwrap()
                .is_some(),
            "plain loads must not consume the cached session"
        );
    }

    /// The hand-desugared identity methods must keep save_identity's change
    /// semantics: new → NewOrUnchanged, same key → NewOrUnchanged, different
    /// key → ReplacedExisting (the last two run on the warm-cache fast path).
    #[tokio::test]
    async fn identity_fast_paths_keep_change_semantics() {
        use wacore::libsignal::protocol::{IdentityKeyPair, IdentityKeyStore as _};
        let mut adapter = test_adapter().await;
        let addr = ProtocolAddress::new("15550003333", 1.into());

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let first = *IdentityKeyPair::generate(&mut rng).identity_key();
        let second = *IdentityKeyPair::generate(&mut rng).identity_key();

        assert!(
            adapter
                .identity_store
                .get_identity(&addr)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            adapter
                .identity_store
                .save_identity(&addr, &first)
                .await
                .unwrap(),
            IdentityChange::NewOrUnchanged
        );
        assert_eq!(
            adapter
                .identity_store
                .save_identity(&addr, &first)
                .await
                .unwrap(),
            IdentityChange::NewOrUnchanged,
            "same key again must be NewOrUnchanged"
        );
        assert_eq!(
            adapter
                .identity_store
                .save_identity(&addr, &second)
                .await
                .unwrap(),
            IdentityChange::ReplacedExisting,
            "a different key must be ReplacedExisting"
        );
        assert_eq!(
            adapter
                .identity_store
                .get_identity(&addr)
                .await
                .unwrap()
                .expect("identity must be cached"),
            second,
            "get_identity must observe the fast-path write"
        );
    }
}

#[cfg(test)]
mod hook_alloc_tests {
    use super::*;
    use crate::test_alloc::min_allocs;
    use wacore::libsignal::protocol::{Direction, IdentityKeyPair};
    use wacore::store::in_memory::InMemoryBackend;

    async fn adapter_for_test() -> SignalProtocolStoreAdapter {
        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let device = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("in-memory persistence manager"),
        );
        SignalProtocolStoreAdapter::new(device.clone(), Arc::new(SignalStoreCache::new()))
    }

    fn some_identity() -> IdentityKey {
        *IdentityKeyPair::generate(&mut rand::rng()).identity_key()
    }

    /// `is_trusted_identity` returns an unconditional `Ok(true)`, but
    /// `#[async_trait]` still boxes that future once per encrypt and once per
    /// decrypt. The hook exists to skip the box, and the only way to know it
    /// does is to count.
    #[tokio::test]
    async fn the_trusted_identity_hook_answers_without_allocating() {
        let adapter = adapter_for_test().await;
        let address = ProtocolAddress::new("bob@s.whatsapp.net", 1.into());
        let identity = some_identity();

        // Through the resolver, not the hook directly: the point is that the
        // whole path allocates nothing, and only the resolver can tell us that.
        // Calling the hook alone would pass even if the resolver ignored it.
        let allocs = min_allocs(0, || {
            futures::FutureExt::now_or_never(wacore::libsignal::protocol::is_trusted_identity(
                &adapter.identity_store,
                &address,
                &identity,
                Direction::Sending,
            ))
            .expect("a hooked store answers on the first poll")
        });
        assert_eq!(
            allocs, 0,
            "the resolver must take the hook; falling through to the boxed future allocates"
        );
    }

    /// Bad path: with nothing cached for the address, the hook must decline so
    /// the caller takes the async path that can read the backend. Answering
    /// from an empty cache would report every identity as new.
    #[tokio::test]
    async fn the_save_identity_hook_declines_when_nothing_is_cached() {
        let mut adapter = adapter_for_test().await;
        let address = ProtocolAddress::new("never-seen@s.whatsapp.net", 1.into());
        let identity = some_identity();

        assert!(
            adapter
                .identity_store
                .try_save_identity(&address, &identity)
                .is_none(),
            "an uncached address has no synchronous answer, so the hook must decline"
        );
    }

    /// Happy path for the same hook: once the entry is cached, the whole
    /// read-compare-write completes synchronously and reports the change
    /// correctly, which is what lets the caller skip the box.
    #[tokio::test]
    async fn the_save_identity_hook_answers_once_the_entry_is_cached() {
        let mut adapter = adapter_for_test().await;
        let address = ProtocolAddress::new("bob@s.whatsapp.net", 1.into());
        let first = some_identity();
        let second = some_identity();

        // Prime the cache through the async path.
        adapter
            .identity_store
            .save_identity(&address, &first)
            .await
            .expect("first save");

        assert!(
            matches!(
                adapter.identity_store.try_save_identity(&address, &first),
                Some(Ok(IdentityChange::NewOrUnchanged))
            ),
            "the same key must report NewOrUnchanged"
        );
        assert!(
            matches!(
                adapter.identity_store.try_save_identity(&address, &second),
                Some(Ok(IdentityChange::ReplacedExisting))
            ),
            "a different key must report ReplacedExisting, or an identity change goes unnoticed"
        );
    }

    /// The session hook has the same contract: decline when the cache cannot
    /// answer, rather than reporting "no session" and forcing a needless
    /// session rebuild.
    #[tokio::test]
    async fn the_session_hook_declines_when_the_cache_cannot_answer() {
        let adapter = adapter_for_test().await;
        let address = ProtocolAddress::new("never-seen@s.whatsapp.net", 1.into());

        assert!(
            adapter.session_store.try_has_session(&address).is_none(),
            "an uncached address has no synchronous answer, so the hook must decline"
        );
    }
}
