//! Pull-based offline backlog request loop.
//!
//! WA Web (`WAWebOfflineHandler.processOfflinePreviewIb` in
//! `docs/captured-js/WAWeb/Offline/Handler.js`) gates offline delivery on the
//! client: the server announces a backlog via `<ib><offline_preview count="N"/>`,
//! delivers a primer (~5 stanzas), and then waits for
//! `<ib><offline_batch count="200"/>` requests before sending more. Without
//! those requests the backlog is silently dropped after the primer.
//!
//! Mirrors the non-adaptive path (`$13`, default when
//! `isOfflineDynamicBatchSizeEnabled` AB flag is off). The state machine
//! tracks WA Web's `$4` (prev-batch-in-flight) flag: a CAS-gated
//! "first arrival of the just-sent batch" deterministically elects a single
//! continuation per cycle, just as WA Web's `$6` timer coalesces
//! synchronously-arriving stanzas into one `$13` call.
//!
//! Additionally paced by processing, which WA Web leaves to the lane: one
//! cumulative `outstanding` per connection counts the stanzas requested and not
//! yet finished, and a continuation is only reserved while it fits one batch.
//! A request is booked when it is decided — never on arrival — so a slow
//! delivery cannot make the count under-represent what the server still owes,
//! and two batches is the most one chat lane can be handed.

use portable_atomic::AtomicU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use futures::FutureExt;
use log::{debug, warn};
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;

use crate::client::Client;

/// Mirrors `WAWebOfflineHandler.v = 200`.
const BATCH_SIZE: u32 = 200;

// Two pulled batches must fit one chat's lane; see `CHAT_QUEUE_ITEMS`.
const _: () = assert!(crate::inbound_queue::CHAT_QUEUE_ITEMS >= 2 * BATCH_SIZE as usize);

/// Mirrors `WAWebOfflineHandler.S = 100`.
const REQUEST_DEBOUNCE: Duration = Duration::from_millis(100);

/// Mirrors `WAWebOfflineResumeConst.OFFLINE_STANZA_TIMEOUT_MS = 60000`: the
/// blocking resume manager arms a `ShiftTimer` on the first preview, shifts it
/// on every offline stanza, and on expiry completes the session itself.
const STANZA_INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60);

/// Parked in `stanza_activity` by the watchdog that completes a stalled drain,
/// so a stanza racing that decision loses the CAS instead of being ignored.
const INACTIVE_CLAIMED: u64 = u64::MAX;

#[derive(Default)]
pub(crate) struct OfflineBatchCoordinator {
    armed: AtomicBool,
    generation: AtomicU64,
    /// WA Web `$4`: true between sending a batch and observing the first
    /// stanza of that batch's response. CAS-gated so concurrent arrivals
    /// deterministically elect a single "first arrival" winner per cycle.
    prev_batch_inflight: AtomicBool,
    /// Bumped by every offline stanza. Stands in for WA Web's `ShiftTimer`
    /// re-arm (`$13`): the watchdog compares the counter across a sleep
    /// instead of reading a clock per stanza, which costs one relaxed
    /// increment on the receive path and in exchange lets the timeout fire
    /// anywhere in [timeout, 2 * timeout) after the last stanza.
    stanza_activity: AtomicU64,
    /// Pacing owned by the armed generation. `arm` swaps a fresh object in and
    /// `reset` takes it out, so a guard or arrival retired by a reconnect can
    /// only ever reach the object it was minted with.
    pacing: std::sync::Mutex<Option<Arc<Pacing>>>,
}

