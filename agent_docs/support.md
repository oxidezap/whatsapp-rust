# Support qualification

Support is qualified per target, feature profile, runtime and storage backend.
A successful Linux build does not qualify another operating system. A compile
check does not establish working sockets, browser persistence or authenticated
WhatsApp behavior. Record the exact commit, toolchain and executed commands in
release evidence; the table below identifies the maintained gates, not a claim
that every gate passed on an arbitrary checkout.

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

`signal` means OS signal handling; it does not select Signal encryption. TLS,
codec, SQLite and native relay dependencies belong to the selected profile.
The sans-I/O VoIP core can compile independently of a native relay provider.
Do not infer that all codecs or platform adapters are available from the
`voip-control` profile alone.

## Reproducing the maintained gates

Use the pinned toolchain in `rust-toolchain.toml` for workspace checks. The
isolated consumer driver selects each requested toolchain and validates directed
negative controls as well as positive profiles:

```console
cargo xt ci consumers run --lane native --toolchain nightly-2026-06-16
cargo xt ci consumers run --lane msrv --toolchain 1.94.1
cargo xt ci consumers run --lane wasm --toolchain nightly-2026-06-16
```

The browser lane needs the `wasm32-unknown-unknown` standard library, matching
`wasm-bindgen` runner, and Chrome/ChromeDriver as configured in `wasm.yml`.
Its runtime and SQLite tests execute in a browser; the release build steps
alone only establish linking. See `tests/api-consumer/README.md` and the registry
for the exact feature combinations rather than expanding them into a separate
list that can drift. Native SQLite also has independent storage consumer tests.

For a minimal compile/link probe matching the browser SDK claim:

```console
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build -p whatsapp-rust --lib --release --target wasm32-unknown-unknown --no-default-features
```

Do not label a target qualified from inspecting these commands. Run the relevant
lane on the candidate and retain its result. A failed advisory or other required
gate remains a readiness blocker even when platform smoke tests pass.
