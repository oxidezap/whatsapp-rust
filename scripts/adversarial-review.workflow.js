export const meta = {
  name: 'adversarial-review',
  description: 'Parameterized adversarial-review harness for whatsapp-rust. Select a track by id (or a whole tier/group) via args; fans out one finder agent per lens (with captured-js compliance diffing where relevant), dedups, adversarially verifies every candidate with independent refuters, and returns only CONFIRMED findings plus a per-track report. Tracks and lenses come from agent_docs/adversarial_review_plan.md.',
  phases: [
    { title: 'Find', detail: 'one finder agent per review lens' },
    { title: 'Verify', detail: 'independent skeptics adversarially confirm/refute each finding' },
    { title: 'Report', detail: 'synthesize surviving findings into a per-track report' },
  ],
}

const REPO = '/home/user/whatsapp-rust'
// Captured WhatsApp-Web JS reference (wire-format ground truth). Indexes:
//   exports-map.json (module -> file), dep-graph.json (deps/dependents), metadata.json (gk gates).
const CJS = '/tmp/claude-0/-home-user-whatsapp-rust/dba2e137-f233-5f8e-9f7d-54da592a62b2/scratchpad/capturedjs/captured-js'

const SPECS = [
  {
    id: "review-signal-crypto-correctness",
    name: "Signal Crypto Core Correctness & KDF Compliance",
    priority: "P0",
    group: "compliance",
    scope: "crypto-signal: session_cipher.rs, ratchet.rs, ratchet/keys.rs, group_cipher.rs, sender_keys.rs, session.rs, protocol.rs, aes_gcm.rs, curve.rs, consts.rs",
    objective: "Confirmed byte-exact KDF/MAC/X3DH parity with captured-js and confirmed refutation-or-fix of: zero-key fallback reachability, skipped-key eviction vs replay boundary, deferred one-time-prekey double-use, partial-rollback correctness, self vs peer forward-jump misclassification, and hand-rolled GHASH correctness.",
    capturedJs: ["GroupCipher.js", "LibraryConfig.js + whatsmeow + libsignal-upstream"],
    verify: "Cross-check constants against three references (captured-js authoritative); write refuters that attempt to construct a Serialized variant bypassing from_pb, an evicted-then-replayed skipped key, and a peer classified as self; run GCM against NIST KAT beyond in-file tests; compile/test both simd and scalar builds.",
    lenses: [
      "byte-level KDF label + DH-order + MAC-truncation diff vs captured-js",
      "fail-open KDF paths (Serialized zero-key, all-zero accept)",
      "skipped-key eviction re-derivation/replay after MAC-key cache drain",
      "prekey lifecycle: consumed_prekey_id delete ordering vs session durability",
      "MAC/signature-before-decrypt ordering on every branch (padding-oracle)",
      "self-session classification unlocking the 25000-step ceiling",
    ],
  },
  {
    id: "review-signal-session-concurrency",
    name: "Signal Session Locking Discipline & Lock-Cache Eviction",
    priority: "P0",
    group: "concurrency",
    scope: "send/group.rs, send/dm.rs, send/encrypt.rs, features/signal.rs, group_cipher.rs; client/adapters.rs, portable_cache.rs, handlers/message.rs; message_enqueue/session_locks",
    objective: "Prove (or produce a failing interleaving for) the two catastrophic guarantees: no two writers ever advance the same Signal/sender-key chain concurrently, and no live coordination lock can be evicted to mint a duplicate.",
    capturedJs: [],
    verify: "For each hypothesis construct the minimal concurrent scenario and show the shared-mutable-state access is unserialized; confirm whether the moka-style Cache can evict a key whose Arc is still strongly held and whether a subsequent get returns a different Arc (the core question); prefer a runnable stress repro over argument.",
    lenses: [
      "cross-path session mutation: SKDM chain-lock vs DM per-device-lock on a shared device",
      "group_encrypt load->advance->store atomicity under sender_key_lock scope",
      "LRU eviction of a strongly-held Arc<Mutex> returning a fresh lock from get_with_by_ref",
      "chat-lane worker lifecycle: evict-then-recreate spawning a second worker; enqueue into dead-worker channel",
      "pkmsg_would_be_emitted take/restore session on retry paths without the main send lock",
      "decrypt_group_message documented-unlocked vs concurrent group encrypt",
    ],
  },
  {
    id: "review-durability-ack-ordering",
    name: "Durable-Before-Ack Invariant (Receive Commit Batch + Offline Sync + Prekey Store)",
    priority: "P0",
    group: "concurrency",
    scope: "message/commit_batch.rs, message/receive.rs (ack gating), message/durability.rs, client/sessions.rs, receipt.rs, store/signal_cache.rs (consumed-prekey), store/persistence_manager.rs (saver ordering)",
    objective: "Confirmed absence (or a concrete instance) of any path that acks/commits a stanza whose Signal/session/prekey state is only in cache, and of any drain\u2192live transition that leaves the semaphore stuck or persists a rowless ratchet advance.",
    capturedJs: [],
    verify: "Inject failure at each individual backend write and check the collection/session/queue recovers vs permanently diverges; specifically resolve the open question of what calls complete_offline_sync on the normal path; treat any 'acked but cache-only' window as confirmed data-loss.",
    lenses: [
      "consumed-prekey durable/defer/ask decision + clear()-drops-buffer atomicity",
      "commit_batch deferred-live-transition + ReinsertGuard epoch/generation gaps",
      "offline-sync finisher: end-marker vs timeout path actually widening semaphore + flushing receipts only after tail durable",
      "dual teardown entry points (cleanup_connection_state vs disconnect/reconnect) double-settle / lost clear",
      "background saver dirty-flag vs snapshot-rebuild ordering on shutdown",
    ],
  },
  {
    id: "review-appstate-sync-integrity",
    name: "App-State (syncd) LTHash, Anti-Tampering & MAC Compliance",
    priority: "P0",
    group: "compliance",
    scope: "appstate/processor.rs, hash.rs, lthash.rs, decode.rs, encode.rs, keys.rs, schemas.rs; appstate_sync.rs; client/app_state.rs; message/special.rs",
    objective: "Confirmed byte-exact MAC/key layout and anti-tampering accounting vs captured-js, plus resolution of the genesis-MAC-skip threat model and the fresh-pairing missing-key race.",
    capturedJs: ["WAWebSyncdAntiTampering", "SyncdCollectionHandler", "MutationKeyApi.Crypto", "ValidateMutations", "LtHash.js"],
    verify: "Table-drive every mutation ordering and diff net LTHash + persisted MAC store against WA-Web semantics; attempt a forged genesis patch under validate_macs=true; run KAT for MAC generators and SIMD/scalar parity across buffer sizes.",
    lenses: [
      "update_hash suppression vs process_patch in-patch overlay for all SET/REMOVE orderings",
      "genesis had_no_prior_state aggregate-MAC skip abuse (server-forced empty reset + crafted baseline)",
      "content/snapshot/patch MAC byte layout + AD-length packing vs captured",
      "stale-snapshot / version-monotonicity gates vs 5-iter batched + 500-iter single loops",
      "missing-key request/wait notifier race (false 'synced' cancelling watchdog)",
      "SIMD vs scalar LTHash lane equivalence + pre-keyed HKDF-extract identity",
    ],
  },
  {
    id: "review-receive-routing-recovery",
    name: "Receive Error-Recovery Matrix, Protocol-Message Gating & Inbound Routing",
    priority: "P0",
    group: "bug",
    scope: "message/receive.rs, retry.rs, dispatch.rs; message_processing.rs; protocol/retry.rs, nack.rs; handlers/router.rs, node_io.rs, notification/*, message.rs, ib.rs",
    objective: "Confirmed every session_outcome \u00d7 group \u00d7 bot combination either acks-or-retries exactly once, correct retry-reason/nack mapping vs captured-js, no self-only protocol message reachable from a spoofed DSM, and enumeration of undispatched/log-only inbound stanzas.",
    capturedJs: ["DecryptionHandler", "RetryRequest", "NackFromStanza", "HandleLoggedInStanza"],
    verify: "Enumerate the outcome-flag cartesian product and trace each to a definite ack/retry terminal; pin nack codes against NackFromStanza.js; attempt to route a protocol message through a peer-authored DSM; grep captured-js HandleLoggedInStanza for the full top-level/notification case list and diff.",
    lenses: [
      "ack/retry/nack matrix holes (stuck offline queue or double-ack)",
      "spoofed device_sent_message smuggling key-share/history-sync/LID-mapping past is_from_me",
      "retry-reason classification BadMac/InvalidPreKeyId/SessionNotFound vs SignalRetryable",
      "chat-lane generation/enqueue-vs-worker-exit stranded message",
      "notification type fall-through + mediaretry log-only stub + missing top-level <error> handler",
      "detached-spawn cancellation inconsistency (prekey_low/identity-change/edge_routing unguarded) + non-serialized cache patches vs message worker",
    ],
  },
  {
    id: "review-wire-transport-robustness",
    name: "Binary Codec & Noise Transport: Hostile-Frame Robustness + Wire Compliance",
    priority: "P1",
    group: "security",
    scope: "binary/decoder.rs, encoder.rs, jid.rs, token.rs, tokens.json, zlib_pool.rs, attrs.rs; noise/state.rs, handshake.rs, framing.rs, edge_routing.rs; src/handshake.rs, socket/noise_socket.rs, client/node_io.rs",
    objective: "Confirmed bounds on decode recursion/allocation from untrusted frames, byte-exact JID + token-dictionary + handshake compliance, and resolution of decrypt-fail read-loop desync + cert-verify feature reachability.",
    capturedJs: ["WA/Wap/Dict.js", "WAFrameSocket/ChatSocket/WANoise"],
    verify: "Extend/audit the existing fuzz target to cover encode+JID variants and prove a depth/alloc cap exists before unmarshal is reached from the socket; diff token tables and JID domain bytes against captured-js; confirm feature-unification cannot enable the cert-skip in any default build.",
    lenses: [
      "unbounded read_node_ref recursion + speculative with_capacity from wire counts + inbound frame-size cap",
      "dual JID parsers ('.' vs '_' agent, domain_type-as-agent-byte) and AD_JID domain-type asymmetry vs decodeJidU",
      "full encode(decode)==id roundtrip incl packed nibble/hex odd/even, BINARY_20 boundary, empty-user JID",
      "SIMD vs scalar pack/unpack equivalence; double_byte dictionary full match + DICT_VERSION lockstep",
      "cert-chain bypass feature reachability + issuer_serial==0 unwrap_or(0); danger-skip transitive enablement",
      "read-loop AEAD-fail silent node drop vs forced reconnect; sender-task backpressure vs caller-held locks",
    ],
  },
  {
    id: "review-send-path-compliance",
    name: "Send-Path Wire Compliance: phash, Namespace Alignment & Warm-Skip Correctness",
    priority: "P1",
    group: "compliance",
    scope: "send/mod.rs, send/group.rs, dm.rs, classify.rs, peer.rs, status.rs, resolved_devices.rs, actions.rs, tctoken_lifecycle.rs; messages.rs (phash)",
    objective: "Confirmed phash byte-match with server phashV2 across LID/agent/device formatting, no mixed PN/LID fanout reaches the wire, and no warm-skip memoization leaves a group member silently keyless.",
    capturedJs: ["WAWeb/Phash/Utils.js", "ParticipantStore", "MsgCreateFanoutStanza", "RetryMsgJob", "E2EProtoUtils"],
    verify: "Diff phash inputs byte-for-byte against captured-js for LID-with-agent and hosted-filtered cases; enumerate own-self/downgrade DM cases for namespace purity; construct a device-add scenario that keeps Arc+generation stable to trigger a false warm-skip.",
    lenses: [
      "participant_list_hash string form (push_ad_to agent/device) + membership set vs phashV2([].concat(A,[B]))",
      "DM namespace alignment (own-device PN->LID rewrite) \u2014 residual mixed-namespace = server 400 (#941)",
      "skdm_warm_memo Arc-ptr+generation false-skip delivering undecryptable skmsg",
      "failed-SKDM devices marked has_key relying on a retry receipt that may never fire",
      "classify.rs type/mediatype + decrypt-fail-hide consistency across DM/SKDM/skmsg (silent revoke drop)",
      "SENDER_KEY_ROTATION_THRESHOLD=1000 guessed constant vs real expiry semantics",
    ],
  },
  {
    id: "review-pairing-prekey-trust",
    name: "Pairing/Auth Trust Root: ADV Signatures, Prekey Watermarks & Passkey Rotation",
    priority: "P1",
    group: "security",
    scope: "pair.rs + wacore/pair.rs, pair_code.rs + wacore/pair_code.rs, prekeys.rs + wacore/prekeys.rs, adv.rs, passkey/flow.rs+mod.rs, companion_reg.rs",
    objective: "Confirmed the ADV verification chain cannot be tricked into accepting a forged/trimmed companion identity, prekey 24-bit watermark math never reuses/regresses an id, and passkey deferred-rotation/auto-confirm cannot commit a wrong adv_secret.",
    capturedJs: ["Adv/SignatureApi.js", "DeviceLinkingApi", "PreKeysJob/getOrGenPreKeys", "Smax passkey modules"],
    verify: "Exhaustively re-derive watermark boundaries near MAX_PREKEY_ID for reuse/regression; attempt an account-key-stripped bundle to force soft-accept; diff HKDF info strings and prefix branching against captured-js; confirm server_has_prekeys durability at pair-success commit.",
    lenses: [
      "do_pair_crypto: HMAC-over-details verified before trusting account_signature_key; single prefix family",
      "validate_adv NoAccountKey soft-accept abuse via stripped account key",
      "plan_prekey_upload/get_or_gen_single/mark_single 24-bit wrap + window-collapse off-by-one",
      "dual-format prekey id parse (3-byte int vs hex) misinterpretation",
      "passkey deferred ADV rotation + skip_handoff_ux auto-confirm on stale/forged continuation",
      "pair_code stage-2 HKDF info strings + combined-secret ordering + 180s window",
    ],
  },
  {
    id: "review-connection-lifecycle-resilience",
    name: "Connection Lifecycle: Reconnect Storms, Keepalive, Offline-Resume & History-Sync",
    priority: "P1",
    group: "concurrency",
    scope: "client/lifecycle.rs, keepalive.rs, client/node_io.rs (handle_success/stream_error), client/offline_resume.rs, history_sync.rs, bot.rs, client.rs (backoff), client/device_topology.rs, shutdown.rs",
    objective: "Confirmed the expected-disconnect fast path cannot become a no-backoff spin, dead-socket detection latency is acceptable, the offline pull-batch matches WAWebOfflineHandler (no dropped/duplicated batch), and detached history-sync/post-login tasks are generation-fenced.",
    capturedJs: ["Offline/Handler.js", "ChatSocket fibonacci/resetDelay", "deadSocketTimer", "StreamError"],
    verify: "Simulate a server emitting repeated 'expected' disconnects and confirm backoff still engages; grep Offline/Handler.js $13 continuation call site to settle the threshold; verify every post-login await re-checks connection_generation before persisting/sending.",
    lenses: [
      "expected_disconnect resets errors + continues with no backoff (weaponizable 515/dead-socket loop)",
      "offline pull-batch continuation threshold (first-arrival-while-pending vs remaining<=C) + no drop after primer",
      "keepalive randomized 15-30s + 20s deadline per-tick detection lag on half-open socket",
      "handle_success detached post-login tree: stale-generation persist / stale signed-prekey upload",
      "history-sync detached chunk tasks + in-flight counter reset elsewhere (stuck notifier / leaked buffer)",
    ],
  },
  {
    id: "review-content-crypto-features",
    name: "Content Add-Ons & Media: msg-secret Envelopes, Poll/Edit/Reaction & CDN Crypto",
    priority: "P1",
    group: "compliance",
    scope: "secret_enc_addon.rs, poll.rs, reaction.rs, message_edit.rs, msg_secret.rs, features/{polls,message_edit,reaction,status,events,rotate_key,chatstate}.rs, client/messaging.rs; upload.rs+wacore/upload.rs, download.rs+wacore/download.rs, mediaconn.rs, media_retry.rs, features/media_reupload.rs",
    objective: "Confirmed add-on HKDF/AAD/use-case parity, correct edit/poll fallback across LID/PN namespaces, safe msg-secret retention horizons, and resolution of media resumable-offset corruption + streaming partial-plaintext + failover/integrity gaps.",
    capturedJs: ["CaseSecret.js", "Reaction/Poll/Edit/Addon modules", "Upload/Manager.js", "CreateMediaKeys", "CryptoMediaRetry"],
    verify: "Diff HKDF/use-case literals against CaseSecret.js; construct a mixed-namespace edit to expose the fallback gap; model a host-failover-after-resume producing a spliced body; confirm every streaming-decrypt caller discards the writer on MAC failure.",
    lenses: [
      "secret_enc_addon HKDF info ordering + AadMode StanzaAndSender only for PollVote/EventResponse",
      "message-edit fallback asymmetry (1 merged ctx) vs reaction/comment 3-combo LID/PN",
      "CAG reaction gate is_default_sub_group as isCag proxy; msg-secret retention horizons vs offline window",
      "resumable upload cross-host/stale offset splicing a corrupt CDN blob",
      "streaming decrypt writing plaintext before trailing MAC verified + no writer truncate on retry",
      "plaintext file_sha256 not verified; per-host fallback_hostname/media_conn single-flight gaps; media key app-info strings per MediaType",
    ],
  },
  {
    id: "review-voip",
    name: "VoIP: Unauthenticated Codec/Recv Fuzz, Crypto KDF & Teardown",
    priority: "P1",
    group: "security",
    scope: "wacore/voip/engine.rs, e2e_srtp.rs, hbh_srtp.rs, sframe.rs, session.rs, registry.rs, stun.rs, relay_parse.rs, mlow/*; src/voip/facade.rs, driver.rs; handlers/call.rs",
    objective: "Prove the MLow decoder + on_rtp path never panics/over-allocates/loops on arbitrary bytes, confirm SRTP/SFrame KDF/nonce parity beyond the single KAT, and flag the unwired abort_all teardown leak and the inline-codec spawn_blocking violation.",
    capturedJs: [],
    verify: "Run a fuzzer against the decoder/parse entry points (opt-in feature build); property-test ROC against an independent RFC3711 impl; grep for any abort_all call site; measure worst-case frame time vs consent/keepalive deadlines.",
    lenses: [
      "MLow decode / on_rtp panic/alloc/loop on unauthenticated AES-CTR recv",
      "SRTP/HBH/SFrame KDF labels + 16-byte LE nonce vs captured wasm (multiple counters/participant-id shapes)",
      "RecvRocTracker.guess_roc under reorder/loss/wrap vs reference",
      "CallRegistry::abort_all never wired into disconnect (ghost call + media-task leak)",
      "codec encode/decode inline on async driver starving STUN keepalive (spawn_blocking rule)",
    ],
  },
  {
    id: "review-features-surface-overhead",
    name: "Feature-Surface Gaps, IQ/usync & Group/MEX + Store Overhead",
    priority: "P2",
    group: "feature-gap",
    scope: "iq/usync.rs + wacore/usync.rs + src/usync.rs, features/{contacts,blocking,presence,profile}.rs, iq/{privacy,tctoken,blocklist,contacts,business}.rs; features/{groups,community,newsletter,comments,labels,mex}.rs, iq/{groups,mex,mex_operations}.rs, stanza/groups.rs; store/{persistence.rs,ab_props.rs}, lid_pn_cache.rs, sender_key_device_cache.rs, cache_config.rs, portable_cache.rs",
    objective: "Enumerate WA-Web capabilities parsed-but-unused or absent, kill duplicate/divergent parsers, and quantify overhead wins (FIFO-vs-LRU, unbounded caches, sequential DB reads, duplicate PersistenceManager, stale MEX doc_ids).",
    capturedJs: ["ExistsJob/SyncDeviceListApi/Usync-Backoff", "SetPrivacyJob", "QueryGroupJob", "xwa2_newsletter*", "MEX doc_ids"],
    verify: "Determine the live device-list parser and prove the other is dead; grep captured-js *Job modules for Rust-less capabilities; confirm which parsed fields have no consumer; measure/confirm cache eviction semantics and the sequential-DB-read asymmetry on the group-send hot path.",
    lenses: [
      "duplicate device-list parsers (DeviceListSpec vs wacore/usync) + duplicate PersistenceManager dead/divergent",
      "parsed-but-ignored protocol fields (usync backoff, media_conn fallback/max_buckets, privacy dhash 409 retry)",
      "group-cache unlocked read-modify-write + phash cache/metadata-API coherence",
      "MEX lenient serde_json defaults masking schema drift + pinned stale doc_ids",
      "feature-gaps: undispatched notification types, newsletter admin ops, group join-approval, is_blocked no-cache IQ storm",
      "overhead: FIFO eviction of hot keys, unbounded lid_pn growth, non-CacheConfig signal-cache 2000 bound, sequential per-user load_device_record, per-command Device clone",
    ],
  },
]