impl OfflineBatchCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        // Disarm first: readers stop minting guards before the object they
        // would count on is gone. Guards already minted keep their own `Arc`
        // and settle the retired object, where nobody reads them any more.
        self.armed.store(false, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.prev_batch_inflight.store(false, Ordering::Release);
        *self.pacing.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    /// Disarm and report whether this call is the one that did it. The
    /// teardown paths use it as a once-guard for the interrupted-resume
    /// event, so a second teardown (or a `connect()` that follows one) stays
    /// silent.
    pub(crate) fn take_armed(&self) -> bool {
        self.armed.swap(false, Ordering::AcqRel)
    }

    #[cfg(test)]
    pub(crate) fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Acquire)
    }

    pub(crate) fn stanza_activity(&self) -> u64 {
        self.stanza_activity.load(Ordering::Acquire)
    }

    pub(crate) fn note_stanza_activity(&self) {
        self.stanza_activity.fetch_add(1, Ordering::AcqRel);
    }

    /// Claim a drain as inactive, but only if no stanza has arrived since
    /// `seen`. A bump landing between the watchdog's read and its decision
    /// fails the CAS, so a drain that is still being fed is never completed
    /// behind the server's back. The sentinel it parks on is never a value
    /// `note_stanza_activity` can reach from a fresh `arm()`.
    fn try_claim_inactive(&self, seen: u64) -> bool {
        self.stanza_activity
            .compare_exchange(seen, INACTIVE_CLAIMED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Arm the resume for `generation` and hand back the channel its parked
    /// requests are released on, for the one task that sends them.
    fn arm(&self, generation: u64, total: usize) -> async_channel::Receiver<()> {
        let (pacing, released) = Pacing::new(generation, total);
        let pacing = Arc::new(pacing);
        // The first request is booked with the arm: the primer that follows is
        // part of the batch it announces.
        pacing.book_first();
        // Set inner state first; `armed` is published last so readers that
        // observe armed=true also see the matching generation, pacing and
        // inflight.
        *self.pacing.lock().unwrap_or_else(|p| p.into_inner()) = Some(pacing);
        self.generation.store(generation, Ordering::Release);
        self.stanza_activity.store(0, Ordering::Release);
        self.prev_batch_inflight.store(true, Ordering::Release);
        self.armed.store(true, Ordering::Release);
        released
    }

    fn is_armed_for(&self, generation: u64) -> bool {
        self.armed.load(Ordering::Acquire) && self.generation.load(Ordering::Acquire) == generation
    }

    /// The pacing of the connection this caller belongs to, or `None` when that
    /// connection is not the armed one. Handing out the `Arc` is what makes a
    /// stale caller harmless: it can only reach the generation it was minted
    /// for.
    fn current_pacing(&self, generation: u64) -> Option<Arc<Pacing>> {
        if !self.is_armed_for(generation) {
            return None;
        }
        self.pacing
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_ref()
            .filter(|pacing| pacing.generation == generation)
            .cloned()
    }

    /// What a whole window without an offline stanza means, for the watchdog.
    pub(crate) fn settle_silent_drain(&self, generation: u64) -> SilentDrain {
        match self.current_pacing(generation) {
            Some(pacing) => pacing.settle_silent(),
            None => SilentDrain::Idle,
        }
    }

    /// Try to claim the "first arrival of just-sent batch" slot. Atomically
    /// transitions prev_batch_inflight true → false. Only one concurrent
    /// caller per batch cycle succeeds.
    fn try_claim_first_arrival(&self) -> bool {
        self.prev_batch_inflight
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Republish "batch in flight" so the next stanza arrival can win the
    /// `try_claim_first_arrival` CAS.
    fn mark_batch_inflight(&self) {
        self.prev_batch_inflight.store(true, Ordering::Release);
    }

    /// Count one offline stanza the message handler is about to queue in a
    /// chat lane and return the guard that finishes it. `None` for anything the
    /// resume does not pace: an unarmed connection, or a live stanza.
    ///
    /// Every stanza the message handler receives is bound for a lane (messages,
    /// status broadcasts and durable group notifications alike), so the tag is
    /// not checked here; the receive path decides which stanzas come here.
    ///
    /// Boxed because it travels in the queued message, where a word is what
    /// the queue's layout budget has room for.
    pub(crate) fn admit(
        &self,
        generation: u64,
        node: &wacore_binary::OwnedNodeRef,
    ) -> Option<Box<OfflinePendingGuard>> {
        node.get().get_attr("offline")?;
        let pacing = self.current_pacing(generation)?;
        pacing.admit_lane_bound();
        Some(Box::new(OfflinePendingGuard { pacing }))
    }

    #[cfg(test)]
    fn test_pacing(&self) -> Option<Arc<Pacing>> {
        let generation = self.generation.load(Ordering::Acquire);
        self.current_pacing(generation)
    }

    #[cfg(test)]
    pub(crate) fn outstanding(&self) -> usize {
        self.test_pacing()
            .map_or(0, |pacing| pacing.state().outstanding)
    }

    #[cfg(test)]
    pub(crate) fn admitted(&self) -> usize {
        self.test_pacing()
            .map_or(0, |pacing| pacing.state().admitted)
    }

    #[cfg(test)]
    pub(crate) fn deferred(&self) -> bool {
        self.test_pacing()
            .is_some_and(|pacing| pacing.state().deferred)
    }
}

/// Backlog pacing for one connection generation (S2-S4).
///
/// One object per generation, one short `Mutex` and never held across an
/// `.await`: every decision that must not interleave with another — booking a
/// request, settling a finish, parking a continuation, releasing one, deciding
/// what a silent drain means — is a single transition. That is what makes the
/// cumulative bound exact and the deferred wakeup impossible to lose, in place
/// of separate counters that could be read and written a window apart.
pub(crate) struct Pacing {
    generation: u64,
    state: std::sync::Mutex<PacingState>,
    /// One message per released request, for the generation's sender task.
    /// A channel rather than a client handle: whoever settles a finish (a lane
    /// worker, a queue being torn down) only signals, and a queued message
    /// carries nothing that ties it to the client, which a lane must not do
    /// where the client is not `Send`.
    released: async_channel::Sender<()>,
}

#[derive(Default)]
struct PacingState {
    /// Stanzas this connection asked the server for and has not finished: a
    /// lane-bound message until its guard drops, any other offline stanza as
    /// soon as it is handled. Booked when the request is decided, so nothing
    /// can arrive that the count does not already own.
    outstanding: usize,
    /// Preview stanzas no request has booked yet. Booking moves one batch from
    /// here to `outstanding`, so no request asks for more than was announced.
    unbooked: usize,
    /// Lane-bound offline messages still unfinished: our own backpressure, as
    /// opposed to the server's silence.
    admitted: usize,
    /// A continuation the gate had to park, owed to the finish that brings
    /// `outstanding` back to one batch.
    deferred: bool,
}

/// One continuation decision, taken as a single reservation.
enum Reservation {
    /// Booked; the caller sends the request.
    Request,
    /// Parked until processing catches up.
    Deferred,
    /// The preview has nothing left to ask for.
    Exhausted,
}

/// What a whole window of silence means for the drain.
pub(crate) enum SilentDrain {
    /// Our own pacing is holding the drain open: wait instead of completing.
    Backpressure,
    /// Nothing is in a lane: the server stopped, and the resume completes.
    Idle,
}

impl Pacing {
    fn new(generation: u64, total: usize) -> (Self, async_channel::Receiver<()>) {
        // Unbounded: every release books its own request, so two releases
        // must never coalesce into one send.
        let (released, receiver) = async_channel::unbounded();
        let pacing = Self {
            generation,
            state: std::sync::Mutex::new(PacingState {
                unbooked: total,
                ..PacingState::default()
            }),
            released,
        };
        (pacing, receiver)
    }

    /// Hand a request a finish released to the sender task. A closed channel
    /// means the generation's task is gone, and so is the request's purpose.
    fn notify(&self, released: bool) -> bool {
        if released {
            let _ = self.released.try_send(());
        }
        released
    }

    /// The one lock every transition takes. A poisoned mutex still holds a
    /// consistent state, so recovery is the right answer here.
    fn state(&self) -> std::sync::MutexGuard<'_, PacingState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The first request of a drain, booked with the arm.
    fn book_first(&self) {
        self.state().book();
    }

    /// Decide one continuation: check the gate and book the request as one
    /// transition, so a request is never sent unreserved.
    fn reserve(&self) -> Reservation {
        let mut state = self.state();
        if state.unbooked == 0 {
            return Reservation::Exhausted;
        }
        if state.outstanding > BATCH_SIZE as usize {
            state.deferred = true;
            return Reservation::Deferred;
        }
        state.book();
        Reservation::Request
    }

    fn admit_lane_bound(&self) {
        // `outstanding` is untouched: the stanza was booked when the request
        // that delivered it was decided, and admission only moves its finish
        // from arrival to the guard on its queue entry.
        self.state().admitted += 1;
    }

    /// One lane-bound message is done with — processed, cancelled, refused, or
    /// dropped with its queue. `true` means this finish resolved a parked
    /// request, which the caller must send.
    fn finish_admitted(&self) -> bool {
        let released = {
            let mut state = self.state();
            state.settle_one();
            state.admitted = state.admitted.saturating_sub(1);
            state.release_deferred()
        };
        self.notify(released)
    }

    /// One offline stanza that no lane holds is done with.
    fn finish_arrival(&self) -> bool {
        let released = {
            let mut state = self.state();
            state.settle_one();
            state.release_deferred()
        };
        self.notify(released)
    }

    /// Decide what a window without a single offline stanza means.
    ///
    /// Silence never settles what the server still owes: a late response is
    /// not an absent one, and booking another batch on top of it is exactly
    /// how a lane would be handed a third. With nothing in a lane, the resume
    /// completes as it did before pacing, and no further request leaves.
    fn settle_silent(&self) -> SilentDrain {
        if self.state().admitted > 0 {
            SilentDrain::Backpressure
        } else {
            SilentDrain::Idle
        }
    }
}

impl PacingState {
    fn settle_one(&mut self) {
        self.outstanding = self.outstanding.saturating_sub(1);
    }

    /// Take one request's worth out of the preview — never more than is left.
    fn book(&mut self) {
        let booking = (BATCH_SIZE as usize).min(self.unbooked);
        self.unbooked -= booking;
        self.outstanding += booking;
    }

    /// Release a parked continuation if this finish brought the lane back
    /// within one batch. Decided under the same lock that parked it, so the
    /// finish that opens the gate can never miss it.
    fn release_deferred(&mut self) -> bool {
        if !self.deferred || self.outstanding > BATCH_SIZE as usize {
            return false;
        }
        self.deferred = false;
        if self.unbooked == 0 {
            // Everything announced is booked; the end marker ends the drain.
            return false;
        }
        self.book();
        true
    }
}

/// One queued offline message's claim on its generation's pacing (S3).
///
/// Counted when the handler decides to queue it and settled when the queued
/// item is dropped, so finishing, cancelling, refusing and tearing the queue
/// down all settle it by the same path: nothing a lane was handed can keep the
/// resume waiting after the lane let go of it.
pub(crate) struct OfflinePendingGuard {
    /// The generation's pacing object, not a counter: a guard retired by a
    /// reconnect can only ever settle what it was minted with.
    pacing: Arc<Pacing>,
}

impl Drop for OfflinePendingGuard {
    fn drop(&mut self) {
        // A release is sent by the generation's own task: a lane worker
        // finishing a message never waits on the socket for it.
        self.pacing.finish_admitted();
    }
}

/// Mirrors `WASmaxOutOfflineBatchRequest.makeBatchRequest`.
pub(crate) fn build_offline_batch_request(count: u32) -> Node {
    NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline_batch")
            .attr("count", count.to_string())
            .build()])
        .build()
}

