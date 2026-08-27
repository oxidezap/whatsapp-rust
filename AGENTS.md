# WhatsApp-Rust

Rust implementation of the WhatsApp protocol: QR pairing, E2E encrypted messaging (1-on-1 + group), media, VoIP, connection management.

Ground truth for protocol behavior is WhatsApp Web itself: query the structured [whatspec](https://github.com/oxidezap/whatspec) IR first, drop to the raw bundle in `docs/captured-js/` when it can't answer, and treat **whatsmeow** (Go) and **Baileys** (TypeScript) as second opinions. See `agent_docs/wa_web_reference.md`.

## Crates

- **wacore** — platform-agnostic core: binary protocol, crypto, IQ types, state traits. Also builds for wasm32 and ESP32, so no Tokio here.
- **waproto** — prost-generated protobufs from `whatsapp.proto`. No feature logic.
- **whatsapp-rust** — Tokio runtime, SQLite persistence (Diesel), high-level API.
- **whatspec-codegen** (`tools/`) — build tooling, never published and outside `default-members`. Regenerates every whatspec-derived file in one pass from a pinned IR commit. Nothing links it.

## Build & verify

```bash
cargo fmt --all
cargo nextest run -p <touched crate> --lib              # fast local loop
cargo clippy --workspace --all-targets -- -D warnings   # what CI enforces
```

CI runs tests through [cargo-nextest](https://nexte.st) (`--profile ci`, config in `.config/nextest.toml`); install it from a [pre-built binary](https://nexte.st/docs/installation/pre-built-binaries/) to reproduce a CI failure locally. `cargo test` still works — with one gap in the other direction: nextest cannot run **doctests**, so CI runs `cargo test --doc` as its own step and a doc example you add is only covered there.

Workspace clippy takes minutes — pushing and letting CI parallelize the matrix is usually faster. E2E tests (`cargo nextest run --profile e2e -p e2e-tests`) need the mock server running; see `agent_docs/e2e_testing.md`.

Touching `unsafe` — the `Yokeable`/`StableDeref` impls in `wacore-binary`'s `node.rs`, the `set_len` in `zlib_pool.rs` — means CI's Miri gate (`.github/workflows/miri.yml`) is what proves it, since neither clippy nor a native test observes an aliasing violation or an uninit read. Locally: `rustup component add miri rust-src && cargo miri test -p wacore-binary --lib`. Interpretation is ~100× native, so a fixture that only makes sense at hundreds of KB (zlib window refill, buffer growth) belongs behind `#[cfg_attr(miri, ignore)]` with a small twin that keeps the `unsafe` covered.

## Gotchas

Things that look correct and are not:

- **Device state.** Never mutate `Device` directly, not even in tests — a write-lock mutation bypasses the cached snapshot. Mutate through `DeviceCommand` + `PersistenceManager::process_command()`; read through `get_device_snapshot()`, which returns a cached `Arc<Device>` (sync, refcount-cheap, safe per message) — hold it and borrow fields instead of cloning them. `get_device_arc()` exists only for store adapters that need `&mut Device` trait access.
- **Locks.** `session_locks` serializes Signal encrypt/decrypt per protocol address; `chat_lanes` (`ChatLane::enqueue_lock` in `src/client.rs`) serializes *incoming* processing per chat. Outgoing sends are deliberately not per-chat locked — WA Web doesn't lock them either.
- **Wire-tagged enums.** Every protocol enum derives `WireEnum`, and its `#[wire = ...]` attribute is the single source of truth for the wire value. Do not also derive `serde::Serialize`/`Deserialize` or add `#[serde(rename_all)]` — the derive owns both. In tagged mode it generates a sibling `<Name>Tag`; parsers must dispatch on `<Name>Tag::try_from(node.tag.as_ref())` rather than string literals, so renaming a tag stays a one-attribute change. Modes and attributes: `agent_docs/protocol_architecture.md`.
- **Event payloads are a frozen API.** Sealed with `#[non_exhaustive]` + `#[derive(bon::Builder)]` and constructed via `Type::builder()…build()`; a maybe-absent field is `Option<T>`, never an empty-string or zero sentinel. The full stability policy is the `Event` doc comment in `wacore/src/types/events.rs`.
- **Generated files are generated, not edited.** `wacore/src/iq/abprops.rs`, `wacore/src/iq/mex_operations.rs`, `wacore/appstate/src/schemas.rs`, `wacore/src/types/wire_enums.rs`, `wacore/src/iq/targets.rs`, `wacore/src/stanza/wire_tags.rs`, `wacore/binary/src/tokens.json`, `waproto/src/whatsapp.proto` and `wacore/src/version/generated.rs` all come out of `cargo run -p whatspec-codegen`, together, from one pinned whatspec commit. An action or flag the protocol carries but the bundle no longer builds goes in a hand-written sibling (`wacore/appstate/src/schemas_unlisted.rs`, `props::stale`), never in the generated file. `wire_enums.rs` binds only the catalog entries listed in the emitter's `WANTED`, because 88 of the 403 have a synthetic name and names repeat across modules; the variants themselves always come from the bundle. A candidate is found by its variant set but decided by its module: two enums agreeing on every value are not the same enum unless the module owns the wire format we parse. `targets.rs` binds the same way and covers `w:g2` only, the one namespace where a request's target is not implied by its namespace. `wire_tags.rs` takes its stanza tags from the union of the `notif`, `srvreq` and `stanza` documents, because the dispatcher table alone omits `iq` and `ack`, which this repository handles; it drops `privacy`, which is the type of an outgoing stanza and never arrives under that tag, so adding it would invite a handler that can never fire.
- **`whatsapp.proto` is not the whole persisted schema.** It comes from whatspec and is regenerated wholesale, so fields we persist but upstream does not declare live in `LOCAL_FIELDS` in `waproto/build.rs`, spliced into the descriptor at build time, and whole retained messages in `LOCAL_BLOCKS` in the codegen's proto emitter. Never hand-edit the `.proto` or `.desc` to add one — the next sync would drop it.
- **Blocking work** — `ureq`, heavy CPU — belongs in `tokio::task::spawn_blocking`; it shares a runtime with the read loop.
- **let-chains**, never nested `if let`. Clippy's `collapsible_if` is denied in CI.
- **No real PII in tests**, including vectors derived from production captures. Regenerate them from fictitious JIDs and numbers.
- **Errors**: `thiserror` for typed errors, `anyhow` where several failure kinds meet. No `.unwrap()` outside tests.

## Adding a feature

Find the wire format before designing anything — see `agent_docs/feature_implementation.md`. IQ requests go through `client.execute(Spec::new(&jid)).await?`, and `IqSpec` constructors take `&Jid` so callers need not clone. Public surface is `pub use` in `src/features/*.rs`, re-exported from `src/features/mod.rs` and `src/lib.rs`.

Comments carry the *why* of a decision, at the single point where it is made. Repeating a rationale at call sites is how it goes stale.

## Detailed docs

Read the one that covers what you are touching:

| Doc | Read it when |
| --- | --- |
| `agent_docs/wa_web_reference.md` | Confirming any protocol behavior, limit, enum value, or stanza shape against real WA Web |
| `agent_docs/protocol_architecture.md` | Building or parsing stanzas: `ProtocolNode`, `IqSpec`, derive macros, node helpers |
| `agent_docs/noise_handshake.md` | Connection setup: XX/IK/fallback selection, server cert cache, failure classification |
| `agent_docs/feature_implementation.md` | Starting a feature and needing its wire format from captured WA Web JS |
| `agent_docs/subsystem_boundary.md` | Adding a feature gate, adding a `Client` field only one subsystem reads, or proposing that a subsystem leave the core |
| `agent_docs/signal_durability.md` | Any code that reads, mutates, persists, or sends Signal state |
| `agent_docs/e2e_testing.md` | Writing or fixing tests under `tests/e2e/` |
| `agent_docs/observability.md` | Adding a cache, counter, or anything reported by `memory_report()` / `stats()` |
| `agent_docs/plugin_architecture.md` | Touching the `plugins` / `client-lifecycle` feature surface |
| `agent_docs/voip_audio_codecs.md` | VoIP media: codec profiles, negotiation, encoded audio API |
| `agent_docs/wam_telemetry.md` | WAM: the generated event catalog, the buffer codec, and what a client may honestly report |
| `agent_docs/binary_size_ci.md` | A size gate failed, or a change adds dependencies or generic instantiations |
| `agent_docs/build_flags.md` | Recommending codegen flags, or asked why a `target-feature` is not a default |
| `agent_docs/debugging.md` | Decoding raw binary-protocol bytes by hand |