// ------------------------------------------------------------------ helpers
const GROUPS = ['performance', 'overhead', 'bug', 'compliance', 'feature-gap', 'security', 'concurrency']
const range = (n) => Array.from({ length: n }, (_, i) => i)
const sevRank = (s) => ({ P0: 0, P1: 1, P2: 2, P3: 3 })[s] ?? 2
const confRank = (c) => ({ high: 2, medium: 1, low: 0 })[c] ?? 1
const dkey = (f) => (f.file || '?') + '|' + String(f.summary || '').toLowerCase().replace(/\s+/g, ' ').slice(0, 70)
const shortFile = (f) => String(f.file || '?').split('/').slice(-1)[0].slice(0, 20)
function majoritySeverity(votes, fallback) {
  const counts = {}
  for (const v of votes) { if (v && v.corrected_severity) counts[v.corrected_severity] = (counts[v.corrected_severity] || 0) + 1 }
  let best = fallback, bestN = -1
  for (const k of Object.keys(counts)) { if (counts[k] > bestN) { best = k; bestN = counts[k] } }
  return best
}

function selectSpecs(a) {
  if (!a) return []
  if (typeof a === 'string') { if (a === 'all') return SPECS.slice(); const s = SPECS.find((x) => x.id === a); return s ? [s] : [] }
  if (Array.isArray(a)) return a.map((id) => SPECS.find((x) => x.id === id)).filter(Boolean)
  if (a.all === true) return SPECS.slice()
  if (a.ids) return a.ids.map((id) => SPECS.find((x) => x.id === id)).filter(Boolean)
  if (a.id) { if (a.id === 'all') return SPECS.slice(); const s = SPECS.find((x) => x.id === a.id); return s ? [s] : [] }
  if (a.tier) return SPECS.filter((x) => x.priority === a.tier)
  if (a.group) return SPECS.filter((x) => x.group === a.group)
  return []
}