/// Called from `IbHandler` on `<ib><offline_preview count="N"/>` with N > 0.
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.offline_first_batch", level = "debug", skip_all, fields(total = total)))]
pub(crate) async fn send_first_batch(client: Arc<Client>, total: usize) {
    let generation = client.connection_generation.load(Ordering::Acquire);
    let released = client.offline_batch.arm(generation, total);
    spawn_release_sender(Arc::clone(&client), generation, released);
    log::info!(
        target: "Client/OfflineResume",
        "Resume armed: total={}, requesting first batch of {}",
        total,
        BATCH_SIZE,
    );
    spawn_inactivity_watchdog(Arc::clone(&client), generation, STANZA_INACTIVITY_TIMEOUT);
    // arm() already published prev_batch_inflight=true. Do NOT republish after
    // the await: an arrival (primer) processed concurrently with this send may
    // have legitimately won the CAS already, and republishing would let a
    // second arrival win again and schedule a duplicate continuation.
    let _ = send_batch(&client, BATCH_SIZE).await;
}

/// WA Web's `ShiftTimer` (`WAWebBlockingOfflineResumeManager.$12`/`$13`): a
/// drain the server stops feeding is completed by the client rather than left
/// open, so consumers of the completion signal are never stranded on a stall.
/// Bounded by the connection generation, since a drain that ends with the
/// connection is reported as interrupted instead.
pub(crate) fn spawn_inactivity_watchdog(client: Arc<Client>, generation: u64, timeout: Duration) {
    let runtime = client.runtime.clone();
    // Subscribed at the spawn, for the connection whose generation this
    // watchdog was armed with — subscribing inside the task would pick up
    // whatever notifier a teardown had already installed.
    let shutdown = client.connection_shutdown_signal();
    runtime
        .spawn(Box::pin(async move {
            let mut seen = client.offline_batch.stanza_activity();
            loop {
                // Raced rather than merely re-checked after waking: the window
                // is a minute long, so on a teardown this task otherwise stays
                // scheduled — holding the whole client graph alive through its
                // `Arc` — for up to that long after the connection it belongs
                // to is gone, once per connection. The checks below are
                // unchanged and still decide everything; this only stops the
                // waiting early.
                futures::select! {
                    _ = client.runtime.sleep(timeout).fuse() => {}
                    _ = wacore::runtime::wait_for_shutdown(&shutdown).fuse() => return,
                }
                if !client.offline_batch.is_armed_for(generation)
                    || client.connection_generation.load(Ordering::Acquire) != generation
                    || client.offline_sync_completed.load(Ordering::Acquire)
                {
                    return;
                }
                let current = client.offline_batch.stanza_activity();
                if current != seen {
                    seen = current;
                    continue;
                }
                // Silence alone does not mean the server gave up: the drain may
                // be waiting on a lane we are still feeding, or on a request we
                // parked ourselves. Completing that would end a resume that is
                // only being paced, and the deferred request would never leave.
                // Decided before the claim, so a drain that stays open is never
                // left parked on the inactive sentinel.
                if matches!(
                    client.offline_batch.settle_silent_drain(generation),
                    SilentDrain::Backpressure
                ) {
                    continue;
                }
                if !client.offline_batch.try_claim_inactive(seen) {
                    // A stanza arrived while this was deciding; the drain is
                    // alive after all, so give it another window.
                    seen = client.offline_batch.stanza_activity();
                    continue;
                }
                let processed = client
                    .offline_sync_metrics
                    .processed_messages
                    .load(Ordering::Acquire);
                log::warn!(
                    target: "Client/OfflineResume",
                    "No offline stanza for {:?}; completing the resume by timeout at {} item(s)",
                    timeout,
                    processed,
                );
                client
                    .complete_offline_sync_for_generation(
                        i32::try_from(processed).unwrap_or(i32::MAX),
                        generation,
                    )
                    .await;
                return;
            }
        }))
        .detach();
}

