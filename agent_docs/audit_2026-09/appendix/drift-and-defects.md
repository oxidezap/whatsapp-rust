# Contract drift and boundary defects: whatsapp-rust → whatsapp-rust-bridge → baileyrs

Scope note. baileyrs pins `@oxidezap/whatsapp-rust-bridge` `0.19.0` (`baileyrs/package.json`) and the bridge checkout is `0.19.0` (`whatsapp-rust-bridge/package.json`), so the comparison below is against the same version. `baileyrs/node_modules` is not installed and `whatsapp-rust-bridge/pkg` is not built, so "the bridge `.d.ts`" below means its sources: the `typescript_custom_section`s in `src/wasm_client.rs` plus the generated `src/generated_types.rs`.

---

## 1. Drift tables (what disagrees today)

### 1a. Event catalogue

| Layer | Count | Source |
|---|---|---|
| Core `Event` variants | 71 | `whatsapp-rust/wacore/src/types/events.rs:218-292` (`EventKind`, index-stable), enum body `:873-1093` |
| Bridge event `type` strings | 67 | `whatsapp-rust-bridge/src/wasm_client.rs:196-292` (36 `serialize` + 16 `serialize_with_proto` + 15 `special`) |
| Bridge deliberately undispatched | 4 | `wasm_client.rs:296-323`: `DecryptedPayload`, `SentFrame`, `RetiredPushNameUpdate`, `EncDecryptFailed` (37+16+15+4 = 71, and a `#[cfg(test)]` list pins it) |
| baileyrs `AdapterMap` keys | 67 | `baileyrs/src/Bridge/schema.ts:72` (`{[K in WhatsAppEvent['type']]}` — totality is a compile error), table `:117-654` |
| baileyrs `DISPATCHERS` keys | 45 canonical types | `baileyrs/src/Socket/events.ts:377` |

**Core → bridge.** Set difference is exactly the 4 undispatched variants; no accidental gap. Two more are lost *at runtime* rather than by catalogue: `Messages` is dropped with an error log if the host has no `onMessageBatch` (`wasm_client.rs:1350-1362`), `HistorySync` if no `onHistorySyncBatch` (`:1333-1336`); `Receipt`/`ServerAck` fall back to the object path when the packed callback is absent (`:1368-1391`). The union entries `message` and `history_sync` describe a host-side reconstruction; the bridge never emits them as objects (`:277-281`).

**Bridge → baileyrs.** The adapter table is total (TS-enforced), so the drift is in what baileyrs *does* with an event. 16 adapters are unconditional `noop` (`schema.ts`, lines as listed) and `noop` is dispatched as a trace log only (`events.ts:943-947`):

| bridge type (schema.ts line) | Baileys upstream event that should be fed (`baileyrs/src/Types/Events.ts`) |
|---|---|
| `self_push_name_updated` (:326) | **`creds.update`** — upstream `processSyncAction(pushNameSetting)` emits `{ me: { …, name } }`. Real gap. |
| `user_about_update` (:395) | **`contacts.update`** with `status` — upstream `handleNotification` status branch. Real gap. |
| `contact_removed` (:256) | `contacts.update`? upstream has no removal event; none required |
| `device_list_update` (:338), `identity_change` (:339) | none in `BaileysEventMap` (upstream refreshes sessions internally) |
| `pairing_code_refresh` (:142), `pair_passkey_request/confirmation/error` (:147-157) | none upstream |
| `quick_reply_update` (:257), `call_log_sync` (:258), `client_expiration_changed` (:327), `business_status_update` (:351), `contact_sync_requested` (:394), `user_status_mute_update` (:409), `offline_sync_preview` (:332) | none upstream (`offline_preview` is logged only upstream too) |

Conditional noops (payload-gated `return { type:'noop' }`): `label_edit_update` (:143), `label_association_update` (:160), `message_label_association_update` (:174), `disappearing_mode_changed` (:343), `newsletter_live_update` (:354), `contact_number_changed` (:391), `delete_chat_update` (:398), `clear_chat_update` (:405), `delete_message_for_me_update` (:413), `notification` (:425). `notification` canonical is itself a trace log (`events.ts:895-896`) — generic `<notification>` never reaches a Baileys event except through `rawNode` CB events.

