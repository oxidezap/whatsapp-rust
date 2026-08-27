//! E2E Session management for Client.

use anyhow::Result;
use rand::rngs::StdRng;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wacore::libsignal::protocol::{
    IdentityChange, PreKeyBundle, SignalProtocolError, UsePQRatchet, process_prekey_bundle,
};
use wacore::libsignal::store::SessionStore;
use wacore::stats::UnkeyableDevice;
use wacore::types::jid::JidExt;
use wacore_binary::Jid;

use super::Client;
use crate::types::events::{Event, OfflineSyncCompleted};

/// Waiter side of an in-flight ensure: it never receives a value, only the
/// close that the leader's drop produces. A leader that panics or is cancelled
/// drops its sender the same way a finished one does, so no waiter can be
/// stranded by an outcome the leader never got to report.
type EnsureWaiter = async_channel::Receiver<std::convert::Infallible>;

/// What a waiter holds while another caller establishes its address.
#[derive(Clone)]
struct EnsureSlot {
    /// Published by the leader, under the same lock that unregisters the slot.
    /// Distinguishes "the leader ran and the server had nothing for this
    /// device" — where retrying only repeats an answered question — from "the
    /// leader died", where nobody has asked yet.
    completed: Arc<std::sync::atomic::AtomicBool>,
    released: EnsureWaiter,
}

/// Which addresses are currently having a session established, keyed by
/// protocol address. See [`Client::ensure_inflight`] for why this exists.
#[derive(Default)]
pub(crate) struct EnsureRegistry {
    inflight: std::sync::Mutex<std::collections::HashMap<Box<str>, EnsureSlot>>,
}

/// How one `ensure` pass split its input against what was already in flight.
struct EnsureReservation {
    /// Claimed by this caller: probed and, if needed, fetched here.
    owned: Vec<Jid>,
    /// Left to another caller's in-flight ensure, with the slot to wait on.
    deferred: Vec<(Jid, EnsureSlot)>,
    /// `None` when nothing was claimed.
    lease: Option<EnsureLease>,
}

/// One address this caller claimed.
///
/// Both halves are per address rather than per lease, because the fetch is
/// chunked: an early chunk the server answered stays answered even if a later
/// one fails, and its waiters should not wait on the later one either. Sharing
/// either half would make an unrelated stalled IQ decide both.
struct EnsureClaim {
    address: Box<str>,
    completed: Arc<std::sync::atomic::AtomicBool>,
    /// Dropped to wake this address's waiters — on completion, or with the
    /// lease if this caller never got that far.
    release: Option<async_channel::Sender<std::convert::Infallible>>,
}

/// The addresses this caller claimed, released on drop.
///
/// Claiming and releasing are both synchronous, so a caller can claim before
/// its first await and cannot leave a claim behind.
struct EnsureLease {
    registry: Arc<EnsureRegistry>,
    claims: Vec<EnsureClaim>,
}

impl EnsureLease {
    /// Record that these addresses were carried as far as this caller could,
    /// and let their waiters go without waiting for the rest of the batch.
    fn mark_completed(&mut self, jids: &[Jid]) {
        let mut finished = Vec::with_capacity(jids.len());
        for jid in jids {
            let address = jid.to_protocol_address_string();
            if let Some(claim) = self
                .claims
                .iter()
                .find(|claim| claim.address.as_ref() == address)
            {
                finished.push(claim.address.clone());
            }
        }

        // Completion is published by the same lock that removes the slot, so
        // "registered" and "complete" are never both true for an onlooker.
        // Otherwise retry recovery deleting this session in that gap would join
        // the slot and inherit a verdict about the session it just deleted.
        //
        // Retired here rather than at the lease's drop for the same reason: a
        // claim that outlives its work is a claim a later caller joins.
        self.registry.complete_and_retire(&finished, &self.claims);

        for address in &finished {
            if let Some(claim) = self
                .claims
                .iter_mut()
                .find(|claim| &claim.address == address)
            {
                drop(claim.release.take());
            }
        }
    }
}

impl Drop for EnsureLease {
    fn drop(&mut self) {
        let outstanding: Vec<Box<str>> = self
            .claims
            .iter()
            .filter(|claim| claim.release.is_some())
            .map(|claim| claim.address.clone())
            .collect();
        self.registry.retire(&outstanding, &self.claims);
    }
}

impl EnsureRegistry {
    /// Split `jids` into the ones this caller now owns and the ones someone
    /// else already claimed.
    ///
    /// Synchronous and taken in one lock, so two callers racing over the same
    /// address cannot both come away as leader.
    fn reserve(self: &Arc<Self>, jids: &[Jid]) -> EnsureReservation {
        use wacore::types::jid::JidExt;

        let mut owned = Vec::with_capacity(jids.len());
        let mut claims = Vec::with_capacity(jids.len());
        let mut deferred = Vec::new();

        {
            let mut inflight = self.map();
            for jid in jids {
                let address = jid.to_protocol_address_string().into_boxed_str();
                match inflight.entry(address) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        deferred.push((jid.clone(), entry.get().clone()));
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let completed = Arc::new(std::sync::atomic::AtomicBool::new(false));
                        let (release, released) = async_channel::bounded(1);
                        claims.push(EnsureClaim {
                            address: entry.key().clone(),
                            completed: completed.clone(),
                            release: Some(release),
                        });
                        entry.insert(EnsureSlot {
                            completed,
                            released,
                        });
                        owned.push(jid.clone());
                    }
                }
            }
        }

        let lease = (!claims.is_empty()).then(|| EnsureLease {
            registry: self.clone(),
            claims,
        });
        EnsureReservation {
            owned,
            deferred,
            lease,
        }
    }

    /// The registry map, ignoring poisoning.
    ///
    /// Nothing inside a critical section here can unwind — the sections do map
    /// lookups and removals on already-owned values — so a poisoned lock would
    /// mean a panic from elsewhere, and honouring it would strand every address
    /// registered at that moment with no way to ever re-claim them.
    fn map(&self) -> std::sync::MutexGuard<'_, std::collections::HashMap<Box<str>, EnsureSlot>> {
        self.inflight
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    /// Remove these addresses, but only where the registered slot is still the
    /// one `claims` describes.
    ///
    /// Identity matters because a claim is retired as soon as its work is done,
    /// which leaves the lease's later drop running against a map another caller
    /// may have re-entered. Removing by address alone would evict that
    /// caller's live claim and let a third one fetch alongside it.
    fn retire(&self, addresses: &[Box<str>], claims: &[EnsureClaim]) {
        self.retire_inner(addresses, claims, false);
    }

    /// [`retire`](Self::retire), publishing completion under the same lock.
    fn complete_and_retire(&self, addresses: &[Box<str>], claims: &[EnsureClaim]) {
        self.retire_inner(addresses, claims, true);
    }

    fn retire_inner(&self, addresses: &[Box<str>], claims: &[EnsureClaim], complete: bool) {
        if addresses.is_empty() {
            return;
        }
        let mut inflight = self.map();
        for address in addresses {
            let Some(claim) = claims.iter().find(|claim| &claim.address == address) else {
                continue;
            };
            if let std::collections::hash_map::Entry::Occupied(entry) =
                inflight.entry(address.clone())
                && Arc::ptr_eq(&entry.get().completed, &claim.completed)
            {
                entry.remove();
            }
            if complete {
                claim.completed.store(true, Ordering::Release);
            }
        }
    }

    /// Whether any registered slot is already marked complete.
    ///
    /// Always false: completion and removal happen under one lock, so the pair
    /// is unobservable. Exposed for the test that holds that invariant down,
    /// since nothing else can see inside the registry.
    #[cfg(test)]
    pub(crate) fn any_completed_still_registered(&self) -> bool {
        self.map()
            .values()
            .any(|slot| slot.completed.load(Ordering::Acquire))
    }

    /// Number of addresses currently being established. Reported by
    /// `memory_report()`; normally zero.
    pub(crate) fn len(&self) -> usize {
        self.map().len()
    }
}

