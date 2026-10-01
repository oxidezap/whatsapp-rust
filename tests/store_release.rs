//! Small downstream-style ownership/async_trait consumers of the additive API.

use std::sync::Arc;
use whatsapp_rust::wacore::runtime::BoxFuture;
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::{
    Client, StoreRelease, async_trait, store::persistence_manager::PersistenceManager,
};

fn boxed_client_release(client: &Client) -> BoxFuture<'static, ()> {
    Box::pin(client.store_release().wait())
}

#[async_trait]
trait Released {
    async fn released(&self);
}

#[async_trait]
impl Released for StoreRelease {
    async fn released(&self) {
        self.wait().await;
    }
}

#[tokio::test]
async fn boxed_static_send_and_async_trait_consumers() {
    fn assert_thread_safe<T: Send + Sync + 'static>() {}
    assert_thread_safe::<StoreRelease>();
    let _: fn(&Client) -> BoxFuture<'static, ()> = boxed_client_release;
    let backend: Arc<dyn whatsapp_rust::store::Backend> = Arc::new(InMemoryBackend::new());
    let manager = PersistenceManager::new(backend.clone()).await.unwrap();
    let snapshot = manager.get_device_snapshot();
    // Existing downstream struct literals and the original Arc identity survive.
    let device = whatsapp_rust::store::Device {
        core: snapshot.core.clone(),
        backend: backend.clone(),
    };
    assert!(Arc::ptr_eq(&backend, &manager.backend()));
    assert!(Arc::ptr_eq(&backend, &device.backend));
    use whatsapp_rust::prelude::StoreRelease as PreludeStoreRelease;
    let release: PreludeStoreRelease = manager.store_release();
    let mut boxed: BoxFuture<'static, ()> = Box::pin(release.wait());
    assert!(
        futures::poll!(boxed.as_mut()).is_pending(),
        "release fence fired while the manager still owns its backend lease"
    );
    drop(manager);
    tokio::time::timeout(std::time::Duration::from_secs(5), boxed)
        .await
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        Released::released(&release),
    )
    .await
    .unwrap();
    // Raw host-owned Device/snapshot/backend handles are deliberately outside
    // the crate's release fence, not transformed or silently closed.
    drop((device, snapshot, backend));
}