// ------------------------------------------------------------------ schemas
const FINDINGS_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object', additionalProperties: false,
        properties: {
          file: { type: 'string', description: 'repo-relative path' },
          line: { type: 'integer', description: '1-indexed anchor line, or 0 if unknown' },
          group: { type: 'string', enum: GROUPS },
          severity: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
          summary: { type: 'string', description: 'one-sentence statement of the defect' },
          evidence: { type: 'string', description: 'concrete code quote / file:line refs / captured-js module path supporting the claim' },
          failure_scenario: { type: 'string', description: 'exact input / interleaving / byte sequence -> wrong outcome' },
          fix_hint: { type: 'string' },
          confidence: { type: 'string', enum: ['low', 'medium', 'high'] },
        },
        required: ['file', 'group', 'severity', 'summary', 'failure_scenario', 'confidence'],
      },
    },
  },
  required: ['findings'],
}

const VERDICT_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    refuted: { type: 'boolean', description: 'true if this is NOT a real issue (code matches spec / guarded / dead code / not triggerable / misread)' },
    corrected_severity: { type: 'string', enum: ['P0', 'P1', 'P2', 'P3'] },
    reasoning: { type: 'string', description: 'what you independently verified in the actual code' },
    repro_note: { type: 'string', description: 'the concrete trigger you constructed, or why none exists' },
  },
  required: ['refuted', 'reasoning'],
}

