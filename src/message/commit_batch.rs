//! Inbound commit batcher: accumulates decrypted messages during the offline
//! drain and commits them per batch — pending-inbound buffer, Signal-cache
//! flush, durability hook, event dispatch and acks all amortize over the
//! batch. Mirrors WA Web's `MessageProcessorCache` (`createSnapshot`: bulk
//! message-table write → bulk signal-store commit → aggregate receipts), with
//! the same flush triggers: batch size, timeout, and end-of-drain.
//!
//! Live messages bypass accumulation and commit as a batch of one, which is
//! also WA Web behavior (the same pipeline with an immediate flush).

use super::raw_pending::{MAX_PENDING_BYTES, Occurrence, PendingDelivery, PendingJournal};
use super::*;
use crate::flush_scope::DrainOutcome;
use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use wacore::store::traits::{
    PendingInboundKey, PendingInboundLease, PendingInboundMutation, PendingInboundPayload,
    PendingInboundRevision,
};
use wacore::types::events::{BatchOrigin, InboundMessage, MessageBatch};

/// WA Web `web_message_processing_cache_size` (400) — the snapshot flush
/// granularity, distinct from the 200-msg offline *pull* size.
const MAX_BATCH_MESSAGES: usize = 400;
/// Byte cap so a media-heavy backlog cannot hold multi-MB protos in memory
/// (WA Web caps by count only; we are stricter).
const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;
/// Safety-net so a slow trickle still commits durably instead of waiting for
/// the size cap or end-of-drain. Deliberately stricter than WA Web (whose cache
/// timeout defaults to 0) because our durability hook must not lag.
const FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Default)]
struct BatchState {
    entries: Vec<PendingDelivery>,
    /// A dropped request must not outlive the reconnect that discarded it.
    commit_ticket: Option<InboundCommitTicket>,
    /// Carrier bytes plus encoded projection estimates, for the flush cap.
    /// This is not an allocator measurement of decoded protobuf graphs.
    bytes: usize,
    timer_armed: bool,
}

struct PendingInboundBatch {
    entries: Vec<PendingDelivery>,
    commit_ticket: Option<InboundCommitTicket>,
}

impl PendingInboundBatch {
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn mark_dropped(self) -> Vec<PendingDelivery> {
        if let Some(ticket) = self.commit_ticket {
            ticket.mark_dropped();
        }
        self.entries
    }
}

pub(crate) struct PendingStorage {
    arena: std::sync::Mutex<Vec<u8>>,
}
impl PendingStorage {
    fn arena(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        self.arena
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

pub(crate) struct InboundCommitBatcher {
    state: std::sync::Mutex<BatchState>,
    #[cfg(test)]
    pub(crate) publication_reached: std::sync::atomic::AtomicBool,
    /// Whether inbound commits accumulate here (offline drain) or commit
    /// immediately (live). Flipped off ONLY by the end-of-drain flush while it
    /// holds the single processing permit, so no stanza can straddle the
    /// transition: gating on `offline_sync_completed` (which flips outside the
    /// permit) would let an in-flight stanza persist Signal state for an
    /// uncommitted batch entry, and let queued drain stanzas commit as Live
    /// ahead of the accumulated batch.
    active: std::sync::atomic::AtomicBool,
    /// Set when the end-of-drain flush could not commit its tail: the guard
    /// restored the entries, and switching to live mode anyway would let the
    /// per-stanza full-cache Signal flush persist their ratchet advances with
    /// no durable rows — redelivery would then be acked as duplicates. The
    /// batcher stays in drain mode instead (entries keep batching, the flush
    /// timer retries the commit) and the next durable flush completes the
    /// deferred transition.
    pending_live: std::sync::atomic::AtomicBool,
    /// Bumped on every take; a timer that observes a stale epoch stands down.
    epoch: AtomicU64,
    /// Test-only injection: fail the next drain commits before their durable
    /// point, exercising the ReinsertGuard/deferred-transition paths (no
    /// backend failure can be injected through the real store).
    #[cfg(test)]
    pub(crate) fail_commits: std::sync::atomic::AtomicBool,
    /// Test-only injection: fail drain Signal flushes AFTER the durable row
    /// write, exercising the rows-stored-but-unflushed retry path.
    #[cfg(test)]
    pub(crate) fail_flushes: std::sync::atomic::AtomicBool,
    /// Reusable encode arena for drain commits, which the processing permit
    /// already serializes — so this lock is never contended there. Live
    /// commits use a local buffer instead: sharing it would serialize
    /// concurrent live-path hook calls that could previously overlap. Release
    /// this buffer when the drain finishes; live traffic cannot reuse it.
    pending_storage: Arc<async_lock::Mutex<PendingStorage>>,
    pub(super) live_retention: live_retention::LiveRetention,
}

impl Default for InboundCommitBatcher {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(BatchState::default()),
            #[cfg(test)]
            publication_reached: std::sync::atomic::AtomicBool::new(false),
            active: std::sync::atomic::AtomicBool::new(true),
            pending_live: std::sync::atomic::AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            pending_storage: Arc::new(async_lock::Mutex::new(PendingStorage {
                arena: std::sync::Mutex::new(Vec::new()),
            })),
            live_retention: Default::default(),
            #[cfg(test)]
            fail_commits: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_flushes: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl InboundCommitBatcher {
    #[cfg(test)]
    pub(crate) fn storage_layout_delta() -> isize {
        (size_of::<Arc<async_lock::Mutex<PendingStorage>>>()
            + size_of::<live_retention::LiveRetention>()) as isize
            - size_of::<async_lock::Mutex<Vec<u8>>>() as isize
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BatchState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poison) => poison.into_inner(),
        }
    }

    /// Take the accumulated batch, invalidating any armed timer.
    fn take(&self) -> PendingInboundBatch {
        let mut state = self.lock();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.bytes = 0;
        state.timer_armed = false;
        PendingInboundBatch {
            entries: std::mem::take(&mut state.entries),
            commit_ticket: state.commit_ticket.take(),
        }
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    /// Whether uncommitted entries are still accumulated. Teardown uses this
    /// to decide if flushing the Signal cache is safe: persisting ratchet
    /// advances for entries about to be dropped would turn their redelivery
    /// into an unrecoverable duplicate.
    pub(crate) fn has_entries(&self) -> bool {
        !self.lock().entries.is_empty()
    }

    /// Entries waiting for commit and their encoded bytes, for
    /// `memory_report()`.
    ///
    /// This is the largest thing one client retains: a drain batch holds
    /// `MAX_BATCH_BYTES` of decoded protos, two orders of magnitude above every
    /// other collection in the report. The byte figure is the same
    /// `message_encoded_len` sum the flush threshold is compared against, so it
    /// is a proxy for the decoded footprint, not a measurement of it — the
    /// decoded `wa::Message` graph is larger than its wire form.
    ///
    /// Two exclusions, both deliberate:
    ///
    /// - **A batch inside its commit.** [`Self::take`] empties this state
    ///   before `commit_inbound_batch` awaits the backend, the Signal flush and
    ///   the durability hook, all of which still hold the messages. Counting
    ///   them would mean re-encoding the batch to price it on the commit path —
    ///   real work on the receive path to sharpen a report — so this reads
    ///   "waiting to be committed", not "resident". `has_entries` draws the
    ///   same line, for the same reason.
    /// - **The reusable encode arena**, whose lock a drain commit holds across
    ///   its backend write; a report must not queue behind one. Worth knowing
    ///   that this hides real memory during the drain: the arena retains its
    ///   capacity between batches, then releases it on the live transition.
    pub(crate) fn pending_stats(&self) -> (usize, usize) {
        let state = self.lock();
        (state.entries.len(), state.bytes)
    }

    /// Switch to immediate (live) commits. Only the end-of-drain flush (or a
    /// later flush completing a deferred transition) calls this, while
    /// holding the processing permit.
    fn deactivate(&self) {
        self.pending_live.store(false, Ordering::Release);
        self.active.store(false, Ordering::Release);
        // Normal transitions hold the processing permit after the final
        // commit, so no encoder owns this lock. A forced exit may race a
        // stalled backend write: never block it or invalidate borrowed rows.
        if let Some(storage) = self.pending_storage.try_lock() {
            *storage.arena() = Vec::new();
        }
    }

    #[cfg(feature = "bench-harness")]
    pub(crate) fn encode_arena_capacity(&self) -> Option<usize> {
        self.pending_storage
            .try_lock()
            .map(|storage| storage.arena().capacity())
    }

    /// Record that the end-of-drain flush failed before its durable point;
    /// see the `pending_live` field docs.
    fn defer_live_transition(&self) {
        self.pending_live.store(true, Ordering::Release);
    }

    fn live_transition_pending(&self) -> bool {
        self.pending_live.load(Ordering::Acquire)
    }

    /// Last-resort drain exit for the unreachable-in-practice case where the
    /// finisher cannot run (`self_weak` upgrade failure): drop any entries
    /// (unacked, so the server redelivers them) and still switch to live mode
    /// — staying active with a widened semaphore would batch live traffic
    /// while flushes hold only 1 of its permits.
    pub(crate) fn force_live_dropping_entries(&self) {
        let dropped = self.take().mark_dropped();
        if !dropped.is_empty() {
            log::warn!(
                "Dropping {} uncommitted inbound messages on forced drain exit; the server will redeliver them",
                dropped.len()
            );
        }
        self.deactivate();
    }

    /// Connection teardown/setup: drop uncommitted entries (they were never
    /// acked, so the server redelivers them) and re-arm accumulation for the
    /// next connection's drain. Returns whether entries were dropped — the
    /// caller must drop the Signal cache with them (their cache-only ratchet
    /// advances have no rows; flushing them later would make each redelivery
    /// an ackable duplicate).
    pub(crate) fn reset(&self) -> bool {
        let dropped = self.take().mark_dropped();
        if !dropped.is_empty() {
            log::debug!(
                "Dropping {} uncommitted inbound messages; the server will redeliver them",
                dropped.len()
            );
        }
        self.pending_live.store(false, Ordering::Release);
        self.active.store(true, Ordering::Release);
        !dropped.is_empty()
    }
}

#[cfg(any(test, feature = "bench-harness"))]
impl Client {
    /// Test/bench mirror of a completed offline sync: the flag, live permit
    /// count and batcher mode move together so a fixture can never run in a
    /// hybrid drain/live state production cannot reach. Kept next to the
    /// production transition (`finish_offline_sync`) so a new step gets added
    /// to both. The `bench-harness` fixture needs it for the same reason a
    /// test does — a send measured in drain mode is not the steady state.
    pub(crate) fn enter_live_mode_for_tests(&self) {
        self.offline_sync_completed.store(true, Ordering::Release);
        // The guard is a generation stamp now; mark the current one finished.
        self.offline_sync_finish_started.store(
            self.connection_generation
                .load(Ordering::Acquire)
                .saturating_add(1),
            Ordering::Release,
        );
        self.swap_message_semaphore(64);
        self.inbound_commit_batch.deactivate();
    }
}

/// Restores a taken drain batch if the commit is cancelled (bounded teardown
/// timeout) or fails before its durable point: taken entries must never
/// vanish while another path can still flush their ratchet advances — a
/// persisted ratchet without a buffered row makes the redelivery an ackable
/// duplicate (silent loss for hook consumers).
struct ReinsertGuard<'a> {
    batcher: &'a InboundCommitBatcher,
    items: Option<Arc<[PendingDelivery]>>,
    commit_ticket: Option<InboundCommitTicket>,
}

impl ReinsertGuard<'_> {
    fn mark_durable(&mut self) {
        self.items = None;
        if let Some(ticket) = self.commit_ticket.take() {
            ticket.mark_durable();
        }
    }
}

impl Drop for ReinsertGuard<'_> {
    fn drop(&mut self) {
        let Some(items) = self.items.take() else {
            return;
        };
        let mut state = self.batcher.lock();
        let mut restored: Vec<PendingDelivery> = items.iter().cloned().collect();
        // Nothing newer can normally exist (the permit serializes drain
        // flushes), but keep arrival order if it ever does.
        restored.append(&mut state.entries);
        state.bytes = restored
            .iter()
            .map(PendingDelivery::retained_wire_bytes)
            .sum();
        // The taking flush bumped the epoch, so no sleeper owns this batch;
        // the next enqueue arms a fresh one, and the drain-end/teardown
        // flushes cover the gap regardless.
        state.timer_armed = false;
        state.entries = restored;
        if let Some(ticket) = self.commit_ticket.take() {
            if state.commit_ticket.is_some() {
                ticket.mark_dropped();
            } else {
                state.commit_ticket = Some(ticket);
            }
        }
        log::warn!(
            "Restored {} uncommitted inbound messages to the batch after a failed or cancelled commit",
            state.entries.len()
        );
    }
}