Baileys upstream events in `Types/Events.ts` that baileyrs never emits (no `emit('…')` outside tests): `blocklist.set`, `blocklist.update`, `newsletter.view`, `newsletter-participants.update`, `newsletter-settings.update`, `chats.lock`, `messages.media-update`.

### 1b. Error kinds

Bridge: 11 kinds, `whatsapp-rust-bridge/src/errors.rs:51-131`. baileyrs reads `.kind` in exactly three places:

- `Socket/bridge-error-boundary.ts:39` — only checks `typeof kind === 'string'` to decide whether to rebuild the stack.
- `Compatibility/derived-stanza-nodes.ts:39` — `'invalid-argument'` (and then greps `reason`/`message` text).
- `Compatibility/all-encryptions-failed.ts:19,36` — `'no-recipient-device'`.

**Never handled by kind: `server`, `timeout`, `not-connected`, `withdrawn`, `disconnected`, `protocol-violation`, `crypto`, `storage`, `internal` (9 of 11).** None is translated into a `Boom`; the per-variant fields `serverCode`, `serverText`, `errorType`, `backoffSeconds`, `operation`, `attempted` are read nowhere in baileyrs. An upstream-style handler doing `err.output.statusCode` on a bridge rejection gets `undefined`. `withdrawn` is especially notable: baileyrs's `Socket/index.ts` never calls `withdrawParkedCalls` (grep at `:228-268,905-909` shows only `setAutoReconnect`/`free`), so parked calls are only ever released by teardown.

`DisconnectReason` (`baileyrs/src/Types/index.ts:34-45`) producibility, with the core's routing (`whatsapp-rust/src/client/node_io.rs:1990-2060`; `ConnectFailureReason` wire codes `wacore/src/types/events.rs:1653-1685`):

| value | produced at | reachable? |
|---|---|---|
| `connectionClosed` 428 | `events.ts:346,395` + Boom fallbacks in `index.ts` | yes |
| `loggedOut` 401 | `events.ts:330,428`, `index.ts:653` | yes |
| `connectionReplaced` 440 | `events.ts:497` | yes |
| `forbidden` 403 | `events.ts:332` (402), `:512` (temporaryBan) | yes (via `temporaryBan`) |
| `timedOut` 408 | `events.ts:341,847` | only via `qrCodesExhausted`; the 408 arm is dead — the core has no 408 `ConnectFailureReason` |
| `unavailableService` 503 | `events.ts:339` | only when `setAutoReconnect(false)`: 500/503 otherwise go to `emitRetrying` (`events.ts:438-445`) |
| `restartRequired` 515 | `events.ts:343` | **never**: the core has no 515 reason; 515 is consumed internally as an expected reconnect |
| `multideviceMismatch` 411 | `events.ts:336` | **never**: no 411 reason in the core |
| `badSession` 500 | only the legacy helper `Utils/generics.ts:374` | **never** from the dispatcher (deliberate, `events.ts:451-455, 498-503`) |
| `connectionLost` 408 | — | **never** by name (same numeric value as `timedOut`, so indistinguishable anyway) |

Also emitted but not a `DisconnectReason` member: `CLIENT_OUTDATED_STATUS = 405` (`events.ts:312, 505`).

### 1c. The two `IqError` enums

| variant | `wacore/src/request.rs:84-105` | `src/request.rs:140-178` |
|---|---|---|
| `Timeout`, `NotConnected`, `Disconnected(Box<Node>)`, `UnexpectedResponseType{got}`, `InternalChannelClosed` | yes | yes (copied) |
| `ServerError{code,text,error_type,backoff}` | yes | yes **plus `response: RejectionStanza`** (`:166`) |
| `Socket`, `EncryptSend`, `ClientState(Box<ClientError>)`, `DuplicateRequestId`, `EncodeError`, `ParseError` | — | yes |

Producers: the wacore enum is produced only by response classification in wacore (`wacore/src/request.rs:125-130,259,286,293,421,446,478,492`, `wacore/src/iq/bot.rs:711`); the `src` enum by the transport/pipeline (`src/request.rs:198-209,454-640`, `src/keepalive.rs`, feature modules). Conversion `IqError::from_response` (`src/request.rs:214-243`) copies the 6 shared variants and has a wildcard `_ => Self::InternalChannelClosed` (`:240-242`) because the wacore enum is `#[non_exhaustive]` — a new wacore variant silently becomes "channel closed".

