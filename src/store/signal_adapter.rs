use crate::store::Device;
use crate::store::signal_cache::SignalStoreCache;
use async_lock::RwLock;
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

#[derive(Clone)]
struct SharedDevice {
    device: Arc<RwLock<Device>>,
    cache: Arc<SignalStoreCache>,
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
    pub fn new(device: Arc<RwLock<Device>>, cache: Arc<SignalStoreCache>) -> Self {
        Self(SharedDevice { device, cache })
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
    pub fn new(device: Arc<RwLock<Device>>, cache: Arc<SignalStoreCache>) -> Self {
        let shared = SharedDevice { device, cache };
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
        let device = self.0.device.read().await;
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
        let device = self.0.device.read().await;
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
        if let Some(known) = self.0.cache.try_has_session(address) {
            return Box::pin(ready(Ok(known)));
        }
        Box::pin(async move {
            let device = self.0.device.read().await;
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

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl IdentityKeyStore for IdentityAdapter {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
        let device = self.0.device.read().await;
        IdentityKeyStore::get_identity_key_pair(&*device)
            .await
            .map_err(signal_err("get_identity_key_pair"))
    }

    async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
        let device = self.0.device.read().await;
        IdentityKeyStore::get_local_registration_id(&*device)
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
        if let Some(prev) = self.0.cache.try_get_identity(address) {
            let change = match parse_cached_identity(prev) {
                Ok(None) => IdentityChange::NewOrUnchanged,
                Ok(Some(existing)) if &existing == identity => IdentityChange::NewOrUnchanged,
                Ok(Some(_)) => IdentityChange::ReplacedExisting,
                Err(e) => return Box::pin(ready(Err(e))),
            };
            if self
                .0
                .cache
                .try_put_identity(address, identity.public_key().public_key_bytes())
            {
                return Box::pin(ready(Ok(change)));
            }
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
            let device = self.0.device.read().await;
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
        let device = self.0.device.read().await;
        WacorePreKeyStore::load_prekey(&*device, prekey_id.into())
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
        let device = self.0.device.read().await;
        let structure = wacore_record::prekey_record_to_structure(record)?;
        WacorePreKeyStore::store_prekey(&*device, prekey_id.into(), structure, false)
            .await
            .map_err(signal_err("backend"))
    }
    async fn remove_pre_key(&mut self, prekey_id: PreKeyId) -> Result<(), SignalProtocolError> {
        // Plain immediate-removal primitive. The inbound pkmsg path does NOT route
        // through here: message_decrypt reports the consumed prekey and the receive
        // path buffers it via buffer_consumed_prekey so the durable delete is
        // atomic with the session flush (matching WAWebSignalProtocolStoreUnifiedApi).
        let device = self.0.device.read().await;
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
        let device = self.0.device.read().await;
        WacoreSignedPreKeyStore::load_signed_prekey(&*device, signed_prekey_id.into())
            .await
            .map_err(signal_err("backend"))?
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
        let device = self.0.device.read().await;
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
    use crate::store::Device;
    use wacore::store::in_memory::InMemoryBackend;

    const PREKEY_ID: u32 = 7777;

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

        let device = Arc::new(RwLock::new(Device::new(backend.clone())));
        let cache = Arc::new(SignalStoreCache::new());
        let adapter = SignalProtocolStoreAdapter::new(device, cache.clone());

        let addr = ProtocolAddress::new("bob".to_string(), 1.into());
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

        let device = Arc::new(RwLock::new(Device::new(backend.clone())));
        let cache = Arc::new(SignalStoreCache::new());
        let mut adapter = SignalProtocolStoreAdapter::new(device, cache.clone());

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

    fn test_adapter() -> SignalProtocolStoreAdapter {
        let backend: Arc<dyn crate::store::Backend> = Arc::new(InMemoryBackend::new());
        let device = Arc::new(RwLock::new(Device::new(backend)));
        SignalProtocolStoreAdapter::new(device, Arc::new(SignalStoreCache::new()))
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
        let address = ProtocolAddress::new("15550006666".to_string(), 1.into());
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

        let device = Arc::new(RwLock::new(Device::new(backend.clone())));
        let mut session_store =
            SignalProtocolStoreAdapter::new(device, cache.clone()).session_store;
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
        let device = Arc::new(RwLock::new(Device::new(backend)));
        let mut adapter = SignalProtocolStoreAdapter::new(device, cache.clone());
        let address = ProtocolAddress::new("15550005555".to_string(), 1.into());
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
        let mut adapter = test_adapter();
        let addr = ProtocolAddress::new("15550002222".to_string(), 1.into());

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
        let mut adapter = test_adapter();
        let addr = ProtocolAddress::new("15550003333".to_string(), 1.into());

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