const REPORT_SCHEMA = {
  type: 'object', additionalProperties: false,
  properties: {
    headline: { type: 'string', description: 'the single most important takeaway for this track' },
    confirmed_count: { type: 'integer' },
    markdown: { type: 'string', description: 'engineer-facing report section: findings ranked by severity, each with file:line, failure scenario, and concrete fix' },
  },
  required: ['headline', 'confirmed_count', 'markdown'],
}

// ------------------------------------------------------------------ prompts
function finderPrompt(spec, lens) {
  const compliance = spec.group === 'compliance' || spec.capturedJs.length > 0
  return `You are a FINDER in an adversarial code review of whatsapp-rust (a Rust reimplementation of WhatsApp Web). Repo root: ${REPO}. Captured WhatsApp-Web JS (wire-format ground truth): ${CJS} — grep ${CJS}/exports-map.json for a module name to find its file.

Review track: ${spec.name}  [${spec.priority} / ${spec.group}]
Scope: ${spec.scope}
Objective: ${spec.objective}
${spec.capturedJs.length ? 'Relevant captured-js references: ' + spec.capturedJs.join(', ') : ''}

YOUR LENS (hunt ONLY through this specific angle): ${lens}

Instructions:
- Read the actual code with Grep/Glob/Read. Go DEEP on the exact functions your lens targets; don't skim.
${compliance ? '- Your lens touches wire/protocol compliance: LOCATE the reference in captured-js (grep exports-map.json for the module, then Read the file) and diff the Rust behavior against it. Cite the captured-js path in evidence.' : '- Reason precisely about the Rust semantics (ownership, locking, async, arithmetic, error branches). Trace the concrete code path.'}
- Report ONLY real, specific, file-anchored candidates found through your lens. Each MUST have a concrete failure_scenario (exact input / interleaving / byte sequence that produces the wrong outcome). No style nits, no speculation.
- A clean result is valuable: if your lens turns up nothing solid, return an empty findings array rather than padding.
- severity: P0 = correctness/security/data-loss/protocol-break; P1 = user-visible bug / real perf / notable compliance gap; P2 = edge/polish; P3 = nit.
- confidence: 'high' ONLY if you traced the exact path and can name the trigger.

Return the structured findings object.`
}

