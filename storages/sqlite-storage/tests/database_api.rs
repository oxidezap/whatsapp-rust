// Native filesystem fixtures and Tokio test runtime; the separate wasm timer
// harness exercises the portable retry path without pretending to run SQLite VFS.
#![cfg(not(target_family = "wasm"))]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use diesel::prelude::*;
use wacore::store::error::StoreError;
use wacore::store::traits::{Backend, DeviceStore, ProtocolStore, SignalStore};
use whatsapp_rust_sqlite_storage::{
    CommitBarrierHook, SqliteDatabase, SqliteDatabaseConfig, SqliteStore, Synchronous,
};

// Each destructive test owns a new database and its sidecars. No real accounts.
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        Self(std::env::temp_dir().join(format!(
            "wa_database_api_{}_{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }
    fn url(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn db_err(e: diesel::result::Error) -> StoreError {
    StoreError::Database(Box::new(e))
}

#[derive(QueryableByName)]
struct Settings {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    cache_size: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    synchronous: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    busy_timeout: i64,
}

async fn settings(db: &SqliteDatabase) -> Settings {
    db.shared()
        .read(|conn| {
            diesel::sql_query("SELECT c.cache_size, s.synchronous, b.timeout AS busy_timeout FROM pragma_cache_size() c, pragma_synchronous() s, pragma_busy_timeout() b")
                .get_result(conn)
                .map_err(db_err)
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn single_account_defaults_and_custom_configuration() {
    let fixture = Fixture::new();
    let store = SqliteStore::new(&fixture.url()).await.unwrap();
    assert_eq!(store.device_id(), 1);
    assert!(!store.exists().await.unwrap());
    assert_eq!(store.create().await.unwrap(), 1);
    assert!(store.load().await.unwrap().is_some());
    let defaults = SqliteDatabaseConfig::default();
    assert_eq!(defaults.pool_size, 1);
    assert_eq!(defaults.read_pool_size, 1);
    assert_eq!(defaults.cache_size_kib, 512);
    let actual = settings(&store.database()).await;
    assert_eq!(actual.cache_size, -512);
    assert_eq!(actual.synchronous, 1);
    assert_eq!(actual.busy_timeout, 30_000);

    let fixture = Fixture::new();
    let custom = SqliteDatabaseConfig {
        cache_size_kib: 128,
        synchronous: Synchronous::Full,
        busy_timeout: std::time::Duration::from_millis(75),
        ..Default::default()
    };
    let store = SqliteStore::open(&fixture.url(), custom).await.unwrap();
    let actual = settings(&store.database()).await;
    assert_eq!(actual.cache_size, -128);
    assert_eq!(actual.synchronous, 2);
    assert_eq!(actual.busy_timeout, 75);
}

#[tokio::test]
async fn shared_accounts_and_selected_administration() {
    let fixture = Fixture::new();
    let db = SqliteDatabase::open(&fixture.url(), Default::default())
        .await
        .unwrap();
    assert!(db.list_devices().await.unwrap().is_empty());
    let a = db.provision_device(1).await.unwrap();
    let b = db.create_device().await.unwrap();
    let b_id = b.device_id();
    assert_ne!(a.device_id(), b_id);
    // Already provisioned is an error, never an implicit destructive reset.
    assert!(db.provision_device(1).await.is_err());
    assert_eq!(
        db.list_devices()
            .await
            .unwrap()
            .iter()
            .map(|d| d.id)
            .collect::<Vec<_>>(),
        vec![1, b_id]
    );

    for (store, bytes) in [(&a, b"first".as_slice()), (&b, b"second".as_slice())] {
        store.put_session("15550000001.1", bytes).await.unwrap();
        store
            .put_identity("15550000001.1", [store.device_id() as u8; 32])
            .await
            .unwrap();
        store
            .put_group_metadata("12000000001@g.us", bytes)
            .await
            .unwrap();
    }
    let reopened = db.store(b_id);
    assert_eq!(
        reopened
            .get_session("15550000001.1")
            .await
            .unwrap()
            .unwrap(),
        b"second".as_slice()
    );
    assert_eq!(
        a.get_session("15550000001.1").await.unwrap().unwrap(),
        b"first".as_slice()
    );
    assert_ne!(
        a.load_identity("15550000001.1").await.unwrap(),
        b.load_identity("15550000001.1").await.unwrap()
    );
    let old_identity = b.load().await.unwrap().unwrap().identity_key.public_key;
    let reset = db.reset_device(b_id).await.unwrap();
    assert_eq!(reset.device_id(), b_id);
    assert_ne!(
        reset.load().await.unwrap().unwrap().identity_key.public_key,
        old_identity
    );
    assert!(reset.get_session("15550000001.1").await.unwrap().is_none());
    assert!(
        reset
            .get_group_metadata("12000000001@g.us")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        reset
            .load_identity("15550000001.1")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        a.get_group_metadata("12000000001@g.us")
            .await
            .unwrap()
            .unwrap(),
        b"first"
    );
    // Existing handles still address the same id after reset (not revoked).
    assert!(b.exists().await.unwrap());
    db.remove_device(b_id).await.unwrap();
    assert!(!b.exists().await.unwrap());
    assert!(a.exists().await.unwrap());
    assert!(
        matches!(db.reset_device(b_id).await, Err(StoreError::DeviceNotFound(id)) if id == b_id)
    );
    assert!(
        matches!(db.remove_device(b_id).await, Err(StoreError::DeviceNotFound(id)) if id == b_id)
    );
    let next = db.create_device().await.unwrap();
    assert!(next.device_id() > b_id);
    assert_eq!(db.list_devices().await.unwrap().len(), 2);
}

#[tokio::test]
async fn memory_uri_and_scope_lifetime() {
    let fixture = Fixture::new();
    let uri = format!("file:{}?mode=memory&cache=shared", fixture.url());
    let db = SqliteDatabase::open(&uri, Default::default())
        .await
        .unwrap();
    let a = db.provision_device(1).await.unwrap();
    let b = db.create_device().await.unwrap();
    a.put_session("15550000001.1", b"memory").await.unwrap();
    assert!(b.get_session("15550000001.1").await.unwrap().is_none());
    let reopened = db.store(1);
    drop(db);
    drop(a);
    assert_eq!(
        reopened
            .get_session("15550000001.1")
            .await
            .unwrap()
            .unwrap(),
        b"memory".as_slice()
    );
    assert!(!fixture.0.exists(), "URI must not become a disk file");
}

#[tokio::test]
async fn pool_reporting_and_barrier_are_not_per_account() {
    let fixture = Fixture::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let hook_calls = calls.clone();
    let barrier: CommitBarrierHook = Arc::new(move || {
        hook_calls.fetch_add(1, Ordering::Relaxed);
        Box::pin(async { Ok(()) })
    });
    let db = SqliteDatabase::open(
        &fixture.url(),
        SqliteDatabaseConfig::default().with_commit_barrier(barrier),
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let a = db.provision_device(1).await.unwrap();
    let b = db.create_device().await.unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 3);
    let _ = db.store(99);
    assert!(!db.device_exists(99).await.unwrap());
    assert_eq!(
        calls.load(Ordering::Relaxed),
        3,
        "selection and reads never commit"
    );
    let pool = db.resource_report().await;
    assert!(pool.memory_bytes.unwrap() > 0);
    assert_eq!(a.scoped_resource_report().memory_bytes, Some(0));
    assert_eq!(b.scoped_resource_report().memory_bytes, Some(0));
    assert!(a.scoped_resource_report().pages.is_none());
    assert_eq!(
        DeviceStore::resource_report(&a).await.memory_bytes,
        pool.memory_bytes
    );
    // External consumer: no SqliteDatabase requirement added to Backend.
    fn backend(store: SqliteStore) -> Arc<dyn Backend> {
        Arc::new(store)
    }
    assert!(backend(b).exists().await.unwrap());
}

#[tokio::test]
async fn open_errors_preserve_source_chain() {
    let config = SqliteDatabaseConfig::default().with_commit_barrier(Arc::new(|| {
        Box::pin(async { Err(StoreError::Validation("fixture durability failure".into())) })
    }));
    let fixture = Fixture::new();
    let result = SqliteDatabase::open(&fixture.url(), config).await;
    let error = result.err().expect("barrier must reject open");
    let source = std::error::Error::source(&error).unwrap();
    assert!(
        source
            .downcast_ref::<whatsapp_rust_sqlite_storage::CommitBarrierError>()
            .is_some()
    );
    assert!(std::error::Error::source(source).is_some());
}