/// Called from `process_node` after the per-stanza `processed_messages` bump.
/// Drives the pull loop: the first stanza of every just-sent batch elects
/// itself via CAS and schedules the next batch (debounced 100ms).
pub(crate) fn on_offline_stanza_arrived(client: &Arc<Client>, pending: usize, lane_bound: bool) {
    let coord = &client.offline_batch;
    let generation = client.connection_generation.load(Ordering::Acquire);
    let Some(pacing) = coord.current_pacing(generation) else {
        return;
    };
    coord.note_stanza_activity();
    // A stanza no lane will hold is finished as it is handled; a lane-bound one
    // is finished by the guard on its queue entry, so its arrival moves nothing.
    if !lane_bound {
        pacing.finish_arrival();
    }
    if pending == 0 {
        // Server has nothing more for us; let the end marker drive completion.
        return;
    }
    if !coord.try_claim_first_arrival() {
        return;
    }
    schedule_continuation(Arc::clone(client), generation);
}

async fn send_batch(client: &Client, count: u32) -> Result<(), ()> {
    let node = build_offline_batch_request(count);
    if let Err(e) = client.send_node(node).await {
        warn!(
            target: "Client/OfflineResume",
            "Failed to send <ib><offline_batch count={}>: {:?}",
            count, e,
        );
        return Err(());
    }
    Ok(())
}