/// Wait for every leader we deferred to, and report back the addresses whose
/// leader died without doing the work. A closed channel is the only outcome
/// there is: leaders never send, they drop.
async fn await_leaders(deferred: Vec<(Jid, EnsureSlot)>) -> Vec<Jid> {
    let mut abandoned = Vec::new();
    for (jid, slot) in deferred {
        let _ = slot.released.recv().await;
        if !slot.completed.load(Ordering::Acquire) {
            abandoned.push(jid);
        }
    }
    abandoned
}

impl Client {
    /// Install a supplied pre-key bundle into the shared Signal cache.
    ///
    /// The caller owns batching and the final durability flush. Keeping those
    /// outside lets multi-device establishment reuse one adapter and one flush.
    pub(crate) async fn install_prekey_bundle_cached(
        &self,
        jid: &Jid,
        bundle: &PreKeyBundle,
        adapter: &mut crate::store::signal_adapter::SignalProtocolStoreAdapter,
        rng: &mut StdRng,
    ) -> Result<IdentityChange, SignalProtocolError> {
        let signal_address = jid.to_protocol_address();
        let session_mutex = self.session_lock_for(signal_address.as_str()).await;
        let session_guard = session_mutex.lock().await;

        let identity_change = process_prekey_bundle(
            &signal_address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
            bundle,
            rng,
            UsePQRatchet::No,
        )
        .await?;

        drop(session_guard);
        if identity_change == IdentityChange::ReplacedExisting {
            self.react_to_local_identity_change(jid);
        }
        Ok(identity_change)
    }

    /// WA Web: `WAWebOfflineResumeConst.OFFLINE_STANZA_TIMEOUT_MS = 60000`
    pub(crate) const DEFAULT_OFFLINE_SYNC_TIMEOUT: Duration = Duration::from_secs(60);

    pub(crate) async fn complete_offline_sync(&self, count: i32) {
        self.offline_sync_metrics
            .active
            .store(false, Ordering::Release);
        match self.offline_sync_metrics.start_time.lock() {
            Ok(mut guard) => *guard = None,
            Err(poison) => *poison.into_inner() = None,
        }

        // Run the finisher once (the semaphore swap is not idempotent). The
        // guard is a dedicated flag, NOT offline_sync_completed: that one only
        // flips after the tail commit so the tail's acks still observe it as
        // false and join the aggregate offline-receipt drain (WA Web
        // `sendAggregateOfflineReceipts`) instead of going out 1:1.
        if self
            .offline_sync_finish_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) else {
            // Practically unreachable (the run loop owns a strong Arc), but a
            // silent skip would leave the client batching forever with a
            // widened semaphore — leave consistent live state behind instead.
            log::error!(
                "complete_offline_sync: self_weak upgrade failed; dropping the drain tail and switching to live mode"
            );
            self.inbound_commit_batch.force_live_dropping_entries();
            self.publish_offline_sync_live_state(count, None);
            return;
        };