Verdict: the *split* is real (wacore has no transport, so `Socket`/`EncryptSend`/`ClientState` cannot live there; `RejectionStanza` needs the owned node type), but the 6 shared variants are an accidental duplication: `is_transport_unavailable`/`is_timeout` are implemented twice (`wacore:112-135`, `src:181-210`), and the bridge maintains two separate mapping arms for the same variants (`whatsapp-rust-bridge/src/errors.rs:301-332` and `:378-382`). A `src` enum wrapping the wacore one (`Core(wacore::IqError)` + `response`) would remove both copies and the lossy wildcard.

### 1d. Group actions per hop

| hop | count | evidence |
|---|---|---|
| Core `GroupNotificationAction` | 43 named + `Unknown{tag}` fallback = 44 | `wacore/src/stanza/groups.rs:174-404` |
| Bridge TS union | 43 | `src/generated_types.rs:655-724` — the `#[wire_fallback] Unknown` variant is **not** in the union |
| baileyrs `CanonicalGroupAction` | 44 (incl. `unknown`) | `src/Bridge/types.ts:282+`; `adaptGroupAction` default → `unknown` (`schema.ts:1149-1150`) |
| Baileys events | 5 + 9 + 3 | see below |

At the baileyrs → Baileys hop (`src/Compatibility/group-notifications.ts:135-191`, `PARTICIPANT_ACTIONS` `src/Types/GroupMetadata.ts:10`):
- `group-participants.update` ← add, remove, promote, demote, modify
- `groups.update` ← subject, description, locked, unlocked, announce, notAnnounce, invite, membershipApprovalMode, memberAddMode
- `group.join-request` ← membershipApprovalRequest, createdMembershipRequests, revokedMembershipRequests
- `chats.update` ← subject, description, ephemeral (`:194-213`); `groups.upsert` ← create (`events.ts:723-751`); stub `messages.upsert` for the same set plus ephemeral (`group-notifications.ts:215-330`)

**Lost at the last hop (no Baileys event at all, `default: return null` at `:181-184`):** delete, link, unlink, linkedGroupPromote, linkedGroupDemote, suspended, unsuspended, growthLocked, growthUnlocked, revokeInvite, noFrequentlyForwarded, frequentlyForwardedOk, autoAddDisabled, capiHostedGroup, groupSafetyCheck, limitSharingEnabled, allowAdminReports, notAllowAdminReports, reports, allowNonAdminSubGroupCreation, notAllowNonAdminSubGroupCreation, createdSubGroupSuggestion, revokedSubGroupSuggestions, changeNumber, unknown — 25 of 44. Nothing is lost core→bridge→canonical except the missing `unknown` union member (a typing gap, not a runtime one).

### 1e. Bridge `.d.ts` vs baileyrs `Bridge/types.ts`

`Bridge/types.ts` is the *canonical* (post-adapter) shape, not a mirror, so the drift shows up as stale assumptions inside `schema.ts`:

| item | bridge declares | baileyrs assumes | effect |
|---|---|---|---|
| `ReceiptType` | PascalCase string union `\| { "Other": string }` (`generated_types.rs:1250-1264`) | comment says `.d.ts` advertises `{type:"delivered"}` (`schema.ts:834-838`); map accepts both spellings | `{ Other: "x" }` hits `parseReceiptType` looking for `raw.type` → `undefined`, not `'other'` (`schema.ts:869-871`) |
| `pair_success.id/lid` | `string` (`wasm_client.rs:274`) | comment says "typed as `Jid`" (`schema.ts:160-162`) | stale comment; both accepted |
| `Jid` | object interface `{user,server,agent,…}` (`generated_types.rs:10-21`) in serde events, string in `special` events | `asJidString` accepts both | none |
| timestamps | mixed: `Receipt.timestamp: string` RFC3339 (`:1244`), `PictureUpdate`, `ContactNumberChanged`, `ServerAck`, `DeleteChatUpdate` strings; `IncomingCall.timestamp: number`, `MissedCall`, `DisappearingModeChanged.setting_timestamp: number`; packed batches `f64` seconds (`wire_batch.rs:573,1123,1189`) | `toUnixSeconds` accepts both, returns **0** on an unparsable string (`primitives.ts:118-122`) | a malformed timestamp becomes the epoch silently |
| `TemporaryBan.expire` | `number` (`generated_types.rs:1356-1358`), semantically a *duration* (`events.rs:1633-1641`) | converted to absolute at `schema.ts:123-131` | ok, but the `.d.ts` does not say "duration" |
| `WhatsAppError` fields | `field`, `reason`, `serverCode`, `serverText`, `errorType`, `backoffSeconds`, `operation`, `attempted` | `BridgeRejection { kind?, reason?, message? }` (`derived-stanza-nodes.ts:21-25`) | only `kind`/`reason`/`message` ever read |