fn schedule_continuation(client: Arc<Client>, generation: u64) {
    let runtime = client.runtime.clone();
    runtime
        .spawn(Box::pin(async move {
            client.runtime.sleep(REQUEST_DEBOUNCE).await;
            if !continuation_allowed(&client, generation) {
                return;
            }
            let Some(pacing) = client.offline_batch.current_pacing(generation) else {
                return;
            };
            match pacing.reserve() {
                Reservation::Request => {
                    debug!(
                        target: "Client/OfflineResume",
                        "Continuation batch (count={})",
                        BATCH_SIZE,
                    );
                    send_continuation(&client, generation).await;
                }
                // Parked under the lock that settles finishes, so the finish
                // that opens the gate always sees it. Nothing here waits.
                Reservation::Deferred => debug!(
                    target: "Client/OfflineResume",
                    "Continuation batch (count={}) deferred until the lane catches up",
                    BATCH_SIZE,
                ),
                // Everything the preview announced is already booked; only the
                // end marker (or the watchdog) has anything left to do.
                Reservation::Exhausted => {}
            }
        }))
        .detach();
}

/// Finish an offline stanza that was counted as lane-bound on arrival but never
/// reached the message handler (a stanza interceptor claimed it), so nothing
/// else will ever settle it.
pub(crate) fn finish_unqueued(client: &Client, generation: u64) {
    if let Some(pacing) = client.offline_batch.current_pacing(generation) {
        pacing.finish_arrival();
    }
}

