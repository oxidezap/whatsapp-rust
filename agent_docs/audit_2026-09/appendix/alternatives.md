# Architecture alternatives — whatsapp-rust / whatsapp-rust-bridge / baileyrs

Date: 2026-09-02. Trees inspected: `whatsapp-rust@b31ae59`, `whatsapp-rust-bridge@b14f62b`, `baileyrs@4aea0da`. Companion to the incremental audit in `../README.md`, which took the three-repository shape as fixed and recommended moving policy one layer down. This document questions the shape.

Every number below was measured on the trees or fetched from crates.io / the npm registry through the session proxy. Where a source was unreachable it says so (GitHub API and `gh` were blocked from this session; CI wall-clock times are therefore inferred from job counts and `timeout-minutes`, not read).

---

## 0. The framing the previous audit did not question

The audit's target architecture (its §3.4) keeps three repositories and three release trains and reduces duplication by adding to the core what "any other binding (Python, C, uniffi)" would need: serde on result types, `ErrorChainExt::classify()`, `reconnect_gate()`, `KvBackend<S>`, `ts-rs` under a feature. Three assumptions carry that plan:

1. **The repository boundary is a given.** It is what forces every mirror the audit counts: `result_types.rs` (80 `Tsify` structs, 2 `From` impls — the other 78 are filled by hand in converters such as `group_metadata_result` at `wasm_client.rs:2994`), `generated_types.rs` (a 1,693-line TypeScript string produced by a 2,355-line `syn` scraper reading `wacore/src/types/events.rs` and `src/send.rs` *of another repository*), `errors.rs` (23 `source()`/`downcast_ref` sites re-deriving what 15 core error enums already say). Rust's orphan rule is the direct cause: the bridge cannot `derive(Tsify)` on a type it does not own.
2. **Other bindings will exist.** Nothing in any of the three repos, their READMEs, issues or `plugin_architecture.md` names one; that doc says the opposite — "bridge, sidecar, and wire-protocol work starts only with a concrete consumer."
3. **The bridge has consumers other than baileyrs.** For the *client* API this is false today (§D); for the *utility* API it is true but on a frozen 0.5.x line the current package no longer even exports (§D).

The previous audit's line-count arithmetic is right. Its conclusion — pay the cost of generalising the core so that a boundary that exists only by accident of git hosting can stay — is what this document argues against.

### Evidence base (numbers reused throughout)

| Measure | Value | Source |
| --- | --- | --- |
| Core commits, last 8 weeks | 307 (~38/week) | `git log --since` |
| Core `!:` (breaking) commits / total | 44 / 1,620 | `git log --oneline` |
| Core published crates / workspace packages | 11 / 18 | `cargo metadata` |
| Core feature count (`whatsapp-rust` / `wacore`) | 25 / 13 | `cargo metadata` |
| Core CI runners per PR | ~25–30 (main.yml 9 jobs + feature-matrix ×11 crates; wasm 1 job/4 builds; miri 3; binary-size 1; codspeed 2; e2e 1; supply-chain 2) | `.github/workflows/*` |
| Core wasm32 builds already in CI | 4 (`whatsapp-rust` no-default, `sqlite-storage` + `wacore/js`, `wacore` `voip,js`, `whatsapp-rust` `voip-mlow`) | `wasm.yml` |
| `Client` fields (pub / pub(crate)) | 145 (4 / 120) | `src/client.rs` |
| `pub async fn` in `src/features` / `src/client` | 175 / 56 | grep |
| `impl IqSpec` | 68 | grep |
| `#[cfg_attr(wasm32, async_trait(?Send))]` pairs | 113 | grep |
| Crates in tree: wasm no-default / native default / +voip / all-features | 153 / 217 / 333 / 340 | `cargo tree` |
| VoIP lines (`wacore/src/voip` + `src/voip`) / MLow / testdata | 72,060 / 20,316 / 16 MB | wc, du |
| `wacore` crate size on crates.io, 0.6.0 → 0.7.0 | 289 KB → 7.29 MB | crates.io API |
| Bridge Rust lines / `#[wasm_bindgen(js_name)]` methods | 25,311 / 174 | wc, grep |
| Bridge methods of uniform shape (parse → `online().await` → one core call → serialize, ≤30 lines, no loop/stream) | **141 / 174 (81%)** | classifier over `src/wasm_client/*.rs` |
| Bridge `Tsify` derives | 146 | grep |
| Bridge pin of core | `git … branch = "main"`, `Cargo.lock` at `9be1057` (Sep 1), 6 commits behind HEAD after one day | `Cargo.lock`, `git log` |
| Bridge releases (Aug 6 → Aug 27) / `BREAKING` entries | 21 / 11 | `CHANGELOG.md` |
| baileyrs releases since Aug 7 / `BREAKING` | 16 / 2; pins bridge `0.19.0` exactly | `CHANGELOG.md`, `package.json` |
| baileyrs non-test, non-fuzz lines (excl. `WAProto/index.d.ts` 14,019) | ≈ 20,800 | wc per dir |
| baileyrs tests / fuzz / `scripts/compatibility` | 27,678 / 12,873 / 4,575 | wc |
| wasm artifact code-section ownership | waproto 17.5%, whatsapp-rust 17.1%, core 10.6%, wacore 10.6%, **js_sys 8.1% (`future_to_promise` monomorphisations)**, bridge 5.5% | `docs/wasm-artifact-private-memory.md` |
| npm dl/month: `@oxidezap/whatsapp-rust-bridge` / `@oxidezap/baileyrs` | 4,017 / 3,320 | npm API |
| npm dl/month: unscoped `whatsapp-rust-bridge` (0.5.5, old repo) | 4,320,459 — it is a runtime dependency of upstream `baileys@7.0.0-rc14` (`"whatsapp-rust-bridge": "0.5.4"`) for exactly two symbols, `LTHashAntiTampering` and `expandAppStateKeys` | npm API, `baileys` tarball |
| crates.io reverse dependencies of `whatsapp-rust` | `mendia` 1.16.0 (`^0.3`), `opencrabs` 0.3.83 (`^0.6`, optional) | crates.io API |
| Other Rust consumer found | `whatshell` 0.1.2 (npm-distributed Rust binary, pins `0.5.0`) | npm tarball `Cargo.toml` |

