# Peripheral-subsystem audit: VoIP, plugins, storage, transports, benches/e2e, feature matrix

Scope: `src/voip/`, `src/client/voip.rs`, `src/handlers/call.rs`, `wacore/src/voip/`, `wacore/src/stanza/call.rs`, `src/plugins/`, `plugins/*`, `storages/sqlite-storage`, `transports/tokio-transport`, `http_clients/ureq-client`, `tests/e2e`, `tests/bench-integration`, `benches/`, `tools/whatspec-codegen` (consumer side), and the feature matrix. All line numbers are against the working tree at `/home/user/whatsapp-rust` on 2026-09-02. No clippy/tests were run; `cargo tree`, `cargo metadata`, `cargo package --list`, and one `cargo check -p whatsapp-rust-wam-catalog` were.

## Headline numbers

| Area | Lines | Code | Tests | Notes |
| --- | ---: | ---: | ---: | --- |
| VoIP in `whatsapp-rust` (`src/voip/**`, `src/client/voip.rs`, `src/handlers/call.rs`) | 24,569 | ~6,900 | ~17,650 | 72% tests |
| VoIP in `wacore` excluding MLow (`wacore/src/voip/*`, `wacore/src/stanza/call.rs`) | 40,722 | ~18,300 | ~22,400 | 55% tests |
| MLow codec (`wacore/src/voip/mlow/`) | 20,316 | ~14,500 | ~5,800 | plus 16 MB `testdata/` |
| **VoIP total** | **85,607** | **~39,700** | **~45,900** | compiles to nothing in a default build |
| `src/plugins/mod.rs` | 6,558 | 3,143 | 3,415 | one file |
| `plugins/wam-catalog/src/{generated,call_sites}.rs` | 131,892 | 131,892 | 0 | generated; 9 of 436 events used |
| `storages/sqlite-storage/src/sqlite_store.rs` | 7,235 | 4,082 | 3,153 | 18 tables |
| `transports/tokio-transport/src/lib.rs` | 536 | 393 | 143 | fine |
| `http_clients/ureq-client/src/lib.rs` | 1,295 | 360 | 935 | fine |

Dependency counts for the root crate (`cargo tree -p whatsapp-rust -e normal`, unique crates): `--no-default-features` 151, default 217, `--features plugins` 218, `--features voip` 333, `--all-features` 340. VoIP is +116 crates (the `rtc-*` stack, `opus`, `aes-gcm`, `zerocopy`, tokio `net`).

---

## Findings, ranked by impact

### 1. VoIP media plane belongs in its own crates; today it is 85k lines inside the two published core crates and ships 16 MB of codec test vectors to every consumer

**Evidence.**
- Gating: `src/lib.rs:201` (`#[cfg(feature = "voip-runtime")] pub mod voip;`), `wacore/src/lib.rs:59` (`#[cfg(feature = "voip")] pub mod voip;`). A default build compiles none of it, and the default feature list (`Cargo.toml:190-197`) has no voip feature.
- The bridge (`/home/user/whatsapp-rust-bridge/Cargo.toml:145`) depends on `whatsapp-rust` with `default-features = false, features = ["danger-skip-cert-chain-verify"]` — no voip feature. Its only call-related API use is `client.voip().reject_call(..)` (`src/wasm_client/signal.rs:23-27`) and `Event::IncomingCall` (`src/wasm_client.rs:226`), both of which are compiled **without** `voip-runtime` (`src/client/voip.rs` `reject`/`reject_call`/`terminate` are ungated; see finding 2).
- `wacore/Cargo.toml` has no `exclude`/`include`. `cargo package -p wacore --list` includes 30 files under `src/voip/mlow/testdata/` (16 MB on disk: `gennoise_vectors.json` 5.8 MB, `exc_pre_lags.json` 2.2 MB, `e2e_vectors.json` 1.8 MB, ...). The packaged crate is **22.1 MiB / 7.5 MiB compressed** (`target/package/wacore-0.7.0.crate` = 7,869,073 bytes). Every `include_str!`/`include_bytes!` of that data is under `#[cfg(test)]` (e.g. `wacore/src/voip/mlow/decoder.rs:556`, `quality_tests.rs:193`), so none of it is needed by a downstream build.
- MLow is self-contained: the only `crate::` references from `wacore/src/voip/mlow/*.rs` are `crate::voip::mlow` (11) and `crate::voip::audio` (1); its consumers are `engine.rs` (17 refs), `driver.rs` (4), `src/voip/transport/native.rs` (2), `mod.rs` (1). It has its own `build.rs` codegen (`wacore/build.rs:1-37`, buffa tables from `tables.desc`), which means **every** `wacore` build (default features, wasm, ESP32) runs a build script and links `buffa-build` + `sha2` as build-deps for a feature that is off.
- MLow is a symbol-for-symbol port of Meta's `smpl_audio_codec` (module names `smpl_*`, identifiers like `nrgres_dbq_Q14`, `smpl_distribute_fcb_surv`, tests named `nrgres_fcbg_match_c_reference` at `param_decode_match.rs:15`, provenance "captured WASM decode vectors" at `mlow/mod.rs:3`). It is a codec library, not protocol code.
- `agent_docs/subsystem_boundary.md` already records `voip-runtime` as "one test away from cuttable": the three coupling edges are `would_emit_pkmsg` (`src/client/sessions.rs`), `register_ack_waiter` (`src/client/messaging.rs`), `should_issue_tc_token` (`src/send/tctoken_lifecycle.rs`), all `pub(crate)` helpers whose only caller is VoIP.