function verifyPrompt(spec, f, vlens, vi) {
  const compliance = spec.group === 'compliance' || spec.capturedJs.length > 0
  return `You are an ADVERSARIAL VERIFIER (independent skeptic #${vi + 1}) in a code review of whatsapp-rust. Repo: ${REPO}. Captured WhatsApp-Web JS ground truth: ${CJS} (grep exports-map.json to locate a module). Your DEFAULT stance is to REFUTE: assume the finding is wrong until the actual code proves otherwise. Confirming a false positive poisons the whole review — be ruthless.

Finding under review (track: ${spec.name}):
  file: ${f.file}:${f.line || '?'}
  group/severity: ${f.group} / ${f.severity}
  summary: ${f.summary}
  claimed failure scenario: ${f.failure_scenario}
  evidence cited by finder: ${f.evidence || '(none)'}
  proposed fix: ${f.fix_hint || '(none)'}

YOUR VERIFICATION LENS: ${vlens}

Do this:
- Open the cited file(s) and read the real code around the claim. Do NOT trust the finder's evidence — re-derive it yourself.
${compliance ? '- Locate the captured-js / whatsmeow reference and check whether the Rust code ACTUALLY deviates, byte/behavior-wise.' : ''}
- Try hard to REFUTE: is it guarded upstream, dead code, an already-held invariant, a misread of the code, or simply not triggerable? Attempt to CONSTRUCT the concrete trigger the finding claims. If you cannot construct it, set refuted=true.
- Set corrected_severity to what the evidence actually supports (downgrade freely).

Return the verdict honestly.`
}

