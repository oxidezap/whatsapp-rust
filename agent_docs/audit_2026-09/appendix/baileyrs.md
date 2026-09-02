# baileyrs architecture / performance / DRY audit

Scope: `src/{Bridge,Socket,Utils,Types,Compatibility,Defaults,Platform,WABinary,WAUSync,WAProto}`, `scripts/`, `Example/`, `package.json`, tsconfig. Bridge = `@oxidezap/whatsapp-rust-bridge` 0.19.0 (source read at `/home/user/whatsapp-rust-bridge`).

Size picture (non-test TS): Bridge 2.3k, Socket 5.1k, Utils ~4.9k, Types 1.9k, Compatibility ~5.6k, WAProto 14k lines of vendored `.d.ts` (790 KB) + 123 KB generated schema table. Tests + fuzz ≈ 31k lines (≈1.5× the source). Sixty-plus percent of the non-proto source is translation between three shapes of the same data: bridge DTO → "canonical" DTO → Baileys DTO.

Answers to the direct questions first, then the ranked findings.

- **Is `Bridge/schema.ts` re-validating what the bridge already typed?** Yes. Every adapter receives `Extract<WhatsAppEvent,{type:T}>['data']` (typed) and immediately treats it as `unknown` through `isObject/asString/asNumber/asJidString` (`schema.ts:1-25` documents why: `any` leaks for `.action`, RFC-3339 vs unix timestamps, `{secs,nanos}` durations, PascalCase vs snake_case receipt types, `Jid` structs vs strings). The defensiveness is a response to bridge inconsistencies, not to untrusted input.
- **`Bridge/types.ts` vs the bridge's `.d.ts` — duplicates?** Yes, nearly all 769 lines. `CanonicalGroupAction` (45 variants, `types.ts:266-360`) is `GroupNotificationAction` with camelCase keys; `CanonicalMessage/Receipt/IncomingCall/Presence/…` mirror `MessageSource`, `Receipt`, `CallAction`, `PresenceUpdate`; `BridgeJid` (`primitives.ts:36`) mirrors `Jid`.
- **Is event adaptation table-driven?** Half. `schema.ts` is a `satisfies AdapterMap` table (compile-time exhaustive), but the entries are hand-written per event, and 26 of ~58 are `noop`. Then `Socket/events.ts` has a second hand-written table (`DISPATCHERS`) from canonical → Baileys. Two tables, three shapes.
- **Layered socket pattern?** Not present — good. `makeWASocket` composes `make*Methods(ctx)` factories and spreads them into one object (`Socket/index.ts:748-1015`). The remaining problem is that `index.ts` is 1,032 lines because it also holds ~25 inline methods and ~400 lines of lifecycle.
- **Repeated wrappers around bridge calls:** `(await ctx.getClient()).x(...)` appears **119 times** across `src/Socket/*.ts` (newsletter 21, groups 20, index 16, privacy 10, profile 8, messages 8, communities 8, …). The error boundary is already a single Proxy (`bridge-error-boundary.ts`), so the repeated bit is only the `await getClient()` + result mapper — one `forward()` helper removes most of it (Finding 6).

---

## Ranked findings

### 1. Bridge should emit one stable, camelCase, string-JID event shape so `Bridge/schema.ts` + `types.ts` + most of `primitives.ts` can be deleted

**Evidence.** `schema.ts` (1,175 lines) and `types.ts` (769) exist because the bridge's *object-event* payloads are inconsistent with its own *wire-batch* payloads:

| Concern | Object events (`WhatsAppEvent.data`) | Wire batches (`MessageWireInfo`, `ts/wire-info.ts:110`) |
|---|---|---|
| JIDs | `Jid` struct `{user,server,agent,device,integrator}` → `asJidString` everywhere (`primitives.ts:36-73`) | plain `string` |
| Keys | snake_case (`push_name`, `is_from_me`) | camelCase |
| Timestamps | RFC-3339 string (`fixtures.ts:14 timestamp: '2026-04-18T05:45:46Z'`) → hand-rolled RFC-3339 parser `primitives.ts:75-120` | unix seconds number |
| Durations | `{secs,nanos}` → `asDurationSeconds` `primitives.ts:178` | — |
| Receipt type | PascalCase because `#[serde(from = "String")]` disables the rename → 26-entry `RECEIPT_TYPE_MAP` with both spellings (`schema.ts:796-822`) | — |
| Sync-action `.action` | `any` in the bridge `.d.ts` → `extractAction` (`schema.ts:74`) and dual-spelling reads `action?.muteEndTimestamp ?? action?.mute_end_timestamp` (`schema.ts:339`) | — |
| `pair_success.id` | declared `Jid`, actually a string (`schema.ts:154-157`) | — |

