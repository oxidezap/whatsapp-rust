# whatsapp-rust-bridge — Rust-side audit

Scope: `src/*.rs`, `src/wasm_client/*.rs`, `Cargo.toml`, `benches/`. Core read for comparison at `/home/user/whatsapp-rust`.
All paths below are relative to `/home/user/whatsapp-rust-bridge` unless prefixed `core:`.

## Inventory (what the 25.3k Rust lines actually are)

| Bucket | Lines | Notes |
|---|---:|---|
| `#[cfg(test)]` modules inside `src/` | **3,795 (15%)** | wasm_client.rs 1,556 · wire_batch.rs 1,002 · errors.rs 753 · js_backend 126 · proto 122 · runtime 70 · camel 65 · others 101 |
| `src/wasm_client.rs` 5,208 | 1,556 test · ~1,450 event plumbing/batching (637–1,600, 1,817–2,180) · ~340 TS `typescript_custom_section` strings (81–118, 320–636) · ~800 conversion helpers (2,877–3,084, 3,133–3,976) · ~245 reconnect gate (2,533–2,775) · ~230 init/create (2,309–2,530) · ~120 parse helpers (2,180–2,308) | zero exported methods, as AGENTS.md says |
| `src/wasm_client/*.rs` 4,764 | **174 exported methods** (business 7, chat_actions 22, connection 23, contacts 19, groups 27, media 7, messaging 14, newsletter 22, signal 33) | 125 `.online()` sites, 59 `unwaited(...)`, 58 explicit `map_err(BridgeError::from)`, 27 `unchecked_param_type` |
| Hand-written mirrors of core types | ~2,350 | result_types 1,364 (57 `into_wasm_abi` types) + signal_records 522 + legacy_session 480 + device_props/client_profile 335 − tests |
| JS adapters | 3,150 | js_backend 1,824 · js_transport 324 · js_crypto 542 · js_cache_store 144 · js_http 95 · js_time 62 · js_bytes 67 · js_keys 42 · runtime 553 |
| Never-shipped feature code | 923 | audio 475 · image_utils 214 · sticker_metadata 234 (`default` excludes them; AGENTS.md: "has never shipped") |
| memory_profile | 1,125 | ~1,000 behind `memory-profiling`, ~90 stub + 11 call sites in hot dispatch |

---

## Ranked findings

### 1. `js_backend.rs` is a generic "Backend over a namespaced KV store" that the core should own (bridge −1,450 / core net ≈ −700)

**Evidence.** `src/js_backend.rs:630-1626` implements all 81 methods of `SignalStore` (25) + `AppSyncStore` (11) + `ProtocolStore` (36) + `MsgSecretStore` (5) + `DeviceStore` (4) on top of three JS callbacks (`get/set/delete`) plus optional `setMany/getMany/deleteMany/listKeys/deletePrefix`. Nothing in it is JS-specific except `js_get_raw/js_set_raw/...` (lines 283–478, ~200 lines). The rest is storage *policy*: meta self-indexes for hosts that cannot enumerate (`META_SENT_MESSAGE_KEYS`, `META_SENDER_KEY_GROUPS`, `META_TC_TOKEN_JIDS`, `META_MSG_SECRET_KEYS`, `MUTATION_MAC_INDEX`, lines 52–61, `needs_self_index` line 493), timestamp-prefixed expiry scans with TOCTOU re-read (`scan_expired`/`confirm_expired`, 504–583), JSON encoding of `Device` with the `#[serde(skip)] account` stored as a protobuf side-channel (1,574–1,603), key-schema (`compound_store_key`, 72). `core:wacore/src/store/in_memory.rs` (2,781 lines) is the same policy re-derived over `HashMap`s.

**Change (core).** Add `wacore::store::kv::{KvStore, KvBackend<S>}`: `KvStore` = `get/set/delete` + optional `set_many/get_many/delete_many/list_keys/delete_prefix` + a `capabilities()` struct; `impl Backend for KvBackend<S: KvStore>` carries the index/expiry/Device-JSON logic once; re-implement `InMemoryBackend` as `KvBackend<HashMapKv>`. The bridge keeps a ~250-line `JsKvStore` (the `js_*_raw` functions + `parse_entry_array` + `JsCallbackError`).

