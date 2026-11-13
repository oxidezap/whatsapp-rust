mod memory;
mod record;
mod seal;
mod signal;

pub mod conformance;

pub use memory::MemoryRecordStore;
pub use record::{
    Fence, FenceLost, Lease, LeaseStore, Namespace, RecordExists, RecordStore, WriteOp,
    is_fence_lost, is_record_exists,
};
pub use seal::{AesGcmSealer, SealError, Sealer};
pub use signal::RecordSignalStore;

use async_trait::async_trait;
use wacore::store::error::Result as StoreResult;
use wacore::store::transaction::{AuthorityKey, SignalOperationStore};
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<R: RecordStore + 'static, S: Sealer + 'static> SignalOperationStore
    for RecordSignalStore<R, S>
{
    fn authority_key(&self) -> AuthorityKey {
        AuthorityKey::new(
            self.records(),
            self.fence().scope.clone(),
            self.fence().generation,
        )
    }

    async fn load_device_keys(&self) -> StoreResult<Option<Vec<u8>>> {
        self.load_device().await
    }

    async fn save_device_keys(&self, blob: &[u8]) -> StoreResult<()> {
        self.save_device(blob).await
    }

    async fn create_device_keys(&self, blob: &[u8]) -> StoreResult<()> {
        self.create_device(blob).await
    }

    fn staged(&self) -> Self {
        RecordSignalStore::staged(self)
    }

    async fn commit(&self) -> StoreResult<()> {
        RecordSignalStore::commit(self).await
    }

    async fn load_aux(&self, ns: Namespace, key: &str) -> StoreResult<Option<Vec<u8>>> {
        RecordSignalStore::load_aux(self, ns, key).await
    }

    async fn put_aux(&self, ns: Namespace, key: &str, value: &[u8]) -> StoreResult<()> {
        RecordSignalStore::put_aux(self, ns, key, value).await
    }

    async fn insert_aux(&self, ns: Namespace, key: &str, value: &[u8]) -> StoreResult<()> {
        RecordSignalStore::insert_aux(self, ns, key, value).await
    }

    async fn insert_prekeys_batch(&self, keys: &[(u32, bytes::Bytes)]) -> StoreResult<()> {
        RecordSignalStore::insert_prekeys_batch(self, keys).await
    }

    async fn delete_aux(&self, ns: Namespace, keys: &[&str]) -> StoreResult<()> {
        RecordSignalStore::delete_aux(self, ns, keys).await
    }

    async fn scan_aux(&self, ns: Namespace, prefix: &str) -> StoreResult<Vec<String>> {
        RecordSignalStore::scan_aux(self, ns, prefix, None).await
    }

    async fn scan_peer_addresses(&self, ns: Namespace, prefix: &str) -> StoreResult<Vec<String>> {
        RecordSignalStore::scan_peer_addresses(self, ns, prefix, None).await
    }
}