Result: `adaptGroupAction` (`schema.ts:947-1120`) is a 170-line `switch` that renames `not_announce → notAnnounce`, `link_type → linkType`, etc. — a mechanical camelCase projection of the bridge's own typed union.

**Recommended change.** In the bridge: (a) serialize events through the existing `camel_serializer` (`src/camel_serializer.rs`) as it already does for protos; (b) serialize `Jid` as `user@server` string in events (a device-stripped string is what every consumer wants — the only address-preserving use is `incoming_call.call_creator`, which can be a second field); (c) `#[serde(with = "ts_seconds")]` on `DateTime` fields, `Duration` as seconds; (d) fix `ReceiptType` serialization; (e) export the nested action proto types so `.action` is not `any`; (f) fix `pair_success` types. In baileyrs: delete `types.ts`, shrink `schema.ts` to the ~6 adapters that carry real logic (`adaptIncomingCall`, `history_sync`, `newsletter_live_update`, `contact_number_changed`, `notification` attr-coercion, `message`), and route `DISPATCHERS` directly on `WhatsAppEvent['type']`.

**LOC:** baileyrs −1,500 to −1,700 (`schema.ts` 1,175→~200, `types.ts` 769→0, `primitives.ts` 219→~40, `Bridge/__tests__/adapt.test.ts` 839 mostly obsolete). Bridge +~60 (serde attributes, a few new fields).
**Risk:** medium — a bridge major version; consumer-visible Baileys events unchanged.
**Behaviour-preserving:** yes at the public API; no at the bridge boundary.

### 2. Bridge should expose run-loop completion (`run(): Promise<TerminalClose>`) so four JS state machines collapse into one

**Evidence.** `Socket/terminal-close.ts:1-53` opens with "This mirrors one decision that lives in the Rust engine… Until the bridge exposes loop completion, this table is how the socket knows a client has become dead weight." `index.ts:560-575` repeats it ("Freeing that automatically needs the bridge to expose loop completion"). Because `run()` returns `void`, baileyrs re-derives terminality from event patterns: `RECONNECTABLE_CONNECT_FAILURE_REASONS` (`terminal-close.ts:44`) mirrors `ConnectFailureReason::should_reconnect()`; `DISPATCHERS.disconnected` (`events.ts:359-370`) and `streamError` (`events.ts:433-451`) special-case `setAutoReconnect(false)` because the engine "dispatches Disconnected and only then tests the flag"; `isAutoReconnectEnabled` is threaded through `EventCallbacks`; `makeTerminalCloseReporter` (158 lines) exists to publish the close exactly once after teardown with a 60 s watchdog; `logout()` (`index.ts:611-690`) counts reporter claims to decide whether to synthesize a close; `makeBridgeClientOwner` (206 lines) is a fourth state machine; `WebSocketClient.close()` a fifth.

**Recommended change.** Bridge: `run()` returns a promise (or takes an `onTerminated(reason: DisconnectReason, error)` callback) that settles exactly once when the loop exits and never again. `logout()` resolves after that settles. baileyrs: `await client.run()` becomes the single source of "this socket is finished"; delete `terminal-close.ts`, the `disconnected`/`streamError` auto-reconnect branches, `isAutoReconnectEnabled`, most of `terminal-close-reporter.ts` (keep the "publish after teardown" ordering, which becomes `run().finally(teardown).then(publish)`), and the `hasReported` bookkeeping in `logout`.