        // The `ib` offline end marker is processed INLINE on the read loop, and
        // the tail commit awaits the processing permit plus the durable write,
        // Signal flush and the consumer's durability hook. Parking the read
        // loop on that would starve IQ responses and pongs — a hook awaiting
        // any server round-trip would deadlock, and a merely slow hook would
        // trip the keepalive at the end of every drain. Ordering does not need
        // the inline await: the single permit already serializes the finisher
        // against stanza processing, so run it off-loop.
        let generation = self.connection_generation.load(Ordering::Acquire);
        self.runtime
            .spawn(Box::pin(async move {
                client.finish_offline_sync(count, generation).await;
            }))
            .detach();
    }

    /// Off-read-loop tail of [`complete_offline_sync`]: commit the drain tail,
    /// then flip the completed flag, widen the semaphore and flush the
    /// aggregate receipts. `generation` guards against a reconnect racing this
    /// task — the new connection resets the drain state and must not have its
    /// flag/semaphore/batcher touched by the old connection's finisher.
    async fn finish_offline_sync(self: &Arc<Self>, count: i32, generation: u64) {
        // Commit the drain tail and flip the batcher to live mode under the
        // still-single processing permit (see flush_inbound_commits_under_permit
        // for the raceless-transition argument), BEFORE widening the semaphore.
        // Receipts flush after, so every receipt's message is durably committed
        // first (WA Web's createSnapshot ordering).
        let durable = self.finish_inbound_commit_drain(generation).await;

        if self.connection_generation.load(Ordering::Acquire) != generation {
            log::debug!(
                "finish_offline_sync: connection generation changed during the tail commit; leaving the new connection's state alone"
            );
            return;
        }

        self.publish_offline_sync_live_state(count, Some(durable));
    }

    /// The drain→live state publication, shared by the finisher and its
    /// upgrade-failure fallback (non-async so the codegen stays out of their
    /// state machines).
    ///
    /// Readers that observe offline_sync_completed=true short-circuit without
    /// touching the semaphore (wait_for_offline_delivery_end returns early),
    /// so the ordering of flag flip vs. semaphore swap is not observable: any
    /// in-flight worker keeps using its old 1-permit Arc and drains normally;
    /// newly-spawned workers pick up the 64-permit semaphore via
    /// read_message_semaphore(). The flag flip happens-before the receipt
    /// drain takes the buffer lock, so late offline receipts either land in
    /// the flush or observe the flag and send 1:1
    /// (see try_buffer_offline_receipt).
    ///
    /// `durable`: `Some(true)` flushes the buffered offline receipts;
    /// `Some(false)` drops them — the tail's durable write failed, its entries
    /// are back in the batcher unacked, and receipting SKDM/session state that
    /// never became durable would trade a redeliverable failure for a
    /// crash-permanent one. In that case the batcher stays in drain mode AND
    /// the semaphore stays at one permit: the whole-cache flush inside a
    /// retry commit is only safe while no other stanza can be mid-decrypt
    /// with unenqueued ratchet advances. The first durable flush completes
    /// the deferred transition (see `complete_deferred_live_transition`) and
    /// widens the semaphore then. `None` (upgrade-failure fallback) leaves
    /// the buffer alone for the connection-state reset to clear.
    fn publish_offline_sync_live_state(&self, count: i32, durable: Option<bool>) {
        self.offline_sync_completed.store(true, Ordering::Release);
        if durable != Some(false) {
            self.swap_message_semaphore(64);
        }
        match durable {
            Some(true) => self.flush_offline_receipts(),
            Some(false) => {
                log::warn!(
                    "finish_offline_sync: tail commit not durable; dropping buffered offline receipts so the server redelivers"
                );
                self.clear_offline_receipt_buffer();
            }
            None => {}
        }
        self.offline_sync_notifier.notify(usize::MAX);
        self.core.event_bus.dispatch(Event::OfflineSyncCompleted(
            OfflineSyncCompleted::builder().count(count).build(),
        ));
    }

    /// Wait for offline message delivery to complete (with timeout).
    pub(crate) async fn wait_for_offline_delivery_end(&self) {
        self.wait_for_offline_delivery_end_with_timeout(Self::DEFAULT_OFFLINE_SYNC_TIMEOUT)
            .await;
    }

    pub(crate) async fn wait_for_offline_delivery_end_with_timeout(&self, timeout: Duration) {
        let wait_generation = self.connection_generation.load(Ordering::Acquire);
        let offline_fut = self.offline_sync_notifier.listen();
        if self.offline_sync_completed.load(Ordering::Relaxed) {
            return;
        }

        if wacore::runtime::timeout(&*self.runtime, timeout, offline_fut)
            .await
            .is_err()
        {
            // Guard: don't complete sync for a stale connection generation.
            // A reconnect may have happened while we were waiting, making this
            // timeout belong to the old connection.
            if self.connection_generation.load(Ordering::Acquire) != wait_generation
                || self.expected_disconnect.load(Ordering::Relaxed)
            {
                log::debug!(
                    target: "Client/OfflineSync",
                    "Offline sync timeout ignored: connection generation changed or disconnected",
                );
                return;
            }

            let processed = self
                .offline_sync_metrics
                .processed_messages
                .load(Ordering::Acquire);
            let expected = self
                .offline_sync_metrics
                .total_messages
                .load(Ordering::Acquire);
            log::warn!(
                target: "Client/OfflineSync",
                "Offline sync timed out after {:?} (processed {} of {} items); marking sync complete",
                timeout,
                processed,
                expected,
            );
            self.complete_offline_sync(i32::try_from(processed).unwrap_or(i32::MAX))
                .await;
            // The finisher runs as a spawned task; keep this helper's contract
            // that live state is in place when it returns (callers start
            // session/send work right after). Ticked so a reconnect or
            // shutdown mid-commit cannot strand this waiter, and bounded by a
            // second `timeout` window: when the marker-triggered finisher was
            // ALREADY running and its tail commit/hook is stuck, the
            // complete_offline_sync above started nothing, and an unbounded
            // wait here would defeat this helper's whole point — callers
            // proceed and the finisher keeps running in the background.
            let deadline = wacore::time::Instant::now() + timeout;
            loop {
                let listener = self.offline_sync_notifier.listen();
                if self.offline_sync_completed.load(Ordering::Acquire)
                    || self.connection_generation.load(Ordering::Acquire) != wait_generation
                    || self.expected_disconnect.load(Ordering::Relaxed)
                {
                    return;
                }
                if wacore::time::Instant::now() >= deadline {
                    log::warn!(
                        target: "Client/OfflineSync",
                        "Drain finisher still running {:?} after the offline sync timeout; proceeding without it",
                        timeout,
                    );
                    return;
                }
                let _ = wacore::runtime::timeout(&*self.runtime, Duration::from_secs(1), listener)
                    .await;
            }
        }
    }

    pub(crate) fn begin_history_sync_task(
        &self,
        payload_bytes: usize,
    ) -> crate::sync_task::HistorySyncTaskTracker {
        self.history_sync_activity.begin(payload_bytes)
    }

    pub async fn wait_for_startup_sync(&self, timeout: Duration) -> Result<()> {
        use anyhow::anyhow;
        use wacore::time::Instant;

        let deadline = Instant::now() + timeout;

        // Register the notified future *before* checking state to avoid missing
        // a notify_waiters() that fires between the check and the await.
        let offline_fut = self.offline_sync_notifier.listen();
        if !self.offline_sync_completed.load(Ordering::Relaxed) {
            let remaining = deadline.saturating_duration_since(Instant::now());
            wacore::runtime::timeout(&*self.runtime, remaining, offline_fut)
                .await
                .map_err(|_| anyhow!("Timeout waiting for offline sync completion"))?;
        }

        loop {
            let history_fut = self.history_sync_activity.listen();
            if self.history_sync_activity.tasks() == 0 {
                return Ok(());
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            wacore::runtime::timeout(&*self.runtime, remaining, history_fut)
                .await
                .map_err(|_| anyhow!("Timeout waiting for history sync tasks to become idle"))?;
        }
    }

    /// Ensure E2E sessions exist for the given device JIDs.
    /// Waits for offline delivery, resolves LID mappings, then batches prekey fetches.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.ensure", level = "debug", skip_all, fields(count = device_jids.len()), err(Debug)))]
    pub(crate) async fn ensure_e2e_sessions(&self, device_jids: &[Jid]) -> Result<()> {
        if device_jids.is_empty() {
            return Ok(());
        }
        self.wait_for_offline_delivery_end().await;
        let resolved_jids = self.resolve_lid_mappings(device_jids).await;
        self.ensure_sessions_inner(resolved_jids).await
    }

    /// Like `ensure_e2e_sessions` but skips `resolve_lid_mappings`. Use when the
    /// caller already resolved JIDs to the correct namespace (e.g., after
    /// alternate PN/LID key normalization in retry handling).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.ensure_resolved", level = "debug", skip_all, fields(count = jids.len()), err(Debug)))]
    pub(crate) async fn ensure_e2e_sessions_resolved(&self, jids: &[Jid]) -> Result<()> {
        if jids.is_empty() {
            return Ok(());
        }
        self.wait_for_offline_delivery_end().await;
        self.ensure_sessions_inner(jids.to_vec()).await
    }
}

/// Whether a prekey fetch failed because the server considers the devices
/// unregistered.
///
/// Batch-wide by nature: the fetch is one IQ, so a `406` answers for every jid
/// in it rather than naming one.
///
/// Asked through `server_rejection` rather than by downcasting to one error
/// type. This preflight calls `fetch_pre_keys` directly and gets a
/// `crate::request::IqError::ServerError`; the fan-out reaches the same fetch
/// through `SendContextResolver`, which re-wraps it as a
/// `wacore::request::ServerErrorCode` to cross the crate boundary. A downcast
/// to either one alone silently answers `false` for the other, and the failure
/// mode of that is the send failing exactly as it did before.
/// The `<error code>` the server attaches to a device it no longer knows.
const UNREGISTERED_DEVICE_CODE: u16 = 406;

fn is_device_unregistered(err: &anyhow::Error) -> bool {
    use crate::error::ErrorChainExt;
    err.server_rejection()
        .is_some_and(|r| r.code == UNREGISTERED_DEVICE_CODE)
}

/// The distinct users named by `jids`, in first-seen order.
///
/// A prekey batch is usually several devices of the same one or two users, and
/// every invalidation takes the registry lock and deletes rows, so visiting a
/// user once per device would pay that repeatedly for no effect. Split out from
/// the invalidation so the deduplication is observable on its own: through the
/// cache it is not, since a user invalidated twice looks exactly like a user
/// invalidated once.
fn distinct_users(jids: &[Jid]) -> smallvec::SmallVec<[&str; 4]> {
    let mut seen: smallvec::SmallVec<[&str; 4]> = smallvec::SmallVec::new();
    for jid in jids {
        if !seen.contains(&jid.user.as_str()) {
            seen.push(jid.user.as_str());
        }
    }
    seen
}