function reportPrompt(spec, confirmed) {
  return `You are writing the report section for review track "${spec.name}" (${spec.priority} / ${spec.group}) of whatsapp-rust. The findings below SURVIVED adversarial verification (independent skeptics could not refute them):

${JSON.stringify(confirmed, null, 2)}

Write a tight, engineer-facing markdown section:
- A one-line headline: the single most important takeaway.
- Findings ranked by severity. Each: a bold line "**[severity] file:line — summary**", then 1-2 lines of concrete failure scenario and a specific fix direction. Merge any duplicates.
- Technical and terse. No preamble, no restating the codebase, no filler.

Return markdown + headline + confirmed_count.`
}

// ------------------------------------------------------------------ per-track harness
const VERIFIERS = (args && args.verifiers) || 3
const MAX_VERIFY = (args && args.maxFindingsPerTrack) || 20
const FINDER_EFFORT = (args && args.finderEffort) || 'high'
const VERIFY_EFFORT = (args && args.verifyEffort) || 'high'
const VERIFY_LENSES = [
  'reproduce-or-refute: construct the concrete failing input / interleaving / byte sequence. If you cannot construct it, refuted=true.',
  'compliance-vs-reference: check the claim against captured-js ground truth / whatsmeow. Refuted if the Rust code actually matches the spec.',
  'false-positive-hunter: find why this is a non-issue — guarded upstream, dead/unreachable code, an invariant already enforced, or a misread of ownership/lock scope.',
]