**LOC.** Bridge −1,450; core +~1,400 in `kv.rs`, −~2,000 in `in_memory.rs`. **Risk:** medium — the on-disk key schema (`store:key` names, meta lists, `device`/`account` split) is a persisted contract for existing JS hosts; it must be carried over byte-for-byte and pinned by a fixture test. **Behaviour-preserving:** yes if the key schema is preserved.

### 2. Result structs are field-by-field mirrors of core structs that lack `Serialize` (bridge −900)

**Evidence.** 10 named `*_to_result` fns + 64 inline `result_types::XResult { … }` literals. Exact 1:1 copies (same field names, only `Jid→String`, `u64→f64`, enum→`as_str()`):
- `GroupMetadataResult` (`src/result_types.rs:587-655`, ~50 fields) ↔ `core:src/features/groups.rs:149 GroupMetadata` (`derive(Debug, Clone, Default, PartialEq, Eq)` — no Serialize); converter `src/wasm_client.rs:2992-3082` (91 lines).
- `MemoryDiagnosticsResult` (`result_types.rs:682-753`, ~55 fields) ↔ `core:src/client.rs:820 ResourceReport` (`derive(Debug, Clone)`); converter `src/wasm_client/connection.rs:533-616` (84 lines of `x: d.x as f64`).
- `NewsletterMetadataResult` ↔ `NewsletterMetadata` (`wasm_client.rs:3671-3687`); `ProductResult`+5 sub-structs ↔ `Product` (`3769-3819`); `BusinessProfileResult`+3 ↔ `BusinessProfile` (`3821-3858`); `BotListResult`+4 ↔ `BotList` (`3217-3272`); `ParticipantChangeResult`, `IsOnWhatsAppResult`, `UserInfoResult`, `CommunitySubgroupResult`, `NewChatMessageCappingResult` (`connection.rs:376-402`), `NewsletterMessageResult/FollowerResult/AdminInfoResult`, `CatalogResult/CollectionResult/OrderResult`, `Reachability` (`result_types.rs:166-200`), `MediaType` (`14-56`, 9-variant copy + `From`).
- Count: **≈24 result types are pure mirrors**, ~22 more are small wrappers; only inputs (`ReadMessageKey`, `SignalSessionBundleInput`, `CatalogOptionsInput`, …) are genuinely bridge-shaped.

**Why the mirror exists.** The core types are not `Serialize`, and even where they are (`Event` payloads) the JS shape differs: events carry `Jid` as `{user, server, agent, device, integrator}` (`generated_types.rs:9-16`) while method results carry `"user@server"` strings — two representations of the same type on one surface.

**Change (core).** Derive `Serialize` (feature `serde`, `rename_all = "camelCase"`, `skip_serializing_if = "Option::is_none"`) on the feature result types and on `ResourceReport`/`MemoryReport`; keep `Jid` serialization as it already is for events. Then each mirror + converter collapses to `serde_wasm_bindgen::to_value(&core_value)` and the TS type moves into `generated_types.rs` (already produced from the core serde schema by `codegen/`). `#[non_exhaustive]` enums (`NewsletterRole`, etc.) get a `Serialize` that emits the variant name, replacing `newsletter_*_str` (`wasm_client.rs:3609-3669`).

**LOC.** Bridge −~900 (result structs −600, converters −300); core +~60 derive lines. **Risk:** medium — the `u64 → f64` and `i64 → String` (money) conventions must be reproduced with `serialize_with`, and JID-as-string vs JID-as-object is a JS-visible change unless a `serialize_with = "jid_as_string"` is used per field. Do it type-by-type; `GroupMetadataResult` + `MemoryDiagnosticsResult` alone are −280 lines. **Behaviour-preserving:** yes with the field-level `serialize_with` shims.

### 3. `signal_records.rs` + `legacy_session.rs` are 1,000 lines of DTO + `From`/`TryFrom` for core types that should just be serde (bridge −850)