**Recommendation.**
1. `wacore-mlow` crate (`wacore/mlow/`): move `wacore/src/voip/mlow/` + `tables.proto/.desc` + `build.rs` there. `wacore`'s `build.rs` disappears (or shrinks to nothing), `buffa-build`/`sha2` leave `wacore`'s `[build-dependencies]`, and the 16 MB testdata leaves the published `wacore` package. Expose the `ForeignAudioCodec`-style trait `wacore::voip::audio` already has so `engine.rs` takes the codec by trait instead of naming `MlowEncoder`/`MlowDecoder` (17 refs).
2. `wacore-voip` crate (`wacore/voip/`): move the rest of `wacore/src/voip/` there, keeping `wacore::types::call` / `types::group_call` / `stanza::call` in `wacore` (they are needed by `Event::IncomingCall` and the ungated `reject_call`).
3. `whatsapp-rust-voip` (`voip/`): `src/voip/**` + the gated halves of `src/client/voip.rs` and `src/handlers/call.rs` (finding 2). Make the three `pub(crate)` helpers `#[doc(hidden)] pub` and record that in `subsystem_boundary.md`, which is the decision the doc says it deferred.
4. Keep `whatsapp_rust::voip::*` paths as `pub use` re-exports behind the existing feature names so no consumer changes.

**Effect.** LOC delta ≈ 0 (moves) minus the `src/voip/{session,registry}.rs` re-export shims (12 lines) and the `wacore/build.rs` voip branch. `cargo package -p wacore` drops from 22 MiB to ~6 MiB. Default `wacore` builds stop running a build script. Workspace `clippy --all-features` and `cargo doc --workspace` get to parallelize 85k lines that today sit on the critical path of the `wacore` and `whatsapp-rust` compilation units. **Risk:** medium (crate boundaries, publish ordering, docs.rs feature lists). **Behaviour-preserving:** yes.

**Quick win regardless of the split:** add `exclude = ["src/voip/mlow/testdata/*"]` to `wacore/Cargo.toml` (1 line; `cargo package` verification still builds because no non-test code includes those files).

### 2. `src/client/voip.rs` and `src/handlers/call.rs` carry 275 of the 284 `voip-runtime` gates in `src/` because ungated signaling and gated media share a file

