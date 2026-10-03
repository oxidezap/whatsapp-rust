//! Database ownership, opening and account administration.

use wacore::stats::StorageResourceReport;
use wacore::store::error::Result;
use wacore::store::traits::DeviceStore;

use crate::{SharedSqlite, SqliteDatabaseConfig, SqliteStore, StoredDeviceSummary};

/// A shared SQLite database, independent of the account selected by a store.
///
/// Open once, then obtain cheap, fixed-device handles with [`Self::store`].
/// Opening runs the existing migrations but does **not** provision an account.
/// Stores keep the connections alive even after this handle is dropped.
/// This type is not a `Backend`: domain traits belong to [`SqliteStore`].
///
/// ```no_run
/// use whatsapp_rust_sqlite_storage::{SqliteDatabase, SqliteDatabaseConfig};
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let db = SqliteDatabase::open("whatsapp.db", SqliteDatabaseConfig::default()).await?;
/// let account = db.create_device().await?;
/// let id = account.device_id();
/// let reopened = db.store(id); // no new connections, no new account
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct SqliteDatabase {
    // Reuse the proven pool/permit/barrier plumbing. The template's device id
    // is never used for account administration or exposed as database identity.
    pub(crate) template: SqliteStore,
}

impl SqliteDatabase {
    /// Canonical opening path. Defaults: one writer, one reader when WAL/private
    /// cache/native support it, 512 KiB cache per connection, Normal sync.
    pub async fn open(database_url: &str, config: SqliteDatabaseConfig) -> Result<Self> {
        Ok(Self {
            template: SqliteStore::build(database_url, 1, config).await?,
        })
    }

    /// Select a fixed device id without checking existence or provisioning it.
    /// All stores obtained here share the writer/read pools, permits and barrier.
    /// Selection opens no connections. Writer concurrency follows `pool_size`;
    /// `read_pool_size` widens only the read side. Count resource reports once
    /// per database, not once per selected account.
    pub fn store(&self, device_id: i32) -> SqliteStore {
        self.template.share_for_device(device_id)
    }

    /// All accounts in this database, ordered by id (not just one store's scope).
    pub async fn list_devices(&self) -> Result<Vec<StoredDeviceSummary>> {
        self.template.list_devices_impl().await
    }

    /// Allocate and provision a fresh account atomically. Its id is available via
    /// [`SqliteStore::device_id`]; the returned store shares this database's pools.
    /// SQLite allocates the id with `AUTOINCREMENT` in the insert transaction,
    /// so concurrent creates cannot collide and removed ids are not reissued.
    pub async fn create_device(&self) -> Result<SqliteStore> {
        self.template
            .create_sibling_device_impl()
            .await
            .map(|(_, store)| store)
    }

    /// Provision exactly `device_id`, for the single-account/restore path.
    /// An existing row is an insert error, not a reset or an idempotent ensure.
    pub async fn provision_device(&self, device_id: i32) -> Result<SqliteStore> {
        let store = self.store(device_id);
        store.create_new_device().await?;
        Ok(store)
    }

    /// Whether the selected device row exists. Does not provision anything.
    pub async fn device_exists(&self, device_id: i32) -> Result<bool> {
        self.template.device_exists_impl(device_id).await
    }

    /// Purge only this account's state and recreate its row with fresh keys under
    /// the same id, in one transaction. Missing id is `DeviceNotFound`.
    ///
    /// Stop that account's background work first: existing handles are **not**
    /// invalidated and their subsequent writes target the recreated account.
    pub async fn reset_device(&self, device_id: i32) -> Result<SqliteStore> {
        self.template.reset_device_impl(device_id).await
    }

    /// Purge only this account's state and remove its row atomically. Allocated
    /// ids are not reissued. Missing id is `DeviceNotFound`.
    ///
    /// Stop that account's background work first: old handles are **not** revoked
    /// and can recreate rows with subsequent writes. No other account is stopped.
    pub async fn remove_device(&self, device_id: i32) -> Result<()> {
        self.template.remove_device_impl(device_id).await
    }

    /// Advanced, database-wide access for adapters with their own tables.
    /// It carries the same pools, semaphores and commit barrier; it is not
    /// device-scoped. Adapters must enforce their own account predicates.
    pub fn shared(&self) -> SharedSqlite {
        self.template.shared()
    }

    /// Whole-pool page-cache estimate and database-wide disk/page statistics.
    /// Count once per opened database, not once per device or database clone.
    /// Best-effort: contention or failed introspection yields absent fields.
    /// The legacy `DeviceStore::resource_report` on stores reports this same pool.
    pub async fn resource_report(&self) -> StorageResourceReport {
        DeviceStore::resource_report(&self.template).await
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn wal_scopes_share_writer_reader_and_barrier() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                for suffix in ["", "-wal", "-shm"] {
                    let mut path = self.0.clone().into_os_string();
                    path.push(suffix);
                    let _ = std::fs::remove_file(path);
                }
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
            "wa_database_wal_{}_{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        let barrier: crate::CommitBarrierHook = Arc::new(|| Box::pin(async { Ok(()) }));
        let config = SqliteDatabaseConfig {
            pool_size: 2,
            read_pool_size: 2,
            ..Default::default()
        }
        .with_commit_barrier(barrier.clone());
        let db = SqliteDatabase::open(&fixture.0.to_string_lossy(), config)
            .await
            .unwrap();
        let a = db.store(1);
        let b = db.store(2);
        assert!(Arc::ptr_eq(&a.db_semaphore, &b.db_semaphore));
        assert_eq!(a.db_semaphore.available_permits(), 2);
        let (ar, br) = (a.reads.as_ref().unwrap(), b.reads.as_ref().unwrap());
        assert!(Arc::ptr_eq(&ar.semaphore, &br.semaphore));
        assert_eq!(ar.semaphore.available_permits(), 2);
        assert!(a.snapshot_safe);
        assert!(Arc::ptr_eq(a.commit_barrier.as_ref().unwrap(), &barrier));
        assert!(Arc::ptr_eq(b.commit_barrier.as_ref().unwrap(), &barrier));
    }

    // A connection-init hook also witnesses that scope selection never opens
    // another connection, even when default readers are declined by the URI.
    #[tokio::test]
    async fn scopes_share_permits_and_memory_uri_declines_readers() {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let uri = format!(
            "file:database_scope_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let inits = Arc::new(AtomicUsize::new(0));
        let hook_inits = inits.clone();
        let db = SqliteDatabase::open(
            &uri,
            SqliteDatabaseConfig::default().with_connection_init(move |_| {
                hook_inits.fetch_add(1, Ordering::Relaxed);
                Ok(())
            }),
        )
        .await
        .unwrap();
        let a = db.store(1);
        let b = db.store(2);
        assert!(Arc::ptr_eq(&a.db_semaphore, &b.db_semaphore));
        assert!(a.reads.is_none());
        assert!(b.reads.is_none());
        assert!(!a.snapshot_safe);
        assert_eq!(inits.load(Ordering::Relaxed), 1);
        let permit = a.db_semaphore.clone().acquire_owned().await.unwrap();
        assert_eq!(b.db_semaphore.available_permits(), 0);
        drop(permit);
        assert_eq!(b.db_semaphore.available_permits(), 1);
    }
}
