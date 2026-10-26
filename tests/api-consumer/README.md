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

See the [support matrix](../../agent_docs/support.md) for target/runtime qualification.

`host_shutdown_budget` demonstrates retaining shutdown across a host deadline and
waiting for storage ownership to release. `bounded_plugin_workers` demonstrates
bounded admission, shared payloads and cooperative drain/cancellation. Run both:

```sh
cargo test --manifest-path tests/api-consumer/Cargo.toml --no-default-features \
  --features native,plugins --test host_shutdown_budget --test bounded_plugin_workers
```
