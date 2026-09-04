# Signal durability

This document is the checklist for code that reads, mutates, persists, or sends
Signal state. The security property is simple: an outbound message key and IV
must never reach the wire twice, including after cancellation, storage failure,
reconnect, or process death.

## State and leases

DM sessions and group sender keys use the same durability scheme:

| State | Counter | Cache gate |
| --- | --- | --- |
| `SessionRecord` | `reserved_sender_chain_index` | `reservation_pending` |
| `SenderKeyRecord` | `reserved_iteration` | `wire_gate_pending` |

The reservation is an exclusive upper bound. A send below it is covered by a
previously persisted lease and can use write-behind. A send at the bound raises
it by `SENDER_CHAIN_RESERVATION_BATCH` and must wait for a successful durable
flush before its ciphertext is published.

A reservation describes one sender chain, not the address. A DH ratchet
replaces the chain in place with fresh random key material and drops the old
one without archiving it, so the inherited ceiling stops describing anything
reachable: `rebase_lease_after_sender_chain_reset` lowers it back to one batch
as part of the same mutation. Lowering is sound only there — no snapshot can
pair the retired chain with the rebased ceiling — and only downward, so a
counter is never published under a ceiling that is not yet durable. Leave it
stranded and a long monologue followed by one peer reply puts the gap past
`MAX_RESERVATION_FAST_FORWARD`, where recovery can neither burn nor accept the
record. A chain that is *archived* rather than discarded keeps its claim:
`promote_fresh_state` burns the outgoing state to the ceiling before resetting
it.

An undecodable session row is reported absent rather than surfaced as a load
error. Every path that could replace it — the peer's next pre-key message, the
retry repair — must load it first, so a propagated error strands the address
permanently. `wa_session_record_quarantined_total` counts these; steady state
is zero.

The cache takes ownership of transient record gates. A failed write, a checked
out record skipped by a flush, or a tombstone whose delete failed must remain
gated. Only the backend operation that persisted that address may release it.
Decrypt-side advances are dirty but not pre-wire gated: they can be derived
forward again after a crash.

Stored records carry the cache incarnation that wrote them:

- A reload in the same live cache is exact. Eviction and `clear_after_flush()`
  must not burn the unused part of a lease.
- A new process or lossy cache reset has a new incarnation. It fast-forwards to
  the stored reservation because any counter below it may already have been
  published.
- Stores that bypass `SignalStoreCache` cannot claim an exact reload. They must
  use the incarnation-aware record format or conservatively recover as a new
  incarnation.

The current batch is 64 while the peer forward-jump limit is 2000. That makes a
single crash gap at most 3.2% of the receiver limit and amortizes a monotonic
sender chain to one synchronous reservation write per 64 messages. Tune this
only with sender-chain run-length and restart data; transport stanza counts do
not expose Signal iterations. Keep the batch well below `MAX_FORWARD_JUMPS` and
run the recovery matrix after any change.

Real crash/send cycles can accumulate burned ranges for a receiver that misses
every intervening message. Crossing its forward-jump bound is recoverable via
the retry/SKDM path, but clean cache reloads must never contribute to that gap.

## Publication boundary

All ciphertext APIs follow this order:

1. Load the record for mutation under its per-address or per-sender-key lock.
2. Derive the message key, advance the chain, and return the record to the
   cache even if the future can be cancelled.
3. If the advance raised a lease, let the cache adopt its transient gate.
4. Call `persist_signal_state_pre_wire()` after every recipient has been
   encrypted and before handing the stanza to the transport.
5. Abort the send if that flush fails.

The predicate is intentionally global. A pending lease for one address can
force another send to flush, but no call site can accidentally omit an address
whose ciphertext is already part of the stanza.

Each gate carries its own lock-free non-empty flag so step 4 does not take the
store locks a flush holds across backend I/O. The flag is owned by the gate and
republished by every mutation of it, because an over-reporting flag only costs a
redundant flush while an under-reporting one publishes an unpersisted lease.

Do not replace the batch-safe flush with a raw cache flush. During offline
drain, inbound rows must become durable before their ratchet advances; otherwise
a crash can turn redelivery into an acknowledged duplicate and lose the event.

## Contract matrix and recovery boundary

The following matrix records the release point for each mutation. A backend may
implement a batch operation as a transaction or as retryable per-key writes;
the cache contract is the same in both cases. If an operation returns an error,
the cache retains every affected dirty entry and gate, including entries that a
backend may have written before reporting the error. A retry may repeat those
bytes when the mutation is still current, or persist a newer mutation that has
superseded them; in either case the cache must confirm the version being
published before releasing its gate.

