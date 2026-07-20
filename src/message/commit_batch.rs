//! Inbound commit batcher: accumulates decrypted messages during the offline
//! drain and commits them per batch — pending-inbound buffer, Signal-cache
//! flush, durability hook, event dispatch and acks all amortize over the
//! batch. Mirrors WA Web's `MessageProcessorCache` (`createSnapshot`: bulk
//! message-table write → bulk signal-store commit → aggregate receipts), with
//! the same flush triggers: batch size, timeout, and end-of-drain.
//!
//! Live messages bypass accumulation and commit as a batch of one, which is
//! also WA Web behavior (the same pipeline with an immediate flush).

use super::*;
use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use wacore::store::traits::{PendingInboundKey, PendingInboundRow};
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
    entries: Vec<InboundMessage>,
    /// A dropped request must not outlive the reconnect that discarded it.
    commit_ticket: Option<InboundCommitTicket>,
    /// Sum of `message_encoded_len` of the entries, for the byte cap.
    bytes: usize,
    timer_armed: bool,
}

struct PendingInboundBatch {
    entries: Vec<InboundMessage>,
    commit_ticket: Option<InboundCommitTicket>,
}

impl PendingInboundBatch {
    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    fn mark_dropped(self) -> Vec<InboundMessage> {
        if let Some(ticket) = self.commit_ticket {
            ticket.mark_dropped();
        }
        self.entries
    }
}

pub(crate) struct InboundCommitBatcher {
    state: std::sync::Mutex<BatchState>,
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
    /// concurrent live-path hook calls that could previously overlap.
    arena: async_lock::Mutex<Vec<u8>>,
}

impl Default for InboundCommitBatcher {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(BatchState::default()),
            active: std::sync::atomic::AtomicBool::new(true),
            pending_live: std::sync::atomic::AtomicBool::new(false),
            epoch: AtomicU64::new(0),
            arena: async_lock::Mutex::new(Vec::new()),
            #[cfg(test)]
            fail_commits: std::sync::atomic::AtomicBool::new(false),
            #[cfg(test)]
            fail_flushes: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

impl InboundCommitBatcher {
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