### 1f. `TSIFY_STRUCTS`

`whatsapp-rust-bridge/codegen/src/main.rs:669-676` lists `BusinessProfile`, `BusinessCategory`, `BusinessHours`, `BusinessHoursConfig`, `GroupMetadataParticipant`, `MembershipRequest`. Actual `result_types.rs` names: `BusinessProfileResult` (:811), `BusinessCategoryResult` (:827), `BusinessHoursResult` (:836), `BusinessHoursConfigResult` (:847), `MembershipRequestResult` (:766); only `GroupMetadataParticipant` (:543) matches. **5 stale entries confirmed.** Consequence: the skip fires on the *core* structs of those names (`wacore/src/iq/business.rs:65,81,90,131`, `wacore/src/iq/groups.rs:3080`), which are therefore absent from `generated_types.rs`. Nothing in the generated surface references them today (no dangling name; the generator would fail at `main.rs:1922`), so nothing is missing from the `.d.ts` — but the comment "already generated by Tsify derives" (`:666-667`) is false, and a future core event embedding `BusinessProfile` would fail generation with a misleading cause.

### 1g. `proto-types.d.ts` vs `WAProto/index.d.ts`

Bridge `ts/proto-types.d.ts`: 1637 interface/class/enum names; baileyrs `src/WAProto/index.d.ts` (facade generated from upstream): 1127. Only-in-bridge: **512** (newer schema — `AIMediaCollectionMessage`, `BotAgentMetadata`, `BizBroadcastInsights*`, …). Only-in-baileyrs: **2** — `BotAvatarMetadata` + `IBotAvatarMetadata` (`WAProto/index.d.ts:942,950`), removed upstream. Usage: of the 204 `proto.X` names referenced in baileyrs code, none is bridge-only; `BotAvatarMetadata` appears only in fuzz tests, which already expect "unknown proto type" (`src/__fuzz__/harness/__tests__/harness.test.ts:468`).

---

## 2. Defects at the boundaries (ranked)

### D1 — HIGH — `use-bridge-store` marks a value cached *before* the critical write, so a failed Signal write is never retried and the core's re-flush is reported as success
- `baileyrs/src/Utils/use-bridge-store.ts:231` `touchCache(cacheKey, value)` then `:242 await writeCritical(...)`; same in `setMany` (`:302` before `:317`). `set` skips when equal to cache (`:226-229`, `:298-300`).
- Repro: fill the disk; send one DM whose sender chain crosses a reservation boundary → core `persist_signal_state_pre_wire` calls `set('session', …)` → `writeFile` throws ENOSPC → the core keeps the gate and aborts the send (correct). Free space; send again → the core re-flushes the *same bytes* → `set` finds them equal to the cache → returns without writing → the core releases the lease gate and publishes ciphertext under a reservation that never reached disk. Kill the process; on restart the store has the old (or no) record, so the fast-forward-to-ceiling rule in `agent_docs/signal_durability.md` ("A newly raised lease reaches durable storage before any ciphertext…") is violated and a counter can be republished.
- Fix (baileyrs): touch the cache only after `writeCritical` resolves; on throw, `cache.delete(cacheKey)`.