async function runTrack(spec) {
  const extraLenses = (args && args.extraLenses) || []
  const lenses = spec.lenses.concat(extraLenses)

  const finderResults = await parallel(lenses.map((lens, i) => () =>
    agent(finderPrompt(spec, lens), { label: `find:${spec.id.replace('review-', '')}:${i}`, phase: 'Find', schema: FINDINGS_SCHEMA, effort: FINDER_EFFORT })
  ))
  const raw = finderResults.filter(Boolean).flatMap((r) => r.findings || [])

  const seen = new Map()
  for (const f of raw) {
    const k = dkey(f)
    if (!seen.has(k)) seen.set(k, f)
    else if (sevRank(f.severity) < sevRank(seen.get(k).severity)) seen.set(k, f)
  }
  let deduped = [...seen.values()]
  deduped.sort((a, b) => sevRank(a.severity) - sevRank(b.severity) || confRank(b.confidence) - confRank(a.confidence))
  let dropped = 0
  if (deduped.length > MAX_VERIFY) { dropped = deduped.length - MAX_VERIFY; deduped = deduped.slice(0, MAX_VERIFY) }
  log(`[${spec.id}] ${raw.length} raw -> ${seen.size} deduped${dropped ? ` (verifying top ${MAX_VERIFY} by severity; ${dropped} lower-priority candidates NOT verified this run)` : ''}`)

  const verified = await parallel(deduped.map((f) => () =>
    parallel(range(VERIFIERS).map((vi) => () =>
      agent(verifyPrompt(spec, f, VERIFY_LENSES[vi % VERIFY_LENSES.length], vi), { label: `verify:${shortFile(f)}:${vi}`, phase: 'Verify', schema: VERDICT_SCHEMA, effort: VERIFY_EFFORT })
    )).then((vs) => {
      const votes = vs.filter(Boolean)
      const refuted = votes.filter((v) => v.refuted).length
      const real = votes.length - refuted
      return { finding: f, votes, confirmed: votes.length > 0 && real > refuted, corrected_severity: majoritySeverity(votes, f.severity) }
    })
  ))
  const confirmed = verified.filter(Boolean).filter((v) => v.confirmed).map((v) => ({
    ...v.finding, severity: v.corrected_severity, verifier_notes: v.votes.map((x) => x.reasoning),
    repro: v.votes.map((x) => x.repro_note).filter(Boolean),
  }))
  confirmed.sort((a, b) => sevRank(a.severity) - sevRank(b.severity) || confRank(b.confidence) - confRank(a.confidence))
  log(`[${spec.id}] CONFIRMED ${confirmed.length}/${deduped.length} (${confirmed.filter((c) => c.severity === 'P0').length} P0, ${confirmed.filter((c) => c.severity === 'P1').length} P1)`)

  let report = null
  if (confirmed.length) report = await agent(reportPrompt(spec, confirmed), { label: `report:${spec.id.replace('review-', '')}`, phase: 'Report', schema: REPORT_SCHEMA, effort: 'high' })
  return { id: spec.id, priority: spec.priority, group: spec.group, raw: raw.length, deduped: deduped.length, dropped, confirmed, report }
}