**Evidence.** `src/signal_records.rs:28-231` declares 15 structs/enums mirroring `core:wacore/libsignal/src/protocol/record_components.rs` (`SessionRecordComponents`, `SessionComponents`, … all `derive(Clone, PartialEq, Eq, Default)` — no serde), then 233–484 are 12 `From`/`TryFrom` impls copying fields both directions, plus two "declaration-only" structs (`SenderSessionChainComponents:60`, `ReceiverSessionChainComponents:76`) that exist only to emit TS types. `src/legacy_session.rs:26-172` repeats the pattern for `LegacySessionRecordV1` (`core:wacore/libsignal/src/protocol/legacy_session.rs:134`, no serde) with 11 more conversions (175–420). The exported surface is 6 functions (`signal_records.rs:486-522`, `legacy_session.rs:444-480`).

**Where it belongs.** The typed model already lives in the core behind `legacy-session-interop`; the Baileys-JSON→`LegacySessionRecordV1` mapping already lives in JS (consumer). What is misplaced is the *serialization*: it is a property of the core types. Deriving `Serialize/Deserialize` (`rename_all = "camelCase"`, `#[serde(with = "serde_bytes")]` on key fields) on the core structs, gated by the same feature, lets the bridge pass them straight through `from_js_input`/`to_value` and generate their TS from the core schema like every event payload.

**LOC.** Bridge −850 (both files → ~120 lines of exports); core +~40 attribute lines. **Risk:** low — the JS field names are already camelCase and byte fields are already `Uint8Array`; the `key_material::<N>` length checks (`signal_records.rs:233`) move to a `TryFrom<Vec<u8>>` in the core, where they arguably already are. **Behaviour-preserving:** yes.

### 4. Error classification (`errors.rs`) is core knowledge implemented in the bridge; every other FFI would re-implement it (bridge −650 / core +400)

**Evidence.** `src/errors.rs:134-247 from_error_chain` walks `source()` downcasting to 8 leaf types; `304-450` maps `IqError` **twice** (`iq_to_bridge` for `whatsapp_rust::request::IqError`, `wacore_iq_to_bridge` for `wacore::request::IqError` — 80 lines of identical arms for two enums the core keeps as twins); `561-728 classify!` enumerates source-less variants of 19 core error types. 753 of 1,574 lines are tests that pin "every domain surfaces its server code" (`929`), "every carrier of a rejection reports it identically" (`1008`) — i.e. tests that the bridge has correctly re-derived what the core meant.

**Change (core).** The core already has `ErrorChainExt` (`core:src/error.rs:106`) answering *timeout* and *transport gone*. Extend it to the full 11-kind classification: `fn classify(&dyn Error) -> ErrorClass { Server{code,text,error_type,backoff} | Timeout | NotConnected | Disconnected | InvalidArgument{field?} | ProtocolViolation | Crypto | Storage | NoRecipientDevice{attempted} | Internal }`. The bridge then keeps `BridgeError` (the wire shape), `Withdrawn`, the JS-side `js_sys::Error` construction (`760-820`), and a 30-line `From<ErrorClass>`. Also collapse the two `IqError` twins in the core (one `From` between them, or one type).

**LOC.** Bridge −650 (incl. −500 tests that become core tests); core +~400. **Risk:** low-medium — the `field` naming for `InvalidArgument` is partly bridge knowledge (`"phoneNumber"`, `"customCode"` at `389-448`); keep a small bridge override table for those. **Behaviour-preserving:** yes.

### 5. Reconnect gating: `Parked`/withdraw is a generic FFI need; the core should expose a cancellable wait (bridge −180)

**Evidence.** `src/wasm_client.rs:2533-2773` — `Unwaited` enum (7 variants, `_why` never read, `2689`), `CoreClient`, `Parked` registry (`BTreeMap<u64, Sender<()>>`), `LeaveOnDrop`, `park()` racing `wait_until_reachable()` against a withdraw signal, `online_committed()`, `withdraw_parked()`. The core has `reachability()` and `wait_until_reachable()` (`core:src/client/lifecycle.rs:2066, 2117`) but its cancellation model is "drop the future", which no FFI host can do (AGENTS.md documents this exactly).