/// What every continuation send starts with: the resume must still be this
/// connection's, and that connection must still be draining.
fn continuation_allowed(client: &Client, generation: u64) -> bool {
    client.offline_batch.is_armed_for(generation)
        && client.connection_generation.load(Ordering::Acquire) == generation
        && !client.offline_sync_completed.load(Ordering::Acquire)
}

async fn send_continuation(client: &Client, generation: u64) {
    let _ = send_batch(client, BATCH_SIZE).await;
    // A request that just left starts a fresh inactivity window: without this
    // a release landing just before a tick lets the watchdog complete the
    // resume before the batch it asked for can be answered.
    client.offline_batch.note_stanza_activity();
    // Re-check generation after the await: a disconnect+reconnect+rearm
    // during the send would leave the coordinator owned by a newer
    // cycle, and republishing here would overwrite that cycle's CAS
    // state and let a duplicate continuation slip through.
    if client.offline_batch.is_armed_for(generation) {
        client.offline_batch.mark_batch_inflight();
    }
}

/// The generation's one sender of parked requests: each message on `released`
/// is a request a finish already booked when it opened the gate. Ends with the
/// connection, or once the generation's pacing is gone and nothing can release
/// another.
fn spawn_release_sender(
    client: Arc<Client>,
    generation: u64,
    released: async_channel::Receiver<()>,
) {
    let runtime = client.runtime.clone();
    // Subscribed at the spawn, as the watchdog does, for this connection.
    let shutdown = client.connection_shutdown_signal();
    runtime
        .spawn(Box::pin(async move {
            loop {
                futures::select! {
                    message = released.recv().fuse() => if message.is_err() {
                        return;
                    },
                    _ = wacore::runtime::wait_for_shutdown(&shutdown).fuse() => return,
                }
                if !continuation_allowed(&client, generation) {
                    return;
                }
                debug!(
                    target: "Client/OfflineResume",
                    "Deferred continuation batch (count={}) once the lane caught up",
                    BATCH_SIZE,
                );
                send_continuation(&client, generation).await;
            }
        }))
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn batch_request_stanza_shape() {
        let node = build_offline_batch_request(200);
        assert_eq!(node.tag.as_ref(), "ib");
        let children = node.children().expect("ib must have children");
        assert_eq!(children.len(), 1);
        let inner = &children[0];
        assert_eq!(inner.tag.as_ref(), "offline_batch");
        let count = inner
            .attrs
            .get("count")
            .map(|v| v.as_str().into_owned())
            .expect("count attr present");
        assert_eq!(count, "200");
    }

    #[test]
    fn first_arrival_cas_is_exclusive() {
        let c = OfflineBatchCoordinator::new();
        c.mark_batch_inflight();
        assert!(c.try_claim_first_arrival(), "first caller wins");
        assert!(
            !c.try_claim_first_arrival(),
            "subsequent callers in same cycle fail"
        );
        c.mark_batch_inflight();
        assert!(
            c.try_claim_first_arrival(),
            "new cycle: another first arrival wins"
        );
    }

    #[test]
    fn arm_publishes_first_batch_inflight() {
        let c = OfflineBatchCoordinator::new();
        assert!(!c.is_armed_for(42));
        c.arm(42, 1_000);
        assert!(c.is_armed_for(42));
        assert!(!c.is_armed_for(43), "stale generation must not be armed");
        assert!(
            c.try_claim_first_arrival(),
            "first stanza after arm wins CAS"
        );
        assert!(
            !c.try_claim_first_arrival(),
            "duplicate arrival in same cycle loses"
        );
    }

    #[test]
    fn reset_disarms_and_clears_inflight() {
        let c = OfflineBatchCoordinator::new();
        c.arm(7, 1_000);
        c.reset();
        assert!(!c.is_armed_for(7));
        assert!(
            !c.try_claim_first_arrival(),
            "reset must leave prev_batch_inflight=false"
        );
        assert!(
            c.current_pacing(7).is_none(),
            "reset must retire the pacing object"
        );
    }

    async fn armed_client(generation: u64, total: usize) -> Arc<Client> {
        let client = crate::test_utils::create_test_client().await;
        client.offline_batch.arm(generation, total);
        client
    }

    fn offline_message_node() -> Arc<wacore_binary::OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(
            &NodeBuilder::new("message")
                .attr("from", "120363000000000042@g.us")
                .attr("id", "OFFLINE-1")
                .attr("offline", "1")
                .build(),
        )
    }

    /// S2: the gate opens exactly while what the last requests still owe fits
    /// one batch, and the check books the new request in the same transition.
    #[tokio::test]
    async fn the_gate_books_one_request_while_the_last_ones_fit_a_batch() {
        let client = armed_client(7, 1_000).await;
        let coord = &client.offline_batch;
        assert_eq!(
            coord.outstanding(),
            BATCH_SIZE as usize,
            "the first request is booked with the arm"
        );
        let pacing = coord.current_pacing(7).expect("armed");

        assert!(
            matches!(pacing.reserve(), Reservation::Request),
            "one batch outstanding still fits another"
        );
        assert_eq!(coord.outstanding(), 2 * BATCH_SIZE as usize);
        assert!(
            matches!(pacing.reserve(), Reservation::Deferred),
            "two batches is where a request has to wait"
        );
        assert_eq!(
            coord.outstanding(),
            2 * BATCH_SIZE as usize,
            "a parked request books nothing"
        );
    }

    /// The preview clamps every booking, so a request never asks for more than
    /// the server announced and nothing is left parked once it is all booked.
    #[tokio::test]
    async fn a_request_never_books_more_than_the_preview_announced() {
        let client = armed_client(7, 3).await;
        let coord = &client.offline_batch;
        assert_eq!(coord.outstanding(), 3, "three items, not a whole batch");
        let pacing = coord.current_pacing(7).expect("armed");
        assert!(matches!(pacing.reserve(), Reservation::Exhausted));
        assert_eq!(coord.outstanding(), 3);
    }

    /// S3, blocker (3) as a named regression: a guard is bound to the pacing
    /// object it was minted with, so a reconnect cannot make it settle the
    /// live generation's counters.
    #[tokio::test]
    async fn a_stale_admission_guard_leaves_the_new_generations_pacing_alone() {
        let client = armed_client(7, 1_000).await;
        let node = offline_message_node();
        let stale = client
            .offline_batch
            .admit(7, &node)
            .expect("armed for generation 7");
        assert_eq!(client.offline_batch.admitted(), 1);

        client.offline_batch.reset();
        client.offline_batch.arm(9, 1_000);
        let live = client
            .offline_batch
            .admit(9, &node)
            .expect("armed for generation 9");
        let (outstanding, admitted) = (
            client.offline_batch.outstanding(),
            client.offline_batch.admitted(),
        );

        drop(stale);
        assert_eq!(
            (
                client.offline_batch.outstanding(),
                client.offline_batch.admitted()
            ),
            (outstanding, admitted),
            "the retired connection's guard must not settle the new generation"
        );
        drop(live);
        assert_eq!(
            client.offline_batch.admitted(),
            0,
            "the live guard still settles its own generation"
        );
        assert_eq!(client.offline_batch.outstanding(), outstanding - 1);
    }

    /// S4: the finish that brings the lane back within one batch is the one
    /// that releases the parked request. Parked and released are the same
    /// object and the same lock, so there is no window to lose it in.
    #[tokio::test]
    async fn the_finish_that_opens_the_gate_releases_the_parked_request() {
        let client = armed_client(7, 1_000).await;
        let batch = BATCH_SIZE as usize;
        let pacing = client.offline_batch.current_pacing(7).expect("armed");
        assert!(
            matches!(pacing.reserve(), Reservation::Request),
            "the second request is booked while the first still fits"
        );
        for _ in 0..2 * batch {
            pacing.admit_lane_bound();
        }
        assert_eq!(client.offline_batch.outstanding(), 2 * batch);
        assert!(matches!(pacing.reserve(), Reservation::Deferred));
        assert!(client.offline_batch.deferred());

        // Every finish but the last one lands on a lane that is still over the
        // gate, so none of them may claim the request.
        let mut released = 0;
        for _ in 0..batch - 1 {
            released += usize::from(pacing.finish_admitted());
        }
        assert_eq!(released, 0);
        assert!(
            client.offline_batch.deferred(),
            "still over the gate, still parked"
        );

        assert!(
            pacing.finish_admitted(),
            "the finish that reaches one batch releases it, exactly once"
        );
        assert!(!client.offline_batch.deferred());
        assert_eq!(
            client.offline_batch.outstanding(),
            2 * batch,
            "the released request is booked as it is released"
        );
    }

    /// S4, blocker (2) as a named regression: in every interleaving of "the
    /// debounce decides" and "the last messages finish", exactly one of the two
    /// books the next request — there is no ordering in which both miss it.
    #[test]
    fn finishing_the_lane_while_a_request_is_being_deferred_always_releases_it() {
        let total = 1_000;
        let batch = BATCH_SIZE as usize;
        for _ in 0..200 {
            let pacing = Arc::new(Pacing::new(7, total).0);
            // Two requests out, and the whole of both in one lane.
            pacing.book_first();
            assert!(matches!(pacing.reserve(), Reservation::Request));
            for _ in 0..2 * batch {
                pacing.admit_lane_bound();
            }

            let line_up = Arc::new(Barrier::new(2));
            let bookings = std::thread::scope(|scope| {
                let debounce = {
                    let pacing = Arc::clone(&pacing);
                    let line_up = Arc::clone(&line_up);
                    scope.spawn(move || {
                        line_up.wait();
                        usize::from(matches!(pacing.reserve(), Reservation::Request))
                    })
                };
                let finishes = {
                    let pacing = Arc::clone(&pacing);
                    let line_up = Arc::clone(&line_up);
                    scope.spawn(move || {
                        line_up.wait();
                        (0..2 * batch).filter(|_| pacing.finish_admitted()).count()
                    })
                };
                debounce.join().expect("debounce") + finishes.join().expect("finishes")
            });

            assert_eq!(
                bookings, 1,
                "exactly one caller may book the request after the lane drains"
            );
            let state = pacing.state();
            assert!(!state.deferred, "the release always clears the park");
            assert_eq!(
                state.unbooked,
                total - 3 * batch,
                "exactly one request more was asked for"
            );
            assert_eq!(
                state.outstanding, batch,
                "its stanzas are owed, and the two arrived batches are settled"
            );
        }
    }

    /// S4: a window of silence while we hold the drain open is our own
    /// backpressure, not the server giving up. Once nothing is in a lane the
    /// resume may complete, but silence never forgives what the server still
    /// owes: the parked request stays parked and nothing more is booked.
    #[tokio::test]
    async fn silence_we_cause_is_not_reported_as_a_stalled_drain() {
        let client = armed_client(7, 1_000).await;
        let coord = &client.offline_batch;
        let node = offline_message_node();
        let guard = coord.admit(7, &node).expect("armed");

        assert!(
            matches!(coord.settle_silent_drain(7), SilentDrain::Backpressure),
            "a lane we are still feeding is not a stalled drain"
        );

        let pacing = coord.current_pacing(7).expect("armed");
        drop(guard);
        assert!(matches!(pacing.reserve(), Reservation::Request));
        assert!(matches!(pacing.reserve(), Reservation::Deferred));
        let owed = coord.outstanding();
        assert!(
            matches!(coord.settle_silent_drain(7), SilentDrain::Idle),
            "nothing is in a lane, so the watchdog may complete"
        );
        assert_eq!(
            coord.outstanding(),
            owed,
            "silence must not settle what the server still owes"
        );
        assert!(
            coord.deferred(),
            "and must not book the parked request on top of it"
        );
    }

    /// S4: a parked request is released by the lane draining below one batch,
    /// through the guard path production uses, not by anything the reader does.
    #[tokio::test]
    async fn the_lane_draining_below_one_batch_releases_the_parked_request() {
        let client = armed_client(7, 1_000).await;
        let coord = &client.offline_batch;
        let batch = BATCH_SIZE as usize;
        let node = offline_message_node();
        let guards: Vec<_> = (0..2 * batch)
            .map(|_| coord.admit(7, &node).expect("armed"))
            .collect();
        let pacing = coord.current_pacing(7).expect("armed");
        assert!(matches!(pacing.reserve(), Reservation::Request));
        assert!(matches!(pacing.reserve(), Reservation::Deferred));
        assert!(coord.deferred());

        drop(guards);
        assert!(
            !coord.deferred(),
            "the drops that brought the lane back under one batch released it"
        );
        assert_eq!(
            coord.outstanding(),
            batch,
            "and the released request is what is now owed"
        );
    }
}
