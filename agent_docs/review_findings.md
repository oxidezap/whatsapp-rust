# Adversarial Review — Consolidated Findings Index

> Master index of every confirmed finding across all three review tiers, ranked by severity.
> Each was surfaced by a per-lens finder and survived 3 independent adversarial skeptics.
> Full detail (failure scenario, evidence, fix) is in the per-tier reports linked below.

**Per-tier reports**: [P0](review_findings_P0.md) · [P1](review_findings_P1.md) · [P2](review_findings_P2.md) · plan: [adversarial_review_plan.md](adversarial_review_plan.md)

## By severity

| P0 | P1 | P2 | P3 | Total |
|---|---|---|---|---|
| 1 | 12 | 20 | 20 | **53** |

## By group

| Group | Count |
|---|---|
| compliance | 24 |
| concurrency | 10 |
| bug | 6 |
| security | 5 |
| overhead | 4 |
| performance | 2 |
| feature-gap | 2 |

## All findings (ranked)

| Sev | Group | File:line | Finding | Track |
|---|---|---|---|---|
| **P0** | concurrency | `src/client/sessions.rs:119` | The offline-sync finisher's publish_offline_sync_live_state widens the message-processing semaphore to 64 and sets offline_sync... | durability-ack-ordering |
| **P1** | bug | `wacore/src/send/group.rs:360` | A single participant's session-setup failure aborts SKDM to the entire group cohort while marking all of them (including own co... | send-path-compliance |
| **P1** | compliance | `wacore/appstate/src/processor.rs:369` | Genesis (had_no_prior_state) patch skips aggregate snapshotMac + patchMac validation, whereas WhatsApp Web validates them for v... | appstate-sync-integrity |
| **P1** | compliance | `wacore/src/protocol/keepalive.rs:36` | Dead-socket watchdog anchors its 20s window to the MOST RECENT send (last_data_sent_ms), whereas WA Web anchors it to the FIRST... | connection-lifecycle-resilience |
| **P1** | compliance | `src/message/receive.rs:1308` | Group (skmsg) decrypt failures other than NoSenderKey/Duplicate are nacked (error=500) instead of retried, permanently dropping... | receive-routing-recovery |
| **P1** | compliance | `src/message/receive.rs:1105` | A PreKeySignalMessage that references a rotated-out signed prekey (SignalProtocolError::InvalidSignedPreKeyId) is NACKed and dr... | receive-routing-recovery |
| **P1** | concurrency | `wacore/src/send/group.rs:393` | The group SKDM pairwise fan-out mutates per-device Signal sessions under only the per-(group,sender) sender_key_lock, never the... | signal-session-concurrency |
| **P1** | concurrency | `src/portable_cache.rs:76` | Capacity-based FIFO eviction in PortableCache::insert_new removes a strongly-held Arc<Mutex> session lock from the map with no ... | signal-session-concurrency |
| **P1** | concurrency | `src/handlers/message.rs:49` | FIFO capacity eviction of a live ChatLane spawns a second concurrent worker for the same chat; the group inbound decrypt path h... | signal-session-concurrency |
| **P1** | security | `wacore/src/prekeys.rs:320` | When validate_adv_with_identity_key returns NoAccountKey (companion device-identity omits account_signature_key AND no stored p... | pairing-prekey-trust |
| **P1** | security | `src/passkey/flow.rs:531` | Passkey skip_handoff_ux (verification-code bypass + auto-confirm) is enabled on every link, including fresh first-time links, b... | pairing-prekey-trust |
| **P1** | security | `wacore/src/voip/e2e_srtp.rs:165` | RecvRocTracker.guess_roc folds ROC/s_l state on unauthenticated packets, letting an on-path attacker permanently desync the rec... | voip |
| **P1** | security | `wacore/binary/src/decoder.rs:504` | read_node_ref recurses through read_content with no depth limit, so a hostile (optionally zlib-compressed) inbound frame overfl... | wire-transport-robustness |
| **P2** | bug | `src/download.rs:458` | Streaming/buffered download_to_writer never truncates the writer, so a host-failover in which the failed host wrote MORE decryp... | content-crypto-features |
| **P2** | bug | `src/message/receive.rs:1348` | The online (immediate) branch of schedule_unknown_device_sync inserts the user into pending_device_sync for dedup but its detac... | receive-routing-recovery |
| **P2** | bug | `src/handlers/message.rs:84` | Newsletter (and status) <message> stanzas are eager-acked at process_node before the chat-lane worker dispatches them, so a wor... | receive-routing-recovery |
| **P2** | bug | `wacore/binary/src/encoder.rs:313` | Encoder's string-JID classifier splits the user at '_' to strip an 'agent', silently truncating any JID whose user contains '_<... | wire-transport-robustness |
| **P2** | compliance | `src/client/app_state.rs:278` | Batched app-state serverSync loop caps at 5 refetch iterations, but WA Web's equivalent loop effectively allows 500, so collect... | appstate-sync-integrity |
| **P2** | compliance | `src/client/offline_resume.rs:119` | The offline batch-pull loop is stopped by a count-derived `pending == 0` guard (and the sibling `processed >= total` in node_io... | connection-lifecycle-resilience |
| **P2** | compliance | `wacore/src/msg_secret.rs:72` | msg-secret retention classifies CAG/community text parents as Text (30-day horizon), but encrypted reactions and community comm... | content-crypto-features |
| **P2** | compliance | `src/upload.rs:73` | Resumable upload sends the tail bytes with a `file_offset` query param, but the WhatsApp MMS server keys partial-append uploads... | content-crypto-features |
| **P2** | compliance | `wacore/src/iq/usync.rs:144` | usync per-subprotocol server backoff (`<error backoff=...>`) is parsed into UsyncSubprotocolError.backoff but never honored — t... | features-surface-overhead |
| **P2** | compliance | `src/features/newsletter.rs:575` | Newsletter verification is matched against uppercase "VERIFIED" while the wire/WA use lowercase, so verified channels are alway... | features-surface-overhead |
| **P2** | compliance | `wacore/src/pair.rs:272` | do_pair_crypto signs the companion device signature with the hosted prefix [6,6] for hosted accounts, but WA Web's generateDevi... | pairing-prekey-trust |
| **P2** | compliance | `wacore/src/send/classify.rs:309` | should_hide_decrypt_fail omits the MESSAGE_UNSCHEDULE protocol-message type that WA Web hides, so an unschedule stanza goes out... | send-path-compliance |
| **P2** | compliance | `src/send/mod.rs:1457` | Group sender-key rotation is triggered by a fabricated chain-iteration threshold (SENDER_KEY_ROTATION_THRESHOLD=1000) that has ... | send-path-compliance |
| **P2** | concurrency | `src/client/app_state.rs:179` | fetch_app_state_with_retry_inner checks initial_app_state_keys_received before registering the notifier listener, so a key-shar... | appstate-sync-integrity |
| **P2** | concurrency | `src/client/lifecycle.rs:402` | The expected_disconnect fast path zeroes the reconnect error counter and continues the run loop with zero sleep, so a server th... | connection-lifecycle-resilience |
| **P2** | concurrency | `src/features/groups.rs:543` | Group-metadata cache uses an unlocked read-modify-write (get -> clone -> await persist -> insert) with no per-group lock, so co... | features-surface-overhead |
| **P2** | concurrency | `src/handlers/message.rs:47` | The same evict-then-recreate duplicate-worker window breaks the per-chat FIFO ordering guarantee the mailbox exists to provide,... | signal-session-concurrency |
| **P2** | overhead | `wacore/src/usync.rs:59` | Duplicate, dead, behaviorally-divergent device-list parser in wacore/src/usync.rs shadows the live DeviceListSpec parser and mi... | features-surface-overhead |
| **P2** | performance | `src/usync.rs:129` | process_device_list_response loads each user's existing DeviceListRecord serially, re-introducing the exact per-user sequential... | features-surface-overhead |
| **P2** | security | `wacore/libsignal/src/protocol/session_cipher.rs:1073` | The wider 25000-step self-session forward-jump ceiling is unlocked by attacker-forgeable identity-key equality, defeating the 2... | signal-crypto-correctness |
| **P3** | bug | `wacore/binary/src/encoder.rs:410` | Encoder classifies any ≤48-char string containing '@' as a JID and emits JID_PAIR, but the decoder rejects JID_PAIR whose serve... | wire-transport-robustness |
| **P3** | compliance | `wacore/src/download.rs:421` | decrypt_stream_to_writer emits AES-CBC plaintext to the caller's writer before the trailing HMAC-SHA256 is verified, diverging ... | content-crypto-features |
| **P3** | compliance | `src/download.rs:17` | Per-host fallback_hostname is dropped when building download requests, so host-failover never tries the CDN's alternate fallbac... | content-crypto-features |
| **P3** | compliance | `src/features/message_edit.rs:504` | Exported decrypt_secret_encrypted_with_fallback collapses poll/event kinds to a single merged fallback ctx, missing WA Web's ho... | content-crypto-features |
| **P3** | compliance | `src/features/newsletter.rs:580` | Newsletter state and viewer role are parsed case-sensitively against lowercase literals whereas WA lowercases first, so any non... | features-surface-overhead |
| **P3** | compliance | `src/prekeys.rs:36` | MAX_PREKEY_ID is treated as an inclusive maximum (16777215) whereas WA Web's PRE_KEY_NON_INCLUSIVE_UPPER_BORDER reserves 167772... | pairing-prekey-trust |
| **P3** | compliance | `src/message/receive.rs:1378` | A decrypted message carrying a device_sent_message wrapper from a non-self sender is only warned, then unwrapped and dispatched... | receive-routing-recovery |
| **P3** | compliance | `src/handlers/notification/mod.rs:76` | Unrecognized notification types are ACKed with a plain <ack> instead of a NACK carrying error=488 (UnrecognizedStanza) as WA We... | receive-routing-recovery |
| **P3** | compliance | `src/client/accessors.rs:494` | No handler is registered for top-level <error> stanzas; they are logged as "unknown top-level node" instead of being parsed lik... | receive-routing-recovery |
| **P3** | compliance | `src/message/receive.rs:1123` | Session (pkmsg/msg) catch-all nacks (error=500) any unclassified Signal decryption error instead of retrying, dropping recovera... | receive-routing-recovery |
| **P3** | compliance | `src/features/signal.rs:315` | create_participant_nodes uses the edit-unaware should_hide_decrypt_fail instead of should_hide_decrypt_fail_for_send, so it can... | send-path-compliance |
| **P3** | compliance | `wacore/noise/src/handshake.rs:195` | verify_server_cert accepts Noise certs with absent issuerSerial/serial fields that WhatsApp Web explicitly rejects, weakening c... | wire-transport-robustness |
| **P3** | concurrency | `src/client/sessions.rs:221` | The history-sync in-flight counter is not generation-fenced, so a stale chunk task enqueued/started in a previous connection de... | connection-lifecycle-resilience |
| **P3** | concurrency | `src/features/rotate_key.rs:54` | Signed-pre-key rotation and one-time-prekey uploads guard the server's advertised signed pre-key under two disjoint locks, so a... | connection-lifecycle-resilience |
| **P3** | feature-gap | `src/mediaconn.rs:26` | media_conn `max_buckets` and per-host `download_buckets` are parsed but dropped before download/upload route selection, so the ... | features-surface-overhead |
| **P3** | feature-gap | `src/features/newsletter.rs:162` | Newsletter admin/management MEX operations are code-generated (doc_ids present in mex_operations.rs) but no Newsletter<'a> meth... | features-surface-overhead |
| **P3** | overhead | `src/mediaconn.rs:63` | refresh_media_conn has no single-flight/in-flight dedup, so concurrent media operations each fire a separate media_conn IQ, div... | content-crypto-features |
| **P3** | overhead | `wacore/src/store/persistence.rs:70` | Second, dead, divergent PersistenceManager in wacore is publicly re-exported but never used, and its get_device_snapshot violat... | features-surface-overhead |
| **P3** | overhead | `src/features/blocking.rs:101` | is_blocked() issues a full blocklist IQ round-trip to the server on every call (no local cache), and the connect-time blocklist... | features-surface-overhead |
| **P3** | performance | `src/portable_cache.rs:77` | PortableCache capacity eviction is insertion-order FIFO (get() never refreshes recency), but the code documents and reasons abo... | features-surface-overhead |

## Highest-priority action list

- **[P0/concurrency]** `src/client/sessions.rs:119` — The offline-sync finisher's publish_offline_sync_live_state widens the message-processing semaphore to 64 and sets offline_sync_completed=true with no generation guard and no lock/permit held, so a concurrent cleanup_connection_state that already reset the state to the next-connection drain values (semaphore=1, completed=false) gets clobbered, leaving the next connection's offline drain running with 64 permits instead of 1 — breaking the single-permit serialization the entire commit_batch durability argument depends on.
  - *Fix*: Make publish_offline_sync_live_state generation-scoped: pass the captured generation through and re-check connection_generation==generation immediately before (and ideally atomically with, e.g. under the semaphore mut...
- **[P1/bug]** `wacore/src/send/group.rs:360` — A single participant's session-setup failure aborts SKDM to the entire group cohort while marking all of them (including own companion devices) has_key=true; own companions are then permanently keyless because retry-driven mark_forget_sender_key excludes own devices.
  - *Fix*: Do not gate the SKDM fan-out on session_plan being Some for the whole cohort; mirror WA Web by attempting per-device SKDM encryption for every device that already has a session even when some sessions failed (isolate ...
- **[P1/compliance]** `wacore/appstate/src/processor.rs:369` — Genesis (had_no_prior_state) patch skips aggregate snapshotMac + patchMac validation, whereas WhatsApp Web validates them for version-1 patches on an empty ltHash, allowing a malicious server to seed a curated baseline undetected.
  - *Fix*: For a genesis (version-1) patch, do not skip the aggregate MAC checks: still validate the provided snapshot_mac and patch_mac against the freshly computed ltHash (over an empty baseline) and treat a missing snapshot_m...
- **[P1/compliance]** `wacore/src/protocol/keepalive.rs:36` — Dead-socket watchdog anchors its 20s window to the MOST RECENT send (last_data_sent_ms), whereas WA Web anchors it to the FIRST send after the last receive, so a half-open socket that keeps emitting outgoing traffic is never detected as dead.
  - *Fix*: Track a separate 'first send since last receive' timestamp (reset to 0 on any receive, set only if currently 0 on send) and evaluate is_dead_socket against that anchor, matching onOrBefore. Do not use the latest-send ...
- **[P1/compliance]** `src/message/receive.rs:1308` — Group (skmsg) decrypt failures other than NoSenderKey/Duplicate are nacked (error=500) instead of retried, permanently dropping recoverable group messages on sender-key desync.
  - *Fix*: For skmsg decrypt failures that are Signal-crypto errors (SignatureValidationFailed, InvalidMessage, InvalidSenderKeySession, UnrecognizedMessageVersion), send a retry receipt via handle_decrypt_failure (e.g. RetryRea...
- **[P1/compliance]** `src/message/receive.rs:1105` — A PreKeySignalMessage that references a rotated-out signed prekey (SignalProtocolError::InvalidSignedPreKeyId) is NACKed and dropped instead of being retried, diverging from WA Web which classifies every Signal decryption error as SignalRetryable and sends a retry receipt.
  - *Fix*: Add SignalProtocolError::InvalidSignedPreKeyId (and other libsignal decrypt errors that WA Web wraps as SignalDecryptionError, e.g. SignatureValidationFailed / InvalidSignedPreKeyId / signed-prekey mismatch) to the re...
- **[P1/concurrency]** `wacore/src/send/group.rs:393` — The group SKDM pairwise fan-out mutates per-device Signal sessions under only the per-(group,sender) sender_key_lock, never the per-device session_lock_for that the DM path uses, so a concurrent DM (or a concurrent send to a different group) advances the same pairwise ratchet concurrently.
  - *Fix*: On the group send path, acquire the per-device session_lock_for each SKDM target device (in cmp_for_lock_order order, consistent with the DM path's build_session_lock_keys/session_mutexes_for) for the duration of the ...
- **[P1/concurrency]** `src/portable_cache.rs:76` — Capacity-based FIFO eviction in PortableCache::insert_new removes a strongly-held Arc<Mutex> session lock from the map with no strong_count guard, so a later get_with_by_ref for the same Signal address mints a fresh mutex and lets two writers advance the same Signal/sender-key chain concurrently.
  - *Fix*: Do not FIFO-evict entries whose value Arc is still strongly held (skip pop_first candidates with Arc::strong_count>1, or make coordination-lock caches unbounded / evict only via run_pending_tasks with the existing str...
- **[P1/concurrency]** `src/handlers/message.rs:49` — FIFO capacity eviction of a live ChatLane spawns a second concurrent worker for the same chat; the group inbound decrypt path holds no sender-key lock, so two workers race the sender-key ratchet read-modify-write, corrupting/regressing the chain.
  - *Fix*: Either make chat lanes non-evictable while their worker is live (e.g. worker exits and is joined before the entry can be reused), or acquire sender_key_lock(sender_key_name) across the load/mutate/store in process_gro...
- **[P1/security]** `wacore/src/prekeys.rs:320` — When validate_adv_with_identity_key returns NoAccountKey (companion device-identity omits account_signature_key AND no stored primary identity), the prekey-bundle parser logs and proceeds, accepting an unverified companion identity key where WhatsApp Web hard-rejects the bundle.
  - *Fix*: Match WA Web: treat NoAccountKey as a hard reject for companion devices (device != 0) — return Err instead of proceeding — since the ADV chain is the only anchor binding a companion's fetched identity to the contact's...
- **[P1/security]** `src/passkey/flow.rs:531` — Passkey skip_handoff_ux (verification-code bypass + auto-confirm) is enabled on every link, including fresh first-time links, because it is gated on the always-present adv_secret_key instead of on prior-link state.
  - *Fix*: Only derive/attach the handoff key when the device actually has a prior linked identity (e.g. snapshot.account.is_some() or snapshot.pn.is_some()/lid.is_some()); leave handoff_key None on a fresh device so skip_handof...
- **[P1/security]** `wacore/src/voip/e2e_srtp.rs:165` — RecvRocTracker.guess_roc folds ROC/s_l state on unauthenticated packets, letting an on-path attacker permanently desync the receiver's rollover counter (persistent media DoS), violating RFC 3711 §3.3.1's post-authentication update rule.
  - *Fix*: Verify the WARP MI tag (constant-time compare of compute_warp_mi_tag over the tag range) before calling guess_roc, and split guess_roc into a pure 'estimate v' step used for the decrypt/verify and a separate 'commit(v...
- **[P1/security]** `wacore/binary/src/decoder.rs:504` — read_node_ref recurses through read_content with no depth limit, so a hostile (optionally zlib-compressed) inbound frame overflows the native stack and aborts the process.
  - *Fix*: Thread a depth counter through read_node_ref/read_content and return BinaryError once a sane cap (e.g. 128, matching realistic stanza nesting) is exceeded, before recursing.
