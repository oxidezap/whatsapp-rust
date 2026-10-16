//! A downstream backend implements only required Signal methods. Default batch
//! methods, cache persistence and error chains are exercised through public APIs.
#![cfg(all(feature = "native", not(target_arch = "wasm32")))]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use wacore::{
    libsignal::protocol::{ProtocolAddress, SessionRecord},
    store::{
        InMemoryBackend, SignalStoreCache,
        error::{Result, StoreError},
        traits::SignalStore,
    },
};
use whatsapp_rust::{async_trait, bytes::Bytes};

#[derive(Debug)]
struct BackendFailure;
impl std::fmt::Display for BackendFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("synthetic write failure")
    }
}
impl std::error::Error for BackendFailure {}

#[derive(Default)]
struct ExternalSignalStore {
    inner: InMemoryBackend,
    sessions: std::sync::Mutex<std::collections::HashMap<String, Bytes>>,
    fail_writes: AtomicBool,
}
#[async_trait]
impl SignalStore for ExternalSignalStore {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()> {
        self.inner.put_identity(address, key).await
    }
    async fn load_identity(&self, address: &str) -> Result<Option<[u8; 32]>> {
        self.inner.load_identity(address).await
    }
    async fn delete_identity(&self, address: &str) -> Result<()> {
        self.inner.delete_identity(address).await
    }
    async fn get_session(&self, address: &str) -> Result<Option<Bytes>> {
        Ok(self.sessions.lock().unwrap().get(address).cloned())
    }
    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()> {
        if self.fail_writes.load(Ordering::SeqCst) && address != "allowed" {
            return Err(StoreError::Database(Box::new(BackendFailure)));
        }
        self.sessions
            .lock()
            .unwrap()
            .insert(address.to_owned(), Bytes::copy_from_slice(session));
        Ok(())
    }
    async fn delete_session(&self, address: &str) -> Result<()> {
        self.sessions.lock().unwrap().remove(address);
        Ok(())
    }
    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> Result<()> {
        self.inner.store_prekey(id, record, uploaded).await
    }
    async fn load_prekey(&self, id: u32) -> Result<Option<Bytes>> {
        self.inner.load_prekey(id).await
    }
    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<()> {
        self.inner.mark_prekeys_uploaded(ids).await
    }
    async fn remove_prekey(&self, id: u32) -> Result<()> {
        self.inner.remove_prekey(id).await
    }
    async fn get_max_prekey_id(&self) -> Result<u32> {
        self.inner.get_max_prekey_id().await
    }
    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()> {
        self.inner.store_signed_prekey(id, record).await
    }
    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        self.inner.load_signed_prekey(id).await
    }
    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>> {
        self.inner.load_all_signed_prekeys().await
    }
    async fn remove_signed_prekey(&self, id: u32) -> Result<()> {
        self.inner.remove_signed_prekey(id).await
    }
    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()> {
        self.inner.put_sender_key(address, record).await
    }
    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>> {
        self.inner.get_sender_key(address).await
    }
    async fn delete_sender_key(&self, address: &str) -> Result<()> {
        self.inner.delete_sender_key(address).await
    }
}

#[test]
fn failed_flush_preserves_source_and_can_be_retried_by_the_consumer() {
    futures::executor::block_on(async {
        let store = ExternalSignalStore::default();
        let cache = SignalStoreCache::new();
        let address = ProtocolAddress::new("15550001111", 1.into());
        let record = SessionRecord::new_fresh();
        let expected = record.serialize().unwrap();
        cache.put_session(&address, record).await;
        assert!(store.get_session(address.as_str()).await.unwrap().is_none());
        store.fail_writes.store(true, Ordering::SeqCst);
        let error = cache.flush(&store).await.unwrap_err();
        let typed = error
            .downcast_ref::<StoreError>()
            .expect("typed store error");
        assert!(matches!(typed, StoreError::Database(_)));
        assert!(
            std::error::Error::source(typed)
                .unwrap()
                .is::<BackendFailure>()
        );
        assert!(store.get_session(address.as_str()).await.unwrap().is_none());
        store.fail_writes.store(false, Ordering::SeqCst);
        cache.flush(&store).await.unwrap();
        let persisted = store.get_session(address.as_str()).await.unwrap().unwrap();
        assert_eq!(persisted.as_ref(), expected);
        let cold_cache = SignalStoreCache::new();
        assert!(
            cold_cache
                .peek_session(&address, &store)
                .await
                .unwrap()
                .is_some()
        );
        cache.flush(&store).await.unwrap();
        assert_eq!(
            store.get_session(address.as_str()).await.unwrap().unwrap(),
            persisted
        );
    });
}