**LOC:** −300 to −400 in `Socket/`, plus ~150 lines of tests that exist only to pin the mirrored decision (`auto-reconnect-terminal-close.test.ts` "setAutoReconnect(false) turns a plain drop terminal", `rate-limited-stream-error.test.ts`).
**Risk:** medium (lifecycle is the area with the most documented past bugs).
**Behaviour-preserving:** yes.

### 3. The message hot path allocates three intermediate objects per message; the object-event `message` adapter is dead code

**Evidence.** `wasm_client.rs:276-280`: "The bridge itself never emits `message`/`history_sync` events — both cross the boundary as wire batches." Yet `schema.ts:718-733` `adaptMessage`/`adaptMessageParts` (~60 lines) handles the object form, reachable only from tests/fuzz. Live path per message (`events.ts:1127-1160`): `WAProto.Message.decode(reader, len)` → bridge codec → `hydrate` re-parents every nested object (`proto-runtime.ts:~830`) → `adaptBridgeMessageWire` builds an 18-field `CanonicalMessage` (`schema.ts:741-780`) → `canonicalMessageToWAMessage` builds `MessageKey` + `WebMessageInfo` + `Long.fromValue(timestamp)` (`events.ts:105-133`) → `emitMessageUpsert`. The canonical object is consumed by exactly one function and carries nothing the `MessageWireInfo` did not.

**Recommended change.** Fuse `adaptBridgeMessageWire` + `canonicalMessageToWAMessage` into `wireInfoToWAMessage(info, message)`; delete `adaptMessage/adaptMessageParts` and the `message` arm of `CanonicalEvent`; keep the side-effect derivation (`reactionMessage`/`protocolMessage`) as a function of the `WAMessage`. Consider `messageTimestamp = info.timestamp` (a number, which the published type `number | Long` already permits) — flagged separately because a consumer calling `.toNumber()` would break.

**LOC:** −120. **Perf:** one fewer 18-property object + closure per message; `Long` allocation optional. **Risk:** low. **Behaviour-preserving:** yes (except the optional Long change).

### 4. History sync makes three passes and a synthetic event before it reaches `messaging-history.set`

**Evidence.** `onHistorySyncBatch` (`events.ts:1162-1166`) → `decodeHistorySyncWireBatch` decodes conversations and builds a fake `{type:'history_sync', data}` (`history-sync-wire.ts:69-76`) → `onEvent` → `adaptBridgeEventViaSchema` → `history_sync` adapter re-reads `overlay.syncType/chunkOrder/progress/batchIndex` off that object (`schema.ts:507-545`) → `processHistoryMessage` (`process-history-message.ts:70-215`) → `CanonicalHistorySync` (13 fields) → `DISPATCHERS.historySync` copies the same 10 fields into the payload (`events.ts:1002-1013`) → `event-buffer.append` copies chats/contacts/messages into `historySets` maps when buffering (`event-buffer.ts:120-160`). Largest payloads in the system, four copies of the metadata, two of the arrays.

**Recommended change.** `onHistorySyncBatch` → `processHistorySyncBatch(batch)` returning the `messaging-history.set` payload directly; the sync-status state machine (`initialBootstrapComplete`, paused timer) takes that payload. Remove the `history_sync` object-event arm.

**LOC:** −80. **Perf:** one object-graph copy fewer per batch. **Risk:** low. **Behaviour-preserving:** yes.

### 5. Three proto layers: what is load-bearing, what is not

