# Offline peer receipt correction (issue 1619)

Related to [issue 1619](https://github.com/oxidezap/whatsapp-rust/issues/1619).
This is a proved wire/durability correction, **not** confirmation that a live
re-pair eliminates the reporter's server-side redelivery or 45–50-minute stream
recycling. No private traffic, credentials, re-pairing, or captured-code execution
was used.

## Observed boundary and history

The report describes three pairing-time `category="peer"` pkmsgs, redelivered
on each reconnect. An aggregate `peer_msg` receipt puts A at the root and B/C
inside `<list>`. The later stream error names B. Those logs suggest that the
server does not consume every list-item ID in this flow; they do not prove its
implementation or timer semantics.

Investigation started from main `0aaba75d1`, after the reporter's
`d9f78b806f1f4ca80c8008caa5846e5d542c2c55`. Receipt aggregation was introduced by
`fc6157b23e7dac9792917e76a69cff3e3090e227`, merged on 2026-06-10 in
[the original aggregation PR](https://github.com/oxidezap/whatsapp-rust/pull/820).
It buffered all eligible offline delivery receipts and grouped by receipt attrs.
The original test contained only one peer: it proved separation from ordinary
messages, but masked coalescing of multiple peers with identical attrs.
Subsequent durable commit/per-snapshot flush changes remain intact.

- **Initiating input:** multiple own-device peer messages enter the offline
  receive/drain path. Pairing is the reported origin, not a necessary synthetic
  test precondition.
- **Exposure/masking:** peers sharing `to`/type/participant attrs coalesced;
  live delivery, a singleton, or different grouping keys hid the divergence.
- **Visible reported symptom:** repeated redelivery and eventual stream
  recycling naming an aggregate list-item ID. This symptom is not reproduced
  against the real server here.

The smallest counterfactual changes only the offline path: the same three
same-key peer infos first traverse the live receipt worker, then the actual
buffer and durable drain flush. Before production edits, the regression failed:
three live frames plus **one** offline aggregate gave 4 frames where 6 were
required. A deliberately failed durable flush retained all three buffered peers
and emitted nothing early. With the correction, this same regression passes.
A preliminary test-only `Option` API typo failed compilation separately; it was
corrected before running the causal control, not counted as bug reproduction.

## Evidence, with limits

The original diagnostic request cited WA Web `2.3000.1047483476` and reported
that raw release bytes were unavailable. The repository instead pins whatspec
`1a441f0329c941fcdb238490a6c604550d8a9939`, WA Web **2.3000.1045368834**. Fetched
manifest, stanza and enum hashes matched `tools/whatspec-codegen/whatspec.lock.json`.

The IR confirms `WAWebSendReceiptJobCommon.RECEIPT_TYPE.PEER_MSG = "peer_msg"`,
individual delivery receipt attrs, and aggregate receipt/list syntax. An
aggregate builder accepting `type` does **not** establish its callers' routing.

For this repository's pin, the `bundle-store` release was accessible. Its
`bundles.lock.json` setHash is
`99a75bd7a4961e15051172c8b99fc460d57e58f7eb47ea9c46824657dd083e00`;
all 516 locked bundle hashes/sizes were verified. Static Babel AST analysis
located these relevant branches without evaluating any captured factory:

- `WAWebHandleMsg` explicitly routes successful `MSG_CATEGORY.peer` messages
  to `WAWebHandleMsgSendReceipt.sendReceipt`, rather than the ordinary offline
  `receiptInfo` cache enqueue.
- That sender's success/old-counter path calls
  `WAWebSendDeliveryReceiptJob.sendDeliveryReceiptsAfterDecryption`, whose
  individual builder selects `PEER_MSG`, preserves the addressed device and
  omits `recipient` for Peer.
- `WAWebSendOfflineDeliveryReceiptJob.sendAggregateOfflineReceipts` chooses
  `SENDER` or `DELIVERY`, not `PEER_MSG`. The cache snapshot stores messages and
  Signal state before invoking it.

**Disconfirming limit:** a preceding `SIGNAL_OLD_COUNTER_ERROR && S(P)` branch
can enqueue `duplicateMsgReceiptInfo` before the peer guard. Its downstream
aggregate routine also chooses sender/delivery rather than peer. Therefore this
static reading does not prove the exact pairing-time repeated-duplicate runtime
path, nor the newer bundle cited in the request. It does not justify changing
our dedup or receipt type. Exact WA runtime control flow and live re-pair/server
confirmation remain unproved.

Current independent corroboration, not oracle authority:

- Baileys `0af2386292907f7d9742d8d41f830d8c48208fa1`,
  `src/Socket/messages-recv.ts`: successful peer traffic selects `peer_msg` and
  calls `sendReceipt(..., [msg.key.id], type)`.
- whatsmeow `8b41cfe6d9c487e17858cf00be40e951f575dbe8`, `message.go` and
  `receipt.go`: the recognized, decoded path sends one receipt, with `peer_msg`
  selected for its own peer-message classification.

Neither success path generally adds a second transport message ACK. These
observations are not proofs of server acceptance or equivalent duplicate logic.

## Correction contract

Only the existing flush's wire strategy changes:

1. Peer entries still enter `offline_receipt_buffer` while the inbound batcher
   is active, including the deferred drain-to-live window.
2. Existing durable commit/snapshot/drain-completion call sites still decide
   when the buffer may flush. Rows → Signal → consumer durability hook ordering
   is unchanged; an unsuccessful flush still withholds receipts.
3. Grouping retains first-appearance order but assigns each Peer its own group.
   At flush, the existing individual delivery builder gives each its original
   root ID and `type="peer_msg"`, no list, no extra ACK, no aggregate timestamp.
4. All non-peer grouping keys, ID order, 256-ID chunks and common aggregate
   timestamp remain unchanged. Existing `to` device, group/status participant,
   and non-peer device-stripped recipient rules remain shared by both builders.
5. The same single `outbound_flush` task owns the batch. Scope close/drop,
   cancellation accounting, shutdown, connection-generation/reset clearing and
   redelivery-on-unsent behavior are unchanged. This is not a new retry promise.

There is no early Peer bypass, new transport ACK, `peer_msg` removal, dedup
workaround, strategy abstraction, public API, encryption, namespace or Signal
mutation change.

## Regression and disconfirming controls

- `offline_peer_receipts_are_sent_individually`: live/offline counterfactual,
  A/B/C sharing all grouping attrs, failed durable flush, three individual
  original-ID receipts, device-preserving `to`, no recipient/list/t/extra ACK,
  tracked task completion and zero retained buffer capacity.
- `encrypted_offline_peers_and_redelivered_duplicates_get_individual_receipts`:
  real own-device pkmsg receive/decrypt, three durable rows and persisted Signal
  state observed while the consumer hook pauses publication, then individual
  receipts; exact ciphertext redelivery takes the real duplicate branch,
  re-buffers/flushes three receipts without republishing message events.
  This is the closest safe user-path fixture, not live server/reconnect evidence.
- `mixed_offline_receipts_keep_routing_and_aggregation_through_shutdown`: the
  public shutdown drains a mixed batch; ordinary A/B/C still use one list,
  same-key peers A/B stay separate, group Peer C keeps its device participant,
  self-fanouts preserve distinct nondefault recipients and aggregate timestamp,
  first-appearance/ID ordering and no extra ACK.
- `offline_peer_receipts_drop_on_closed_scope_and_reset`: closed outbound scope
  drops the flush without leaked tracking; reset clears stragglers so a new
  offline window emits only its new peer ID.
- The formerly singleton grouping test now uses **two** same-key peers.
  `peer_self_fanout_is_peer_msg_without_recipient`, chunking, large-backlog
  ordering/allocation controls and existing commit/teardown regressions remain.

Focused nextest validation ran 11 tests successfully (exit 0). The full local
and current-source forge gate outcomes belong in the PR's validation record;
this focused result alone is not merge readiness. Existing native/wasm feature,
MSRV, consumer, allocation/future/size and security checks are not waived. Any
separate unresolved host/dependency security audit remains a blocker, not proof
against or approval of this receipt correction. Keep the issue open until the
report-required live confirmation is separately authorized and obtained.