impl Client {
    /// Refreshes the device list of every user named in `jids`, once each.
    async fn invalidate_device_caches_for(&self, jids: &[Jid]) {
        for user in distinct_users(jids) {
            self.invalidate_device_cache(user).await;
        }
    }
}

impl Client {
    /// Core session-check + prekey-fetch logic shared by both entry points.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.ensure_inner", level = "debug", skip_all, fields(count = jids.len()), err(Debug)))]
    async fn ensure_sessions_inner(&self, mut jids: Vec<Jid>) -> Result<()> {
        // A pass returns only the addresses whose leader died without doing the
        // work — not everything it deferred. A leader that ran and found no
        // bundle has already asked the question, so repeating its fetch would
        // only ask again; a leader that was cancelled asked nothing.
        //
        // Bounded, because an address a fresh caller keeps claiming and then
        // abandoning would otherwise loop here for as long as that lasts.
        const ENSURE_PASSES: usize = 3;

        for _ in 0..ENSURE_PASSES {
            jids = self.ensure_sessions_pass(jids).await?;
            if jids.is_empty() {
                return Ok(());
            }
            // An abandoned leader can have installed a session and then failed
            // to persist it, and the next pass's warm-cache probe would read
            // that entry and call the address established. Re-run the flush
            // here so the storage failure is reported to this caller rather
            // than swallowed, as it was when every caller ran its own.
            self.flush_signal_cache_batch_safe().await?;
        }

        // Reported, not swallowed: before coalescing, this caller would have
        // run its own fetch and propagated whatever that returned. Succeeding
        // here would hand the fan-out an address nobody ever fetched for.
        Err(anyhow::anyhow!(
            "session establishment for {} address(es) was abandoned by \
             {ENSURE_PASSES} successive callers",
            jids.len()
        ))
    }

