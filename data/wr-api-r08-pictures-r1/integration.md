# R08 — neutral picture lookup facade

Base checked against origin/default: `e32ec562a207c95188693ec1a2738ea69c08fa48` (main).
R08 was still present. This is an intentional pre-1.0 discoverability change,
not a protocol fix. No workflows, registry, shared lockfiles or protocol builders
are changed.

## API and migration

```rust,ignore
// Before: client.contacts().lookup_picture(request).await?
let outcome = client.pictures().lookup(request).await?;
```

`Client::pictures() -> Pictures<'_>` is available unconditionally. `Pictures` is
re-exported at `whatsapp_rust::Pictures` and `whatsapp_rust::features::Pictures`.
`Contacts::lookup_picture` is removed, not deprecated or retained as an alias.
Existing `ProfilePictureRequest`, `ProfilePictureTarget`, `ProfilePictureType`,
`ProfilePictureLookup` and `ProfilePicture` imports are unchanged.
The return remains `Result<ProfilePictureLookup, ContactError>` intentionally:
this relocation preserves the existing error/source contract rather than also
introducing a renamed error type or requiring consumers to change error handling.

Contact, Group and Community routes use the same shared internals as before.
Token discovery, token-first/common-group fallback, own/special/PSA handling,
conditional IDs, timeout/cancellation, community namespace/attributes and the
absence of automatic fallback remain unchanged. 401/403 and 404 retain their
successful classifications; 429 remains an IQ rejection with the original
stanza and optional backoff. Other errors are not collapsed into absence.
Setters/removals remain on Profile/Groups/Community. `Groups::get_profile_pictures`
remains the distinct batch operation. No generic image-operation layer is added.

All repository callers, including directed tests, metrics controls and the
profile-picture/privacy-token E2E sources, migrate to the new facade. The old
picture-consolidation contract's adoption snippets are updated too.

## Consumer fixture modes (handoff to R05)

R05 is the separate consumer CI registry/workflow finding and implementation
lane; these R08 fixture modes are its integration inputs.

Manifest: `tests/fixtures/pictures_consumer/Cargo.toml` (standalone workspace,
existing committed lockfile, dependency default-features=false).

- Native/MSRV: `cargo +1.94.0 build --locked --manifest-path tests/fixtures/pictures_consumer/Cargo.toml`.
- Offline executable: `cargo run --locked --manifest-path tests/fixtures/pictures_consumer/Cargo.toml`.
- Negative removal control (must register in addition to binary build):
  `cargo test --locked --doc --manifest-path tests/fixtures/pictures_consumer/Cargo.toml -- --nocapture`.
  `src/lib.rs` contains `compile_fail,E0599` using precisely the removed
  `Contacts::lookup_picture`. All imports/request construction have a positive
  counterpart in the binary, which exercises the new Pictures type, all three
  targets, options, Send boxed native/async_trait futures and unchanged batch API.
- WASM: `RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --locked --release --target wasm32-unknown-unknown --manifest-path tests/fixtures/pictures_consumer/Cargo.toml`.
  WASM boxing and async_trait deliberately use non-Send futures as before.
- Root directed tests: `cargo test --locked -p whatsapp-rust --lib features::pictures`;
  metrics version adds `--features metrics`. `cargo test --locked -p whatsapp-rust --test profile_picture_lookup` covers downstream imports/boxing.
- Rustdoc: `cargo test --locked -p whatsapp-rust --doc` covers the moved lookup example
  and a second removal control on the public `Pictures` documentation. This runs
  in the existing Build & Test job's workspace doctest step, independently of
  R05 wiring the standalone fixture's doctests into the central registry.

## Validation

Implementation complete; local serialized Cargo validation and hosted CI pending.
Existing routed matrices now invoke the new facade and cover both sizes and
conditional IDs, all routes, envelope/nested errors, response metadata, token
policy, timeout/waiter cleanup, explicit consumer-only community fallback and
unchanged batch/mutation controls. A disconnected-client check moves from Contacts
to Pictures; no production lookup internals are changed.
`cargo fmt --all`, standalone fixture formatting and `git diff --check` pass.
No authenticated server acceptance or performance improvement is claimed.
