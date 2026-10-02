# A03: encapsulated client and managed device state

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`. This is the bounded A03 handoff to A08, not the consolidated migration guide or a release.

## Consumer migration

| Removed public implementation detail | Supported replacement |
| --- | --- |
| `Client::enable_auto_reconnect: Arc<AtomicBool>` | `set_auto_reconnect(bool)`, `auto_reconnect_enabled()` |
| `Client::group_cache: OnceLock<Arc<GroupCache>>` | `groups()` lookup operations and their existing freshness settings; `memory_report().await.group_cache` for diagnostics; `run_cache_maintenance()` for manual maintenance |
| `Client::custom_enc_handlers: OnceLock<HashMap<…>>` | `ClientBuilder`/`BotBuilder` encrypted-handler registration; `has_enc_handler(&str)`; `memory_report().await.custom_enc_handlers` |
| `Client::http_client: Arc<dyn HttpClient>` | `http_client() -> &Arc<dyn HttpClient>` for reuse by a session-independent `MediaDownloader` or another host adapter |
| `PersistenceManager::get_device_arc() -> Arc<RwLock<Device>>` | `get_device_snapshot() -> Arc<Device>` for reads; `process_command` or `modify_device` for ordinary writes; `modify_device_async` for advanced async `&mut Device` trait access |

The cache, handler registry and injected HTTP field are crate-visible worker implementation state, not consumer-visible fields. Reconnection state is private to `client` and its descendants. No replacement returns a writable manager lock, atomic or registry/container.

`set_auto_reconnect` is a runtime preference, not shutdown: it does not close a live socket or resume pause. Re-enabling cannot clear a terminal shutdown or protocol verdict, even after the diagnostic reason has been consumed; a new Client is required. The first attempt remains allowed when disabled before `run`. Disabling during reconnect backoff wakes the loop without waiting for the timer and preserves the observed connect/disconnect/protocol cause in `AutoReconnectDisabled`. Re-enabling does not shorten an existing backoff. A filtered future reuses existing session-state notifications but ignores unrelated hints, preserving the unchanged Client layout guard. Preference and irreversible protocol verdict share a private one-byte atomic state under the existing Arc; a concurrent re-enable cannot erase the verdict. Pause/resume and shutdown retain their independent existing contracts.

Handlers install at build time, last registration for the same payload type wins, and a custom handler takes precedence over the built-in type. No new runtime registration path is offered. Arc registration preserves the host allocation; inspecting the registry does not allocate an empty map.

`modify_device_async` takes a higher-ranked boxed callback:

```rust,ignore
pm.modify_device_async(move |device| Box::pin(async move {
    // Perform an advanced trait call using &mut Device here.
})).await;
```

The callback cannot return a borrow of the device. It holds the existing device write lock; never re-enter the manager or call `flush` inside it. Normal Signal work continues to use `SignalProtocolStoreAdapter`/`SenderKeyAdapter` with per-call snapshots and the existing cache/durability machinery. This is not a new Signal executor or transaction.

Both modifier paths publish dirty-before-snapshot under the write guard and notify saving on guard destruction. Success, returned errors, unwind and cancellation after acquisition publish partial mutations; none promises rollback, durable flush or ordering of backend I/O detached by the host. Cancellation before acquisition makes no mutation. Even a clean/final flush waits for an active modifier's existing write guard before deciding that nothing needs saving; the device flush is not timed out, so the host must finish or cancel scoped modifiers and complete backend saves for `shutdown` to finish. The separate inbound/outbound/Signal drain deadlines remain unchanged; no false durability success is returned while a modifier is active. Previously retained snapshots remain point-in-time views, and backend Arc identity is unchanged.

## Visibility classification

- **Internal worker modules:** `keepalive`, `message`, `history_sync`, `receipt`, `retry`. Their supported inherent `Client` methods remain public. They declare no public host types; imports of those empty public module namespaces are deliberate Rust API removals.
- **Supported advanced/manual seams retained:** `Connection` and manual read loop; `ClientBuild::into_parts`, `sync_task::{MajorSyncTask, HistorySyncTaskTracker}` and `process_sync_task`; `handshake`, `pair`, `pair_code`; `request`, `socket`, `transport`; `pdo::PendingPdoRequest`; `prekeys::PreKeyUtils`; `appstate_sync` processor/error/mutation exports; `session`, `unified_session`, media/upload/download; raw send/edit/retry/receipt operations and Signal store/cache adapters.
- **Host-implemented traits retained and unsealed:** backend/store traits, Runtime, HttpClient, Transport/TransportFactory, EncHandler, EventHandler, admission/durability/resolver/interceptor/lifecycle/plugin traits. Plugin capability handles and raw/decrypted/sent-frame leases keep their existing weak ownership.
- **Detached advanced storage:** public owned `Device`, its direct Signal trait implementations, `DeviceStore`/`DeviceRwLockWrapper`, and `PersistenceManager::backend` remain supported. A host can lock its own detached Device, not obtain the manager's live lock. `StoreRelease` still excludes host-owned backend/snapshot handles.
- **Coherent dependency surface retained:** `wacore`, `wacore_binary`, `waproto`, anyhow, async_trait, async_channel, bytes, futures, serde/serde_json, chrono and buffa reexports. No future A13 send/edit facade or A06 typed reference entry is hidden.

## Usage evidence

A whole-worktree Rust/reference search at the base found:

- Public reconnect writes only in `tests/{lifecycle_outcomes,connect_admission,event_delivery_public}.rs`; these are migrated to the setter. Internal stream verdict/shutdown writes retain their existing meaning.
- HTTP field reads in client/download/upload workers; Arc identity assertions in bot tests. The actual public bridge has two HTTP field reads for buffered stream upload and resumable upload checks: both migrate to `client.http_client().execute(request)` without adding socket or session requirements.
- Cache OnceLock use only in workers and internal tests; external consumers use group features/diagnostic reports. No group-cache algorithm change.
- Registry mutation in the builder and one private synthetic message fixture; no tracked external registry mutation. EncHandler remains implementable and its public builder paths remain.
- Raw manager lock only in internal tests, **not** production adapters. Four Signal tests now use the scoped async callback with the same direct Signal operations; read-only cache/backend/prekey tests use snapshots or backend handles. Production Signal adapters already read snapshots per call.
- Metrics/WAM plugins, public manual lifecycle fixtures, and the standalone single-dependency consumer require no private fields. Public bridge hooks (raw/decrypted/failure/sent-frame leases, stanza interceptors, admission hooks, async handler traits, lifecycle/plugins) are preserved.
- Public `oxidezap/whatsapp-rust-bridge` was read at pinned `1fa1484ff98eb90d3f083c8a510769635537e535` through forge contents, not another lane's checkout. Its `setAutoReconnect` wrapper writes the old atomic (`src/wasm_client/connection.rs:386`); replace that store with `self.client.unwaited(Unwaited::Local).set_auto_reconnect(enabled)`. `src/wasm_client/media.rs:306` and `src/wasm_client.rs:3797` use the HTTP field and need the accessor. Its manual sync receiver/worker (`MajorSyncTask`/`process_sync_task`), shutdown Drop hook, JS Http/Runtime/Transport/cache/backend traits and DeviceCommand paths remain supported. No manager writable-lock or cache/registry access was found in the inspected bridge client/connection/media/signal/runtime/backend/cache/HTTP/admission sources. The live bridge producer/source/pin is not modified by this task; its owner must apply these three source migrations when adopting A03. Forge indexed searches returned no matches even for known consumers, so those search negatives are explicitly **not** a completeness claim.

Proofs: `tests/client_encapsulation.rs`; `tests/consumers/encapsulation` (independent manifest, no root dev-feature unification); separate compile-fail rustdocs for each of the four fields and the removed manager lock. Existing directed/public lifecycle, handler, PN/LID Signal and ownership tests remain necessary. See `report.md` for actual executions, including failures and pending/unrun gates.