| Operation | Required ordering | Gate release | Existing proof |
| --- | --- | --- | --- |
| Session put, including a raised reservation | `put_session` returns to the cache, then `flush` persists the serialized record | Only for addresses included in a successful `put_sessions_batch` | `failed_flush_keeps_the_gate_closed`, `session_lease_gates_until_a_successful_flush` |
| Sender-key put, including a raised iteration lease | Encryption returns the record to the cache, then `flush` persists it; a pending distribution remains an in-memory retry buffer until the gated send succeeds | Only for names included in a successful `put_sender_keys_batch` | `failed_flush_keeps_the_sender_key_gate_closed`, `only_encrypt_marked_sender_keys_gate_the_wire` |
| Session or sender-key delete | Cache installs a tombstone before backend deletion | Only after the corresponding delete operation succeeds | `session_tombstone_keeps_gate_until_delete_is_durable`, `sender_key_tombstone_keeps_gate_until_delete_is_durable`, `a_failed_durable_delete_keeps_the_gate_closed` |
| Batch failure or partial progress | Retry the still-current serialized values and tombstones, or confirm a newer value superseded them | No entry is released by the failed call; entries already confirmed by an earlier successful call may be released individually | `partial_session_batch_failure_keeps_all_gates_for_retry`, `incomplete_session_flush_retains_newer_state_and_fails_closed_on_recovery` |
| Consumed one-time prekey | Persist the promoted session before removing its prekey | Remove only after the session is known durable | `consumed_prekey_stays_durable_until_session_flush`, `checked_out_session_defers_prekey_delete_until_durable`, `failed_session_flush_does_not_delete_prekey` |
| Outbound publication | Encrypt and return state, then call `persist_signal_state_pre_wire` | The transport is reached only after a successful flush when a gate is raised | `test_send_aborts_before_wire_when_lease_persist_fails` in `tests/e2e/tests/session_reuse.rs`; the chaos and SQLite tests cover state recovery separately |

The recovery precondition is a current, confirmed backend snapshot and a cache
incarnation that identifies whether the snapshot was written by this live
cache. A clean reload in the same incarnation preserves the exact unused lease;
a new incarnation advances to the persisted exclusive ceiling because a lower
counter may already have reached the wire. This is a recovery rule for a
confirmed current row, not an anti-rollback mechanism. If an operator restores
an arbitrarily old snapshot, the bytes contain no independent monotonic source
that can distinguish that restore from a legitimate older database. The API
can demonstrate the distinction between clean reload and new-incarnation
recovery, but cannot detect arbitrary snapshot rollback from the snapshot alone.
The focused proofs are `dm_clean_reload_is_exact_but_new_cache_burns_the_lease`,
`repeated_clean_reloads_keep_group_messages_within_forward_jump_limit`, and
`an_unreadable_session_row_is_reported_absent_so_recovery_can_replace_it`.

SQLite uses WAL with `Synchronous::Normal` by default. The durability boundary
is the successful SQLite transaction observed by the cache, which gives the
required process-crash ordering for committed rows and batched deletes. Normal
does not promise that every commit survives sudden power loss before a WAL
checkpoint; callers requiring that stronger storage claim can select
`Synchronous::Full`. The cache contract does not silently promote all writes to
Full because that would change the configured cost without improving the
ordering proof. The effective default and the configurable Full mode are
covered by the SQLite pragma tests and the ignored subprocess restart test;
that process test is not a simulation of power loss.

## Cancellation, deletion, and teardown

`SessionCheckout` owns the only mutable copy while a session operation is in
flight. Its drop path returns the advanced record synchronously or queues the
restore if the cache lock is contended. A checkout token and recovery generation
prevent a stale owner from overwriting a delete, newer owner, or lossy reset.

Deletion is durable state. A session or sender-key tombstone must retain any
existing pre-wire gate until the backend delete succeeds. A consumed one-time
prekey is deleted only after its promoted session is durable, so a crash cannot
lose both recovery inputs.

Clean reconnect teardown flushes and then calls `clear_after_flush()`. Dirty
state from a failed final flush stays resident for the next attempt. `clear()`
is lossy: it changes the incarnation and is only valid when the corresponding
uncommitted inbound work is also dropped so the server can redeliver it.

## Review checklist

For any new or changed ciphertext path, verify:

- The chain mutation is returned to the cache across every error and
  cancellation edge.
- A newly raised lease reaches durable storage before any ciphertext derived
  from it reaches the transport.
- A failed flush aborts publication and leaves both dirty state and gate intact.
- Covered sends avoid synchronous storage without skipping eventual
  write-behind.
- Clean eviction/reload preserves the exact counter; crash reload burns to the
  exclusive reservation ceiling.
- Deletes cannot be undone by a stale checkout or in-flight sender-key writer.
- Lock ordering matches existing session, sender-key, inbound-drain, and cache
  ordering; no backend await is added under an unrelated device lock.
- Tests use fictitious JIDs and never log key material, plaintext, or production
  identifiers.

## Verification

Focused unit tests live beside `SignalStoreCache`, record serialization, and
the libsignal ciphers. The deterministic state machine combines DM and group
sends, failed writes, cancellation, checkout/flush overlap, tombstones, clean
reloads, lossy clears, crash recovery, out-of-order group delivery, receiver
state loss, and retry redistribution.

```bash
# Small matrix in normal CI
cargo test -p wacore signal_durability_chaos_smoke

# Nightly-sized local run
SIGNAL_CHAOS_SEEDS=128 SIGNAL_CHAOS_STEPS=256 \
  cargo test -p wacore --lib signal_durability_chaos_nightly -- --ignored --nocapture

# Replay the seed printed by a failure
SIGNAL_CHAOS_SEED=0x... SIGNAL_CHAOS_SEEDS=1 SIGNAL_CHAOS_STEPS=256 \
  cargo test -p wacore --lib signal_durability_chaos_nightly -- --ignored --nocapture

# Real SQLite database across a SIGKILL and restart on Unix
cargo test -p whatsapp-rust --test signal_durability_sqlite \
  signal_durability_sqlite_process_restart -- --ignored --exact --nocapture
```

The scheduled workflow runs the large state-machine matrix and the SQLite
subprocess test. A state-machine failure reports its replay seed, step, and
action; a failed SQLite job retains its synthetic database as a short-lived CI
artifact.