**Change (core).** `Client::reconnect_gate() -> ReconnectGate` with `async fn wait(&self) -> Result<(), Withdrawn>` and `fn withdraw_all(&self) -> u32`, built on the existing `wait_for_reachability` notifiers. The bridge keeps `CoreClient { online() / unwaited() }` as a ~40-line visibility wrapper and the `Unwaited` enum as documentation. Python/uniffi/C bindings get the same primitive for free.

**LOC.** Bridge −180; core +~120. **Risk:** low. **Behaviour-preserving:** yes. *Keep* the `online()`/`unwaited(why)` split — it is cheap (one branch) and its value is at the call site.

### 6. Per-method boilerplate: ~174 methods × 6–10 lines of the same chain (bridge −400 to −600)

**Evidence.** The dominant shape (`chat_actions.rs:13-33`, `newsletter.rs:14-88`, `groups.rs:13-30`, …):
```rust
let jid = parse_jid(jid)?;
self.client.online().await?.<feature>().<call>(&jid, …).await.map_err(crate::errors::BridgeError::from)
```
125 `.online()` sites; 58 explicit `map_err(BridgeError::from)` where `?` would do (`From` impls exist for every core error type — `errors.rs:484-560, 561-728`); `chat_actions.rs:pin_chat/archive_chat/mute_chat` each duplicate the whole chain in both `if` arms (lines 18–33, 45–68, 76–93) instead of choosing the fn once. Nine `Reflect::set(…).map_err(|e| internal(format!("{e:?}")))` per hand-built object in `signal.rs:203-246`.

**Change (bridge).** (a) Replace `.await.map_err(crate::errors::BridgeError::from)` with `.await?; Ok(())` (58 sites, −116 lines, mechanical). (b) A `macro_rules! core_call { ($self:ident . $feat:ident . $method:ident ( $($arg:expr),* )) => … }` or an `async fn online_call<T, E, F>(&self, f: impl FnOnce(&Client) -> F) -> Result<T, BridgeError>` collapses the 7-line chain to one line for ~90 straightforward methods. (c) Hand-built JS objects in `signal.rs:222-246, 183-220` → `#[derive(Serialize, Tsify)]` structs (also gives them a TS type, which `getUSyncDevices` currently lacks — returns `JsValue`).

**LOC.** −400 to −600. **Risk:** very low. **Behaviour-preserving:** yes.

### 7. Six copies of "get JS function, await maybe-promise, map error" across the adapters (bridge −150)

**Evidence.**
- Required-function extraction: `js_transport.rs:156-167` (×3), `js_http.rs:32-34`, `js_cache_store.rs:27-38` (×4), `js_crypto.rs:97-101 extract`, `wasm_client.rs:2384-2395` (×3 with `internal(...)` — wrong kind for a caller error), `wasm_client.rs:940-956 optional_method`, `js_crypto.rs:102-113 extract_optional`, `wasm_client.rs:2398-2402 opt_fn`.
- Await-maybe-promise: `js_transport.rs:249 resolve_maybe`, `js_http.rs:70-76` inline, `js_cache_store.rs:55 resolve_promise`, `js_backend.rs:1647 resolve_promise`.
- Each adapter's `from_js` returns `Result<Self, JsValue>` with a bare string (`"transport.connect must be a function"`), then `create_whatsapp_client` converts via `From<JsValue> for BridgeError` → `Internal` (`errors.rs:547-556`) — so a caller passing a malformed transport object gets `kind: "internal"`, which AGENTS.md calls the wrong answer.

**Change (bridge).** One `src/js_fn.rs` (~50 lines): `required(obj, name, field) -> Result<Function, BridgeError::InvalidArgument{field}>`, `optional(...)`, `async fn settle(JsValue) -> Result<JsValue, JsValue>`. Delete the six copies.

**LOC.** −150. **Risk:** none. **Behaviour-preserving:** *not quite* — error kinds for malformed config objects change from `internal` to `invalid-argument` (a fix AGENTS.md asks for).

### 8. `uploadEncryptedMediaStream` re-implements the core's CDN upload driver (bridge −170)

