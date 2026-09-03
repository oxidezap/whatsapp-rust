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
}

impl OfflineBatchCoordinator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&self) {
        self.armed.store(false, Ordering::Release);
        self.generation.store(0, Ordering::Release);
        self.prev_batch_inflight.store(false, Ordering::Release);
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

    fn arm(&self, generation: u64) {
        // Set inner state first; `armed` is published last so readers that
        // observe armed=true also see the matching generation and inflight.
        self.generation.store(generation, Ordering::Release);
        self.stanza_activity.store(0, Ordering::Release);
        self.prev_batch_inflight.store(true, Ordering::Release);
        self.armed.store(true, Ordering::Release);
    }

    fn is_armed_for(&self, generation: u64) -> bool {
        self.armed.load(Ordering::Acquire) && self.generation.load(Ordering::Acquire) == generation
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
    client.offline_batch.arm(generation);
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
pub(crate) fn on_offline_stanza_arrived(client: &Arc<Client>, pending: usize) {
    let coord = &client.offline_batch;
    let generation = client.connection_generation.load(Ordering::Acquire);
    if !coord.is_armed_for(generation) {
        return;
    }
    coord.note_stanza_activity();
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
            if !client.offline_batch.is_armed_for(generation) {
                return;
            }
            if client.connection_generation.load(Ordering::Acquire) != generation {
                return;
            }
            if client.offline_sync_completed.load(Ordering::Acquire) {
                return;
            }
            debug!(
                target: "Client/OfflineResume",
                "Continuation batch (count={})",
                BATCH_SIZE,
            );
            let _ = send_batch(&client, BATCH_SIZE).await;
            // Re-check generation after the await: a disconnect+reconnect+rearm
            // during the send would leave the coordinator owned by a newer
            // cycle, and republishing here would overwrite that cycle's CAS
            // state and let a duplicate continuation slip through.
            if client.offline_batch.is_armed_for(generation) {
                client.offline_batch.mark_batch_inflight();
            }
        }))
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

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
        c.arm(42);
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
        c.arm(7);
        c.reset();
        assert!(!c.is_armed_for(7));
        assert!(
            !c.try_claim_first_arrival(),
            "reset must leave prev_batch_inflight=false"
        );
    }
}
