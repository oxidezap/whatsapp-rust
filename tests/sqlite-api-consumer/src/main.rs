//! Compile/size probe: retains the boxed opening future, never opens a live account.
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use whatsapp_rust::store::traits::{Backend, DeviceStore};
use whatsapp_rust::store::{SqliteDatabase, SqliteDatabaseConfig, SqliteStore};
use whatsapp_rust::wacore::store::error::{Result, StoreError};

async fn capabilities(url: &str) -> Result<Arc<dyn Backend>> {
    let simple = SqliteStore::open(url).await?;
    assert_eq!(simple.device_id(), 1);
    // Creating the database facade from an existing store opens no connections.
    let db: SqliteDatabase = simple.database();
    let _ = db.resource_report().await;
    assert_eq!(simple.scoped_resource_report().memory_bytes, Some(0));
    let adapter = db.shared();
    adapter.read(|_| Ok(())).await?;
    let account = db.provision_device(1).await?;
    assert!(account.exists().await?);
    let second = db.create_device().await?;
    let selected = db.store(second.device_id());
    assert!(db.device_exists(selected.device_id()).await?);
    assert_eq!(db.list_devices().await?.len(), 2);
    drop(db);
    adapter.run(|_| Ok(())).await?;
    assert!(selected.exists().await?);
    Ok(Arc::new(account))
}

// DeviceStore exposes local futures on wasm32; retain the Send proof on native.
#[cfg(not(target_arch = "wasm32"))]
type OpeningFuture = Pin<Box<dyn Future<Output = Result<Arc<dyn Backend>>> + Send>>;
#[cfg(target_arch = "wasm32")]
type OpeningFuture = Pin<Box<dyn Future<Output = Result<Arc<dyn Backend>>>>>;

fn main() {
    // Type erasure also proves downstream boxed-future/trait-object compatibility.
    // A vtable retains poll code for both native and WASM release measurements.
    let future: OpeningFuture = Box::pin(capabilities(
        "file:sqlite_external_probe?mode=memory&cache=shared",
    ));
    drop(std::hint::black_box(future));
    let config = SqliteDatabaseConfig::default().with_read_pool_size(0);
    drop(std::hint::black_box(SqliteDatabase::open(
        "unused.db",
        config,
    )));
    let _ = std::mem::size_of::<StoreError>();
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn host_can_use_scoped_backends_and_database_wide_adapters() {
        let backend = capabilities("file:sqlite_external_test?mode=memory&cache=shared")
            .await
            .unwrap();
        assert!(backend.exists().await.unwrap());
    }
}
