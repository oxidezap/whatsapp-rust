# Portable lifecycle consumer

This standalone crate compiles builder/manual receiver ownership, run, logout
and shutdown through `async_trait` and a boxed host future. It uses no default
features, no Tokio, and does not execute its graph or contact a server.

`size_probe` retains the public lifecycle graph via an observable function
pointer; it is a consumer binary, not an rlib-size comparison. Its source uses
canonical APIs already available on the round's base, so the same source can
measure native and wasm base/head under identical toolchain, lock resolution,
release defaults and target flags. No size saving is assumed from removing names.

```sh
cargo build --manifest-path tests/fixtures/lifecycle_consumer/Cargo.toml --release
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build \
  --manifest-path tests/fixtures/lifecycle_consumer/Cargo.toml --release \
  --target wasm32-unknown-unknown
```

Keep the existing repository Binary Size budgets and pinned toolchain unchanged;
this small consumer is additional evidence, not a replacement for that gate.