### D2 — HIGH — critical writes are not crash-durable, and non-critical write errors are swallowed
- `writeCritical` is a plain in-place `writeFile` with no fsync and no tmp+rename (`use-bridge-store.ts:82-89`); `flushWrite` swallows *every* error, not just ENOENT (`:100-104`), for `msg_secret`, `sent_message`, `app_state`, … `flushAll` (`:132-148`) therefore cannot report them to `end()`'s `flushStores`.
- Repro: SIGKILL during a session write → truncated file → the core reads an undecodable row and reports it *absent* (`signal_durability.md`, "An undecodable session row is reported absent") → the peer's next message is undecryptable until the retry path rebuilds the session. Repro 2: ENOSPC while a history sync writes 20k message secrets → nothing logged, secrets gone, later poll votes/reactions fail to decrypt.
- Fix (baileyrs): write to `<path>.tmp`, `fsync`, `rename`; make `flushWrite` propagate non-ENOENT errors into `flushAll`.

### D3 — HIGH — with `setAutoReconnect(false)` an *expected* disconnect ends the run loop with no event, so `connection.update { close }` is never emitted
- `whatsapp-rust/src/client/lifecycle.rs:965-1003`: `Event::Disconnected` is dispatched only for an *unexpected* exit; `:757-761` then breaks the loop on `!enable_auto_reconnect` silently. Post-pairing 515 sets `expected_disconnect` (`:2480-2493` test documents it); `client.reconnect()` sets `intentional_reconnect` (`:961`).
- Repro: `sock.setAutoReconnect(false)`; scan a fresh QR → `pair_success` → server 515 → core exits at `:759` ("Auto-reconnect disabled, shutting down") → bridge emits nothing (`run()` returns `void`, `terminal-close.ts:9-13`) → baileyrs stays at `connecting`; the 60 s watchdog (`terminal-close-reporter.ts:37,123-126`) never arms because nothing was claimed.
- Fix (core): dispatch a terminal event (or `Disconnected` with a `reason`) at the `:757` break; or (bridge) resolve a promise from `run()` / emit on loop exit.

### D4 — HIGH — baileyrs mirrors `enable_auto_reconnect` locally while the core clears it on its own, producing a spurious `connecting` after a terminal close, or **two `close` events**
- baileyrs decides from its own `autoReconnectEnabled` (`Socket/index.ts:297,442,905-909`); the core clears its flag itself on 402/405/non-reconnectable failures (`node_io.rs:1990-1993`, `:1820-1873`) *without* setting `expected_disconnect` (only the logged-out/replaced branches do). After `TemporaryBan`/`ClientOutdated`/`ConnectFailure` the read loop exit is classified *unexpected* before any await (`lifecycle.rs:965-990`) and `Disconnected` is dispatched (`:996-1001`). The `disconnected` dispatcher (`events.ts:393-403`) then reads the stale mirror.
- Repro A (mirror true): 402 ban → `close` claimed → `Disconnected` → `emitRetrying` → consumer sees `connecting` for an engine that is finished. Repro B (consumer called `setAutoReconnect(false)`): same 402 → `emitClose` twice → `reportAfter` claims twice (`terminal-close-reporter.ts:109-116`, once-guard is per claim) → two `connection.update { close }`.
- The comment at `events.ts:383-386` ("every terminal path marks `expected_disconnect` first") is false for these branches.
- Fix (baileyrs): in `disconnected`, ask the bridge `reachability()` (`connection.rs:241`) and treat `'finished'` as terminal instead of the mirror; make the reporter's once-guard global (`claimed` already exists) so a second terminal claim on an already-claimed socket does not publish again.