#[derive(Clone, Copy)]
struct PendingCommitOutcome {
    persisted: bool,
    delivered: bool,
}
impl PendingCommitOutcome {
    const FAILED: Self = Self {
        persisted: false,
        delivered: false,
    };
    const BUFFERED: Self = Self {
        persisted: true,
        delivered: false,
    };
    const DELIVERED: Self = Self {
        persisted: true,
        delivered: true,
    };
}

struct StoredPendingGroup {
    chat: String,
    sender: String,
    id: String,
    revision: PendingInboundRevision,
    complete: bool,
}

// Keep one concrete commit poll body shared across drain and replay callers.
// The future stays pinned on the caller's stack; this adds no heap owner.
#[inline(never)]
fn poll_pending_commit<F: Future<Output = PendingCommitOutcome>>(
    future: std::pin::Pin<&mut F>,
    cx: &mut std::task::Context<'_>,
) -> std::task::Poll<PendingCommitOutcome> {
    future.poll(cx)
}

impl Client {
    // Group sender-key addresses ignore the device. New rows use that same
    // identity; retain an existing physical spelling so legacy CAS cleanup
    // still targets the row whose bytes were actually read.
    fn pending_sender(info: &MessageInfo) -> String {
        if info.source.chat.is_group() {
            info.source.sender.to_non_ad_string()
        } else {
            info.source.sender.to_string()
        }
    }

    pub(super) async fn read_pending_for_replay(
        &self,
        info: &MessageInfo,
    ) -> anyhow::Result<Option<(String, wacore::store::traits::PendingInboundRecord)>> {
        let owner = Arc::new(self.inbound_commit_batch.pending_storage.lock_arc().await);
        let lease = PendingInboundLease::new(owner);
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        let backend = self.persistence_manager.backend();
        let key = PendingInboundKey {
            chat: &chat,
            sender: &sender,
            id: &info.id,
        };
        if info.source.chat.is_group() {
            return Ok(backend
                .read_pending_inbound_group_bounded(key, MAX_PENDING_BYTES, lease)
                .await?);
        }
        Ok(backend
            .read_pending_inbound_bounded(key, MAX_PENDING_BYTES, lease)
            .await?
            .map(|record| (sender, record)))
    }

    async fn persist_pending_group(
        &self,
        items: &[PendingDelivery],
        is_drain: bool,
    ) -> anyhow::Result<Vec<StoredPendingGroup>> {
        // Workers retain this mutation owner after receive cancellation. It
        // covers only storage; never hold it while invoking consumer code.
        let owner = Arc::new(self.inbound_commit_batch.pending_storage.lock_arc().await);
        // The detached request retains both mutation exclusion and admission
        // charges after the receive future is cancelled.
        let charges: Vec<_> = items
            .iter()
            .filter_map(|item| item.retention.clone())
            .collect();
        let lease = PendingInboundLease::new(Arc::new((Arc::clone(&owner), charges)));
        let backend = self.persistence_manager.backend();
        let mut groups: Vec<(String, String, String, Vec<&PendingDelivery>)> = Vec::new();
        for item in items {
            let chat = item.info.source.chat.to_string();
            let incoming_sender = item.storage_sender.as_ref().map_or_else(
                || item.info.source.sender.to_string(),
                |sender| sender.to_string(),
            );
            let canonical = Self::pending_sender(&item.info);
            let sender = if item.storage_sender.is_some()
                || canonical != incoming_sender
                    && backend
                        .read_pending_inbound_bounded(
                            PendingInboundKey {
                                chat: &chat,
                                sender: &incoming_sender,
                                id: &item.info.id,
                            },
                            MAX_PENDING_BYTES,
                            lease.clone(),
                        )
                        .await?
                        .is_some()
            {
                incoming_sender
            } else {
                canonical
            };
            if let Some(group) = groups
                .iter_mut()
                .find(|group| group.0 == chat && group.1 == sender && group.2 == item.info.id)
            {
                group.3.push(item);
            } else {
                groups.push((chat, sender, item.info.id.to_string(), vec![item]));
            }
        }
        let mut stored = Vec::with_capacity(groups.len());
        for (chat, sender, id, deliveries) in groups {
            let key = PendingInboundKey {
                chat: &chat,
                sender: &sender,
                id: &id,
            };
            let record = backend
                .read_pending_inbound_bounded(key, MAX_PENDING_BYTES, lease.clone())
                .await?;
            let expected = record
                .as_ref()
                .map_or(PendingInboundRevision::missing(), |record| {
                    record.revision()
                });
            let mut journal = match record {
                Some(record) => {
                    let (bytes, _read_lease) = record.into_bytes();
                    PendingJournal::decode(bytes)?
                }
                None => PendingJournal::default(),
            };
            let mut covered = Vec::with_capacity(deliveries.len());
            for delivery in deliveries {
                let projection = if is_drain {
                    let mut arena = owner.arena();
                    arena.clear();
                    waproto::codec::message_encode_into(&delivery.message, &mut arena);
                    bytes::Bytes::copy_from_slice(&arena)
                } else {
                    bytes::Bytes::from(waproto::codec::message_to_vec(&delivery.message))
                };
                let (index, original) = match &delivery.carrier {
                    Some((index, original)) => (*index, original.clone()),
                    None => (None, projection.clone()),
                };
                let occurrence = Occurrence {
                    producer: delivery.origin.map(|origin| origin.stanza),

                    index,
                    original,
                    projection,
                };
                covered.push(occurrence.clone());
                journal.merge(occurrence)?;
            }
            let complete = journal.occurrences.iter().all(|entry| {
                covered.iter().any(|incoming| {
                    incoming.producer == entry.producer
                        && incoming.index == entry.index
                        && incoming.original == entry.original
                        && incoming.projection == entry.projection
                })
            });
            let encoded = journal.encode()?;
            let payload = PendingInboundPayload::new(encoded, lease.clone());
            let revision = payload.revision();
            anyhow::ensure!(
                backend
                    .replace_pending_inbound_if(key, expected, payload, MAX_PENDING_BYTES)
                    .await?
                    == PendingInboundMutation::Applied,
                "pending occurrence revision conflict"
            );
            stored.push(StoredPendingGroup {
                chat,
                sender,
                id,
                revision,
                complete,
            });
        }
        Ok(stored)
    }

