# A12 — one SQLite database, fixed-device stores

## Why

Opening `SqliteStore::new_for_device` for every account opens independent pools.
`SqliteDatabase` makes the ownership boundary explicit: open/configure/administer
once, then select cheap, fixed-device `SqliteStore` handles. It reuses the existing
`share_for_device` plumbing, rather than adding a second pool implementation.
There are no schema, migration, retention, ACK or commit-order changes.

## Single account

Existing `SqliteStore::new("whatsapp.db").await?` continues to work. The canonical
single-account path is `SqliteStore::open(url, config)`; it selects id 1. Opening
never provisions a row. The client's existing `DeviceStore::create` provisioning
path remains unchanged.

```rust,no_run
use whatsapp_rust_sqlite_storage::{SqliteStore, SqliteDatabaseConfig};
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let store = SqliteStore::open("whatsapp.db", SqliteDatabaseConfig::default()).await?;
// Pass store to your existing backend/client construction.
# Ok(()) }
```

Defaults are unchanged: write pool 1, requested read pool 1, cache 512 KiB per
connection, Normal synchronous, busy timeout 30 seconds, mmap unset, incremental
vacuum off (400 pages if enabled). Readers are only opened for native WAL without
shared cache. In-memory/shared-cache URIs and WASM keep the supported single-queue
fallback. Bare `:memory:` retains its previous unsupported snapshot/opening
behavior; use a named `file:...?...mode=memory&cache=shared` URI for memory tests.
Connection-init, thread-pool, mmap, vacuum and commit-barrier options are the same
configuration, not a second set of defaults. `SqliteDatabaseConfig` is an alias of
`SqliteStoreConfig`; existing config struct literals and builder methods work.

## Multiple accounts

Before:

```rust,no_run
# use whatsapp_rust_sqlite_storage::SqliteStore;
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let first = SqliteStore::new_for_device("whatsapp.db", 1).await?;
let second = SqliteStore::new_for_device("whatsapp.db", 2).await?; // another pool
# Ok(()) }
```

After:

```rust,no_run
use whatsapp_rust_sqlite_storage::{SqliteDatabase, SqliteDatabaseConfig};
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let db = SqliteDatabase::open("whatsapp.db", SqliteDatabaseConfig::default()).await?;
for account in db.list_devices().await? {
    let store = db.store(account.id); // same pools, permits and durability barrier
}
let fresh = db.create_device().await?; // allocated, provisioned, fixed-device handle
let fresh_id = fresh.device_id();
let existing = db.store(fresh_id); // selection only; no existence check or insert
// For explicit-id provisioning on a fresh database:
// let primary = db.provision_device(1).await?;
# Ok(()) }
```

Dropping the database does not close connections owned by a remaining store.
`store(id)` does not imply that id exists. `provision_device(id)` is a plain insert:
an existing row remains an error, never an implicit reset. `create_device()` uses
the existing transactional AUTOINCREMENT allocation; it returns a store whose
`device_id()` identifies the newly provisioned row.

Administration is database-wide and explicit:

- `list_devices()` lists every row, sorted by id.
- `device_exists(id)` checks only the selected row.
- `reset_device(id)` purges that account and recreates its keys under the same id,
  returning its scoped store.
- `remove_device(id)` purges that account and removes its row, returning `()`.
- Reset/remove on a missing id return `StoreError::DeviceNotFound(id)`.

**Stop the selected account's clients/savers/flushers before reset/remove.** These
operations do not revoke old store handles, cancel background work or remove other
accounts. Old handles can still write/recreate rows. These preexisting semantics
are deliberately preserved, not strengthened by the new facade.

## Scope helpers are now internal (breaking)

The old public `*_for_device` persistence helpers duplicated domain methods while
allowing a fixed-device handle to target another account. Select the scope first
and call the existing open traits instead. No domain trait or required backend
method changes.

```rust,no_run
use whatsapp_rust_sqlite_storage::{SqliteDatabase, SqliteDatabaseConfig};
use wacore::store::traits::{DeviceStore, SignalStore, AppSyncStore};
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let db = SqliteDatabase::open("whatsapp.db", SqliteDatabaseConfig::default()).await?;
let account = db.store(2);
account.put_identity("15550000001.1", [7; 32]).await?;
let identity = account.load_identity("15550000001.1").await?;
let session = account.get_session("15550000001.1").await?;
let device = account.load().await?;
// Save an immutable device through DeviceStore::save when implementing an adapter.
// Client-side device changes still use DeviceCommand + PersistenceManager, not locks.
# Ok(()) }
```

This applies to device load/save, identity/session/sender-key operations and
app-state key/version/MAC/patch helpers. Their same-named domain methods (without
`_for_device`) take the selected store's id. MsgSecretStore reading and timestamps
are unchanged; A07 owns that transition.

The four legacy opening constructors, `share_for_device`, `shared`, and existing
store administration methods remain supported compatibility wrappers. New code
should use database administration. No deprecation warnings are emitted during
this transition; removal of these wrappers would require a future breaking release
with a separately announced migration (not an automatic removal next release).

## Reports and advanced access

`db.resource_report().await` describes the whole writer/read pool and database:
count it **once per opened database**, including when the database handle is cloned.
`store.scoped_resource_report()` excludes shared resources and reports zero
per-device payload/cache memory (there are no per-account caches in this adapter).
It does not measure the small Rust handle, RSS, or SQLite lookaside allocations.

For compatibility, `DeviceStore::resource_report(&store).await` still reports the
whole pool. Summing that legacy report or `Client::resource_report()` across stores
sharing the pool duplicates its estimate. Multi-account aggregators must instead
count the database report once, excluding the legacy per-client storage component.
PRAGMA contention/failure still means absent, not zero, in the pool report.

`db.shared()` and `store.shared()` preserve `SharedSqlite`: advanced cross-crate
adapters with their own tables use the same permits, readers and post-commit
barrier. This capability is **database-wide**, not an account authorization
boundary; adapter queries must include their own scope predicates. No raw pool or
mutable device lock is newly exposed. `store.database()` recovers administration
and pool reporting without reopening. The storage traits remain open and
independently implementable; `SqliteDatabase` is not a `Backend` requirement.

## Public paths and extension policy

`whatsapp_rust_sqlite_storage::{SqliteDatabase, SqliteDatabaseConfig, SqliteStore}`;
also `whatsapp_rust::store::{SqliteDatabase, SqliteDatabaseConfig}` under the existing
`sqlite-storage` feature. `SharedSqlite` remains exported by the adapter crate.
New handles have private fields and constructors, so their internals can evolve.
The config alias preserves the existing configuration's compatibility contract;
no new struct-literal DTO or sealed trait is introduced.