#[test]
fn external_backend_defaults_preserve_missing_rows_and_conservative_scan() {
    futures::executor::block_on(async {
        let store = ExternalSignalStore::default();
        let first: Arc<str> = Arc::from("15550001111:1");
        let missing: Arc<str> = Arc::from("15550002222:1");
        store
            .put_sessions_batch(&[(first.clone(), Bytes::from_static(b"opaque"))])
            .await
            .unwrap();
        let rows = store
            .get_sessions_batch(&[missing.clone(), first.clone()])
            .await
            .unwrap();
        assert_eq!(rows, vec![(first.clone(), Bytes::from_static(b"opaque"))]);
        assert!(!store.has_session(&missing).await.unwrap());
        assert!(store.has_signal_state_for_user("unseen").await.unwrap());
        store
            .delete_sessions_batch(std::slice::from_ref(&first))
            .await
            .unwrap();
        store
            .delete_sessions_batch(std::slice::from_ref(&first))
            .await
            .unwrap();
        assert!(!store.has_session(&first).await.unwrap());
    });
}

#[test]
fn unsupported_device_operations_are_inspectable_without_parsing_messages() {
    use wacore::libsignal::store::{SessionStore as _, SignedPreKeyStore as _};
    futures::executor::block_on(async {
        let device = whatsapp_rust::store::Device::new(Arc::new(InMemoryBackend::new()));
        for error in [
            device
                .get_sub_device_sessions("15550001111")
                .await
                .unwrap_err(),
            device.delete_all_sessions("15550001111").await.unwrap_err(),
            device.load_signed_prekeys().await.unwrap_err(),
        ] {
            let io = error
                .downcast_ref::<std::io::Error>()
                .expect("typed unsupported operation");
            assert_eq!(io.kind(), std::io::ErrorKind::Unsupported);
        }
    });
}

#[test]
fn adapter_distinguishes_missing_prekeys_from_malformed_persisted_records() {
    use wacore::libsignal::protocol::{PreKeyStore as _, SignalProtocolError};
    use whatsapp_rust::store::{
        persistence_manager::PersistenceManager, signal_adapter::SignalProtocolStoreAdapter,
    };
    futures::executor::block_on(async {
        let backend = Arc::new(InMemoryBackend::new());
        // An empty protobuf decodes but lacks the required key material.
        backend.store_prekey(41, b"", false).await.unwrap();
        let manager = Arc::new(PersistenceManager::new(backend).await.unwrap());
        let adapter = SignalProtocolStoreAdapter::new(manager, Arc::new(SignalStoreCache::new()));
        let corrupt = adapter
            .pre_key_store
            .get_pre_key(41.into())
            .await
            .err()
            .expect("invalid stored key");
        assert!(matches!(corrupt, SignalProtocolError::BackendError(..)));
        let source = std::error::Error::source(&corrupt).expect("stored record cause");
        assert!(source.is::<SignalProtocolError>());
        let missing = adapter
            .pre_key_store
            .get_pre_key(42.into())
            .await
            .err()
            .expect("missing key");
        assert!(matches!(missing, SignalProtocolError::InvalidPreKeyId));
    });
}

