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

Do not replace the batch-safe flush with a raw cache flush. During offline
drain, inbound rows must become durable before their ratchet advances; otherwise
a crash can turn redelivery into an acknowledged duplicate and lose the event.

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
reloads, lossy clears, crash recovery, out-of-order group delivery, and retry
redistribution.

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
