# Support qualification

Qualification applies to a specific target, feature profile, runtime and store.
The table lists maintained gates; candidate results belong in release/PR evidence.
Compilation does not establish runtime or authenticated WhatsApp compatibility.

| Target/profile | Runtime and storage | Maintained validation | Qualification boundary |
| --- | --- | --- | --- |
| `x86_64-unknown-linux-gnu`, default SDK | Tokio, native transport/HTTP, bundled SQLite | `main.yml` builds/tests/links; `consumers.yml` runs `native-default,contracts` outside workspace feature unification | Primary native test environment. Tests use synthetic data; account/server compatibility is separate. |
| Linux, SDK with defaults disabled | Injected runtime, transport, HTTP and store; opt-in `plugins`, `voip-control`, `voip-runtime`, `voip-mlow` | Native isolated profiles in `tools/xtask/consumers.json`; opt-ins compile separately | Only the selected feature graph is qualified. A custom backend must meet its documented contracts. |
| Published crates on Rust `1.94.1`, Linux | Same isolated native profiles and storage consumers | `consumers.yml` MSRV lane; `main.yml` stable/MSRV checks | Tooling has a higher compiler floor. A newer compiler passing is not MSRV evidence. |
| `wasm32-unknown-unknown`, single-threaded browser | Host local runtime/transport/HTTP/store, defaults disabled, `js` where required | `wasm.yml` release links SDK/core/VoIP; isolated `core,js` profiles and browser runtime lifecycle tests | No WASI or atomics/threads qualification. Does not imply the native relay dialer or default native SDK works in a browser. |
| Browser SQLite profile | Single-connection platform shim, host SQLite initialization | `wasm.yml` links storage with `js`; `wasm-execution` runs durability/timer fixtures with `wasm-test` storage | The test host supplies SQLite exports. This does not qualify every embedding or persistent filesystem implementation. |
| `aarch64-unknown-linux-gnu` Docker artifact | Dockerfile-selected binary, native runtime/storage | `docker.yml` builds and pushes on native `ubuntu-24.04-arm` for main/tags | Artifact build qualification only. The x86 test suite is not an ARM runtime test, and Docker publication is not performed by a PR validation command. |
| Windows and macOS | Potential native adapters | No maintained OS runner in the above workflows | Unqualified. Linux results are not OS support evidence. |
| ESP32/embedded | Core with target-specific runtime/storage/transport supplied externally | No maintained target gate for the SDK | Experimental portability intent. Neither a complete SDK build nor resource/network behavior is qualified here. |
| Other targets, WASI, threaded WASM and arbitrary feature combinations | Host-specific | No implicit qualification | Unqualified until a reproducible consumer and appropriate execution/link gate exist. |

`signal` selects OS signal handling. Codecs, TLS, SQLite and relay providers
remain profile-specific; `voip-control` alone does not select them all.

## Reproducing the maintained gates

Run isolated profiles with the pinned toolchain and supported MSRV:

```console
cargo xt ci consumers run --lane native --toolchain nightly-2026-06-16
cargo xt ci consumers run --lane msrv --toolchain 1.94.1
cargo xt ci consumers run --lane wasm --toolchain nightly-2026-06-16
```

The browser lane requires the WASM standard library, matching `wasm-bindgen`
runner and Chrome/ChromeDriver configured in `wasm.yml`. Browser execution tests
cover runtime/storage behavior; release builds establish linking only. Exact
feature combinations live in `tools/xtask/consumers.json`.