### D5 — MEDIUM — bridge event channel drops events on overflow
- `wasm_client.rs:1301-1312` `enqueue` uses `try_send` on `async_channel::bounded(EVENT_CHANNEL_CAPACITY = 16_384)` (`:648`) and only `log::warn!`s on failure. The producer runs whenever the consumer awaits: `yield_to_io()` is a macrotask (`:1078-1081`), during which the transport can deliver and decode frames.
- Repro: offline drain of >16k stanzas while the host's `onMessageBatch` is slow (a 50-callback burst then yield, `:647,1043-1060`) — messages/receipts vanish; a later receipt then refers to a message the host never saw.
- Fix (bridge): await `send()` (backpressure into the core's dispatch) or surface a `events_dropped` event with counts.

### D6 — MEDIUM — `Device` and `account` are two keys written non-atomically, and a `None` account never deletes the old key
- `js_backend.rs:1574-1587` writes `device` JSON then `account` bytes (no `setMany`); `:1589-1603` re-attaches whatever `account` bytes exist. Nothing deletes `account` when `device.account` is `None`.
- Repro: pair → logout → re-pair on the same folder; between the fresh `Device` save and the new `PairSuccess`, `load()` returns the new device with the *previous* account identity (stale `ADVSignedDeviceIdentity` presented to the server). Crash between the two writes leaves the pair out of step.
- Fix (bridge): delete `account` on `None`; write both through `js_put_many` when `setMany` exists, or embed the bytes in the JSON record.

### D7 — MEDIUM — caller input reported as `internal`
`AGENTS.md` calls this a contract violation. Confirmed sites (all `crate::errors::internal`):
- version array shape `wasm_client.rs:2188-2205`; `wantedPreKeyCount` `:2225-2228`; missing/invalid store callbacks `:2393-2403` (all `createWhatsAppClient` inputs)
- `js_to_node` tag `:3353-3355` (reached by `sendNode` `signal.rs:56-59` and every extra-nodes path); `js_node_array_to_vec` `:3544`
- undecodable message bytes `:3526`, `:3573`, `signal.rs:692` (`sendMessageBytes`, `retransmitMessageBytes`, `signalCreateParticipantNodes`)
- `retry_count` out of `u8` `messaging.rs:139`; `run()` called twice `connection.rs:23` (AGENTS.md's own `connect()` example says this is `invalid-argument` with the operation as `field`)
- server/crypto outcomes mislabelled: `MediaRetryResult::NotFound/DecryptionError/GeneralError → internal` `media.rs:430-436` (should be `server`/`crypto`)
Fix (bridge): `invalid_arg(field, …)` at each; `From<JidError>` hard-codes `field: "jid"` (`errors.rs:522-528`), so `sendMessageBytes` recipients (`:3574-3576`), `retransmitMessageBytes` (`chat_jid` + `requester_jid`, `messaging.rs:136-137`) and `signalDecryptGroupMessage` all report `"jid"` whichever was bad — needs a `parse_jid_named(field, s)`.

### D8 — MEDIUM — `IqError::from_response` wildcard collapses new wacore variants into `InternalChannelClosed`
`src/request.rs:240-242`; the bridge then maps `InternalChannelClosed` to `None`/fallthrough (`errors.rs:321-326`), so a future wacore classification error surfaces as `internal` with a "channel closed" message. Fix (core): wrap instead of copy (see 1c).

### D9 — LOW/MEDIUM — sqlite retry loops: 7 inline copies diverge from `with_retry`
`storages/sqlite-storage/src/sqlite_store.rs`: `with_retry` (`:780-830`) caps at `10·2^min(attempt,4)` ms and warns from the 2nd retry (`:815-822`). Inline copies: `put_identity_for_device` (`:1225-1279`) and `put_session_for_device` (`:1359-1414`) use `10 * 2u64.pow(attempt)` (`:1264,:1399`) and warn on *every* retry; `store_prekey` (`:2055`), `store_prekeys_batch` (`:2121`), `remove_prekey` (`:2229`), `store_signed_prekey` (`:2325`), `remove_signed_prekey` (`:2413`) use the capped formula but **never log**. Because retries only happen for `attempt < 5`, the "uncapped" formula is numerically identical today (max 160 ms; 310 ms total before the error). Observable effect under sustained `SQLITE_BUSY`: identity/session writes emit 5 warnings each; a consumed-prekey removal fails after 310 ms with **no** log line, so the pre-key-reuse hazard `signal_durability.md` describes ("consumed one-time prekey is deleted only after…") is invisible in logs. Owner: core; fold into `with_retry`. Not on the bridge path (bridge uses `js_backend`).

### D10 — LOW — `scan_expired`/`confirm_expired` TOCTOU
`js_backend.rs:504-575`, used at `:1348-1367` and `:1541-1551`: an await separates `confirm_expired` from `js_delete_many`; a `put_msg_secret` from a concurrent decrypt can re-write a victim in that gap and be deleted. The comment acknowledges it needs a compare-and-delete primitive. Effect: a secret rewritten in that window is lost; low frequency. Owner: bridge/store contract.

### D11 — LOW — `setMany` atomicity is a host promise the bridge cannot verify
`js_backend.rs:251-263` falls back to per-key writes when `setMany` is absent and returns `Err` on the first failure, so the core keeps every gate (conservative, no key reuse). With `setMany` present, `use-bridge-store.ts:336-338` runs critical writes concurrently and rejects on the first failure — partial persistence plus a rejection is safe *only* because the core re-flushes, which D1 breaks. Nothing in `JsStoreCallbacks` states that `setMany` must reject on any partial failure; a host that resolves anyway would release leases silently. Owner: bridge (document + test the contract).

### D12 — LOW — receipt/timestamp serialization inconsistencies (confirmed attributes)
- `ReceiptType` derives only `Deserialize` with `#[serde(from = "String")]` (`wacore/src/types/presence.rs:26-29`) and hand-implements `Serialize` as `serialize_unit_variant(index, variant_name)` (`:127-145`), so the object path emits `"Delivered"` while the wire attr is `"delivery"`; the packed path also uses `variant_name()` (`wire_batch.rs:716-719`) so the two bridge paths agree with each other, but `Other(s)` is `{ Other: s }` on one and a flag+string on the other (`:708,:717`) and baileyrs maps the object form to `undefined` (see 1e).
- `Receipt.timestamp: DateTime<Utc>` with no serde attribute (`events.rs:2159`) → RFC3339 string (`generated_types.rs:1244`); packed receipt → `f64` seconds (`wire_batch.rs:1123`); `IncomingCall.timestamp` → integer. Three formats for one concept; `toUnixSeconds` hides it and turns a parse failure into `0`.

### Ordering and withdrawal (checked, no defect found)
- Message→receipt order is preserved inside the bridge: the channel is FIFO, cross-kind lookahead is parked in `pending_event` (`wasm_client.rs:1433-1441`, `:1520-1531`), and an open envelope is flushed before any non-envelope callback (`:1319-1325`). The only reorder is host-side: a callback returning a thenable defers its decode, which the bridge detects after the fact and cannot undo for the batch already handed over (`:733-745`); baileyrs's handlers are synchronous (`events.ts:1229-1237`). The one way a receipt is seen before its message is the drop path in D5 / missing `onMessageBatch` (`:1360`).
- `withdrawParkedCalls` releases only `online()` waiters (`wasm_client.rs:2724-2726`, `park()` `:2748-2768`); the event consumer task is independent and unaffected. `readMessages`/`markPlayed` use `online_committed()` and are not withdrawable by design (`:2709-2716`).
- Core gate vs bridge: the bridge reads the core's `reachability()` (`:2678-2683`) rather than mirroring it, and every terminal transition notifies (`lifecycle.rs:201-204`, `:189`). The one lag: `setAutoReconnect(false)` writes the atomic directly (`connection.rs:204-209`) with no notify, so a call parked in `Reconnecting` stays parked until the backoff sleep (≤ 900 s) ends and the loop reaches `:757`; `withdrawParkedCalls` is the escape and baileyrs never calls it.

---

## 3. Conclusion — what should be generated from one source

1. The event catalogue already flows core → bridge by codegen (`generated_types.rs`), but the 67-entry `type` union in `wasm_client.rs:196-292`, the 4-entry undispatched list, and baileyrs's 16-entry noop set are three hand-kept mirrors of one enum; emit the union and the noop/Baileys-event mapping table from `EventKind`.
2. Error kinds and `DisconnectReason` are a hand-built three-way map (`ConnectFailureReason` → bridge event → `events.ts:325-348`) with three dead arms and nine unhandled kinds; generate the reason→status table from the core enum and have the bridge emit a `terminal` flag so baileyrs stops mirroring `enable_auto_reconnect` (D3/D4).
3. The group-action union is generated but the last hop (`group-notifications.ts:135-191`) is a hand switch that loses 25/44 variants; a generated `wire tag → Baileys event` table (with an explicit "no Baileys event" column) would make the loss visible in the diff instead of in `default: return null`.
