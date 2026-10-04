# Group DTO consumer contract

This is an independent workspace (no root workspace/dev-dependency feature
unification). All identities are fictitious. It tests creation inputs and
mockable full metadata/participant outputs, not authenticated server behavior.

## Modes

```console
cargo test --locked --manifest-path tests/fixtures/groups_consumer/Cargo.toml
cargo test --locked --manifest-path tests/fixtures/groups_consumer/Cargo.toml --no-default-features
cargo +1.94.0 test --locked --manifest-path tests/fixtures/groups_consumer/Cargo.toml
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo check --locked --manifest-path tests/fixtures/groups_consumer/Cargo.toml --no-default-features --features js --target wasm32-unknown-unknown
```

Default mode includes `whatsapp-rust` (with its default features disabled),
input constructor/builder tests, mock output construction and field reading,
open destructuring patterns, generic builder conversion, and hierarchy/overview
projection tests. No-default-features mode checks the pure `wacore` inputs
without the Tokio runtime crate. WASM checks that input-only mode with the
browser RNG backend. No fixture needs a server.

Doctests are negative controls for old literals, exhaustive destructuring,
identity-free Default, and builders missing required fields. Accessible imports
and successful construction are exercised by the unit tests. Nightly rustdoc
checks the annotated E0639/E0638/E0599 diagnostics; stable rustdoc accepts any
compilation failure, so use nightly to verify causes as well as MSRV to verify
consumer support. Missing builder identities use compile-fail controls without
an error-code annotation because bon's typestate diagnostic can vary.

Do not use `--all-features` as a WASM substitute: it enables the runtime output
mode intentionally excluded by the core-only WASM fixture check.
