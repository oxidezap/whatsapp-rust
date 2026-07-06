# Adversarial Review — Findings (P1 tier)

> Produced by `scripts/adversarial-review.workflow.js` (`args: { tier: "P1" }`), run `wf_d830e71b-363`:
> 124 agents, 0 errors, ~6.8M tokens. Each candidate survived 3 independent adversarial
> skeptics (default-refute; majority-real to be kept). **25 confirmed**. File-anchored, with a
> concrete failure scenario and captured WhatsApp-Web JS cross-reference where it is a compliance issue.

## Summary

| Sev | Group | File:line | Finding |
|---|---|---|---|
| **P1** | compliance | `wacore/src/protocol/keepalive.rs:36` | Dead-socket watchdog anchors its 20s window to the MOST RECENT send (last_data_sent_ms), whereas WA Web anchors it to the FIRST send after the last... |
| **P1** | security | `wacore/src/prekeys.rs:320` | When validate_adv_with_identity_key returns NoAccountKey (companion device-identity omits account_signature_key AND no stored primary identity), th... |
| **P1** | security | `src/passkey/flow.rs:531` | Passkey skip_handoff_ux (verification-code bypass + auto-confirm) is enabled on every link, including fresh first-time links, because it is gated o... |
| **P1** | bug | `wacore/src/send/group.rs:360` | A single participant's session-setup failure aborts SKDM to the entire group cohort while marking all of them (including own companion devices) has... |
| **P1** | security | `wacore/src/voip/e2e_srtp.rs:165` | RecvRocTracker.guess_roc folds ROC/s_l state on unauthenticated packets, letting an on-path attacker permanently desync the receiver's rollover cou... |
| **P1** | security | `wacore/binary/src/decoder.rs:504` | read_node_ref recurses through read_content with no depth limit, so a hostile (optionally zlib-compressed) inbound frame overflows the native stack... |
| **P2** | concurrency | `src/client/lifecycle.rs:402` | The expected_disconnect fast path zeroes the reconnect error counter and continues the run loop with zero sleep, so a server that repeatedly emits ... |
| **P2** | compliance | `src/client/offline_resume.rs:119` | The offline batch-pull loop is stopped by a count-derived `pending == 0` guard (and the sibling `processed >= total` in node_io.rs), but WAWebOffli... |
| **P2** | compliance | `wacore/src/msg_secret.rs:72` | msg-secret retention classifies CAG/community text parents as Text (30-day horizon), but encrypted reactions and community comments to those parent... |
| **P2** | compliance | `src/upload.rs:73` | Resumable upload sends the tail bytes with a `file_offset` query param, but the WhatsApp MMS server keys partial-append uploads on `bytestart`/`byt... |
| **P2** | bug | `src/download.rs:458` | Streaming/buffered download_to_writer never truncates the writer, so a host-failover in which the failed host wrote MORE decrypted bytes than the s... |
| **P2** | compliance | `wacore/src/pair.rs:272` | do_pair_crypto signs the companion device signature with the hosted prefix [6,6] for hosted accounts, but WA Web's generateDeviceSignature uncondit... |
| **P2** | compliance | `wacore/src/send/classify.rs:309` | should_hide_decrypt_fail omits the MESSAGE_UNSCHEDULE protocol-message type that WA Web hides, so an unschedule stanza goes out without decrypt-fai... |
| **P2** | compliance | `src/send/mod.rs:1457` | Group sender-key rotation is triggered by a fabricated chain-iteration threshold (SENDER_KEY_ROTATION_THRESHOLD=1000) that has no analog in WA Web,... |
| **P2** | bug | `wacore/binary/src/encoder.rs:313` | Encoder's string-JID classifier splits the user at '_' to strip an 'agent', silently truncating any JID whose user contains '_<0-255>', diverging f... |
| **P3** | concurrency | `src/client/sessions.rs:221` | The history-sync in-flight counter is not generation-fenced, so a stale chunk task enqueued/started in a previous connection decrements the new con... |
| **P3** | concurrency | `src/features/rotate_key.rs:54` | Signed-pre-key rotation and one-time-prekey uploads guard the server's advertised signed pre-key under two disjoint locks, so a sibling post-login ... |
| **P3** | compliance | `wacore/src/download.rs:421` | decrypt_stream_to_writer emits AES-CBC plaintext to the caller's writer before the trailing HMAC-SHA256 is verified, diverging from WA Web's hmacAn... |
| **P3** | compliance | `src/download.rs:17` | Per-host fallback_hostname is dropped when building download requests, so host-failover never tries the CDN's alternate fallback domain that WA Web... |
| **P3** | compliance | `src/features/message_edit.rs:504` | Exported decrypt_secret_encrypted_with_fallback collapses poll/event kinds to a single merged fallback ctx, missing WA Web's homogeneous both-LID /... |
| **P3** | overhead | `src/mediaconn.rs:63` | refresh_media_conn has no single-flight/in-flight dedup, so concurrent media operations each fire a separate media_conn IQ, diverging from WA Web's... |
| **P3** | compliance | `src/prekeys.rs:36` | MAX_PREKEY_ID is treated as an inclusive maximum (16777215) whereas WA Web's PRE_KEY_NON_INCLUSIVE_UPPER_BORDER reserves 16777215 as a non-inclusiv... |
| **P3** | compliance | `src/features/signal.rs:315` | create_participant_nodes uses the edit-unaware should_hide_decrypt_fail instead of should_hide_decrypt_fail_for_send, so it cannot hide edit-family... |
| **P3** | compliance | `wacore/noise/src/handshake.rs:195` | verify_server_cert accepts Noise certs with absent issuerSerial/serial fields that WhatsApp Web explicitly rejects, weakening cert-chain structural... |
| **P3** | bug | `wacore/binary/src/encoder.rs:410` | Encoder classifies any ≤48-char string containing '@' as a JID and emits JID_PAIR, but the decoder rejects JID_PAIR whose server is not a known Wha... |

**Totals**: P1=6, P2=9, P3=10 (25 confirmed across 6 tracks).

---

## `review-wire-transport-robustness` — Binary Codec & Noise Transport: Hostile-Frame Robustness + Wire Compliance

*P1 · group: security · raw 5 → deduped 5 → **confirmed 4***

**A hostile inbound frame can stack-overflow and abort the process via unbounded node-recursion; three lower-severity encoder/cert wire-compliance divergences round out the track.**

## Binary Codec & Noise Transport: Hostile-Frame Robustness + Wire Compliance

**[P1] wacore/binary/src/decoder.rs:504 — Unbounded recursion in `read_node_ref`/`read_content` lets a hostile frame overflow the native stack and abort the process.**
`read_node_ref` (504) → `read_content` (519) → LIST_8/LIST_16 branch (468–475) loops `self.read_node_ref()` with no depth counter anywhere in the chain. A post-Noise server/compromised relay sends a frame whose zlib-decompressed payload is a repeating 5-byte level `F8 02 <tag> F8 01` (each level = one child, ~5 bytes on the wire, tiny after zlib, decompressed buffer stays under the 16 MiB `MAX_DECOMPRESSED_SIZE` cap in util.rs). Reachable from `node_io.rs:271` `OwnedNodeRef::new` via `decrypt_frame` → `unmarshal_ref`. Tens of thousands of levels blow a ~2 MB Tokio worker stack → uncatchable SIGSEGV/abort — the exact outcome the crate's own fuzz harness (`fuzz_targets/unmarshal_ref.rs`) forbids. JS analog throws a catchable `RangeError`; the Rust port is strictly worse. *Fix:* thread a depth counter through `read_node_ref`/`read_content` and return `BinaryError` once a sane cap (~128, matching realistic stanza nesting) is exceeded, before recursing. (Attacker is constrained to the authenticated server, not an arbitrary MITM — one verifier argued P2 on that basis; the process-abort/DoS impact keeps it at the top of the fix list either way.)

**[P2] wacore/binary/src/encoder.rs:313 — String-JID classifier splits the user on `_` to strip a non-existent "agent", silently truncating any user matching `text_<0-255>`.**
`parse_jid_meta` does `user_agent.find('_')` and, if the tail parses as `u8`, sets `user_end = underscore_idx`. `"hello_12@s.whatsapp.net"` as a string attr encodes `JID_PAIR("hello")` — `_12` dropped — while the typed `Jid{user:"hello_12"}` and `FromStr`/`parse_jid_fast` (which only treat `.` as the agent separator) both encode `JID_PAIR("hello_12")`. Same logical JID, two divergent wire byte sequences; WA Web's `WapJid.toString` never emits an agent token at all. Outbound-only and not hit by current numeric-user traffic, but a latent silent-corruption/consistency defect for any underscore-bearing username string. *Fix:* drop the `_` branch so `user_end = user_agent.len()` (WA JID strings carry no agent); add an underscore-user case to `test_jid_string_vs_direct_encoding_matches`.

**[P3] wacore/binary/src/encoder.rs:410 — Encoder classifies any ≤48-char `@`-string as a JID and emits `JID_PAIR`, but its own decoder rejects unknown-server `JID_PAIR`, so encode↔decode is not identity.**
`classify_string_hint` gates JID classification only on `s.len() <= 48`; `"x@example.com"` → `write_jid_from_meta` emits `JID_PAIR{user:"x", server:"example.com"}`. Feeding those bytes back through `read_jid_pair` (decoder.rs:125) → `Server::try_from("example.com")` → `Err` (hard fail, no fallback). Empirically reproduced for both attr values and String content; encode succeeds, self-decode hard-fails, and the WA server would receive an unmappable `JID_PAIR`. Graceful `Err` not a crash, and no confirmed protocol path feeds such a string today. *Fix:* only treat a string as a JID when its server parses to a known `Server` (mirror `Server::try_from`/`server_supports_ad_jid`); otherwise fall through to `RawBytes`. (One verifier saw the surfaced error as `InvalidNode` rather than `AttrParse` — immaterial to the round-trip failure.)

**[P3] wacore/noise/src/handshake.rs:195 — `verify_server_cert` accepts Noise certs with absent `issuerSerial`/`serial` that WA Web explicitly rejects.**
Line 195 `intermediate_details.issuer_serial.unwrap_or(0)` maps a missing field to `0 == WA_CERT_ISSUER_SERIAL`, passing the root-issuance check; line 227 compares `leaf.issuer_serial != intermediate.serial` as `Option<u32>`, so `None == None` passes the leaf-binding check; `serial` presence is never validated. WA Web's `ChainCertificateA6.js` `k()` rejects absent `issuerSerial`/`serial`/`key` with `invalid-certificate` on both intermediate and leaf. Defense-in-depth/wire-compliance only — the XEdDSA checks in `verify_cert_step` still require the WA root private key, so no standalone auth bypass. *Fix:* require `intermediate.issuer_serial`, `intermediate.serial`, and `leaf.issuer_serial` to be `Some(_)` before the equality checks, instead of `unwrap_or(0)`/`Option==Option`.


## `review-send-path-compliance` — Send-Path Wire Compliance: phash, Namespace Alignment & Warm-Skip

*P1 · group: compliance · raw 4 → deduped 4 → **confirmed 4***

**One P1 send-path abort leaves own companion devices permanently keyless in groups; three narrower decrypt-fail/rotation parity gaps.**

## Send-Path Wire Compliance: phash, Namespace Alignment & Warm-Skip Correctness

**[P1] wacore/src/send/group.rs:360 — One participant's session-setup failure aborts SKDM for the whole group cohort, permanently orphaning own companion devices.**
Cold `force_skdm` send to a group whose distribution list is `{external A, external B, own companion C}`. If A's prekey bundle hard-fails (non-406 fetch error or `process_prekey_bundle` Err) in `ensure_sessions_for_devices` (encrypt.rs:523/607), `session_plan` becomes `None` (group.rs:360-372). Because the entire SKDM fan-out is gated on `if let Some(plan) = session_plan` (group.rs:399/408), B and C get no SKDM even though their sessions established fine. The skmsg still ships; phash is computed over the full correct device set so the server reports no mismatch and never forces a resend. On ACK, the full `{A,B,C}` is marked `has_key=true` with `exclude_own_devices=false` (mod.rs:1167, group.rs:561, mod.rs:1941). B recovers via a retry receipt → `mark_forget_sender_key` flips it cold → re-SKDM'd. But C shares our own user, so its retry hits `mark_forget_sender_key` with `exclude_own_devices=true` (sender_keys.rs:76-84), which filters own-user JIDs and returns early with no DB write (sender_keys.rs:33-44). C stays `has_key=true` forever; `filter_skdm_targets` + `device_and_primary_warm` skip it on every send. C can never decrypt our group messages from this device until an unrelated full rotation (participant removal / PN↔LID migration). WA Web (`GroupSkmsgJob.js:39-52`, `GroupKeyDistributionMsg.js` per-device try/catch) swallows the session failure and still SKDMs every device with a valid session, dropping only broken A.
*Fix:* isolate the failure per device — attempt per-device SKDM encryption for every device that already has a session instead of returning Err from the whole cohort. Alternatively, only mark a device `has_key=true` when it was actually in `skdm_encrypted_devices` (or already warm), so a companion that missed its SKDM is re-targeted next send.

**[P2] wacore/src/send/classify.rs:309 — `should_hide_decrypt_fail` omits `MESSAGE_UNSCHEDULE`, so unschedule stanzas ship without `decrypt-fail="hide"`.**
The `protocol_message.type` arm accepts only `EphemeralSyncResponse`, `RequestWelcomeMessage`, `GroupMemberLabelChange`, or `edited_message`. A message with `protocol_message.type = MessageUnschedule` (proto 32) returns `false`, so all three send paths (DM/SKDM/SKMSG) emit the `<enc>` node without the hide attribute; recipients who can't decrypt render a persistent "waiting for this message" placeholder WA Web suppresses (`EProtoUtils.js:122`). Latent — the library never constructs this type internally, only downstream callers do.
*Fix:* add `|| t == ProtocolType::MessageUnschedule` to the arm.

**[P2] src/send/mod.rs:1457 — Fabricated 1000-iteration chain threshold triggers a full-group SKDM rotation with no WA Web analog.**
`SENDER_KEY_ROTATION_THRESHOLD = 1000` (its own comment admits the value is invented) forces `needs_rotation` once the per-message sender-chain iteration reaches 1000. On send #1001 the code does `delete_sender_key` + `clear_sender_key_devices` + cache `invalidate` and redistributes a fresh SKDM (with prekey fetch/X3DH for any sessionless device) to every device of every participant — a latency/rate-limit burst on a single send. WA Web rotates only on the event-driven `participant.rotateKey` flag (membership/identity/audience changes); `PERIODIC_ROTATION=5` exists in the enum but is posted nowhere in the captured bundle. Note: one verifier argues `PERIODIC_ROTATION`'s existence means periodic rotation is a legitimate (server-flag-driven) WA operation, making this a trigger-source/cadence divergence with a guessed threshold rather than a hard defect — hence P2.
*Fix:* drop the count-based path or gate it behind a server-signalled rotate flag; keep rotation event-driven via the existing `mark_forget_sender_key` + `clear_sender_key_devices` machinery.

**[P3] src/features/signal.rs:315 — Public `create_participant_nodes` uses edit-unaware `should_hide_decrypt_fail`, dropping `decrypt-fail="hide"` for edit-family stanzas.**
The three canonical paths (dm.rs:138/318, group.rs:425/493) call `should_hide_decrypt_fail_for_send(edit.as_ref(), message)`, but line 315 calls the plain classifier with no `EditAttribute`. An external caller encrypting an edit whose payload isn't itself infra (e.g. `secret_encrypted_message{secret_enc_type=MESSAGE_EDIT}`, or `protocol_message.type=MESSAGE_EDIT` with no `edited_message`) gets `hide=false` where the canonical paths emit `hide=true`. No in-tree caller — latent for external SDK consumers; UX-only (placeholder suppression).
*Fix:* thread an `EditAttribute` in and call `should_hide_decrypt_fail_for_send`, or document that callers must pre-classify edits.


## `review-pairing-prekey-trust` — Pairing/Auth Trust Root: ADV Signatures, Prekey Watermarks & Passkey

*P1 · group: security · raw 4 → deduped 4 → **confirmed 4***

**Two P1 trust-root defects: companion prekey bundles are soft-accepted where WA Web hard-rejects, and passkey verification-code UX is structurally disabled on every fresh link.**

## Pairing/Auth Trust Root — Confirmed Findings

Two P1 trust-root gaps, one P2 hosted-signature compliance deviation, one P3 off-by-one. All survived adversarial verification against captured WA Web JS ground truth.

---

**[P1] wacore/src/prekeys.rs:320 — NoAccountKey ADV result is soft-accepted; a companion Signal session is established under a server-supplied identity key where WA Web hard-rejects.**
When `validate_adv_with_identity_key` returns `NoAccountKey` (companion `device-identity` omits `account_signature_key` AND no stored primary identity — `wacore/src/adv.rs:74-80`), `node_to_pre_key_bundle_ref` only `log::debug!`s and returns `Ok(bundle)` (`:308-330`). On first-ever contact, `collect_account_identities` (`src/prekeys.rs:161`) snapshots the store before the fetch, so a companion `Bob:5` has no device-0 fallback; a malicious relay returns `<identity>=attacker key` + ADV blob with the account key stripped, and the victim encrypts to `Bob:5` under the attacker key. WA Web's `createSignalSession` throws `'invalid identityKey fetched'` on the same input (`SessionApi.js`; `SignatureApi.js` invariant `s(0,56344)`). Identical soft-accept on the receive/retry path at `src/retry.rs:1005-1007`.
*Fix:* for `device != 0`, treat `NoAccountKey` as a hard `Err` reject; require the stored device-0 fallback and compare any in-blob `account_signature_key` against it (`adv.rs:74` never does this comparison today, so even the `Valid` path is only meaningful via the fallback).
*Severity note:* one verifier argued incremental MITM surface is bounded — the branch only fires while device-0 is itself TOFU-trusted/server-controllable — but the deviation from WA Web's hard reject is real and the code comment consciously documents the gap. Fix both call sites.

**[P1] src/passkey/flow.rs:531 — passkey `skip_handoff_ux` (code bypass + auto-confirm) is enabled on every link, including fresh first-time links, disabling the verification-code MITM check.**
`drive_passkey_request` derives `handoff_key` unconditionally from `snapshot.adv_secret_key` — 32 random bytes always present at device creation (`wacore/src/store/device.rs:372-373`) via an HKDF-expand that never errors — with no gate on `account`/`pn`/`lid`. So `skip_handoff_ux = handoff_proof.is_some()` (`flow.rs:120-122`) is structurally always true, and the "fresh link ⇒ show XXXX-XXXX code" branch is dead. When a continuation arrives from `SERVER_JID` (the only check, `:577-583`) carrying an attacker-chosen `primary_ephemeral_identity`, `drive_continuation` (`:474`) auto-fires `send_passkey_confirmation` — `confirm()` encrypts the rotated `new_adv_secret` under `ECDH(companion_eph, attacker_primary_pub)` and commits the rotation, leaking the pairing trust root with no human code compare. Contradicts the code's own re-link-only intent (`shortcake.rs:26-29`, `flow.rs:524-525`).
*Fix:* only derive/attach `handoff_key` when a prior linked identity exists (`snapshot.account.is_some()` / `pn` / `lid`); leave it `None` on a fresh device so `skip_handoff_ux` is false and the code UX runs.
*Exploit precondition:* active disclosure needs a compromised server/relay (or Noise break) to inject the `SERVER_JID` continuation; the suppressed code UX is exactly the defense-in-depth control against that case, so it must not be disabled on fresh links.

**[P2] wacore/src/pair.rs:272 — hosted device signature is generated with prefix `[6,6]`, but WA Web's `generateDeviceSignature` unconditionally uses the E2EE prefix `[6,1]`.**
For `account_type == HOSTED`, `is_hosted_account` (`:182`) drives the device-signature GENERATE step to `ADV_HOSTED_PREFIX_DEVICE_SIGNATURE_VERIFICATION` (`[6,6]`), signing `[6,6] || details || identityPub || accountSigKey`. WA Web's `generateDeviceSignature` (`SignatureApi.js` fn N, invoked at `HandlePairSuccess.js:137-141`) has no hosted branch and always emits `[6,1]`; `[6,6]` appears only in the VERIFY helper (fn M). The companion thus returns a signature over bytes a real companion never produces.
*Fix:* use `[6,1]` for the device-signature GENERATE step regardless of `is_hosted_account`; split the device-signature prefix decision from the shared HMAC/account-sig `is_hosted_account` family.
*Note:* medium confidence — WA Web's verify path does accept `[6,6]` for genuinely-hosted devices, so concrete server rejection hinges on unobservable server-side gating; the byte-level divergence from the reference generator is unambiguous. Confined to the hosted/business-coex path (mainstream E2EE unaffected).

**[P3] src/prekeys.rs:36 — `MAX_PREKEY_ID` (16777215) is inclusive, so the allocator can mint id 16777215 which WA Web reserves as a non-inclusive border.**
Boundary checks use strict `>` (`:90`, `:113`) and `start_prekey_id(16777215,0)` returns 16777215 (test `:1049`), so a window ending exactly at the top mints id 16777215. WA Web's `makePreKeys` (`WA/Signal/Keys.js` `i = n+1 >= s ? 1 : n+1`) caps assigned one-time-prekey ids at 16777214.
*Fix:* make the border exclusive (valid ids `1..=16777214`): wrap on `first + wanted - 1 >= MAX_PREKEY_ID` and treat `id + 1 >= MAX_PREKEY_ID` as the wrap trigger in `mark_single`/`start_prekey_id`.
*Note:* low confidence / practically unreachable (~16.7M monotonic allocations). The finder's cited JS line (`StoreApi.js:393`) is the wrong namespace (signed prekeys); the off-by-one holds only against the correct one-time-prekey path (`Keys.js`). WA Web's read-validation accepts 16777215 anyway, so downstream harm is nil — cosmetic spec alignment.


## `review-connection-lifecycle-resilience` — Connection Lifecycle: Reconnect, Keepalive, Offline-Resume & History-Sync

*P1 · group: concurrency · raw 6 → deduped 6 → **confirmed 5***

**Connection lifecycle deviates from WA Web in five verified ways — the keepalive dead-socket watchdog can be suppressed indefinitely by outgoing traffic, and expected-disconnect reconnects have no backoff floor.**

## Connection Lifecycle: Reconnect Storms, Keepalive, Offline-Resume & History-Sync

Five confirmed findings. All are genuine deviations from WA Web ground truth (captured JS); severities reflect post-verification triggerability.

**[P1] wacore/src/protocol/keepalive.rs:36 — dead-socket watchdog anchors to the most-recent send, not the first send after a receive.**
`is_dead_socket` evaluates `ms_since(last_sent_ms)`, and `record_frame_sent` (stats.rs:96-102) overwrites `last_data_sent_ms` on *every* outgoing frame via the single send chokepoint (noise_socket.rs:177). On a half-open socket (peer silently gone, reads hang, writes buffer) any app frame — message/receipt/presence — landing inside the ping's 20s wait keeps `ms_since(last_sent) < 20000`, so the watchdog returns false. Since `is_dead_socket` is the *only* steady-state reconnect trigger in the loop (TransientFailure just increments an unread `error_count`), detection is suppressed for as long as the app keeps emitting traffic (~every 10-18s). WA Web's `onOrBefore` (Timer.js:27) keeps the *earlier* deadline: the first send after a receive-cancel arms a 20s deadline and later sends never push it back, so it fires `softCloseSocket` 20s after the first post-receive send. Backstops cap this to a bounded self-healing delay (a >20s traffic lull lets the idle ping age out; TCP retransmit exhaustion ~15min eventually errors the write), so it is a stall/silent-loss window, not truly infinite.
*Fix:* track a separate "first send since last receive" timestamp — reset to 0 on any receive, set only when currently 0 on send — and evaluate `is_dead_socket` against that anchor. Do not use the latest-send timestamp.

**[P2] src/client/lifecycle.rs:402 — expected_disconnect fast path zeroes the reconnect counter and continues with zero sleep.**
On the `expected_disconnect` branch the code does `auto_reconnect_errors.store(0)` + `connected_at_ms.store(0)` then `continue`, skipping the stability-gated fibonacci backoff at 414-430 entirely. A server that emits `<stream:error code="515">` before `<success>` on every connection (node_io.rs:1215 sets the flag; 515 does not disable auto_reconnect) drives back-to-back reconnects at handshake speed with the error counter pinned at 0 — a 0s storm the code's own comment (node_io.rs:626) warns against. `connected_at_ms` is never set when 515 precedes `<success>`, so even the normal backoff would treat the cycle as unstable, but `continue` bypasses it. The dead-socket path (keepalive.rs:219 → reconnect_immediately → lifecycle.rs:726) hits the same branch but is rate-limited to ~1/20s. Requires an anomalous/hostile server — real network death fails inside `connect()` and takes the correct backoff path (line 335). Impact is self-inflicted CPU/bandwidth/battery waste against a broken server; mislabeled "concurrency" (no race). WA Web uses a 10s fibonacci base (Comms.js:126, never zero) and `cancelReset()` on stream errors to preserve/escalate backoff.
*Fix:* track consecutive expected/immediate reconnects; above a small threshold (or when `connected_at_ms` shows the last cycle never reached stable `<success>`) fall through to the fibonacci backoff instead of `continue`, and do not reset the counter on stream-error-driven reconnects.

**[P2] src/client/offline_resume.rs:119 — offline batch-pull loop terminates on the preview `count` instead of the `<ib><offline>` end marker.**
`on_offline_stanza_arrived` short-circuits on `pending == 0`, and node_io.rs:378 flips `offline_sync_metrics.active=false` once `processed >= total` (total = `<ib><offline_preview count>`), after which node_io.rs:355's `if active` guard stops both counting and continuation. WA Web (Handler.js `$13`) never gates on a preview count — it paces on decrypt backpressure (`$3 <= C`) and completes only on the `<ib><offline>` end marker; `count` is an estimate (OfflineResumeManager.js clamps progress via `Math.min(...,100)` and *adds* mid-resume previews). If the server holds more offline-attributed stanzas than announced and withholds them pending the next `<ib><offline_batch>` request, the tail is never requested. Due to the one-batch look-ahead (each batch's first arrival schedules the next), overshoot up to one BATCH_SIZE (200) is still delivered; a real stall needs backlog > count + 200 with no updated preview. Not an indefinite hang — `wait_for_offline_delivery_end` has a 60s timeout (sessions.rs:138) that force-completes; the undelivered tail persists server-side and redelivers on reconnect. A mid-resume second `<ib><offline_preview>` resets total/processed and re-arms, so that path recovers.
*Fix:* drive continuation/completion from the pull-protocol signals (batch drain + `<ib><offline>` end marker), not the preview count; stop setting `active=false` on `processed >= total` and treat `count` purely as a progress denominator.

**[P3] src/client/sessions.rs:221 — history-sync in-flight counter is not generation-fenced.**
`begin/finish_history_sync_task` mutate a single global `AtomicUsize` with no `connection_generation` check (unlike the fenced offline path at offline_resume.rs:150). Gen N: chunk A's detached task blocks in `download_to_writer`/blocking decode (socket-independent, survives disconnect). Disconnect → `cleanup_connection_state` does an unconditional `store(0)` + notify (lifecycle.rs:874). Reconnect bumps generation; Gen N+1 chunk C → `begin` sets counter=1. Stale task A now finishes → `finish` `fetch_sub` sees previous==1, takes the clamp branch, stores 0 and fires `history_sync_idle_notifier`. A `wait_for_startup_sync` caller (sessions.rs:255) reads 0 and returns `Ok` while chunk C is still unprocessed. Self-healing afterward (C's own finish underflows to clamp) and C still eventually dispatches its event, so no data loss — just premature startup-complete signaling.
*Fix:* tag each MajorSyncTask with the `connection_generation` captured at `begin`; in `finish_history_sync_task` drop the decrement+notify when the task's generation != current, or track counts per-generation and only notify the current-generation waiter.

**[P3] src/features/rotate_key.rs:54 — signed-pre-key rotation and prekey uploads use disjoint locks, allowing transient server-side skey reversion.**
Rotation guards only `signed_pre_key_rotation_lock` (try_lock); every one-time-prekey upload path guards only `prekey_upload_lock` (prekeys.rs:269/807, device.rs:77) — disjoint mutexes. `rotate_signed_pre_key` sends its rotate IQ (line 171) *before* promoting `device.signed_pre_key` locally (line 220), while `upload_pre_keys_pass` reads `device_snapshot.signed_pre_key` (prekeys.rs:482) and re-declares it verbatim (prekeys.rs:654). If a concurrent digest-404 or prekey-low upload reads the pre-promotion snapshot and its IQ lands after the rotate IQ, the server's advertised signed pre-key reverts new_id→old_id while local state says new_id. The authors documented this reversion class (node_io.rs:748) and fixed only the login-upload→rotate ordering on one task, leaving the digest-404 (node_io.rs:836) and server-pushed prekey-low paths uncovered. Narrow/self-healing: old_id is retained (`SIGNED_PRE_KEY_RETENTION=3`, ~3 weekly rotations) so pkmsg against the reverted key stay decryptable, and the next rotation/upload re-advertises a current key well within that window — so no realistic undecryptable-message outcome, just a benign transient. WA Web does not share a lock either but avoids the window by not invoking digestKey at login and chaining digest only after rotation failure.
*Fix:* make rotation and all one-time-prekey upload paths mutually exclusive on a single lock (or have rotation hold `prekey_upload_lock` across its upload+promote) so an upload re-declaring the current skey can never be sent while a rotation is in flight.


## `review-content-crypto-features` — Content Add-Ons & Media: msg-secret, Poll/Edit/Reaction & CDN Crypto

*P1 · group: compliance · raw 7 → deduped 7 → **confirmed 7***

**Standalone msg-secret retention and media resume/failover paths deviate from WA Web, breaking add-ons on old CAG posts, corrupting resumed uploads, and leaving stale/unverified tails on downloads.**

## Content Add-Ons & Media — confirmed findings (7)

**[P2] wacore/src/msg_secret.rs:72 — Text retention (30d) prunes msg-secrets for CAG/community parents that accept time-unbounded reactions & comments.**
`classify`/`classify_from_flags` only special-case bot and poll/event; a plain-text Community Announcement Group post falls to `RetentionClass::Text` → `expires_at = parent_ts + 30d`. Under the default Managed policy the prune sweep deletes the row, so at T+31d an outbound `send_reaction`→`resolve_outgoing_addon_parent` (src/message/msg_secret.rs:776) misses and returns `SendError::InvalidRequest`, inbound `enc_reaction`/`enc_comment` silently fail to decrypt (message_edit.rs:399), and history-sync won't re-seed (`within_seed_horizon`, history_sync.rs:294). WA Web keeps `messageSecret` on the stored message with no time bound and reacts/comments to arbitrarily old CAG posts. Fix: add a retention class for reaction/comment-capable (CAG/community, non-bot, non-poll) parents that is never pruned — or extend the Text horizon far past the 20-min edit window — and update the module-doc enumeration (lines 6-9) to list reactions and comments.

**[P2] src/upload.rs:73 — Resumable upload signals the tail with `file_offset`, a param the MMS CDN doesn't recognize.**
On a resumed >5 MiB upload, `build_upload_request` appends only `&file_offset=<offset>` and streams `ciphertext[offset..]`. WA Web exclusively signals a partial via `bytestart`/`byteend` (`byteRange`, ClientFormatUploadUrl.js:27-28); `file_offset` appears nowhere in captured JS. The server treats the request as a fresh full upload of the tail → body hash ≠ encFilehash → 415. Failover masks it in the multi-host case, but a single/last host (or all-hosts-partial) fails permanently. Fix: emit `&bytestart=<offset>&byteend=<ciphertext_len>` and update the `resumable_upload_sends_tail_from_offset` assertion (upload.rs:663).

**[P2] src/download.rs:458 — Streaming/buffered download-to-writer never truncates, leaving a stale garbage tail on host failover.**
`decrypt_stream_to_writer` writes plaintext blocks (wacore/src/download.rs:421) before the MAC check at :439. A host-A 200 with an oversized/corrupt body writes N bytes then fails MAC → generic error → intra-attempt failover to host B reusing the same writer; B seeks to 0, writes the shorter valid payload, seeks to 0, returns `Ok(())`. No `set_len`/truncate exists (only the doc comment at :355), so the file is `max(A,B)` bytes: valid prefix + stale tail, returned as success. The in-memory `download()` path allocates a fresh buffer per attempt for exactly this reason. Fix: track bytes written and `set_len` after a successful decrypt (require `W: Write+Seek+…truncate`), or truncate to 0 at the start of every attempt rather than only seeking.

**[P3] wacore/src/download.rs:421 — Streaming decrypt emits AES-CBC plaintext before HMAC verification.**
Plaintext for all-but-final blocks is written to the caller's writer as chunks arrive; the trailing HMAC is only compared at :437-439. WA Web's `hmacAndDecrypt` (and its per-chunk sidecar-MAC streaming path) verifies before producing any plaintext. On MAC failure a file-backed writer retains unauthenticated bytes; harm is gated behind the documented "discard on Err" contract and the in-memory path already drops its buffer, so this is defense-in-depth. Fix: verify the full HMAC before emitting final block(s), or enforce/couple with the truncate-on-Err fix above.

**[P3] src/download.rs:17 — Per-host `fallback_hostname` is dropped, so failover never tries the CDN's alternate fallback domain.**
`From<&MediaConn>` copies only `h.hostname` into `wacore::download::MediaHost` (which has no fallback field), though `MediaConnHost.fallback_hostname` is parsed. WA Web's `ClientSelectHost` retries `i.fallback` when the primary domain is unreachable and no sibling host entry carries the alt domain. Rust is strictly less resilient in that narrow shape. Fix: carry `fallback_hostname` through `MediaHost` and emit an additional `DownloadRequest` (or in-loop retry) for the fallback domain.

**[P3] src/features/message_edit.rs:504 — Exported `decrypt_secret_encrypted_with_fallback` collapses poll/event kinds to one merged fallback ctx, missing homogeneous both-LID/both-PN attempts.**
For non-reaction kinds it tries only primary + a single `(fb_orig|orig, fb_sender|sender)` ctx. For a cross-addressed addon (creatorPN, editorLID) with swapped fallbacks, both attempts are MIXED pairs; the true both-LID `(creatorLID,editorLID)` and both-PN pairs are never tried, so a both-LID-encrypted poll/event addon fails GCM where WA Web's `decryptAddOn` attempt 1 succeeds. Only reachable via the public helper (internal receive path does its own 4-combo; internal caller hardcodes MESSAGE_EDIT). Fix: route non-reaction kinds through the same explicit 4-combo enumeration used by the reaction/comment branch (lines 470-474), or restrict the merged-ctx shortcut to MESSAGE_EDIT.

**[P3] src/mediaconn.rs:63 — `refresh_media_conn` has no single-flight dedup; concurrent media ops each fire a separate media_conn IQ.**
After reconnect or `invalidate_media_conn()` (None on 401/403), N queued downloads/uploads each read the cache as None and independently `execute(MediaConnSpec::new())` — N redundant `w:m` set IQs, last-write-wins. WA Web wraps its host fetch in `WAMemoizeConcurrent` so concurrent refreshes coalesce. Correctness is unaffected (equivalent fresh data); the cost is redundant IQs / throttling risk in the post-invalidation window. Fix: guard the fetch with a single-flight primitive so concurrent callers await one IQ.


## `review-voip` — VoIP: Unauthenticated Codec/Recv, Crypto KDF & Teardown

*P1 · group: security · raw 2 → deduped 2 → **confirmed 1***

**An on-path relay can permanently desync the E2E-SRTP receiver's rollover counter with a handful of unauthenticated RTP packets, silently killing audio for the rest of the call.**

## VoIP: Unauthenticated Codec/Recv Fuzz, Crypto KDF & Teardown

**1 confirmed finding.**

**[P1] wacore/src/voip/e2e_srtp.rs:165 — `RecvRocTracker::guess_roc` folds ROC/`s_l` state on unauthenticated packets, letting an on-path relay permanently desync the receiver's rollover counter (persistent media DoS), violating RFC 3711 §3.3.1.**

`unprotect_audio` (`session.rs:221-241`) strips the WARP MI tag with a bare slice (`session.rs:225`, doc at :218 literally says "not verified") and never authenticates it — no `verify` exists anywhere in `wacore/src/voip` (`warp.rs:1-56` has only `compute`/`append`). It then calls `guess_roc(header.sequence_number)` (`session.rs:231`) *before* any decrypt/auth. `guess_roc` (`e2e_srtp.rs:146-175`) mutates state unconditionally: the `v==roc` branch advances `s_l` (166-168), the `v==roc+1` branch commits `roc=v; s_l=seq` (169-172). The path is live and gated by nothing: `engine.rs` `on_packet` → `classify_relay_packet` → `on_rtp` (`engine.rs:632`) → `unprotect_audio`; `integrity_key` is used only for STUN MESSAGE-INTEGRITY, so injected RTP reaches `guess_roc` with only a length check (`session.rs:222`) and a parseable header.

Failure scenario: active call, sender true `roc=0`. A malicious relay (exactly the party E2E-SRTP defends against, and it holds HBH keys / terminates DTLS so it can inject onto the media path) sends a short staircase — e.g. from `(roc=k, s_l=0x7FFE)`: inject `seq=0xFFFE` (→ `v==roc`, `s_l=0xFFFE`), then `seq=0x7FFD` (`s_l-seq=0x8001>0x8000` → `v==roc+1`, commits `roc=k+1`). Two packets per increment, no keys needed. After N cycles receiver `roc=N` with `s_l` low, so the sender's real low-seq frames satisfy `seq-s_l<0x8000` → `v==roc=N`. Each then builds its CTR IV with the wrong ROC (`build_e2e_rtp_iv`, `e2e_srtp.rs:88`; `packet_index = roc<<16 | seq`) → garbage plaintext for every remaining audio frame. Because `guess_roc` self-heals only ±1 (154-172), any offset ≥2 is permanent → silent, total media DoS. (Note the finder's original `0x8100/0x0100` staircase hits the `== 0x8000, not > 0x8000` boundary and does not bump; the corrected staircase above does.)

Fix: constant-time-verify the WARP MI tag (`compute_warp_mi_tag` over the tag range) before touching `guess_roc`, and split `guess_roc` into a pure `estimate(seq) -> v` (used to build the IV for decrypt/verify) and a separate `commit(v, seq)` invoked only after the MI tag authenticates — the post-authentication update RFC 3711 §3.3.1 mandates.

Severity note: this is availability-only — SFrame/GCM still authenticates audio content, so no forged audio is accepted (no confidentiality/integrity break). One verifier argues P3 on the grounds that the only parties who can inject onto `on_rtp` (the relay or an active client↔relay MITM) already have trivial DoS by dropping media. The added harm is real but bounded: it is a *persistent* poisoning from a few packets versus transient jamming, and it closes a concrete RFC 3711 §3.3.1 hardening gap that would matter if the transport/auth model tightens. Prioritize the fix as a correctness/hardening item even if you down-rank the DoS impact.

