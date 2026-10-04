# WhatsApp-Rust

Rust implementation of the WhatsApp protocol: QR pairing, E2E encrypted messaging (1-on-1 + group), media, VoIP, connection management.

Ground truth for protocol behavior is WhatsApp Web itself: query the structured [whatspec](https://github.com/oxidezap/whatspec) IR first, restore and inspect the exact locked raw bundle when it cannot answer, and treat **whatsmeow** (Go) and **Baileys** (TypeScript) as second opinions. See `agent_docs/wa_web_reference.md`.

## Crates

- **wacore** — platform-agnostic core: binary protocol, crypto, IQ types, state traits. Also builds for wasm32 and ESP32, so no Tokio here.
- **waproto** — buffa-generated protobufs from `whatsapp.proto`. No feature logic.
- **whatsapp-rust** — Tokio runtime, SQLite persistence (Diesel), high-level API.
- **whatspec-codegen** (`tools/`) — build tooling, never published and outside `default-members`. Regenerates every whatspec-derived file in one pass from a pinned IR commit. Nothing links it.

## Repository tasks

Use `cargo xt --help` for descriptors, MLOW oracles and CI maintenance.
The lightweight dispatcher and CI tasks live in `tools/xtask`; capture-backed
tasks run in release through `tools/oracle-task`. Both stay outside default-members;
`xtask-support` is local and shared with whatspec-codegen. The WhatsApp wasm
host/specs live under `tools/oracle-*`; they use commit-pinned `unwasm-core`
for static analysis and whatspec `wa-store` for capture transport. These tool
members are excluded from `default-members`.
Do not add Python/Bash task implementations or runtime dependencies on these tools.

## Build & verify

```bash
cargo fmt --all
cargo nextest run -p <touched crate> --lib              # fast local loop
cargo clippy --workspace --all-targets -- -D warnings   # what CI enforces
```

CI runs tests through [cargo-nextest](https://nexte.st) (`--profile ci`, config in `.config/nextest.toml`); install it from a [pre-built binary](https://nexte.st/docs/installation/pre-built-binaries/) to reproduce a CI failure locally. `cargo test` still works — with one gap in the other direction: nextest cannot run **doctests**, so CI runs `cargo test --doc` as its own step and a doc example you add is only covered there.

Workspace clippy takes minutes — pushing and letting CI parallelize the matrix is usually faster. E2E tests (`cargo nextest run --profile e2e -p e2e-tests`) need the mock server running; see `agent_docs/testing.md`.

Touching `unsafe` — the `Yokeable`/`StableDeref` impls in `wacore-binary`'s `node.rs`, the `set_len` in `zlib_pool.rs` — means CI's Miri gate (`.github/workflows/miri.yml`) is what proves it, since neither clippy nor a native test observes an aliasing violation or an uninit read. Locally: `rustup component add miri rust-src && cargo miri test -p wacore-binary --lib`. Interpretation is ~100× native, so a fixture that only makes sense at hundreds of KB (zlib window refill, buffer growth) belongs behind `#[cfg_attr(miri, ignore)]` with a small twin that keeps the `unsafe` covered.

## Gotchas

Things that look correct and are not:

- **Device state.** Never mutate `Device` directly, not even in tests — a write-lock mutation bypasses the cached snapshot. Mutate through `DeviceCommand` + `PersistenceManager::process_command()`; read through `get_device_snapshot()`, which returns a cached `Arc<Device>` (sync, refcount-cheap, safe per message) — hold it and borrow fields instead of cloning them. `modify_device_async()` scopes advanced adapters' `&mut Device` trait access while preserving snapshot and save publication, including on cancellation.
- **Locks.** `session_locks` serializes Signal encrypt/decrypt per protocol address; `chat_lanes` (`ChatLane::enqueue_lock` in `src/client.rs`) serializes *incoming* processing per chat. Outgoing sends are deliberately not per-chat locked — WA Web doesn't lock them either.
- **Wire-tagged enums.** Every protocol enum derives `WireEnum`, and its `#[wire = ...]` attribute is the single source of truth for the wire value. Do not also derive `serde::Serialize`/`Deserialize` or add `#[serde(rename_all)]` — the derive owns both. In tagged mode it generates a sibling `<Name>Tag`; parsers must dispatch on `<Name>Tag::try_from(node.tag.as_ref())` rather than string literals, so renaming a tag stays a one-attribute change. Modes and attributes: `agent_docs/architecture.md`.
- **Event payloads are a frozen API.** Sealed with `#[non_exhaustive]` + `#[derive(bon::Builder)]` and constructed via `Type::builder()…build()`; a maybe-absent field is `Option<T>`, never an empty-string or zero sentinel. The full stability policy is the `Event` doc comment in `wacore/src/types/events.rs`.
- **Generated files are generated, not edited.** Regenerate all domains together from the pinned whatspec commit; see `agent_docs/codegen.md`. Inspect the emitter and manifest for current catalog coverage. Unlisted protocol items belong in hand-written siblings.
- **`whatsapp.proto` is not the whole persisted schema.** It comes from whatspec and is regenerated wholesale, so fields we persist but upstream does not declare live in `LOCAL_FIELDS` in `waproto/build.rs`, spliced into the descriptor at build time, and whole retained messages in `LOCAL_BLOCKS` in the codegen's proto emitter. Never hand-edit the `.proto` or `.desc` to add one — the next sync would drop it.
- **Blocking work** — `ureq`, heavy CPU — belongs in `tokio::task::spawn_blocking`; it shares a runtime with the read loop.
- **let-chains**, never nested `if let`. Clippy's `collapsible_if` is denied in CI.
- **No real PII in tests**, including vectors derived from production captures. Regenerate them from fictitious JIDs and numbers.
- **Errors**: `thiserror` for typed errors, `anyhow` where several failure kinds meet. No `.unwrap()` outside tests.

## Adding a feature

Find the wire format before designing anything — see `agent_docs/architecture.md`. IQ requests go through `client.execute(Spec::new(&jid)).await?`, and `IqSpec` constructors take `&Jid` so callers need not clone. Public surface is `pub use` in `src/features/*.rs`, re-exported from `src/features/mod.rs` and `src/lib.rs`.

Comments carry the *why* of a decision, at the single point where it is made. Repeating a rationale at call sites is how it goes stale.

## Working guides

- `agent_docs/architecture.md`: layers, protocol types, ownership and durability.
- `agent_docs/testing.md`: local/CI checks, E2E, layout budgets and captures.
- `agent_docs/codegen.md`: generated files, feature graphs and build policy.
- `agent_docs/observability.md`: counters, reports, tracing and measurement scope.
- `agent_docs/wa_web_reference.md`: verify behavior against the pinned source.

Consumer guides and migrations belong in https://github.com/oxidezap/whatsapp-rust-docs.
Update their examples with breaking changes. Keep only permanent contributor
instructions here; PR descriptions and Git history retain task evidence.