    /// Switch to immediate (live) commits. Only the end-of-drain flush (or a
    /// later flush completing a deferred transition) calls this, while
    /// holding the processing permit.
    fn deactivate(&self) {
        self.pending_live.store(false, Ordering::Release);
        self.active.store(false, Ordering::Release);
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

#[cfg(test)]
impl Client {
    /// Test-only mirror of a completed offline sync: the flag, live permit
    /// count and batcher mode move together so tests can never run in a
    /// hybrid drain/live state production cannot reach. Kept next to the
    /// production transition (`finish_offline_sync`) so a new step gets added
    /// to both.
    pub(crate) fn enter_live_mode_for_tests(&self) {
        self.offline_sync_completed.store(true, Ordering::Release);
        self.offline_sync_finish_started
            .store(true, Ordering::Release);
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
    items: Option<std::sync::Arc<[InboundMessage]>>,
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
        let mut restored: Vec<InboundMessage> = items.iter().cloned().collect();
        // Nothing newer can normally exist (the permit serializes drain
        // flushes), but keep arrival order if it ever does.
        restored.append(&mut state.entries);
        state.bytes = restored
            .iter()
            .map(|i| waproto::codec::message_encoded_len(&i.message))
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

impl Client {
    fn enqueue_inbound_commit(
        &self,
        item: InboundMessage,
        track_commit: bool,
    ) -> (Option<u64>, Option<InboundCommitTicket>) {
        let batcher = &self.inbound_commit_batch;
        let mut state = batcher.lock();
        state.bytes += waproto::codec::message_encoded_len(&item.message);
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

    /// Batch a message decrypted while the offline drain is active, or commit
    /// immediately (batch of one) on the live path.
    pub(crate) async fn commit_or_batch_inbound(
        self: &Arc<Self>,
        item: InboundMessage,
        track_commit: bool,
    ) -> InboundCommitState {
        if !self.inbound_commit_batch.is_active() {
            // Arc::from([item]) builds the event/hook slice in one allocation;
            // a Vec would add an alloc+dealloc per live message (measured
            // ~18ns and 2x the allocations of this step).
            return if self
                .commit_inbound_batch(std::sync::Arc::from([item]), BatchOrigin::Live, None)
                .await
            {
                InboundCommitState::Durable
            } else {
                InboundCommitState::Failed
            };
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
                .commit_inbound_batch(entries.into(), BatchOrigin::OfflineDrain, commit_ticket)
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
            self.commit_inbound_batch(
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
        self.inbound_commit_batch.deactivate();
        self.swap_message_semaphore(64);
        // Receipts buffered during the deferred window (SKDM-only stanzas
        // keep buffering while the batcher is active) are safe to send now:
        // the durable flush that triggered this completion persisted their
        // Signal state.
        self.flush_offline_receipts();
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
    ) {
        let settle = async {
            let _permit = self.acquire_message_processing_permit().await;
            let batch = self.inbound_commit_batch.take();
            let durable = if batch.is_empty() {
                true
            } else {
                self.commit_inbound_batch(
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
                    Ok(()) => self.signal_cache.clear_after_flush().await,
                    // Committed/acked state the server never redelivers: keep
                    // it resident so the next successful flush persists it.
                    // Safe to carry across the reconnect — the teardown
                    // generation bump plus the post-permit re-check mean no
                    // late decrypt can mix rowless advances into it.
                    Err(e) => log::error!(
                        "cleanup_connection_state: signal cache flush failed, keeping cache to avoid dropping Signal state: {e:?}"
                    ),
                }
            } else {
                log::warn!(
                    "cleanup_connection_state: dropping unflushed Signal state for uncommitted drain entries; the server redelivers them"
                );
                self.signal_cache.clear().await;
            }
        };
        if wacore::runtime::timeout(&*self.runtime, limit, settle)
            .await
            .is_err()
        {
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
    ) -> bool {
        match wacore::runtime::timeout(
            &*self.runtime,
            limit,
            self.flush_inbound_commits_under_permit(false, None, None),
        )
        .await
        {
            Ok(durable) => durable,
            Err(_) => {
                log::warn!(
                    "Timed out committing the inbound drain batch during teardown; leaving entries for redelivery"
                );
                false
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
        self.commit_inbound_batch(
            batch.entries.into(),
            BatchOrigin::OfflineDrain,
            batch.commit_ticket,
        )
        .await
    }

    /// Commit one batch: durable buffer → Signal flush → hook → clear buffer →
    /// acks → event. Nothing is acked or observable before it is durable (WA
    /// Web's `createSnapshot` ordering); acks precede the event dispatch so a
    /// misbehaving synchronous handler cannot suppress them — the contract the
    /// pre-batch at-most-once path had. A crash between ack and event trades
    /// exactly like that old path: the consumer's durable copy is the hook
    /// commit, not the event. On any commit failure everything stays unacked
    /// and the server redelivers the whole batch.
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
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.commit_batch", level = "debug", skip_all, fields(count = items.len())))]
    pub(crate) async fn commit_inbound_batch(
        self: &Arc<Self>,
        items: std::sync::Arc<[InboundMessage]>,
        origin: BatchOrigin,
        commit_ticket: Option<InboundCommitTicket>,
    ) -> bool {
        let is_drain = matches!(origin, BatchOrigin::OfflineDrain);
        debug_assert!(is_drain || commit_ticket.is_none());
        if items.is_empty() {
            debug_assert!(commit_ticket.is_none());
            return true;
        }
        let mut reinsert = ReinsertGuard {
            batcher: &self.inbound_commit_batch,
            items: is_drain.then(|| std::sync::Arc::clone(&items)),
            commit_ticket,
        };
        #[cfg(test)]
        if self
            .inbound_commit_batch
            .fail_commits
            .load(Ordering::Acquire)
        {
            return false;
        }

        if let Some(hook) = self.inbound_durability_hook() {
            // Key strings live for the whole commit; rows borrow them and the
            // encode arena, so the batch write allocates nothing per row
            // beyond these.
            let keys: Vec<(String, String)> = items
                .iter()
                .map(|m| {
                    (
                        m.info.source.chat.to_string(),
                        m.info.source.sender.to_string(),
                    )
                })
                .collect();

            let backend = self.persistence_manager.backend();
            // Encode scope: the buffer lives only through the durable write,
            // never across the slower flush/hook steps below. Drain reuses the
            // shared arena (uncontended: the permit serializes drain flushes);
            // live (batch of one) uses a local buffer so concurrent live
            // commits never queue on a shared lock while a slow hook runs.
            {
                let mut local_arena;
                let mut shared_arena;
                let arena: &mut Vec<u8> = if is_drain {
                    shared_arena = self.inbound_commit_batch.arena.lock().await;
                    &mut shared_arena
                } else {
                    // Exact-size reservation: geometric growth would realloc
                    // several times per live message.
                    local_arena =
                        Vec::with_capacity(waproto::codec::message_encoded_len(&items[0].message));
                    &mut local_arena
                };
                arena.clear();
                let mut ranges = Vec::with_capacity(items.len());
                for item in items.iter() {
                    let start = arena.len();
                    waproto::codec::message_encode_into(&item.message, arena);
                    ranges.push(start..arena.len());
                }
                let rows: Vec<PendingInboundRow<'_>> = items
                    .iter()
                    .zip(&keys)
                    .zip(&ranges)
                    .map(|((item, (chat, sender)), range)| PendingInboundRow {
                        chat,
                        sender,
                        id: &item.info.id,
                        message: &arena[range.clone()],
                    })
                    .collect();

                // Fail closed: without a durable buffered copy, do not run the
                // hook and do not ack — the entries return to the batcher (via
                // the guard) and the server redelivers once storage recovers.
                if let Err(e) = backend.store_pending_inbound_batch(&rows).await {
                    log::error!(
                        "Failed to buffer inbound batch of {}; suppressing acks for redelivery: {e:?}",
                        items.len()
                    );
                    return false;
                }
            }
            // A failed flush reports not-durable so buffered receipts are held
            // back. The guard is still armed: the rows are in place and
            // re-storing them is idempotent (replace-into), so the restored
            // entries make the batch retryable THIS session — the retry
            // re-runs the full commit (rows → flush → hook → acks → event)
            // instead of parking the messages until a reconnect replay.
            if is_drain && !self.drain_signal_flush_reporting().await {
                return false;
            }
            // Rows durable and Signal flushed: from here on a cancelled
            // future must not restore the entries — redelivery replays from
            // the rows.
            reinsert.mark_durable();

            if let Err(e) = hook.on_messages(self.clone(), &items).await {
                log::warn!(
                    "Inbound durability hook failed for batch of {}; suppressing acks for redelivery: {e:?}",
                    items.len()
                );
                return true;
            }

            let delete_keys: Vec<PendingInboundKey<'_>> = items
                .iter()
                .zip(&keys)
                .map(|(item, (chat, sender))| PendingInboundKey {
                    chat,
                    sender,
                    id: &item.info.id,
                })
                .collect();
            if let Err(e) = backend.delete_pending_inbound_batch(&delete_keys).await {
                // Leftover rows replay as duplicates; the idempotent hook
                // re-commits and the replay path clears them.
                log::debug!(
                    "Failed to clear {} buffered inbound messages: {e:?}",
                    delete_keys.len()
                );
            }
        } else {
            // No hook = at-most-once: the flush is the durable point, so a
            // failure restores the entries (guard still armed) for a retry.
            if is_drain && !self.drain_signal_flush_reporting().await {
                return false;
            }
            reinsert.mark_durable();
        }

        // Acks first (everything durable by now): handle_event runs
        // synchronously, so a handler that panics or blocks must not be able
        // to suppress acks for messages the consumer already owns — the
        // pre-batch at-most-once path acked before dispatching too.
        for item in items.iter() {
            self.ack_received_message(&item.info);
        }
        // WA Web `createSnapshot` sends `sendAggregateOfflineReceipts` per
        // snapshot, not once at drain end. Flush the receipts this batch's acks
        // just buffered now that the batch is durable: bounds the buffer over a
        // large backlog and caps redelivery on a mid-drain disconnect to a
        // single snapshot instead of the whole backlog (a disconnect clears the
        // buffer, so anything already flushed is not re-sent). Gated on the
        // durable path — a non-durable batch returned above without acking.
        if is_drain {
            self.flush_offline_receipts();
        }
        self.core.event_bus.dispatch(Event::Messages(
            MessageBatch::builder()
                .messages(items)
                .origin(origin)
                .build(),
        ));
        true
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
                .push(batch.iter().map(|m| m.info.id.clone()).collect());
            Ok(())
        }
    }

    fn item(id: &str) -> InboundMessage {
        InboundMessage::builder()
            .message(Arc::new(wa::Message {
                conversation: Some(format!("text {id}")),
                ..Default::default()
            }))
            .info(Arc::new(MessageInfo {
                id: id.to_string(),
                source: MessageSource {
                    chat: "100@g.us".parse().unwrap(),
                    sender: "200@s.whatsapp.net".parse().unwrap(),
                    ..Default::default()
                },
                ..Default::default()
            }))
            .build()
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
        client.core.event_bus.add_handler(handler);

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

    // Live traffic commits immediately as a batch of one.
    #[tokio::test]
    async fn live_commits_as_batch_of_one() {
        let client = create_test_client_with_failing_http("batch_live").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);
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
        client.core.event_bus.add_handler(handler);

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
        client.core.event_bus.add_handler(handler);

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
        client.core.event_bus.add_handler(handler);

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
}