    async fn cleanup_pending_group(&self, rows: &[StoredPendingGroup]) -> anyhow::Result<()> {
        let owner = Arc::new(self.inbound_commit_batch.pending_storage.lock_arc().await);
        let lease = PendingInboundLease::new(owner);
        let backend = self.persistence_manager.backend();
        let mut failure = None;
        for row in rows {
            let key = PendingInboundKey {
                chat: &row.chat,
                sender: &row.sender,
                id: &row.id,
            };
            let result = match backend
                .delete_pending_inbound_if(key, row.revision, MAX_PENDING_BYTES, lease.clone())
                .await
            {
                Ok(PendingInboundMutation::Applied) => Ok(()),
                Ok(_) => Err(anyhow::anyhow!(
                    "pending occurrence cleanup revision conflict"
                )),
                Err(error) => {
                    // An error can follow an applied deletion. Verify absence
                    // under the same owner, and never delete a successor.
                    match backend
                        .read_pending_inbound_bounded(key, MAX_PENDING_BYTES, lease.clone())
                        .await
                    {
                        Ok(None) => Ok(()),
                        Ok(Some(_)) => Err(error.into()),
                        Err(error) => Err(error.into()),
                    }
                }
            };
            if let Err(error) = result {
                // Retain ACK failure, but clean other owned revisions: leaving
                // both physical aliases would make bounded replay ambiguous.
                failure.get_or_insert(error);
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl Client {
    fn enqueue_inbound_commit(
        &self,
        item: PendingDelivery,
        track_commit: bool,
    ) -> (Option<u64>, Option<InboundCommitTicket>) {
        let batcher = &self.inbound_commit_batch;
        let mut state = batcher.lock();
        state.bytes += item.retained_wire_bytes();
        state.entries.push(item);
        let ticket = track_commit.then(|| {
            state
                .commit_ticket
                .get_or_insert_with(InboundCommitTicket::new)
                .clone()
        });
        let epoch = if !state.timer_armed {
            state.timer_armed = true;
            Some(batcher.epoch.load(Ordering::Acquire))
        } else {
            None
        };
        (epoch, ticket)
    }

    pub(super) async fn persist_live_retained_before_decrypt(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
    ) -> anyhow::Result<()> {
        let retained = self.inbound_commit_batch.live_retention.for_message(info);
        if !retained.is_empty() {
            self.persist_pending_group(&retained, false).await?;
            self.inbound_commit_batch.live_retention.stored(&retained);
        }
        let Some((sender, record)) = self.read_pending_for_replay(info).await? else {
            return Ok(());
        };
        let (bytes, read_lease) = record.into_bytes();
        let journal = PendingJournal::decode(bytes)?;
        let storage_sender: Arc<str> = sender.into();
        drop(read_lease);
        let mut predecessor = Vec::with_capacity(journal.occurrences.len());
        for (position, entry) in journal.occurrences.into_iter().enumerate() {
            let position = entry.index.map_or(position, |index| index as usize);
            let mut delivery = PendingDelivery::from_occurrence(entry, position, info, None, None)?;
            delivery.storage_sender = Some(Arc::clone(&storage_sender));
            predecessor.push(delivery);
        }
        // A hook can flush Signal itself. Recover the durable predecessor
        // before decrypt advances the incoming ratchet, without acknowledging
        // the incoming stanza. Refusal leaves its ciphertext untouched.
        anyhow::ensure!(
            self.commit_pending_batch_result(predecessor.into(), BatchOrigin::Live, None, false)
                .await
                .delivered,
            "predecessor hook has not recovered before decrypt"
        );
        Ok(())
    }

    async fn live_pending_retry_batch(
        self: &Arc<Self>,
        item: &PendingDelivery,
    ) -> anyhow::Result<Option<Vec<PendingDelivery>>> {
        let Some(origin) = item
            .origin
            .filter(|_| self.inbound_durability_hook().is_some())
        else {
            return Ok(None);
        };
        let Some((sender, record)) = self.read_pending_for_replay(&item.info).await? else {
            return Ok(None);
        };
        let (bytes, read_lease) = record.into_bytes();
        let journal = PendingJournal::decode(bytes)?;
        drop(read_lease);
        let storage_sender: Arc<str> = sender.into();
        let projection = waproto::codec::message_to_vec(&item.message);
        let index = u32::try_from(origin.position)?;
        if !journal.occurrences.iter().any(|entry| {
            entry
                .producer
                .is_some_and(|producer| producer != origin.stanza)
                && entry.index == Some(index)
                && entry.projection.as_ref() == projection
        }) {
            return Ok(None);
        }
        // Retry a failed hook with the actual retained occurrences, never
        // substitute projection equality for their storage ownership.
        let original_len = item
            .carrier
            .as_ref()
            .map_or(projection.len(), |(_, raw)| raw.len());
        // Capacity recovery belongs before decrypt: invoking a predecessor
        // hook here could flush the incoming ratchet without its carrier.
        anyhow::ensure!(
            journal.fits_additional(original_len, projection.len()),
            "pending predecessor requires recovery before decrypt"
        );
        let mut retained = Vec::with_capacity(journal.occurrences.len() + 1);
        for (position, entry) in journal.occurrences.into_iter().enumerate() {
            let position = entry.index.map_or(position, |index| index as usize);
            let shared_message =
                (entry.projection.as_ref() == projection).then(|| Arc::clone(&item.message));
            let mut delivery = PendingDelivery::from_occurrence(
                entry,
                position,
                &item.info,
                shared_message,
                None,
            )?;
            delivery.storage_sender = Some(Arc::clone(&storage_sender));
            retained.push(delivery);
        }
        retained.push(item.clone());
        Ok(Some(retained))
    }

    /// Batch a message decrypted while the offline drain is active, or commit
    /// immediately (batch of one) on the live path.
    #[cfg(test)]
    pub(crate) async fn commit_or_batch_inbound(
        self: &Arc<Self>,
        item: InboundMessage,
        track_commit: bool,
    ) -> InboundCommitState {
        self.commit_or_batch_pending(item.into(), track_commit)
            .await
    }

    pub(super) async fn commit_or_batch_pending(
        self: &Arc<Self>,
        mut item: PendingDelivery,
        track_commit: bool,
    ) -> InboundCommitState {
        item.receipt = self
            .inbound_commit_batch
            .live_retention
            .receipt_for(&item.info);
        if !self.inbound_commit_batch.is_active() {
            if self.inbound_durability_hook().is_some()
                && let Err(error) = self.inbound_commit_batch.live_retention.retain(&mut item)
            {
                log::warn!("Live retention admission refused: {error:?}");
                return InboundCommitState::Failed;
            }
            let items: Arc<[PendingDelivery]> = match self.live_pending_retry_batch(&item).await {
                Ok(Some(retained)) => retained.into(),
                Ok(None) => Arc::from([item]),
                Err(error) => {
                    log::warn!("Failed to retain live pending retry: {error:?}");
                    return InboundCommitState::Failed;
                }
            };
            let outcome = self
                .commit_pending_batch_result(Arc::clone(&items), BatchOrigin::Live, None, true)
                .await;
            if !outcome.delivered {
                for item in items.iter() {
                    self.inbound_commit_batch
                        .live_retention
                        .fail_receipt(&item.info);
                }
            }
            return if outcome.delivered {
                InboundCommitState::Durable
            } else if outcome.persisted {
                InboundCommitState::Buffered
            } else {
                InboundCommitState::Failed
            };
        }
        // Raw replay can reenter while the offline batcher is active. Keep its
        // existing admission alive in queued storage even after the raw slot
        // is retired; ordinary drain arrivals do not create live reservations.
        if item.retention.is_none() {
            item.retention = self
                .inbound_commit_batch
                .live_retention
                .charge_for_origin(item.origin);
        }
        let (timer_epoch, ticket) = self.enqueue_inbound_commit(item, track_commit);
        if let Some(epoch) = timer_epoch {
            // Weak: a sleeper must not keep the whole Client graph alive for
            // up to 3s after the app drops its handle.
            let client = Arc::downgrade(self);
            let runtime = self.runtime.clone();
            self.runtime
                .spawn(Box::pin(async move {
                    runtime.sleep(FLUSH_TIMEOUT).await;
                    if let Some(client) = client.upgrade()
                        && client.inbound_commit_batch.epoch.load(Ordering::Acquire) == epoch
                    {
                        // The epoch re-checks after permit acquisition: a
                        // size-trigger flush racing this sleeper must not let
                        // it commit a batch that is only milliseconds old.
                        let _ = client
                            .flush_inbound_commits_under_permit(false, Some(epoch), None)
                            .await;
                    }
                }))
                .detach();
        }
        InboundCommitState::Deferred(ticket)
    }

    /// Size/byte-cap check, run at the end of stanza processing while the
    /// global processing permit is still held (so the Signal flush inside the
    /// commit cannot interleave with a half-processed stanza).
    pub(crate) async fn maybe_flush_inbound_commits(self: &Arc<Self>) {
        let over = {
            let state = self.inbound_commit_batch.lock();
            state.entries.len() >= MAX_BATCH_MESSAGES || state.bytes >= MAX_BATCH_BYTES
        };
        if over {
            let PendingInboundBatch {
                entries,
                commit_ticket,
            } = self.inbound_commit_batch.take();
            let durable = self
                .commit_pending_batch(entries.into(), BatchOrigin::OfflineDrain, commit_ticket)
                .await;
            // A durable commit may be the retry that clears a deferred
            // drain→live transition (failed end-of-drain tail).
            if durable && self.inbound_commit_batch.live_transition_pending() {
                self.complete_deferred_live_transition();
            }
        }
    }

    /// Flush after acquiring a global processing permit, so no stanza is
    /// mid-decrypt when the Signal cache is flushed: a crash could otherwise
    /// persist a ratchet advance for a message no batch has committed, turning
    /// its redelivery into an unrecoverable duplicate. During the drain the
    /// semaphore holds a single permit, so this fully serializes with stanza
    /// processing. An empty batch is NOT a no-op: it still flushes the Signal
    /// cache under the permit (idempotent when the cache is already clean), so
    /// an out-of-band batch-safe caller that raced the drain finisher still
    /// gets its advance persisted rather than silently skipped.
    ///
    /// With `deactivate`, this is the end-of-drain transition: commit the tail
    /// batch and switch the batcher to live mode under the same permit hold.
    /// That is what makes the transition raceless — no stanza is mid-flight
    /// when the mode flips (a stanza's enqueue and its stanza-end flush always
    /// agree), and every stanza still queued behind this permit commits as
    /// Live strictly AFTER the tail batch, preserving arrival order across the
    /// boundary. Runs before the semaphore widens to the live permit count.
    ///
    /// Returns whether the drain state is durable (see
    /// [`commit_inbound_batch`](Self::commit_inbound_batch)); callers gate the
    /// buffered offline-receipt flush on it.
    pub(crate) async fn flush_inbound_commits_under_permit(
        self: &Arc<Self>,
        deactivate: bool,
        expected_epoch: Option<u64>,
        expected_generation: Option<u64>,
    ) -> bool {
        let _permit = self.acquire_message_processing_permit().await;
        if let Some(generation) = expected_generation
            && self.connection_generation.load(Ordering::Acquire) != generation
        {
            // A reconnect reset the batcher while this (drain-finisher) call
            // waited for the permit; the new connection owns the state now, so
            // touching it here would take/deactivate the NEW drain.
            return true;
        }
        if let Some(epoch) = expected_epoch
            && self.inbound_commit_batch.epoch.load(Ordering::Acquire) != epoch
        {
            // Another flush took this sleeper's batch while it waited for the
            // permit; whatever accumulates now belongs to a newer timer.
            return true;
        }
        let was_draining = self.inbound_commit_batch.is_active();
        let batch = self.inbound_commit_batch.take();
        let durable = if batch.is_empty() {
            // Even with nothing to commit, flush the Signal cache under the
            // permit. While draining this persists SKDM-only advances (stanzas
            // that mutate Signal state without enqueueing a message), whose
            // buffered receipts flush right after this function's
            // drain-end/teardown call sites — a failed flush reports
            // not-durable to hold them back (the cache keeps its dirty entries
            // for a later retry). When the batcher deactivated between an
            // out-of-band batch-safe caller's is_active() check and this
            // permit, the SAME flush persists that caller's advance instead of
            // silently no-oping, so is_active() never has to stand in for "was
            // flushed". Idempotent and cheap when the cache is already clean.
            self.drain_signal_flush_reporting().await
        } else {
            self.commit_pending_batch(
                batch.entries.into(),
                BatchOrigin::OfflineDrain,
                batch.commit_ticket,
            )
            .await
        };
        if let Some(generation) = expected_generation
            && self.connection_generation.load(Ordering::Acquire) != generation
        {
            // The pre-take check only covers up to the take: a teardown can
            // time out around the awaited commit above and reset the batcher,
            // so mutating the mode now would deactivate (or defer) the NEW
            // connection's drain. The commit itself was still sound — its
            // entries were taken before the reset and its rows are durable.
            return durable;
        }
        if durable {
            self.flush_offline_receipts();
        }
        if deactivate {
            if durable {
                self.inbound_commit_batch.deactivate();
            } else if was_draining {
                // The guard restored the entries; switching to live mode now
                // would let per-stanza full-cache flushes persist their
                // ratchet advances with no durable rows (acked-duplicate loss
                // on redelivery). Keep batching — the flush timer and later
                // flushes retry the commit, and the first durable one
                // completes this transition.
                self.inbound_commit_batch.defer_live_transition();
                self.arm_deferred_transition_retry();
                log::warn!(
                    "End-of-drain commit failed; staying in drain mode (single permit) until a durable flush completes the live transition"
                );
            }
        } else if durable && was_draining && self.inbound_commit_batch.live_transition_pending() {
            self.complete_deferred_live_transition();
        }
        durable
    }

    /// Finish a drain→live transition that a failed end-of-drain tail commit
    /// deferred: the finisher already published completion but left the
    /// batcher in drain mode AND the semaphore at one permit — the flush
    /// invariant (no stanza mid-decrypt while the whole Signal cache is
    /// persisted) only holds while stanzas are serialized. Called with the
    /// permit held and a durable commit just done, so the flip is as raceless
    /// as the normal end-of-drain one.
    pub(crate) fn complete_deferred_live_transition(&self) {
        // Receipts buffered during the deferred window (SKDM-only stanzas
        // keep buffering while the batcher is active) are safe to send now:
        // the durable flush that triggered this completion persisted their
        // Signal state.
        self.flush_offline_receipts();
        self.inbound_commit_batch.deactivate();
        self.swap_message_semaphore(64);
        // Key-share jobs may have observed the earlier failed transition.
        self.offline_sync_notifier.notify(usize::MAX);
        log::info!("Deferred drain-to-live transition completed after a durable flush");
    }

    /// Retry loop for a deferred transition, armed once at defer time: the
    /// guard restored the entries with their timer disarmed and the drain-end
    /// flush has already run, so on an idle connection NOTHING else would
    /// retry the commit — the tail (or dirty SKDM-only state) would sit
    /// uncommitted until teardown. Exits as soon as the transition completes
    /// (here or via any other durable flush) or a reconnect resets the
    /// batcher.
    fn arm_deferred_transition_retry(self: &Arc<Self>) {
        let client = Arc::downgrade(self);
        let runtime = self.runtime.clone();
        // Generation-scoped like the finisher: after a reconnect this stale
        // task stands down (a re-deferring new connection arms its own retry)
        // instead of flushing the new connection's state.
        let generation = self.connection_generation.load(Ordering::Acquire);
        self.runtime
            .spawn(Box::pin(async move {
                loop {
                    runtime.sleep(FLUSH_TIMEOUT).await;
                    let Some(client) = client.upgrade() else {
                        return;
                    };
                    if !client.inbound_commit_batch.live_transition_pending()
                        || client.connection_generation.load(Ordering::Acquire) != generation
                    {
                        return;
                    }
                    let _ = client
                        .flush_inbound_commits_under_permit(false, None, Some(generation))
                        .await;
                }
            }))
            .detach();
    }

    /// Teardown twin of [`flush_inbound_commits_bounded`]: commit the batch
    /// AND settle the Signal cache (flush-or-drop, then clear) in ONE
    /// permit-held section. Doing these as separate steps released the permit
    /// in between, and an old chat-lane worker (they drain until the
    /// connection generation changes) could grab it and be mid-decrypt —
    /// ratchets advanced, entry not yet enqueued — while the raw flush
    /// persisted its rowless advances; the reset then dropped the entry and
    /// its redelivery was acked as a duplicate.
    ///
    /// On timeout (stalled permit holder / hung hook) the cache is cleared
    /// WITHOUT flushing: everything dirty then belongs to uncommitted
    /// entries, and dropping both sides keeps redelivery consistent.
    pub(crate) async fn teardown_inbound_commits_bounded(
        self: &Arc<Self>,
        limit: std::time::Duration,
    ) -> DrainOutcome {
        let settle = async {
            let _permit = self.acquire_message_processing_permit().await;
            let batch = self.inbound_commit_batch.take();
            let durable = if batch.is_empty() {
                true
            } else {
                self.commit_pending_batch(
                    batch.entries.into(),
                    BatchOrigin::OfflineDrain,
                    batch.commit_ticket,
                )
                .await
            };
            // Permit still held: no stanza can be mid-decrypt while the cache
            // is settled. Flush-before-clear preserves in-flight sender-key
            // advances (a disconnect is not a logout); a failed or
            // non-durable commit leaves entries restored, whose ratchets must
            // drop with them instead of persisting rowless.
            if durable && !self.inbound_commit_batch.has_entries() {
                match self.flush_signal_cache().await {
                    Ok(()) => {
                        self.flush_offline_receipts();
                        self.signal_cache.clear_after_flush().await;
                        DrainOutcome::Completed
                    }
                    // Committed/acked state the server never redelivers: keep
                    // it resident so the next successful flush persists it.
                    // Safe to carry across the reconnect — the teardown
                    // generation bump plus the post-permit re-check mean no
                    // late decrypt can mix rowless advances into it.
                    Err(e) => {
                        log::error!(
                            "cleanup_connection_state: signal cache flush failed, keeping cache to avoid dropping Signal state: {e:?}"
                        );
                        DrainOutcome::Failed
                    }
                }
            } else {
                log::warn!(
                    "cleanup_connection_state: dropping unflushed Signal state for uncommitted drain entries; the server redelivers them"
                );
                self.signal_cache.clear().await;
                DrainOutcome::Failed
            }
        };
        match wacore::runtime::timeout(&*self.runtime, limit, settle).await {
            Ok(outcome) => outcome,
            Err(_) => {
                // Drop the whole dirty cache — do NOT sample has_entries() (it
                // races the permit holder we timed out on, per the earlier
                // review). A hook-timeout is the common case: the settle held
                // the permit and its ReinsertGuard has restored the entries by
                // now, and clearing drops their rowless advances safely (rows
                // never persisted → server redelivers). The rare case is a
                // worker hung mid-decrypt holding the permit; it may have
                // advanced a ratchet WITHOUT enqueueing an entry (SKDM-only),
                // which the reset-coupled clear cannot see — so the only
                // rowless-safe action is to clear unconditionally here. The one
                // thing this can drop is committed state a PRIOR connection's
                // failed flush retained AND no flush since re-persisted: a
                // total-storage-outage corner where the system is already
                // degraded, and losing redeliverable-on-reauth state beats
                // silently acking a rowless duplicate.
                log::warn!(
                    "Timed out settling the inbound drain during teardown; dropping the dirty Signal cache so no rowless ratchet advance can persist"
                );
                self.signal_cache.clear().await;
                DrainOutcome::TimedOut
            }
        }
    }

    /// [`flush_inbound_commits_under_permit`](Self::flush_inbound_commits_under_permit)
    /// with a deadline, for pre-close receipt gating: a stalled permit holder
    /// or a hung hook must not wedge disconnect/reconnect. On timeout (or a
    /// failed durable write) the entries stay in the batcher unacked — the
    /// caller must not flush buffered receipts or persist the Signal cache
    /// for them, and the server redelivers them on the next connect.
    pub(crate) async fn flush_inbound_commits_bounded(
        self: &Arc<Self>,
        limit: std::time::Duration,
    ) -> DrainOutcome {
        match wacore::runtime::timeout(
            &*self.runtime,
            limit,
            self.flush_inbound_commits_under_permit(false, None, None),
        )
        .await
        {
            Ok(true) => DrainOutcome::Completed,
            Ok(false) => DrainOutcome::Failed,
            Err(_) => {
                log::warn!(
                    "Timed out committing the inbound drain batch during teardown; leaving entries for redelivery"
                );
                DrainOutcome::TimedOut
            }
        }
    }

    /// End-of-drain transition; see
    /// [`flush_inbound_commits_under_permit`](Self::flush_inbound_commits_under_permit).
    /// `generation` scopes it to the connection whose drain is ending, so a
    /// stale finisher that only gets the permit after a reconnect stands down
    /// instead of taking or deactivating the new connection's drain.
    pub(crate) async fn finish_inbound_commit_drain(self: &Arc<Self>, generation: u64) -> bool {
        self.flush_inbound_commits_under_permit(true, None, Some(generation))
            .await
    }

    /// Drain-mode Signal flush that reports success instead of swallowing it:
    /// the drain call sites gate the buffered offline-receipt flush on this,
    /// because an SKDM receipt must never be sent while its sender-key state
    /// is only in the cache. On failure the cache keeps its dirty entries for
    /// a later retry.
    async fn drain_signal_flush_reporting(&self) -> bool {
        #[cfg(test)]
        if self
            .inbound_commit_batch
            .fail_flushes
            .load(Ordering::Acquire)
        {
            return false;
        }
        match self.flush_signal_cache().await {
            Ok(()) => true,
            Err(e) => {
                log::error!(
                    "Failed to flush signal cache (commit_batch): {e:?}; holding buffered receipts for redelivery"
                );
                false
            }
        }
    }

    /// Commit any accumulated entries while the caller ALREADY HOLDS the
    /// processing permit (mid-stanza recovery paths like UntrustedIdentity),
    /// so a full-cache Signal flush that follows cannot persist ratchet
    /// advances for entries without a durable buffered row. Returns whether
    /// that follow-up flush is safe: on a failed commit the entries are back
    /// in the batcher (unbuffered), and the caller must skip its flush.
    #[must_use = "a false result means the entries are back in the batcher and the follow-up Signal flush must be skipped"]
    pub(crate) async fn commit_inbound_batch_holding_permit(self: &Arc<Self>) -> bool {
        let batch = self.inbound_commit_batch.take();
        if batch.is_empty() {
            return true;
        }
        // Deliberately NOT completing a pending deferred transition here even
        // on a durable commit: the caller holds a permit from the old
        // single-permit semaphore and follows up with a raw whole-cache
        // flush, which is only safe while that permit excludes every other
        // stanza — widening to 64 permits first would let new workers be
        // mid-decrypt under it. The deferred-retry loop completes the
        // transition moments later, outside any raw-flush window.
        self.commit_pending_batch(
            batch.entries.into(),
            BatchOrigin::OfflineDrain,
            batch.commit_ticket,
        )
        .await
    }

    /// Collapse deliveries of one message inside a batch, keeping the first.
    ///
    /// Returns the input untouched when there is nothing to drop, which is
    /// every live batch (they hold one message) and almost every drained one.
    ///
    /// A repeat is identity *and* content: a resend re-encrypts the same
    /// message, so the decoded protos are equal. Identity alone is not enough,
    /// because one stanza id can carry several genuinely different payloads
    /// that share an `info`. The msmsg loop in `handle_incoming_message`
    /// dispatches every bot-reply part under one id, and a drain batch can hold
    /// an unresolved secret envelope beside the retry that resolved to the
    /// inner message; collapsing on identity alone would drop those and ack
    /// them, losing content with nothing left to redeliver it. Comparing the
    /// message keeps them and costs a structural compare only where identities
    /// actually collide, which is the rare case this whole function exists for.
    fn dedup_batch_by_message(&self, items: Arc<[PendingDelivery]>) -> Arc<[PendingDelivery]> {
        if items.len() < 2 {
            return items;
        }
        /// Distinct payloads compared per identity before the collapse gives
        /// up on it. A resend repeats one message, so a real duplicate is
        /// found against the first or second entry; an identity accumulating
        /// more than this is not the pattern this function exists for, and
        /// without a bound a participant could reuse one id across a full
        /// 400-message drain batch and force ~80k structural compares on the
        /// receive path while it holds the processing permit. Giving up keeps
        /// everything, which is the safe direction.
        const MAX_COMPARED_PER_ID: usize = 8;
        /// A kept payload, with its wire form filled in only if some later
        /// item in the batch turns out to share its identity.
        struct Kept {
            info: Arc<MessageInfo>,
            message: Arc<waproto::whatsapp::Message>,
            encoded: Option<Vec<u8>>,
            origin: Option<raw_pending::DeliveryOrigin>,
        }
        /// Compare the wire form rather than the decoded proto. `wa::Message`
        /// derives `PartialEq`, but nothing else calls it, so one `==` here
        /// makes the whole message tree's comparison code live and costs
        /// ~236 KiB of binary. Encoding is already instantiated for the commit
        /// path's durable write, so this adds no code and one memcmp.
        fn wire(msg: &waproto::whatsapp::Message) -> Vec<u8> {
            let mut buf = Vec::with_capacity(waproto::codec::message_encoded_len(msg));
            waproto::codec::message_encode_into(msg, &mut buf);
            buf
        }
        type Seen = std::collections::HashMap<DispatchKey, Vec<Kept>>;
        fn keep(seen: &mut Seen, item: &PendingDelivery) -> bool {
            let kept = seen.entry(Client::dispatch_key(&item.info)).or_default();
            if kept.len() >= MAX_COMPARED_PER_ID {
                return true;
            }
            // Encoded lazily and at most once per side, so a batch whose
            // identities are all distinct (every live batch, and almost every
            // drained one) never encodes anything here.
            let mut candidate: Option<Vec<u8>> = None;
            for k in kept.iter_mut() {
                // Never collapse two dispatches of one stanza. They share the
                // `MessageInfo` the stanza was parsed into, which the msmsg
                // loop hands to every bot-reply part, so equal parts are not a
                // repeat of each other. Best-effort by construction: two
                // separate stanzas always allocate separate infos, so this
                // never exempts a genuine resend, while a part whose info was
                // copied on write falls through to the content compare.
                if let (Some(left), Some(right)) = (k.origin, item.origin) {
                    if left.stanza == right.stanza && left.position != right.position {
                        continue;
                    }
                } else if Arc::ptr_eq(&k.info, &item.info) {
                    continue;
                }
                if Arc::ptr_eq(&k.message, &item.message) {
                    return false;
                }
                let candidate = candidate.get_or_insert_with(|| wire(&item.message));
                let existing = k.encoded.get_or_insert_with(|| wire(&k.message));
                if *existing == *candidate
                    || (k.message.sender_key_distribution_message.is_set()
                        || item.message.sender_key_distribution_message.is_set())
                        && MessageDispatch::fingerprint_encoded(
                            existing,
                            k.message.sender_key_distribution_message.is_set(),
                        ) == MessageDispatch::fingerprint_encoded(
                            candidate,
                            item.message.sender_key_distribution_message.is_set(),
                        )
                {
                    return false;
                }
            }
            kept.push(Kept {
                info: Arc::clone(&item.info),
                message: Arc::clone(&item.message),
                encoded: candidate,
                origin: item.origin,
            });
            true
        }
        let mut seen = Seen::with_capacity(items.len());
        if items.iter().all(|item| keep(&mut seen, item)) {
            return items;
        }
        seen.clear();
        let kept: Arc<[PendingDelivery]> = items
            .iter()
            .filter(|item| keep(&mut seen, item))
            .cloned()
            .collect();
        // Counted like any other suppression: what this drops never reaches a
        // consumer, so leaving it out would make the metric disagree with what
        // the consumer saw for exactly the deliveries it exists to explain.
        for _ in 0..(items.len() - kept.len()) {
            self.duplicate_dispatch_suppressed
                .fetch_add(1, Ordering::Relaxed);
            wacore::telemetry::recv("duplicate_resend");
        }
        kept
    }

    /// Commit one batch: durable buffer → Signal flush → hook → event →
    /// conditional cleanup → ACK. A hook-confirmed event must precede cleanup
    /// so partially deleted rows cannot lose publication. Completed dispatch
    /// claims suppress a second event on cleanup retry or reconnect. The
    /// no-hook path retains its at-most-once order: flush → ACK → event.
    /// Nothing is acked or observable before its durability boundary.
    ///
    /// Drain commits also flush the Signal cache (bulk signal-store commit per
    /// snapshot, WA Web ordering); live commits leave it to the per-stanza
    /// flush at the end of processing. A drain batch whose durable write fails
    /// (or whose future is cancelled by a bounded teardown flush before it) is
    /// restored to the batcher, so "entries still batched" stays an accurate
    /// signal for teardown's flush-vs-drop decision.
    ///
    /// Returns whether the durable state (buffer rows + Signal flush)
    /// committed — the gate for flushing buffered offline receipts. A hook
    /// failure still returns `true`: its rows are durable and the replay path
    /// retries it, so already-buffered receipts of other messages stay safe.
    #[cfg(test)]
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.commit_batch", level = "debug", skip_all, fields(count = items.len())))]
    pub(crate) async fn commit_inbound_batch(
        self: &Arc<Self>,
        items: Arc<[InboundMessage]>,
        origin: BatchOrigin,
        commit_ticket: Option<InboundCommitTicket>,
    ) -> bool {
        let pending: Arc<[PendingDelivery]> = items.iter().cloned().map(Into::into).collect();
        self.commit_pending_batch(pending, origin, commit_ticket)
            .await
    }

    pub(super) async fn commit_pending_batch(
        self: &Arc<Self>,
        items: Arc<[PendingDelivery]>,
        origin: BatchOrigin,
        commit_ticket: Option<InboundCommitTicket>,
    ) -> bool {
        self.commit_pending_batch_result(items, origin, commit_ticket, true)
            .await
            .persisted
    }

    pub(super) async fn replay_pending_batch(
        self: &Arc<Self>,
        items: Arc<[PendingDelivery]>,
    ) -> bool {
        self.commit_pending_batch_result(items, BatchOrigin::Live, None, true)
            .await
            .delivered
    }

    async fn commit_pending_batch_result(
        self: &Arc<Self>,
        items: Arc<[PendingDelivery]>,
        origin: BatchOrigin,
        commit_ticket: Option<InboundCommitTicket>,
        acknowledge: bool,
    ) -> PendingCommitOutcome {
        let future =
            self.commit_pending_batch_result_inner(items, origin, commit_ticket, acknowledge);
        let mut future = std::pin::pin!(future);
        std::future::poll_fn(|cx| poll_pending_commit(future.as_mut(), cx)).await
    }

    async fn commit_pending_batch_result_inner(
        self: &Arc<Self>,
        items: Arc<[PendingDelivery]>,
        origin: BatchOrigin,
        commit_ticket: Option<InboundCommitTicket>,
        acknowledge: bool,
    ) -> PendingCommitOutcome {
        let is_drain = matches!(origin, BatchOrigin::OfflineDrain);
        debug_assert!(is_drain || commit_ticket.is_none());
        if items.is_empty() {
            debug_assert!(commit_ticket.is_none());
            return PendingCommitOutcome::DELIVERED;
        }
        // A drain can accumulate two deliveries of one message before the batch
        // commits: both read the gate before either claim landed, so the
        // collapse has to happen here too. Every stanza that arrived is still
        // acked from `arrived`; only what the hook and the consumer see is
        // reduced to one copy.
        let arrived = Arc::clone(&items);
        // Already delivered entries still need persistence and cleanup, but
        // must not hide an equivalent entry that the consumer has not seen.
        let items = if items.iter().any(|entry| entry.consumer_claimed) {
            items
                .iter()
                .filter(|entry| !entry.consumer_claimed)
                .cloned()
                .collect()
        } else {
            items
        };
        let items = if self.dispatch_gate_enabled() {
            self.dedup_batch_by_message(items)
        } else {
            items
        };
        let mut reinsert = ReinsertGuard {
            batcher: &self.inbound_commit_batch,
            items: is_drain.then(|| Arc::clone(&arrived)),
            commit_ticket,
        };
        #[cfg(test)]
        if self
            .inbound_commit_batch
            .fail_commits
            .load(Ordering::Acquire)
        {
            return PendingCommitOutcome::FAILED;
        }

        // Persist every arrived physical key before collapsing consumer aliases.
        // Cleanup then compares each retained revision, including spellings
        // whose identical projection is represented by another hook item.
        let pending = Arc::clone(&arrived);
        let deliveries = items;
        let items: Arc<[InboundMessage]> =
            deliveries.iter().map(|entry| entry.item.clone()).collect();
        let arrived: Arc<[InboundMessage]> =
            arrived.iter().map(|entry| entry.item.clone()).collect();

        // Rides on the dispatched event so a consumer that the hook already
        // fed can skip re-materializing the batch.
        let mut hook_committed = false;
        let mut cleanup = None;
        let dispatch_gate = self.dispatch_gate_enabled();
        let mut fingerprints =
            smallvec::SmallVec::<[(Option<DispatchFingerprint>, bool); 1]>::new();
        if let Some(hook) = self.inbound_durability_hook() {
            if dispatch_gate {
                fingerprints.reserve_exact(items.len());
            }
            let stored = match self.persist_pending_group(&pending, is_drain).await {
                Ok(stored) => stored,
                Err(error) => {
                    log::error!("Failed to buffer inbound occurrence group: {error:?}");
                    return PendingCommitOutcome::FAILED;
                }
            };
            if dispatch_gate {
                let mut scratch = Vec::new();
                for delivery in deliveries.iter() {
                    fingerprints.push((
                        (!crate::features::message_edit::carries_secret_encrypted(
                            &delivery.message,
                        ))
                        .then(|| {
                            MessageDispatch::fingerprint_into(&delivery.message, &mut scratch)
                        }),
                        delivery.storage_sender.is_some(),
                    ));
                }
            }
            drop(deliveries);
            // A failed flush reports not-durable so buffered receipts are held
            // back. The guard is still armed: the rows are in place and
            // re-storing them is idempotent (replace-into), so the restored
            // entries make the batch retryable THIS session — the retry
            // re-runs the full commit (rows → flush → hook → event → cleanup → ACK)
            // instead of parking the messages until a reconnect replay.
            if is_drain && !self.drain_signal_flush_reporting().await {
                return PendingCommitOutcome::FAILED;
            }
            // Rows durable and Signal flushed: from here on a cancelled
            // future must not restore the entries — redelivery replays from
            // the rows.
            reinsert.mark_durable();
            self.inbound_commit_batch.live_retention.stored(&pending);

            if !items.is_empty()
                && let Err(e) = hook.on_messages(self.clone(), &items).await
            {
                log::warn!(
                    "Inbound durability hook failed for batch of {}; suppressing acks for redelivery: {e:?}",
                    items.len()
                );
                return PendingCommitOutcome::BUFFERED;
            }
            hook_committed = true;

            // A commit covering only a subset cannot delete prior failed
            // occurrences. Redelivery replays the complete retained group.
            if stored.iter().any(|row| !row.complete) {
                return PendingCommitOutcome::BUFFERED;
            }
            cleanup = Some(stored);
        } else {
            drop(deliveries);
            // No hook = at-most-once: the flush is the durable point, so a
            // failure restores the entries (guard still armed) for a retry.
            if is_drain && !self.drain_signal_flush_reporting().await {
                return PendingCommitOutcome::FAILED;
            }
        }

        reinsert.mark_durable();
        #[cfg(test)]
        self.inbound_commit_batch
            .publication_reached
            .store(true, Ordering::Release);
        // Admission and dispatch cannot yield after the commit boundary. Only
        // admission holds the cache lock; consumer callbacks never own it.
        let mut retained: Option<Vec<InboundMessage>> = None;
        let mut publication = PublicationGuard::default();
        // One encode buffer serves fingerprints not computed in the hook path.
        let mut encode_scratch = Vec::new();
        for (index, item) in items.iter().enumerate() {
            // A later PDO may carry another part under this id. Comparing it
            // without retaining plaintext requires this ordinary digest now.
            let (fingerprint, stored_replay) =
                fingerprints.get(index).copied().unwrap_or_else(|| {
                    (
                        (dispatch_gate
                            && !crate::features::message_edit::carries_secret_encrypted(
                                &item.message,
                            ))
                        .then(|| {
                            MessageDispatch::fingerprint_into(&item.message, &mut encode_scratch)
                        }),
                        false,
                    )
                });
            // Only retained-row replay skips a completed hook publication.
            // Fresh multipart occurrences still use ordinary admission.
            let suppress = stored_replay
                && fingerprint.is_some_and(|fp| self.completed_hook_publication(&item.info, &fp))
                || self.admit_message_dispatch(
                    &item.info,
                    false,
                    fingerprint,
                    hook_committed,
                    &mut publication,
                );
            if suppress {
                retained.get_or_insert_with(|| items[..index].to_vec());
                self.duplicate_dispatch_suppressed
                    .fetch_add(1, Ordering::Relaxed);
                wacore::telemetry::recv("duplicate_resend");
            } else if let Some(retained) = &mut retained {
                retained.push(item.clone());
            }
        }
        let items = retained.map(Arc::from).unwrap_or(items);
        let mut unpublished = Some(items);
        if let Some(rows) = cleanup {
            // The hook has committed the complete batch. Publish and finalize
            // its dedup claims before the first destructive await: cleanup may
            // partially apply, return an error, or outlive a cancelled caller.
            // ACK remains gated by every conditional deletion succeeding.
            if let Some(items) = unpublished.take() {
                self.publish_pending_messages(items, origin, hook_committed);
            }
            std::mem::take(&mut publication).complete();
            if let Err(error) = self.cleanup_pending_group(&rows).await {
                log::warn!("Pending occurrence cleanup conflict; suppressing ACK: {error:?}");
                return PendingCommitOutcome::BUFFERED;
            }
        }
        // The no-hook path still schedules receipts before synchronous
        // consumer code can block or panic.
        let mut acknowledged = std::collections::HashSet::with_capacity(arrived.len());
        for item in pending.iter() {
            // The scope belongs to this arrival, including after cancellation.
            // Never redirect a late commit into another attempt's scope.
            if acknowledge && acknowledged.insert(Arc::as_ptr(&item.info) as usize) {
                match &item.receipt {
                    Some(scope) if scope.defer() => {}
                    Some(_) => self.ack_received_message_unscoped(&item.info),
                    None => self.ack_received_message(&item.info),
                }
            }
        }
        if is_drain {
            self.flush_offline_receipts();
        }
        if let Some(items) = unpublished {
            self.publish_pending_messages(items, origin, hook_committed);
        }
        publication.complete();
        PendingCommitOutcome::DELIVERED
    }

    #[inline(never)]
    fn publish_pending_messages(
        &self,
        items: Arc<[InboundMessage]>,
        origin: BatchOrigin,
        hook_committed: bool,
    ) {
        if !items.is_empty() {
            self.core.event_bus.dispatch(Event::Messages(
                MessageBatch::builder()
                    .messages(items)
                    .origin(origin)
                    .hook_committed(hook_committed)
                    .build(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client_with_failing_http;
    use crate::types::durability_hook::InboundDurabilityHook;
    use crate::types::message::{MessageInfo, MessageSource};
    use std::sync::Mutex;
    use wacore::types::events::ChannelEventHandler;

    struct RecordingHook {
        batches: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for RecordingHook {
        async fn on_messages(
            &self,
            _client: Arc<Client>,
            batch: &[InboundMessage],
        ) -> anyhow::Result<()> {
            self.batches
                .lock()
                .expect("hook lock")
                .push(batch.iter().map(|m| m.info.id.to_string()).collect());
            Ok(())
        }
    }

    fn item(id: &str) -> InboundMessage {
        InboundMessage::builder()
            .message(Arc::new({
                let mut proto_fixture = wa::Message::default();
                proto_fixture.conversation = Some(format!("text {id}"));
                proto_fixture
            }))
            .info(Arc::new(MessageInfo {
                id: id.into(),
                source: MessageSource {
                    chat: "100@g.us".parse().unwrap(),
                    sender: "200@s.whatsapp.net".parse().unwrap(),
                    ..Default::default()
                },
                ..Default::default()
            }))
            .build()
    }

    async fn capture_pending_receipts(
        client: &Arc<Client>,
    ) -> Arc<crate::transport::mock::CapturingMockTransport> {
        let transport = Arc::new(crate::transport::mock::CapturingMockTransport::new());
        let socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport.clone(),
            wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
            wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(socket));
        client.set_connected_for_test(true);
        super::super::tests::seed_test_pn(client).await;
        transport
    }

    #[tokio::test]
    async fn raw_pending_completed_hook_claim_preserves_fresh_multipart() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let client = crate::test_utils::create_test_client_with_backend(backend).await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let original = item("FRESH_MULTIPART");
        let producer = raw_pending::StanzaOrigin::new();
        let bytes = bytes::Bytes::from(waproto::codec::message_to_vec(&original.message));
        let part = |position| {
            let mut delivery = PendingDelivery::from(original.clone());
            delivery.origin = Some(raw_pending::DeliveryOrigin {
                stanza: producer,
                position,
            });
            delivery.carrier = Some((Some(position as u32), bytes.clone()));
            delivery
        };
        assert!(
            client
                .commit_pending_batch_result(
                    vec![part(0), part(1)].into(),
                    BatchOrigin::OfflineDrain,
                    None,
                    false
                )
                .await
                .delivered
        );
        let event = rx.try_recv().unwrap();
        let Event::Messages(batch) = event.as_ref() else {
            panic!("expected message batch")
        };
        assert_eq!(
            batch.len(),
            2,
            "equal parts within one producer are not repeats"
        );
        assert_eq!(hook.batches.lock().unwrap()[0].len(), 2);
        assert!(
            client
                .commit_pending_batch_result(vec![part(2)].into(), BatchOrigin::Live, None, false)
                .await
                .delivered
        );
        let event = rx.try_recv().unwrap();
        let Event::Messages(batch) = event.as_ref() else {
            panic!("expected message batch")
        };
        assert_eq!(
            batch.len(),
            1,
            "a fresh later part must survive the completed hook claim"
        );
        assert_eq!(hook.batches.lock().unwrap()[1].len(), 1);
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn raw_pending_partial_alias_cleanup_redelivery_does_not_republish() {
        for failed_index in [0, 1] {
            use wacore::store::in_memory::InMemoryBackend;
            use wacore::store::traits::ProtocolStore;
            let backend = Arc::new(InMemoryBackend::new());
            let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
            let transport = capture_pending_receipts(&client).await;
            let hook = Arc::new(RecordingHook {
                batches: Mutex::new(Vec::new()),
            });
            client
                .inbound_durability_hook
                .set(hook.clone())
                .ok()
                .unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let bare = PendingDelivery::from(item("ALIAS_PARTIAL"));
            let mut legacy = item("ALIAS_PARTIAL");
            let mut legacy_info = (*legacy.info).clone();
            legacy_info.source.sender = "200:75@s.whatsapp.net".parse().unwrap();
            legacy.info = Arc::new(legacy_info);
            let mut legacy = PendingDelivery::from(legacy);
            legacy.storage_sender = Some(Arc::from("200:75@s.whatsapp.net"));
            let payload = waproto::codec::message_to_vec(&bare.message);
            for sender in ["200:75@s.whatsapp.net", "200@s.whatsapp.net"] {
                backend
                    .store_pending_inbound("100@g.us", sender, "ALIAS_PARTIAL", &payload)
                    .await
                    .unwrap();
            }
            backend.fail_one_pending_delete(failed_index, false);
            let outcome = client
                .commit_pending_batch_result(
                    vec![legacy, bare.clone()].into(),
                    BatchOrigin::OfflineDrain,
                    None,
                    true,
                )
                .await;
            assert!(outcome.persisted);
            assert!(!outcome.delivered);
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200:75@s.whatsapp.net", "ALIAS_PARTIAL")
                    .await
                    .unwrap()
                    .is_none()
                    == (failed_index != 0)
            );
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "ALIAS_PARTIAL")
                    .await
                    .unwrap()
                    .is_some()
                    == (failed_index == 1)
            );
            let event = rx
                .try_recv()
                .expect("deleted alias must not lose the consumer event");
            let Event::Messages(batch) = event.as_ref() else {
                panic!("expected message batch")
            };
            assert_eq!(batch.len(), 1, "physical aliases are one consumer message");
            assert!(batch.hook_committed);
            assert_eq!(hook.batches.lock().unwrap()[0].len(), 1);
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            assert_eq!(
                super::super::tests::delivery_receipts_for(&transport.sent(), "ALIAS_PARTIAL"),
                0
            );
            backend.set_fail_pending_delete(None);
            client.cleanup_connection_state().await;
            client.enter_live_mode_for_tests();
            let transport = capture_pending_receipts(&client).await;
            // Use the retained-row redelivery path, not a synthetic re-commit of
            // the deleted alias. Its new MessageInfo must find the completed claim.
            assert!(client.ack_or_replay_to_hook(&bare.info).await);
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "ALIAS_PARTIAL")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200:75@s.whatsapp.net", "ALIAS_PARTIAL")
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(rx.try_recv().is_err());
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            assert_eq!(
                super::super::tests::delivery_receipts_for(&transport.sent(), "ALIAS_PARTIAL"),
                1
            );
        }
    }

    #[tokio::test]
    async fn raw_pending_partial_cleanup_conflict_preserves_successor_and_event() {
        use wacore::store::in_memory::InMemoryBackend;
        use wacore::store::traits::ProtocolStore;
        struct ReplacingHook {
            backend: Arc<InMemoryBackend>,
            id: &'static str,
        }
        #[async_trait::async_trait]
        impl InboundDurabilityHook for ReplacingHook {
            async fn on_messages(
                &self,
                _: Arc<Client>,
                _: &[InboundMessage],
            ) -> anyhow::Result<()> {
                // Another writer installs a successor after the hook commits.
                self.backend
                    .store_pending_inbound("100@g.us", "200@s.whatsapp.net", self.id, &[9])
                    .await?;
                Ok(())
            }
        }
        let ids = ["CAS_FIRST", "CAS_SECOND", "CAS_THIRD"];
        for failed_index in [1, 2, 0] {
            let backend = Arc::new(InMemoryBackend::new());
            let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
            let transport = capture_pending_receipts(&client).await;
            client
                .inbound_durability_hook
                .set(Arc::new(ReplacingHook {
                    backend: backend.clone(),
                    id: ids[failed_index],
                }))
                .ok()
                .unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let entries: Arc<[PendingDelivery]> = ids
                .iter()
                .map(|id| PendingDelivery::from(item(id)))
                .collect();
            let outcome = client
                .commit_pending_batch_result(
                    Arc::clone(&entries),
                    BatchOrigin::OfflineDrain,
                    None,
                    true,
                )
                .await;
            assert!(outcome.persisted);
            assert!(!outcome.delivered);
            let event = rx
                .try_recv()
                .expect("a cleanup conflict cannot erase the hook commit");
            let Event::Messages(batch) = event.as_ref() else {
                panic!("expected message batch")
            };
            assert!(batch.hook_committed);
            assert_eq!(
                batch.iter().map(|m| m.info.id.as_str()).collect::<Vec<_>>(),
                ids
            );
            for (index, id) in ids.iter().enumerate() {
                let stored = backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                    .await
                    .unwrap();
                assert_eq!(stored.is_none(), index != failed_index);
                if index == failed_index {
                    assert_eq!(stored.unwrap(), [9]);
                }
            }
            // Invalid successor bytes fail closed on retry; they must neither
            // be deleted nor authorize ACK or a second publication.
            let retry = client
                .commit_pending_batch_result(entries, BatchOrigin::Live, None, true)
                .await;
            assert!(!retry.delivered);
            assert_eq!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", ids[failed_index])
                    .await
                    .unwrap()
                    .unwrap(),
                [9]
            );
            assert!(rx.try_recv().is_err());
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            for id in ids {
                assert_eq!(
                    super::super::tests::delivery_receipts_for(&transport.sent(), id),
                    0
                );
            }
        }
    }

    #[tokio::test]
    async fn raw_pending_cancelled_partial_cleanup_keeps_publication_claims() {
        use wacore::store::in_memory::InMemoryBackend;
        use wacore::store::traits::ProtocolStore;
        for prefix in 0..3 {
            let backend = Arc::new(InMemoryBackend::new());
            let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
            let transport = capture_pending_receipts(&client).await;
            let hook = Arc::new(RecordingHook {
                batches: Mutex::new(Vec::new()),
            });
            client.inbound_durability_hook.set(hook).ok().unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let ids = ["CANCEL_FIRST", "CANCEL_SECOND", "CANCEL_THIRD"];
            let entries: Arc<[PendingDelivery]> = ids
                .iter()
                .map(|id| PendingDelivery::from(item(id)))
                .collect();
            let gate = Arc::new(async_lock::Barrier::new(2));
            backend.park_pending_delete(prefix, Arc::clone(&gate));
            let worker = tokio::spawn({
                let client = Arc::clone(&client);
                let entries = Arc::clone(&entries);
                async move {
                    client
                        .commit_pending_batch_result(entries, BatchOrigin::OfflineDrain, None, true)
                        .await
                }
            });
            tokio::time::timeout(std::time::Duration::from_secs(5), gate.wait())
                .await
                .unwrap();
            worker.abort();
            assert!(matches!(worker.await, Err(error) if error.is_cancelled()));
            let event = rx
                .try_recv()
                .expect("cleanup cancellation must not erase committed publication");
            let Event::Messages(batch) = event.as_ref() else {
                panic!("expected message batch")
            };
            assert_eq!(
                batch.iter().map(|m| m.info.id.as_str()).collect::<Vec<_>>(),
                ids
            );
            assert!(batch.hook_committed);
            for (index, id) in ids.iter().enumerate() {
                assert_eq!(
                    backend
                        .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                        .await
                        .unwrap()
                        .is_none(),
                    index < prefix as usize
                );
            }
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            for id in ids {
                assert_eq!(
                    super::super::tests::delivery_receipts_for(&transport.sent(), id),
                    0
                );
            }
            backend.set_fail_pending_delete(None);
            let generation = client.connection_generation.load(Ordering::Acquire);
            client.cleanup_connection_state().await;
            assert_eq!(
                client.connection_generation.load(Ordering::Acquire),
                generation + 1
            );
            client.enter_live_mode_for_tests();
            let transport = capture_pending_receipts(&client).await;
            for entry in entries.iter() {
                client.ack_or_replay_to_hook(&entry.info).await;
                assert!(
                    backend
                        .get_pending_inbound("100@g.us", "200@s.whatsapp.net", &entry.info.id)
                        .await
                        .unwrap()
                        .is_none()
                );
            }
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            for id in ids {
                assert_eq!(
                    super::super::tests::delivery_receipts_for(&transport.sent(), id),
                    1
                );
            }
            assert!(
                rx.try_recv().is_err(),
                "cancelled cleanup must retain completed dedup claims"
            );
        }
    }

    #[tokio::test]
    async fn raw_pending_partial_cleanup_keeps_whole_batch_published() {
        use wacore::store::in_memory::InMemoryBackend;
        use wacore::store::traits::ProtocolStore;
        for failed_index in [1, 2, 0] {
            let backend = Arc::new(InMemoryBackend::new());
            let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
            let transport = capture_pending_receipts(&client).await;
            let hook = Arc::new(RecordingHook {
                batches: Mutex::new(Vec::new()),
            });
            client
                .inbound_durability_hook
                .set(hook.clone())
                .ok()
                .unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let ids = ["PARTIAL_FIRST", "PARTIAL_SECOND", "PARTIAL_THIRD"];
            let entries: Arc<[PendingDelivery]> = ids
                .iter()
                .map(|id| PendingDelivery::from(item(id)))
                .collect();
            backend.fail_one_pending_delete(failed_index, false);
            let outcome = client
                .commit_pending_batch_result(
                    Arc::clone(&entries),
                    BatchOrigin::OfflineDrain,
                    None,
                    true,
                )
                .await;
            assert!(outcome.persisted);
            assert!(
                !outcome.delivered,
                "unfinished cleanup must not authorize ACK"
            );
            assert_eq!(hook.batches.lock().unwrap()[0], ids);
            for (index, id) in ids.iter().enumerate() {
                assert_eq!(
                    backend
                        .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                        .await
                        .unwrap()
                        .is_none(),
                    index != failed_index as usize
                );
            }
            let event = rx
                .try_recv()
                .expect("a hook-committed batch must not lose its deleted prefix");
            let Event::Messages(batch) = event.as_ref() else {
                panic!("expected message batch")
            };
            assert!(batch.hook_committed);
            assert_eq!(
                batch.iter().map(|m| m.info.id.as_str()).collect::<Vec<_>>(),
                ids
            );
            assert!(rx.try_recv().is_err());
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            for id in ids {
                assert_eq!(
                    super::super::tests::delivery_receipts_for(&transport.sent(), id),
                    0
                );
            }
            backend.set_fail_pending_delete(None);
            // Real connection cleanup retires the generation and resets drain
            // state; completed publication claims must survive that transition.
            let generation = client.connection_generation.load(Ordering::Acquire);
            client.cleanup_connection_state().await;
            assert_eq!(
                client.connection_generation.load(Ordering::Acquire),
                generation + 1
            );
            client.enter_live_mode_for_tests();
            let transport = capture_pending_receipts(&client).await;
            for entry in entries.iter() {
                client.ack_or_replay_to_hook(&entry.info).await;
                assert!(
                    backend
                        .get_pending_inbound("100@g.us", "200@s.whatsapp.net", &entry.info.id)
                        .await
                        .unwrap()
                        .is_none()
                );
            }
            crate::test_utils::wait_for_outbound_tasks(&client).await;
            for id in ids {
                assert_eq!(
                    super::super::tests::delivery_receipts_for(&transport.sent(), id),
                    1
                );
            }
            assert!(
                rx.try_recv().is_err(),
                "cleanup retry must not duplicate publication"
            );
            for id in ids {
                assert!(
                    backend
                        .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                        .await
                        .unwrap()
                        .is_none()
                );
            }
        }
    }

    #[tokio::test]
    async fn raw_pending_cleanup_error_after_delete_still_publishes() {
        use wacore::store::in_memory::InMemoryBackend;
        use wacore::store::traits::ProtocolStore;
        for after_commit in [false, true] {
            let backend = Arc::new(InMemoryBackend::new());
            let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
            let hook = Arc::new(RecordingHook {
                batches: Mutex::new(Vec::new()),
            });
            client
                .inbound_durability_hook
                .set(hook.clone())
                .ok()
                .unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            backend.set_fail_pending_delete(Some(after_commit));
            let outcome = client
                .commit_pending_batch_result(
                    vec![PendingDelivery::from(item("AMBIGUOUS_DELETE"))].into(),
                    BatchOrigin::Live,
                    None,
                    false,
                )
                .await;
            assert!(outcome.persisted);
            assert_eq!(outcome.delivered, after_commit);
            assert_eq!(hook.batches.lock().unwrap().len(), 1);
            assert!(rx.try_recv().is_ok(), "the hook committed before cleanup");
            assert!(rx.try_recv().is_err());
            assert_eq!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "AMBIGUOUS_DELETE")
                    .await
                    .unwrap()
                    .is_none(),
                after_commit
            );
        }
    }

    #[tokio::test]
    async fn raw_pending_group_spellings_share_one_new_row_and_cleanup() {
        let client = create_test_client_with_failing_http("canonical_group_pending").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        client.inbound_durability_hook.set(hook).ok().unwrap();
        let bare = item("CANONICAL_GROUP");
        let mut device = bare.clone();
        let mut info = (*device.info).clone();
        info.source.sender = "200:75@s.whatsapp.net".parse().unwrap();
        device.info = Arc::new(info);
        let outcome = client
            .commit_pending_batch_result(
                vec![PendingDelivery::from(bare), PendingDelivery::from(device)].into(),
                BatchOrigin::Live,
                None,
                false,
            )
            .await;
        assert!(outcome.delivered);
        let backend = client.persistence_manager.backend();
        for sender in ["200@s.whatsapp.net", "200:75@s.whatsapp.net"] {
            assert!(
                backend
                    .get_pending_inbound("100@g.us", sender, "CANONICAL_GROUP")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[tokio::test]
    async fn raw_pending_origin_preserves_parts_but_collapses_retry_projection() {
        let client = create_test_client_with_failing_http("raw_origin_dedup").await;
        let first = raw_pending::StanzaOrigin::new();
        let retry = raw_pending::StanzaOrigin::new();
        let delivery = |origin, position, text: &str| {
            let mut inbound = item("SAME_ID");
            let mut message = wa::Message::default();
            message.conversation = Some(text.into());
            inbound.message = Arc::new(message);
            PendingDelivery {
                carrier: Some((
                    Some(u32::try_from(position).unwrap()),
                    waproto::codec::message_to_vec(&inbound.message).into(),
                )),
                item: inbound,

                retention: None,
                receipt: None,
                storage_sender: None,
                consumer_claimed: false,
                origin: Some(raw_pending::DeliveryOrigin {
                    stanza: origin,
                    position,
                }),
            }
        };
        let arrivals: Arc<[PendingDelivery]> = vec![
            delivery(first, 0, "A"),
            delivery(first, 1, "A"),
            delivery(first, 2, "B"),
            delivery(retry, 0, "A"),
            delivery(retry, 3, "C"),
        ]
        .into();
        let kept = client.dedup_batch_by_message(arrivals.clone());
        let texts: Vec<_> = kept
            .iter()
            .map(|entry| entry.message.conversation.as_deref().unwrap())
            .collect();
        assert_eq!(texts, ["A", "A", "B", "C"]);
        assert_eq!(arrivals.len(), 5, "raw storage still owns every arrival");
        assert_eq!(kept[0].origin, arrivals[0].origin);
        assert_eq!(kept[1].origin, arrivals[1].origin);
    }

    #[tokio::test]
    async fn raw_pending_claimed_entry_cannot_hide_unclaimed_delivery() {
        let client = create_test_client_with_failing_http("claimed_dedup").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let delivery = |claimed| PendingDelivery {
            item: item("CLAIMED_FIRST"),
            carrier: None,
            retention: None,
            receipt: None,
            storage_sender: None,
            consumer_claimed: claimed,
            origin: None,
        };
        let outcome = client
            .commit_pending_batch_result(
                vec![delivery(true), delivery(false)].into(),
                BatchOrigin::Live,
                None,
                false,
            )
            .await;
        assert!(outcome.delivered);
        assert_eq!(
            *hook.batches.lock().unwrap(),
            vec![vec!["CLAIMED_FIRST".to_owned()]]
        );
    }

    #[tokio::test]
    async fn drain_encode_arena_is_reused_then_released_in_live_mode() {
        let client = create_test_client_with_failing_http("drain_arena_lifetime").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        client.inbound_commit_batch.reset();

        let mut capacity = None;
        for id in ["ARENA_1", "ARENA_2"] {
            let mut incoming = item(id);
            incoming.message = Arc::new({
                let mut proto_fixture = wa::Message::default();
                proto_fixture.conversation = Some("x".repeat(16 * 1024));
                proto_fixture
            });
            client.commit_or_batch_inbound(incoming, false).await;
            assert!(
                client
                    .flush_inbound_commits_under_permit(false, None, None)
                    .await
            );
            let storage = client.inbound_commit_batch.pending_storage.lock().await;
            let arena = storage.arena();
            assert!(arena.capacity() >= 16 * 1024);
            if let Some(previous) = capacity {
                assert_eq!(arena.capacity(), previous, "reuse between drain batches");
            }
            capacity = Some(arena.capacity());
        }

        assert_eq!(hook.batches.lock().unwrap().len(), 2);
        assert!(
            client
                .flush_inbound_commits_under_permit(true, None, None)
                .await
        );
        assert!(!client.inbound_commit_batch.is_active());
        assert_eq!(
            client
                .inbound_commit_batch
                .pending_storage
                .lock()
                .await
                .arena()
                .capacity(),
            0
        );
    }

    #[tokio::test]
    async fn forced_live_transition_leaves_an_inflight_encoder_untouched() {
        let batcher = InboundCommitBatcher::default();
        batcher.reset();
        let storage = batcher.pending_storage.lock().await;
        let mut arena = storage.arena();
        arena.extend_from_slice(b"pending durable row");
        batcher.force_live_dropping_entries();
        assert!(!batcher.is_active());
        assert_eq!(&*arena, b"pending durable row");
    }

    // During the drain, messages accumulate and one flush commits them all in
    // arrival order as a single hook call and a single OfflineDrain event.
    #[tokio::test]
    async fn drain_accumulates_then_commits_in_order() {
        let client = create_test_client_with_failing_http("batch_drain").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        client.inbound_commit_batch.reset();
        for id in ["B1", "B2", "B3"] {
            client.commit_or_batch_inbound(item(id), false).await;
        }
        assert!(
            hook.batches.lock().expect("hook lock").is_empty(),
            "sub-threshold entries must accumulate, not commit"
        );

        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;

        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches, vec![vec!["B1", "B2", "B3"]]);

        let event = rx.try_recv().expect("one batch event");
        let batch = event.as_messages().expect("Messages event");
        assert_eq!(batch.origin, BatchOrigin::OfflineDrain);
        let ids: Vec<&str> = batch.iter().map(|m| m.info.id.as_str()).collect();
        assert_eq!(ids, ["B1", "B2", "B3"]);
        assert!(rx.try_recv().is_err(), "exactly one event for the batch");

        // A committed batch leaves no buffered copies behind.
        let backend = client.persistence_manager.backend();
        for id in ["B1", "B2", "B3"] {
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    // The accumulating batch is the largest allocation one client holds (4 MiB
    // of decoded protos at the flush threshold, two orders of magnitude above
    // every cache in the report), so it has to be visible while it accumulates
    // rather than only in aggregate afterwards. The flush assertion pins the
    // other half of `pending_stats`'s contract: it counts what is waiting to be
    // committed, so handing the batch to a commit clears it.
    #[tokio::test]
    async fn memory_report_names_the_uncommitted_drain_batch() {
        let client = create_test_client_with_failing_http("batch_report").await;
        client.inbound_commit_batch.reset();

        let empty = client.memory_report().await;
        assert_eq!(empty.inbound_commit_batch.entries, 0);
        assert_eq!(empty.inbound_commit_batch.bytes, 0);

        for id in ["R1", "R2"] {
            client.commit_or_batch_inbound(item(id), false).await;
        }

        let held = client.memory_report().await;
        assert_eq!(held.inbound_commit_batch.entries, 2);
        assert!(
            held.inbound_commit_batch.bytes > 0,
            "accumulated entries must carry their encoded-byte estimate"
        );
        assert!(
            held.total_estimated_bytes() >= held.inbound_commit_batch.bytes,
            "the batch must reach the report's byte total, not only its own field"
        );

        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;

        let flushed = client.memory_report().await;
        assert_eq!(flushed.inbound_commit_batch.entries, 0);
        assert_eq!(flushed.inbound_commit_batch.bytes, 0);
    }

    // Live traffic commits immediately as a batch of one.
    #[tokio::test]
    async fn live_commits_as_batch_of_one() {
        let client = create_test_client_with_failing_http("batch_live").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        client.commit_or_batch_inbound(item("L1"), false).await;

        assert_eq!(
            hook.batches.lock().expect("hook lock").clone(),
            vec![vec!["L1"]]
        );
        let event = rx.try_recv().expect("live event");
        let batch = event.as_messages().expect("Messages event");
        assert_eq!(batch.origin, BatchOrigin::Live);
        assert_eq!(batch.len(), 1);
    }

    // The size trigger commits a full batch from the stanza-end check.
    #[tokio::test]
    async fn size_trigger_flushes_full_batch() {
        let client = create_test_client_with_failing_http("batch_size").await;
        client.inbound_commit_batch.reset();
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        for i in 0..MAX_BATCH_MESSAGES {
            client
                .commit_or_batch_inbound(item(&format!("S{i}")), false)
                .await;
        }
        client.maybe_flush_inbound_commits().await;

        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches.len(), 1, "one commit for the full batch");
        assert_eq!(batches[0].len(), MAX_BATCH_MESSAGES);
        assert_eq!(batches[0][0], "S0");
        assert_eq!(
            batches[0][MAX_BATCH_MESSAGES - 1],
            format!("S{}", MAX_BATCH_MESSAGES - 1)
        );
    }

    // Without a hook, the drain still batches the event dispatch.
    #[tokio::test]
    async fn drain_without_hook_batches_events() {
        let client = create_test_client_with_failing_http("batch_no_hook").await;
        client.inbound_commit_batch.reset();
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        client.commit_or_batch_inbound(item("N1"), false).await;
        client.commit_or_batch_inbound(item("N2"), false).await;
        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;

        let event = rx.try_recv().expect("one batch event");
        assert_eq!(
            event
                .messages()
                .map(|m| m.info.id.as_str())
                .collect::<Vec<_>>(),
            ["N1", "N2"]
        );
    }

    // End-of-drain transition: the tail batch commits first (as OfflineDrain),
    // the batcher flips to live mode, and anything after commits as Live —
    // never interleaved ahead of the tail.
    #[tokio::test]
    async fn finish_drain_commits_tail_then_switches_to_live() {
        let client = create_test_client_with_failing_http("batch_transition").await;
        client.inbound_commit_batch.reset();
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        client.commit_or_batch_inbound(item("T1"), false).await;
        client.commit_or_batch_inbound(item("T2"), false).await;
        let generation = client.connection_generation.load(Ordering::Acquire);
        client.finish_inbound_commit_drain(generation).await;
        assert!(!client.inbound_commit_batch.is_active());
        // A message arriving after the transition commits immediately as Live.
        client.commit_or_batch_inbound(item("T3"), false).await;

        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(
            batches,
            vec![vec!["T1", "T2"], vec!["T3"]],
            "tail batch must commit before, and separately from, live traffic"
        );
        let first = rx.try_recv().expect("tail event");
        assert_eq!(
            first.as_messages().expect("Messages").origin,
            BatchOrigin::OfflineDrain
        );
        let second = rx.try_recv().expect("live event");
        assert_eq!(
            second.as_messages().expect("Messages").origin,
            BatchOrigin::Live
        );
    }

    // reset() drops uncommitted entries: no hook call, no event, and the
    // pending buffer was never written (the server redelivers instead).
    #[tokio::test]
    async fn clear_drops_uncommitted_entries() {
        let client = create_test_client_with_failing_http("batch_clear").await;
        client.inbound_commit_batch.reset();
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        client.commit_or_batch_inbound(item("C1"), false).await;
        client.inbound_commit_batch.reset();
        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;

        assert!(hook.batches.lock().expect("hook lock").is_empty());
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn reset_cancels_a_tracked_deferred_commit() {
        let client = create_test_client_with_failing_http("batch_tracked_reset").await;
        client.inbound_commit_batch.reset();

        let ticket = match client.commit_or_batch_inbound(item("C2"), true).await {
            InboundCommitState::Deferred(Some(ticket)) => ticket,
            _ => panic!("drain commit must be tracked"),
        };
        assert_eq!(ticket.state(), InboundCommitTicketState::Pending);

        assert!(client.inbound_commit_batch.reset());
        assert_eq!(ticket.state(), InboundCommitTicketState::Dropped);
    }

    // A failed end-of-drain tail must NOT switch to live mode: the restored
    // entries would sit in an inactive batcher while per-stanza full-cache
    // flushes persist their ratchet advances rowless, turning redelivery into
    // acked duplicates. The transition defers until a durable flush.
    #[tokio::test]
    async fn failed_tail_defers_live_transition_until_durable_flush() {
        let client = create_test_client_with_failing_http("batch_defer").await;
        // Full drain state: batching AND the single-permit semaphore (the
        // test client starts in live mode with 64 permits).
        client.inbound_commit_batch.reset();
        client.swap_message_semaphore(1);
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.commit_or_batch_inbound(item("T1"), false).await;

        client
            .inbound_commit_batch
            .fail_commits
            .store(true, Ordering::Release);
        let generation = client.connection_generation.load(Ordering::Acquire);
        let durable = client.finish_inbound_commit_drain(generation).await;
        assert!(!durable, "injected failure must report not-durable");
        assert!(
            client.inbound_commit_batch.is_active(),
            "failed tail must keep the batcher in drain mode"
        );
        assert!(
            client.inbound_commit_batch.has_entries(),
            "the guard must restore the taken entries"
        );

        // With the transition deferred, later traffic keeps batching instead
        // of committing rowless around the restored tail.
        client
            .inbound_commit_batch
            .fail_commits
            .store(false, Ordering::Release);
        client.commit_or_batch_inbound(item("T2"), false).await;
        assert!(
            hook.batches.lock().expect("hook lock").is_empty(),
            "deferred mode must accumulate, not commit live"
        );

        assert_eq!(
            available_permits(&client),
            1,
            "the deferred transition must keep stanzas serialized"
        );

        let durable = client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;
        assert!(durable);
        assert!(
            !client.inbound_commit_batch.is_active(),
            "the first durable flush completes the deferred transition"
        );
        assert!(!client.inbound_commit_batch.has_entries());
        assert_eq!(
            available_permits(&client),
            64,
            "completing the deferred transition widens the semaphore"
        );
        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(
            batches,
            vec![vec!["T1", "T2"]],
            "restored tail commits first, in arrival order"
        );
    }

    // With no further inbound traffic, the retry loop armed at defer time
    // must commit the restored tail and complete the transition on its own.
    #[tokio::test]
    async fn deferred_transition_retries_automatically() {
        let client = create_test_client_with_failing_http("batch_defer_retry").await;
        client.inbound_commit_batch.reset();
        client.swap_message_semaphore(1);
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.commit_or_batch_inbound(item("R1"), false).await;
        client
            .inbound_commit_batch
            .fail_commits
            .store(true, Ordering::Release);
        let generation = client.connection_generation.load(Ordering::Acquire);
        assert!(!client.finish_inbound_commit_drain(generation).await);
        client
            .inbound_commit_batch
            .fail_commits
            .store(false, Ordering::Release);

        for _ in 0..100 {
            if !client.inbound_commit_batch.is_active() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert!(
            !client.inbound_commit_batch.is_active(),
            "the armed retry must complete the transition without new traffic"
        );
        assert_eq!(available_permits(&client), 64);
        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches, vec![vec!["R1"]]);
    }

    // A Signal-flush failure after the durable row write must keep the batch
    // retryable this session: entries restored (re-storing rows is
    // idempotent), hook not yet run, and the retry commits everything.
    #[tokio::test]
    async fn flush_failure_after_rows_keeps_batch_retryable() {
        let client = create_test_client_with_failing_http("batch_flush_fail").await;
        client.inbound_commit_batch.reset();
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.commit_or_batch_inbound(item("F1"), false).await;
        client
            .inbound_commit_batch
            .fail_flushes
            .store(true, Ordering::Release);
        let durable = client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;
        assert!(!durable);
        assert!(
            client.inbound_commit_batch.has_entries(),
            "entries must be restored for a same-session retry"
        );
        assert!(
            hook.batches.lock().expect("hook lock").is_empty(),
            "hook must not run before the Signal flush succeeds"
        );
        let backend = client.persistence_manager.backend();
        assert!(
            backend
                .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "F1")
                .await
                .unwrap()
                .is_some(),
            "the durable row from the failed attempt stays in place"
        );

        client
            .inbound_commit_batch
            .fail_flushes
            .store(false, Ordering::Release);
        let durable = client
            .flush_inbound_commits_under_permit(false, None, None)
            .await;
        assert!(durable);
        assert!(!client.inbound_commit_batch.has_entries());
        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches, vec![vec!["F1"]]);
        assert!(
            backend
                .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "F1")
                .await
                .unwrap()
                .is_none(),
            "the retry commit clears the buffered row"
        );
    }

    // The batch-safe wrapper enters on an is_active() check that the drain
    // finisher can invalidate before it acquires the permit. When the
    // under-permit commit deactivates the batcher (here, by completing a
    // deferred transition), the wrapper must fall through to the raw Signal
    // flush instead of returning Ok on the stale check — otherwise the
    // caller's out-of-band advance is silently skipped. This drives that
    // completion through the wrapper and asserts it lands in live mode.
    #[tokio::test]
    async fn batch_safe_flush_completes_deferred_transition() {
        let client = create_test_client_with_failing_http("batch_safe_defer").await;
        client.inbound_commit_batch.reset();
        client.swap_message_semaphore(1);
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.commit_or_batch_inbound(item("B1"), false).await;
        client
            .inbound_commit_batch
            .fail_commits
            .store(true, Ordering::Release);
        let generation = client.connection_generation.load(Ordering::Acquire);
        assert!(!client.finish_inbound_commit_drain(generation).await);
        assert!(
            client.inbound_commit_batch.is_active() && client.inbound_commit_batch.has_entries(),
            "failed tail defers the transition and restores the batch"
        );
        client
            .inbound_commit_batch
            .fail_commits
            .store(false, Ordering::Release);

        // Enters on is_active()==true, commits the restored tail under the
        // permit, completes the deferred transition (deactivates), then falls
        // through to the raw flush.
        client
            .flush_signal_cache_batch_safe()
            .await
            .expect("batch-safe flush must succeed once the tail commits");

        assert!(
            !client.inbound_commit_batch.is_active(),
            "the batch-safe flush must complete the deferred transition"
        );
        assert!(!client.inbound_commit_batch.has_entries());
        assert_eq!(
            available_permits(&client),
            64,
            "completing the transition widens the semaphore"
        );
        assert_eq!(
            hook.batches.lock().expect("hook lock").clone(),
            vec![vec!["B1"]]
        );
    }

    // An empty commit must still flush the Signal cache under the permit —
    // this is what lets flush_signal_cache_batch_safe persist an out-of-band
    // advance when the drain finisher deactivated the batcher between its
    // is_active() check and the permit. Observable via the injected flush
    // failure: skipping the flush would wrongly report durable (the pre-fix
    // empty+live path returned true without flushing).
    #[tokio::test]
    async fn empty_commit_still_flushes_under_permit() {
        let client = create_test_client_with_failing_http("batch_empty_flush").await;
        // Live mode, empty batcher: was_draining is false under the permit.
        assert!(!client.inbound_commit_batch.is_active());
        assert!(!client.inbound_commit_batch.has_entries());

        client
            .inbound_commit_batch
            .fail_flushes
            .store(true, Ordering::Release);
        assert!(
            !client
                .flush_inbound_commits_under_permit(false, None, None)
                .await,
            "an empty commit must still hit the Signal cache and surface its failure"
        );

        client
            .inbound_commit_batch
            .fail_flushes
            .store(false, Ordering::Release);
        assert!(
            client
                .flush_inbound_commits_under_permit(false, None, None)
                .await,
            "with the flush succeeding the empty commit reports durable"
        );
    }

    // WA Web createSnapshot flushes aggregate offline receipts per snapshot;
    // a durable OfflineDrain commit must drain the buffer instead of holding it
    // until end-of-drain.
    #[tokio::test]
    async fn durable_drain_commit_flushes_offline_receipts_per_snapshot() {
        let client = create_test_client_with_failing_http("batch_offline_flush").await;
        client.inbound_commit_batch.reset(); // back into drain mode

        // Batcher active → the receipt buffers instead of going 1:1.
        let buffered = item("R1").info;
        assert!(client.try_buffer_offline_receipt(&buffered));
        assert_eq!(client.offline_receipt_buffer.lock().expect("buf").len(), 1);

        client.commit_or_batch_inbound(item("D1"), false).await;
        assert!(
            client
                .flush_inbound_commits_under_permit(false, None, None)
                .await,
            "the drain batch must commit durably"
        );
        assert!(
            client
                .offline_receipt_buffer
                .lock()
                .expect("buf")
                .is_empty(),
            "a durable drain commit flushes the buffered offline receipts per snapshot"
        );
    }

    fn available_permits(client: &Client) -> usize {
        let semaphore = match client.message_processing_semaphore.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        let mut guards = Vec::new();
        while let Some(guard) = semaphore.try_acquire() {
            guards.push(guard);
        }
        guards.len()
    }
    #[tokio::test]
    async fn raw_pending_near_full_retry_must_reach_recovered_hook() {
        use std::sync::atomic::AtomicUsize;
        struct RecoveringHook(AtomicUsize);
        #[async_trait::async_trait]
        impl InboundDurabilityHook for RecoveringHook {
            async fn on_messages(
                &self,
                _: Arc<Client>,
                _: &[InboundMessage],
            ) -> anyhow::Result<()> {
                anyhow::ensure!(
                    self.0.fetch_add(1, Ordering::SeqCst) != 0,
                    "first hook fails"
                );
                Ok(())
            }
        }
        let client = create_test_client_with_failing_http("near_full_retry_progress").await;
        client.enter_live_mode_for_tests();
        let hook = Arc::new(RecoveringHook(AtomicUsize::new(0)));
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let mut inbound = item("NEAR_FULL_RETRY");
        let mut message = wa::Message::default();
        message.conversation = Some("x".repeat(1800 * 1024));
        inbound.message = Arc::new(message);
        let make = || PendingDelivery {
            carrier: Some((
                Some(0),
                waproto::codec::message_to_vec(&inbound.message).into(),
            )),
            item: inbound.clone(),
            retention: None,
            receipt: None,
            storage_sender: None,
            consumer_claimed: false,
            origin: Some(raw_pending::DeliveryOrigin {
                stanza: raw_pending::StanzaOrigin::new(),
                position: 0,
            }),
        };
        client.commit_or_batch_pending(make(), false).await;
        assert_eq!(
            hook.0.load(Ordering::SeqCst),
            1,
            "first admitted row reaches failing hook"
        );
        // Production recovers the predecessor before the next decrypt.
        client
            .persist_live_retained_before_decrypt(&inbound.info)
            .await
            .unwrap();
        client.commit_or_batch_pending(make(), false).await;
        assert_eq!(
            hook.0.load(Ordering::SeqCst),
            3,
            "recovery delivers the older row and then the new occurrence after the first failed hook"
        );
    }
}