    /// One claim-probe-fetch pass. Returns the jids whose in-flight leader
    /// ended without carrying them, for the caller to re-examine.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.ensure_pass", level = "debug", skip_all, fields(count = jids.len()), err(Debug)))]
    async fn ensure_sessions_pass(&self, mut jids: Vec<Jid>) -> Result<Vec<Jid>> {
        use wacore::types::jid::JidExt;

        /// What the probe learned about one address.
        enum Probed {
            HasSession,
            NeedsFetch,
            /// The backend could not answer.
            Unknown,
        }

        // Warm-cache pre-filter: a cached session answers synchronously, so
        // the common live-send case skips the probe-stream machinery below
        // entirely. Contended or unknown entries fall through to the probe.
        // Retain in place (reusing the input allocation) and rewrite one reusable
        // address per jid instead of allocating a fresh ProtocolAddress for every
        // lookup key. A plain local (not thread-local): the async probe below owns
        // its own address per concurrent task.
        let mut reusable_addr = wacore::types::jid::make_reusable_protocol_address();
        jids.retain(|jid| {
            jid.reset_protocol_address(&mut reusable_addr);
            self.signal_cache.try_has_session(&reusable_addr) != Some(true)
        });
        if jids.is_empty() {
            return Ok(Vec::new());
        }

        // Claim before the first await, so a burst of callers for one address
        // cannot all read "no session" and all fetch. Ordered like WA Web's
        // `ensureE2ESessions`: reserve first, probe only what we reserved.
        let EnsureReservation {
            owned: jids,
            deferred,
            mut lease,
        } = self.ensure_inflight.reserve(&jids);
        if jids.is_empty() {
            return Ok(await_leaders(deferred).await);
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot();

        // Probe sessions concurrently: a cold-cache multi-recipient ensure would
        // otherwise serialize the per-device DB reads (warm hits serialize on the
        // cache mutex anyway). Order is irrelevant — misses are chunked for the fetch.
        use futures::StreamExt;
        const SESSION_PROBE_CONCURRENCY: usize = 16;
        let backend = device_snapshot.backend.clone();
        let probed: Vec<(Jid, Probed)> = futures::stream::iter(jids)
            .map(|jid| {
                let backend = backend.clone();
                async move {
                    let signal_addr = jid.to_protocol_address();
                    // Check cache first (includes unflushed sessions), fall back to backend.
                    match self.signal_cache.has_session(&signal_addr, &*backend).await {
                        Ok(true) => (jid, Probed::HasSession),
                        Ok(false) => (jid, Probed::NeedsFetch),
                        Err(e) => {
                            log::warn!("Failed to check session for {}: {}", jid.observe(), e);
                            (jid, Probed::Unknown)
                        }
                    }
                }
            })
            .buffer_unordered(SESSION_PROBE_CONCURRENCY)
            .collect()
            .await;
        let mut jids_needing_sessions = Vec::with_capacity(probed.len());
        let mut satisfied = Vec::new();
        for (jid, outcome) in probed {
            match outcome {
                Probed::NeedsFetch => jids_needing_sessions.push(jid),
                Probed::HasSession => satisfied.push(jid),
                // Neither fetched nor marked. Reporting a read error as an
                // answer would spend one backend blip on the whole burst, which
                // is the silently short fan-out `fetch_and_establish_sessions`
                // refuses to produce for a refused batch.
                Probed::Unknown => {}
            }
        }

        // Nobody has to fetch for an address that already has a session, so it
        // is complete whatever the rest of this call does.
        if let Some(lease) = &mut lease {
            lease.mark_completed(&satisfied);
        }

        // Marked per chunk, as each is answered: an answered fetch that carried
        // no bundle for a device is still an answer, and our waiters must not
        // re-ask it. A later chunk failing does not unanswer an earlier one.
        let mut fetched = Ok(());
        for batch in jids_needing_sessions.chunks(crate::session::SESSION_CHECK_BATCH_SIZE) {
            match self.fetch_and_establish_sessions(batch).await {
                Ok(_) => {
                    if let Some(lease) = &mut lease {
                        lease.mark_completed(batch);
                    }
                }
                Err(error) => {
                    fetched = Err(error);
                    break;
                }
            }
        }

        // Release before waiting: a leader that keeps its claim while blocking
        // on someone else's would deadlock two callers whose batches overlap in
        // opposite order.
        drop(lease);
        // Before waiting, not after: our own fetch already decided this call's
        // answer, and a deferred leader can sit on a prekey IQ for the full
        // request timeout. Waiting first would park a send that has nothing
        // left to learn.
        fetched?;

        Ok(await_leaders(deferred).await)
    }

    /// Fetch prekeys and establish sessions for a batch of JIDs.
    /// Returns the number of sessions successfully established.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.fetch_establish", level = "debug", skip_all, fields(count = jids.len()), err(Debug)))]
    async fn fetch_and_establish_sessions(&self, jids: &[Jid]) -> Result<usize, anyhow::Error> {
        if jids.is_empty() {
            return Ok(0);
        }

        let prekey_bundles = match self
            .fetch_pre_keys(jids, Some(wacore::iq::prekeys::PreKeyFetchReason::Identity))
            .await
        {
            Ok(bundles) => bundles,
            // A `406` means the server no longer knows these devices, so the
            // cached list that named them is stale. Refresh it before giving up,
            // or the retry resolves the same absent device and collects the same
            // 406 forever. It cannot affect the send in flight, whose device set
            // is already resolved.
            //
            // The error still propagates, and deliberately so. The fetch is one
            // IQ over a batch of up to `SESSION_CHECK_BATCH_SIZE` devices, and a
            // 406 answers for the whole batch without naming which device it is
            // about. Continuing would mean treating every device in that batch
            // as having no prekeys, so a registered device that merely lacked a
            // local session would be skipped by the fan-out and the message
            // would go out to fewer devices than intended, with nothing to say
            // so. A failed send is visible and now retries against a refreshed
            // list; a silently short fan-out is neither.
            Err(e) if is_device_unregistered(&e) => {
                log::debug!(
                    "Prekey fetch returned 406 for {} device(s); \
                     refreshing their device lists before failing the send",
                    jids.len()
                );
                // `BatchRefused`, not `Rejected`: one IQ answered for the whole
                // batch without naming a device, so a registered device can be
                // in here and the label must not claim otherwise.
                self.stats
                    .record_unkeyable_devices(UnkeyableDevice::BatchRefused, jids.len() as u64);
                self.invalidate_device_caches_for(jids).await;
                return Err(e);
            }
            Err(e) => {
                // Same devices left unkeyed as a refusal leaves, for a reason
                // that names none of them: a timeout, a dropped socket, a 429.
                self.stats
                    .record_unkeyable_devices(UnkeyableDevice::FetchFailed, jids.len() as u64);
                return Err(e);
            }
        };

        // The server named these individually, which is the per-device signal a
        // batch-wide failure cannot give: refresh exactly their device lists and
        // leave the rest of the batch alone. The send continues, because the
        // devices that did come back with a bundle are unaffected and skipping
        // them would deliver to fewer devices for a reason that only concerns
        // the named ones.
        if !prekey_bundles.rejected.is_empty() {
            // Only a 406 claims the device is gone, so only a 406 pays for a
            // device-list refresh. Every other code is counted under its class
            // and left alone: a 5xx or a rate limit says the server could not
            // answer, and refreshing on those turns a server-side wobble into a
            // usync fan-out across every group the bot sends to.
            for device in &prekey_bundles.rejected {
                self.stats
                    .record_unkeyable_device(UnkeyableDevice::Rejected(device.code));
            }
            let rejected: Vec<Jid> = prekey_bundles
                .rejected
                .iter()
                .filter(|device| device.code == UNREGISTERED_DEVICE_CODE)
                .map(|device| device.jid.clone())
                .collect();
            if !rejected.is_empty() {
                log::debug!(
                    "prekey fetch rejected {} of {} device(s) as unregistered; \
                     refreshing their device lists",
                    rejected.len(),
                    jids.len()
                );
                self.invalidate_device_caches_for(&rejected).await;
            }
        }

        let mut adapter = self.signal_adapter();
        let mut rng = rand::make_rng::<StdRng>();

        let mut success_count = 0;
        let mut missing_count = 0;
        let mut failed_count = 0;

        for jid in jids {
            if let Some(bundle) = prekey_bundles.bundles.get(jid) {
                match self
                    .install_prekey_bundle_cached(jid, bundle, &mut adapter, &mut rng)
                    .await
                {
                    Ok(_) => {
                        success_count += 1;
                        log::debug!("Successfully established session with {}", jid.observe());
                    }
                    Err(e) => {
                        failed_count += 1;
                        self.stats
                            .record_unkeyable_device(UnkeyableDevice::SessionSetup);
                        log::warn!("Failed to establish session with {}: {}", jid.observe(), e);
                    }
                }
            } else {
                missing_count += 1;
                // A device the server named is counted as that rejection above;
                // counting it here too would report one drop as two.
                if !prekey_bundles
                    .rejected
                    .iter()
                    .any(|device| device.jid == *jid)
                {
                    self.stats
                        .record_unkeyable_device(UnkeyableDevice::NoBundle);
                }
                if jid.device == 0 {
                    log::warn!(
                        "Server did not return prekeys for primary phone {}",
                        jid.observe()
                    );
                } else {
                    log::debug!("Server did not return prekeys for {}", jid.observe());
                }
            }
        }

        if missing_count > 0 || failed_count > 0 {
            log::debug!(
                "Session establishment: {} succeeded, {} missing prekeys, {} failed (of {} requested)",
                success_count,
                missing_count,
                failed_count,
                jids.len()
            );
        }

        // Flush after all sessions established. Batch-safe: retry receipts
        // reach here mid-drain, and post-timeout senders reach here during a
        // deferred live transition — in both windows a raw whole-cache flush
        // would persist rowless drain entries' ratchet advances.
        if success_count > 0 {
            self.flush_signal_cache_batch_safe().await?;
        }

        Ok(success_count)
    }

    /// Log primary phone (device 0) session state at login.
    /// Migration is lazy via try_pn_to_lid_migration_decrypt on first message.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.primary_phone_check",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn establish_primary_phone_session_immediate(&self) -> Result<()> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();

        let own_pn = device_snapshot
            .pn
            .clone()
            .ok_or_else(|| anyhow::Error::from(crate::client::ClientError::NotLoggedIn))?;

        let Some(ref own_lid) = device_snapshot.lid else {
            log::debug!("No own LID yet, skipping primary phone session check");
            return Ok(());
        };

        let primary_phone_lid = own_lid.with_device(0);
        let primary_phone_pn = own_pn.with_device(0);

        let lid_exists = self
            .check_session_exists(&primary_phone_lid)
            .await
            .unwrap_or(false);
        let pn_exists = self
            .check_session_exists(&primary_phone_pn)
            .await
            .unwrap_or(false);

        match (lid_exists, pn_exists) {
            (true, _) => log::debug!("LID session with {} exists", primary_phone_lid.observe()),
            (false, true) => {
                log::debug!("PN-only session for own device 0 — will migrate on first message")
            }
            (false, false) => {
                log::debug!("No session with own device 0 — will establish on first message")
            }
        }

        Ok(())
    }

