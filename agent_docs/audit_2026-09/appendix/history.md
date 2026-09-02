# Git-history view of maintenance cost: whatsapp-rust, whatsapp-rust-bridge, baileyrs

Window: 12 months, 2025-09-02 to 2026-09-02, non-merge commits unless stated. "fix" = subject starts with `fix`/`revert`/`hotfix` (case-insensitive). Sizes are line counts at HEAD. Generated files, lockfiles and CHANGELOG are excluded from hot-file and coupling tables (see section 6 for the generated set).

## 0. History depth obtained

All three were shallow clones at 51 commits. `git fetch --unshallow origin` succeeded on the first try for each (no retry needed).

| Repo | Before | After unshallow | First commit | Commits in 12 mo | Tags |
| --- | --- | --- | --- | --- | --- |
| whatsapp-rust (core) | 51 | 1,620 | 2025-06-26 | 1,277 (1,270 non-merge) | 10 |
| whatsapp-rust-bridge | 51 | 285 | 2025-08-18 | 267 (259) | 21 |
| baileyrs | 51 | 150 | 2026-04-16 | 150 (all of it) | 16 |

Monthly volume (core): 54, 52, 17, 45, 81, 23, 177, 127, 56, 270, 197, 164, 14 (Sep 2025 to Sep 2026). Bridge: 96 of its 267 commits are from August 2026; baileyrs: 77 of 150. The August spike is the 1.0-push / release-please era, and most of the cascade evidence below comes from it.

Two file moves matter for reading the core numbers: `src/client.rs` was split into `src/client/{lifecycle,node_io,accessors,app_state}.rs` on 2026-06-05 (it was 5,660 lines on 2026-04-01, 914 right after the split, 2,185 today), and `src/send.rs` became `src/send/mod.rs` on 2026-06-18 (222 commits with `--follow`). Counts below are per path unless marked.

## 1. Hot files vs size

### Core (top 25 by commits; fix = fix/revert commits touching it)

| # | File | Commits | Lines | Fix | Note |
| --- | --- | --- | --- | --- | --- |
| 1 | src/client.rs | 293 | 2,185 | 94 | split 06-05, re-grew 914 to 2,185 in 3 months |
| 2 | src/message.rs | 130 | 439 | 47 | most of it moved to src/message/receive.rs on 06-05 |
| 3 | Cargo.toml | 130 | 414 | 9 | version bumps across 9 crates (see coupling) |
| 4 | src/retry.rs | 117 | 5,357 | 32 | 150 commits all-time; 333 lines in Oct 2025, 5,357 now |
| 5 | src/lib.rs | 114 | 318 | 9 | re-export surface |
| 6 | wacore/src/send.rs | 99 | 83 | 28 | now a shim; real code in wacore/src/send/ |
| 7 | wacore/Cargo.toml | 89 | 181 | 4 | |
| 8 | wacore/src/types/events.rs | 88 | 3,272 | 23 | frozen event API, still 88 touches |
| 9 | storages/sqlite-storage/src/sqlite_store.rs | 82 | 7,235 | 19 | huge and hot |
| 10 | src/client/lifecycle.rs | 80 | 3,569 | 29 | born 06-05: 80 commits in 90 days |
| 11 | src/bot.rs | 80 | 2,433 | 14 | |
| 12 | src/receipt.rs | 61 | 3,631 | 14 | longest all-fix streak in repo (9 consecutive fix commits) |
| 13 | src/send/mod.rs | 60 (222 w/ follow) | 8,158 | 17 (51 w/ src/send.rs) | huge and hot |
| 14 | src/features/groups.rs | 59 | 3,081 | 12 | |
| 15 | src/appstate_sync.rs | 55 | 1,782 | 15 | |
| 16 | src/client/sessions.rs | 54 | 1,936 | 14 | |
| 17 | src/client/node_io.rs | 53 | 2,145 | 14 | born 06-05 |
| 18 | wacore/binary/src/jid.rs | 52 | 2,836 | 12 | |
| 19 | src/client/tests.rs | 51 | 7,833 | 18 | |
| 20 | src/client/device_registry.rs | 51 | 4,124 | 13 | |
| 21 | wacore/src/store/traits.rs | 51 | 1,222 | 14 | |
| 22 | wacore/src/messages.rs | 48 | 2,971 | 8 | |
| 23 | src/pdo.rs | 47 | 1,811 | 12 | |
| 24 | src/prekeys.rs | 47 | 1,491 | 11 | |
| 25 | src/features/mod.rs | 45 | 120 | 3 | |

Next: src/message/tests.rs 45 (16,246 lines), wacore/binary/src/encoder.rs 44, wacore/src/iq/groups.rs 43 (5,758), wacore/src/store/device.rs 43, wacore/src/store/signal_cache.rs 40 (6,226), src/client/lid_pn.rs 40 (3,183).

Churn volume per commit (12 mo added+deleted / commits): client.rs 72, retry.rs 88, lifecycle.rs 55, events.rs 54, sqlite_store.rs 167, send/mod.rs 179, signal_cache.rs 186, app_state.rs 228, history_sync.rs 241, voip/facade.rs 390, voip/engine.rs 719, plugins/mod.rs 852. The last four are "written in big chunks", the first four are "edited constantly in small pieces": the second pattern is the maintenance-cost one.

Recent heat (last 3 months only, commits / fixes): src/client.rs 92/32, lifecycle.rs 80/29, send/mod.rs 60/17, node_io.rs 53/14, tests.rs 51/18, events.rs 47/15, retry.rs 44/14, message/receive.rs 38/13, app_state.rs 36/13, voip/facade.rs 35/14, bot.rs 35, sqlite_store.rs 32/8, signal_cache.rs 26.

### Core: large files and whether anyone touches them