#[test]
fn default_batch_failure_can_leave_a_committed_prefix() {
    futures::executor::block_on(async {
        let store = ExternalSignalStore::default();
        store.fail_writes.store(true, Ordering::SeqCst);
        let rows = ["allowed", "rejected", "unreached"]
            .map(|address| (Arc::<str>::from(address), Bytes::from_static(b"opaque")));
        let error = store.put_sessions_batch(&rows).await.unwrap_err();
        assert!(matches!(error, StoreError::Database(_)));
        assert!(store.has_session("allowed").await.unwrap());
        assert!(!store.has_session("rejected").await.unwrap());
        assert!(!store.has_session("unreached").await.unwrap());
        store.fail_writes.store(false, Ordering::SeqCst);
        store.put_sessions_batch(&rows).await.unwrap();
        assert_eq!(
            store
                .get_sessions_batch(&rows.map(|(address, _)| address))
                .await
                .unwrap()
                .len(),
            3
        );
    });
}

#[test]
fn exhausted_contention_remains_classifiable_by_portable_consumers() {
    let error = StoreError::RetriesExhausted {
        op: "external_write".into(),
        attempts: 6,
        source: Box::new(BackendFailure),
    };
    assert!(error.is_database_busy_or_locked());
    match error {
        StoreError::RetriesExhausted { op, attempts, .. } => {
            assert_eq!(op, "external_write");
            assert_eq!(attempts, 6);
        }
        _ => panic!("exhaustion classification changed"),
    }
}

#[test]
fn typed_permanent_errors_are_not_contention() {
    for error in [
        StoreError::Validation("busy worker count is invalid".into()),
        StoreError::InvalidConfig("locked mode requires a key".into()),
        StoreError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "database is locked by policy",
        )),
    ] {
        assert!(!error.is_database_busy_or_locked(), "{error:?}");
    }
}

#[test]
fn exhausted_contention_preserves_the_backend_cause() {
    let error = StoreError::RetriesExhausted {
        op: "external_write".into(),
        attempts: 6,
        source: Box::new(BackendFailure),
    };
    let cause = std::error::Error::source(&error).expect("exhaustion lost its backend cause");
    assert!(cause.is::<BackendFailure>());
    assert!(!error.to_string().contains("synthetic write failure"));
}

#[derive(Debug)]
struct OpaqueContext(StoreError);
impl std::fmt::Display for OpaqueContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("opaque busy context")
    }
}
impl std::error::Error for OpaqueContext {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn typed_causes_override_opaque_context_messages() {
    let permanent = StoreError::Connection(Box::new(OpaqueContext(StoreError::Validation(
        "invalid worker count".into(),
    ))));
    assert!(!permanent.is_database_busy_or_locked());
    let contention = StoreError::Database(Box::new(StoreError::RetriesExhausted {
        op: "save".into(),
        attempts: 3,
        source: Box::new(BackendFailure),
    }));
    assert!(contention.is_database_busy_or_locked());
}

#[test]
fn opaque_database_causes_keep_the_legacy_text_fallback() {
    for message in ["database is LOCKED", "SQLITE_BUSY: write contention"] {
        for error in [
            StoreError::Database(Box::new(std::io::Error::other(message))),
            StoreError::Connection(Box::new(std::io::Error::other(message))),
        ] {
            assert!(error.is_database_busy_or_locked());
        }
    }
    let permanent = StoreError::Database(Box::new(BackendFailure));
    assert!(!permanent.is_database_busy_or_locked());
    let nested = StoreError::Connection(Box::new(OpaqueContext(StoreError::Database(Box::new(
        BackendFailure,
    )))));
    assert!(nested.is_database_busy_or_locked());
    // Opaque messages cannot distinguish contention from unrelated wording.
    let ambiguous = StoreError::Database(Box::new(std::io::Error::other(
        "busy worker count is invalid",
    )));
    assert!(ambiguous.is_database_busy_or_locked());
}