    /// Whether `message_encrypt` for `jid` would emit a pkmsg (no session, or a
    /// session with an un-acked pre-key still pending). Reuses the send path's
    /// pre-flight so the voip offer treats a session-present-but-unacked device as
    /// pkmsg too, not as plain msg.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn would_emit_pkmsg(&self, jid: &Jid) -> Result<bool, anyhow::Error> {
        let device_store = self.persistence_manager.clone();
        let mut adapter = self.signal_adapter_from(device_store);
        let signal_addr = jid.to_protocol_address();
        wacore::send::pkmsg_would_be_emitted(&mut adapter.session_store, &signal_addr).await
    }

    /// Check if a session exists for the given JID.
    pub(crate) async fn check_session_exists(&self, jid: &Jid) -> Result<bool, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let signal_addr = jid.to_protocol_address();

        device_snapshot
            .contains_session(&signal_addr)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to check session for {}: {}", jid.observe(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{JidExt, Node, NodeValue, Server};

    /// Answer the prekey fetch the client just wrote with a `<list>` of
    /// `users`, so a test can hand the session path exactly the response shape
    /// it wants to account for.
    async fn answer_prekey_fetch(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        users: Vec<Node>,
    ) {
        let iq = crate::test_utils::decode_sent_iq(transport, 0).await;
        let request_id = iq
            .get()
            .attrs()
            .optional_string("id")
            .expect("the prekey fetch carries an id")
            .into_owned();
        let response = NodeBuilder::new("iq")
            .attr("id", request_id.as_str())
            .attr("type", "result")
            .children([NodeBuilder::new("list").children(users).build()])
            .build();
        crate::test_utils::answer_iq(client, &request_id, &response).await;
    }

    /// A device that comes back with an `<error>` instead of key material is
    /// one the next send cannot reach. `stats()` is the surface a consumer
    /// polls, so the count has to be readable there and not only in a log.
    ///
    /// The non-406 half also pins the behavior decision: a code that does not
    /// claim the device is gone is counted and nothing else, so a server-side
    /// failure cannot trigger a device-list refresh storm.
    #[tokio::test]
    async fn a_rejected_device_is_counted_on_the_stats_snapshot() {
        for code in ["406", "503"] {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            let gone = Jid::pn_device("5511900000060", 0);

            let fetch = tokio::spawn({
                let client = client.clone();
                let jids = vec![gone.clone()];
                async move { client.fetch_and_establish_sessions(&jids).await }
            });

            answer_prekey_fetch(
                &client,
                &transport,
                vec![
                    NodeBuilder::new("user")
                        .attr("jid", NodeValue::Jid(gone.clone()))
                        .children([NodeBuilder::new("error")
                            .attr("code", code)
                            .attr("text", "not-acceptable")
                            .build()])
                        .build(),
                ],
            )
            .await;

            let established = fetch
                .await
                .expect("join")
                .expect("the fetch itself is fine");
            assert_eq!(established, 0, "a rejected device establishes no session");

            let stats = client.stats();
            assert_eq!(
                stats.devices_unkeyed_rejected, 1,
                "the {code} rejection must be counted"
            );
            assert_eq!(
                stats.devices_unkeyed_no_bundle, 0,
                "the rejection is why the bundle is missing; counting both reports one drop as two"
            );
            assert_eq!(stats.devices_unkeyed_total(), 1);
        }
    }

    /// A batch-wide 406 answers for every device at once and fails the send, so
    /// nothing goes on the wire. It is still counted, once per device it
    /// answered for: the counters measure keying attempts, and a metric that
    /// went quiet on the batch failure would be quietest exactly when keying is
    /// failing hardest.
    #[tokio::test]
    async fn a_batch_wide_refusal_counts_every_device_even_though_it_fails_the_send() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let first = Jid::pn_device("5511900000063", 0);
        let second = Jid::pn_device("5511900000064", 0);

        let fetch = tokio::spawn({
            let client = client.clone();
            let jids = vec![first, second];
            async move { client.fetch_and_establish_sessions(&jids).await }
        });

        let iq = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let request_id = iq
            .get()
            .attrs()
            .optional_string("id")
            .expect("the prekey fetch carries an id")
            .into_owned();
        let refusal = NodeBuilder::new("iq")
            .attr("id", request_id.as_str())
            .attr("type", "error")
            .children([NodeBuilder::new("error")
                .attr("code", "406")
                .attr("text", "not-acceptable")
                .build()])
            .build();
        crate::test_utils::answer_iq(&client, &request_id, &refusal).await;

        fetch
            .await
            .expect("join")
            .expect_err("a batch-wide 406 fails the send rather than shortening the fan-out");

        assert_eq!(
            client.stats().devices_unkeyed_rejected,
            2,
            "both devices the refusal answered for are counted"
        );
        assert_eq!(
            UnkeyableDevice::BatchRefused.label(),
            "refused_batch",
            "a batch refusal must not carry the label of a named rejection"
        );
        assert_eq!(client.stats().devices_unkeyed_no_bundle, 0);
    }

    /// A fetch that fails for anything other than a 406 leaves the same devices
    /// unkeyed and refuses none of them, so it gets its own reason instead of
    /// counting nothing.
    #[tokio::test]
    async fn a_fetch_that_never_answered_is_counted_apart_from_a_refusal() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let peer = Jid::pn_device("5511900000065", 0);

        let fetch = tokio::spawn({
            let client = client.clone();
            let jids = vec![peer];
            async move { client.fetch_and_establish_sessions(&jids).await }
        });

        let iq = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let request_id = iq
            .get()
            .attrs()
            .optional_string("id")
            .expect("the prekey fetch carries an id")
            .into_owned();
        let unavailable = NodeBuilder::new("iq")
            .attr("id", request_id.as_str())
            .attr("type", "error")
            .children([NodeBuilder::new("error")
                .attr("code", "503")
                .attr("text", "service-unavailable")
                .build()])
            .build();
        crate::test_utils::answer_iq(&client, &request_id, &unavailable).await;

        fetch
            .await
            .expect("join")
            .expect_err("a failed fetch still fails the establish");

        let stats = client.stats();
        assert_eq!(stats.devices_unkeyed_fetch_failed, 1);
        assert_eq!(
            stats.devices_unkeyed_rejected, 0,
            "an outage refuses nobody; calling it a rejection would send the \
             reader looking for a device that is gone"
        );
    }

    /// The other half of the same response: a device the server simply omits.
    /// It is ambiguous rather than condemned, and it is counted as such.
    #[tokio::test]
    async fn a_device_that_came_back_without_a_bundle_is_counted_on_the_snapshot() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let absent = Jid::pn_device("5511900000061", 0);

        let fetch = tokio::spawn({
            let client = client.clone();
            let jids = vec![absent.clone()];
            async move { client.fetch_and_establish_sessions(&jids).await }
        });

        answer_prekey_fetch(&client, &transport, Vec::new()).await;

        fetch
            .await
            .expect("join")
            .expect("an empty list is not an error");

        let stats = client.stats();
        assert_eq!(stats.devices_unkeyed_no_bundle, 1);
        assert_eq!(stats.devices_unkeyed_rejected, 0);
    }

    /// Nothing is counted when the device is keyed, so the counters measure
    /// breakage rather than traffic.
    #[tokio::test]
    async fn a_device_that_establishes_counts_nothing() {
        use wacore::iq::prekeys::PreKeyBundleUserNode;
        use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair, PreKeyBundle};
        use wacore::protocol::ProtocolNode;

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        assert_eq!(
            client.stats().devices_unkeyed_total(),
            0,
            "a fresh client has counted nothing"
        );
        let peer = Jid::pn_device("5511900000062", 0);

        // A bundle whose signed-prekey signature verifies, so X3DH completes.
        let mut rng = rand::make_rng::<StdRng>();
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let signature = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .expect("sign the signed prekey");
        let bundle = PreKeyBundle::new(
            1,
            0u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            signature,
            *receiver.identity_key(),
        )
        .expect("build the bundle");

        let fetch = tokio::spawn({
            let client = client.clone();
            let jids = vec![peer.clone()];
            async move { client.fetch_and_establish_sessions(&jids).await }
        });

        answer_prekey_fetch(
            &client,
            &transport,
            vec![
                PreKeyBundleUserNode::from_bundle(peer.clone(), &bundle, None)
                    .expect("build the user node")
                    .into_node(),
            ],
        )
        .await;

        let established = fetch.await.expect("join").expect("establish");
        assert_eq!(established, 1, "the session must actually be established");
        assert_eq!(
            client.stats().devices_unkeyed_total(),
            0,
            "a device that was keyed must not reach any of the counters"
        );
    }

    /// The 406 the preflight now tolerates is recognised by its server code and
    /// nothing else: any other failure must still fail the send, or a transport
    /// or auth error would be silently downgraded into "these devices have no
    /// prekeys" and the message would go out to fewer devices than it should.
    #[test]
    fn only_a_406_counts_as_an_unregistered_device() {
        // Both spellings, because they are both real: this preflight calls
        // `fetch_pre_keys` directly and receives the first, while the fan-out
        // goes through `SendContextResolver`, which re-wraps it as the second.
        let as_iq_error = |code| {
            anyhow::Error::new(crate::test_utils::server_error_iq(
                code,
                "not-acceptable",
                None,
                None,
            ))
        };
        let as_shared = |code| {
            anyhow::Error::new(wacore::request::ServerErrorCode {
                code,
                text: "not-acceptable".to_string(),
                error_type: None,
                backoff: None,
            })
        };

        assert!(
            is_device_unregistered(&as_iq_error(406)),
            "the error this preflight actually receives must be recognised"
        );
        assert!(is_device_unregistered(&as_shared(406)));

        for code in [400, 401, 403, 404, 429, 500, 503] {
            assert!(
                !is_device_unregistered(&as_iq_error(code)),
                "a {code} must not be treated as an unregistered device"
            );
            assert!(!is_device_unregistered(&as_shared(code)));
        }

        // Not every failure is a server error at all.
        assert!(!is_device_unregistered(&anyhow::anyhow!("socket closed")));
    }

    /// A prekey batch is several devices of the same one or two users, and each
    /// invalidation takes the registry lock and deletes rows, so the users are
    /// visited once each rather than once per device.
    ///
    /// Asserted on the list rather than through the cache: an entry invalidated
    /// twice is indistinguishable from one invalidated once, so a cache-level
    /// test would pass no matter how many times each user was visited.
    #[test]
    fn a_batch_names_each_user_once_in_order() {
        let a = Jid::pn("5511900000050");
        let b = Jid::pn("5511900000051");
        let jids = vec![
            a.with_device(0),
            a.with_device(1),
            b.with_device(0),
            a.with_device(2),
            b.with_device(3),
        ];

        assert_eq!(
            distinct_users(&jids).as_slice(),
            [a.user.as_str(), b.user.as_str()],
            "each user once, in the order the batch first names them"
        );

        assert!(distinct_users(&[]).is_empty());
        assert_eq!(
            distinct_users(std::slice::from_ref(&a.with_device(7))).as_slice(),
            [a.user.as_str()]
        );
    }

    #[test]
    fn test_primary_phone_jid_creation_from_pn() {
        let own_pn = Jid::pn("559999999999");
        let primary_phone_jid = own_pn.with_device(0);

        assert_eq!(primary_phone_jid.user, "559999999999");
        assert_eq!(primary_phone_jid.server, Server::Pn);
        assert_eq!(primary_phone_jid.device, 0);
        assert_eq!(primary_phone_jid.agent, 0);
        assert_eq!(primary_phone_jid.to_string(), "559999999999@s.whatsapp.net");
    }

    #[test]
    fn test_primary_phone_jid_overwrites_existing_device() {
        // Edge case: pn with device ID should still produce device 0
        let own_pn = Jid::pn_device("559999999999", 33);
        let primary_phone_jid = own_pn.with_device(0);

        assert_eq!(primary_phone_jid.user, "559999999999");
        assert_eq!(primary_phone_jid.server, Server::Pn);
        assert_eq!(primary_phone_jid.device, 0);
    }

    #[test]
    fn test_primary_phone_jid_is_not_ad() {
        let primary_phone_jid = Jid::pn("559999999999").with_device(0);
        assert!(!primary_phone_jid.is_ad()); // device 0 is NOT an additional device
    }

    #[test]
    fn test_linked_device_is_ad() {
        let linked_device_jid = Jid::pn_device("559999999999", 33);
        assert!(linked_device_jid.is_ad()); // device > 0 IS an additional device
    }

    #[test]
    fn test_primary_phone_jid_from_lid() {
        let own_lid = Jid::lid("100000000000001");
        let primary_phone_jid = own_lid.with_device(0);

        assert_eq!(primary_phone_jid.user, "100000000000001");
        assert_eq!(primary_phone_jid.server, Server::Lid);
        assert_eq!(primary_phone_jid.device, 0);
        assert!(!primary_phone_jid.is_ad());
    }

    #[test]
    fn test_primary_phone_jid_roundtrip() {
        let own_pn = Jid::pn("559999999999");
        let primary_phone_jid = own_pn.with_device(0);

        let jid_string = primary_phone_jid.to_string();
        assert_eq!(jid_string, "559999999999@s.whatsapp.net");

        let parsed: Jid = jid_string.parse().expect("JID should be parseable");
        assert_eq!(parsed.user, "559999999999");
        assert_eq!(parsed.server, Server::Pn);
        assert_eq!(parsed.device, 0);
    }

    #[test]
    fn test_with_device_preserves_identity() {
        let pn = Jid::pn("1234567890");
        let pn_device_0 = pn.with_device(0);
        let pn_device_5 = pn.with_device(5);

        assert_eq!(pn_device_0.user, pn_device_5.user);
        assert_eq!(pn_device_0.server, pn_device_5.server);
        assert_eq!(pn_device_0.device, 0);
        assert_eq!(pn_device_5.device, 5);

        let lid = Jid::lid("100000012345678");
        let lid_device_0 = lid.with_device(0);
        let lid_device_33 = lid.with_device(33);

        assert_eq!(lid_device_0.user, lid_device_33.user);
        assert_eq!(lid_device_0.server, lid_device_33.server);
        assert_eq!(lid_device_0.device, 0);
        assert_eq!(lid_device_33.device, 33);
    }

    #[test]
    fn test_primary_phone_vs_companion_devices() {
        let user = "559999999999";
        let primary = Jid::pn(user).with_device(0);
        let companion_web = Jid::pn_device(user, 33);
        let companion_desktop = Jid::pn_device(user, 34);

        // All share the same user
        assert_eq!(primary.user, companion_web.user);
        assert_eq!(primary.user, companion_desktop.user);

        // But have different device IDs
        assert_eq!(primary.device, 0);
        assert_eq!(companion_web.device, 33);
        assert_eq!(companion_desktop.device, 34);

        // Primary is NOT AD, companions ARE AD
        assert!(!primary.is_ad());
        assert!(companion_web.is_ad());
        assert!(companion_desktop.is_ad());
    }

    /// Session check must succeed before establishment (fail-safe behavior).
    #[test]
    fn test_session_check_behavior_documentation() {
        // Ok(true) -> skip, Ok(false) -> establish, Err -> fail-safe
        enum SessionCheckResult {
            Exists,
            NotExists,
            CheckFailed,
        }

        fn should_establish_session(
            check_result: SessionCheckResult,
        ) -> Result<bool, &'static str> {
            match check_result {
                SessionCheckResult::Exists => Ok(false),   // Don't establish
                SessionCheckResult::NotExists => Ok(true), // Do establish
                SessionCheckResult::CheckFailed => Err("Cannot verify - fail safe"),
            }
        }

        // Test cases
        assert_eq!(
            should_establish_session(SessionCheckResult::Exists),
            Ok(false)
        );
        assert_eq!(
            should_establish_session(SessionCheckResult::NotExists),
            Ok(true)
        );
        assert!(should_establish_session(SessionCheckResult::CheckFailed).is_err());
    }

    /// Protocol address format: {user}[:device]@{server}.0
    #[test]
    fn test_protocol_address_format_for_session_lookup() {
        use wacore::types::jid::JidExt;

        let pn = Jid::pn("559999999999").with_device(0);
        let addr = pn.to_protocol_address();
        assert_eq!(addr.name(), "559999999999@c.us");
        assert_eq!(u32::from(addr.device_id()), 0);
        assert_eq!(addr.to_string(), "559999999999@c.us.0");

        let companion = Jid::pn_device("559999999999", 33);
        let companion_addr = companion.to_protocol_address();
        assert_eq!(companion_addr.name(), "559999999999:33@c.us");
        assert_eq!(companion_addr.to_string(), "559999999999:33@c.us.0");

        let lid = Jid::lid("100000000000001").with_device(0);
        let lid_addr = lid.to_protocol_address();
        assert_eq!(lid_addr.name(), "100000000000001@lid");
        assert_eq!(u32::from(lid_addr.device_id()), 0);
        assert_eq!(lid_addr.to_string(), "100000000000001@lid.0");

        let lid_device = Jid::lid_device("100000000000001", 33);
        let lid_device_addr = lid_device.to_protocol_address();
        assert_eq!(lid_device_addr.name(), "100000000000001:33@lid");
        assert_eq!(lid_device_addr.to_string(), "100000000000001:33@lid.0");
    }

    #[test]
    fn test_filter_logic_for_session_establishment() {
        let jids = vec![
            Jid::pn_device("111", 0),
            Jid::pn_device("222", 0),
            Jid::pn_device("333", 0),
        ];

        // Simulate contains_session results
        let session_exists = |jid: &Jid| -> Result<bool, &'static str> {
            match jid.user.as_str() {
                "111" => Ok(true),        // Session exists
                "222" => Ok(false),       // No session
                "333" => Err("DB error"), // Error
                _ => Ok(false),
            }
        };

        // Apply filter logic (matching ensure_e2e_sessions behavior)
        let mut jids_needing_sessions = Vec::with_capacity(jids.len());
        for jid in &jids {
            match session_exists(jid) {
                Ok(true) => {}                                        // Skip - session exists
                Ok(false) => jids_needing_sessions.push(jid.clone()), // Needs session
                Err(e) => eprintln!("Warning: failed to check {}: {}", jid, e), // Skip on error
            }
        }

        // Only "222" should need a session
        assert_eq!(jids_needing_sessions.len(), 1);
        assert_eq!(jids_needing_sessions[0].user, "222");
    }

    // PN and LID have independent Signal sessions

    #[test]
    fn test_dual_addressing_pn_and_lid_are_independent() {
        let pn_address = Jid::pn("551199887766").with_device(0);
        let lid_address = Jid::lid("236395184570386").with_device(0);

        assert_ne!(pn_address.user, lid_address.user);
        assert_ne!(pn_address.server, lid_address.server);

        use wacore::types::jid::JidExt;
        let pn_signal_addr = pn_address.to_protocol_address();
        let lid_signal_addr = lid_address.to_protocol_address();

        assert_ne!(pn_signal_addr.name(), lid_signal_addr.name());
        assert_eq!(pn_signal_addr.name(), "551199887766@c.us");
        assert_eq!(lid_signal_addr.name(), "236395184570386@lid");
        assert_eq!(pn_address.device, 0);
        assert_eq!(lid_address.device, 0);
    }

    #[test]
    fn test_lid_extraction_from_own_device() {
        let own_lid_with_device = Jid::lid_device("236395184570386", 61);
        let primary_lid = own_lid_with_device.with_device(0);

        assert_eq!(primary_lid.user, "236395184570386");
        assert_eq!(primary_lid.device, 0);
        assert!(!primary_lid.is_ad());
    }

    /// PN sessions established proactively, LID sessions established by primary phone.
    #[test]
    fn test_stale_session_scenario_documentation() {
        fn should_establish_pn_session(pn_exists: bool) -> bool {
            !pn_exists
        }

        fn should_establish_lid_session(_lid_exists: bool) -> bool {
            false // Primary phone establishes LID sessions via pkmsg
        }

        // PN exists -> don't establish
        assert!(!should_establish_pn_session(true));
        // PN doesn't exist -> establish
        assert!(should_establish_pn_session(false));
        // LID never established proactively
        assert!(!should_establish_lid_session(true));
        assert!(!should_establish_lid_session(false));
    }

    /// Retry mechanism: error=1 (NoSession), error=4 (InvalidMessage/MAC failure)
    #[test]
    fn test_retry_mechanism_for_stale_sessions() {
        const RETRY_ERROR_NO_SESSION: u8 = 1;
        const RETRY_ERROR_INVALID_MESSAGE: u8 = 4;

        fn action_for_error(error_code: u8) -> &'static str {
            match error_code {
                RETRY_ERROR_NO_SESSION => "Establish new session via prekey",
                RETRY_ERROR_INVALID_MESSAGE => "Delete stale session, resend message",
                _ => "Unknown error",
            }
        }

        assert_eq!(
            action_for_error(RETRY_ERROR_NO_SESSION),
            "Establish new session via prekey"
        );
        assert_eq!(
            action_for_error(RETRY_ERROR_INVALID_MESSAGE),
            "Delete stale session, resend message"
        );
    }

    #[test]
    fn test_session_establishment_lookup_normalization() {
        use std::collections::HashMap;
        use wacore_binary::Jid;

        // Represents the bundle map returned by fetch_pre_keys
        // (keys are normalized by parsing logic as verified in wacore/src/prekeys.rs)
        let mut prekey_bundles: HashMap<Jid, ()> = HashMap::new(); // Using () as mock bundle placeholder

        let normalized_jid = Jid::lid("123456789"); // agent=0
        prekey_bundles.insert(normalized_jid.clone(), ());

        // Represents the JID from the device list (e.g. from ensure_e2e_sessions)
        // which might have agent=1 due to some upstream source or parsing quirk
        let mut requested_jid = Jid::lid("123456789");
        requested_jid.agent = 1;

        // The agent is inert on a LID, so it does not hide the bundle: the raw
        // lookup finds it. Normalising the key first was the workaround this
        // replaced, and the helper that did it is gone.
        assert!(
            prekey_bundles.contains_key(&requested_jid),
            "an inert agent must not hide the bundle"
        );
        assert_eq!(requested_jid, normalized_jid);
    }
}