**Evidence.** `grep -c 'feature = "voip-runtime"'`: `src/client/voip.rs` **156**, `src/handlers/call.rs` **119**; whole `src/` 284 (the doc's count at `ff4ac10` was 171, so it has grown 66%). The ungated surface is small: `Voip::reject` (`src/client/voip.rs:1088`), `reject_call` (`:1103`), `terminate` (`:2032`), `CallError` (`:1009`), and the `<call>` stanza router. Everything else in `impl Voip` (`accept`, `call`, `group_call`, `call_link`, `join_call_link`, `admit_waiting_user`, ... `:1152-1832`) is `#[cfg(feature = "voip-runtime")]` per method. `CallHandler::handle` is one **980-line** function (`src/handlers/call.rs:54-1034`) with 39 `CallAction::` arms and per-arm gates inside the match.

`agent_docs/subsystem_boundary.md` states the principle itself: "The difference is not the subsystem, it is whether the subsystem owns its own files." These two files are the counter-example.

**Recommendation.** Split each file on the gate: `src/client/voip.rs` keeps `Voip { reject, reject_call, terminate }` + `CallError` (ungated, ~250 lines); the rest moves to `src/voip/client.rs` under the single gated `mod voip`. `src/handlers/call.rs` keeps the parse + the offer/reject/terminate arms; media/group/call-link arms become `src/voip/handler.rs` functions the core calls through one `#[cfg]` seam (or through `Subsystem::handle_call_action`, which would be the second asker the doc requires for a fifth hook — VoIP and `pdo` both want a per-action point). Also break `handle` into one `async fn` per `CallAction` variant, the way `src/handlers/notification/` is organized.

**Effect.** −270 attribute lines, `handle` from 980 lines to ~40 + per-arm fns; gate count in `src/` drops from 284 to ~15, which is what makes finding 1 mechanical. **Risk:** low. **Behaviour-preserving:** yes.

### 3. `plugins/wam-catalog`: 132k generated lines to serve 9 event types

**Evidence.**
- `generated.rs` 79,029 lines (22,942 are `///` doc lines), 436 event structs + 436 `impl WamEvent` + 898 enums + 46 globals (`plugins/wam-catalog/src/generated.rs:86,45410,45837`). `call_sites.rs` 52,863 lines behind `parity` (`lib.rs:44-45`).
- Runtime consumers (`grep events::` in `plugins/wam`): `ReceiptStanzaReceive`, `WamDroppedEvent`, `MessageReceive`, `E2eMessageRecv`, `WamClientErrors`, `WebcSocketConnect`, `WebWamForceFlush`, `EncDecryptFailureReason` — 8 events + the `WamEvent` trait (`plugins/wam/src/runtime.rs:27`, `derive.rs:23`, `lib.rs:82`). `lib.rs:12-13` of the catalog itself says "the runtime that uses a dozen of its 436 events".
- `cargo check -p whatsapp-rust-wam-catalog` (deps cached): **23.8 s**; with `parity`: 19.9 s (noise; the two files cost about the same). The crate is not in `default-members`, but it is in `members`, so every CI leg that runs `--workspace` (build, clippy, doc, deny: `.github/workflows/main.yml:267,269,343`) pays for it, and rustdoc renders 1,334 items nobody links.
- The repo already has the right pattern: `wacore/src/types/wire_enums.rs` binds "only the catalog entries listed in the emitter's `WANTED`" (206 lines out of 403 catalog entries), per `AGENTS.md`.

**Recommendation.** In `tools/whatspec-codegen/src/emit/wam.rs`, emit the typed structs/impls only for a `WANTED` list (the 8 events + the enums they reference), and emit the full catalog as **data** (`const EVENTS: &[EventDef { name, code, channel, weights, fields: &[FieldDef] }]`) — one array literal, no derives, no `impl` per event — which is all `parity.rs` and `stats_stanza.rs` actually need to check a field id. Move `call_sites.rs` next to the parity test (`plugins/wam/tests/data/`) as a `parity`-only include, or into a `whatsapp-rust-wam-evidence` crate outside `members`.

**Effect.** −110k to −125k lines in tree; catalog check time from ~24 s to a few seconds; adding a 9th event is a `WANTED` edit, not a policy change. **Risk:** low (parity test keeps the same evidence). **Behaviour-preserving:** yes.

### 4. `sqlite_store.rs`: the retry loop is hand-unrolled eight times next to a helper that does it

**Evidence.** `with_retry` (`storages/sqlite-storage/src/sqlite_store.rs:781-840`) is used 27 times. Seven more methods inline a byte-for-byte copy of the same `for attempt in 0..=MAX_RETRIES { permit; spawn_blocking; match ... }` loop: `put_identity_for_device` (`:1225-1281`), `put_session_for_device` (`:1359-1416`), `store_prekey` (`:2055-2109`), `store_prekeys_batch` (`:2121-2180`), and the loops at `:2229`, `:2325`, `:2413` (each ~55 lines). They differ only in the Diesel statement and the `warn!` text — and they have **diverged**: the helper caps backoff at `10 * (1 << attempt.min(4))` = 160 ms and warns from the second retry (`:812-822`), while `put_identity_for_device` uses `10 * 2u64.pow(attempt)` (uncapped, 320 ms at attempt 5) and warns on every retry (`:1268-1274`). `is_retriable_sqlite_error` is shared, so the retriable set is the same.

Also: 40 `StoreError::Connection(Box::new(e))` and 93 `StoreError::Database(Box::new(e))` `map_err` sites; `DieselOrStore` exists (`:23-40`) only to bridge this inside `spawn_blocking`.

**Recommendation.** Route the seven loops through `with_retry` (the `Arc<str>`/`Bytes` refcount-clone-per-attempt optimisation they carry is already what `with_retry`'s `make_op: Fn() -> Box<dyn FnOnce>` shape supports). Add a private `trait ConnErr { fn conn_err(self) -> StoreError }`-style extension for the two `map_err` shapes (orphan rules stop a `From<DieselError> for StoreError` in this crate).

**Effect.** −350 lines; one backoff policy. **Risk:** low. **Behaviour-preserving:** almost — backoff cap and warn cadence become the helper's (arguably the intended ones).

### 5. `sqlite_store.rs`: the `_for_device` public API doubles every Signal/app-state method

**Evidence.** 20 `pub async fn *_for_device(.., device_id: i32)` methods (`:1213-1836`) and 20 one-line trait forwarders `self.x_for_device(.., self.device_id)` (e.g. `SignalStore::put_identity` `:1879-1882`, `delete_identity` `:1932`, `get_session` `:1937`, `put_session` `:2010`, ...). 106 mentions of `_for_device` in the file. `SqliteStore::new_for_device` / `with_config_for_device` (`:452,460`) already produce a store bound to a device id, and `SharedSqlite` (`shared.rs:18`) exists for sharing a pool across per-device stores.

**Recommendation.** Make the trait impl the only implementation and drop the `_for_device` twins; a caller that needs another device's rows uses `SharedSqlite::store_for_device(id)` (a `SqliteStore` with `device_id` set). If a couple of multi-account admin paths genuinely need cross-device access from one handle, keep those two and delete the other eighteen.

**Effect.** −300 to −400 lines; one code path per table op. **Risk:** medium (public API of the storage crate; `share_for_device_tests` at `:7033` shows there are consumers of the concept). **Behaviour-preserving:** yes.

### 6. `CallRegistry` exposes 99 public methods; 26 are `_if_current` twins of the other 26

**Evidence.** `wacore/src/voip/registry.rs:692-3107` (`impl CallRegistry`): 99 `pub fn`s; 26 end in `_if_current` and duplicate the body of their sibling with `.filter(|entry| entry.generation == generation)` inserted, e.g. `set_raised_hand` (`:1223-1231`) vs `set_raised_hand_if_current` (`:1233-1249`), `set_screen_share`/`_if_current`, `apply_group_update`/`_if_current`, `send_call_event`/`_if_current`, `remove`/`remove_if_current`/`remove_if_current_with_phase`. `CallEntry` (`:229`) has 30 fields.

**Recommendation.** One private accessor `fn entry_mut(&self, call_id, generation: Option<u64>) -> Option<&mut CallEntry>` and one public method per operation taking `generation: Option<u64>` (or a `CallRef<'_> { call_id, generation }` key). Call sites that pass the generation keep passing it; those that don't pass `None`.

**Effect.** −300 lines, API halves. **Risk:** low. **Behaviour-preserving:** yes.

### 7. `src/voip/facade.rs`: four Drop-guard teardown types with the same fields and the same drop body

**Evidence.** `RegisteredCall` (`:2535-2640`), `AnswerTeardown` (`:2640-2900`), `GroupOfferTeardown` (`:2652, 2747-2809`), `GroupRekeyTeardown` (`:2662-2745`). Each holds `{client: Weak<Client>, registry, call_id, call_creator, generation, armed}`, exposes `disarm()`, and its `Drop` does `if !armed return; upgrade client; clone fields; runtime.spawn(async { take transition lane; remove_if_current; send <terminate> })` with a different terminal stanza. `AnswerTeardown` additionally carries the lane guard so a cancelled future can hand it to the detached retry (`:2828-2837`) — the one real difference.

Aside from the guards, the file's non-test part is 1,476 lines with `attach_engine` at 294 lines (`:3014`), `place_call` 271 (`:1785`), and three `start()` bodies (225-268 lines each at `:196`, `:584`, `:717`) that all do the same audio/video endpoint negotiation through `AudioEndpoints`/`VideoEndpoints`/`NegotiatedAudioPlan` before diverging.

**Recommendation.** One `Teardown { kind: TeardownKind::{Answer{peer, lane}, GroupOffer{only_if_admitted}, GroupRekey}, ... }` with a single `Drop`; extract the shared pre-flight of the three `start()`s into `fn negotiate_endpoints(&mut self) -> Result<Negotiated, CallError>`.

**Effect.** −150 to −250 lines; one place for the generation-ownership invariant. **Risk:** medium (this is the code that guarantees `<terminate>` is sent exactly once per generation; the tests at `:7999-10354` cover it well). **Behaviour-preserving:** yes.

### 8. VoIP hot paths are single functions of 350–670 lines

**Evidence.** `wacore/src/voip/driver.rs::run_call_with_clock_and_wallclock` **667 lines**; `wacore/src/voip/engine.rs::on_rtp` **403**, `on_group_rtp` **350**, `CallConfig::for_group` **284**, `observe_group_codec_content` 188, `apply_group_update` 175. `src/handlers/call.rs::handle` 980 (finding 2).

The split itself (engine = sans-IO state machine; driver = select loop over relay/channels/timer; registry = per-call ownership + generations; session = SRTP/SFrame pipelines; facade = builders + `CallHandle`) is principled — there is exactly one `CallPhase` enum (`wacore/src/voip/session.rs:37`), referenced from 7 files, and the engine holds media state (`MediaState`, `VideoPlaneState`, `GroupEngineState` at `engine.rs:895-1121`) that the registry does not mirror. What is duplicated is not state but *dispatch*: `on_rtp` and `on_group_rtp` both inline demux → replay check → HBH unprotect → E2E unprotect → codec corroboration → jitter/stats, with the group version adding roster lookup.

**Recommendation.** Factor the shared RTP receive stages into `fn unprotect_and_classify(&mut self, now, pkt, roster: Option<&Roster>) -> Result<Classified, Drop>` used by both; split `run_call_with_clock_and_wallclock` into named handlers per select arm. No API change.

**Effect.** ~−200 lines from de-duplicating the two RTP paths; large complexity win. **Risk:** medium (hot path; the 7k lines of engine tests are the safety net). **Behaviour-preserving:** yes.

### 9. `src/plugins/mod.rs` is one 6,558-line file, 52% of it tests

**Evidence.** `#[cfg(test)] mod tests` starts at `src/plugins/mod.rs:3144` and runs to the end (3,415 lines). The non-test 3,143 lines hold six separable units: capability/config/manifest types (`:57-210`), `PluginResources` + task tracking + weak indices (`:378-1030`), diagnostics (`:1033-1170`), the per-capability handles `PluginTasks`/`PluginCoreEvents`/`PluginStanzaInterception`/`PluginMessaging`/`PluginIq`/`PluginContext`/`PluginConnectionScope` (`:1171-1830`), the erased-plugin adapters + `ApiRegistry` (`:1827-1960`), and `PluginPlan`/`PluginHost`/`ClientLifecycle` impl (`:1960-3140`). `PluginAdapter<P>` and `UntypedPluginAdapter<P>` (`:1890-1960`) are two 40-line impls of `ErasedClientPlugin` that differ only in `marker_type_id`.

**Recommendation.** `src/plugins/{mod,capability,resources,context,host,adapter}.rs` + `src/plugins/tests/`. Replace the two adapters with one `PluginAdapter<P: ?Sized>` over a `trait PluginMarker { fn type_id() -> Option<TypeId> }` blanket-implemented for both plugin traits.

**Effect.** −40 lines; file sizes 300–900. **Risk:** none. **Behaviour-preserving:** yes.

### 10. Feature matrix: 33 root features, several are aliases or gate nothing

Root crate features (`Cargo.toml:139-231`): `debug-snapshots`, `legacy-session-interop`, `client-lifecycle`, `plugins`, `passkey`, `tracing`, `metrics`, `tracing-pii`, `bench-harness`, `danger-skip-tls-verify`, `danger-skip-cert-chain-verify`, `default`, `ureq-client`, `tokio-transport`, `tokio-runtime`, `signal`, `sqlite-storage`, `tokio-native`, `voip-runtime`, `voip-relay-native`, `voip`, `voip-encoded`, `voip-mlow`, `voip-libopus` (24 named + the workspace-forwarded ones). `wacore`: 12 (`debug-snapshots`, `legacy-session-interop`, `test-util`, `tracing`, `metrics`, `tracing-pii`, `danger-skip-cert-chain-verify`, `js`, `voip`, `voip-mlow`, `dhat-heap`, `bench-internals`).

`#[cfg(feature = ...)]` counts, `src/`: `voip-runtime` 284, `tracing` 197, `plugins` 91, `client-lifecycle` 72, `voip-libopus` 18, `tokio-runtime` 16, `ureq-client` 14, `voip-relay-native` 5, `tokio-transport` 5, `bench-harness` 4, `signal` 3, `passkey` 3, `debug-snapshots` 3, `sqlite-storage` 2, `voip-mlow` 1 (test-only, `src/voip/transport/native.rs:1726`), `tracing-pii` 1, **`metrics` 0, `tokio-native` 0, `voip-encoded` 0, `voip` 0**. `wacore/src/`: `voip-mlow` 48, `tracing` 34, `test-util` 26, `voip` 22, `bench-internals` 3, `metrics` 2, `debug-snapshots` 2, `tracing-pii` 1.

Specific findings:
- **`voip-encoded = ["voip-runtime"]`** (`Cargo.toml:224`) gates nothing; its three mentions are doc comments (`src/voip/mod.rs:29`, `src/client/voip.rs:1150,1162`). It exists so `voip-libopus` can depend on it. Fold into `voip-runtime` (keep the name as an alias if a consumer might spell it).
- **`tokio-native = ["tokio-runtime", "tokio/rt-multi-thread"]`** has zero gates; **`bench-harness`** is the same set plus one gated module (`src/lib.rs`, 4 gates). `tokio-native`, `signal`, `bench-harness` are three ways to say "tokio-runtime plus a tokio feature".
- **`metrics`** on the root crate (`Cargo.toml:168-171`, "Optional metrics (counters/histograms/gauges…)") gates nothing under `src/`; the two sites are in `wacore/src/telemetry.rs:22,240`. Either the root doc is overstated or `src/` should emit.
- **`client-lifecycle`** is enabled only by `plugins` (`Cargo.toml:155`) and by nothing else in the workspace (`grep client-lifecycle --include=Cargo.toml` → root only), yet costs 72 gates in five core files. In practice `client-lifecycle ⇔ plugins`. Merge (keep the name as an alias) unless an external consumer is known to use it alone.
- **`wacore/js`** only forwards `getrandom/wasm_js`; the bridge declares `getrandom` with `wasm_js` itself (`bridge/Cargo.toml:108`). Its sole consumer is the CI wasm guard (`.github/workflows/wasm.yml:71,97`).
- **Combinations that cannot build** are all intentional and guarded: `voip-relay-native` × wasm32/espidf (`src/voip/mod.rs:23-33` `compile_error!`); `dhat-heap`/`danger-*`/`tracing-pii`/`js` excluded from the shared test run by `scripts/ci/test_features.sh`. No accidental one found; `cargo hack --each-feature --no-dev-deps` per crate (`main.yml:410-412`) is what keeps that true, and every alias feature is one more leg of that walk.
- **`tracing`** at 197 gates in `src/` (vs 34 in `wacore`) is the densest gate in the repo. Cross-cutting by definition, but a `#[cfg_attr(feature = "tracing", instrument(...))]`/`trace_span!`-style shim that expands to nothing would replace most attribute pairs. (Other agent's files; noted for the matrix only.)

**Effect.** Retiring `voip-encoded`, `tokio-native`, and (possibly) `client-lifecycle` as real features: −3 `cargo hack` legs per CI run, −72 gates if lifecycle merges. **Risk:** low if kept as aliases. **Behaviour-preserving:** yes.

### 11. Dependency audit

- **Duplicate versions (all upstream-driven; `deny.toml:53` has `multiple-versions = "deny"` with a skip list):** `base64` 0.22 (buffa, tokio-websockets, dev: metrics-exporter-prometheus) vs 0.23 (ours, ureq); `hashbrown` 0.15 (buffa) vs 0.17; `getrandom` 0.2 (ring, rand_core 0.6 via crypto-common) vs 0.4; `syn` 2 (asn1-rs-derive, curve25519-dalek-derive, buffa-codegen, diesel's darling 0.21) vs 3 (async-trait, bon, serde_derive, thiserror, tokio-macros, futures-macro, wacore-derive); `darling` 0.21 (diesel_derives) vs 0.24 (bon-macros); `winnow` 0.7 + 1.0 (both via `toml` ← `migrations_internals` ← `diesel_migrations`). Actionable now: pin our `base64` to `0.22` until buffa/tokio-websockets move (−1 crate in the default build). The `toml`+2×`winnow` chain is pulled only so `diesel_migrations` can read a `diesel.toml` at build time.
- **Heavy dep used trivially:** `examples/voip-cli` is a workspace member that links `cpal` (ALSA) and `ringbuf` (`examples/voip-cli/Cargo.toml`); `main.yml:249` already has a comment about it needing system libs. It is compiled by every `--workspace --all-features --all-targets` build/clippy leg. Move it out of `members` into `[workspace] exclude` with its own lockfile, or make it an example under `--no-default-features` gating.
- **Removable without touching `wacore`:** `scopeguard` in the root crate has two real uses (`src/voip/facade.rs:3246`, `src/retry.rs:514`); a 6-line local `Defer` guard removes the dependency. `itoa` (2 refs) and `hashbrown` (2 refs) in `src/` could come through `wacore` re-exports. `chrono` in `src/` (13 refs) and `hex` (5) are kept anyway by `wacore`, so no crate saved.
- **VoIP stack:** `rtc-dtls`/`rtc-sctp`/`rtc-datachannel`/`rtc-shared` pinned `=0.21.0-beta.2` (`Cargo.toml:266-279`), +116 crates over default. Already optional and well-justified in comments; nothing to do beyond finding 1.

### 12. Transport/HTTP abstraction: right-sized on the transport side, one wide half on the HTTP side, and eleven mock implementations

**Evidence.** `wacore/src/net.rs`: `Transport { send, disconnect, resource_report (default) }` (`:99-118`), `TransportFactory { create_transport }` (`:119`). Implementors: `tokio-transport` (`lib.rs:180-204,361`), bridge `js_transport.rs:260,304` (324 lines), `bench_support.rs:54,65` — all fit exactly. `HttpClient` (`:204-250`) has six methods: `execute` (async) plus `supports_streaming`/`execute_streaming` and `supports_upload_streaming`/`execute_upload`, which are **synchronous, blocking** functions on an async trait (`src/download.rs:1589` "std, not async: `execute_streaming` is a blocking call"). Only `ureq-client` implements the streaming half (`lib.rs:254-350`); the bridge's `js_http.rs` (95 lines) implements `execute` only; the two consumers are `src/download.rs:399,723,741` and `src/upload.rs:499`.

Test doubles: 11 `impl HttpClient for …` (`src/test_utils.rs:195,208`, `src/download.rs:1641,2086`, `src/bot.rs:1545`, `src/version.rs:271,283`, `src/features/presence.rs:263`, `src/bench_support.rs:77`, `plugins/metrics/src/lib.rs:414`) and 14 `impl Transport for …` (seven in `src/socket/noise_socket.rs` alone, plus `src/transport.rs:20,136`, `tests/handshake_integration.rs:105`, `tests/handshake_span_scope.rs:119`, `plugins/metrics/src/lib.rs:393`).

**Recommendation.** Keep `Transport`. For `HttpClient`, split the streaming half into `trait StreamingHttpClient: HttpClient` implemented by ureq only, and let `download.rs`/`upload.rs` downcast via a `fn as_streaming(&self) -> Option<&dyn StreamingHttpClient>` default method — same behaviour, no `supports_*` booleans, and a wasm implementor stops seeing blocking fns it cannot implement. Consolidate the mocks into `src/test_utils.rs` (`MockHttpClient` with a `Vec<Response>` script and a captured-request log covers the canned/routed/status-only/header-capturing variants) and a `RecordingTransport` with an optional fail-after-N — roughly −250 lines across `src/`, `tests/`, `plugins/metrics`.

**Risk:** low. **Behaviour-preserving:** yes.

### 13. Benches and e2e: one good shared harness, but nine counting allocators and three fixture builders

**Evidence.** `tests/e2e/src/lib.rs` (682 lines; `TestClient` with 23 methods at `:172-613`) is reused by `tests/bench-integration/benches/integration.rs` — good. Duplicated: `impl GlobalAlloc` appears nine times (`src/lib.rs:23` test-only counting allocator, `wacore/binary/tests/{jid_non_ad_arc_alloc,marshal_exact_hint_alloc,attrs_inline_alloc,jid_identity_alloc}.rs`, `wacore/tests/hash_table_bytes_matches_the_allocator.rs`, `examples/alloc_tracking.rs`, `tests/e2e/tests/per_client_retention.rs`, `tests/bench-integration/benches/integration.rs`). Three independent client-fixture builders exist for three layers — `src/bench_support.rs` (562 lines, `bench-harness`), `wacore/benches/send_receive_benchmark.rs` (1,155 lines, own `warm_pair`), and e2e `TestClient` — which is defensible, but `#![recursion_limit = "512"]` is repeated in `src/lib.rs`, `tests/e2e/src/lib.rs:4`, `tests/bench-integration/benches/integration.rs:11` and `examples/demo.rs` for the same `--all-features` future-size reason.

**Recommendation.** A `wacore::test_util::alloc::{CountingAlloc, DeterministicAlloc}` (behind the existing `test-util` feature) replaces seven of the nine; the two in `wacore/binary` can share a `tests/common/mod.rs`. **Effect:** −200 lines. **Risk:** none.

### 14. Generated IQ registries compile into every default `wacore` build; the `WANTED` pattern is applied to one file but not the two biggest

**Evidence.** `wacore/src/iq/abprops.rs` 18,718 lines / 2,664 `pub const` (43 external refs), `wacore/src/iq/mex_operations.rs` 11,032 lines / 572 consts + 947 structs (**9** external refs). Both are unconditionally compiled, unlike the WAM catalog. `wire_enums.rs` (206 lines) demonstrates the `WANTED` approach the AGENTS.md documents. `tests/ab_prop_watch_coverage.rs` (175 lines) reads `ALL`, so a data-only `ALL` slice must remain.

**Recommendation.** Emit typed items only for a `WANTED` list; emit the remainder as one `ALL: &[AbPropDef]` data table (for coverage tests and `props::stale`). Consts are cheap for codegen but 947 structs with derives in `mex_operations.rs` are not. **Effect:** −25k lines from the default build. **Risk:** low. (Consumer side only; `wacore` internals are the other agent's.)

### 15. Small VoIP structure debris

- `src/voip/session.rs` (7 lines) and `src/voip/registry.rs` (5 lines) are pure `pub use wacore::voip::…` re-exports "to keep the path stable"; `src/voip/driver.rs` (53 lines) is one struct + one 8-line fn. Fold all three into `src/voip/mod.rs`. −20 lines.
- `wacore/src/voip/mlow/mod.rs:10-43` declares `golden`, `quality_tests`, `quality_metrics`, `param_decode_match` as ordinary `mod`s; each is `#![cfg(test)]` internally. Gate them at the declaration so a reader (and rustdoc) sees they are tests. 0 lines.
- `storages/sqlite-storage/src/sqlite_store.rs:6903-6972` has a test that `include_str!`s its own source and scans for `pub async fn` names to detect misrouted reads. It works, but it is the reason the file cannot be split (finding 4/5 make the split natural: `signal.rs`, `app_state.rs`, `device.rs`, `msg_secret.rs`, with the scan pointed at the directory).

---

## Answers to the specific questions

- **VoIP code vs tests:** 85.6k lines total, ~39.7k code (14.5k of it MLow) and ~45.9k tests. The two largest files are 87% tests (`facade.rs` 10,053/11,529) and 63% tests (`engine.rs` 7,121/11,238). `driver.rs` is 41% tests (mod tests at `wacore/src/voip/driver.rs:1780`).
- **Is the facade/engine/driver/registry/session split principled?** Yes as a layering (one `CallPhase`, engine owns media state, registry owns ownership/generation, driver owns the select loop, facade owns builders/handles). The duplication is *within* layers (finding 6, 7, 8), not between them.
- **Is MLow a hand-port that belongs in its own crate?** Yes on both counts (finding 1): C-symbol-named modules, "match C reference" tests, 12 `crate::` references all internal, its own `build.rs` codegen, and 16 MB of vectors that ship in the `wacore` package.
- **Does VoIP belong in the core crate?** The *signaling* half does (bridge calls `reject_call`, consumes `Event::IncomingCall`); the media plane does not — a default build compiles zero of it, the bridge enables none of it, and `subsystem_boundary.md` already identifies the three helpers that are the only coupling.
- **What consumes the WAM catalog?** `plugins/wam` only, 8 events + the trait; `call_sites.rs` is consumed only by the `parity` test. Neither is needed at runtime by any published crate; the catalog crate is `publish = false`.
- **sqlite_store repetition:** the retry loop ×8 and the `_for_device` ×20 doubling are the two real ones; Diesel schema (`schema.rs`, 219 lines, 18 tables) vs store traits is not duplicated — the schema is the only per-table declaration. Reads already take one erased `read_query` (the file's `:701-709` comment records the 90 KiB `.text` reason). No N+1 found: batch paths use one `spawn_blocking` + `IN (...)` (`:1834`). Per-call `pool.get()` inside `spawn_blocking` is the correct shape for r2d2.

## Quick wins (each ≤ 1 hour, no behaviour change)

1. `wacore/Cargo.toml`: `exclude = ["src/voip/mlow/testdata/*"]` — packaged crate 22 MiB → ~6 MiB. (finding 1)
2. Route the seven inline retry loops in `sqlite_store.rs` (`:1225, :1359, :2055, :2121, :2229, :2325, :2413`) through `with_retry`. −350 lines. (finding 4)
3. `CallRegistry`: private `entry_mut(call_id, Option<generation>)`, delete 26 `_if_current` twins. −300 lines. (finding 6)
4. Fold `src/voip/{session,registry,driver}.rs` into `src/voip/mod.rs`. −20 lines. (finding 15)
5. Gate the four test-only `mlow` modules at their `mod` declarations. (finding 15)
6. Move `examples/voip-cli` out of `[workspace] members` (or into `exclude`) so `--workspace --all-features` CI stops linking `cpal`/ALSA. (finding 11)
7. Retire `voip-encoded` and `tokio-native` as real features (keep as aliases); −2 `cargo hack` legs. (finding 10)
8. Replace `scopeguard` with a 6-line local guard (2 uses). −1 dependency. (finding 11)
9. Pin `base64` to `0.22` to match buffa/tokio-websockets. −1 duplicate crate in the default build. (finding 11)
10. Split `src/plugins/mod.rs` tests into `src/plugins/tests/`. 0 lines, file halves. (finding 9)
11. Merge `PluginAdapter`/`UntypedPluginAdapter` via a marker trait. −40 lines. (finding 9)
12. Emit `call_sites.rs` as a `parity`-only data file next to the test instead of a `pub mod` of a workspace member. −53k lines from `cargo doc/clippy --workspace`. (finding 3)
13. Fix the root `metrics` feature doc (`Cargo.toml:168-171`) or add the `src/` emit sites it promises. (finding 10)
14. Consolidate the nine `GlobalAlloc` test allocators behind `wacore/test-util`. −200 lines. (finding 13)