**Evidence.** `src/wasm_client/media.rs:255-395` hand-rolls: media-conn refresh with auth retry (`attempt` loop, `force_refresh`), host failover, the `?resume=1` progress check with `serde_json::Value` parsing (`300-323`), the `?auth=&token=` URL format (`326-329`), response parsing (`340-360`), plus `wasm_client.rs:3476-3517` (`base64_url_encode`, `is_auth_error`, `stream_upload_via_js`). The core has exactly this driver: `core:src/upload.rs:180 upload_media_with_retry` (host failover, auth refresh, resume — `src/upload.rs:20-64`) and a public `Client::upload_stream<S: UploadSource>` (`src/upload.rs:463`). The bridge's version also *buffers the whole stream into a `Vec`* (`stream_upload_via_js:3500-3507`), so the "streaming" method is not streaming, and it uses `resize(…, 0) + copy_to` (zero-fill + double length read — the exact pattern `js_bytes.rs` exists to avoid).

**Change.** Bridge: implement `UploadSource` for a JS `getBody` factory that buffers once (`len()` is required up front — it already knows `file_length`), call `client.upload_stream(source, info, mt)`. Delete the loop, `base64_url_encode`, `is_auth_error`, `stream_upload_via_js`, and the `base64` dependency. **Core (optional):** a `Client::upload_encrypted_reader(impl Read)` if `UploadSource::reader_from` re-reads are undesirable for a one-shot stream.