**Evidence.**
1. Bridge: ts-proto codec + `proto` namespace (`@oxidezap/whatsapp-rust-bridge/proto-types`, 19,596-line d.ts).
2. `Compatibility/proto-runtime.ts` (898): builds protobufjs-shaped constructors over all 498 codecs — `fromObject/toObject/create/toJSON`, prototype defaults, oneof accessors, `LongBinaryReader`, the `repairMessage` copy-on-write path, lazily. Driven by `WAProto/compatibility-schema.ts` (123 KB generated with **protobufjs** from upstream's `.proto`).
3. `WAProto/index.d.ts` (14,019 lines, 790 KB): a verbatim copy of `node_modules/baileys/WAProto/index.d.ts` (`waproto-facade.ts:7-20`), swapped in as `lib/WAProto/runtime.d.ts` at build (`--copy-build`). It imports `protobufjs` types, which is the only reason `protobufjs` is a **runtime** dependency (zero runtime imports in `src/`; `grep` confirms).

Layer 2 is the genuine minimal facade: `Exact<typeof localProto, typeof upstreamProto>` (`scripts/compatibility/type-contracts.ts:56`) and the `instanceof`/defaults/`toJSON` semantics need it. Layer 3 is the cheapest way to get exact types but ships 790 KB and drags `protobufjs` into `dependencies`.

**Recommended change.** (a) Bridge exports the compact field schema (name, kind, ref, flags, oneof) it already has in its generator (`scripts/gen-protobufjs-dts.ts`) as `proto-schema.json`; baileyrs deletes `compatibility-schema.ts` and the protobufjs-based generator half of `waproto-facade.ts` (−~100 lines script, −123 KB generated). (b) Move `protobufjs` to `peerDependenciesMeta.optional` or replace the `$protobuf.Writer/Reader` references in the vendored d.ts with a 20-line local `ProtoWriter/ProtoReader` interface during `--sync` (a regex over the header), then drop the dependency. (c) Bridge decode hook `decode(reader, len, { onMessage })` or a "decode onto prototype" option so `hydrate`'s full-tree re-parenting per decode (`proto-runtime.ts:~820-850`) becomes unnecessary — that walk is paid on every inbound message.

**LOC:** −~150 source, −123 KB generated, one runtime dep fewer. **Risk:** low for (a)/(b), medium for (c). **Behaviour-preserving:** yes.

### 6. One `forward()` helper for the 119 `(await ctx.getClient()).x(...)` pass-throughs

**Evidence.** `Socket/privacy.ts:33-72` is nine methods of the form `assertArgumentDomain(name,'value',value,SET); await (await ctx.getClient()).updatePrivacySetting('last', value)`. `profile.ts`, `presence.ts`, `prekeys.ts`, `blocking.ts`, `contacts.ts` are entirely this shape. `newsletter.ts` has 14 of 21 methods as `bridgeNewsletterMetadataToBaileys(await (await ctx.getClient()).x(...))`. `groups.ts` 12 of 16.

**Recommended change.**
```ts
const forward = <K extends keyof WasmWhatsAppClient, R = ReturnType<WasmWhatsAppClient[K]>>(
  ctx, method: K, opts?: { map?: (r) => R; check?: [param, index, domain] }
) => async (...args) => { …assert…; return map((await ctx.getClient())[method](...args)) }
```
and tables for the uniform files, e.g. `PRIVACY = { updateLastSeenPrivacy: ['last', WA_PRIVACY_VALUES], … }`.

**LOC:** −250 to −300. **Risk:** low (the `assertArgumentDomain`-before-first-await stack-frame property must be kept: run the check synchronously inside the returned function before awaiting). **Behaviour-preserving:** yes.

### 7. `Socket/index.ts` (1,032 lines) mixes lifecycle, init, and ~25 feature methods

**Evidence.** Lifecycle/teardown/logout: `index.ts:140-330, 486-690`. Init: `:363-470`. Inline feature methods that belong in existing factories: `sendNode/assertSessions/getUSyncDevices/sendRawMessage/createParticipantNodes` (`:866-905`), `sendPresenceUpdate` (`:918-935`), `waUploadToServer` (`:942-948`), `updateDefaultDisappearingMode/rejectCall/fetchReachoutTimelock/getBusinessProfile/fetchMessageHistory/sendStatusMessage` (`:950-997`), `downloadMedia` (`:1010-1020`), `query/waitForMessage/waitForConnectionUpdate` (`:692-745`). Five unused mutexes (`:296-300`) exist only for API parity.

**Recommended change.** `socket-lifecycle.ts` (owner + teardown + end/logout + terminal-close wiring), `socket-init.ts`, and move the inline methods into `messages.ts`/`presence.ts`/`business.ts`/`server-queries.ts`. `index.ts` becomes ~250 lines of composition.

**LOC:** net ~0 (moves), but each file becomes reviewable. **Risk:** low. **Behaviour-preserving:** yes.

### 8. ~820 lines of exported-but-never-wired Signal/retry machinery

**Evidence.** The socket sets `messageRetryManager: null` (`internals.ts:170`), yet `Compatibility/public-api/message-retry-manager.ts` (236) + `internal/expiring-lru-map.ts` (82) implement the full upstream class. `addTransactionCapability`/`makeCacheableSignalKeyStore` (`auth-utils.ts`, 224) + `PreKeyManager` (99) + `async-primitives.ts` (44) + `cache-store.ts` (21) are only reached via `getExposedKeys()` when a consumer reads `sock.authState.keys` or passes a custom `makeSignalRepository` (`index.ts:91, 288-292`). `identity-change-handler.ts` (67): the engine handles identity changes and the bridge event is a `noop` (`schema.ts:590`). `Utils/signal.ts` (154): `extractE2ESessionFromRetryReceipt`, `parseAndInjectE2ESessions`, `xmppPreKey`, `extractDeviceJids` — stanza-level Signal plumbing the engine owns (`assertSessions`, `getUSyncDevices`); no socket caller. `messages-media.ts:340-429`: media-retry HKDF/AES-GCM in JS while the socket uses `client.requestMediaReupload` (`messages.ts:163`).

**Recommended change.** Keep the exports (declaration audit demands them) but (a) move them under a single `src/Compatibility/standalone/` with a README stating "not used by the socket", (b) make `getExposedKeys` construct the transaction facade only on first `authState.keys` access — already lazy, fine — and (c) drop `ExpiringLruMap` + `makeStandaloneCacheStore` in favour of one class. If the audit tolerates it, mark `parseAndInjectE2ESessions`/`extractE2ESessionFromRetryReceipt` deprecated and delete in the next major.

**LOC:** −100 now (LRU/cache-store/mutex consolidation), −820 if the exports can go. **Risk:** low / API-visible. **Behaviour-preserving:** yes / no.

### 9. Legacy-store projection re-implements the core's persisted byte formats in TypeScript

**Evidence.** `legacy-store/device.ts:98-127` hand-serializes the core's `Device` JSON (`noise_key` as a 64-number array = private‖public, `app_version_primary`, `edge_routing_info`, …); `codecs/basic.ts` does the same for `sync_key`, `sync_version` (LTHash, `hash: number[128]`), `tc_token`, `device_list`, `lid_mapping` JSON shapes; `prekey` uses `proto.PreKeyRecordStructure`. For sessions and sender keys the bridge *already* exposes neutral codecs (`importLegacySessionRecordV1`, `projectLegacySessionRecordV1`, `decodeSenderKeyRecordComponents` — `codecs/signal.ts:2-11`), which is the right pattern. Total legacy-store: ~1,960 lines (`adapter` 335, `routing` 214, `constants` 225, `common` 142, `device` 233, `codecs` 547, `multi-file` 93, `validation` 53, `native-projection` 125).

**Recommended change.** Bridge exposes `encodeDeviceRecord(fields)/decodeDeviceRecord(bytes)` and the equivalent pair for the five JSON stores (typed with tsify, same as the session helpers). `device.ts` and `codecs/basic.ts` reduce to field mapping between Baileys creds and the bridge's typed record.

**LOC:** −350 in baileyrs; bridge +~150. **Risk:** medium (auth migration is high-stakes; the existing `wrap-legacy-store-*.test.ts` suite — 2,200 lines — covers it). **Behaviour-preserving:** yes.

### 10. `package.json`: `exports` makes every internal module public; two unused/misplaced deps; a second TypeScript

**Evidence.** `exports["./lib/*"]` and `["./lib/*.js"]` (`package.json:33-40`) expose `lib/Bridge/schema.js`, `lib/Compatibility/**`, `lib/Socket/**` as importable entry points, so every refactor above is semver-visible. `@bufbuild/protobuf` (devDependency): zero references anywhere. `protobufjs` (dependency): used only by `scripts/compatibility/waproto-facade.ts` and as a types import in the vendored d.ts (Finding 5). `typescript-compat-auditor: npm:typescript@6.0.3`: a second full TypeScript pinned solely for `audit-core.ts` because TS 7 has no checker API — worth a comment in `package.json` and a plan (e.g. `ts-morph`-free structural diff on emitted `.d.ts` text, or run the auditor under the same TS once 7.x exposes one).

**Recommended change.** Narrow `exports` to `.`, `./logger`, `./lib/Utils/*`, `./lib/Types/*`, `./lib/WAProto/*`, `./lib/WABinary/*`, `./lib/WAUSync/*` (the upstream deep-import surface) and keep `Bridge/Compatibility/Socket` internal. Remove `@bufbuild/protobuf`. Move `protobufjs` per Finding 5.

**LOC:** −3 lines, −1 runtime dep, −1 unused dep. **Risk:** low (deep imports of internals are unsupported upstream too). **Behaviour-preserving:** API-visible for anyone deep-importing internals.

### 11. Tooling under `scripts/compatibility` overlaps tests and itself

**Evidence.**
- `audit-core.ts` (1,043, TS-checker structural diff) and `type-contracts.ts` (301, `Exact<>` type asserts) both answer "do our declarations match upstream". The type-asserts file is the cheaper, always-on one; the auditor adds coverage percentages and a CLI.
- `proto-runtime-audit.ts` (245) vs `src/__fuzz__/proto-codec.fuzz.test.ts` (1,293) targets `proto:type-coverage`, `proto:field-names`, `proto:field-numbers` vs `src/__tests__/proto-runtime-compatibility.test.ts`. The audit script's `KNOWN_WIRE_GAPS` (`proto-runtime-audit.ts:44-64`) duplicates `NOT_ENCODED_FIELDS` + `RENAMED_PROTO_FIELDS` in `src/__fuzz__/harness/divergence.ts:68-99` — two allowlists for the same twelve gaps.
- `lifecycle-contract-core.ts` (780) + tests (364 + 153 e2e) state invariants LC-001…; `auto-reconnect-terminal-close.test.ts` (421) and `socket-dispose-integration.test.ts` (692) test the same promises directly against the dispatcher.
- `check-layer-boundaries.ts` (148) greps sibling checkouts `../whatsapp-rust` and `../whatsapp-rust-bridge` for the word "baileys" — a cross-repo lint that only runs in a monorepo-style layout and belongs in those repos' CI.

**Recommended change.** One allowlist: make `proto-runtime-audit.ts` import the divergence registry. Fold `proto-runtime-audit` into the fuzz `proto:*` targets (they already iterate the same schema). Keep `audit-core.ts` (it is the drop-in claim) and delete `type-contracts.ts` only if the auditor covers `Exact<typeof proto>` — otherwise keep both but document which is authoritative. Move `check-layer-boundaries.ts`'s core/bridge halves to those repos.

**LOC:** −300 to −500. **Risk:** low. **Behaviour-preserving:** n/a.

### 12. Test harness duplication: seven `makeHarness`, five `makeCtx`

**Evidence.** `newsletter-compatibility.test.ts:46-70`, `server-queries-compatibility.test.ts:32-50`, `business-compatibility.test.ts:37`, `privacy-compatibility.test.ts:30`, `socket-internals-compatibility.test.ts:38`, `chat-modify-compatibility.test.ts:41` each define the same Proxy-client-recording-calls harness (verbatim: `new Proxy({}, { get: (_t, prop) => { if (prop === 'then') return undefined; … calls.push([prop,args]) } })` + `{ ev: new EventEmitter(), logger, getClient: async () => client } as SocketContext`). `regressions.test.ts:41`, `group-participant-stub.test.ts:35`, `rate-limited-stream-error.test.ts:51`, `quoted-media-enum-regression.test.ts:24`, `auto-reconnect-terminal-close.test.ts:45` each build a full stub `SocketContext`.

**Recommended change.** `src/__tests__/helpers/socket-harness.ts` exporting `makeRecordingClient(overrides, defaultResult)` and `makeSocketContext(partial)`; `collect/collectMany` from `regressions.test.ts:62-85` also belong there.

**LOC:** −200. **Risk:** none. **Behaviour-preserving:** yes.

### 13. `__fuzz__/harness/divergence.ts` (1,796) is 1,000 lines of comparison logic wearing a registry's name

**Evidence.** Only 25 `KnownDivergence` entries (`divergence.ts:1062+`); lines 68–1060 are `undoRenames`, `DECODE_OMITTED_PATHS` rules, `KNOWN_OMITTED_FIELDS`, `CLEANED_JID_FIELDS`, `MERGE_PRECEDENCE_*`, surrogate/ASCII normalisation, `FLT_MAX` handling — matcher machinery that duplicates the role of `harness/compare.ts` (398). `harness/__tests__/harness.test.ts` (1,098) tests the harness itself.

**Recommended change.** Move matcher/normalisation code into `compare.ts` (or `normalise.ts`); leave `divergence.ts` as data + the `when` predicates that reference it. No fewer lines, but the registry becomes readable and the review-date policy auditable.

**LOC:** ~0 (moves). **Risk:** none.

### 14. Duplicated primitives across layers

**Evidence.** `toNumber` (`Utils/generics.ts:80-100`) and `asInt64` (`Bridge/primitives.ts:150-167`) both reconstruct `high * 2^32 + (low >>> 0)`; `jidStr` (`Socket/types.ts:30`) = `bridgeJidToString` (`primitives.ts:53`); `isObject` in `primitives.ts:15` and `proto-runtime.ts:84` plus `asRecord` in `Socket/message-capping.ts:3`; `mapReachoutTimelock` payload-unwrapping (`reachout.ts:20-40`) and `extractMessageCappingPayload` (`message-capping.ts:8-25`) implement the same "MEX wrapper or bare payload" search; `AsyncMutex` + `makeMutex` + `makeKeyedMutex` + `SerialQueue` (three files) for five mutexes the socket never uses.

**Recommended change.** One `Utils/int64.ts`, one `isObject`, one `unwrapMexPayload(value, key)`; delete `jidStr`.

**LOC:** −80. **Risk:** none. **Behaviour-preserving:** yes.

### 15. `Compatibility/` classification (what is shim vs logic)

Of ~5,600 non-test lines:

| Group | Files | Lines | Verdict |
|---|---|---|---|
| Pure shape shims (renames/aliases) | `socket-results`, `group-metadata`, `newsletter-results`, `media-type`, `message-keys`, `message-upsert`, `stanza-responses`, `group-stub-params`, `participating-refresh`, `all-encryptions-failed`, `derived-stanza-nodes`, `encode-proto`, `enum-types`, `internal/numeric-enum`, `internal/native-memory-store` | ~800 | Needed; shrink with Finding 6 tables. `derived-stanza-nodes` and `all-encryptions-failed` exist to reverse bridge-side refusals — bridge could return `{ droppedNodes: ['biz'] }` instead of rejecting, deleting both (−130). |
| Baileys semantics (real logic) | `group-notifications` (398), `usync/adapter` (325), `message-relay` (114), `websocket-client` (183), `tagged-message-waiter` (49), `signal-repository` (165) | ~1,230 | Needed. |
| Proto facade | `proto-runtime` (898) | 898 | Needed (Finding 5). |
| Legacy store | `legacy-store/**` | ~1,960 | Needed; shrink per Finding 9. |
| Standalone, unused by socket | `public-api/message-retry-manager`, `auth-utils`, `pre-key-manager`, `identity-change-handler`, `stanza-ack`, `internal/expiring-lru-map`, `async-primitives`, `cache-store`, `make-mutex` | ~820 | API-parity only (Finding 8). |

Roughly 15% shim, 40% logic, 30% legacy store, 15% dead-for-the-socket.

---

## Quick wins (each ≤ 1 hour, behaviour-preserving unless noted)

1. **Avoid copying every uploaded Buffer.** `Socket/index.ts:943` `data instanceof Uint8Array && !Buffer.isBuffer(data) ? data : new Uint8Array(data)` copies every `Buffer` upload. Use `new Uint8Array(data.buffer, data.byteOffset, data.byteLength)` (a view) if the intent is to shed the `Buffer` subclass; same for `sendRawMessage` at `:875`.
2. **Avoid copying every downloaded media.** `Utils/messages.ts:1032` `Buffer.from(data)` copies the whole download; `Buffer.from(data.buffer, data.byteOffset, data.byteLength)` is zero-copy.
3. **`useBridgeStore.set` equality check copies both sides.** `use-bridge-store.ts:227` `Buffer.from(prev).equals(Buffer.from(value))`; `setMany` at `:296` already uses `prev.length === value.length && Buffer.compare(prev, value) === 0`. Use the same, and factor `set`/`setMany` and `get`/`getMany` bodies into `putOne`/`getOne` closures (−60 lines; the two pairs are copy-pasted).
4. **Remove `@bufbuild/protobuf`** from devDependencies (unused).
5. **Delete `adaptMessage`/`adaptMessageParts`** (`schema.ts:718-790`) and the fixtures that only exercise them — the bridge never emits object `message` events (Finding 3, first half).
6. **Merge `toNumber`/`asInt64`, `jidStr`/`bridgeJidToString`, `isObject`×3** (Finding 14).
7. **Single MEX payload unwrapper** for `reachout.ts` and `message-capping.ts`.
8. **Privacy table.** Replace `privacy.ts:33-72` nine methods with a 9-row table + one generator (−45 lines).
9. **Shared test harness** for the seven `makeHarness` copies (−200 lines).
10. **One proto-gap allowlist.** `proto-runtime-audit.ts:44-64` should import `NOT_ENCODED_FIELDS`/`RENAMED_PROTO_FIELDS` from `divergence.ts`.
11. **Drop the five unused mutexes** from the socket object (`index.ts:296-300, 855-859`) or build them lazily via getters — they are constructed per socket for API parity only. (API-visible only if a consumer reads them before use; getters keep that working.)
12. **`Object.keys(ADAPTERS)` set is built once — fine — but `KNOWN_BRIDGE_EVENT_TYPES` is exported from two modules** (`adapt.ts:27`, `schema.ts:702`); keep one.
13. **`Example/example.ts` `contacts.upsert` logs `message-receipt.update`** (`:409-410`) — copy-paste bug in the example.

## What the bridge would need to add (summary of "bridge should expose X so baileyrs can delete Y")

| Bridge addition | baileyrs deletion |
|---|---|
| Events serialized camelCase, JIDs as strings, timestamps as unix seconds, durations as seconds, `ReceiptType` lowercased, action types exported (Finding 1) | `Bridge/types.ts`, ~80% of `Bridge/schema.ts`, ~80% of `Bridge/primitives.ts`, most of `Bridge/__tests__/adapt.test.ts` |
| `run(): Promise<TerminalReason>` or `onTerminated` callback (Finding 2) | `Socket/terminal-close.ts`, `isAutoReconnectEnabled` plumbing, the `disconnected`/`streamError` special cases, most of `terminal-close-reporter.ts`, `logout()` claim counting |
| Compact proto field schema export; optional decode-onto-prototype hook (Finding 5) | `WAProto/compatibility-schema.ts` (123 KB), protobufjs from `dependencies`, `hydrate` tree copy per message |
| Typed encode/decode for the device, sync_key, sync_version, tc_token, device_list, lid_mapping records — same pattern as `importLegacySessionRecordV1` (Finding 9) | `legacy-store/device.ts` format code, `legacy-store/codecs/basic.ts` |
| `relayMessage*` returning `{ id, droppedDerivedNodes }` instead of rejecting on a derived-node conflict; `no-recipient-device` as a typed result (Finding 15) | `Compatibility/derived-stanza-nodes.ts`, `all-encryptions-failed.ts` |
| A `history_sync` batch that carries `syncType/chunkOrder/progress/batchIndex` as typed fields (it does) — no bridge change; baileyrs stops building a fake event (Finding 4) | `history-sync-wire.ts` synthetic event, `history_sync` adapter |

Estimated total if all findings land: **−4,000 to −4,500 lines of baileyrs source** (~20%), −900 KB of shipped declarations/schema, one runtime dependency fewer, and the message hot path drops from three intermediate allocations per message to one.
