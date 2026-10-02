# SQLite API consolidation contract

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55` (pre-1.0 intentional Rust API breaks).

## Canonical construction and administration

- `SqliteStore::open(url).await?`: default configuration, fixed account id 1,
  no implicit provisioning. This replaces `new(url)`; the former two-argument
  `open(url, config)` also moves to the database path below.
- `SqliteDatabase::open(url, SqliteDatabaseConfig::default()).await?`:
  shared database owner. `db.store(id)` selects without checking/provisioning.
- Custom configuration: `SqliteDatabase::open(url, config).await?.store(id)`.
  Replaces `with_config`, `new_for_device`, `with_config_for_device`.
- Administration belongs exclusively to `SqliteDatabase`: `list_devices`,
  `create_device`, `provision_device`, `device_exists`, `reset_device`,
  `remove_device`. `create_sibling_device`'s tuple becomes a store;
  obtain its id with `store.device_id()`.
- `DeviceStore::exists/create/load/save` remain the scoped backend contract.
  `create_new_device` is now internal, not a second provisioning API.
- `share_for_device` is internal: use `db.store(id)` or
  `store.database().store(id)`. `store.database()` opens no connections.
- `SqliteDatabaseConfig` is the concrete configuration type, with exactly one
  implementation/default. `SqliteStoreConfig` is removed, not a transition
  alias: repository usage demonstrates no distinct capability requiring it.
  This task does not bump a release version or decide a release date.

## Advanced controls and persistence

`SharedSqlite` remains public and cloneable, available from either database or
store. Its database-wide read/write/snapshot/transaction capabilities remain
available for external adapters, which must supply their own account predicates.
No device-id SQL helper with a distinct external capability was found: existing
load/save helpers are already private; scope/provision/existence cover their
public use cases. Test-only query helpers are not production exports.

Pools, reader/writer permits, barrier retention and post-commit ordering are
unchanged. Stores/adapters retain resources after the database owner is dropped;
last-reference release is distinct from client shutdown. Scoped reports remain
zero payload memory; database and `DeviceStore` reports remain whole-pool reports.
Do not sum the whole-pool report over scoped stores.

No migrations, schema, persisted formats, legacy migration guards, PN/LID data,
Signal semantics or secret-read methods are removed/changed. A07 owns secret
reads independently in `sqlite_store.rs`; these edits only affect construction,
configuration, administration and their callers. Existing administrative reset
and removal semantics are unchanged, including caller-owned lifecycle quiescence
and non-revocation of already-issued stores.

A08 should use these names in the consolidated migration guide. Historical
reader-retention evidence retains the names it actually measured.

## Validation

Existing storage tests and public file-backed controls are retained. Additional
public tests close all handles and reopen a real file, checking account isolation,
identity keys, PN/LID listing and external adapter data; another test exercises
external adapter last-reference/barrier retention. Native/WASM consumers, actual
head CI and configured substantive bot results are recorded separately. Removing
names alone is not evidence of binary/RAM savings.
