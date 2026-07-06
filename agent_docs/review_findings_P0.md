# Adversarial Review — Findings (P0 tier)

> Produced by `scripts/adversarial-review.workflow.js` (`args: { tier: "P0" }`), run `wf_7822d229-dc9`:
> 91 agents, 0 errors, ~5.5M tokens. Each candidate was found by a per-lens finder and had to survive
> 3 independent adversarial skeptics (default-refute; majority-real to be kept). **17 confirmed** of the
> raw candidates. Severities were set/corrected by the verifiers; each finding is file-anchored with a
> concrete failure scenario and cross-referenced to captured WhatsApp-Web JS where it is a compliance issue.

## Summary

| Sev | Group | File:line | Finding |
|---|---|---|---|
| **P0** | concurrency | `src/client/sessions.rs:119` | The offline-sync finisher's publish_offline_sync_live_state widens the message-processing semaphore to 64 and sets offline_sync_completed=true with... |
| **P1** | compliance | `wacore/appstate/src/processor.rs:369` | Genesis (had_no_prior_state) patch skips aggregate snapshotMac + patchMac validation, whereas WhatsApp Web validates them for version-1 patches on ... |
| **P1** | compliance | `src/message/receive.rs:1308` | Group (skmsg) decrypt failures other than NoSenderKey/Duplicate are nacked (error=500) instead of retried, permanently dropping recoverable group m... |
| **P1** | compliance | `src/message/receive.rs:1105` | A PreKeySignalMessage that references a rotated-out signed prekey (SignalProtocolError::InvalidSignedPreKeyId) is NACKed and dropped instead of bei... |
| **P1** | concurrency | `wacore/src/send/group.rs:393` | The group SKDM pairwise fan-out mutates per-device Signal sessions under only the per-(group,sender) sender_key_lock, never the per-device session_... |
| **P1** | concurrency | `src/portable_cache.rs:76` | Capacity-based FIFO eviction in PortableCache::insert_new removes a strongly-held Arc<Mutex> session lock from the map with no strong_count guard, ... |
| **P1** | concurrency | `src/handlers/message.rs:49` | FIFO capacity eviction of a live ChatLane spawns a second concurrent worker for the same chat; the group inbound decrypt path holds no sender-key l... |
| **P2** | compliance | `src/client/app_state.rs:278` | Batched app-state serverSync loop caps at 5 refetch iterations, but WA Web's equivalent loop effectively allows 500, so collections needing >5 pagi... |
| **P2** | concurrency | `src/client/app_state.rs:179` | fetch_app_state_with_retry_inner checks initial_app_state_keys_received before registering the notifier listener, so a key-share arriving in the lo... |
| **P2** | bug | `src/message/receive.rs:1348` | The online (immediate) branch of schedule_unknown_device_sync inserts the user into pending_device_sync for dedup but its detached task never remov... |
| **P2** | bug | `src/handlers/message.rs:84` | Newsletter (and status) <message> stanzas are eager-acked at process_node before the chat-lane worker dispatches them, so a worker generation-break... |
| **P2** | security | `wacore/libsignal/src/protocol/session_cipher.rs:1073` | The wider 25000-step self-session forward-jump ceiling is unlocked by attacker-forgeable identity-key equality, defeating the 2000-step DoS bound a... |
| **P2** | concurrency | `src/handlers/message.rs:47` | The same evict-then-recreate duplicate-worker window breaks the per-chat FIFO ordering guarantee the mailbox exists to provide, causing out-of-orde... |
| **P3** | compliance | `src/message/receive.rs:1378` | A decrypted message carrying a device_sent_message wrapper from a non-self sender is only warned, then unwrapped and dispatched/acked, whereas capt... |
| **P3** | compliance | `src/handlers/notification/mod.rs:76` | Unrecognized notification types are ACKed with a plain <ack> instead of a NACK carrying error=488 (UnrecognizedStanza) as WA Web does. |
| **P3** | compliance | `src/client/accessors.rs:494` | No handler is registered for top-level <error> stanzas; they are logged as "unknown top-level node" instead of being parsed like WA Web's handleError. |
| **P3** | compliance | `src/message/receive.rs:1123` | Session (pkmsg/msg) catch-all nacks (error=500) any unclassified Signal decryption error instead of retrying, dropping recoverable 1:1 messages (e.... |

**Totals**: P0=1, P1=6, P2=6, P3=4 (17 confirmed across 5 tracks).

---

## `review-signal-crypto-correctness` — Signal Crypto Core Correctness & KDF Compliance

*P0 · group: compliance · raw 1 → deduped 1 → **confirmed 1***

**A pre-authentication, spoofable "self-session" classification unlocks a 12.5x-wider forward-jump ceiling (25000 vs 2000 KDF steps), diverging from WA Web's uniform 2000 cap.**

## Signal Crypto Core Correctness & KDF Compliance

**Headline:** A spoofable, pre-MAC "self-session" check unlocks a 25000-step forward-jump ceiling — 12.5x WA Web's uniform 2000 cap — spending KDF work on unauthenticated messages.

**[P2] wacore/libsignal/src/protocol/session_cipher.rs:1073 — self-session ceiling is unlocked by attacker-forgeable identity-key equality, gating work done before MAC verification.**
`decrypt_with_pending_state` calls `get_or_create_message_key` (line 937) *before* `verify_mac` (line 957), so the forward-jump derivation loop (1086-1090) runs pre-authentication. The loop limit comes from `forward_jump_limit(state.session_with_self()?)` (1073), which returns `MAX_FORWARD_JUMPS_SELF = 25_000` (consts.rs:17) vs `MAX_FORWARD_JUMPS = 2_000` for peers. `session_with_self()` (state/session.rs:181-189) is pure byte equality `remote_identity_public == local_identity_public` — and `remote_identity` is taken straight from the attacker-controlled `identity_key` field of the inbound `PreKeySignalMessage` (session.rs:129), with `is_trusted_identity` (src/store/signal.rs:232-242) unconditionally `Ok(true)` and no Bob-side signature check.

Failure scenario: attacker fetches the victim's own public prekey bundle (learns victim IK_pub + a live `signed_pre_key_id`), sends a `PreKeySignalMessage` with `identity_key = victim IK_pub`, `pre_key_id = None`, the valid signed-prekey id, and an inner `SignalMessage{ counter: 25000 }` with garbage ciphertext/MAC. `initialize_bob_session` builds a session where `remote_identity == local_identity` → `session_with_self() == true` → `limit = 25000`; `jump = 25000` passes the `jump > limit` guard, so ~25000 HMAC-SHA256 message keys are derived before `verify_mac` fails and the transaction rolls back. Fully unauthenticated, repeatable (each fresh `base_key` spawns a new self-classified session), ~12.5x the CPU the 2000-step cap exists to enforce. WA Web (captured-js WA/Crypto/LibraryConfig.js:5 `signalFutureMessagesMax: 2e3`; WA/Signal/Cipher.js:328-330) applies `n > r` uniformly with no self/peer branch and checks it before deriving.

Fix: match WA Web — apply one uniform forward-jump cap regardless of `session_with_self`; or at minimum grant the wider ceiling only *after* MAC verification succeeds. Do not gate a security ceiling on public-identity-key equality evaluated before authentication. The consts.rs rationale ("the peer is trusted, so the DoS concern does not apply") is invalid: the classification is attacker-forgeable and checked pre-MAC.

Scope note: bounded CPU amplification against a single flooded client — message-key buffer stays capped at `MAX_MESSAGE_KEYS = 2000` (eviction, no unbounded memory), state fully rolls back, no crash/data compromise. The 2000-step peer path already permits unauthenticated KDF work matching WA Web; the self exception only raises the marginal cost, hence P2.


## `review-signal-session-concurrency` — Signal Session Locking Discipline & Lock-Cache Eviction

*P0 · group: concurrency · raw 4 → deduped 4 → **confirmed 4***

**Signal/sender-key ratchets can be advanced by two writers at once: the group send path and the group inbound path both lack the per-device/per-sender lock the DM path holds, and capacity-FIFO eviction of live coordination locks/lanes mints duplicate serializers.**

## Signal Session Locking Discipline & Lock-Cache Eviction

Four confirmed concurrency defects, all rooted in the same theme: a Signal double-ratchet or sender-key chain read-modify-write runs without a lock that another code path holds under a *different* key (or no lock at all), so two writers advance the same chain and last-write-wins drops a ratchet step. Findings 3 and 4 share one root cause (live-lane eviction) and one fix.

---

**[P1] wacore/src/send/group.rs:393 — group SKDM pairwise fan-out is serialized only by `sender_key_lock(group+own_addr)`, never the per-device `session_lock_for` the DM path uses.**
`prepare_group_stanza` holds `sender_key_lock(&sender_key_name)` (group.rs:393-397) around `encrypt_for_devices_with_sessions` (group.rs:426), which mutates each recipient device's *pairwise* Signal session via `message_encrypt` (load→advance chain→store, `session_cipher.rs:107-119`, non-atomic). The DM path guards those same sessions with a disjoint mutex: `session_lock_for(<device protocol addr>)` via `build_session_lock_keys`/`session_mutexes_for` (src/send/mod.rs:1848-1853, adapters.rs:37). Two different lock keys ⇒ no mutual exclusion. Sends are not per-chat locked (AGENTS.md), so a concurrent group send + DM (or two cold group sends sharing a recipient/own-companion device) both read chain index N and both store N+1; one advance is lost. If the dropped output was the SKDM, the member never gets the sender key and every subsequent skmsg to that group is undecryptable until a retry re-distributes. Genuine desync is gated on a **cold/evicted session-cache window** — the warm path self-heals via the `CheckedOut` mechanism (`signal_cache.rs:346-361`), but the cold backend read at `signal_cache.rs:363-377` returns a second live index-N record ("without caching to avoid conflict") and both writers commit.
*Fix:* on the group send path, acquire `session_lock_for` for each SKDM target device (in `cmp_for_lock_order` order, matching the DM path) across the pairwise fan-out. `sender_key_lock` need only cover the sender-key chain itself.

**[P1] src/portable_cache.rs:76 — capacity FIFO eviction removes a strongly-held `Arc<Mutex>` session lock, so a later `get_with_by_ref` mints a fresh mutex and two writers race the same chain.**
`session_locks` is a capacity-only `PortableCache<String, Arc<async_lock::Mutex<()>>>` (cap 10_000, no TTL; lifecycle.rs:160-162, cache_config.rs:349). `insert_new` evicts unconditionally: `while map.len() >= cap { order.pop_first() → map.remove(oldest_key) }` (lines 74-98), with **no `strong_count` guard** — unlike `reclaim_init_lock` (:453) and `run_pending_tasks` (:477), which do guard. Eviction is pure insertion-order FIFO (`get`'s fast path never refreshes `seq`, portable_cache.rs:190-206), so a long-lived, actively-used address is the *first* victim. Task A holds mutex M1 across an await inside `message_decrypt` (signal.rs:60-70); after A's addr is FIFO-evicted, Task B's `session_lock_for` misses and mints M2≠M1, locks it uncontended, and enters the ratchet for the same session concurrently ⇒ counter/nonce reuse on encrypt, `SessionError` on decrypt. The authors acknowledge this exact hazard at cache_config.rs:346-348 but "mitigate" only by sizing generously — probabilistic, not a guard. Requires >10_000 distinct protocol addresses in one session (reachable in large-community deployments) plus the eviction-during-hold timing.
*Fix:* never FIFO-evict entries whose value `Arc::strong_count > 1` (skip such `pop_first` candidates), or make coordination-lock caches unbounded / evict only via the existing `strong_count`-guarded `run_pending_tasks` path.

**[P1] src/handlers/message.rs:49 — FIFO eviction of a live `ChatLane` spawns a second worker for the same chat; the group inbound decrypt path holds no sender-key lock, so two workers race the receiving sender-key ratchet.**
Same eviction root cause as above, applied to `chat_lanes` (capacity-only FIFO, cap 5000, no TTL; lifecycle.rs:157-165). Evicting a lane drops its `queue_tx` but the worker owns `rx` and keeps draining mid-decrypt — it only self-exits on a `connection_generation` **mismatch** (message.rs:79-86), not on eviction. A subsequent stanza for that chat misses the cache and `create_chat_lane` spawns a second worker at the *same* generation. `process_group_enc_batch` (receive.rs:1170-1202) calls `group_decrypt` with **no** `sender_key_lock` — contrast the 1-1 path's `session_lock_for` at receive.rs:600. `group_decrypt` is load→mutate→store (`group_cipher.rs:174/219/252`) and `load_sender_key` returns `Arc::unwrap_or_clone` of the cached record (signal_adapter.rs:280-296), so both workers mutate private copies and the last store wins — losing a chain advance and its persisted skipped-message keys. Result: subsequent legitimate skmsg fail (bad-mac / `NoSenderKeyState` / spurious `DuplicatedMessage`) and retry-storm until the sender rotates an SKDM. The global processing semaphore does not save this — it's swapped to 64 permits online (sessions.rs:119, commit_batch.rs:182/405), so the ChatLane worker is the only per-chat serializer. Requires >5000 distinct active chats plus the evict-while-decrypting window; recoverable via SKDM resend.
*Fix (shared with P2 below):* make chat lanes non-evictable while their worker is live (join the worker before the entry can be reused), or acquire `sender_key_lock(sender_key_name)` across load/mutate/store in `process_group_enc_batch`, mirroring the 1-1 path.

**[P2] src/handlers/message.rs:47 — the same evict-then-recreate duplicate-worker window breaks per-chat FIFO ordering for 1-1 chats.**
Softer symptom of the finding above (same root cause, same fix). For 1-1 chats `session_lock_for` still serializes the raw ratchet, so no chain corruption — but with W1 draining the evicted lane's backlog and W2 on the new lane, the mailbox's FIFO ordering (comment at message.rs:45-46) is gone. A session-establishing `PreKeySignalMessage` stuck in W1's backlog can be overtaken by a dependent `SignalMessage` in W2 that grabs the session lock first, finds no session, and emits a `NoSession` failure + spurious retry receipt. Self-heals on server resend. Same trigger (>5000 chats + timing) and same fix as the P1 above — prevent live-lane eviction so per-chat processing stays strictly single-worker.

---

**Common thread / recommended sequencing:** (1) add a `strong_count > 1` skip to `PortableCache::insert_new` — one change that closes both lock-cache findings (session_locks and chat_lanes) and is consistent with the file's existing guarded paths; (2) add per-device `session_lock_for` to the group SKDM send fan-out; (3) add `sender_key_lock` to the group inbound decrypt path. Steps 2-3 close the lock-*discipline* asymmetry between the group and DM paths independent of eviction.


## `review-durability-ack-ordering` — Durable-Before-Ack Invariant

*P0 · group: concurrency · raw 2 → deduped 2 → **confirmed 1***

**The offline-sync finisher clobbers cleanup's single-permit reset, letting the next connection's offline drain run with 64 permits and silently lose inbound messages.**

**[P0] src/client/sessions.rs:119 — `publish_offline_sync_live_state` writes `offline_sync_completed=true` + `swap_message_semaphore(64)` with no generation guard and no permit/lock held, racing a concurrent `cleanup_connection_state`.**

Failure scenario (multi-thread tokio): offline drain active on generation G. The detached finisher (`finish_offline_sync(count, G)`) re-checks `connection_generation==G` at sessions.rs:81 and passes, but there is no await and no lock between that check and the sync swap at line 119 — the `finish_inbound_commit_drain(G)` permit was already released at line 79. A concurrent `cleanup_connection_state` (run-loop graceful exit or `Client::disconnect`) then runs fully: `fetch_add` gen→G+1 (lifecycle.rs:759), teardown, `swap_message_semaphore(1)` (lifecycle.rs:840), `offline_sync_completed.store(false)` (846), `inbound_commit_batch.reset()` (854). The finisher resumes and overwrites: `store(true)` + `swap_message_semaphore(64)`. Nothing on the reconnect path re-narrows the semaphore — `connect_graph` (lifecycle.rs:487-507) resets the flag + batcher to drain mode but never touches the semaphore, and `handle_success` (node_io.rs:620) only bumps the generation. The only production `swap(1)` is cleanup's (lifecycle.rs:840; the swap(1) sites at commit_batch.rs:1025/1095/1199 are `#[cfg(test)]`), and the initial value is `Semaphore::new(1)`. So the clobbered 64 persists into the next connection's offline drain: multiple offline stanzas decrypt concurrently while the whole-cache Signal flush in `commit_inbound_batch`/`flush_inbound_commits_under_permit` holds only 1 of 64 permits — persisting rowless ratchet advances for mid-decrypt, not-yet-enqueued stanzas. A mid-drain crash/disconnect then leaves a persisted ratchet for an uncommitted message; its redelivery is acked as a duplicate with no buffered copy → **silent inbound message loss**, the exact durable-before-ack invariant break.

Window is narrow (a store + a mutex lock, closeable only by raw OS preemption, not an async yield) but genuinely unguarded — no happens-before separates the finisher and cleanup tasks on separate worker threads.

Fix: make `publish_offline_sync_live_state` generation-scoped — thread the captured generation through and re-check `connection_generation==generation` immediately before the `swap_message_semaphore(64)`/`offline_sync_completed` writes, bailing if it changed, mirroring the guard already inside `flush_inbound_commits_under_permit` (commit_batch.rs:330-337). Better: perform the flag+semaphore transition atomically under the semaphore mutex (or while holding the processing permit) so it serializes against cleanup's `teardown_inbound_commits_bounded` and last-writer ordering is enforced.


## `review-appstate-sync-integrity` — App-State (syncd) LTHash, Anti-Tampering & MAC Compliance

*P0 · group: compliance · raw 4 → deduped 4 → **confirmed 3***

**A malicious server can seed a curated app-state baseline undetected because genesis patches skip aggregate MAC validation that WA Web enforces unconditionally.**

## App-State (syncd) LTHash, Anti-Tampering & MAC Compliance

**Headline:** Genesis app-state patches bypass aggregate snapshot/patch MAC validation that WhatsApp Web enforces on every incoming patch — a compromised server can seed a curated baseline the Rust client silently accepts.

### P1 — `wacore/appstate/src/processor.rs:369` — Genesis patch skips aggregate snapshotMac + patchMac validation
`validate_patch_macs` returns `Ok(())` immediately when `had_no_prior_state` (version==0, all-zero ltHash), never comparing server-supplied `snapshot_mac`/`patch_mac` against the computed ltHash. On the real inbound path (`app_state.rs:383` calls `process_patch_lists(true)`), a fresh/reset collection routes a snapshot-less v1 patch through the genesis branch (`appstate_sync.rs:407-434`), applies and persists it at v1 with no aggregate MAC check. Per-record MACs in `decode_record` authenticate individual records but do not bind the record *set* to a version, so a malicious server can cherry-pick a curated subset of genuine, previously-relayed records (each with a valid content/index MAC) — dropping a delete/block/mute — with a bogus or absent aggregate MAC. WA Web calls `computeLtHashAndValidatePatch` unconditionally (`CollectionHandler.js:1629`); its `K` (patchMac) hard-throws `SyncdFatalError` on mismatch even for t===1 (`AntiTampering.js:641-660`) — the `t!==1` guard at line 350 only suppresses the retryable empty-lthash throw, not MAC validation.
**Fix:** For a genesis (v1) patch, still recompute and validate `snapshot_mac` and `patch_mac` over the empty baseline; treat a missing aggregate MAC as failure. Special-case only the version-continuity/empty-lthash retryable error for genesis, never the cryptographic MAC checks. The "no baseline" rationale is wrong — the baseline is the known empty hash.

### P2 — `src/client/app_state.rs:278` — Batched serverSync caps at 5 refetch iterations vs WA Web's effective 500
`MAX_ITERATIONS = 5` (comment misreads WA Web's `C` as 5). Each outer iteration issues one IQ page per collection and re-queues on `has_more_patches` (lines 444-446), so 5 iterations == max 5 paginated pages; on exhaustion it only logs a warn and returns `Ok(())` (458-466). WA Web's loop `(l < y || (i.length>0 && l < C)) && i.length !== 0` with `y=5, C=500` collapses to `l < 500` (the `i.length !== 0` gate forces `i.length>0` true), so it syncs up to 500 pages. A large-backlog collection needing >5 pages under-syncs (missing contacts/chats/pushnames); the critical path then cancels its watchdog and dispatches `Connected` on the false `Ok`. Self-heals across reconnects (version persisted per page) but persists for the life of a stable connection. The single-collection path already uses `MAX_PAGINATION_ITERATIONS=500` (line 492), making the batched path an outlier.
**Fix:** Raise `MAX_ITERATIONS` to 500 and correct the comment.

### P2 — `src/client/app_state.rs:179` — Non-sticky notifier listener registered after the flag check → lost wake, 10s stall
`fetch_app_state_with_retry_inner` loads `initial_app_state_keys_received` at line 179 but only calls `initial_keys_synced_notifier.listen()` at line 184 (inline in `rt_timeout`). The notifier is `event_listener::Event` (non-sticky). If the primary's auto-shared `AppStateSyncKeyShare` is processed (`special.rs:120-124`: store key, then `notify(usize::MAX)`) in the load→listen gap, it wakes zero listeners; the retry then blocks the full 10s even though the key is already persisted, before `continue` re-runs the sync and succeeds. Bounded, self-healing, off the critical watchdog path. Two sibling sites (`app_state.rs:643`, `node_io.rs:980`) deliberately register the listener *before* the check with explicit "not sticky" comments — this site violates that pattern.
**Fix:** Register the listener before the flag load: `let listener = self.initial_keys_synced_notifier.listen(); if !self.initial_app_state_keys_received.load(Relaxed) { rt_timeout(rt, 10s, listener).await }`.


## `review-receive-routing-recovery` — Receive Error-Recovery Matrix, Protocol-Message Gating & Routing

*P0 · group: bug · raw 8 → deduped 8 → **confirmed 8***

**Signal decrypt failures on the receive path are terminally NACKed (error=500) instead of retried, silently dropping recoverable 1:1 and group messages that WA Web would recover via retry receipt.**

## Receive Error-Recovery, Protocol-Message Gating & Inbound Routing

The dominant theme: the receive path's decrypt-error matches route unclassified Signal-crypto errors to a terminal `spawn_nack(UnhandledError)` (ack error=500), which tells the server to stop retransmitting. WA Web classifies every `SignalDecryptionError` as `SignalRetryable → RETRY` and sends a retry receipt. Result is permanent, silent message loss on key desync — the opposite of the intended recovery. Findings ranked by severity.

---

**[P1] src/message/receive.rs:1308 — Group (skmsg) decrypt failures are NACKed instead of retried, dropping recoverable group messages on sender-key desync.**
When a participant rotates their sender key / re-registers, `group_decrypt()` returns `SignatureValidationFailed`, `InvalidMessage("decryption failed")`, `InvalidSenderKeySession`, or `UnrecognizedMessageVersion` (`wacore/libsignal/src/protocol/group_cipher.rs:206-250`) — none normalized to `NoSenderKeyState`/`DuplicatedMessage`. All hit the catch-all `Err(e) =>` arm (1278-1310) which nacks 500 for non-status chats; the server drops the message with no resend. The sibling 1:1 path (1000-1053) already treats `BadMac`/`InvalidMessage` as retryable, so the group path is asymmetric.
*Fix:* for skmsg Signal-crypto errors (`SignatureValidationFailed`, `InvalidMessage`, `InvalidSenderKeySession`, `UnrecognizedMessageVersion`), call `handle_decrypt_failure` (retry receipt) instead of `spawn_nack`; reserve 500 for genuinely non-Signal errors. A retry receipt prompts the sender to resend SKDM + message.

**[P1] src/message/receive.rs:1105/1123 — Session (pkmsg/msg) catch-all NACKs unclassified Signal decrypt errors, dropping recoverable 1:1 messages (merged with the InvalidSignedPreKeyId case).**
The session-decrypt match handles `SessionNotFound`, `BadMac|InvalidMessage`, `InvalidPreKeyId`, `UntrustedIdentity`, `DuplicatedMessage` — but not `InvalidSignedPreKeyId`. A `PreKeySignalMessage` whose header names a signed prekey we've pruned past `SIGNED_PRE_KEY_RETENTION` (rotate_key.rs:245-251) makes `SignedPreKeyAdapter::get_signed_pre_key` (signal_adapter.rs:256) return `InvalidSignedPreKeyId`, which falls to the `else` at 1105/1123 → `spawn_nack(UnhandledError)`, no retry receipt, stanza cleared from the offline queue → permanent loss. The sibling `InvalidPreKeyId` case (1054) IS retried, making this an inconsistent drop. (Verifiers refuted the finder's `SignatureValidationFailed`/`UnrecognizedMessageVersion` triggers on the 1:1 path — those are normalized to `BadMac`/`InvalidMessage` by `decrypt_message_with_record` — so `InvalidSignedPreKeyId` is the real reachable trigger.)
*Fix:* add `InvalidSignedPreKeyId` (and other libsignal decrypt errors WA Web wraps as `SignalDecryptionError`) to the retryable arm → `handle_decrypt_failure(RetryReason::InvalidKeyId/InvalidKey)`; the retry receipt carries our live bundle so the peer rebuilds against the current signed prekey. Keep the 500 nack only for genuinely non-Signal errors (WA Web's `C.Unknown → PARSE_ERROR`).

---

**[P2] src/message/receive.rs:1348 — Online branch of `schedule_unknown_device_sync` leaks its dedup entry, permanently disabling unknown-device recovery for that user for the connection.**
`pending_device_sync.add(U)` (1334) is a shared dedup guard, but the online detached task (1348-1356) only runs `invalidate_device_cache` + `get_user_devices` and never removes `U`. The only drains are `take_all()` on `<ib><offline>` completion (ib.rs:203, once per connection) and `clear()` on teardown (lifecycle.rs:844). On a steady connection, if `U` later adds device `D2` and its `<notification type="devices">` is missed/reordered, a message from `D2` calls `add(U)→false` → early return at 1335: no invalidation, no usync. `D2` is never learned; send fanouts silently omit it until reconnect.
*Fix:* in the online branch, remove `user_jid` from `pending_device_sync` when the detached task finishes (add a `PendingDeviceSync::remove(&jid)` guard), mirroring the offline `take_all()` drain.

**[P2] src/handlers/message.rs:84 — Newsletter/status `<message>` stanzas are eager-acked at `process_node` before the chat-lane worker dispatches, so a worker generation-break drop during teardown loses them with no redelivery.**
`should_ack` (node_io.rs:480-486) returns true for newsletter/status, so `process_node` (node_io.rs:455) sends `<ack class="message">` at enqueue time — before the worker runs `handle_newsletter_message`. If a second stanza `N2` is buffered while the worker is busy and the connection tears down (`connection_generation` bumped at lifecycle.rs:759, `chat_lanes.clear()` at 810 leaves `N2` in the async_channel), the worker's top-of-loop generation check (message.rs:79-86) breaks without dispatching `N2`. `N2` was already acked → server never redelivers → permanent loss. DM/group are immune because `should_ack` returns false for them (ack deferred post-decrypt inside the worker). WA Web's `MsgSendReceipt.js` acks as a function of the process result, never eagerly.
*Fix:* move the newsletter/status transport ack into the worker after `handle_newsletter_message` succeeds (mirroring deferred DM/group ack), or have the generation-break path leave these unacked so the server redelivers.

---

**[P3] src/message/receive.rs:1378 — Non-self `device_sent_message` wrapper is only warned, then unwrapped/dispatched/acked, whereas WA Web aborts with `DeviceSentMessageError` (INVALID_DSM) and nacks.**
An authenticated peer sends a decrypted `Message` with `device_sent_message` set and `is_from_me=false`; line 1379 warns but does not return, so `unwrap_device_sent` (1406) + `dispatch_parsed_message` (1504) surface the inner content and send a delivery receipt. WA Web's `validateMsgDestination` fails `isMeAccount(author)` → throws → drop. Low security impact (no attribution spoof; self-only protocol messages are re-gated post-unwrap at 1428/1446/1458/1478) — it's an ack-vs-drop divergence.
*Fix:* when `device_sent_message.is_set() && !info.source.is_from_me`, return an outcome that nacks rather than acks.

**[P3] src/handlers/notification/mod.rs:76 — Unrecognized notification types get a plain `<ack>` instead of a NACK with error=488 (UnrecognizedStanza).**
The `other =>` arm dispatches `Event::Notification` and `should_ack` returns true → plain success ack. WA Web's `createNackFromStanza(UnrecognizedStanza=488)` sends `<ack ... error="488">`. Both consume the stanza (no redelivery), so impact is only the missing server-side error/telemetry signal. Note: only reproduces for a type unknown to *both* clients — the finder's `w:growth` example is wrong (WA Web handles it).
*Fix:* emit error=488 for notification types Rust does not functionally process, or gate the raw-event fallback to only WA Web-surfaced types (a blanket 488 would mis-nack pay/psa/server/hosted which WA Web handles).

**[P3] src/client/accessors.rs:494 — No handler registered for top-level `<error>` stanzas; they log as "unknown top-level node" instead of being parsed like WA Web's `handleError`.**
A server `<error code="479"/>` (smax-invalid) finds no handler → `warn!("Received unknown top-level node")`; `should_ack` excludes "error" so nothing is acked (matching WA Web's NO_ACK). Purely diagnostic: the specific 479=SMAX_INVALID signal is lost and the stanza is mislabeled.
*Fix:* add an `ErrorHandler` for tag `"error"` that parses `code` and logs known codes (479=smax-invalid); keep it NO_ACK.