// ------------------------------------------------------------------ entry
const selected = selectSpecs(args)
if (selected.length === 0) {
  log('No track selected. Available tracks:')
  for (const s of SPECS) log(`  [${s.priority}/${s.group}] ${s.id}`)
  log('Pass args: { id: "review-..." } | { ids: [...] } | { tier: "P0" } | { group: "compliance" } | { all: true }')
  log('Optional: { verifiers: 3, maxFindingsPerTrack: 20, finderEffort: "high", verifyEffort: "high", parallelTracks: false, extraLenses: [...] }')
  return { error: 'no-selection', available: SPECS.map((s) => ({ id: s.id, priority: s.priority, group: s.group })) }
}
log(`Adversarial review — ${selected.length} track(s): ${selected.map((s) => s.id).join(', ')} | ${VERIFIERS} verifiers/finding, top ${MAX_VERIFY}/track`)

let results = []
if (args && args.parallelTracks) {
  results = (await parallel(selected.map((sp) => () => runTrack(sp)))).filter(Boolean)
} else {
  for (const sp of selected) results.push(await runTrack(sp))
}

return {
  tracks: results.map((t) => ({ id: t.id, priority: t.priority, group: t.group, confirmed: t.confirmed, report: t.report, raw: t.raw, deduped: t.deduped, dropped: t.dropped })),
  totalConfirmed: results.reduce((n, t) => n + (t.confirmed ? t.confirmed.length : 0), 0),
}