**LOC.** −170; drops `base64` and one of two `serde_json::Value` uses. **Risk:** low — the core driver is the one `uploadMedia` already uses. **Behaviour-preserving:** yes (error kinds improve: `internal("Upload failed on all hosts")` becomes the core's typed error).

### 9. `device_props.rs` / `client_profile.rs`: input DTOs + 25-variant enum copy + merge policy that belongs in the core (bridge −250)

**Evidence.** `src/device_props.rs:19-77` copies `waproto::device_props::PlatformType` (25 variants) and a 25-arm `From`; `142-166 merge_into` implements "partial override merges into `default_history_sync_config()` so callers don't drop the support_* claims" — a rule about the core's `DevicePropsOverride` (`core:wacore/src/store/device.rs:122`, `derive(Debug, Clone, Default)`, no serde). `client_profile.rs:16-40` re-implements "apply overrides onto a preset" over `core:wacore/src/client_profile.rs:36-78` presets.

**Change (core).** `DevicePropsOverride: Deserialize` (camelCase, `PlatformType` via `#[serde(with = "…")]` on the prost enum name) and `DevicePropsOverride::with_history_sync_patch(HistorySyncPatch)` owning the merge; `ClientProfile: Deserialize` via a tagged `ClientProfileSpec { preset, os_version, overrides }`. Bridge: `from_js_input::<DevicePropsOverride>` directly.

**LOC.** Bridge −250; core +~80. **Risk:** low. **Behaviour-preserving:** yes.

### 10. `wire_batch.rs`: not a protobuf duplicate, but two string-table implementations for one protocol (bridge −150)

**Evidence.** It is a bespoke *framing* around already-prost-encoded payloads (`wire_batch.rs:511 waproto::codec::message_encode_into`) plus fixed-width metadata records; it does **not** re-encode protobuf and has nothing to do with `wacore-binary`'s XMPP node marshal (different domain: stanza framing vs. event batching). Using `wacore-binary` here would be wrong. What *is* duplicated is the cross-batch dictionary: `WireStringTable` (`118-320`: u32 indices, byte ceiling, `commit/abandon/roll`, `PACKED_FLAG_CLEAR_AFTER`) for messages vs `FlatBatchWriter` (`737-899`: u16 slots, separate JID cache, `begin/invalidate`) for receipts/acks. Both implement definitions-region + inline-region + reset-flag with different widths and different roll semantics, and both are mirrored twice in `ts/wire-info.ts`. Tests are 1,002 of 2,211 lines.

**Change (bridge, coordinated with ts/).** One `StringTable<Idx: u16|u32>` with the message table's roll/abandon semantics (they are stricter and the receipt path would gain `CLEAR_AFTER`). **LOC.** −150 Rust, similar in TS. **Risk:** medium — wire format for receipts changes (u16→u32 slots or vice versa); must be a coordinated version bump. **Behaviour-preserving:** wire bytes change, semantics don't. Lower priority than 1–9; the format is well-tested.

### 11. `camel_serializer.rs` cannot be deleted today, but 60% of its reason to exist is a `waproto` build attribute (bridge −350 if done in core)

**Evidence.** Used in exactly three places: the 16 `serialize_with_proto` event fields (`wasm_client.rs:181-190`), `getAccount` (`connection.rs:525`), and the `LazyHistorySync` envelope (`wasm_client.rs:2035`). It exists because prost-generated types serialize snake_case, `serde_wasm_bindgen` has no rename option, and the JS side wants protobufjs conventions: camelCase keys, `Long {low,high,unsigned}` for 64-bit, `Uint8Array` for `Vec<u8>`, default-skipping (`camel_serializer.rs:1-20`). The camelCase half is trivially a core change: `prost_build.type_attribute(".", "#[serde(rename_all = \"camelCase\")]")` in `core:waproto/build.rs` (which already splices `LOCAL_FIELDS` into the descriptor). The `Long` and `Vec<u8>→Uint8Array` halves need either `serialize_with` attributes on i64/u64/bytes fields (also emit-able from `build.rs` via `field_attribute`) or the custom serializer. Default-skipping (`Defaults::{Skip,Keep,KeepPresent}`, `560-700`) is protobufjs semantics and stays a bridge decision.

Also note three separate JS-string interning caches doing one job: `CAMEL_KEY_CACHE` (`camel_serializer.rs:36`), `INTERNED_NAMES` (`wasm_client.rs:127`), `js_keys!` (`js_keys.rs`) — plus `JID_VALUE_CACHE` (`camel_serializer.rs:66`) which runs `parse_jid_fast` on *every* string field serialized (`serialize_str:330`), including message text.

**Change.** Core: emit `rename_all = "camelCase"` + `serialize_with` for 64-bit/bytes from `waproto/build.rs` behind a `js-serde` feature. Bridge: then `camel_serializer` shrinks to the ~150-line default-skipping `Serializer` wrapper around `serde_wasm_bindgen`, or is replaced by a post-pass. Merge the three key caches into `js_keys`. **LOC.** −350 (−650 if the skip semantics can be expressed as `skip_serializing_if` from build.rs). **Risk:** medium — `Long` shape is a documented JS contract (AGENTS.md "Money"). **Behaviour-preserving:** must be, verified by the existing `dispatched_event_tests`.

### 12. `audio.rs`, `image_utils.rs`, `sticker_metadata.rs` are consumer concerns that never ship (bridge −923, −3 deps)

**Evidence.** `Cargo.toml:47-49` — `audio`/`image`/`sticker` are outside `default`; AGENTS.md: "anything not in `default` has never shipped." They pull `symphonia`, `image`, `img-parts`, `uuid` (only user: `sticker_metadata.rs`), and `serde_json` for EXIF JSON. They wrap no core capability (they are Baileys `generateWaveform`/`extractImageThumb`/`sticker EXIF` conveniences); the core already has `wacore::webp::is_animated` and `wacore::sticker_pack` for the protocol-facing side. Keeping them costs a second test tree (`test/`), three optional deps in `Cargo.lock`, and the `getEnabledFeatures()` runtime probe (`lib.rs:64-84`).

**Change.** Move to a separate npm/wasm package (or into baileyrs), delete from the bridge. **LOC.** −923, `test/` directory, 4 dependencies. **Risk:** none for shipped artifacts. **Behaviour-preserving:** yes for `default`.

### 13. `runtime.rs` is legitimately the bridge's, except the spawn throttle which papers over a core bug (bridge −70)

**Evidence.** The core ships no wasm `Runtime` (`core:src/runtime_impl.rs` is `cfg(not(wasm32))` only), so the timer registry (`runtime.rs:62-240`), `setImmediate` yield (`245-460`) and `yield_frequency()=1` belong here. But `runtime.rs:265-345` (SPAWN_QUEUE, `SPAWN_BATCH_SIZE=16`, `drain_spawn_queue`) exists because "hundreds of per-chat workers … contend on an upstream 1-permit semaphore … each permit release wakes ALL waiters" (comment at 268–275). That is a thundering-herd in the core's offline-sync fan-out, and rate-limiting *spawns* is the wrong layer to fix it.

**Change (core).** Bound per-chat worker concurrency at the spawn site (e.g. `Semaphore::acquire_owned` before spawn, or a fixed worker pool) so a permit release wakes one waiter. Then delete the throttle. **LOC.** Bridge −70. **Risk:** low once the core fix lands; needs the offline-replay stress case re-run. **Behaviour-preserving:** yes.

### 14. `memory_profile.rs` instrumentation leaks into the production dispatch path (bridge −60 in hot code)

**Evidence.** 11 calls (`record_history_sync`, `record_history_conversation`, `record_history_batch`, `record_history_event_{enqueued,dequeued,completed,cancelled}`, `enter_scope` ×7) inside `dispatch_history_sync_wire_batches`/`emit_hs_wire_batch`/`enqueue` (`wasm_client.rs:1290-1302, 1849-2023`) compile to no-ops in production but make a 130-line function 40% instrumentation. `history_event_compressed_bytes`/`record_history_event_dequeued` (`1824-1836`) exist only to feed them.

**Change.** Behind a `#[cfg(feature = "memory-profiling")]` `HistoryScope` guard type with an inert twin, so the dispatch reads as one `let _s = profile::history(Phase::Decode);` per phase; or move the counters into the core's own `tracing` spans (`Cargo.toml:70`: "The core's own tracing metadata supplies task names"). **LOC.** −60 in the hot path. **Risk:** none. **Behaviour-preserving:** yes.

### 15. Test volume inside `src/`: 3,795 lines, 45% of `wire_batch.rs`, 48% of `errors.rs`

**Evidence.** Counts above. All run only under `wasm-pack test --node` (`Cargo.toml:154-159`); each `#[cfg(test)]` module aliases `wasm_bindgen_test as test`. Items 1–4 above move ~1,300 of these lines into the core as tests of core behaviour (`errors.rs:929 every_domain_surfaces_its_server_code` is a test of the core's error chain, not of the bridge). The remaining bridge tests (event delivery, packed-batch borrowing, node round-trip) are appropriate, but `wasm_client.rs`'s 1,556 test lines belong in `src/wasm_client/tests/` (sibling modules already reach private items) so the 5,208-line file becomes ~3,600.

**Change.** Mechanical move; `#[path]`-free since child modules see the parent's privates. **LOC.** 0 net, −1,556 from the largest file. **Risk:** none.

---

## Performance across the boundary (findings that are not code-size)

| Site | Issue | Fix |
|---|---|---|
| `wasm_client.rs:3387` `js_to_node` | `Uint8Array::from(content_val)` then `.to_vec()` — the two-length-read + zero-fill path `js_bytes.rs:1-12` documents | `js_bytes::to_vec(&content_val.unchecked_into())` |
| `wasm_client.rs:3500-3506` `stream_upload_via_js` | `resize(len, 0)` + `copy_to` per chunk (zero-fill + second length read) | `js_bytes::append`; disappears with finding 8 |
| `media.rs:79-142` `downloadMediaStream` | Core has no streaming download (`download_from_params` returns the whole `Vec<u8>`, comment at 82–88), so the "stream" is a chunker over a fully materialised buffer | **Core:** a `download_to_writer`/`DownloadWriter` path exists (`core:wacore/src/download.rs:327 DownloadWriter`) — expose `Client::download_stream` and feed the JS sink chunk-by-chunk |
| `proto.rs:31-58` `to_js_value` fallback | On any out-of-range i64, the entire event is re-serialised through `serde_json::Value` and walked; keeps serde_json's `Value` machinery in the binary for one rare field | Acceptable; but it is the only production `serde_json` use once findings 1 and 8 land (js_backend's JSON moves to core, media JSON goes) — worth measuring whether `serde_json` can leave `default` |
| `camel_serializer.rs:330` `serialize_str` | `is_jid_like` → `parse_jid_fast` on every string, including message bodies, to decide interning | Intern by *field name* (the serializer sees the key in `serialize_field`) instead of by sniffing the value |
| `wasm_client.rs:2047` history remainder | `history_sync_to_vec` → `Vec` → `Uint8Array::from` (two copies) | Encode into a reused `Vec` and `cross_bytes` as the packed paths do (`wire_batch.rs:423`) |
| `wasm_client.rs:1290-1303` `enqueue` | `try_send` on a 16,384-deep channel; overflow is `log::warn!` and the event is **dropped** | Not a perf issue but a correctness cliff worth surfacing to the host (a counter on `getMemoryDiagnostics`, or back-pressure via `send().await` on the core's dispatch task) |
| Event path generally | Messages/receipts/acks are packed and coalesced into one envelope; other 36 variants go through `serde_wasm_bindgen` one object per event with a 50-callback yield budget (`647`). This is already the right design; no per-event JSON round-trips exist. | — |

---

## Quick wins (each < 1 hour, no design discussion)

1. Delete the 58 redundant `.map_err(crate::errors::BridgeError::from)` — `?` already converts (`From` impls at `errors.rs:484-728`). −116 lines.
2. `chat_actions.rs:13-93` — pick the core fn once (`let f = if pin { pin_chat } else { unpin_chat }`) instead of duplicating the `online()` chain in both arms (×3 methods). −40 lines.
3. `wasm_client.rs:2384-2395` store `get/set/delete` extraction reports `internal(...)` for a caller-supplied malformed store object → `invalid_arg("store.get", …)`. Same for `parse_optional_version` / `parse_optional_count` (`2180-2237`, six `internal` returns for caller input) and `js_node_array_to_vec:3543` / `js_to_node:3352-3355`. Contract fix AGENTS.md explicitly asks for.
4. Introduce `src/js_fn.rs` (`required`, `optional`, `settle`) and delete the six copies (finding 7).
5. `js_to_node:3387` and `stream_upload_via_js:3504` → `js_bytes::to_vec` / `append`.
6. Move `#[cfg(test)]` modules out of `wasm_client.rs` into `src/wasm_client/tests_*.rs` (finding 15). −1,556 lines from the file.
7. `signal.rs:222-246 signalEncryptMessage` and `183-220 getUSyncDevices` → `#[derive(Serialize, Tsify)]` result structs; gives both a TS type (currently `JsValue`/`any`) and removes 9 `Reflect::set(...).map_err(...)` lines.
8. Merge `INTERNED_NAMES` (`wasm_client.rs:127-140`) into `js_keys!` — same thread-local pattern, one module.
9. `Cargo.toml`: `base64` is used only by the upload loop (finding 8); `uuid` only by `sticker_metadata.rs` (finding 12). After those land, drop both.
10. `compression.rs`, `crypto.rs`, `addon_crypto.rs` (226 lines, 9 free functions) → one `utils.rs`, and switch their `Result<_, JsValue>` string errors to `BridgeError` (AGENTS.md lists this as the known deviation; `error_value` in `wasm_utils.rs` then has one caller left).

## Summary of where the deletions come from

| Finding | Bridge Δ | Core Δ | Layer |
|---|---:|---:|---|
| 1 KV backend | −1,450 | +1,400 / −2,000 in_memory | core |
| 2 Serialize on result types | −900 | +60 | core |
| 3 serde on signal/legacy records | −850 | +40 | core |
| 4 ErrorClass | −650 | +400 | core |
| 6 method boilerplate | −500 | — | bridge |
| 11 camelCase from waproto build.rs | −350 | +30 | core |
| 12 unshipped media codecs | −923 | — | consumer |
| 9 device props / profile serde | −250 | +80 | core |
| 5 reconnect gate | −180 | +120 | core |
| 8 upload driver | −170 | 0 | bridge (core has it) |
| 7 js_fn helpers | −150 | — | bridge |
| 10 one string table | −150 | — | bridge+ts |
| 13, 14 | −130 | +40 | core / bridge |
| **Total** | **≈ −6,650 (−26%)** | | |