| File | Lines | Commits 12 mo | Fix | Verdict |
| --- | --- | --- | --- | --- |
| src/message/tests.rs | 16,246 | 45 | 17 | test file; cost is compile time, not design |
| src/voip/facade.rs | 11,529 | 35 (all since June) | 14 | new subsystem being stabilized, 46% fix rate |
| wacore/src/voip/engine.rs | 11,238 | 17 | 7 | new, written in 700-line chunks |
| src/send/mod.rs | 8,158 | 60 | 17 | huge AND hot |
| src/client/tests.rs | 7,833 | 51 | 18 | test file |
| storages/sqlite-storage/src/sqlite_store.rs | 7,235 | 82 | 19 | huge AND hot; coupled 1.0 with schema.rs, 0.78 with store/traits.rs |
| src/client/app_state.rs | 6,850 | 36 | 13 | hot in bursts |
| wacore/src/send/tests.rs | 6,691 | 27 | 10 | test file |
| wacore/src/voip/registry.rs | 6,599 | 15 | 9 | new |
| src/plugins/mod.rs | 6,558 | 8 | 0 | cold: leave alone |
| wacore/src/store/signal_cache.rs | 6,226 | 40 | 10 | 22 perf commits; fixes stopped 08-07 |
| wacore/src/iq/groups.rs | 5,758 | 43 | 6 | hot but low fix ratio: stable growth |
| src/client/voip.rs | 5,464 | 19 | 10 | new |
| src/retry.rs | 5,357 | 117 | 32 | huge AND hot AND fragile |
| src/handlers/call.rs | 4,898 | 25 | 11 | new |
| wacore/src/history_sync.rs | 4,846 | 33 | 8 | 15% fix ratio; medium-cold |
| wacore/src/voip/driver.rs | 4,311 | 11 | 3 | cold |
| src/client/device_registry.rs | 4,124 | 51 | 13 | hot |
| src/client/lifecycle.rs | 3,569 | 80 | 29 | hot AND fragile |

VoIP is 33,600 lines across facade/engine/registry/driver and has existed for 3 months (80 commits, 37 fixes = 46%). It is the youngest large surface; its fix rate is a stabilization curve, not a design signal yet.

### Bridge (top 25)

| # | File | Commits | Lines | Fix |
| --- | --- | --- | --- | --- |
| 1 | package.json | 121 | 72 | 17 |
| 2 | src/wasm_client.rs | 65 | 5,208 | 12 |
| 3 | Cargo.toml | 50 | 200 | 3 |
| 4 | src/lib.rs | 36 | 125 | 0 |
| 5 | ts/index.ts | 23 | 77 | 4 |
| 6 | .github/workflows/ci.yml | 22 | 374 | 6 |
| 7 | .release-please-manifest.json | 22 | 3 | 0 |
| 8 | src/js_backend.rs | 19 | 1,824 | 0 |
| 9 | src/result_types.rs | 17 | 1,364 | 2 |
| 10 | AGENTS.md | 17 | 237 | 6 |
| 11 | src/errors.rs | 15 | 1,574 | 4 |
| 12 | codegen/src/main.rs | 13 | 2,355 | 3 |
| 13 | src/proto.rs | 13 | 229 | 1 |
| 14 | src/js_transport.rs | 12 | 324 | 1 |
| 15 | src/runtime.rs | 12 | 553 | 0 |
| 16 | tests/e2e-messaging.test.ts | 11 | 257 | 3 |
| 17 | .github/workflows/release.yml | 10 | 412 | 3 |
| 18 | src/wire_batch.rs | 10 | 2,211 | 2 |
| 19 | src/camel_serializer.rs | 10 | 813 | 0 |
| 20 | src/audio.rs | 9 | 475 | 0 |
| 21 | ts/proto.ts | 8 | 349 | 3 |
| 22 | src/image_utils.rs | 8 | 214 | 0 |
| 23 | ts/wire-info.ts | 7 | 1,174 | 2 |
| 24 | src/crypto.rs | 7 | 97 | 1 |
| 25 | src/wasm_client/signal.rs | 6 | 727 | 2 |