---

## A. Monorepo — `whatsapp-rust/bindings/wasm` inside the core workspace

Dependency direction unchanged (core never depends on the binding); what changes is that the binding crate and the core types live under one `Cargo.lock`, one toolchain pin and one PR.

### What it deletes (bridge files that exist only because of the repo boundary)

| File | Lines | Why it exists | Under A |
| --- | ---: | --- | --- |
| `codegen/src/main.rs` | 2,355 | `syn`-scrapes `wacore/src/types/events.rs` + `src/send.rs` from a sibling checkout to emit TS declarations; has already drifted (`TSIFY_STRUCTS` names five structs no longer in `result_types.rs`) | deleted — `#[cfg_attr(feature = "ts", derive(Tsify))]` on the core types under a `ts` feature, exported by wasm-bindgen's own `typescript_custom_section` |
| `codegen/src/proto_gen.rs` | 785 | dead second proto-type generator | deleted (dead today regardless) |
| `src/generated_types.rs` | 1,693 | output of the scraper | deleted; replaced by derive output at compile time |
| `src/result_types.rs` | 1,364 | 80 `Tsify` structs mirroring core results because core types can't derive `Tsify` from outside | ≈ 250 remain (types that genuinely differ on the JS side: `PriceResult` with `amount_1000` as string, `MediaType`, `Reachability`) |
| `src/signal_records.rs` | 522 | 14 `From`/`TryFrom` DTOs for `SessionRecordComponents` and friends | deleted; `derive(Serialize, Deserialize, Tsify)` on the `wacore-libsignal` component structs under the same `legacy-session-interop` feature |
| `src/legacy_session.rs` | 480 | 21 `From`/`TryFrom` DTOs for the v1 session model | deleted, same mechanism |
| `src/device_props.rs` | 217 | its own header says why: "prost types don't derive `Tsify`/`Deserialize`" — a 25-variant copy of `PlatformType` | ≈ 40 remain (the merge policy the audit already flags as core's) |
| `src/client_profile.rs` | 118 | same | deleted |
| `src/errors.rs` | 1,574 (822 production) | classifies core errors into 11 kinds by walking `source()` and downcasting 8 types, tested by 752 lines proving the re-derivation | ≈ 300 remain: the `BridgeError` wire shape and `From<ErrorClass>`; the classification moves next to the enums it classifies (the audit's `ErrorChainExt::classify()`, but now an internal function, not a public promise) |
| `codegen/` CI job, `gen:bridge-types` script, "fail if generated types drifted" step | — | boundary hygiene | deleted |

Total: **≈ 6,500–7,000 lines of the bridge's 25,311**, the same 26% the previous audit reaches — but reached by deleting the reason for the mirrors rather than by adding public API to the core for hypothetical bindings.

Two things the boundary is *not* responsible for and that A does not touch: `wire_batch.rs` (2,211, event framing — legitimately the JS boundary's own), `js_backend.rs` (1,824 — the audit's `KvBackend<S>` finding stands on its own merits, see §Recommended).

### What it costs the core

- **CI.** GitHub's API was unreachable from this session, so I cannot quote minutes. What can be said: the core already runs four wasm32 builds per PR in `wasm.yml`, and `feature-matrix` runs one runner per published crate (11). A bindings crate adds one `cargo build --target wasm32-unknown-unknown -p whatsapp-rust-wasm` (superset of the existing `whatsapp-rust` no-default wasm build, so the dependency compile is shared through sccache) plus the bridge's current Bun job (`wasm-pack` release + `bun test`, the step the bridge's own AGENTS.md calls "memory-hungry"). Net: +1–2 runners on a matrix of 25–30, and the `feature-matrix` leg must **exclude** the bindings crate (it must not be in `default-members`, and `feature_matrix_crates.sh` selects by `publish != []`, so mark it `publish = false` — the npm package is the artifact, not a crate).
- **A core PR must keep the wasm build green.** This is a cost only if core changes routinely break the bridge; the evidence is that they do: 11 of 21 bridge releases in three weeks carry `BREAKING`, and the bridge tracks `branch = "main"` and re-locks with `bump:wacore` — i.e. today the break is discovered one repo later and one release later. Moving it into the PR is the same work earlier, with the person who made the change doing it.
- **Release coupling.** The core releases by hand-bumping `Cargo.toml` and running `cargo-release`; the bridge uses release-please with a version cadence of days. Under A the npm package can still release independently (its version is `package.json`, not `Cargo.toml`), but it is built from a core commit and inherits the core's `0.x` cadence for *breaking* changes. That is already the truth (`branch = "main"`); A just stops hiding it.
- **The `ts` feature adds `tsify`/`wasm-bindgen` as optional deps of `wacore`, `wacore-libsignal`, `whatsapp-rust`.** They are proc-macro-only and gated; `cargo hack --each-feature` will compile them once per crate. `ts-rs` avoids the wasm-bindgen dep but cannot produce the `into_wasm_abi` glue, so tsify is the right derive here.

### Does the core already half-own this concern?

Yes, and more than half:

- `wasm.yml` builds `whatsapp-rust`, `sqlite-storage`, `wacore` and the VoIP runtime for `wasm32-unknown-unknown` on every PR.
- `Cargo.toml` carries a `[target.'cfg(all(target_arch = "wasm32", target_os = "unknown"))']` getrandom `wasm_js` dependency with a comment naming "downstream consumers like whatsapp-rust-bridge".
- 305 `target_arch = "wasm32"` cfgs and 113 `async_trait(?Send)` pairs exist for exactly one consumer.
- `wacore`'s `js` feature is `["getrandom/wasm_js"]` and nothing else — the bridge sets that directly, so the feature is already vestigial (the audit says remove; agreed).
- `legacy-session-interop` (2,194 lines in `wacore-libsignal/src/protocol/legacy_session.rs`) exists for baileyrs's `useLegacyMultiFileAuthState`, two layers up.

The core has been paying the wasm tax without owning the wasm artifact. That is the worst of both: it cannot see what breaks, and it cannot use the feature it pays for (`derive(Tsify)`).

### What it breaks

- Nothing for npm consumers: the package name, `exports` and `.d.ts` are unchanged; only the build's source tree moves.
- Nothing for crates.io consumers: no published crate changes.
- The bridge's forks (`@devlikeapro/…`, `@kezaa/…`, `-baron`) all fork the 0.5.x utility line, which the current repo no longer exports (`LTHashAntiTampering`/`expandAppStateKeys` are absent from `src/` and `ts/`), so they are unaffected.
- The bridge's git history: keep the old repo archived; `git subtree add` preserves history if wanted.

### Verdict: **recommend.**

The scraper, the DTO layers and the drift CI are the boundary's cost made visible. The alternative the previous audit proposes — teach the core to serialise camelCase, classify its own errors and expose `ts-rs` output "for any binding" — is a larger public-API commitment made on behalf of consumers that do not exist, to preserve a boundary that only a `git remote` defends.

---

## B. Bridge as a generated artefact

### Evidence

Classifier over the 174 exported methods (`src/wasm_client/*.rs`, whitespace-tolerant match on `.online().await` / `.unwaited(`):

| shape | count | examples |
| --- | ---: | --- |
| uniform: `parse_jid` → `online().await?` → one `client.<feature>().<method>(..)` → `map_err`/`to_js` | **141** | `group_leave`, `mark_chat_as_read`, 29 of 33 in `signal.rs`, 20 of 22 in `newsletter.rs` |
| result-shaping (>30 lines converting a core result into a mirror struct) | 10 | `get_catalog`, `get_collections`, `get_order`, `update_business_profile`, `is_on_whatsapp`, `find_by_username`, `get_memory_diagnostics` (79 lines), `newsletter_messages`, `signal_install_prekey_bundle`, `request_media_reupload` |
| batch/loop over a JS array | 8 | `read_messages`, `mark_played`, `fetch_user_info`, `group_fetch_all_participating`, `community_fetch_all_participating`, `get_usync_devices`, `add_lid_pn_mappings`, `group_setting_update` |
| two-arm `if` duplicating the chain | 5 | `pin_chat`, `mute_chat`, `archive_chat`, `star_message`, `update_block_status` |
| streams | 3 | `download_media_stream` (46 lines), `encrypt_media_stream` (77), `upload_encrypted_media_stream` (117) |
| local, no core call | 7 | `reachability`, `wait_until_reachable`, `withdraw_parked_calls`, `jid_to_signal_protocol_address`, `newsletter_mute`, `get_core_allocation_snapshot` |

So 81% is table-shaped. Of the 19% that is not, the 10 result-shapers disappear under A (they exist to fill mirror structs), leaving ~23 hand-written methods — streams, batches and the reconnect-gate helpers.

### Three ways to generate, and what each is worth

1. **A local macro in the bindings crate** (`export_online! { groupLeave => groups().leave(jid: Jid) -> () }`). Each uniform method is 6–10 lines; 141 × ~7 = ~1,000 lines saved (4% of the bridge). It keeps wasm-bindgen as the actual codegen (`.d.ts` per method, typed args), so nothing is lost. Cheap and safe. The previous audit's finding 2.4#6 (58 redundant `map_err`, duplicated `if` arms) is the same observation at smaller grain.
2. **An `#[export_binding]` attribute macro in the core** that emits wasm-bindgen glue from `impl Client` methods. This puts wasm-bindgen's model (JsValue, `?Send` futures, `js_name` casing, the `online()` reconnect gate that is bridge policy) into a proc-macro the core owns, for one consumer. It also cannot express the gate choice (`online` vs `unwaited(Unwaited::Local)` etc.), which the bridge's AGENTS.md says must be written at the call site. **Reject.**
3. **One dispatch entry (`call(method: &str, args: JsValue)`) with a table** instead of 174 exports. This is the only variant that changes the artifact: `js_sys::future_to_promise` monomorphisations are 8.1% of the code section (one per async export) and `serde-wasm-bindgen` 2.5%; a single exported future would collapse most of that (a rough upper bound of −0.4 MiB of a 5.24 MiB artifact, −0.4 MiB `Private_Dirty` per process by the doc's 1.05×-bytes fit). Costs: the `.d.ts` must then be generated separately from the same table (feasible — the table is the source), argument errors become runtime string-dispatch errors, and `tests/exported-surface.test.ts` ("every exported method must settle") loses its subject. **Consider only if the artifact-size gate is what is being optimised**; the bridge has already chosen size-vs-shape trade-offs (`--one-caller-inline-max-function-size 2000`) with measurement, and this one should be measured the same way before deciding.

### Python / uniffi from the same source

uniffi requires `Send + Sync` object handles and its own async runtime binding; the core's wasm configuration is built on `?Send` async traits (113 pairs) and a `Runtime` trait with a non-`Send` wasm shape. A uniffi binding would be the *native* configuration, a different feature set, a different runtime and a different error-mapping table — nothing about it is "the same source" as the wasm glue except the method names. No consumer asks for it. `plugin_architecture.md` already records the policy: foreign-language work starts with a concrete consumer. **Reject.**

### Comparison with existing derive patterns

`IqSpec` (68 impls) and `ProtocolNode`/`WireEnum` derives are declarative because their input is a *wire shape* the codegen can read. A binding export's input is a *policy* (which gate, which error field, batch or not), which is why 33 methods do not fit and why the 141 that do are already only 7 lines each. There is little left to generate.

### Verdict: **consider** (variant 1 only, as part of the A cleanup); **reject** the core attribute macro and uniffi.

---

## C. baileyrs as a thin generated shim

### How baileyrs's ~20.8k non-test lines divide

| bucket | lines | what |
| --- | ---: | --- |
| (i) Baileys API-shape adaptation — hand-written by nature | ≈ 9,000 | `Utils/messages.ts` 1,283 (content → `proto.IMessage` generation), `Socket/events.ts` 1,255 + `Utils/event-buffer.ts` 567 (Baileys event buffering semantics), `Compatibility/legacy-store/*` ≈ 1,600 (upstream `{creds, keys}` auth-state projection), `Compatibility/proto-runtime.ts` 898 (protobufjs-style `fromObject/create/decode` facade), `Types/*` 1,869 (Baileys type surface), `Utils/link-preview.ts`, `messages-media.ts`, `generics.ts` ≈ 1,270 |
| (ii) mechanical forwarding a table could generate | ≈ 1,100 (upper bound), realistically −600 | 123 `getClient()` sites across `Socket/*.ts`; whole files that are forwarders: `privacy.ts` 98, `profile.ts` 71, `contacts.ts` 43, `blocking.ts` 18, `presence.ts` 19, `prekeys.ts` 26, plus most of `newsletter.ts` 173, `groups.ts` 211, `communities.ts` 133, `business.ts` 135, `chat-actions.ts` 192. Each still needs a per-entry argument map (Baileys arg order, JID normalisation, result reshaping to a Baileys type), so a table entry is 2–4 lines, not 0 |
| anti-corruption layer the previous audit already attributes to bridge shape inconsistency | ≈ 2,200 | `Bridge/schema.ts` 1,175, `types.ts` 769, `primitives.ts` 219 — goes away when the bridge emits one shape (audit §2.6#1); under A that is an internal change |
| (iii) compatibility tooling | 4,575 scripts + 12,873 fuzz + ~50 `*-compatibility.test.ts` suites | see below |
| `WAProto/index.d.ts` | 14,019 lines / 790 KB | verbatim copy of `node_modules/baileys/WAProto/index.d.ts` made by `scripts/compatibility/waproto-facade.ts` from the `baileys@7.0.0-rc13` devDependency |

Generating (ii) from the bridge `.d.ts` saves ~600 lines and adds a generator plus a table that has to know Baileys signatures — the table *is* the adaptation. **Not worth a generator; worth the `forward(ctx, method, {map, check})` helper the previous audit proposes** (its §2.6#6).

### Is the differential fuzz a permanent cost or a migration aid?

Permanent, as long as the product promise is "drop-in replacement for `@whiskeysockets/baileys`" — and that promise is the whole reason baileyrs exists (README: `npm install @whiskeysockets/baileys@npm:@oxidezap/baileyrs`). Upstream moves (baileyrs's devDep is `7.0.0-rc13`; npm's latest is `rc14`), so parity is a moving target and the fuzz is a regression gate against a moving reference, not a one-time check. Its own README says it found "21 differences: 18 open, 3 deliberate" and each carries a review date that fails the nightly when it lapses — that is a maintenance contract.

What *can* be cut is scope, not existence:

- `proto-codec.fuzz.test.ts` ("do the Rust/WASM codec and protobufjs agree across all 498 message types") tests the **bridge's** codec, not baileyrs. It belongs beside the codec (`tests/proto-*.test.ts` in the bridge already pin merge, packed-repeated, wire-type and framing behaviour by hand). Move it.
- `scripts/compatibility/check-layer-boundaries.ts` greps sibling checkouts (`../whatsapp-rust`) — cross-repo lint that under A is a workspace lint.
- `audit-core.ts` (1,043) and `type-contracts.ts` (301) answer the same question (audit §2.6#11).

### Consumer value of the verbatim 790 KB `.d.ts`

TypeScript is structural, so "works with the same code" needs only that every `proto.X` a consumer names exists with compatible members. Verbatim parity buys two things beyond that: (a) nominal identity for consumers who re-export upstream types (`import type { proto } from '@whiskeysockets/baileys'` in their own `.d.ts`) — real but rare; (b) zero risk of a missed name. It costs: 900 KB shipped, `protobufjs` as a *runtime* dependency with zero `src/` imports (it is there so the copied `.d.ts` resolves `Long`/`$protobuf` names), and a second proto type layer over the bridge's own `proto-types.d.ts` (19,596 lines) — three declarations of one schema between the two packages.

Generating the facade from the bridge's descriptor with protobufjs naming (`proto.IMessage`, `proto.Message.decode`, nested namespaces) would be structurally identical for every type the codec implements, and `compat:audit:proto` (which already exists) would prove it. Names upstream declares that the descriptor does not carry are the exact list that audit prints. **Do this; keep the audit.**

### Verdict: **consider** — no generator for (ii); one `forward()` helper; move codec fuzz to the bridge; replace the verbatim `.d.ts` with a generated facade and drop `protobufjs` from runtime deps; keep the differential fuzz and the `.d.ts` audit as the permanent cost of the drop-in promise.

---

## D. Collapse bridge + baileyrs into one npm package

### Who consumes `@oxidezap/whatsapp-rust-bridge` directly?

Measured, not assumed:

| package | dl/month | relationship |
| --- | ---: | --- |
| `@oxidezap/whatsapp-rust-bridge` 0.19.0 | 4,017 | the client-API package |
| `@oxidezap/baileyrs` 0.2.10 | 3,320 | pins it exactly; the only known consumer of the client API |
| `whatsapp-rust-bridge` 0.5.5 (unscoped, old repo) | 4,320,459 | **runtime dependency of upstream `baileys@7.0.0-rc14`** (`"whatsapp-rust-bridge": "0.5.4"`), imported in `lib/Utils/lt-hash.js`, `crypto.js`, `chat-utils.js` for `LTHashAntiTampering` and `expandAppStateKeys` only |
| `@devlikeapro/whatsapp-rust-bridge` 0.5.2 | 2,397 | fork of the 0.5 line (WAHA) |
| `@zeppeliorg/wbails` 1.1.9 | 4,953 | Baileys fork; `lib/Utils/native-bridge.js` `require('whatsapp-rust-bridge')` dynamically |
| `@kezaa/whatsapp-rust-bridge`, `whatsapp-rust-bridge-baron` | 50, 40 | forks of the 0.5 line |
| `whatshell` | 779,106 | not a bridge consumer — a Rust binary on npm depending on `whatsapp-rust 0.5.0` |

The bridge is two products under one name. From 2025-08-18 to 2026-03-19 it was a **utilities package** (`appstate`, `binary`, `crypto`, `curve`, `group_cipher`, `noise_session`, `session_*`, `storage_adapter`, `audio`, `image`, `sticker` — the modules `src/lib.rs` listed at `f7fa85a`) that upstream Baileys adopted for LT-hash and key expansion. On 2026-03-19 ("chore: phase 1") `wasm_client.rs` arrived and the package became a **client engine**; the scoped package no longer exports the two symbols upstream imports. The utility audience is pinned to a frozen line in a different repo and is not served by anything in the current tree.

So for the client API, D's premise holds: **one consumer.** The bridge's own README ("High-performance WhatsApp utilities … Binary Protocol / Libsignal / App State / Audio / Image / Sticker") still describes the first product; `examples/` has a single `connect.ts`.

### What the split costs today

- Two release trains that release for each other: 21 bridge releases and 16 baileyrs releases between Aug 6 and Aug 27; at least five baileyrs releases are pure `deps: bump @oxidezap/whatsapp-rust-bridge` entries, and baileyrs's release config had to be patched to "recognise deps commits so a dependency bump reaches the changelog".
- Exact pin (`0.19.0`) with no range, so every bridge fix needs a baileyrs release to reach a user.
- Two proto type layers (`ts/proto-types.d.ts` 19.6k in the bridge; `WAProto/index.d.ts` 14k + `proto-runtime.ts` in baileyrs) and the anti-corruption layer (`Bridge/schema.ts`, `types.ts`, `primitives.ts` — 2,163 lines) that exists because the two sides disagree about JID/timestamp/case and neither can change without a coordinated major.

### What the split buys

A JS consumer that wants the engine without the Baileys API shape. None exists today. The audience most likely to want one — upstream Baileys and its forks — has already shown what it wants: two pure functions, not a client.

### Verdict: **reject as stated, recommend the other collapse.**

Folding the bridge *into baileyrs* would make the engine's TS surface a private detail of a Baileys clone, which is the wrong owner: the engine's shape is determined by the core (every `BREAKING` in the bridge changelog is a core change surfacing). Folding the bridge *into the core* (§A) gives the engine package the owner that actually changes it, and leaves baileyrs as the one repository whose reference point is upstream Baileys rather than the core. Keep publishing `@oxidezap/whatsapp-rust-bridge` from the core monorepo; a utility-only entry point (`LTHashAntiTampering`, `expandAppStateKeys`, the signal codecs) costs nothing to keep and is the one thing upstream Baileys has demonstrated demand for.

---

## E. Feature-split the core into more crates

### What is already split and what `cargo tree` says

`wacore`, `wacore-binary`, `wacore-libsignal`, `wacore-appstate`, `wacore-noise`, `wacore-derive`, `waproto`, plus the three adapter crates — 11 published. The proposals are VoIP, plugins, app-state and history-sync.

| feature set | crates in tree | delta vs default |
| --- | ---: | ---: |
| wasm32, `--no-default-features` (what the bridge builds) | 153 | −64 |
| native default | 217 | — |
| native + `voip` | 333 | **+116** |
| native + `plugins` | 218 | +1 (`bon`) |
| `--all-features` | 340 | +123 |

- **VoIP**: 116 crates, 72,060 lines (of which MLow 20,316 with its own `build.rs` that runs on every `wacore` build, and 16 MB of test vectors that `wacore/Cargo.toml` does not `exclude`, so the published `wacore` 0.7.0 is 7.29 MB against 0.6.0's 289 KB — 25×). In `src/` it carries 284 of ~720 feature gates. `subsystem_boundary.md` classes `voip-runtime` as coupled-but-disciplined (three `pub(crate)` helpers with a single caller). This is the one split with a measurable payoff at every level: crate size, dependency count, gate count, and `cargo check` time for the 153-crate wasm build that never wanted it.
- **Plugins**: adds one dependency and 150 KiB of binary; the gate count (91 + 72 for `client-lifecycle`) is the price of the seam existing, as the boundary doc says. A crate split would need the host's hooks to be `pub`, widening the API to save nothing. **Reject.**
- **App-state**: already `wacore-appstate`; what remains in `src/client/app_state.rs` (3.7k) is the client's sync driver, reached from `connect()`. **Reject** (organisational split into modules is the audit's §2.1#6 and is fine).
- **History-sync**: reached from `connect()`; the bridge's memory doc is explicit that "history sync, app state and media download are reached from `connect()`, not from an exported method, so no amount of gating removes them". A crate boundary would not change the wasm artifact by a byte. **Reject.**

### What the wasm build actually pulls in

From `docs/wasm-artifact-private-memory.md`: the artifact is 5.24 MiB, costing 6.72 MiB `Private_Dirty` per process (fit: 1.052 × bytes + 78 B × function, R² 0.9993). Code section by crate: `waproto` 17.5%, `whatsapp-rust` 17.1%, `core` 10.6%, `wacore` 10.6%, `js_sys` 8.1%, `whatsapp-rust-bridge` 5.5%, `alloc` 5.2%, `wacore-libsignal` 4.1%, `hashbrown` 3.2%, `serde_json` 3.0%, `serde-wasm-bindgen` 2.5%, `wacore-binary` 1.9%. The largest single function is the core's generated `Message` decoder (177 KB before the inliner cap). No crate split changes any of this; the levers that do are the ones the previous audit found in the *generated* files — 79% of `mex_operations.rs` unreferenced, 2,648 of 2,664 abprops unread, `waproto`'s 752 messages of which ~140 are named — i.e. a `WANTED` filter, not a crate boundary.

### Verdict: **consider — VoIP only** (`wacore-mlow`, `wacore-voip`, `whatsapp-rust-voip`, signalling stays), exactly as the previous audit's §2.3#1; the other three splits **reject**.

---

## F. External consumer risk and semver

### Who depends on the published crates

crates.io reverse dependencies (queried with a User-Agent through the proxy; GitHub code search unavailable):

| consumer | version req | surface used |
| --- | --- | --- |
| `mendia` 1.16.0 (gitlab, Telegram/WhatsApp movie bot) | `whatsapp-rust ^0.3`, `-sqlite-storage`, `-tokio-transport`, `-ureq-http-client` | `Bot`, `Client`, `Event`, `Message`, `ImageMessage`, `UploadResponse`, `MediaType`, `Jid` — 8 symbols |
| `opencrabs` 0.3.83 (agent framework, `whatsapp` optional feature) | `^0.6`, `default-features = false` | `Client`, `Bot`, `SendOptions`, `UploadOptions`, `Event`, `MessageInfo`, `ReceiptType`, `MediaType`, **`wacore::store::traits::{Backend, SignalStore, DeviceStore}`**, `appstate::processor::AppStateMutationMAC`, `appstate::hash::HashState`, 39 `waproto` message types. Its source carries the comment "the old blanket `Backend for Arc<T>` impl is gone" — it has already been broken once by a store-trait change |
| `whatshell` 0.1.2 (npm-distributed Rust CLI) | `= 0.5.0` family | client + sqlite + tokio + ureq |
| `wacore`, `wacore-binary`, `waproto` | only the workspace siblings and `opencrabs` | — |

None of the three names a `features::*Error` enum, calls a `_for_device` method, or touches the wacore items the previous audit lists as dead (`messages.rs` ×7, `reporting_token`, `usync::parse_get_user_devices_response*`, `stanza/call::build_*`). All three pin or lag: none tracks 0.7.

### Semver impact of the previous audit's proposals

All published crates are `0.x`, where Cargo treats a minor bump as breaking. Ratings are "what the bump must be" and "who outside the workspace notices".

| proposal | whatsapp-rust | wacore / siblings | sqlite-storage | external blast |
| --- | --- | --- | --- | --- |
| `FeatureError` unification (18 enums in `src/features`) | **0.8 (breaking)** | — | — | none of the three consumers names one; the bridge names 15 — under A that is an in-tree change |
| `_for_device` removal (23 `pub async fn` in `sqlite_store.rs`) | — | — | **0.8 (breaking)** | 89 in-repo call sites; `mendia`/`whatshell` depend on the crate but use only construction — none |
| dead `pub fn` removal in wacore | — | **0.8 (breaking)**; `cargo-semver-checks` will flag it (informational, and it covers `wacore`, `wacore-binary`, `waproto` only — not `whatsapp-rust`) | — | none found |
| VoIP crate split | 0.8 if the `voip*` feature names are kept as forwarders it is API-compatible for everyone who never enabled them (all three) | wacore 0.8 | — | none |
| `Client` field regrouping | 4 fields are `pub`, 120 `pub(crate)`, the rest private; if the 4 `pub` ones stay, **patch-level**; if they move into a sub-struct, breaking | — | — | none of the three reads a `Client` field |
| delete the middle libsignal store-trait family (audit `wacore.md` #3) | — | **0.8, and the one with a known victim**: `opencrabs` implements `wacore::store::traits` and has been bitten by exactly this shape of change | — | **high** — announce, keep the old trait names as blanket impls for one release |
| serde/camelCase on result types, `ErrorChainExt::classify`, `reconnect_gate`, `run() -> TerminalReason` (audit Phase 1) | additive → 0.7.x minor in spirit, but `run()`'s return type change is breaking → 0.8 | — | — | `mendia`/`opencrabs` call `bot.run()`; check the signature they use before changing |

### Does the release tooling handle breaking changes routinely?

Yes, in every repository, in different ways:

- **Core**: 44 conventional-commit `!:` markers in 1,620 commits; 10 releases (0.1.0 → 0.7.0 in eleven months, every one a minor); no `CHANGELOG.md` (GitHub releases); `cargo-release` with `tag = false, push = false` and a preflight that validates the version string; `cargo-semver-checks` informational and partial. Breaking is the norm, and the tooling neither blocks it nor records it beyond the commit subject.
- **Bridge**: release-please, 21 releases in 3 weeks, 11 `BREAKING` sections.
- **baileyrs**: release-please, 16 releases in 3 weeks, 2 `BREAKING` sections — the only one of the three whose consumers are asked to expect stability, which is right for a drop-in replacement.

The practical reading: a core 0.8 that batches every breaking item above costs external consumers one migration they are already used to (they lag by one to four minors today), and costs the bridge nothing under A because the bridge moves in the same PR.

---

## G. WA Web as the architectural yardstick

`AGENTS.md`: ground truth is WA Web; whatspec IR first, captured bundle second, whatsmeow/Baileys as second opinions. The generated files regenerate wholesale from one pinned whatspec commit. The rule for *keeping* something WA Web no longer builds already exists and is well-followed: "an action or flag the protocol carries but the bundle no longer builds goes in a hand-written sibling", with evidence:

- `wacore/appstate/src/schemas_unlisted.rs` — `LABEL_MESSAGE`: WA Web's action table stops at `label_edit/label_jid/label_sublist`, but the protobuf registry declares it, the Business mobile clients emit it, and whatsmeow and Baileys both send it. Kept, with the evidence in the doc comment.
- `wacore/src/iq/props.rs::stale` — two ab-props (`privacy_token_only_check_lid`, `profile_pic_privacy_token`) the current bundle no longer ships; the comment says "flag them for removal if the gated feature is reworked". Kept, with an explicit removal trigger.
- `wire_tags.rs` deliberately *drops* `privacy` because it never arrives under that tag — a delete decided by the direction of the stanza.

What lacks a rule is the other direction — protocol surface *we* carry that WA Web never had or that exists for a consumer's migration:

| item | lines | reason it exists | owner / expiry today |
| --- | ---: | --- | --- |
| `wacore-libsignal/src/protocol/legacy_session.rs` (feature `legacy-session-interop`) | 2,194 | baileyrs's `useLegacyMultiFileAuthState` imports Baileys JSON auth state | none written; the bridge enables it by default (`legacy-session` in `default`, AGENTS.md: "never remove one from `default`") |
| noise "unregistered gate … for legacy databases written before the registration gate" | small | pre-0.x databases | none |
| `CallAction` deprecated item | 1 | `#[deprecated(since = "0.6.0")]` — the only `#[deprecated]` in the tree | implicit: next breaking release |
| generated-but-unused: `mex_operations.rs` 11,032 (79% unreferenced), `abprops.rs` 18,718 (16 of 2,664 read), `wam-catalog` 133,142 (9 events served), `waproto` 185k generated for ~140 named messages | ≈ 163k | WA Web *has* it; we mirror it wholesale | the `WANTED` pattern in `emit/enums.rs` and `emit/iq_targets.rs` is the precedent; `abprops` already has `props::WATCHED` as the read-side filter |

The yardstick therefore cuts both ways and the repo applies it only one way. A principled rule, stated once in `wa_web_reference.md`:

1. **What WA Web has and we do not use is a codegen filter, not source.** Emit through `WANTED` (already done for `wire_enums` and `targets`; extend to `mex`, `abprops`, `wam`, and `waproto` via `build.rs`'s descriptor rewrite). Nothing is lost — the IR is committed upstream and the lock names the commit.
2. **What we have and WA Web does not needs an owner and an expiry.** Every hand-written sibling (`stale`, `schemas_unlisted`, `legacy_*`, a compat gate) records (a) the second opinion or capture that still carries it and (b) the event that retires it: the whatspec bump that removes the last reader, or the consumer whose migration window closes (for `legacy-session-interop`: baileyrs's 0.1 → native-store cut-over, after which the feature leaves the bridge's `default` on a major and is deleted from the core one release later).
3. **Sequencing that WA Web does not do is a finding, not a feature.** The 10 comments in `src/` and `wacore/` of the form "WA Web does not / no longer" are the current record; each should either be a test that pins the divergence (`retry.rs:497` — WA Web doesn't dedupe receipts, we do) or a deletion.

### Verdict: **recommend** the rule; it costs no code and turns the 163k generated lines into the previous audit's Phase-0 item with a principle behind it.

---

## Recommended target architecture

Different from the previous audit's §3.4 in one structural respect: **two repositories, not three**, with the engine's JS package built where the engine lives.

```
oxidezap/whatsapp-rust  (one workspace, one lock, one CI)
├── wacore*, waproto, whatsapp-rust, adapters        published crates, as today
├── wacore-mlow, wacore-voip, whatsapp-rust-voip     VoIP out of the default path (E)
├── plugins/, tools/whatspec-codegen                 unpublished, as today
└── bindings/wasm/                                   publish = false crate; npm @oxidezap/whatsapp-rust-bridge
    ├── src/   ~15k lines: wasm_client/* (141 table-shaped + ~23 hand-written), js_* adapters,
    │          wire_batch, BridgeError wire shape, one event shape (camelCase, JID string, unix secs)
    ├── ts/    ts-proto codec, proto-namespace, wire-info
    └── no codegen/, no generated_types.rs, no result_types mirrors, no signal_records/legacy_session DTOs
        → the core's types derive Tsify/serde under a `ts` feature only this crate enables

oxidezap/baileyrs  (reference point: upstream Baileys, not the core)
├── Socket/* with forward(ctx, method, {map, check}); DISPATCHERS keyed on the bridge event type
├── Compatibility/proto-runtime.ts as the single proto facade; WAProto .d.ts generated from the
│   bridge descriptor with protobufjs naming; protobufjs off the runtime dependency list
├── legacy-store/* (the drop-in promise), event-buffer, messages.ts       hand-written, stays
└── differential fuzz + .d.ts audit                                      permanent; codec fuzz moves to bindings/wasm
```

What this keeps from the previous audit unchanged: Phase 0 in full (test-module moves, `WANTED` filters, fixture sharing, dead-code deletion), `KvBackend<S: KvStore>` (the `js_backend.rs` finding is about policy duplicated against `in_memory.rs`, not about the boundary), `InboundMessage.raw`, `download_stream`, one event shape, `run() -> TerminalReason`, VoIP crates, `Client` regrouping.

What it drops from the previous audit: `ts-rs` as a public core feature "for any binding" (replaced by a `ts` feature whose only consumer is in-tree), `ErrorChainExt::classify()` and camelCase serde as *public API promises* (they become in-tree conveniences the bindings crate uses, free to change), and the three-repo release choreography.

### Migration order

Each step is shippable alone; order is by risk-per-line-deleted.

1. **Phase 0 (all repos, no API change).** Everything the previous audit's Phase 0 lists. Plus: `wacore/Cargo.toml` `exclude` for the 16 MB testdata (a patch release of `wacore` fixes the 25× crate size on its own). Plus the WA-Web rule (§G) written into `wa_web_reference.md`.
2. **Move the bridge into `bindings/wasm` (A).** `git subtree add` for history; `publish = false`; not in `default-members`; a `wasm-bindings` CI job path-filtered on `bindings/**`, `src/**`, `wacore/**`, `waproto/**` that runs the bridge's three jobs (fmt/clippy incl. reduced feature sets, `wasm-pack` build + `bun test` + size/shape gates, wasm32 unit tests). Delete `codegen/`. Publish `@oxidezap/whatsapp-rust-bridge` from here on the same version line (0.20.0, no consumer-visible change). Archive the old repo with a pointer.
3. **`ts` feature in the core (in-tree, minor).** `#[cfg_attr(feature = "ts", derive(Tsify))]` + `#[cfg_attr(feature = "ts", serde(rename_all = "camelCase"))]`-style attributes on the result types, event payloads, Signal record components and `DevicePropsOverride`; `bindings/wasm` enables it. Delete `generated_types.rs`, most of `result_types.rs`, `signal_records.rs`, `legacy_session.rs`, `device_props.rs`, `client_profile.rs`. Bridge major (0.21.0) only if any `.d.ts` name changes; aim for none.
4. **One event shape + `run() -> TerminalReason` + `KvBackend` (core 0.8, bridge major).** The previous audit's Phase 1–2, now done in one PR set because both sides are in one tree. baileyrs deletes `Bridge/types.ts`, ~80% of `schema.ts`, `terminal-close.ts`.
5. **VoIP crates (core 0.8, same release as 4).** Feature names `voip*` stay as forwarders so nobody who never enabled them notices.
6. **baileyrs (C).** `forward()` helper; generated WAProto facade; `protobufjs` out of `dependencies`; codec fuzz moved to `bindings/wasm/tests`; `check-layer-boundaries.ts` deleted (now a workspace lint). baileyrs minor unless `exports["./lib/*"]` is narrowed, which is the one change worth a baileyrs major and should ride with it.
7. **Expiry of `legacy-session-interop`** per the §G rule, on a date baileyrs announces.

### Semver consequences per package

| package | steps 1–3 | steps 4–5 | step 6–7 |
| --- | --- | --- | --- |
| `whatsapp-rust` (crates.io) | 0.7.x patch (`exclude`, dead code under `WANTED` is not public API) → 0.7.x minor-in-spirit for the `ts` feature (additive) | **0.8.0** (event shape, `run()` return type, `FeatureError`, VoIP forwarders, store-trait cleanup — batch them) | 0.8.x |
| `wacore`, `wacore-libsignal`, `wacore-appstate`, `waproto` | 0.7.x (`exclude`; `ts` feature additive) | **0.8.0** (dead `pub fn` removal, middle trait family — with a one-release blanket-impl bridge for `opencrabs`) | 0.8.x |
| `whatsapp-rust-sqlite-storage` | 0.7.x | **0.8.0** (`_for_device`) | — |
| `@oxidezap/whatsapp-rust-bridge` (npm, now from the core repo) | 0.20.0 (source move, no surface change) → 0.21.0 only if a `.d.ts` name moves | **major** (0.22.0 in 0.x terms; one event shape, `TerminalReason`) | — |
| `@oxidezap/baileyrs` | patch | minor (deletions are internal unless `./lib/*` exports are counted, which they are today) | **major** when `exports["./lib/*"]` is narrowed and `protobufjs` leaves runtime deps |
| upstream `baileys` (pins `whatsapp-rust-bridge@0.5.4`) | unaffected — a different package on a frozen line; optionally offer the two symbols from the new package's utility entry point | unaffected | unaffected |

### What I could not verify

- CI wall-clock per job: GitHub's API and `gh` were blocked from this session. Job counts and `timeout-minutes` are from the workflow files.
- GitHub code search for private/unpublished consumers of the crates or the scoped npm package. Registry and crates.io data only.
- Whether `wbails` bundles its own wasm or resolves the unscoped bridge at runtime (its `native-bridge.js` does a dynamic `require('whatsapp-rust-bridge')`; the tarball carries no `.wasm`).
- The `run()` signature `mendia`/`opencrabs` call — flagged above as the one Phase-1 change with an external caller.
