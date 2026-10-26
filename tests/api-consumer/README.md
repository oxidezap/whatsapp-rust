# Public API consumer

One standalone manifest and lock verify downstream visibility without root
workspace dev-dependencies. Run profiles separately with `cargo xt ci consumers
run --lane native`; use `--lane msrv --toolchain 1.94.1` or `--lane wasm` for
published MSRV and browser linking. Add new contracts to the owning module,
not a new crate. `tools/xtask/consumers.json` defines the executed profiles.

- `core` checks group inputs without the SDK or Tokio. `core,js` is its WASM graph.
- `sdk` covers creation/store traits, typed actions, MEX, pictures, status, DTOs,
  event interests, lifecycle admission and encapsulation with defaults disabled.
- `native` runs construction, event delivery, downloads and lifecycle outcomes.
- `plugins`, `voip-control`, `voip-runtime` and `voip-mlow` check those independent
  opt-ins. The SDK's native runtime adapter and SQLite stay disabled until selected.
- The `sqlite` profile tests native store construction and links browser futures
  in the `wasm` lane with the storage crate's `wasm-test` feature. `sdk,requests` checks request
  branches separately from default requests.

Directed negatives run through xtask's shared diagnostic validator. It checks
one error, the exact code/type and the primary source span. Positive controls
compile the same imports. Rustdoc covers additional examples and rejection cases.

The previous domain fixtures map to like-named modules under `src/`, including
client_options, events/event_kinds, lifecycle/lifecycle_graph, sqlite,
creation/download/mex/pictures/requests, groups/groups_lookup,
voip_control/peer_video and media_cache/actions/community/status. Root functional
lookup tests stay in `tests/group_lookup_contract.rs`; call-action parser/wire
checks stay in `wacore/tests/call_action_public.rs`. Historical module aliases no
longer need repeated negative proofs; the removed peer-video variant keeps one
directed diagnostic control. Domain bins retain portable link probes.

The [support qualification matrix](../../agent_docs/support.md) maps these
profiles to targets, runtimes and storage, and distinguishes linking from
execution and unqualified portability. Record actual lane results on the
candidate commit before making a support claim.

The host policy fixtures are account-free and execute against the public API:

- `host_shutdown_budget` runs with `native`. A pinned shutdown future survives a
  host observation deadline. The host reports cleanup pending, retains owners,
  resumes the save, inspects the returned report, then drops client/manager
  handles before observing `StoreRelease`. A separate cancelled flush proves
  that the older save commits before the newer snapshot. The parked background saver test drops the host manager before
  releasing the worker. Release observes crate ownership; it does not close a host-owned backend or establish durability.
- `bounded_plugin_workers` runs with `native,plugins`. A downstream plugin starts
  two cooperative workers, rejects admission before readiness or after closure,
  bounds queued jobs and queued plus in-flight bytes, and counts DropNewest
  rejections. Shutdown seals admission and drains accepted work. Real work needs
  cooperative cancellation and its own I/O deadlines; the host task-drain timeout
  can report incomplete cleanup when work ignores them. No quota or scheduler is
  added to the SDK. `TaskCapacityExceeded` remains counter overflow protection,
  not a configured task budget. The foreign adapter fixture rejects oversized
  bytes before copying into `Bytes` and checks sharing across two router
  endpoints. Endpoint capacity counts envelopes, while endpoint count and total
  router retention require separate adapter policies.

Run both fixtures with:

```sh
cargo test --manifest-path tests/api-consumer/Cargo.toml --no-default-features \
  --features native,plugins --test host_shutdown_budget --test bounded_plugin_workers
```