`src/wasm_client.rs` size: 3,130 (Apr) -> 3,796 (Jun) -> 5,578 (Aug 1) -> 4,928 (Aug 20, after the 08-08 split #26 moved 4,664 lines into `src/wasm_client/*.rs`) -> 5,208 now. Since the split the root file has had 8 commits, the eleven submodules 5 combined: the split moved lines, not the change surface. It holds 201 `fn`s and 14 `#[wasm_bindgen]` blocks.

Large and cold in the bridge: ts/proto-types.d.ts 19,596 lines (generated, 6 commits), src/memory_profile.rs 1,125 (1 commit), codegen/src/proto_gen.rs 785 (2), src/wasm_client/connection.rs 656 (3), src/legacy_session.rs 480 (1), src/signal_records.rs 522 (2). Hand-written volume: 23,618 lines Rust, 2,264 lines TS, 8,988 lines tests, 106,023 lines generated.

### baileyrs (top 25)

| # | File | Commits | Lines | Fix |
| --- | --- | --- | --- | --- |
| 1 | package.json | 86 | 134 | 20 |
| 2 | src/Socket/events.ts | 23 | 1,255 | 5 |
| 3 | src/Socket/index.ts | 22 | 1,032 | 7 |
| 4 | src/Utils/messages.ts | 19 | 1,283 | 7 |
| 5 | .release-please-manifest.json | 17 | 3 | 0 |
| 6 | README.md | 16 | 461 | 1 |
| 7 | src/Socket/messages.ts | 16 | 356 | 7 |
| 8 | src/__tests__/regressions.test.ts | 14 | 2,238 | 3 |
| 9 | src/__fuzz__/harness/divergence.ts | 13 | 1,796 | 5 |
| 10 | src/Utils/wrap-legacy-store.ts | 12 | 17 | 2 |
| 11 | src/__fuzz__/harness/__tests__/harness.test.ts | 10 | 1,098 | 3 |
| 12 | src/Bridge/schema.ts | 10 | 1,175 | 3 |
| 13 | src/Bridge/types.ts | 9 | 769 | 2 |
| 14 | src/Socket/transport.ts | 9 | 185 | 1 |
| 15 | src/Types/Message.ts | 8 | 459 | 3 |
| 16 | src/Utils/__tests__/wrap-legacy-store-coverage.test.ts | 8 | 412 | 2 |
| 17 | src/Utils/event-buffer.ts | 8 | 567 | 4 |
| 18 | Example/example.ts | 8 | 434 | 3 |
| 19 | src/__tests__/e2e/retrocompat-api.test-e2e.ts | 8 | 251 | 0 |
| 20 | src/__tests__/e2e/send-receive-message.test-e2e.ts | 8 | 446 | 0 |
| 21 | src/Socket/groups.ts | 7 | 211 | 1 |
| 22 | src/Types/Events.ts | 7 | 205 | 0 |
| 23 | src/Bridge/__tests__/adapt.test.ts | 7 | 839 | 2 |
| 24 | src/__tests__/e2e/baileys-handoff.test-e2e.ts | 7 | 812 | 0 |
| 25 | src/Compatibility/proto-runtime.ts | 6 | 898 | 1 |

Large and cold in baileyrs: src/__fuzz__/pure-differential.fuzz.test.ts 1,509 (1 commit), proto-codec.fuzz.test.ts 1,293 (2), scripts/compatibility/audit-core.ts 1,043 (1), bridge-events.fuzz.test.ts 1,005 (4), scripts/compatibility/lifecycle-contract-core.ts 780 (1), socket-dispose-integration.test.ts 692 (1). baileyrs is 20,778 lines of source and 37,788 lines of tests/fuzz (1.8x). The single hottest non-manifest asset is a test harness (`divergence.ts`, 13 commits, 5 fixes, co-changes 1.0 with its own test).

## 2. Co-change coupling

Computed over the 200 hottest files per repo; pair reported if shared commits >= 8 and shared / min(commits_a, commits_b) >= 0.5. Commits touching more than 60 files (6 in core, 3 in baileyrs: mass renames and formatting) were skipped.

### Core: source pairs (manifests excluded)

| A | B | Shared | A total | B total | Ratio |
| --- | --- | --- | --- | --- | --- |
| src/client.rs | src/client/lifecycle.rs | 62 | 293 | 80 | 0.78 |
| storages/sqlite-storage/src/sqlite_store.rs | wacore/src/store/traits.rs | 40 | 82 | 51 | 0.78 |
| src/features/mod.rs | src/lib.rs | 38 | 45 | 114 | 0.84 |
| src/client.rs | src/client/accessors.rs | 30 | 293 | 37 | 0.81 |
| src/client.rs | src/handlers/ib.rs | 28 | 293 | 37 | 0.76 |
| src/client/accessors.rs | src/client/lifecycle.rs | 27 | 37 | 80 | 0.73 |
| storages/sqlite-storage/src/schema.rs | sqlite_store.rs | 23 | 23 | 82 | 1.00 |
| src/client/sender_keys.rs | src/retry.rs | 21 | 30 | 117 | 0.70 |
| src/cache_config.rs | src/client.rs | 20 | 26 | 293 | 0.77 |
| src/client.rs | src/handlers/router.rs | 18 | 293 | 22 | 0.82 |
| wacore/appstate/src/hash.rs | wacore/appstate/src/processor.rs | 17 | 21 | 31 | 0.81 |
| src/client.rs | src/lid_pn_cache.rs | 16 | 293 | 24 | 0.67 |
| wacore/src/stanza/call.rs | wacore/src/types/call.rs | 13 | 23 | 15 | 0.87 |
| sqlite_store.rs | wacore/src/store/commands.rs | 13 | 82 | 18 | 0.72 |
| wacore/src/store/commands.rs | wacore/src/store/device.rs | 13 | 18 | 43 | 0.72 |
| src/voip/facade.rs | wacore/src/voip/registry.rs | 12 | 35 | 15 | 0.80 |
| src/client.rs | tests/e2e/tests/memory_soak.rs | 12 | 293 | 16 | 0.75 |
| src/voip/facade.rs | wacore/src/voip/engine.rs | 12 | 35 | 17 | 0.71 |
| wacore/binary/src/encoder.rs | wacore/binary/src/marshal.rs | 11 | 44 | 14 | 0.79 |
| src/features/community.rs | src/features/groups.rs | 11 | 14 | 59 | 0.79 |
| src/handlers/call.rs | wacore/src/types/call.rs | 11 | 25 | 15 | 0.73 |
| src/client/lifecycle.rs | src/message/dispatch.rs | 11 | 80 | 16 | 0.69 |
| src/voip/facade.rs | src/voip/mod.rs | 10 | 35 | 12 | 0.83 |
| wacore/src/voip/driver.rs | wacore/src/voip/engine.rs | 9 | 11 | 17 | 0.82 |

Manifest cluster: the root `Cargo.toml` co-changes with `wacore/derive/Cargo.toml` (0.92), `wacore/appstate` (0.83), `wacore/noise` (0.81), `waproto` (0.81), `http_clients/ureq-client` (0.79), `wacore/binary` (0.73), `transports/tokio-transport`, `storages/sqlite-storage` at 10 to 33 shared commits each. Every release bumps nine manifests by hand.

Reading: `src/client.rs` is the hub of five distinct couplings (lifecycle, accessors, ib handler, router, cache_config, lid_pn_cache), which is the signature of a god-object whose split merely relocated the methods. The storage trait and its only implementation move in lockstep (40 shared of 51 trait commits), so the "platform-agnostic trait" boundary is not buying independent change. `retry.rs` and `sender_keys.rs` share 21 commits: retry logic owns sender-key state.

### Bridge

| A | B | Shared | A total | B total | Ratio |
| --- | --- | --- | --- | --- | --- |
| src/result_types.rs | src/wasm_client.rs | 17 | 17 | 65 | 1.00 |
| src/js_transport.rs | src/wasm_client.rs | 11 | 12 | 65 | 0.92 |
| src/runtime.rs | src/wasm_client.rs | 11 | 12 | 65 | 0.92 |
| src/camel_serializer.rs | src/wasm_client.rs | 9 | 10 | 65 | 0.90 |
| src/proto.rs | src/wasm_client.rs | 11 | 13 | 65 | 0.85 |
| src/js_backend.rs | src/wasm_client.rs | 16 | 19 | 65 | 0.84 |
| src/wasm_client.rs | tests/e2e-messaging.test.ts | 9 | 65 | 11 | 0.82 |
| src/wasm_client.rs | src/wire_batch.rs | 8 | 65 | 10 | 0.80 |
| src/lib.rs | src/runtime.rs | 9 | 36 | 12 | 0.75 |
| src/errors.rs | src/wasm_client.rs | 11 | 15 | 65 | 0.73 |
| src/wasm_client.rs | ts/index.ts | 16 | 65 | 23 | 0.70 |
| src/proto.rs | ts/index.ts | 9 | 13 | 23 | 0.69 |
| codegen/src/main.rs | src/wasm_client.rs | 8 | 13 | 65 | 0.62 |
| src/lib.rs | src/wasm_client.rs | 19 | 36 | 65 | 0.53 |
| package.json | .release-please-manifest.json | 21 | 121 | 22 | 0.95 |
| .github/workflows/ci.yml | release.yml | 9 | 22 | 10 | 0.90 |

Every result/error/transport/runtime/serializer file in the bridge changes only when `wasm_client.rs` changes. `result_types.rs` never changed without it (17/17). The bridge is one module with nine file names.

### baileyrs

| A | B | Shared | A total | B total | Ratio |
| --- | --- | --- | --- | --- | --- |
| src/__fuzz__/harness/__tests__/harness.test.ts | src/__fuzz__/harness/divergence.ts | 10 | 10 | 13 | 1.00 |
| src/Socket/events.ts | src/__tests__/regressions.test.ts | 10 | 23 | 14 | 0.71 |
| src/Socket/events.ts | src/Socket/index.ts | 11 | 23 | 22 | 0.50 |
| src/Socket/messages.ts | src/Utils/messages.ts | 8 | 16 | 19 | 0.50 |
| package.json | .release-please-manifest.json | 16 | 17 | 86 | 0.94 |
| package.json | src/Socket/events.ts | 14 | 86 | 23 | 0.61 |
| package.json | README.md | 10 | 86 | 16 | 0.62 |

`Bridge/schema.ts` with `Socket/events.ts` did not cross the threshold as a pair (they share 7 commits), but both change in bridge-bump commits (section 3). Coupling in baileyrs is low overall; the code is young and the change surface is dominated by dependency bumps.

## 3. Cross-repo cascade

Method. Bridge: every commit where `Cargo.lock`'s `whatsapp-rust` git rev changed (62 commits; the git pin started 2026-03-22). Lag = bridge commit date minus the date of the pinned core commit. baileyrs: every commit where `package.json`'s `whatsapp-rust-bridge` / `@oxidezap/whatsapp-rust-bridge` version changed (46 commits since the initial commit on 2026-04-16). Lag = baileyrs commit date minus the bridge tag date (or, for pre-tag alpha versions, the date of the first bridge commit carrying that version). "Hand-written" excludes manifests, generated files, tests, docs, CI and scripts.

### Core -> bridge

| Metric | Value |
| --- | --- |
| Bridge commits that move the core pin | 62 (41% of the 152 bridge commits since the pin began) |
| Core commits absorbed through those pins | 866 (core made 865 non-merge commits in that period: everything flows) |
| Core commits per pin | ~14 median-ish (range 1 to 180; #19 on 06-11 took 135, the 07-23 "major rewrite" took 180) |
| Lag core commit -> bridge pin | median 0 days, mean 0.1, max 2 |
| Pins that also touch non-manifest files | 49 / 62 |
| Pins that touch hand-written bridge source | 44 / 62 (71%) |
| Pins that are manifest-only | 13 / 62 |
| Pins that are generated-only or generated+tests | 5 / 62 |
| Pins marked breaking (`!`) | 8 / 62, all in Aug 2026 |
| Pins since core v0.7.0 (08-06) that point at unreleased core commits | 13 / 15 |

Hand-written bridge files touched inside pin commits: `src/wasm_client.rs` 35, `src/lib.rs` 13, `src/js_backend.rs` 13, `src/result_types.rs` 12, `ts/index.ts` 11, `src/js_transport.rs` 9, `src/errors.rs` 9, `codegen/src/main.rs` 7, `src/runtime.rs` 6, `src/proto.rs` 6, `src/camel_serializer.rs` 6. In addition `src/generated_types.rs` (the bridge's codegen mirror of core types) changes in 26 commits, 18 of them pins.

### Bridge -> baileyrs

| Metric | Value |
| --- | --- |
| baileyrs commits that move the bridge version | 46 in 132 days (one every 2.9 days) |
| Lag bridge release -> baileyrs adoption | median 0 days, mean 0.0, max 1 |
| Bumps touching non-manifest files | 29 / 46 |
| Bumps touching hand-written baileyrs source | 21 / 46 (46%) |
| Bumps that are manifest-only | 17 / 46 |
| Bumps where the bridge range contained a core pin change | 36 of the 41 measurable |
| ... of which baileyrs also edited hand-written source | 19 / 36 (53%) |
| Bumps with hand-written edits but no core change in range | 1 |
| End-to-end lag core commit -> baileyrs (via the pinned rev in the adopted bridge tag) | median 0, max 1 day (14 samples) |

Hand-written baileyrs files touched inside bump commits: `src/Socket/events.ts` 15, `src/Socket/index.ts` 10, `src/Bridge/types.ts` 9, `src/Socket/messages.ts` 8, `src/Utils/messages.ts` 7, `src/Bridge/schema.ts` 7, `src/Bridge/adapt.ts` 6, `src/Socket/groups.ts` 5, `src/Types/{Auth,Events,Message,Socket,index}.ts` 5 each, `src/Utils/wrap-legacy-store.ts` 5.

So the cascade is not a latency problem (a single maintainer lands all three the same day); it is a hand-mirroring tax: roughly 0.7 hand-edited bridge commits per core pin, and roughly 0.5 hand-edited baileyrs commits per bridge bump, with the edits landing in the same six or seven files every time.

### Concrete chains (core -> bridge -> baileyrs, same day unless noted)

1. 2026-08-12, bridge `5b2a9d0` `fix(deps)!: update whatsapp-rust so a degraded sync still announces the connection` (core `874328f3`): `src/generated_types.rs` +15, `ts/generated/whatsapp.ts` +2,532, `ts/proto-types.d.ts` +616, `tests/proto-schema-divergences.test.ts`, `scripts/check-size.ts` -> baileyrs `a687c45` `feat(events)!: move to bridge 0.10.0` touching 24 files: `Bridge/primitives.ts`, `Bridge/schema.ts`, `Bridge/types.ts`, `Compatibility/encode-proto.ts`, `Compatibility/proto-runtime.ts`, `Socket/events.ts`, `Socket/groups.ts`, `Socket/index.ts`, `Socket/messages.ts`, `Types/Auth.ts`, `Types/Events.ts`, ... (baileyrs' one breaking release of the month).
2. 2026-08-12, bridge `7355996` `fix(deps)!: ... caller's <biz> no longer duplicates the derived one` (core `8cea605f`): 1 line in `src/generated_types.rs` -> baileyrs `46bdb82` (bridge 0.11.0): `src/Compatibility/derived-stanza-nodes.ts`, `src/Socket/messages.ts`, new regression test.
3. 2026-08-14, bridge `472e6d8` `fix(deps)!: ... stranded participant recovers, and report a send that reached nobody` (core `3772e92b`): `src/errors.rs` -> baileyrs `14ba3b6` (0.13.0): `src/Compatibility/all-encryptions-failed.ts`, `src/Socket/messages.ts`, new test.
4. 2026-08-17, bridge `7df72f8` `feat(deps)!: a message's envelope type and mediatype cross typed, and a build retirement deadline becomes an event` (core `0d54574c`): `src/generated_types.rs` +66/-17, `src/wasm_client.rs`, `tests/event-union.test.ts` -> baileyrs `1d4ae34` (0.14.0): `Bridge/schema.ts`, `Bridge/types.ts`, `Socket/events.ts`, `__fuzz__/generators/bridge-event.ts`, `bridge-events.fuzz.test.ts`, `regressions.test.ts`.
5. 2026-08-20/21, core `08a281ac` -> bridge `9bfd7ee` `feat(client)!: hold a call through a reconnect instead of refusing it` touching `result_types.rs`, `wasm_client.rs` and all eleven `wasm_client/*.rs` submodules -> baileyrs `fafc395` (0.17.0): README + `closed-domain-arguments.test.ts` only (the bridge absorbed this one).
6. 2026-06-11, bridge `3b4eb13` `chore(deps): bump whatsapp-rust (#19)` absorbing 135 core commits: `errors.rs`, `generated_types.rs`, `js_backend.rs`, `js_http.rs`, `result_types.rs`, `wasm_client.rs`, `ts/generated/whatsapp.ts` +8,150, `ts/proto-types.d.ts` +1,897, four test files.
7. 2026-09-01, bridge `dfb4fff` `feat: carry the core's username surface across the boundary` (core `9be10573`, 29 core commits): `codegen/src/main.rs`, `errors.rs`, `generated_types.rs`, `js_backend.rs`, `result_types.rs`, `wasm_client.rs`, `wasm_client/contacts.rs`, `tests/event-union.test.ts`. Not yet adopted by baileyrs at HEAD.

## 4. Fix-after-fix churn

Commit type mix: core 350 fix, 263 perf, 261 feat, 139 chore, 66 refactor, 9 revert (fix+revert = 28.9% of 1,270). Bridge 49 fix + 2 revert of 259 (19.7%). baileyrs 43 fix, 0 revert of 150 (28.7%).

Core reverts: 8 of the 9 are one episode (2025-09-23, the storage-decoupling series reverted and re-reverted); the ninth is `#484` "revert DM send to bare recipient JID" (2026-04-02). Bridge reverts: BoltFFI artifact (#46), OIDC diagnostic workflow. baileyrs: none.

### Core areas (fix commits / commits touching the area, 12 mo)

| Area (paths) | Fix | Total | Fix % |
| --- | --- | --- | --- |
| VoIP (src/voip, wacore/src/voip, handlers/call.rs, stanza/call.rs, client/voip.rs) | 37 | 80 | 46% |
| lifecycle / reconnect (src/client/lifecycle.rs) | 29 | 80 | 36% |
| message receive (src/message*, handlers/message*) | 72 | 208 | 35% |
| client hub (client.rs + lifecycle + accessors + node_io) | 105 | 334 | 31% |
| lid_pn (src/client/lid_pn.rs) | 12 | 40 | 30% |
| send (src/send, wacore/src/send*) | 54 | 188 | 29% |
| retry (src/retry.rs) | 32 | 117 | 27% |
| signal store + sessions (client/sessions.rs, prekeys.rs, wacore/src/store, libsignal store) | 57 | 212 | 27% |
| device_registry | 13 | 51 | 25% |
| signal cache (wacore/src/store/signal_cache.rs) | 10 | 40 | 25% (plus 22 perf) |
| app_state (client/app_state.rs, appstate_sync.rs, wacore/appstate) | 34 | 141 | 24% |
| receipt.rs | 14 | 61 | 23% |
| groups (features/groups.rs, iq/groups.rs) | 15 | 79 | 19% |
| sqlite storage | 20 | 107 | 19% |
| binary / jid (wacore/binary) | 26 | 166 | 16% |
| history sync (wacore/src/history_sync.rs, src/history_sync.rs) | 8 | 55 | 15% |
| noise (wacore/noise, client/noise*) | 2 | 42 | 5% |

### Core: files with most fix commits, and how often a fix is followed by another fix on the same file within 7 days

| File | Fixes | Commits | fix->fix <= 7d | Re-fix rate | Longest run of consecutive fix-only commits |
| --- | --- | --- | --- | --- | --- |
| src/client.rs | 94 | 293 | 84 | 0.90 | 8 |
| src/message.rs | 47 | 130 | 38 | 0.83 | 6 |
| src/send.rs (pre-rename) | 34 | 115 | 30 | 0.91 | 5 |
| src/retry.rs | 32 | 117 | 21 | 0.68 | 4 |
| src/client/lifecycle.rs | 29 | 80 | 25 | 0.89 | 5 |
| wacore/src/send.rs | 28 | 99 | 22 | 0.81 | 5 |
| wacore/src/types/events.rs | 23 | 88 | 15 | 0.68 | 4 |
| sqlite_store.rs | 19 | 82 | 11 | 0.61 | 2 |
| src/client/tests.rs | 18 | 51 | 12 | 0.71 | 5 |
| src/message/tests.rs | 17 | 45 | 13 | 0.81 | 4 |
| src/send/mod.rs | 17 | 60 | 12 | 0.75 | 3 |
| src/handlers/notification.rs | 16 | 64 | 9 | 0.60 | 3 |
| src/appstate_sync.rs | 15 | 55 | 6 | 0.43 | 5 |
| src/client/node_io.rs | 14 | 53 | 10 | 0.77 | 4 |
| src/voip/facade.rs | 14 | 35 | 9 | 0.69 | 3 |
| src/receipt.rs | 14 | 61 | 8 | 0.62 | 9 |
| src/bot.rs | 14 | 80 | 6 | 0.46 | 6 |
| src/client/sessions.rs | 14 | 54 | 5 | 0.38 | 3 |
| wacore/src/store/traits.rs | 14 | 51 | 5 | 0.38 | 4 |
| src/client/app_state.rs | 13 | 36 | 9 | 0.75 | 4 |
| src/client/device_registry.rs | 13 | 51 | 8 | 0.67 | 4 |
| src/message/receive.rs | 13 | 38 | 7 | 0.58 | 3 |

The lifecycle fix sequence in its last five weeks reads as one unresolved design question (reconnect vs terminal shutdown ordering): `#1179` race the reconnect backoff against the terminal shutdown (07-29), `#1221` release transport and feed guards before awaiting (08-07), `#1264` stop the run loop announcing a reconnect that disconnect() forbids (08-10), `#1291` announce a connection the critical sync left degraded (08-11), `#1379` measure the dead-socket deadline with a clock that cannot jump (09-01), `#1380` end an interrupted resume with a signal instead of silence (09-01), `#1387` close the ordering gaps the terminal lock still left open (09-02). Eleven fix commits on `lifecycle.rs` in Aug-Sep alone. Five of these were re-exported to the bridge as `deps!` releases.

`signal_cache.rs`: 10 fixes, 6 of them in a 23-day window (07-15 `#1027` gate the sender-key advance before the wire, `#1042` retain durability gates through deletes, `#1043` serialize sender-key mutations, 07-16 `#1044` recover cancelled checkouts, 07-27 `#1149` DH ratchet stranding the counter lease, 08-07 `#1229` cold loads spanning a flush and eviction). None since. The file's 22 perf commits are the reason it is 6,226 lines.

`app_state`: 34 fixes with two visible bursts: 2026-06-08 had five fix commits in one day (`#748` require snapshot MAC, `#751` "repair main build broken by #748/#750 merge", `#766`, `#769`, `#773`) and 2026-08-04 three (`#1205`, `#1207`, `#1208` "unbreak the bootstrap gate and read the right lifecycle signals"). The August ones are lifecycle-coupled (`#1207`/`#1208` also touch `lifecycle.rs`).

`history sync`: 8 fixes in 55 commits and none of them are re-fixes of each other (pushname match, tc-token, jid spelling). Stable.

### Bridge

| Area | Fix | Total | Notes |
| --- | --- | --- | --- |
| src/wasm_client.rs | 12 | 65 | 4 of them on one day (08-08, #25/#28/#29/#30: argument validation and error kinds); re-fix rate 0.73 |
| src/wasm_client/ submodules | 4 | 11 | |
| proto codec (ts/proto*.ts, src/proto.rs, scripts/gen-ts-proto.ts, codegen/) | 11 | 35 | 31% |
| ts/ (all hand-written TS) | 12 | 45 | |
| src/errors.rs | 4 | 15 | re-fix rate 1.0 |
| scripts/gen-ts-proto.ts | 4 | 6 | re-fix rate 1.0 |
| AGENTS.md | 6 | 17 | re-fix rate 1.0, 6 consecutive fix commits |
| src/result_types.rs | 2 | 17 | |
| src/js_backend.rs | 0 | 19 | |
| runtime / transport | 1 | 17 | |

Proto codec fix run, 2026-08-10 to 08-21: `#38` numeric input contract, `#39` invalid UTF-8 policy, `#42!` 64-bit fields outside the safe range, `#44` framed payloads, `#49!` where a field ends and how deep a message goes, `#52` generated declarations point at the right types, `#57` packed string region unit, `#75` receipt id count outgrew its byte. Eight fixes, three breaking, in twelve days, each regenerating `ts/generated/whatsapp.ts` (1,260 to 2,212 line diffs).

### baileyrs

| Area | Fix | Total |
| --- | --- | --- |
| src/Socket/ (all) | 14 | 46 |
| src/Socket/index.ts | 7 | 22 |
| src/Socket/messages.ts | 7 | 16 (re-fix rate 0.83) |
| src/Utils/messages.ts | 7 | 19 |
| src/Socket/events.ts | 5 | 23 |
| legacy-store (Compatibility/legacy-store, Utils/wrap-legacy-store.ts) | 5 | 15 |
| src/Utils/event-buffer.ts | 4 | 8 (re-fix 1.0) |
| src/Bridge/{schema,types,adapt}.ts | 4 | 13 |
| src/Socket/terminal-close*.ts | 1 | 1 (created 08-07, #20) |
| src/Socket/bridge-client-owner.ts | 1 | 1 (created Aug) |
| proto-runtime (Compatibility/proto-runtime.ts, src/WAProto) | 1 | 7 |

Lifecycle-related fixes in baileyrs: `#6` audit-driven socket lifecycle (05-06), `#11` zombie connection after stream-error 500 (05-17), `#20!` make close mean the socket is finished, as upstream does (08-07), `#26` bridge 0.7.0 plus the auto-reconnect-terminal-close test (08-08), `#77` a 429 stream error is a rejected session (08-19). Four substantive fixes in four months; `terminal-close.ts` and `bridge-client-owner.ts` have one commit each. The reconnect fragility lives in core `lifecycle.rs`, and baileyrs sees it as `deps` bumps rather than as its own fixes.

## 5. Author concentration (names only)

| Repo | Distinct authors 12 mo | Of which bots | Bot commits | Top author | Top author share | Other humans |
| --- | --- | --- | --- | --- | --- | --- |
| core | 28 | 5 (dependabot, Copilot, copilot-swe-agent, codspeed, jules) | 77 | João Lucas (two name spellings + `jlucaso1`) | 1,160 / 1,277 = 90.8% (96.7% of human commits) | 22 people, 29 commits (2.3%); largest is Salientekill with 11 |
| bridge | 8 | 3 (release bot, dependabot, Claude) | 25 | João Lucas | 239 / 267 = 89.5% | devlikepro, Zaidan Yusuf Akbar, Salientekill: 1 each (1.1%) |
| baileyrs | 5 | 2 (release bot, Claude) | 17 | João Lucas | 127 / 150 = 84.7% | Salientekill 4, Zaidan Yusuf Akbar 2 (4%) |

External human contributions in core land in `events.rs`, `wacore/src/send.rs`, `pdo.rs`, `handlers/notification.rs`, `features/app_state_resync.rs`, `history_sync.rs`, `call.rs` (Salientekill, Nizar Izzuddin Yatim Fadlan, Maximilian Winter, Sumit Kumar, Zaidan Yusuf Akbar, oon arfiandwi, ekrem7, arsa0x, Alessandro Ricottone). In the bridge: `wire_batch.rs`, `ts/wire-info.ts`, `errors.rs`, `js_backend.rs`, `js_http.rs`. In baileyrs: `Socket/events.ts`, `Socket/index.ts`, `Socket/messages.ts`, `unsupported-config.ts` and their tests.

Bus factor is 1 across all three repos. This cuts both ways for a roadmap: coordination cost of a large restructure is near zero (the same person lands core, bridge and baileyrs the same day, which the lag numbers prove), but nothing is protected by a second reviewer's mental model, so the safety net has to be tests and generated contracts rather than review.

## 6. Generated-file churn

### Core

| File / set | Lines now | Commits 12 mo | Largest diffs |
| --- | --- | --- | --- |
| plugins/wam-catalog/src/{generated,call_sites,lib,tests}.rs | 133,142 | 1 | `c1ffe8da` (08-21) +133,142 in one commit |
| wacore/src/iq/abprops.rs | 18,718 | 4 | `3f6ffe44` +14,794 (initial, 06-05); `ba1ac3bf` +1,903/-2,268; `874328f3` +3,687/-196; `c1ffe8da` +826/-28 |
| wacore/src/iq/mex_operations.rs | 11,032 | 5 | `a0c7bc23` +9,003 (initial); `3a98a876` +1,339/-165 (09-01); `ba1ac3bf` +476/-185; `64adf5bd` +381/-64 |
| waproto/src/whatsapp.proto | 7,739 | 7 | `ea8237b3` +4,868/-4,801; `c6131f3c` +1,339/-493; `8d35b042` +732; `c1ffe8da` +764 |
| waproto/src/whatsapp.rs (prost output, tracked until 06-10) | 0 | 10 | `4a1d2ec2` +4,398/-3,193; `d3645a8f` -20,008 (untracked) |
| wacore/appstate/src/schemas.rs | 1,472 | 5 | `6c7220c0` +1,439 |
| wire_enums.rs / targets.rs / wire_tags.rs / version/generated.rs | 206 / 54 / 107 / 16 | 3 / 2 / 2 / 2 | all under 150 lines |
| Total generated | 172,309 | 25 commits (2.0% of 1,270) | +188,820 / -33,133 |

Every one of the 14 whatspec regeneration commits changes only generated files plus manifests: no hand-written code rides along. Generated lines are 24.7% of all lines added in the year (188,820 of 765,906) but 2% of commits. Hand-written non-test Rust in `src/` + `wacore/` is 344,162 lines, so generated code is one third of the compiled surface.

### Bridge

| File | Lines now | Commits 12 mo | Largest diffs |
| --- | --- | --- | --- |
| ts/generated/whatsapp.ts (ts-proto) | 81,016 | 8 | `707508c` +61,276 (initial); `ddf084b` +24,344/-15,866 (08-21 spec bundle); `0f755bf` +9,238/-8,071; `3b4eb13` +8,150/-299; `470d554` +1,954/-2,212; `0bddd55` +1,260/-1,260 |
| ts/proto-types.d.ts | 19,596 | 6 | `e3fb0ad` +14,584; `ddf084b` +2,133; `3b4eb13` +1,897; `5b2a9d0` +616; `f12e8c9` +428/-425 |
| src/generated_types.rs (bridge codegen mirror of core types) | 1,693 | 26 | small: `0f755bf` +388/-56; `0f3eb10` +199/-5; `2a27217` +103/-28; `7df72f8` +66/-17 |
| ts/generated/whatsapp-surface.txt (schema guard) | 3,705 | 4 | `b449c4c` +3,064 |
| src/generated_proto_types.rs (deleted 07-24) | 0 | 2 | +13,887 / -13,887 |
| Total generated | 106,023 | 36 commits (13.9% of 259) | +149,065 / -43,055 |

Generated lines are 65.8% of all lines added to the bridge in the year (149,065 of 226,475). Unlike core, five bridge regenerations coincide with codec fixes in the same commit (`#42`, `#44`, `#49`, `#52`, `#51`), so a reviewer sees a 2,000-line generated diff on top of a behavioural change. Hand-written bridge code is 23,618 lines of Rust plus 2,264 of TS.

### baileyrs

`src/WAProto/{index.d.ts,runtime.ts,compatibility-schema.ts}`: 14,052 lines, 4 commits, +14,056/-4. It was copied in on 2026-07-23 (`fe9a641`, bump to bridge alpha.37, 202 files) and has been touched three times since for one-line edits. It is a frozen snapshot of the bridge's dts, not a regenerated artifact, and it has not tracked the two bridge schema regenerations of August (`5b2a9d0`, `ddf084b`).

## 7. Release cadence and breaking changes

### Core (no CHANGELOG; cargo-release + GitHub Actions tagging)

| Tag | Date | Commits since previous | Commits marked `!` |
| --- | --- | --- | --- |
| v0.1.0-alpha | 2025-10-07 | 409 | 0 |
| v0.2.0 | 2025-12-26 | 98 | 0 |
| v0.3.0 | 2026-03-07 | 122 | 1 |
| v0.4.0 | 2026-03-20 | 91 | 0 |
| v0.4.1 / v0.4.2 / v0.4.3 / v0.5.0 | 2026-03-21/22 | 5 / 12 / 3 / 4 | 0 |
| v0.6.0 | 2026-05-11 | 182 | 29 |
| v0.7.0 | 2026-08-06 | 536 | 14 |
| unreleased at HEAD | | 158 | 0 by marker; 8 of the bridge's pins into this range are labelled `deps!` |

44 `!`-marked commits in 12 months (3.5%), 15 of them in the five days 04-07 to 04-12 (the `perf!` series #503 to #527: CompactString, yoke, Arc<Event>, ChatLane, NodeStr). Ten releases, two of which carried 90% of the declared breakage. The current unreleased range has zero declared breaking commits while downstream declared eight; the `!` marker is being applied at the bridge, not at the source.

### Bridge (CHANGELOG.md, release-please)

21 tags, all between 2026-08-06 and 2026-08-26 (v0.6.1 to v0.19.0): one release per day, ten minor bumps in 20 days. Before tags, `package.json` went through 98 distinct versions since 2025-08 (`0.6.0-alpha.1` to `alpha.43`, etc.): 15 in Apr 2026, 12 in May, 23 in Aug. 11 of the 21 tagged releases have a `BREAKING CHANGES` section (12 breaking commits). Eight of the twelve are `deps!` pass-throughs of a core change (#51, #55, #64, #67, #73, #84, #86 and the 0.10.0 events one); four are native to the bridge (`proto` #42 and #49, `wire` #58, `client` #76). Sections used: Bug Fixes in 15 releases, Features in 6, Performance in 4.

### baileyrs (CHANGELOG.md, release-please)

16 tags between 2026-08-07 and 2026-08-26 (v0.0.35 to v0.2.10); 43 distinct `package.json` versions since 2026-04-16 (`0.0.7` to `0.2.10`). 2 releases with `BREAKING CHANGES`: 0.1.0 (`close` means the socket is finished, #20) and 0.2.0 (bridge 0.10.0 events, #55). 8 of the last 10 releases contain a `Dependencies: move to bridge x` entry, and 5 of those 10 contain nothing else. baileyrs' release train is the bridge's release train, one day later at most.

## Conclusions for a refactoring roadmap

1. **The core `Client` hub is the single most expensive thing in the three repos and the June split did not fix it.** `src/client.rs` + `lifecycle.rs` + `accessors.rs` + `node_io.rs`: 334 commits, 105 fixes (31%), re-fix rate 0.89 to 0.90, and `client.rs` still co-changes with six other files at ratio 0.67 to 0.82. The root file re-grew from 914 to 2,185 lines in 90 days; `lifecycle.rs` took 80 commits and 29 fixes in its first 90 days. The last seven lifecycle fixes (07-29 to 09-02) are all reconnect-vs-terminal-shutdown ordering, and five of them were re-shipped downstream as `deps!` releases. This is hot, fragile, and cascading: invest here first, and invest in the state machine (who may announce a reconnect, who owns the terminal lock), not in file layout.

2. **`retry.rs` (5,357 lines, 117 commits, 32 fixes, grew 16x in a year) and the send path (`src/send.rs` -> `src/send/mod.rs`, 222 commits with follow, 8,158 lines, 51 fixes, re-fix rate 0.91) are the second tier.** `retry.rs` shares 21 commits with `sender_keys.rs` (0.70): retry owns sender-key state that arguably belongs to the signal layer. These are "huge and hot", the priority class; each is also over 5,000 lines of hand-written code, which is where the per-commit churn of 88 to 179 lines comes from.

3. **Message receive (72 fixes / 208 commits, 35%) and `receipt.rs` (9 consecutive fix-only commits, the longest run in the repo) deserve a design pass before more features land on them**, but their surface is spread across `message.rs`, `message/receive.rs`, `message/dispatch.rs`, `handlers/*` and `pdo.rs`, so the fix is a boundary, not a split.

4. **`signal_cache.rs` is dense (6,226 lines, 22 perf commits) but currently quiet**: 10 fixes, 6 of them in a 23-day durability-gate series that ended 08-07, none since. Do not open it for a structural refactor now; it is the file where perf work is concentrated and the durability invariants were just re-proven. Leave it behind its tests until the lifecycle work needs it.

5. **App state is medium priority and its cost is fragmentation, not size.** 141 commits, 34 fixes across three homes (`src/client/app_state.rs` 6,850 lines, `src/appstate_sync.rs`, `wacore/appstate/`), fixes arriving in bursts (5 on 06-08, 3 on 08-04), and `hash.rs`/`processor.rs` co-change at 0.81. The August fixes were lifecycle-coupled (`#1207`/`#1208`), so it will move again when item 1 moves; sequence it after.

6. **Leave the cold giants alone.** `src/plugins/mod.rs` (6,558 lines, 8 commits, 0 fixes), `wacore/src/voip/driver.rs` (4,311 / 11 / 3), `wacore/src/iq/groups.rs` (5,758 / 43 / 6: hot but not fragile, it grows), `wacore/src/history_sync.rs` (4,846 / 33 / 8, no re-fixes). VoIP overall (33,600 lines, 80 commits, 46% fix) is three months old and on a stabilization curve; refactoring it now would be refactoring code whose requirements are still being discovered. The 16k- and 7.8k-line test files cost compile time, not design attention.

7. **Storage trait and its implementation are one unit.** `wacore/src/store/traits.rs` and `sqlite_store.rs` share 40 of the trait's 51 commits (0.78); `schema.rs` never changes without `sqlite_store.rs` (23/23). The "platform-agnostic trait" boundary is not delivering independent evolution; either the trait is too wide (every new persisted field is a trait change) or the split is in the wrong place. `sqlite_store.rs` at 7,235 lines with 82 commits is huge-and-hot on its own.

8. **The cross-repo cascade costs edits, not days.** Lag is 0 days at every hop (median 0, max 2 core->bridge, max 1 bridge->baileyrs). What it costs is 44 hand-edited bridge commits per 62 core pins (71%) and 21 hand-edited baileyrs commits per 46 bridge bumps (46%), landing in the same files every time: bridge `wasm_client.rs` (35 of 62 pins), `js_backend.rs`, `result_types.rs`, `errors.rs`, `ts/index.ts`; baileyrs `Socket/events.ts` (15 of 46), `Socket/index.ts`, `Bridge/types.ts`, `Bridge/schema.ts`, `Types/*.ts`. That is the contract-drift tax, and it is concentrated enough to be automated: `generated_types.rs` already exists on the bridge side, but `Bridge/schema.ts`/`Bridge/types.ts` and `src/WAProto/index.d.ts` (frozen since 07-23, has missed two bridge regenerations) are hand-mirrored and should be emitted from the bridge's generated surface instead.

9. **The bridge is one module with nine file names, and `wasm_client.rs` is its hub.** `result_types.rs` changed in 17 of 17 commits together with `wasm_client.rs`; `js_transport.rs`, `runtime.rs`, `camel_serializer.rs`, `proto.rs`, `js_backend.rs` all at 0.84 to 0.92. The 08-08 split into `wasm_client/*.rs` moved 4,664 lines but the root has since seen 8 commits to the submodules' 5 and grew back to 5,208 lines. The next split should be along "what changes on a core pin" (event/result mapping, error kinds) versus "what changes on a JS-API decision", because that is the axis the history shows.

10. **The bridge proto codec is its fragile area, and it is also its diff-noise area.** 35 commits, 11 fixes, 3 breaking in 12 days (08-10 to 08-21), `gen-ts-proto.ts` re-fix rate 1.0, and each fix regenerates `ts/generated/whatsapp.ts` by 1,200 to 24,000 lines in the same commit. 66% of all lines added to the bridge in the year are generated. Separate codec-behaviour commits from regeneration commits (core already does this: all 14 of its regen commits are pure) so a reviewer can see the 40-line change under the 2,000-line diff. baileyrs' `proto-runtime.ts` is cold (7 commits, 1 fix): the fragility did not propagate, so the boundary there is working.

11. **baileyrs' own fragility is small and sits in messages and events, not lifecycle.** `Socket/messages.ts` (7 fixes / 16, re-fix 0.83), `Utils/messages.ts` (7/19), `event-buffer.ts` (4/8), `Socket/index.ts` (7/22). `terminal-close.ts` and `bridge-client-owner.ts` are one-commit-old files; the four substantive lifecycle fixes in four months were all "match upstream close semantics" and the underlying reconnect churn is in core (item 1). Tests outnumber source 1.8:1 and the hottest test asset (`__fuzz__/harness/divergence.ts`, 13 commits, 5 fixes) is doing real work catching bridge-event drift; keep funding it.

12. **Release semantics are inverted relative to where change originates.** Core releases rarely (v0.6.0 carried 182 commits and 29 breaking changes; v0.7.0 carried 536 and 14; 158 commits are unreleased now with zero `!` markers) while the bridge tags daily and marks 11 of 21 releases breaking, 8 of them `deps!` pass-throughs of unreleased core commits, and baileyrs re-releases each of those within a day. 13 of the last 15 bridge pins point at core commits that are not in any core release. Either the bridge pins core tags (which would introduce the lag that is currently zero, so probably not) or the `!` marker moves to the core commit that actually breaks the surface, so the changelog that users read (bridge, baileyrs) is derived rather than authored. Also worth noting: core's root `Cargo.toml` co-changes with eight sub-crate manifests at 0.73 to 0.92; workspace-inherited versions would remove a nine-file touch from every release.

13. **Bus factor 1 means the refactoring budget is bounded by one person's attention, and by tests rather than review.** 91% / 90% / 85% of commits are one maintainer; external humans contributed 2.3% / 1.1% / 4%. Large restructurings are cheap to coordinate (every cascade already lands same-day) but there is no second reader; sequencing should prefer changes that add a generated contract or a test harness as the safety net over changes that rely on careful review. The 6-commit fix run on the bridge's `AGENTS.md` (re-fix rate 1.0) is a small tell that the written contracts are being iterated as fast as the code.
