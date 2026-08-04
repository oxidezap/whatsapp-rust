//! App-state collection sync and mutation dispatch.

use super::*;
use crate::request::DEFAULT_IQ_TIMEOUT;

/// Concurrency cap for pre-downloading app-state external blobs (independent CDN
/// GETs, keyed by directPath — LTHash ordering is in patch application, not blob
/// fetching). WA Web fans these out under `Promise.all` (`Syncd/CollectionHandler`);
/// bounded here because a snapshot can be multi-MB and a batch carries several.
const APPSTATE_BLOB_DOWNLOAD_CONCURRENCY: usize = 4;
const APP_STATE_KEY_REQUEST_DEDUP: Duration = Duration::from_secs(24 * 3600);
const APP_STATE_KEY_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const APP_STATE_KEY_PARTIAL_RETRY: Duration = Duration::from_secs(10);
const APP_STATE_KEY_RETRY_MAX: Duration = Duration::from_secs(60);
/// How many times an outgoing patch is rebuilt against a newer base before the
/// send gives up. WA Web's `serverSync` runs the same resolve-and-retry loop
/// with `y = 5` (`WAWebSyncdServerSync`).
const APP_STATE_PATCH_SEND_ATTEMPTS: usize = 5;
/// How long a sync waits for another writer to release a collection.
///
/// A holder's honest worst case is not derivable: a patch send keeps the
/// reservation across `fetch_app_state_with_retry_inner`, which may page up to
/// `MAX_PAGINATION_ITERATIONS` IQs, so any bound short enough to be useful can
/// expire on healthy work. What makes the bound safe is not its size but that
/// running out is never lossy — the collection comes back `retryable` and the
/// retry scheduler picks it up. This value covers the common case
/// ([`APP_STATE_PATCH_SEND_ATTEMPTS`] attempts of [`DEFAULT_IQ_TIMEOUT`], plus
/// one) so the scheduler is the exception rather than the rule. The bound exists
/// because the sync worker's intake loop runs non-history tasks inline, so a
/// reservation that never releases would stall everything queued behind it.
const APP_STATE_RESERVATION_WAIT: Duration =
    Duration::from_secs(DEFAULT_IQ_TIMEOUT.as_secs() * (APP_STATE_PATCH_SEND_ATTEMPTS as u64 + 1));
/// Spacing between re-syncs of a collection a run left retryable, mirroring the
/// syncd backoff WA Web applies to exactly this case (`WASyncdConst`:
/// `BACKOFF_MIN_TIMEOUT` 1s, `BACKOFF_BASE` 2, `BACKOFF_MAX_TIMEOUT` 1h).
const APP_STATE_RETRY_BACKOFF_MIN: Duration = Duration::from_secs(1);
const APP_STATE_RETRY_BACKOFF_MAX: Duration = Duration::from_secs(60 * 60);
/// How many spaced attempts one connection makes before leaving the collection
/// to the next sync trigger. WA Web keeps retrying against a persisted
/// first-failure timestamp and only gives up after two days; without that column
/// this is what a single connection can promise, and the doubling already puts
/// the last wait minutes out.
const APP_STATE_RETRY_MAX_ROUNDS: u32 = 8;

/// Delay before retry `round` (zero-based), doubling from
/// [`APP_STATE_RETRY_BACKOFF_MIN`] and clamped at
/// [`APP_STATE_RETRY_BACKOFF_MAX`].
fn app_state_retry_backoff(round: u32) -> Duration {
    APP_STATE_RETRY_BACKOFF_MIN
        .saturating_mul(2u32.saturating_pow(round))
        .min(APP_STATE_RETRY_BACKOFF_MAX)
}

/// The connection a piece of app-state work belongs to, and the clock it runs
/// against.
///
/// Every sync path awaits round trips, and between them the socket can retire
/// or the bootstrap's watchdog can fire. Checking that by hand at each await
/// boundary is what this replaces: the checks drifted apart, some paths grew a
/// check the next one forgot, and the same load answered two different
/// questions. A scope makes the question single — [`Client::admits`] — and the
/// answer typed, so a new boundary that forgets to ask is a boundary that
/// cannot compile against the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SyncScope {
    /// The connection this work was started for.
    generation: u64,
    /// When the work stops being worth doing. Only the initial bootstrap sets
    /// one, because only it runs under a watchdog that reconnects underneath.
    deadline: Option<wacore::time::Instant>,
}

/// Why a scope no longer admits its work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeLost {
    /// The connection was replaced. Anything this work would publish or persist
    /// belongs to a socket that is gone.
    Retired,
    /// The deadline passed. The watchdog either has reconnected or is about to,
    /// so finishing would race it.
    Expired,
}

impl SyncScope {
    /// The generation this scope is pinned to. Used by logs and by tests that
    /// need to retire a connection out from under a scope.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn generation(self) -> u64 {
        self.generation
    }

    /// How long the work has left, or `None` when it is not on a clock.
    pub(crate) fn remaining(self) -> Option<Duration> {
        self.deadline
            .map(|d| d.saturating_duration_since(wacore::time::Instant::now()))
    }

    /// Whether this scope is bound to a deadline at all, which is what
    /// distinguishes the bootstrap from every background trigger.
    pub(crate) fn is_bootstrap(self) -> bool {
        self.deadline.is_some()
    }

    /// Move to the live connection, for work that must outlive a reconnect.
    ///
    /// Returns whether it moved. The caller decides what that costs: an outcome
    /// computed for the old socket must not be published, and a retry that
    /// rebinds can no longer settle the bootstrap it was scheduled by.
    pub(crate) fn rebind(&mut self, to: u64) -> bool {
        let moved = self.generation != to;
        self.generation = to;
        moved
    }
}

/// The initial-bootstrap flag, tagged with the connection that last wrote it.
///
/// A plain flag cannot be settled safely. Deciding whether the writer still owns
/// the connection and then writing are two operations, and every attempt to
/// bridge them failed a different way: checking first missed a retirement in the
/// gap, and rolling back afterwards clobbered whatever the replacement had
/// written in the meantime. Packing the generation into the same word makes the
/// pair a single compare-and-swap, so a writer from a retired connection simply
/// loses — there is no window left to lose in.
#[derive(Debug)]
pub(crate) struct BootstrapGate(AtomicU64);

impl BootstrapGate {
    /// Armed by pairing before any connection exists, so generation zero owns
    /// the first write and every later connection outranks it.
    pub(crate) fn new(outstanding: bool) -> Self {
        Self(AtomicU64::new(Self::encode(0, outstanding)))
    }

    const fn encode(generation: u64, outstanding: bool) -> u64 {
        (generation << 1) | outstanding as u64
    }

    /// Whether the bootstrap still owes work, whoever last said so.
    pub(crate) fn is_armed(&self) -> bool {
        self.0.load(Ordering::Acquire) & 1 == 1
    }

    /// Arm unconditionally, for pairing: there is no scope yet, and a fresh
    /// pairing owes a bootstrap no matter what any connection thinks.
    pub(crate) fn arm_for_pairing(&self) {
        self.0
            .store(Self::encode(u64::MAX >> 1, true), Ordering::Release);
    }

    /// Record `outstanding` on behalf of `generation`, unless a newer connection
    /// has already had its say.
    ///
    /// Returns whether the write took. A stale writer losing here is the point,
    /// not a failure.
    pub(crate) fn settle(&self, generation: u64, outstanding: bool) -> bool {
        let mut current = self.0.load(Ordering::Acquire);
        loop {
            // Strictly newer wins. Equal is the same connection revising its own
            // answer, which it is entitled to do.
            if (current >> 1) > generation {
                return false;
            }
            match self.0.compare_exchange_weak(
                current,
                Self::encode(generation, outstanding),
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return true,
                Err(observed) => current = observed,
            }
        }
    }
}

/// What kind of work holds a collection's reservation.
///
/// Skipping behind a holder is only sound when that holder is doing the same
/// fetch. A patch send takes the same reservation and never fetches, so a sync
/// that skipped behind one would silently drop the work its caller asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncHolder {
    /// A collection sync: fetches from the server, then writes the collection's
    /// version and mutation MACs.
    Sync,
    /// A patch send: writes the same rows, but never fetches.
    PatchSend,
}

/// Why a sync did not get the collection reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservationSkip {
    /// An equivalent sync already holds it, so it is already doing this work.
    EquivalentSyncInFlight,
    /// The holder did not release within the bound.
    WaitTimedOut,
}

/// What finishing a sync, or the retries it schedules, settles beyond the
/// collections themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SyncSettles {
    /// Only the collections. Every trigger except the initial bootstrap: a
    /// `server_sync` or a dirty bit says nothing about whether the first full
    /// sync ever finished.
    JustTheCollections,
    /// The initial bootstrap too. Its gate stays armed while anything it asked
    /// for is still outstanding, so recovering the last of them — even rounds
    /// later — is what stands it down.
    InitialSync,
}

/// What a sync should do when the collection is already reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReservationWait {
    /// Skip when an equivalent sync holds it. The batched sync asks for whatever
    /// the server has for a set of collections, so another sync of the same
    /// collection does this call's work and dropping it costs nothing.
    SkipBehindSync,
    /// Wait for whoever holds it. A consumer asking for one specific collection
    /// is not made redundant by a sync already in flight: a full sync asks for
    /// the snapshot while an incremental one asks for patches after the
    /// persisted version, so skipping would turn the request into a no-op.
    Always,
}

/// What a batched sync achieved, collection by collection.
///
/// A single `Ok`/`Err` for the whole batch cannot carry this. The initial
/// bootstrap must not read "one collection was refused" as permission to
/// dispatch Connected, while a background `server_sync` only wants to log it,
/// and both read the same return value. So the call reports what happened and
/// each caller decides what a partial result means for it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct BatchedSyncOutcome {
    /// Applied and persisted.
    pub(crate) synced: Vec<WAPatchName>,
    /// The server refused the collection outright (400/404). Repeating the same
    /// request gets the same answer, so retrying on its own never clears this.
    pub(crate) fatal: Vec<WAPatchName>,
    /// Did not sync, but a later attempt can: a retryable server error, a decode
    /// key that never landed, or the iteration cap.
    pub(crate) retryable: Vec<WAPatchName>,
    /// Another holder had it reserved, so this call did nothing for it.
    pub(crate) skipped: Vec<WAPatchName>,
}

impl BatchedSyncOutcome {
    /// Every collection this call did not leave synced, whatever the reason.
    pub(crate) fn unsynced(&self) -> impl Iterator<Item = WAPatchName> + '_ {
        self.fatal
            .iter()
            .chain(&self.retryable)
            .chain(&self.skipped)
            .copied()
    }

    /// True when every collection asked for came back synced.
    pub(crate) fn all_synced(&self) -> bool {
        self.unsynced().next().is_none()
    }
}

/// In-flight dedup registry for app-state collection syncs.
///
/// Reservations carry a per-begin token so a release can only ever remove the
/// reservation it belongs to: a stale task finishing after a reconnect cleared
/// the registry cannot evict the newer generation's reservation for the same
/// collection. Releases run from the guard's `Drop`, so a cancelled sync
/// (timeout, abort, teardown) can never strand a collection as "in flight".
/// The mutex is synchronous and never held across an await.
pub(crate) struct SyncInFlight {
    entries: std::sync::Mutex<HashMap<WAPatchName, (u64, SyncHolder)>>,
    next_token: AtomicU64,
    /// Notified whenever a reservation is released, so [`SyncInFlight::begin`]
    /// can wait for one instead of spinning.
    released: event_listener::Event,
}

impl SyncInFlight {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: std::sync::Mutex::new(HashMap::new()),
            next_token: AtomicU64::new(0),
            released: event_listener::Event::new(),
        })
    }

    /// Reserve `name` for `holder`, or report what already holds it.
    pub(crate) fn try_begin_as(
        self: &Arc<Self>,
        name: WAPatchName,
        holder: SyncHolder,
    ) -> Result<SyncInFlightGuard, SyncHolder> {
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(&(_, current)) = entries.get(&name) {
            return Err(current);
        }
        entries.insert(name, (token, holder));
        Ok(SyncInFlightGuard {
            registry: Arc::clone(self),
            name,
            token,
        })
    }

    /// Reserve `name` for a sync, or `None` when anything already holds it.
    ///
    /// Test-only: production callers go through
    /// [`Client::reserve_for_sync`](crate::client::Client::reserve_for_sync),
    /// which has to tell an equivalent sync apart from a patch send. Keeping the
    /// shorthand here lets the registry's own tests stay about the token and
    /// wake-up rules rather than repeating a holder kind they do not exercise.
    #[cfg(test)]
    pub(crate) fn try_begin(self: &Arc<Self>, name: WAPatchName) -> Option<SyncInFlightGuard> {
        self.try_begin_as(name, SyncHolder::Sync).ok()
    }

    /// Reserve `name`, waiting for the current holder to finish.
    ///
    /// A patch send cannot skip: it must not write the collection's version and
    /// mutation MACs while a sync is writing them, and it needs the base a
    /// concurrent sync is about to move. Cancelling this future simply stops
    /// waiting; nothing is reserved until the guard is returned.
    pub(crate) async fn begin(
        self: &Arc<Self>,
        name: WAPatchName,
        holder: SyncHolder,
    ) -> SyncInFlightGuard {
        loop {
            // Register the listener before re-checking, so a release landing
            // between the check and the wait cannot be missed.
            let released = self.released.listen();
            if let Ok(guard) = self.try_begin_as(name, holder) {
                return guard;
            }
            released.await;
        }
    }

    /// Drop every reservation, releasing backing storage. Guards from before
    /// the clear become no-ops thanks to the token check.
    pub(crate) fn clear(&self) {
        *self.entries.lock().unwrap_or_else(|p| p.into_inner()) = HashMap::new();
        self.released.notify(usize::MAX);
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

pub(crate) struct SyncInFlightGuard {
    registry: Arc<SyncInFlight>,
    name: WAPatchName,
    token: u64,
}

impl Drop for SyncInFlightGuard {
    fn drop(&mut self) {
        let mut entries = self
            .registry
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if entries
            .get(&self.name)
            .is_some_and(|&(t, _)| t == self.token)
        {
            entries.remove(&self.name);
        }
        drop(entries);
        // Waiters are keyed by nothing, so wake all of them and let each
        // re-check its own collection.
        self.registry.released.notify(usize::MAX);
    }
}

fn initial_app_state_key_retry(timeout: Duration) -> Duration {
    (timeout / 2)
        .max(Duration::from_millis(1))
        .min(APP_STATE_KEY_PARTIAL_RETRY)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppStateKeyRequestDelivery {
    AllPeers,
    SomePeers,
}

struct AppStateKeyRequestSchedule {
    retry_at: wacore::time::Instant,
    sent: bool,
}

enum AppStateKeyRequestProgress {
    Scheduled(AppStateKeyRequestSchedule),
    KeysReady,
    TimedOut,
}

#[cold]
#[inline(never)]
fn classify_app_state_key_request_failures(
    peer_count: usize,
    failure_count: usize,
    failures: &str,
) -> Result<AppStateKeyRequestDelivery, anyhow::Error> {
    if failure_count == peer_count {
        return Err(anyhow::anyhow!(
            "app-state key request failed for all {peer_count} peer device(s): {failures}"
        ));
    }
    warn!(
        "App-state key request failed for {failure_count}/{peer_count} peer device(s): {failures}"
    );
    Ok(AppStateKeyRequestDelivery::SomePeers)
}

#[cold]
#[inline(never)]
fn append_app_state_key_request_failure(
    failures: &mut Option<String>,
    message: std::fmt::Arguments<'_>,
) {
    let failures = failures.get_or_insert_with(String::new);
    if !failures.is_empty() {
        failures.push_str(", ");
    }
    let _ = std::fmt::Write::write_fmt(failures, message);
}

async fn collect_app_state_key_request_results<F, E>(
    runtime: &dyn Runtime,
    mut requests: futures::stream::FuturesUnordered<F>,
    timeout: Duration,
) -> Result<AppStateKeyRequestDelivery, anyhow::Error>
where
    F: Future<Output = (u16, std::result::Result<(), E>)>,
    E: std::fmt::Display,
{
    use futures::StreamExt;
    use futures::future::Either;

    let peer_count = requests.len();
    let mut failure_count = 0;
    let mut failures = None;
    let mut deadline = runtime.sleep(timeout);
    while !requests.is_empty() {
        match futures::future::select(requests.next(), deadline.as_mut()).await {
            Either::Left((Some((device, result)), _)) => {
                if let Err(error) = result {
                    failure_count += 1;
                    append_app_state_key_request_failure(
                        &mut failures,
                        format_args!("device {device}: {error}"),
                    );
                }
            }
            Either::Left((None, _)) => break,
            Either::Right(((), _)) => {
                let timed_out = requests.len();
                failure_count += timed_out;
                append_app_state_key_request_failure(
                    &mut failures,
                    format_args!("{timed_out} peer request(s) timed out"),
                );
                break;
            }
        }
    }

    if failure_count != 0 {
        return classify_app_state_key_request_failures(
            peer_count,
            failure_count,
            failures.as_deref().unwrap_or_default(),
        );
    }
    Ok(AppStateKeyRequestDelivery::AllPeers)
}

async fn app_state_keys_available(
    backend: &dyn crate::store::traits::Backend,
    key_ids: &[Vec<u8>],
) -> bool {
    for key_id in key_ids {
        if backend.get_sync_key(key_id).await.ok().flatten().is_none() {
            return false;
        }
    }
    true
}

async fn remove_available_app_state_keys(
    backend: &dyn crate::store::traits::Backend,
    missing: &mut Vec<Vec<u8>>,
) {
    let mut index = 0;
    while index < missing.len() {
        if backend
            .get_sync_key(&missing[index])
            .await
            .ok()
            .flatten()
            .is_some()
        {
            missing.swap_remove(index);
        } else {
            index += 1;
        }
    }
}

fn finalize_app_state_key_request_peers(
    mut peers: Vec<Jid>,
    current_device: u16,
    primary: Jid,
) -> Result<Vec<Jid>, anyhow::Error> {
    // WA Web derives every sibling address from the account's PN namespace.
    for peer in &mut peers {
        peer.user.clone_from(&primary.user);
        peer.server = primary.server;
        peer.agent = primary.agent;
        peer.integrator = primary.integrator;
    }
    peers.retain(|jid| jid.device != current_device);
    wacore::types::jid::sort_dedup_by_device(&mut peers);
    if peers.is_empty() && current_device != primary.device {
        peers.push(primary);
    }
    if peers.is_empty() {
        return Err(anyhow::anyhow!(
            "no peer devices available for app-state key request"
        ));
    }
    Ok(peers)
}

impl Client {
    pub(crate) async fn get_app_state_processor(&self) -> Arc<AppStateProcessor> {
        let mut guard = self.app_state_processor.lock().await;
        if let Some(proc) = guard.as_ref() {
            return proc.clone();
        }
        debug!("Initializing AppStateProcessor for the first time.");
        let proc = Arc::new(AppStateProcessor::new(
            self.persistence_manager.backend(),
            self.runtime.clone(),
        ));
        *guard = Some(proc.clone());
        proc
    }

    /// Pre-download every external blob (snapshots + patch external mutations)
    /// referenced by `patch_lists`, keyed by directPath, fetching concurrently
    /// (bounded by [`APPSTATE_BLOB_DOWNLOAD_CONCURRENCY`]). A failed download is
    /// logged and omitted; the later inline step surfaces the missing blob as
    /// before. Mirrors WA Web's parallel syncd blob fetch.
    async fn pre_download_external_blobs(
        &self,
        patch_lists: &[wacore::appstate::patch_decode::PatchList],
    ) -> HashMap<String, Vec<u8>> {
        use futures::StreamExt;

        // Kept only so a failed download logs the right message (snapshot vs patch).
        enum BlobKind {
            Snapshot(WAPatchName),
            Mutation(u64),
        }

        // Clone the (small) blob ref into each job so the task owns its input and
        // captures only `&self` (keeps the future Send); the directPath is
        // recovered from the moved `ext` after the fetch. Dedup by directPath so
        // patches sharing a blob don't fetch it twice into the same map key.
        let mut jobs: Vec<(wa::ExternalBlobReference, BlobKind)> = Vec::new();
        let mut seen_paths: HashSet<&str> = HashSet::new();
        for pl in patch_lists {
            if let Some(ext) = &pl.snapshot_ref
                && let Some(path) = ext.direct_path.as_deref()
                && seen_paths.insert(path)
            {
                jobs.push((ext.clone(), BlobKind::Snapshot(pl.name)));
            }
            for patch in &pl.patches {
                if let Some(ext) = patch.external_mutations.as_option()
                    && let Some(path) = ext.direct_path.as_deref()
                    && seen_paths.insert(path)
                {
                    let v = patch
                        .version
                        .as_option()
                        .and_then(|v| v.version)
                        .unwrap_or(0);
                    jobs.push((ext.clone(), BlobKind::Mutation(v)));
                }
            }
        }

        if jobs.is_empty() {
            return HashMap::new();
        }

        let mut pre_downloaded = HashMap::with_capacity(jobs.len());
        let results = futures::stream::iter(jobs.into_iter().map(|(ext, kind)| async move {
            let bytes = self.download(&ext).await;
            // directPath presence was checked when the job was built.
            (ext.direct_path, kind, bytes)
        }))
        .buffer_unordered(APPSTATE_BLOB_DOWNLOAD_CONCURRENCY)
        .collect::<Vec<_>>()
        .await;

        for (path, kind, res) in results {
            match res {
                Ok(bytes) => {
                    if let BlobKind::Mutation(v) = kind {
                        debug!(target: "Client/AppState", "Downloaded external mutations for patch v{} ({} bytes)", v, bytes.len());
                    } else {
                        debug!(target: "Client/AppState", "Downloaded external snapshot ({} bytes)", bytes.len());
                    }
                    if let Some(path) = path {
                        pre_downloaded.insert(path, bytes);
                    }
                }
                Err(e) => match kind {
                    BlobKind::Snapshot(name) => {
                        warn!("Failed to download external snapshot for {:?}: {e}", name)
                    }
                    BlobKind::Mutation(v) => {
                        warn!(
                            "Failed to download external mutations for patch v{}: {e}",
                            v
                        )
                    }
                },
            }
        }

        pre_downloaded
    }

    pub(crate) fn start_sync_task_worker(
        self: &Arc<Self>,
        receiver: async_channel::Receiver<MajorSyncTask>,
    ) {
        const HISTORY_SYNC_CONCURRENCY: usize = 2;

        let worker_client = Arc::downgrade(self);
        let history_permits = Arc::new(async_lock::Semaphore::new(HISTORY_SYNC_CONCURRENCY));
        self.runtime
            .spawn(Box::pin(async move {
                while let Ok(task) = receiver.recv().await {
                    let Some(worker_client) = worker_client.upgrade() else {
                        break;
                    };

                    if matches!(task, MajorSyncTask::HistorySync { .. }) {
                        let permit = history_permits.acquire_arc().await;
                        let task_client = worker_client.clone();
                        worker_client
                            .runtime
                            .spawn(Box::pin(async move {
                                let _permit = permit;
                                task_client.process_sync_task(task).await;
                            }))
                            .detach();
                    } else {
                        worker_client.process_sync_task(task).await;
                    }
                }
                info!(
                    "Sync worker intake loop finished (detached history-sync tasks may still be running)."
                );
            }))
            .detach();
    }

    /// Public entry point for processing [`MajorSyncTask`] from the sync channel.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.appstate.sync_task", level = "debug", skip_all)
    )]
    pub async fn process_sync_task(self: &Arc<Self>, task: MajorSyncTask) {
        match task {
            MajorSyncTask::HistorySync {
                message_id,
                notification,
                mut tracker,
            } => {
                self.process_history_sync_task_tracked(message_id, *notification, &mut tracker)
                    .await;
            }
            MajorSyncTask::AppStateSync { name, full_sync } => {
                // Reserve the collection like every other sync path does.
                // Unreserved, this writes the same version and mutation-MAC
                // rows as a concurrent `sync_collections_batched`, leaving the
                // ltHash disagreeing with the MAC store.
                //
                // Waits for any holder: this is a consumer asking for one named
                // collection, and a sync already in flight is not necessarily
                // fetching what it asked for.
                let _guard = match self
                    .reserve_for_sync(name, ReservationWait::Always, self.sync_scope(None))
                    .await
                {
                    Ok(guard) => guard,
                    Err(ReservationSkip::EquivalentSyncInFlight) => {
                        debug!(target: "Client/AppState", "Skipping app state sync task {name:?}: an equivalent sync holds it");
                        return;
                    }
                    // The bound ran out, so nobody is covering this collection
                    // and the consumer asked for it. Retried as the task it was,
                    // not as a plain collection sync: the batched path asks for a
                    // snapshot only when the persisted version is zero, so
                    // rescheduling a `full_sync` request that way would quietly
                    // downgrade it to incremental and the snapshot would never
                    // happen.
                    Err(ReservationSkip::WaitTimedOut) => {
                        warn!(target: "Client/AppState", "Gave up waiting to sync {name:?}; scheduling a retry");
                        self.schedule_app_state_task_retry(name, full_sync);
                        return;
                    }
                };
                if let Err(e) = self.process_app_state_sync_task(name, full_sync).await {
                    log::warn!("App state sync task for {name:?} failed: {e}");
                }
            }
        }
    }

    /// Sync one collection, retrying a missing decode key and a locked DB.
    ///
    /// Takes no in-flight reservation of its own: the only caller is the patch
    /// send, which already holds the collection's reservation for the whole
    /// build-send-resolve cycle and would deadlock on its own guard. The
    /// batched path reserves its collections in
    /// [`sync_collections_batched`](Self::sync_collections_batched).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.fetch", level = "debug", skip_all, fields(name = ?name), err(Debug)))]
    async fn fetch_app_state_with_retry_inner(&self, name: WAPatchName) -> Result<()> {
        let _t = wacore::telemetry::timer(wacore::telemetry::APPSTATE_SYNC_DURATION);
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            // full_sync=false lets process_app_state_sync_task auto-detect:
            // version 0 → snapshot (full sync), version > 0 → incremental patches.
            // Matches WA Web which only requests snapshot when version is undefined.
            let res = self.process_app_state_sync_task(name, false).await;
            match res {
                Ok(()) => {
                    wacore::telemetry::appstate_sync("ok");
                    return Ok(());
                }
                Err(e) => {
                    if e.downcast_ref::<crate::appstate_sync::AppStateSyncError>()
                        .is_some_and(|ase| {
                            matches!(ase, crate::appstate_sync::AppStateSyncError::KeyNotFound(_))
                        })
                        && attempt == 1
                    {
                        if !self.initial_app_state_keys_received.load(Ordering::Relaxed) {
                            debug!(target: "Client/AppState", "App state key missing for {:?}; waiting up to 10s for key share then retrying", name);
                            if rt_timeout(
                                &*self.runtime,
                                Duration::from_secs(10),
                                self.initial_keys_synced_notifier.listen(),
                            )
                            .await
                            .is_err()
                            {
                                warn!(target: "Client/AppState", "Timeout waiting for key share for {:?}; retrying anyway", name);
                            }
                        }
                        continue;
                    }
                    let is_db_locked = e
                        .downcast_ref::<wacore::store::error::StoreError>()
                        .is_some_and(|se| se.is_database_busy_or_locked())
                        || e.downcast_ref::<crate::appstate_sync::AppStateSyncError>()
                            .is_some_and(|ase| match ase {
                                crate::appstate_sync::AppStateSyncError::Store(se) => {
                                    se.is_database_busy_or_locked()
                                }
                                _ => false,
                            });
                    if is_db_locked && attempt < APP_STATE_RETRY_MAX_ATTEMPTS {
                        let backoff = Duration::from_millis(200 * attempt as u64 + 150);
                        warn!(target: "Client/AppState", "Attempt {} for {:?} failed due to locked DB; backing off {:?} and retrying", attempt, name, backoff);
                        self.runtime.sleep(backoff).await;
                        continue;
                    }
                    wacore::telemetry::appstate_sync("fail");
                    return Err(e);
                }
            }
        }
    }

    /// Log and surface a sync whose caller has no decision to make.
    ///
    /// The initial bootstrap is the only path that changes what it does based on
    /// the outcome. Every other caller just needs an incomplete sync to be
    /// visible instead of swallowed, which is how a collection could stop
    /// syncing without anything saying so.
    ///
    /// Readiness is read, not assumed: a `syncd_app_state` dirty bit can start a
    /// sync while offline stanzas are still being processed, so this can run
    /// before the connection ever reaches `Connected`.
    ///
    /// `generation` is the connection the sync was started on. Checking it here
    /// rather than at each call site is deliberate: every caller awaits a round
    /// trip before reporting, and one that forgot would publish a retired
    /// socket's refusal to a consumer whose documented response is to log out or
    /// force a recovery — on the live session.
    ///
    /// `requested` is what the sync was asked to cover. It is only needed for
    /// the failure arm: a top-level error produces no outcome at all, so there
    /// are no per-collection buckets to retry from, and for a dirty-bit request
    /// the trigger is already consumed — nothing would ask again.
    pub(crate) fn report_background_sync(
        self: &Arc<Self>,
        label: &str,
        scope: SyncScope,
        settles: SyncSettles,
        requested: &[WAPatchName],
        result: Result<BatchedSyncOutcome>,
    ) {
        self.report_background_sync_stranded(label, scope, settles, requested, false, result)
    }

    /// [`report_background_sync`](Self::report_background_sync) for a caller that
    /// already knows something is unrecoverable outside this result.
    ///
    /// A collection the server refused is not in `requested` — retrying it is
    /// pointless — but it is still why the bootstrap is unfinished, and a later
    /// clean round would otherwise settle the gate on its behalf.
    pub(crate) fn report_background_sync_stranded(
        self: &Arc<Self>,
        label: &str,
        scope: SyncScope,
        settles: SyncSettles,
        requested: &[WAPatchName],
        stranded_elsewhere: bool,
        result: Result<BatchedSyncOutcome>,
    ) {
        if let Err(lost) = self.admits(scope) {
            debug!(target: "Client/AppState", "{label}: outcome dropped ({lost:?})");
            return;
        }
        match result {
            Ok(outcome) if outcome.all_synced() => {}
            Ok(outcome) => {
                warn!(
                    target: "Client/AppState",
                    "{label}: incomplete (fatal={:?} retryable={:?} skipped={:?})",
                    outcome.fatal, outcome.retryable, outcome.skipped
                );
                self.dispatch_app_state_sync_failed(
                    &outcome,
                    self.is_ready.load(Ordering::Relaxed),
                );
                // Seeded with what this outcome already stranded. A fatal or
                // skipped collection here is not in the retryable list the
                // scheduler carries, so without this the scheduler would start
                // clean and let a later successful round settle the bootstrap
                // for collections that never synced and will not be retried.
                let already_stranded =
                    stranded_elsewhere || !outcome.fatal.is_empty() || !outcome.skipped.is_empty();
                self.schedule_app_state_retry(outcome.retryable, scope, settles, already_stranded);
            }
            Err(e) => {
                self.log_sync_error(label, &e);
                // An IQ timeout, a malformed response, a failed blob fetch or a
                // store error takes the whole batch down without producing
                // buckets, so nothing above reaches the scheduler. Retry what
                // was asked for: the collections are no less stale for the
                // failure having been global, and the request that prompted it
                // is not coming back.
                //
                // Unasserted: the observable is a detached task that sleeps
                // before doing anything, and the two probes tried for it both
                // passed with the requeue removed.
                self.schedule_app_state_retry(
                    requested.to_vec(),
                    scope,
                    settles,
                    stranded_elsewhere,
                );
            }
        }
    }

    /// Open a scope for work starting now on the live connection.
    pub(crate) fn sync_scope(&self, deadline: Option<wacore::time::Instant>) -> SyncScope {
        SyncScope {
            generation: self.connection_generation.load(Ordering::SeqCst),
            deadline,
        }
    }

    /// Whether `scope`'s work may still proceed.
    ///
    /// The single place either question is asked. Call it at every boundary that
    /// follows an await and precedes something observable — a write, a dispatch,
    /// a scheduled retry — and nowhere else, so there is one answer per boundary
    /// rather than one per author.
    pub(crate) fn admits(&self, scope: SyncScope) -> Result<(), ScopeLost> {
        if self.connection_generation.load(Ordering::SeqCst) != scope.generation {
            return Err(ScopeLost::Retired);
        }
        if let Some(deadline) = scope.deadline
            && wacore::time::Instant::now() >= deadline
        {
            return Err(ScopeLost::Expired);
        }
        Ok(())
    }

    /// Record whether the initial bootstrap still has work outstanding.
    ///
    /// The gate is shared across connections, so a task from a retired one must
    /// not touch it: clearing would let the live connection skip a bootstrap it
    /// still needs, and arming would cost it one it does not. Routing every
    /// write through here is what keeps that check from being the caller's job —
    /// it was forgotten twice when it was.
    pub(crate) fn settle_bootstrap(&self, scope: SyncScope, outstanding: bool) {
        // Two guards, closing two different holes, which is why neither alone
        // was enough on the previous attempts.
        //
        // The admission check keeps a writer whose connection is already gone
        // from having a say at all. The compare-and-swap keeps any write from
        // clobbering one made on behalf of a newer connection. Between them the
        // worst case is a stale write that slips through the check and lands
        // before the replacement settles — and the replacement's own settle then
        // outranks it permanently, because the tag only ever moves forward.
        //
        // Expiry is deliberately not consulted: running out of time is exactly
        // when the bootstrap has to stay armed, and that is a write this
        // connection is still entitled to make.
        if self.admits(scope) == Err(ScopeLost::Retired) {
            debug!(
                target: "Client/AppState",
                "Bootstrap gate left alone: connection {} retired", scope.generation
            );
            return;
        }
        if !self
            .needs_initial_full_sync
            .settle(scope.generation, outstanding)
        {
            debug!(
                target: "Client/AppState",
                "Bootstrap gate left to a newer connection than {}", scope.generation
            );
            return;
        }
        if outstanding {
            warn!(target: "Client/AppState", "Initial App State Sync incomplete; bootstrap stays armed");
        } else {
            debug!(target: "Client/AppState", "Initial App State Sync completed.");
        }
    }

    /// Report an incomplete batched sync to consumers.
    ///
    /// `connected` says whether the client went on to dispatch `Connected`
    /// anyway, which is the difference between "degraded but usable" and "still
    /// retrying", and is the only part a consumer cannot infer from the buckets.
    pub(crate) fn dispatch_app_state_sync_failed(
        &self,
        outcome: &BatchedSyncOutcome,
        connected: bool,
    ) {
        let names = |v: &[WAPatchName]| v.iter().map(|n| n.as_str().to_string()).collect();
        self.core.event_bus.dispatch(Event::AppStateSyncFailed(
            crate::types::events::AppStateSyncFailed::builder()
                .fatal(names(&outcome.fatal))
                .retryable(names(&outcome.retryable))
                .skipped(names(&outcome.skipped))
                .connected(connected)
                .build(),
        ));
    }

    /// Retry one consumer-issued sync task, preserving the mode it asked for.
    ///
    /// Separate from [`schedule_app_state_retry`](Self::schedule_app_state_retry)
    /// because a task carries `full_sync`, and the batched path cannot express
    /// it: that one requests a snapshot only when the persisted version is zero,
    /// so a full sync routed through it becomes incremental and the caller's
    /// snapshot silently never happens.
    fn schedule_app_state_task_retry(self: &Arc<Self>, name: WAPatchName, full_sync: bool) {
        let mut scope = self.sync_scope(None);
        let client = self.clone();
        self.runtime.spawn_detached(Box::pin(async move {
            // Attempts and rounds are counted separately: a wait that ran out
            // never reached the server, so spending an attempt on it would let a
            // long-lived holder burn the budget without the sync being tried
            // once. Rounds still bound the loop overall.
            let mut attempts = 0u32;
            for round in 0..APP_STATE_RETRY_MAX_ROUNDS * 2 {
                if attempts >= APP_STATE_RETRY_MAX_ROUNDS {
                    break;
                }
                client.runtime.sleep(app_state_retry_backoff(round)).await;
                if client.is_stopping() {
                    debug!(target: "Client/AppState", "App state task retry cancelled: client stopped");
                    return;
                }
                // Rebound, not discarded: nothing on the replacement connection
                // re-issues a consumer's task, and a `full_sync` one is the
                // snapshot request this scheduler exists to keep alive. A
                // planned reconnect must not be what loses it.
                scope.rebind(client.connection_generation.load(Ordering::SeqCst));

                // Wait the planned reconnect out rather than spending an attempt
                // on it. `process_app_state_sync_task` answers `Ok(())` at its
                // own shutdown guard without contacting the server, and
                // `expected_disconnect` makes that guard true for a reconnect as
                // well as a stop — so attempting here would read a no-op as a
                // completed sync and drop the request for good.
                if client.is_reconnecting() {
                    debug!(target: "Client/AppState", "Holding the {name:?} retry: a reconnect is in flight");
                    continue;
                }

                let guard = match client
                    .reserve_for_sync(name, ReservationWait::Always, scope)
                    .await
                {
                    Ok(guard) => guard,
                    // Someone equivalent picked it up, so the request is covered.
                    Err(ReservationSkip::EquivalentSyncInFlight) => return,
                    Err(ReservationSkip::WaitTimedOut) => {
                        warn!(target: "Client/AppState", "Still waiting on the writer holding {name:?}");
                        continue;
                    }
                };
                // The reservation wait is itself an await, so the reconnect can
                // start inside it and the guard above is already stale. Asked
                // again before an attempt is counted, because the callee would
                // answer `Ok(())` at its own shutdown guard and this loop would
                // read that no-op as the sync having happened.
                if client.is_shutting_down() || client.admits(scope).is_err() {
                    debug!(target: "Client/AppState", "Dropping the {name:?} attempt: state moved while reserving");
                    drop(guard);
                    continue;
                }
                attempts += 1;
                match client.process_app_state_sync_task(name, full_sync).await {
                    // An `Ok` only counts when the connection held throughout.
                    // The callee breaks its pagination and answers `Ok(())` the
                    // moment it observes a shutdown, so a reconnect starting
                    // mid-attempt would otherwise end the scheduler and lose the
                    // request — the `full_sync` snapshot included.
                    Ok(()) if !client.is_shutting_down() && client.admits(scope).is_ok() => return,
                    Ok(()) => {
                        debug!(target: "Client/AppState", "The {name:?} attempt was cut short; keeping it queued");
                    }
                    Err(e) => {
                        drop(guard);
                        client.log_sync_error("app state task retry", &e);
                    }
                }
            }
            warn!(
                target: "Client/AppState",
                "App state task for {name:?} still unsynced after {attempts} attempts"
            );
        }));
    }

    /// Re-sync collections a run left retryable, spaced the way WA Web spaces
    /// the same case (`WASyncdConst`: 1s base, doubling, capped at an hour).
    ///
    /// A transient error takes the collection out of the batched loop rather
    /// than being re-asked inside it, which is what WA Web does — but WA Web
    /// hands it to a retry state machine afterwards, and without one a single
    /// 500 would leave the collection stale until some unrelated trigger came
    /// along. This is that machine, minus the persisted two-day expiry.
    ///
    /// The scope is taken from the caller, which has already validated it, so
    /// dispatching the failure event in between — consumer handlers run
    /// synchronously and may disconnect — cannot silently rebind these retries
    /// to whatever replaced the connection.
    ///
    /// `already_stranded` carries forward what the originating outcome left
    /// behind but did not hand over: a refusal, or a collection another writer
    /// held. Those are not in `collections`, so without it a later clean round
    /// would look like everything recovered.
    pub(crate) fn schedule_app_state_retry(
        self: &Arc<Self>,
        collections: Vec<WAPatchName>,
        scope: SyncScope,
        settles: SyncSettles,
        already_stranded: bool,
    ) {
        if collections.is_empty() {
            return;
        }
        let client = self.clone();
        self.runtime.spawn_detached(Box::pin(async move {
            let mut scope = scope;
            let mut settles = settles;
            let mut pending = collections;
            // Sticky, and only for misses that do not come back. A round can
            // strand one collection as fatal or skipped while another stays
            // retryable; if that last one then succeeds, its own `all_synced()`
            // is true even though the first never synced. Retryable ones are
            // exactly what the next round carries, so counting them would keep
            // the gate armed forever after any transient round.
            let mut left_unresolved = already_stranded;
            for round in 0..APP_STATE_RETRY_MAX_ROUNDS {
                client.runtime.sleep(app_state_retry_backoff(round)).await;
                // Only an actual stop ends this. `is_shutting_down()` is also
                // true for a planned reconnect, and giving up there would drop
                // the collections for good: the trigger that asked for them is
                // already consumed, and the reconnection path runs no bootstrap
                // once the push name is known.
                if client.is_stopping() {
                    debug!(target: "Client/AppState", "App state retry cancelled: client stopped");
                    return;
                }
                if scope.rebind(client.connection_generation.load(Ordering::SeqCst)) {
                    // The work carries over; the authority to settle does not.
                    // This run no longer belongs to the bootstrap that scheduled
                    // it, so it must never stand that gate down.
                    settles = SyncSettles::JustTheCollections;
                }

                debug!(
                    target: "Client/AppState",
                    "Retrying app state {pending:?} (round {}/{APP_STATE_RETRY_MAX_ROUNDS})",
                    round + 1
                );
                let result = client
                    .sync_collections_batched(pending.clone(), scope)
                    .await;

                // Rebinding here drops the outcome, not the work: publishing a
                // retired socket's refusal could have a consumer log out the
                // live session, while abandoning the names would strand them.
                if scope.rebind(client.connection_generation.load(Ordering::SeqCst)) {
                    debug!(target: "Client/AppState", "App state retry outcome dropped; rebound");
                    settles = SyncSettles::JustTheCollections;
                    continue;
                }

                match result {
                    Ok(outcome) => {
                        if !outcome.all_synced() {
                            client.dispatch_app_state_sync_failed(
                                &outcome,
                                client.is_ready.load(Ordering::Relaxed),
                            );
                        }
                        if !outcome.fatal.is_empty() || !outcome.skipped.is_empty() {
                            left_unresolved = true;
                        }
                        pending = outcome.retryable;
                        if pending.is_empty() {
                            if settles == SyncSettles::InitialSync {
                                client.settle_bootstrap(scope, left_unresolved);
                            }
                            return;
                        }
                    }
                    Err(e) => client.log_sync_error("app state retry", &e),
                }
            }
            warn!(
                target: "Client/AppState",
                "App state {pending:?} still unsynced after {APP_STATE_RETRY_MAX_ROUNDS} retries; \
                 leaving them to the next sync trigger"
            );
            // Consumers heard about every round that produced buckets, but a
            // sequence that ends here — or one whose rounds all failed before
            // producing any — would otherwise finish in silence, with the
            // collections still stale. `retryable` is exactly what these are:
            // not synced, and a later trigger can still fix them.
            if client.admits(scope).is_ok() {
                let exhausted = BatchedSyncOutcome {
                    retryable: pending,
                    ..Default::default()
                };
                client.dispatch_app_state_sync_failed(
                    &exhausted,
                    client.is_ready.load(Ordering::Relaxed),
                );
            }
        }));
    }

    /// Reserve `name` for a sync.
    ///
    /// Skipping is only ever sound behind an equivalent sync, and only for a
    /// caller whose request that sync subsumes — see [`ReservationWait`]. A
    /// patch send is never equivalent: it takes the same reservation and never
    /// fetches, so a sync that skipped behind one would be dropped silently and
    /// the patches that prompted it would go unfetched. Every wait is bounded by
    /// [`APP_STATE_RESERVATION_WAIT`], because the sync worker's intake loop
    /// runs non-history tasks inline and a wedged holder would otherwise stall
    /// everything queued behind it.
    /// The scope caps the wait further: the bootstrap runs under a watchdog that
    /// reconnects on wall-clock, so waiting past its deadline only guarantees the
    /// work lands on a socket that is already gone.
    async fn reserve_for_sync(
        &self,
        name: WAPatchName,
        wait: ReservationWait,
        scope: SyncScope,
    ) -> Result<SyncInFlightGuard, ReservationSkip> {
        match self.app_state_syncing.try_begin_as(name, SyncHolder::Sync) {
            Ok(guard) => return Ok(guard),
            Err(SyncHolder::Sync) if wait == ReservationWait::SkipBehindSync => {
                return Err(ReservationSkip::EquivalentSyncInFlight);
            }
            Err(holder) => {
                debug!(target: "Client/AppState", "Waiting for the {holder:?} holding {name:?}");
            }
        }
        let bound = match scope.remaining() {
            Some(remaining) => APP_STATE_RESERVATION_WAIT.min(remaining),
            None => APP_STATE_RESERVATION_WAIT,
        };
        rt_timeout(
            &*self.runtime,
            bound,
            self.app_state_syncing.begin(name, SyncHolder::Sync),
        )
        .await
        .map_err(|_| ReservationSkip::WaitTimedOut)
    }

    /// Sync multiple collections in a single IQ request, re-fetching those with `has_more_patches`.
    /// Mirrors WA Web's `serverSync()` outer loop (`WAWebSyncdServerSync`).
    ///
    /// `scope` pins the work to the connection that asked for it and, for the
    /// initial bootstrap, to the 180s critical-sync deadline. Everything that
    /// follows an await re-asks [`Client::admits`] before writing, publishing or
    /// scheduling, so a batch cannot outlive the socket it belongs to. The
    /// deadline also bounds the missing-key wait, letting the explicit
    /// `AppStateSyncKeyRequest` fallback recover a late key on this connection
    /// without running past the watchdog.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.sync_batched", level = "debug", skip_all, fields(count = collections.len()), err(Debug)))]
    pub(crate) async fn sync_collections_batched(
        &self,
        collections: Vec<WAPatchName>,
        scope: SyncScope,
    ) -> Result<BatchedSyncOutcome> {
        let mut outcome = BatchedSyncOutcome::default();
        if collections.is_empty() {
            return Ok(outcome);
        }

        // In-flight dedup. The guards release on every exit path, including
        // cancellation. A collection we could not reserve is reported skipped
        // rather than silently dropped: this call did nothing for it, and only
        // the caller knows whether that is acceptable.
        // A caller can name the same collection twice — a `server_sync`
        // notification may repeat a `<collection>` child. Reserving it once and
        // then hitting our own reservation on the second pass would file one
        // collection under both `synced` and `skipped`, making `all_synced()`
        // false and publishing a failure that blames a writer who never existed.
        let mut seen = HashSet::with_capacity(collections.len());
        let collections: Vec<WAPatchName> = collections
            .into_iter()
            .filter(|name| seen.insert(*name))
            .collect();

        // The bootstrap cannot skip: it has to know the collection is synced
        // before it dispatches Connected, and an equivalent sync in flight only
        // tells it someone else is trying. So when a deadline is supplied — which
        // is what marks the critical path — it waits for whoever holds it, and
        // finding the collection already synced afterwards is the fast case.
        // Background callers still skip, because for them the in-flight sync
        // genuinely does the work.
        let wait = if scope.is_bootstrap() {
            ReservationWait::Always
        } else {
            ReservationWait::SkipBehindSync
        };

        let mut guards = Vec::with_capacity(collections.len());
        let mut pending = Vec::with_capacity(collections.len());
        for name in collections {
            // Asked per reservation, not once for the batch: each wait can burn
            // the remaining deadline, and the watchdog fires on wall-clock
            // regardless of which collection the batch is stuck on.
            if let Err(lost) = self.admits(scope) {
                warn!(target: "Client/AppState", "Not reserving {name:?}: {lost:?}");
                outcome.retryable.push(name);
                continue;
            }
            match self.reserve_for_sync(name, wait, scope).await {
                Ok(guard) => {
                    guards.push(guard);
                    pending.push(name);
                }
                // An equivalent sync in flight is doing this work, so the
                // collection is covered and only worth reporting. A wait that ran
                // out is not covered by anyone, so it belongs with the misses
                // that deserve another attempt.
                Err(ReservationSkip::EquivalentSyncInFlight) => {
                    debug!(target: "Client/AppState", "Skipping {name:?} in batch: an equivalent sync holds it");
                    outcome.skipped.push(name);
                }
                Err(ReservationSkip::WaitTimedOut) => {
                    warn!(target: "Client/AppState", "Gave up waiting for the writer holding {name:?}");
                    outcome.retryable.push(name);
                }
            }
        }

        if pending.is_empty() {
            return Ok(outcome);
        }

        self.sync_collections_batched_inner(pending, scope, &mut outcome)
            .await?;

        // A run that crossed its deadline is not a clean one, even if every
        // collection it touched came back applied. Reporting it as fully synced
        // would let the bootstrap abort its watchdog and dispatch Connected on a
        // scope that has already expired, which is the outcome the deadline
        // exists to prevent. Moving the synced names to `retryable` sends it
        // down the retry path instead; the versions are persisted, so the next
        // attempt resumes rather than repeats.
        if !outcome.synced.is_empty()
            && let Err(lost @ ScopeLost::Expired) = self.admits(scope)
        {
            warn!(
                target: "Client/AppState",
                "Batched sync: {:?} applied but the run outlived its scope ({lost:?})",
                outcome.synced
            );
            let applied = std::mem::take(&mut outcome.synced);
            outcome.retryable.extend(applied);
        }

        Ok(outcome)
    }

    async fn sync_collections_batched_inner(
        &self,
        mut pending: Vec<WAPatchName>,
        scope: SyncScope,
        outcome: &mut BatchedSyncOutcome,
    ) -> Result<()> {
        use wacore::appstate::patch_decode::CollectionSyncError;
        // WA Web's own bound. Its loop reads `(l < y || (i.length > 0 && l < C))`
        // with `y = 5` and `C = 500`, and since the body only runs while there
        // is something left to refetch, that collapses to `l < 500` — the `y`
        // never bites. Rounds are back-to-back there too; the syncd backoff
        // applies to retrying a *failed* collection later, not to paging one
        // that is still making progress.
        const MAX_ITERATIONS: usize = 500;
        let mut iteration = 0;

        while !pending.is_empty() && iteration < MAX_ITERATIONS {
            // With the cap at WA Web's 500, a collection paging healthily can
            // outlast the bootstrap's watchdog and be cut off mid-page on every
            // attempt, never reaching readiness though every page succeeded.
            // Stopping here instead keeps the progress: the versions applied so
            // far are persisted and the reconnect resumes from them.
            if let Err(lost) = self.admits(scope) {
                warn!(
                    target: "Client/AppState",
                    "Batched sync: stopping with {pending:?} still paging ({lost:?})"
                );
                outcome.retryable.extend(pending);
                return Ok(());
            }
            iteration += 1;
            debug!(
                target: "Client/AppState",
                "Batched sync iteration {}/{}: {:?}",
                iteration, MAX_ITERATIONS, pending
            );

            let backend = self.persistence_manager.backend();

            // Build multi-collection IQ, tracking which collections need a snapshot
            let mut collection_nodes = Vec::with_capacity(pending.len());
            let mut was_snapshot = HashSet::new();
            for &name in &pending {
                let state = backend.get_version(name.as_str()).await?;
                let want_snapshot = state.version == 0;
                if want_snapshot {
                    was_snapshot.insert(name);
                }
                let mut builder = NodeBuilder::new("collection")
                    .attr("name", name.as_str())
                    .attr(
                        "return_snapshot",
                        if want_snapshot { "true" } else { "false" },
                    );
                if !want_snapshot {
                    builder = builder.attr("version", state.version);
                }
                collection_nodes.push(builder.build());
            }

            let sync_node = NodeBuilder::new("sync").children(collection_nodes).build();
            let iq = crate::request::InfoQuery {
                namespace: "w:sync:app:state",
                query_type: crate::request::InfoQueryType::Set,
                to: server_jid().clone(),
                target: None,
                id: None,
                content: Some(wacore_binary::NodeContent::Nodes(vec![sync_node])),
                timeout: Some(Duration::from_secs(30)),
            };

            let resp = self.send_iq(iq).await?;

            // The IQ can outrun the scope, so the round is re-admitted before
            // any of it is trusted.
            if let Err(lost) = self.admits(scope) {
                warn!(
                    target: "Client/AppState",
                    "Batched sync: dropping the response for {pending:?} ({lost:?})"
                );
                outcome.retryable.extend(pending);
                return Ok(());
            }

            // Parse the response once here for pre-download; the same parsed
            // lists are handed to the processor below (no second parse).
            let mut patch_lists =
                wacore::appstate::patch_decode::parse_patch_lists_ref(resp.get())?;

            // Drop a repeated collection before anything is applied. The
            // processor persists each list it is handed — mutation MACs and the
            // version — so two entries for one collection would be applied
            // twice, and the second could advance the MAC store past the version
            // the first then writes back, leaving the ltHash disagreeing with
            // the MACs. A collection outside `pending` is worse still: nothing
            // reserved it, so applying it can interleave with a concurrent sync
            // or patch send for that collection, and it dispatches mutations
            // nobody asked for. Checking after the fact only fixes the
            // bookkeeping.
            {
                let requested: HashSet<WAPatchName> = pending.iter().copied().collect();
                let mut seen: HashSet<WAPatchName> = HashSet::new();
                patch_lists.retain(|pl| {
                    if !requested.contains(&pl.name) {
                        warn!(
                            target: "Client/AppState",
                            "Batched sync: response carried unrequested collection {:?}; dropping it",
                            pl.name
                        );
                        return false;
                    }
                    if seen.insert(pl.name) {
                        return true;
                    }
                    warn!(
                        target: "Client/AppState",
                        "Batched sync: response repeated collection {:?}; dropping the duplicate",
                        pl.name
                    );
                    false
                });
            }

            let proc = self.get_app_state_processor().await;
            // Pre-download all external blobs for all collections in the response,
            // concurrently (independent CDN GETs, keyed by directPath).
            let pre_downloaded = self.pre_download_external_blobs(&patch_lists).await;

            let download = |ext: &wa::ExternalBlobReference| -> Result<Vec<u8>> {
                if let Some(path) = &ext.direct_path {
                    if let Some(bytes) = pre_downloaded.get(path) {
                        Ok(bytes.clone())
                    } else {
                        Err(anyhow::anyhow!(
                            "external blob not pre-downloaded: {}",
                            path
                        ))
                    }
                } else {
                    Err(anyhow::anyhow!("external blob has no directPath"))
                }
            };

            // Request any missing decode keys and wait for them BEFORE processing. Inline
            // each list's external blobs first so the SNAPSHOT's key_id (inside the blob,
            // not the patch metadata) is visible -- else process_patch_lists aborts with
            // KeyNotFound on the snapshot key. If the share doesn't land in time, skip
            // this batch instead of aborting; it re-syncs on a later cycle once the key
            // arrives (process_patch_lists is all-or-nothing on a missing key anyway).
            let mut missing_all: Vec<Vec<u8>> = Vec::new();
            for pl in &mut patch_lists {
                if let Ok(m) = proc.missing_key_ids_after_inline(pl, &download).await {
                    missing_all.extend(m);
                }
            }
            // Bound the key wait by the critical-sync deadline when one was given
            // (initial bootstrap), so a late/never-auto-shared key still recovers via
            // the explicit request on this connection; otherwise a fixed short wait.
            let key_wait = scope.remaining().unwrap_or(APP_STATE_KEY_REQUEST_TIMEOUT);
            if !missing_all.is_empty() && !self.request_keys_and_wait(missing_all, key_wait).await {
                // The re-shared key didn't land in time. Nothing in this round can
                // be decoded, so report every collection still pending as
                // retryable rather than as synced: they re-sync on a later cycle
                // once the share arrives, and the keys we DID repair are already
                // persisted.
                warn!(
                    target: "Client/AppState",
                    "Batched sync: decode key(s) still missing after re-request, deferring {pending:?}"
                );
                outcome.retryable.extend(pending);
                return Ok(());
            }

            // The last gate before anything is written. Blob downloads and the
            // key wait both sit between the previous check and here, and neither
            // is bounded by the scope, so this is where a response that is no
            // longer ours stops being applied.
            if let Err(lost) = self.admits(scope) {
                warn!(
                    target: "Client/AppState",
                    "Batched sync: not applying {pending:?} ({lost:?})"
                );
                outcome.retryable.extend(pending);
                return Ok(());
            }

            // Applied one collection at a time rather than handing the whole
            // batch over, because the processor persists each list as it goes:
            // a batch that starts inside the scope can still be writing its
            // third collection well outside it. Per-list is the finest boundary
            // available without teaching `wacore` about connections, and it caps
            // what a retired scope can commit at one collection instead of five.
            let mut results = Vec::with_capacity(patch_lists.len());
            for pl in patch_lists {
                if let Err(lost) = self.admits(scope) {
                    warn!(
                        target: "Client/AppState",
                        "Batched sync: stopping before {:?} ({lost:?})", pl.name
                    );
                    break;
                }
                results.push(proc.process_one_patch_list(pl, &download, true).await?);
            }

            let mut needs_refetch = Vec::new();
            // A `<sync>` that simply omits a requested `<collection>` parses
            // fine, so without this the collection lands in no bucket at all and
            // `all_synced()` reports a batch that never covered it. Track what
            // the response actually accounted for and reconcile below.
            // Duplicates are already gone, dropped above before anything applied
            // them.
            let mut answered: HashSet<WAPatchName> = HashSet::new();

            for (mutations, new_state, list) in results {
                let name = list.name;
                answered.insert(name);

                // No admission check here, deliberately. `process_one_patch_list`
                // persists the collection's version and mutation MACs before it
                // returns, so by this point the cursor has already moved. A
                // collection dropped here would never be re-sent — the retry
                // asks from the advanced version — and its mutations would be
                // lost for good, `setting_pushName` and the NCT salt included.
                // The scope is checked before the apply, which is the last
                // moment a collection can still be declined; after it,
                // dispatching is not optional.

                // Handle per-collection errors
                if let Some(ref err) = list.error {
                    match err {
                        CollectionSyncError::Conflict { has_more } => {
                            if *has_more {
                                // ConflictHasMore: server has more patches, must refetch.
                                warn!(target: "Client/AppState", "Collection {:?} conflict (has_more=true), will refetch", name);
                                needs_refetch.push(name);
                            } else {
                                // Conflict without has_more: WA Web treats this as success
                                // when there are no pending mutations to push (which is
                                // always the case for us since we don't push app state).
                                debug!(target: "Client/AppState", "Collection {:?} conflict (has_more=false), treating as success (no pending mutations)", name);
                                outcome.synced.push(name);
                            }
                            continue;
                        }
                        CollectionSyncError::Fatal { code, text } => {
                            warn!(target: "Client/AppState", "Collection {:?} fatal error {}: {}", name, code, text);
                            outcome.fatal.push(name);
                            continue;
                        }
                        CollectionSyncError::Retry { code, text } => {
                            // Done for this run, not refetched inside it. WA Web
                            // routes ErrorRetry to `doneCollections` and leaves
                            // the next attempt to its retry state machine, which
                            // spaces them; refetching here would instead hammer
                            // the same failing collection for every iteration
                            // the cap allows.
                            warn!(target: "Client/AppState", "Collection {:?} retryable error {}: {}", name, code, text);
                            outcome.retryable.push(name);
                            continue;
                        }
                    }
                }

                // Handle missing keys
                let missing = match proc.get_missing_key_ids(&list).await {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("Failed to get missing key IDs for {:?}: {}", name, e);
                        Vec::new()
                    }
                };
                self.request_missing_keys_with_dedup(&missing, APP_STATE_KEY_REQUEST_DEDUP)
                    .await;

                // full_sync is true only when this collection had a snapshot
                // (version was 0 before sync). This prevents server_sync-triggered
                // incremental syncs from being incorrectly marked as full syncs.
                let full_sync = was_snapshot.contains(&name);
                wacore::telemetry::appstate_mutations(mutations.len() as u64);
                for m in mutations {
                    self.dispatch_app_state_mutation(&m, full_sync).await;
                }

                // Save version
                backend
                    .set_version(name.as_str(), new_state.clone())
                    .await?;

                // Check if this collection needs more patches
                if list.has_more_patches {
                    needs_refetch.push(name);
                } else {
                    outcome.synced.push(name);
                }

                debug!(
                    target: "Client/AppState",
                    "Batched sync: {:?} done (version={}, has_more={})",
                    name, new_state.version, list.has_more_patches
                );
            }

            // Anything asked for that the response did not mention is not synced
            // and did not fail either; treat it as retryable so it is neither
            // reported as done nor re-asked immediately in this loop.
            for name in pending {
                if !answered.contains(&name) {
                    warn!(
                        target: "Client/AppState",
                        "Batched sync: response omitted collection {name:?}"
                    );
                    outcome.retryable.push(name);
                }
            }

            pending = needs_refetch;
        }

        if !pending.is_empty() {
            // Still paging when the cap ran out. Retryable, not fatal: the
            // versions applied so far are persisted, so a later sync resumes
            // where this one stopped. WA Web classifies the same exhaustion as
            // `ErrorRetry`.
            warn!(
                target: "Client/AppState",
                "Batched sync: max iterations ({}) reached for {:?}",
                MAX_ITERATIONS, pending
            );
            outcome.retryable.extend(pending);
        }

        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.sync", level = "debug", skip_all, fields(name = ?name, full_sync = full_sync), err(Debug)))]
    pub(crate) async fn process_app_state_sync_task(
        &self,
        name: WAPatchName,
        full_sync: bool,
    ) -> Result<()> {
        if self.is_shutting_down() {
            debug!(target: "Client/AppState", "Skipping app state sync task {:?}: client is shutting down", name);
            return Ok(());
        }

        let backend = self.persistence_manager.backend();
        let mut full_sync = full_sync;

        let mut state = backend.get_version(name.as_str()).await?;
        if state.version == 0 {
            full_sync = true;
        }

        let mut has_more = true;
        let mut want_snapshot = full_sync;
        // Safety cap to prevent infinite loops if the server keeps returning
        // has_more_patches=true without advancing the version (WA Web uses 500).
        const MAX_PAGINATION_ITERATIONS: u32 = 500;
        let mut iteration = 0u32;

        while has_more {
            if self.is_shutting_down() {
                debug!(target: "Client/AppState", "Stopping app state sync task {:?}: shutdown detected", name);
                break;
            }
            iteration += 1;
            if iteration > MAX_PAGINATION_ITERATIONS {
                warn!(target: "Client/AppState", "App state sync for {:?} exceeded {} iterations, aborting", name, MAX_PAGINATION_ITERATIONS);
                break;
            }
            debug!(target: "Client/AppState", "Fetching app state patch batch: name={:?} want_snapshot={want_snapshot} version={} full_sync={} has_more_previous={}", name, state.version, full_sync, has_more);

            let mut collection_builder = NodeBuilder::new("collection")
                .attr("name", name.as_str())
                .attr(
                    "return_snapshot",
                    if want_snapshot { "true" } else { "false" },
                );
            if !want_snapshot {
                collection_builder = collection_builder.attr("version", state.version);
            }
            let sync_node = NodeBuilder::new("sync")
                .children([collection_builder.build()])
                .build();
            let iq = crate::request::InfoQuery {
                namespace: "w:sync:app:state",
                query_type: crate::request::InfoQueryType::Set,
                to: server_jid().clone(),
                target: None,
                id: None,
                content: Some(wacore_binary::NodeContent::Nodes(vec![sync_node])),
                timeout: None,
            };

            let resp = self.send_iq(iq).await?;
            if self.is_shutting_down() {
                debug!(target: "Client/AppState", "Discarding app state sync response for {:?}: shutdown detected", name);
                break;
            }
            debug!(target: "Client/AppState", "Received IQ response for {:?}; decoding patches", name);

            let _decode_start = wacore::time::Instant::now();

            // Parse the response once here; the same parsed list is handed to the
            // processor below (no second parse).
            let mut pl = wacore::appstate::patch_decode::parse_patch_list_ref(resp.get())?;
            debug!(target: "Client/AppState", "Parsed patch list for {:?}: has_snapshot_ref={} has_more_patches={} patches_count={}",
                name, pl.snapshot_ref.is_some(), pl.has_more_patches, pl.patches.len());

            let proc = self.get_app_state_processor().await;

            // Pre-download all external blobs (snapshot and patch mutations),
            // concurrently, keyed by directPath.
            let pre_downloaded = self
                .pre_download_external_blobs(std::slice::from_ref(&pl))
                .await;

            let download = |ext: &wa::ExternalBlobReference| -> Result<Vec<u8>> {
                if let Some(path) = &ext.direct_path {
                    if let Some(bytes) = pre_downloaded.get(path) {
                        Ok(bytes.clone())
                    } else {
                        Err(anyhow::anyhow!(
                            "external blob not pre-downloaded: {}",
                            path
                        ))
                    }
                } else {
                    Err(anyhow::anyhow!("external blob has no directPath"))
                }
            };

            // Request any missing decode keys and wait for them BEFORE processing. Inline
            // the blobs first so the SNAPSHOT's key_id (inside its external blob, not the
            // patch metadata) is visible -- else process aborts with KeyNotFound on the
            // snapshot key. If the share doesn't land in time, skip this collection
            // instead of aborting; it re-syncs on a later cycle once the key arrives.
            let missing = proc
                .missing_key_ids_after_inline(&mut pl, &download)
                .await
                .unwrap_or_default();
            if !missing.is_empty()
                && !self
                    .request_keys_and_wait(missing, APP_STATE_KEY_REQUEST_TIMEOUT)
                    .await
            {
                // Report failure (not a partial success) so the caller retries instead of
                // treating the collection as synced; it re-syncs once the share lands.
                // Pages already decoded this run have their version persisted.
                return Err(anyhow::anyhow!(
                    "app-state decode key(s) for {name:?} still missing after re-request; deferring sync"
                ));
            }

            let (mutations, new_state, list) =
                proc.process_parsed_patch_list(pl, &download, true).await?;
            let decode_elapsed = _decode_start.elapsed();
            if decode_elapsed.as_millis() > 500 {
                debug!(target: "Client/AppState", "Patch decode for {:?} took {:?}", name, decode_elapsed);
            }

            let missing = match proc.get_missing_key_ids(&list).await {
                Ok(v) => v,
                Err(e) => {
                    warn!("Failed to get missing key IDs for {:?}: {}", name, e);
                    Vec::new()
                }
            };
            self.request_missing_keys_with_dedup(&missing, APP_STATE_KEY_REQUEST_DEDUP)
                .await;

            wacore::telemetry::appstate_mutations(mutations.len() as u64);
            for m in mutations {
                debug!(target: "Client/AppState", "Dispatching mutation kind={} index_len={} full_sync={}", m.index.first().map(|s| s.as_str()).unwrap_or(""), m.index.len(), full_sync);
                self.dispatch_app_state_mutation(&m, full_sync).await;
            }

            state = new_state;
            has_more = list.has_more_patches;
            // After the first batch, never request a snapshot again — only incremental patches.
            want_snapshot = false;
            debug!(target: "Client/AppState", "After processing batch name={:?} has_more={has_more} new_version={}", name, state.version);
        }

        backend.set_version(name.as_str(), state.clone()).await?;

        debug!(target: "Client/AppState", "Completed and saved app state sync for {:?} (final version={})", name, state.version);
        Ok(())
    }

    /// Request the missing decode keys, wait up to `timeout` for the re-share, then
    /// VERIFY they actually landed. Returns true only when every requested key is now
    /// stored (the caller may process); false means the share didn't arrive in time and
    /// the caller must NOT process -- doing so would abort with KeyNotFound -- and should
    /// skip the collection so it re-syncs on a later cycle. Empty input returns true
    /// (nothing to wait for). Waits even when the per-key dedup suppressed the send: a
    /// deduped request means an earlier one is still in flight, so the key may yet land
    /// here, and a re-verify that fails can't be masked by treating "request sent" as
    /// success or by a wake from an unrelated key share.
    async fn request_keys_and_wait(&self, mut missing: Vec<Vec<u8>>, timeout: Duration) -> bool {
        if missing.is_empty() {
            return true;
        }
        let deadline = wacore::time::Instant::now() + timeout;
        let backend = self.persistence_manager.backend();
        let mut retry_after = initial_app_state_key_retry(timeout);
        loop {
            let listener = self.initial_keys_synced_notifier.listen();
            remove_available_app_state_keys(&*backend, &mut missing).await;
            if missing.is_empty() {
                return true;
            }

            let request = self.request_missing_keys_with_dedup(&missing, retry_after);
            let schedule = match self
                .await_app_state_key_request(&*backend, &missing, deadline, listener, request)
                .await
            {
                AppStateKeyRequestProgress::Scheduled(schedule) => schedule,
                AppStateKeyRequestProgress::KeysReady => return true,
                AppStateKeyRequestProgress::TimedOut => return false,
            };
            if schedule.sent {
                debug!(target: "Client/AppState", "Requested {} missing app-state key(s); retrying after {retry_after:?} if no share arrives", missing.len());
                retry_after = retry_after.saturating_mul(2).min(APP_STATE_KEY_RETRY_MAX);
            }

            let listener = self.initial_keys_synced_notifier.listen();
            remove_available_app_state_keys(&*backend, &mut missing).await;
            if missing.is_empty() {
                return true;
            }

            let remaining = deadline.saturating_duration_since(wacore::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }

            let retry_wait = schedule
                .retry_at
                .saturating_duration_since(wacore::time::Instant::now());
            let wait = remaining.min(retry_wait);
            if !wait.is_zero() {
                let _ = rt_timeout(&*self.runtime, wait, listener).await;
            }
        }
    }

    async fn await_app_state_key_request<F>(
        &self,
        backend: &dyn crate::store::traits::Backend,
        missing: &[Vec<u8>],
        deadline: wacore::time::Instant,
        mut listener: event_listener::EventListener,
        request: F,
    ) -> AppStateKeyRequestProgress
    where
        F: Future<Output = AppStateKeyRequestSchedule>,
    {
        futures::pin_mut!(request);
        loop {
            let remaining = deadline.saturating_duration_since(wacore::time::Instant::now());
            if remaining.is_zero() {
                return if app_state_keys_available(backend, missing).await {
                    AppStateKeyRequestProgress::KeysReady
                } else {
                    AppStateKeyRequestProgress::TimedOut
                };
            }

            let notified = rt_timeout(&*self.runtime, remaining, listener);
            futures::pin_mut!(notified);
            match futures::future::select(request.as_mut(), notified.as_mut()).await {
                futures::future::Either::Left((schedule, _)) => {
                    return AppStateKeyRequestProgress::Scheduled(schedule);
                }
                futures::future::Either::Right((notification, _)) => {
                    let next_listener = self.initial_keys_synced_notifier.listen();
                    if app_state_keys_available(backend, missing).await {
                        return AppStateKeyRequestProgress::KeysReady;
                    }
                    if notification.is_err() {
                        return AppStateKeyRequestProgress::TimedOut;
                    }
                    listener = next_listener;
                }
            }
        }
    }

    /// Request missing app-state keys with dedup stamps.
    /// Total failure removes stamps; partial fanout gets a short retry deadline.
    async fn request_missing_keys_with_dedup(
        &self,
        missing: &[Vec<u8>],
        retry_after: Duration,
    ) -> AppStateKeyRequestSchedule {
        if missing.is_empty() {
            return AppStateKeyRequestSchedule {
                retry_at: wacore::time::Instant::now() + retry_after,
                sent: false,
            };
        }
        let mut guard = self.app_state_key_requests.lock().await;
        let now = wacore::time::Instant::now();
        let requested_retry_at = now + retry_after;
        guard.retain(|_, retry_at| now < *retry_at);

        let mut to_request: Option<Vec<&[u8]>> = None;
        let mut next_retry_at = requested_retry_at;
        for key_id in missing {
            if let Some(retry_at) = guard.get_mut(key_id.as_slice()) {
                if *retry_at > requested_retry_at {
                    *retry_at = requested_retry_at;
                }
                next_retry_at = next_retry_at.min(*retry_at);
            } else {
                guard.insert(key_id.clone(), requested_retry_at);
                to_request
                    .get_or_insert_with(|| Vec::with_capacity(missing.len()))
                    .push(key_id.as_slice());
            }
        }
        drop(guard);

        let Some(to_request) = to_request else {
            return AppStateKeyRequestSchedule {
                retry_at: next_retry_at,
                sent: false,
            };
        };

        match self
            .request_app_state_keys(&to_request, retry_after.min(APP_STATE_KEY_REQUEST_TIMEOUT))
            .await
        {
            Ok(AppStateKeyRequestDelivery::AllPeers) => AppStateKeyRequestSchedule {
                retry_at: next_retry_at,
                sent: true,
            },
            Ok(AppStateKeyRequestDelivery::SomePeers) => {
                let retry_at = wacore::time::Instant::now() + APP_STATE_KEY_PARTIAL_RETRY;
                let mut guard = self.app_state_key_requests.lock().await;
                for key_id in &to_request {
                    if let Some(deadline) = guard.get_mut(*key_id) {
                        *deadline = (*deadline).min(retry_at);
                    }
                }
                AppStateKeyRequestSchedule {
                    retry_at: next_retry_at.min(retry_at),
                    sent: true,
                }
            }
            Err(e) => {
                warn!("Failed to send app state key request: {e}");
                let mut guard = self.app_state_key_requests.lock().await;
                for key_id in &to_request {
                    if guard
                        .get(*key_id)
                        .is_some_and(|deadline| *deadline == requested_retry_at)
                    {
                        guard.remove(*key_id);
                    }
                }
                AppStateKeyRequestSchedule {
                    retry_at: requested_retry_at,
                    sent: false,
                }
            }
        }
    }

    async fn app_state_key_request_peers(&self) -> Result<Vec<Jid>, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let own_jid = device_snapshot
            .pn
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("no own JID available for app-state key request"))?;
        let current_device = own_jid.device;
        let primary = own_jid.to_non_ad();
        drop(device_snapshot);

        let peers = match self.get_user_devices(std::slice::from_ref(&primary)).await {
            Ok(devices) => devices,
            Err(error) => {
                warn!(
                    "Own device-list query failed; requesting app-state keys from primary only: {error}"
                );
                Vec::new()
            }
        };
        finalize_app_state_key_request_peers(peers, current_device, primary)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.request_keys", level = "debug", skip_all, fields(count = raw_key_ids.len()), err(Debug)))]
    async fn request_app_state_keys(
        &self,
        raw_key_ids: &[&[u8]],
        fanout_timeout: Duration,
    ) -> Result<AppStateKeyRequestDelivery, anyhow::Error> {
        if raw_key_ids.is_empty() {
            return Ok(AppStateKeyRequestDelivery::AllPeers);
        }
        let peers = self.app_state_key_request_peers().await?;
        let key_ids: Vec<wa::message::AppStateSyncKeyId> = raw_key_ids
            .iter()
            .map(|k| wa::message::AppStateSyncKeyId {
                key_id: Some(k.to_vec()),
            })
            .collect();
        let msg = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::AppStateSyncKeyRequest),
                app_state_sync_key_request: buffa::MessageField::some(
                    wa::message::AppStateSyncKeyRequest { key_ids },
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let requests = futures::stream::FuturesUnordered::new();
        for peer in peers {
            let msg = &msg;
            requests.push(async move {
                let device = peer.device;
                let result = async {
                    self.ensure_e2e_sessions(std::slice::from_ref(&peer))
                        .await?;
                    let request_id = self.generate_message_id();
                    self.send_message_impl(
                        peer,
                        msg,
                        crate::send::SendPipelineOptions {
                            request_id: Some(&request_id),
                            peer: true,
                            ..Default::default()
                        },
                    )
                    .await
                }
                .await;
                (device, result)
            });
        }

        collect_app_state_key_request_results(&*self.runtime, requests, fanout_timeout).await
    }

    /// Send an app state patch to the server for a given collection.
    ///
    /// The server enforces optimistic concurrency on the collection `version`:
    /// a patch built on a base another device has already moved past is refused
    /// with `<collection type="error"><error code="409">`, *inside an otherwise
    /// successful IQ*, together with the patches that won. WA Web resolves that
    /// by applying the winners and letting `serverSync` re-queue the collection
    /// while pending mutations remain, so the mutation is re-sent on the new
    /// base instead of being dropped; this mirrors that, bounded by the same
    /// iteration cap WA Web uses (`ServerSync.js`, `y = 5`).
    ///
    /// `400`/`404` are fatal and anything else retryable, per
    /// `WAWebSyncdResponseParser`. All of them are errors here — never `Ok`.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.send_patch", level = "debug", skip_all, fields(name = %collection_name, count = mutations.len()), err(Debug)))]
    pub(crate) async fn send_app_state_patch(
        &self,
        collection_name: &str,
        mutations: Vec<wa::SyncdMutation>,
    ) -> Result<()> {
        use wacore::appstate::patch_decode::CollectionSyncError;

        let patch_name = collection_name.parse::<WAPatchName>().ok();
        // Held across the whole build-send-resolve cycle: the base version is
        // read at build time and only stops being valid once the send lands, so
        // releasing earlier would let a second verb build on a base this one is
        // about to consume. Deliberately held over the trailing re-sync too —
        // dropping it there would let the next send start from a base the
        // re-sync is about to move, trading a short wait for the 409s this
        // whole path exists to avoid.
        let _send_guard = self.app_state_send_lock.lock().await;
        // The send lock only orders sends against each other. This one orders
        // the send against the sync worker, which writes the same version and
        // mutation-MAC rows: without it, a conflict response for vN could be
        // absorbed while a sync is persisting vN+1, and the interleaved writes
        // would leave the ltHash disagreeing with the MAC store — the very
        // divergence #1156 is about. Waits rather than skipping, and the
        // re-syncs below go through `_inner` because this task already holds
        // the reservation they would otherwise take.
        let _collection_guard = match patch_name {
            Some(name) => Some(
                self.app_state_syncing
                    .begin(name, SyncHolder::PatchSend)
                    .await,
            ),
            None => None,
        };
        let proc = self.get_app_state_processor().await;

        for attempt in 1..=APP_STATE_PATCH_SEND_ATTEMPTS {
            // Cloned per attempt because a conflict rebuilds the patch against
            // the winner's base; verbs carry one or two mutations, and this only
            // runs on the (rare) conflict path after the first attempt.
            let (patch_bytes, base_version) =
                proc.build_patch(collection_name, mutations.clone()).await?;

            let collection_node = NodeBuilder::new("collection")
                .attr("name", collection_name)
                .attr("version", base_version)
                .attr("return_snapshot", "false")
                .children([NodeBuilder::new("patch").bytes(patch_bytes).build()])
                .build();
            let sync_node = NodeBuilder::new("sync").children([collection_node]).build();
            let iq = crate::request::InfoQuery {
                namespace: "w:sync:app:state",
                query_type: crate::request::InfoQueryType::Set,
                to: server_jid().clone(),
                target: None,
                id: None,
                content: Some(wacore_binary::NodeContent::Nodes(vec![sync_node])),
                timeout: None,
            };

            let resp = self.send_iq(iq).await?;
            let resp = resp.get().to_owned();
            // Absence and malformation are different answers. A response with no
            // `<sync><collection>` at all carries no per-collection verdict —
            // a transport-level failure would have come back as
            // `<iq type="error">` and been raised by send_iq already — so it is
            // an accepted patch. A collection that IS present but does not parse
            // may well be carrying the rejection, and manufacturing an empty
            // success from it would drop the mutation exactly as before.
            let list = match wacore::appstate::patch_decode::parse_patch_list(&resp) {
                Ok(list) => list,
                Err(e)
                    if resp
                        .get_optional_child_by_tag(&["sync", "collection"])
                        .is_none() =>
                {
                    debug!(
                        target: "Client/AppState",
                        "Patch response for {collection_name} carried no collection verdict ({e}); treating as accepted"
                    );
                    wacore::appstate::patch_decode::PatchList {
                        name: patch_name.unwrap_or(WAPatchName::Unknown),
                        has_more_patches: false,
                        patches: Vec::new(),
                        snapshot: None,
                        snapshot_ref: None,
                        error: None,
                    }
                }
                Err(e) => {
                    return Err(e.context(format!(
                        "unreadable app-state patch response for {collection_name}"
                    )));
                }
            };
            if Some(list.name) != patch_name {
                return Err(anyhow::anyhow!(
                    "app-state patch response collection mismatch: requested {collection_name}, got {}",
                    list.name.as_str()
                ));
            }

            match list.error {
                None => {
                    // Re-sync to pick up whatever else moved while we were sending.
                    // Matches whatsmeow's fetchAppState after a successful send.
                    if let Some(patch_name) = patch_name
                        && let Err(e) = self.fetch_app_state_with_retry_inner(patch_name).await
                    {
                        log::warn!("Failed to re-sync {collection_name} after patch send: {e}");
                    }
                    return Ok(());
                }
                Some(CollectionSyncError::Conflict { has_more }) => {
                    warn!(
                        target: "Client/AppState",
                        "Patch for {collection_name} conflicted on v{base_version} \
                         (attempt {attempt}/{APP_STATE_PATCH_SEND_ATTEMPTS}, has_more={has_more}); \
                         applying the conflicting patches and rebuilding"
                    );
                    self.absorb_conflicting_patches(collection_name, patch_name, list, has_more)
                        .await;
                }
                Some(error) => {
                    return Err(anyhow::anyhow!(
                        "app-state patch for {collection_name} rejected: {error}"
                    ));
                }
            }
        }

        Err(anyhow::anyhow!(
            "app-state patch for {collection_name} still conflicting after \
             {APP_STATE_PATCH_SEND_ATTEMPTS} attempts"
        ))
    }

    /// Fold the patches a 409 response carried into local state, so the retry
    /// builds on the base that actually won.
    ///
    /// Best-effort by design: if the response carried nothing usable (or failed
    /// to apply — a missing decode key, a bad blob), a plain re-sync is the
    /// fallback that advances the base. Either way the caller retries; the only
    /// unrecoverable outcome is making no progress, which the attempt cap turns
    /// into an error rather than a silent drop.
    async fn absorb_conflicting_patches(
        &self,
        collection_name: &str,
        patch_name: Option<WAPatchName>,
        mut list: wacore::appstate::patch_decode::PatchList,
        has_more: bool,
    ) {
        // The error tag described the send; the patches under it are ordinary
        // inbound data, so clear it before handing the list to the processor.
        list.error = None;
        let applied = if list.patches.is_empty() && list.snapshot_ref.is_none() {
            false
        } else {
            let pre_downloaded = self
                .pre_download_external_blobs(std::slice::from_ref(&list))
                .await;
            let download = |ext: &wa::ExternalBlobReference| -> Result<Vec<u8>> {
                let path = ext
                    .direct_path
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("external blob has no directPath"))?;
                pre_downloaded
                    .get(path)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("external blob not pre-downloaded: {path}"))
            };
            let proc = self.get_app_state_processor().await;
            match proc.process_parsed_patch_list(list, &download, true).await {
                Ok((mutations, _, _)) => {
                    wacore::telemetry::appstate_mutations(mutations.len() as u64);
                    for m in &mutations {
                        self.dispatch_app_state_mutation(m, false).await;
                    }
                    true
                }
                Err(e) => {
                    warn!(
                        target: "Client/AppState",
                        "Failed to apply the patches {collection_name} conflicted with: {e:#}"
                    );
                    false
                }
            }
        };

        // `has_more` means the server held patches back, so even a clean apply
        // leaves the base short of the head.
        if (!applied || has_more)
            && let Some(patch_name) = patch_name
            && let Err(e) = self.fetch_app_state_with_retry_inner(patch_name).await
        {
            warn!(
                target: "Client/AppState",
                "Failed to re-sync {collection_name} after a patch conflict: {e}"
            );
        }
    }

    async fn dispatch_app_state_mutation(
        &self,
        m: &crate::appstate_sync::Mutation,
        full_sync: bool,
    ) {
        use wacore::types::events::Event;

        if m.index.is_empty() {
            return;
        }

        // NCT salt sync — handles both "set" (store salt) and "remove" (clear salt).
        // Source: WAWebNctSaltSync, syncd collection RegularHigh, action "nct_salt_sync".
        if m.index[0] == "nct_salt_sync" {
            if m.operation == wa::syncd_mutation::SyncdOperation::Remove {
                debug!(target: "Client/AppState", "Removing NCT salt via app state sync");
                self.persistence_manager
                    .process_command(DeviceCommand::SetNctSalt(None))
                    .await;
            } else if let Some(val) = &m.action_value
                && let Some(act) = val.nct_salt_sync_action.as_option()
                && let Some(salt) = &act.salt
            {
                if salt.is_empty() {
                    warn!(target: "Client/AppState", "nct_salt_sync mutation has empty salt, ignoring");
                } else {
                    debug!(target: "Client/AppState", "Stored NCT salt via app state sync ({} bytes)", salt.len());
                    self.persistence_manager
                        .process_command(DeviceCommand::SetNctSalt(Some(salt.clone())))
                        .await;
                }
            } else {
                warn!(target: "Client/AppState", "nct_salt_sync mutation missing salt in action value");
            }
            return;
        }

        // All remaining mutations only care about Set operations
        if m.operation != wa::syncd_mutation::SyncdOperation::Set {
            return;
        }

        // Delegate chat-related mutations (mute, pin, archive, star, contact, etc.)
        if crate::features::chat_actions::dispatch_chat_mutation(&self.core.event_bus, m, full_sync)
        {
            return;
        }

        // Label mutations have their own index shape (labelId, not a chat JID at
        // index[1]), so they are dispatched separately from chat actions.
        if crate::features::labels::dispatch_label_mutation(&self.core.event_bus, m, full_sync) {
            return;
        }

        // Handle client-internal mutations that need persistence/presence access
        if m.index[0] == "setting_pushName"
            && let Some(val) = &m.action_value
            && let Some(act) = val.push_name_setting.as_option()
            && let Some(new_name) = &act.name
        {
            let new_name = new_name.clone();
            let bus = self.core.event_bus.clone();

            let snapshot = self.persistence_manager.get_device_snapshot();
            let old = snapshot.push_name.clone();
            if old != new_name {
                debug!(target: "Client/AppState", "Persisting push name from app state mutation: '{}' (old='{}')", new_name, old);
                self.persistence_manager
                    .process_command(DeviceCommand::SetPushName(new_name.clone()))
                    .await;
                bus.dispatch(Event::SelfPushNameUpdated(
                    crate::types::events::SelfPushNameUpdated::builder()
                        .from_server(true)
                        .old_name(old.clone())
                        .new_name(new_name.clone())
                        .build(),
                ));

                // WhatsApp Web sends presence immediately when receiving pushname
                if old.is_empty() && !new_name.is_empty() {
                    debug!(target: "Client/AppState", "Sending presence after receiving initial pushname from app state sync");
                    if let Err(e) = self.presence().set_available().await {
                        warn!(target: "Client/AppState", "Failed to send presence after pushname sync: {e:?}");
                    }
                }
            } else {
                debug!(target: "Client/AppState", "Push name mutation received but name unchanged: '{}'", new_name);
            }
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.clean_dirty", level = "debug", skip_all, fields(bit = ?bit), err(Debug)))]
    pub async fn clean_dirty_bits(
        &self,
        bit: wacore::iq::dirty::DirtyBit,
    ) -> Result<(), crate::request::IqError> {
        use wacore::iq::dirty::CleanDirtyBitsSpec;

        let spec = CleanDirtyBitsSpec::single(bit);
        self.execute(spec).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn key_arrival_finishes_before_a_slow_fanout() {
        let client = crate::test_utils::create_test_client_with_name("appstate_slow_peer").await;
        let backend = client.persistence_manager.backend();
        let key_id = vec![7, 8, 9, 10];
        let listener = client.initial_keys_synced_notifier.listen();
        let notifier = client.initial_keys_synced_notifier.clone();
        let writer = backend.clone();
        let stored_id = key_id.clone();
        let (fanout_polled_tx, fanout_polled_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            fanout_polled_rx.await.expect("fanout must be polled");
            writer
                .set_sync_key(
                    &stored_id,
                    crate::store::traits::AppStateSyncKey {
                        key_data: vec![7; 32],
                        ..Default::default()
                    },
                )
                .await
                .expect("store recovered key");
            notifier.notify(usize::MAX);
        });

        let slow_fanout = async move {
            let _ = fanout_polled_tx.send(());
            std::future::pending::<AppStateKeyRequestSchedule>().await
        };

        let progress = client
            .await_app_state_key_request(
                &*backend,
                std::slice::from_ref(&key_id),
                wacore::time::Instant::now() + Duration::from_secs(1),
                listener,
                slow_fanout,
            )
            .await;

        assert!(matches!(progress, AppStateKeyRequestProgress::KeysReady));
    }

    #[tokio::test]
    async fn passive_key_request_fanout_is_bounded() {
        async fn peer_request(
            device: u16,
            completes: bool,
        ) -> (u16, std::result::Result<(), anyhow::Error>) {
            if !completes {
                std::future::pending::<()>().await;
            }
            (device, Ok(()))
        }

        let client =
            crate::test_utils::create_test_client_with_name("appstate_fanout_timeout").await;
        let requests = futures::stream::FuturesUnordered::new();
        requests.push(peer_request(1, true));
        requests.push(peer_request(2, false));

        let delivery = tokio::time::timeout(
            Duration::from_secs(1),
            collect_app_state_key_request_results(
                &*client.runtime,
                requests,
                Duration::from_millis(20),
            ),
        )
        .await
        .expect("fanout collection must finish")
        .expect("one completed peer must preserve partial delivery");

        assert_eq!(delivery, AppStateKeyRequestDelivery::SomePeers);
    }

    #[test]
    fn empty_companion_discovery_falls_back_to_primary() {
        let primary: Jid = "5511000000000@s.whatsapp.net".parse().expect("primary jid");
        let peers = finalize_app_state_key_request_peers(Vec::new(), 7, primary.clone())
            .expect("companion fallback");
        assert_eq!(peers, vec![primary.clone()]);
        assert!(finalize_app_state_key_request_peers(Vec::new(), 0, primary).is_err());
    }

    #[test]
    fn app_state_peers_use_the_own_pn_namespace() {
        let primary = Jid::pn("5511000000000");
        let peers = finalize_app_state_key_request_peers(
            vec![
                Jid::lid_device("100000000000001", 0),
                Jid::lid_device("100000000000001", 7),
                Jid::pn_device("5511000000000", 7),
            ],
            33,
            primary.clone(),
        )
        .expect("peer devices");

        assert_eq!(peers, vec![primary, Jid::pn_device("5511000000000", 7)]);
    }

    #[tokio::test]
    async fn active_key_wait_shortens_a_passive_dedup_stamp() {
        let client = crate::test_utils::create_test_client_with_name("appstate_retry_stamp").await;
        let key_id = vec![1, 2, 3, 4];
        client.app_state_key_requests.lock().await.insert(
            key_id.clone(),
            wacore::time::Instant::now() + APP_STATE_KEY_REQUEST_DEDUP,
        );

        let started = wacore::time::Instant::now();
        let schedule = client
            .request_missing_keys_with_dedup(
                std::slice::from_ref(&key_id),
                APP_STATE_KEY_PARTIAL_RETRY,
            )
            .await;

        assert!(
            !schedule.sent,
            "an in-flight request must not be duplicated"
        );
        assert!(schedule.retry_at > started);
        assert!(
            schedule.retry_at.saturating_duration_since(started)
                <= APP_STATE_KEY_PARTIAL_RETRY + Duration::from_millis(100),
            "an active waiter must retry before the passive 24-hour deadline"
        );
        assert_eq!(
            client
                .app_state_key_requests
                .lock()
                .await
                .get(key_id.as_slice())
                .copied(),
            Some(schedule.retry_at)
        );
    }

    #[test]
    fn ordinary_key_wait_leaves_time_for_a_retry() {
        let retry = initial_app_state_key_retry(APP_STATE_KEY_REQUEST_TIMEOUT);

        assert_eq!(retry, Duration::from_secs(5));
        assert!(retry < APP_STATE_KEY_REQUEST_TIMEOUT);
        assert_eq!(
            initial_app_state_key_retry(Duration::from_secs(180)),
            APP_STATE_KEY_PARTIAL_RETRY
        );
    }
}

// ─── #1157: the app-state send path must read the server's answer ───────────
//
// `w:sync:app:state` enforces optimistic concurrency on the collection's
// `version`. A patch built against a stale base is not rejected at the IQ
// level: the IQ succeeds and the failure is reported *inside* it, as
// `<collection type="error"><error code="409"/>`, carrying the patches that
// won. WA Web reads exactly that (`WAWebSyncdResponseParser`, fn `h`) and maps
// it onto `CollectionState.Conflict{,HasMore}`; the collection then goes
// through `applyAppStateSyncResponse` like any other, and `serverSync` re-queues
// it for another round as long as pending mutations remain — so the mutation is
// re-sent on the winner's base instead of being dropped. `400`/`404` map to
// `ErrorFatal`, anything else to `ErrorRetry`.
//
// These tests pin what the send path must make of each response shape: a 409 it
// can resolve (rebuild and resend), a 409 it cannot (an error, after exhausting
// the rebuild attempts), a fatal code (an error, not retried), and a response
// carrying no collection verdict at all (accepted). Discarding the response —
// which is what made a 409 indistinguishable from success — fails all four.
#[cfg(test)]
mod send_patch_response_tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use wacore_binary::node::Node;

    /// Seed the client's store with an app-state key so `build_patch` can sign,
    /// and give the collection a non-zero base so the IQ carries a `version`.
    async fn seed_collection(client: &Arc<Client>, collection: &str) -> Vec<u8> {
        let backend = client.persistence_manager.backend();
        let key_id = b"send-patch-key".to_vec();
        backend
            .set_sync_key(
                &key_id,
                crate::store::traits::AppStateSyncKey {
                    key_data: vec![5u8; 32],
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept a sync key");
        backend
            .set_version(
                collection,
                wacore::appstate::hash::HashState {
                    version: 7,
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept a version");
        key_id
    }

    /// A `<collection>` the server marks as failed, mirroring the shape
    /// `WAWebSyncdResponseParser` reads.
    fn collection_error_result(request_id: &str, collection: &str, code: &str) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", request_id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("sync")
                .children([NodeBuilder::new("collection")
                    .attr("name", collection)
                    .attr("type", "error")
                    .children([NodeBuilder::new("error")
                        .attr("code", code)
                        .attr("text", "")
                        .build()])
                    .build()])
                .build()])
            .build()
    }

    /// A collection the server reports as clean and up to date.
    fn empty_sync_result(request_id: &str, collection: &str) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", request_id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("sync")
                .children([NodeBuilder::new("collection")
                    .attr("name", collection)
                    .build()])
                .build()])
            .build()
    }

    const COLLECTION: &str = "regular_low";

    /// Answers every IQ the client writes, in order, with whatever `reply`
    /// returns for it — `Some(code)` for a `<collection type="error">`, `None`
    /// for a clean result. Runs forever: callers race it against the send, so a
    /// send that stops writing simply drops this future.
    ///
    /// `reply` is told the send-attempt number for patch IQs (0 for the
    /// re-syncs in between), which is what lets a test answer "conflict once,
    /// then accept".
    async fn serve_iqs(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        patch_attempts: &AtomicUsize,
        response_collection: &str,
        mut reply: impl FnMut(usize) -> Option<&'static str>,
    ) {
        let mut frame = 0usize;
        loop {
            let node = crate::test_utils::decode_sent_iq(transport, frame).await;
            let node = node.get().to_owned();
            let id = node
                .attrs()
                .optional_string("id")
                .expect("every IQ carries an id")
                .into_owned();
            let attempt = if node
                .get_optional_child_by_tag(&["sync", "collection", "patch"])
                .is_some()
            {
                patch_attempts.fetch_add(1, Ordering::Relaxed) + 1
            } else {
                0
            };
            let response = match reply(attempt) {
                Some(code) => collection_error_result(&id, response_collection, code),
                None => empty_sync_result(&id, response_collection),
            };
            crate::test_utils::answer_iq(client, &id, &response).await;
            frame += 1;
        }
    }

    /// Drives one `send_app_state_patch` to completion against `reply`, and
    /// reports how many patch IQs reached the wire.
    async fn send_against(reply: impl FnMut(usize) -> Option<&'static str>) -> (Result<()>, usize) {
        send_against_collection(COLLECTION, reply).await
    }

    async fn send_against_collection(
        response_collection: &'static str,
        reply: impl FnMut(usize) -> Option<&'static str>,
    ) -> (Result<()>, usize) {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        seed_collection(&client, COLLECTION).await;

        let mut send = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .send_app_state_patch(COLLECTION, vec![wa::SyncdMutation::default()])
                    .await
            })
        };

        let patch_attempts = AtomicUsize::new(0);
        let server = serve_iqs(
            &client,
            &transport,
            &patch_attempts,
            response_collection,
            reply,
        );
        futures::pin_mut!(server);
        let result = futures::select! {
            result = (&mut send).fuse() => result.expect("the send task should not panic"),
            () = server.as_mut().fuse() => unreachable!("the responder never completes"),
        };

        (result, patch_attempts.load(Ordering::Relaxed))
    }

    #[tokio::test]
    async fn response_for_a_different_collection_is_rejected() {
        for error in [None, Some("409")] {
            let (result, patches) = send_against_collection("regular_high", move |_| error).await;
            assert!(
                result.is_err(),
                "a response for another collection must not accept or absorb this send"
            );
            assert_eq!(
                patches, 1,
                "a mismatched response must fail before retrying the mutation"
            );
        }
    }

    /// A 409 means the patch was built on a stale base and did NOT land. A
    /// server that keeps rejecting must end as an error, never as success — a
    /// `markChatAsRead` that silently lost must not be reported as done.
    #[tokio::test]
    async fn unresolvable_conflict_is_not_reported_as_success() {
        let (result, patches) = send_against(|_| Some("409")).await;
        assert!(
            result.is_err(),
            "a 409 conflict means the mutation was dropped; reporting Ok hides the loss"
        );
        assert_eq!(
            patches, APP_STATE_PATCH_SEND_ATTEMPTS,
            "the send must exhaust its rebuild attempts before giving up"
        );
    }

    /// The resolution path: the first attempt loses the race, the client
    /// rebuilds against the new base, and the second attempt lands. That is WA
    /// Web's conflict loop, and the mutation survives it.
    #[tokio::test]
    async fn conflict_is_resolved_by_rebuilding_and_resending() {
        let (result, patches) =
            send_against(|attempt| if attempt == 1 { Some("409") } else { None }).await;
        result.expect("a conflict the server later accepts must succeed, not fail");
        assert_eq!(
            patches, 2,
            "the losing patch must be rebuilt and re-sent exactly once"
        );
    }

    /// A bare `<iq type="result"/>` carries no per-collection verdict, so there
    /// is nothing to reject: reading the response must not turn a peer that
    /// answers tersely into a failing send.
    #[tokio::test]
    async fn response_without_a_collection_verdict_is_accepted() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        seed_collection(&client, COLLECTION).await;

        let mut send = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .send_app_state_patch(COLLECTION, vec![wa::SyncdMutation::default()])
                    .await
            })
        };

        let bare = async {
            let mut frame = 0usize;
            loop {
                let node = crate::test_utils::decode_sent_iq(&transport, frame).await;
                let id = node
                    .get()
                    .attrs()
                    .optional_string("id")
                    .expect("every IQ carries an id")
                    .into_owned();
                crate::test_utils::answer_iq(
                    &client,
                    &id,
                    &NodeBuilder::new("iq")
                        .attr("type", "result")
                        .attr("id", &id)
                        .attr("from", "s.whatsapp.net")
                        .build(),
                )
                .await;
                frame += 1;
            }
        };
        futures::pin_mut!(bare);

        let result = futures::select! {
            result = (&mut send).fuse() => result.expect("the send task should not panic"),
            () = bare.as_mut().fuse() => unreachable!("the responder never completes"),
        };
        result.expect("a terse but successful response must not read as a rejection");
    }

    /// A `<collection>` that IS present but does not parse may be carrying the
    /// rejection. Manufacturing an empty success from it would drop the
    /// mutation exactly as discarding the response did.
    #[tokio::test]
    async fn unreadable_collection_is_not_mistaken_for_an_absent_one() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        seed_collection(&client, COLLECTION).await;

        let mut send = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .send_app_state_patch(COLLECTION, vec![wa::SyncdMutation::default()])
                    .await
            })
        };

        let malformed = async {
            let mut frame = 0usize;
            loop {
                let node = crate::test_utils::decode_sent_iq(&transport, frame).await;
                let id = node
                    .get()
                    .attrs()
                    .optional_string("id")
                    .expect("every IQ carries an id")
                    .into_owned();
                // A collection with no `name`: present, unreadable.
                crate::test_utils::answer_iq(
                    &client,
                    &id,
                    &NodeBuilder::new("iq")
                        .attr("type", "result")
                        .attr("id", &id)
                        .attr("from", "s.whatsapp.net")
                        .children([NodeBuilder::new("sync")
                            .children([NodeBuilder::new("collection")
                                .attr("type", "error")
                                .build()])
                            .build()])
                        .build(),
                )
                .await;
                frame += 1;
            }
        };
        futures::pin_mut!(malformed);

        let result = futures::select! {
            result = (&mut send).fuse() => result.expect("the send task should not panic"),
            () = malformed.as_mut().fuse() => unreachable!("the responder never completes"),
        };
        assert!(
            result.is_err(),
            "a collection we cannot read may be the rejection; it must not read as success"
        );
    }

    /// 400/404 are `ErrorFatal` in WA Web and `ErrAppStateUpdate` in whatsmeow —
    /// never success, and never retried.
    #[tokio::test]
    async fn fatal_collection_error_is_not_reported_as_success() {
        let (result, patches) = send_against(|_| Some("400")).await;
        assert!(
            result.is_err(),
            "a fatal collection error must surface to the caller, not read as success"
        );
        assert_eq!(patches, 1, "a fatal error must not be retried");
    }
}

#[cfg(test)]
mod sync_in_flight_tests {
    use super::*;

    /// A consumer-issued full sync must not run alongside a sync already
    /// writing the collection's version and mutation MACs — and must not be
    /// dropped either, since the snapshot it asks for is not what an
    /// incremental sync in flight is fetching.
    #[tokio::test]
    async fn a_full_sync_task_waits_for_the_collection() {
        let client = crate::test_utils::create_test_client_with_name("appstate-task-wait").await;
        let held = client
            .app_state_syncing
            .try_begin(WAPatchName::CriticalBlock)
            .expect("reserve the collection first");

        let task = tokio::spawn({
            let client = Arc::clone(&client);
            async move {
                client
                    .process_sync_task(MajorSyncTask::AppStateSync {
                        name: WAPatchName::CriticalBlock,
                        full_sync: true,
                    })
                    .await;
            }
        });

        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        assert!(
            !task.is_finished(),
            "the task ran while the collection was reserved"
        );
        assert_eq!(
            client.app_state_syncing.len(),
            1,
            "only the held reservation"
        );

        drop(held);
        // The client has no socket, so the sync itself fails fast with
        // NotConnected; what matters is that it got to run and released.
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("released collection must let the task proceed")
            .expect("task panicked");
        assert_eq!(
            client.app_state_syncing.len(),
            0,
            "the task's own reservation must be released"
        );
    }

    /// An incremental task waits too: the holder may be a patch send, which
    /// never fetches, so skipping would drop the requested sync entirely.
    #[tokio::test]
    async fn an_incremental_sync_task_also_waits() {
        let client = crate::test_utils::create_test_client_with_name("appstate-task-skip").await;
        let held = client
            .app_state_syncing
            .try_begin(WAPatchName::Regular)
            .expect("reserve the collection first");

        let task = tokio::spawn({
            let client = Arc::clone(&client);
            async move {
                client
                    .process_sync_task(MajorSyncTask::AppStateSync {
                        name: WAPatchName::Regular,
                        full_sync: false,
                    })
                    .await;
            }
        });

        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        assert!(
            !task.is_finished(),
            "the task ran while the collection was reserved"
        );

        drop(held);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("released collection must let the task proceed")
            .expect("task panicked");
        assert_eq!(client.app_state_syncing.len(), 0, "reservation released");
    }

    #[test]
    fn second_begin_blocked_until_release() {
        let registry = SyncInFlight::new();
        let guard = registry
            .try_begin(WAPatchName::Regular)
            .expect("first begin must reserve");
        assert!(
            registry.try_begin(WAPatchName::Regular).is_none(),
            "in-flight collection must dedup"
        );
        // Other collections are independent.
        assert!(registry.try_begin(WAPatchName::CriticalBlock).is_some());

        drop(guard);
        assert!(
            registry.try_begin(WAPatchName::Regular).is_some(),
            "release (including cancellation drop) must free the slot"
        );
    }

    #[test]
    fn stale_guard_does_not_clobber_new_generation() {
        let registry = SyncInFlight::new();
        // Generation 1 reserves, then a reconnect clears the registry while
        // the task is still in flight.
        let stale = registry
            .try_begin(WAPatchName::Regular)
            .expect("gen-1 reserve");
        registry.clear();

        // Generation 2 reserves the same collection.
        let fresh = registry
            .try_begin(WAPatchName::Regular)
            .expect("post-clear reserve");

        // The stale task finishing must NOT evict generation 2's reservation.
        drop(stale);
        assert!(
            registry.try_begin(WAPatchName::Regular).is_none(),
            "stale release clobbered the new generation's reservation"
        );

        drop(fresh);
        assert!(registry.try_begin(WAPatchName::Regular).is_some());
    }

    /// A patch send cannot treat "already in flight" as "nothing to do": it has
    /// to write the same version and mutation-MAC rows the sync writes, so it
    /// waits for the holder instead of skipping.
    #[tokio::test]
    async fn begin_waits_for_the_holder_instead_of_skipping() {
        let registry = SyncInFlight::new();
        let held = registry
            .try_begin(WAPatchName::Regular)
            .expect("first reserve");

        let (reserved_tx, mut reserved_rx) = tokio::sync::oneshot::channel();
        let waiter = {
            let registry = Arc::clone(&registry);
            tokio::spawn(async move {
                let guard = registry.begin(WAPatchName::Regular, SyncHolder::Sync).await;
                let _ = reserved_tx.send(());
                guard
            })
        };

        // A parked listener is proof the waiter reached its await point — the
        // observable a "still waiting" assertion needs instead of a sleep.
        crate::test_utils::poll_until("the waiter to park on the registry", || {
            registry.released.total_listeners() >= 1
        })
        .await;
        assert!(
            reserved_rx.try_recv().is_err(),
            "begin must not resolve while the collection is held"
        );

        drop(held);
        let guard = waiter.await.expect("the waiter should not panic");
        assert!(
            registry.try_begin(WAPatchName::Regular).is_none(),
            "the waiter must now hold the reservation, not merely have observed it free"
        );

        drop(guard);
        assert!(registry.try_begin(WAPatchName::Regular).is_some());
    }
}

#[cfg(test)]
pub(crate) mod batched_sync_outcome_tests {
    use super::*;
    use wacore_binary::node::Node;

    /// One `<iq result>` answering a whole batch, with each collection either
    /// clean or carrying an `<error code>` — the shape
    /// `WAWebSyncdResponseParser` reads.
    pub(crate) fn batch_result(request_id: &str, collections: &[(&str, Option<&str>)]) -> Node {
        let children: Vec<Node> = collections
            .iter()
            .map(|(name, error)| {
                let builder = NodeBuilder::new("collection").attr("name", *name);
                match error {
                    Some(code) => builder
                        .attr("type", "error")
                        .children([NodeBuilder::new("error")
                            .attr("code", *code)
                            .attr("text", "")
                            .build()])
                        .build(),
                    None => builder.build(),
                }
            })
            .collect();
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", request_id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("sync").children(children).build()])
            .build()
    }

    /// Runs one batched sync against a server that answers every IQ with
    /// `collections`, and reports the outcome plus how many IQs reached the
    /// wire. The responder never completes, so it is raced against the sync.
    pub(crate) async fn sync_against(
        request: Vec<WAPatchName>,
        collections: &'static [(&'static str, Option<&'static str>)],
    ) -> (BatchedSyncOutcome, usize) {
        use futures::FutureExt;
        let (client, transport) = crate::test_utils::create_iq_test_client().await;

        let mut sync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                let scope = client.sync_scope(None);
                client.sync_collections_batched(request, scope).await
            })
        };

        let sent = AtomicU64::new(0);
        let server = async {
            let mut frame = 0usize;
            loop {
                let node = crate::test_utils::decode_sent_iq(&transport, frame).await;
                let node = node.get().to_owned();
                let id = node
                    .attrs()
                    .optional_string("id")
                    .expect("every IQ carries an id")
                    .into_owned();
                sent.fetch_add(1, Ordering::Relaxed);
                let response = batch_result(&id, collections);
                crate::test_utils::answer_iq(&client, &id, &response).await;
                frame += 1;
            }
        };
        futures::pin_mut!(server);
        let outcome = futures::select! {
            result = (&mut sync).fuse() => result
                .expect("the sync task should not panic")
                .expect("a per-collection error is an outcome, not a transport failure"),
            () = server.as_mut().fuse() => unreachable!("the responder never completes"),
        };

        (outcome, sent.load(Ordering::Relaxed) as usize)
    }

    /// A refused collection used to be logged and dropped, and the batch still
    /// reported success — which the initial bootstrap reads as permission to
    /// dispatch Connected.
    #[tokio::test]
    async fn a_refused_collection_is_reported_fatal_not_synced() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::CriticalBlock, WAPatchName::CriticalUnblockLow],
            &[
                ("critical_block", Some("404")),
                ("critical_unblock_low", None),
            ],
        )
        .await;

        assert_eq!(outcome.fatal, vec![WAPatchName::CriticalBlock]);
        assert_eq!(outcome.synced, vec![WAPatchName::CriticalUnblockLow]);
        assert!(!outcome.all_synced(), "the batch did not fully sync");
    }

    /// A retryable collection is done for this run. WA Web routes ErrorRetry to
    /// `doneCollections`, never to `refetchCollections`, so it must not be
    /// re-asked inside the same loop — with a 500-iteration cap that would mean
    /// hammering a failing collection 500 times.
    #[tokio::test]
    async fn a_retryable_collection_is_not_refetched_in_the_same_run() {
        let (outcome, iqs) =
            sync_against(vec![WAPatchName::Regular], &[("regular", Some("500"))]).await;

        assert_eq!(outcome.retryable, vec![WAPatchName::Regular]);
        assert!(outcome.fatal.is_empty(), "500 is not terminal");
        assert_eq!(iqs, 1, "a retryable error must not be re-asked in this run");
    }

    /// The batch reported `Ok(())` when every collection was already in flight,
    /// which reads identically to "all synced" at the call site.
    #[tokio::test]
    async fn collections_held_by_another_sync_are_reported_skipped() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let _held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::CriticalBlock, SyncHolder::Sync)
            .expect("reserve the collection first");

        let outcome = client
            .sync_collections_batched(vec![WAPatchName::CriticalBlock], client.sync_scope(None))
            .await
            .expect("skipping is an outcome, not an error");

        assert_eq!(outcome.skipped, vec![WAPatchName::CriticalBlock]);
        assert!(outcome.synced.is_empty());
        assert!(!outcome.all_synced(), "a skipped collection did not sync");
        assert!(
            transport.sent().is_empty(),
            "a skipped batch must not reach the wire"
        );
    }

    /// A patch send holds the same reservation and never fetches, so a sync
    /// that skipped behind one would be dropped and the patches that prompted
    /// it would go unfetched.
    #[tokio::test]
    async fn a_patch_send_holder_is_waited_out_not_skipped() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::Regular, SyncHolder::PatchSend)
            .expect("reserve the collection first");

        let reserve = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .reserve_for_sync(
                        WAPatchName::Regular,
                        ReservationWait::SkipBehindSync,
                        client.sync_scope(None),
                    )
                    .await
                    .map(drop)
            })
        };

        crate::test_utils::poll_until("the sync to park behind the patch send", || {
            client.app_state_syncing.released.total_listeners() >= 1
        })
        .await;
        assert!(
            !reserve.is_finished(),
            "a sync must wait for a patch send, not skip it"
        );

        drop(held);
        tokio::time::timeout(Duration::from_secs(5), reserve)
            .await
            .expect("releasing the send must let the sync proceed")
            .expect("the reserve task should not panic")
            .expect("the sync must get the reservation, not time out");
    }

    #[test]
    fn all_synced_is_false_for_every_kind_of_miss() {
        let mut outcome = BatchedSyncOutcome::default();
        assert!(outcome.all_synced(), "an empty batch missed nothing");

        outcome.synced.push(WAPatchName::Regular);
        assert!(outcome.all_synced());

        let buckets: [fn(&mut BatchedSyncOutcome) -> &mut Vec<WAPatchName>; 3] =
            [|o| &mut o.fatal, |o| &mut o.retryable, |o| &mut o.skipped];
        for bucket in buckets {
            bucket(&mut outcome).push(WAPatchName::CriticalBlock);
            assert!(!outcome.all_synced());
            bucket(&mut outcome).clear();
        }
    }
}

#[cfg(test)]
mod batched_sync_reconciliation_tests {
    use super::*;
    use crate::client::app_state::batched_sync_outcome_tests::{batch_result, sync_against};

    /// A `<sync>` that simply leaves a requested collection out parses fine, so
    /// nothing used to record it and `all_synced()` reported a batch that never
    /// covered it — the same false success this module exists to stop.
    #[tokio::test]
    async fn a_collection_the_response_omits_is_not_reported_synced() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::CriticalBlock, WAPatchName::CriticalUnblockLow],
            &[("critical_unblock_low", None)],
        )
        .await;

        assert_eq!(outcome.synced, vec![WAPatchName::CriticalUnblockLow]);
        assert_eq!(
            outcome.retryable,
            vec![WAPatchName::CriticalBlock],
            "an omitted collection is a miss, not a success"
        );
        assert!(!outcome.all_synced());
    }

    /// An empty `<sync/>` accounts for nothing at all.
    #[tokio::test]
    async fn an_empty_response_leaves_every_collection_unsynced() {
        let (outcome, _) = sync_against(vec![WAPatchName::Regular], &[]).await;

        assert!(outcome.synced.is_empty());
        assert_eq!(outcome.retryable, vec![WAPatchName::Regular]);
        assert!(!outcome.all_synced());
    }

    /// A repeated collection must be applied once, not twice.
    #[tokio::test]
    async fn a_repeated_collection_is_counted_once() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::Regular],
            &[("regular", None), ("regular", None)],
        )
        .await;

        assert_eq!(outcome.synced, vec![WAPatchName::Regular]);
        assert!(outcome.all_synced());
    }

    /// A wait that runs out leaves the collection uncovered by anyone, so it is
    /// a miss worth retrying rather than a skip someone else is handling.
    ///
    /// Paused time so the bound actually elapses instead of the test sitting
    /// there for [`APP_STATE_RESERVATION_WAIT`]; the holder never releases, so
    /// the timeout is the only way out.
    #[tokio::test(start_paused = true)]
    async fn a_wait_that_runs_out_reports_the_collection_retryable() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        // A patch send is never equivalent work, so the batched sync waits for
        // it rather than skipping — which is what puts the bound in play.
        let _held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::Regular, SyncHolder::PatchSend)
            .expect("reserve the collection first");

        let outcome = client
            .sync_collections_batched(vec![WAPatchName::Regular], client.sync_scope(None))
            .await
            .expect("a wait that ran out is an outcome, not a transport failure");

        assert_eq!(
            outcome.retryable,
            vec![WAPatchName::Regular],
            "nobody is covering it, so it has to come back around"
        );
        assert!(
            outcome.skipped.is_empty(),
            "skipped means an equivalent sync has it, which is not the case here"
        );
        assert!(
            transport.sent().is_empty(),
            "the sync never got its turn, so nothing should reach the wire"
        );
    }

    /// `batch_result` is shared with the outcome tests; this keeps the helper
    /// honest about the shape it builds.
    #[test]
    fn batch_result_marks_errors_on_the_named_collection() {
        let node = batch_result("id-1", &[("regular", Some("500"))]);
        let collection = node
            .get_optional_child_by_tag(&["sync", "collection"])
            .expect("the helper builds sync/collection");
        assert_eq!(
            collection.attrs().optional_string("type").as_deref(),
            Some("error")
        );
    }
}

#[cfg(test)]
mod duplicate_collection_tests {
    use super::*;
    use crate::client::app_state::batched_sync_outcome_tests::sync_against;

    /// The processor persists every list it is handed, so a repeated collection
    /// has to be dropped before it is processed, not reconciled afterwards.
    /// Reconciling late still applies the collection twice, and the second
    /// application can move the MAC store past the version the first writes
    /// back.
    #[tokio::test]
    async fn a_duplicate_collection_is_dropped_before_it_is_applied() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::Regular],
            &[("regular", None), ("regular", None)],
        )
        .await;

        assert_eq!(
            outcome.synced,
            vec![WAPatchName::Regular],
            "the collection is accounted for exactly once"
        );
        assert!(outcome.all_synced());
    }

    /// A duplicate must not make the batch look incomplete either: it is the
    /// same collection, already answered.
    #[tokio::test]
    async fn a_duplicate_does_not_leave_the_batch_unsynced() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::CriticalBlock, WAPatchName::CriticalUnblockLow],
            &[
                ("critical_block", None),
                ("critical_block", None),
                ("critical_unblock_low", None),
            ],
        )
        .await;

        assert!(outcome.retryable.is_empty(), "both were answered");
        assert!(outcome.all_synced());
    }
}

#[cfg(test)]
mod background_report_tests {
    use super::*;
    use crate::types::events::{EventHandler, EventInterest, EventKind};

    struct FailureCounter(Arc<AtomicU64>);

    impl EventHandler for FailureCounter {
        fn handle_event(&self, event: Arc<Event>) {
            if matches!(&*event, Event::AppStateSyncFailed(_)) {
                self.0.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// A background sync awaits a round trip before it reports, so its outcome
    /// can belong to a socket that has since been replaced. Publishing it would
    /// hand a consumer a refusal whose documented response — logout, or forcing
    /// a recovery — would land on the healthy session that took its place.
    #[tokio::test]
    async fn an_outcome_from_a_retired_connection_is_not_published() {
        let client = crate::test_utils::create_test_client_with_name("bg-report-gen").await;
        let retired_scope = client.sync_scope(None);

        let seen = Arc::new(AtomicU64::new(0));
        let _subscription = client.subscribe(
            EventInterest::of(&[EventKind::AppStateSyncFailed]),
            Arc::new(FailureCounter(Arc::clone(&seen))),
        );

        let mut outcome = BatchedSyncOutcome::default();
        outcome.fatal.push(WAPatchName::CriticalBlock);

        // The connection this sync belonged to is gone.
        client
            .connection_generation
            .store(retired_scope.generation() + 1, Ordering::SeqCst);
        client.report_background_sync(
            "test",
            retired_scope,
            SyncSettles::JustTheCollections,
            &[],
            Ok(outcome.clone()),
        );
        assert_eq!(
            seen.load(Ordering::Relaxed),
            0,
            "a retired connection's refusal must not reach consumers"
        );

        // The live one still reports.
        client.report_background_sync(
            "test",
            client.sync_scope(None),
            SyncSettles::JustTheCollections,
            &[],
            Ok(outcome),
        );
        assert_eq!(seen.load(Ordering::Relaxed), 1);
    }
}

#[cfg(test)]
mod retry_gate_tests {
    use super::*;

    /// The bootstrap gate stands down on proof of sync, not on an empty
    /// retryable bucket. A round that ends in a refusal, or behind another sync,
    /// also leaves nothing to retry while the collection is still not synced,
    /// and clearing there lets the next connection skip the only thing that
    /// guarantees another attempt.
    #[test]
    fn only_a_fully_synced_outcome_settles_the_bootstrap() {
        let mut fatal = BatchedSyncOutcome::default();
        fatal.fatal.push(WAPatchName::CriticalBlock);
        assert!(fatal.retryable.is_empty(), "nothing left to retry");
        assert!(
            !fatal.all_synced(),
            "but the collection is not synced, so the gate must stay armed"
        );

        let mut skipped = BatchedSyncOutcome::default();
        skipped.skipped.push(WAPatchName::CriticalBlock);
        assert!(skipped.retryable.is_empty());
        assert!(
            !skipped.all_synced(),
            "another sync holding it is not proof it succeeded"
        );

        let mut synced = BatchedSyncOutcome::default();
        synced.synced.push(WAPatchName::CriticalBlock);
        assert!(synced.all_synced(), "this is the only case that settles it");
    }

    // Not covered: that the scheduler refuses to run for a retired generation.
    // Two attempts at it passed with the guard removed — the gate stays armed
    // because the wrongly-attempted sync fails anyway on a socketless client,
    // and the attempt never reaches the capturing transport either. Asserting
    // on the gate or on the wire both hold for the wrong reason, and a test
    // that passes without the fix is worse than none. It needs a connected
    // fixture that can complete a sync, which the report-path test below
    // already has for its own case.

    /// The backoff doubles from the minimum and clamps, matching the syncd
    /// spacing WA Web applies to the same case.
    #[test]
    fn retry_backoff_doubles_then_clamps() {
        assert_eq!(app_state_retry_backoff(0), APP_STATE_RETRY_BACKOFF_MIN);
        assert_eq!(app_state_retry_backoff(1), APP_STATE_RETRY_BACKOFF_MIN * 2);
        assert_eq!(app_state_retry_backoff(3), APP_STATE_RETRY_BACKOFF_MIN * 8);
        assert_eq!(
            app_state_retry_backoff(u32::MAX),
            APP_STATE_RETRY_BACKOFF_MAX,
            "an absurd round must clamp, not overflow"
        );
    }
}

#[cfg(test)]
mod request_hygiene_tests {
    use super::*;
    use crate::client::app_state::batched_sync_outcome_tests::sync_against;

    /// A `server_sync` notification can repeat a `<collection>` child. Reserving
    /// the name once and then tripping over our own reservation on the second
    /// pass filed one collection under both `synced` and `skipped`, which made
    /// `all_synced()` false and published a failure blaming a writer that never
    /// existed.
    #[tokio::test]
    async fn a_collection_requested_twice_is_reserved_once() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::Regular, WAPatchName::Regular],
            &[("regular", None)],
        )
        .await;

        assert_eq!(outcome.synced, vec![WAPatchName::Regular]);
        assert!(
            outcome.skipped.is_empty(),
            "the only holder was this same call"
        );
        assert!(outcome.all_synced());
    }

    /// Nothing reserved a collection we did not ask for, so applying it can
    /// interleave with a concurrent writer for that collection — and it would
    /// dispatch mutations nobody requested.
    #[tokio::test]
    async fn an_unrequested_collection_in_the_response_is_dropped() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::Regular],
            &[("regular", None), ("critical_block", None)],
        )
        .await;

        assert_eq!(outcome.synced, vec![WAPatchName::Regular]);
        assert!(
            !outcome.synced.contains(&WAPatchName::CriticalBlock),
            "an unrequested collection must not be applied or reported"
        );
        assert!(outcome.all_synced());
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;

    /// An expired deadline stops the batch before it reserves anything, so no
    /// collection is reported synced and nothing reaches the wire.
    ///
    /// Covers the pre-reservation check only. The second check — after the IQ
    /// returns and before the response is applied — needs a deadline that
    /// expires mid-round, which takes a responder driving a paused clock; that
    /// path is unasserted.
    #[tokio::test]
    async fn an_expired_deadline_stops_the_batch_before_it_reserves() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;

        let sync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                let scope = client.sync_scope(Some(wacore::time::Instant::now()));
                client
                    .sync_collections_batched(vec![WAPatchName::Regular], scope)
                    .await
            })
        };

        let outcome = tokio::time::timeout(Duration::from_secs(5), sync)
            .await
            .expect("an expired deadline must not block")
            .expect("the sync task should not panic")
            .expect("a deadline is an outcome, not a transport failure");

        assert_eq!(outcome.retryable, vec![WAPatchName::Regular]);
        assert!(outcome.synced.is_empty());
        assert!(
            transport.sent().is_empty(),
            "nothing may reach the wire past the deadline"
        );
    }
}

#[cfg(test)]
mod sync_scope_tests {
    use super::*;

    /// The predicate every boundary asks. Both answers matter and they are not
    /// interchangeable: a retired scope must never write or publish, while an
    /// expired one is simply out of time on a connection that is still live.
    #[tokio::test]
    async fn a_scope_stops_admitting_when_its_connection_or_clock_goes() {
        let client = crate::test_utils::create_test_client_with_name("scope-admits").await;

        let live = client.sync_scope(None);
        assert_eq!(client.admits(live), Ok(()));

        let expired = client.sync_scope(Some(wacore::time::Instant::now()));
        assert_eq!(client.admits(expired), Err(ScopeLost::Expired));

        let generous = client.sync_scope(Some(
            wacore::time::Instant::now() + Duration::from_secs(600),
        ));
        assert_eq!(client.admits(generous), Ok(()));

        client
            .connection_generation
            .store(live.generation() + 1, Ordering::SeqCst);
        assert_eq!(client.admits(live), Err(ScopeLost::Retired));
        assert_eq!(
            client.admits(generous),
            Err(ScopeLost::Retired),
            "a retired connection outranks having time left"
        );
    }

    /// The bootstrap gate is shared across connections, so a task from a retired
    /// one must not touch it in either direction: clearing lets the live
    /// connection skip a bootstrap it still needs, arming costs it one it does
    /// not. Routing every write through `settle_bootstrap` is what makes that
    /// check impossible to forget — it was forgotten twice when it was the
    /// caller's job.
    #[tokio::test]
    async fn a_retired_scope_cannot_move_the_bootstrap_gate() {
        let client = crate::test_utils::create_test_client_with_name("scope-gate").await;
        let retired = client.sync_scope(None);
        client
            .connection_generation
            .store(retired.generation() + 1, Ordering::SeqCst);

        for armed in [true, false] {
            // Seeded as the connection that is live at seeding time, so the
            // retired scope below is genuinely outranked rather than merely
            // equal to the tag it left behind.
            client
                .needs_initial_full_sync
                .settle(client.connection_generation.load(Ordering::SeqCst), armed);
            client.settle_bootstrap(retired, !armed);
            assert_eq!(
                client.needs_initial_full_sync.is_armed(),
                armed,
                "a retired scope must leave the gate exactly as it found it"
            );
        }

        // The live connection still owns it, both ways.
        let live = client.sync_scope(None);
        client.settle_bootstrap(live, true);
        assert!(client.needs_initial_full_sync.is_armed());
        client.settle_bootstrap(live, false);
        assert!(!client.needs_initial_full_sync.is_armed());
    }

    /// An expired scope still owns its connection, so it may settle the gate —
    /// running out of time is exactly when the bootstrap needs to stay armed.
    #[tokio::test]
    async fn an_expired_scope_may_still_arm_the_gate() {
        let client = crate::test_utils::create_test_client_with_name("scope-expired-gate").await;
        let expired = client.sync_scope(Some(wacore::time::Instant::now()));
        client
            .needs_initial_full_sync
            .settle(expired.generation(), false);

        client.settle_bootstrap(expired, true);
        assert!(
            client.needs_initial_full_sync.is_armed(),
            "an expired bootstrap is unfinished, and must say so"
        );
    }

    /// Rebinding is what lets work outlive a planned reconnect, and it reports
    /// whether it moved so the caller can drop the authority that does not
    /// carry over with it.
    #[tokio::test]
    async fn rebinding_reports_whether_the_connection_moved() {
        let client = crate::test_utils::create_test_client_with_name("scope-rebind").await;
        let mut scope = client.sync_scope(None);
        let original = scope.generation();

        assert!(!scope.rebind(original), "same connection is not a move");
        assert_eq!(client.admits(scope), Ok(()));

        assert!(scope.rebind(original + 1), "a different connection is");
        assert_eq!(scope.generation(), original + 1);
    }

    /// A scope with no deadline is a background trigger; one with a deadline is
    /// the bootstrap. That distinction decides whether the batch waits behind an
    /// equivalent sync or skips it, so it has to be readable from the scope
    /// alone rather than re-derived at each call site.
    #[tokio::test]
    async fn only_a_deadline_marks_the_bootstrap() {
        let client = crate::test_utils::create_test_client_with_name("scope-kind").await;
        assert!(!client.sync_scope(None).is_bootstrap());
        assert!(
            client
                .sync_scope(Some(wacore::time::Instant::now()))
                .is_bootstrap()
        );
    }
}

#[cfg(test)]
mod task_retry_tests {
    use super::*;

    /// `process_app_state_sync_task` answers `Ok(())` at its own shutdown guard
    /// without contacting the server, and `expected_disconnect` makes that guard
    /// true for an ordinary reconnect as well as a stop. A retry that attempted
    /// there would read the no-op as a completed sync and drop the consumer's
    /// request — a `full_sync` snapshot included — without ever asking for it.
    #[tokio::test]
    async fn a_planned_reconnect_is_not_a_completed_sync() {
        let client = crate::test_utils::create_test_client_with_name("task-retry-reconnect").await;

        // A live client with a planned reconnect in flight: the run loop is up,
        // and only `expected_disconnect` is set. This is the state
        // `reconnect_immediately()` leaves behind, and the one the retry loop
        // used to walk straight through.
        client.is_running.store(true, Ordering::Relaxed);
        client.expected_disconnect.store(true, Ordering::Relaxed);
        assert!(
            client.is_running.load(Ordering::Relaxed),
            "the retry loop's own guard still admits this state"
        );
        assert!(
            client.is_shutting_down(),
            "but it does make the callee's shutdown guard true"
        );
        assert!(
            client
                .process_app_state_sync_task(WAPatchName::Regular, true)
                .await
                .is_ok(),
            "the callee reports Ok without doing anything, which is the trap"
        );

        client.expected_disconnect.store(false, Ordering::Relaxed);
        assert!(
            !client.is_shutting_down(),
            "and the hold lifts once the reconnect settles"
        );
    }
}

#[cfg(test)]
mod apply_boundary_tests {
    use super::*;
    use crate::client::app_state::batched_sync_outcome_tests::sync_against;

    /// `process_one_patch_list` persists the version and mutation MACs before it
    /// returns, so once a collection is applied its cursor has moved and the
    /// server will not send those mutations again. Declining it after that point
    /// loses them for good — `setting_pushName` and the NCT salt included — so
    /// the admission check has to sit before the apply, and dispatching after it
    /// is not optional.
    ///
    /// Asserted as the property that matters: a collection the batch applied is
    /// reported synced, never retryable, because "retry" cannot recover it.
    ///
    /// This pins the invariant, not the race that violated it. Reproducing that
    /// needs the scope to be lost between two collections' applies, and the only
    /// hook for it would be inside the loop; a scope retired any earlier is
    /// caught by the pre-apply check and nothing is applied at all.
    #[tokio::test]
    async fn an_applied_collection_is_never_reported_retryable() {
        let (outcome, _) = sync_against(
            vec![WAPatchName::CriticalBlock, WAPatchName::CriticalUnblockLow],
            &[
                ("critical_block", None),
                ("critical_unblock_low", Some("500")),
            ],
        )
        .await;

        assert_eq!(
            outcome.synced,
            vec![WAPatchName::CriticalBlock],
            "an applied collection is synced"
        );
        assert!(
            !outcome.retryable.contains(&WAPatchName::CriticalBlock),
            "and must never also be queued for a retry that cannot re-fetch it"
        );
        assert_eq!(outcome.retryable, vec![WAPatchName::CriticalUnblockLow]);
    }
}

#[cfg(test)]
mod bootstrap_gate_tests {
    use super::*;

    /// The tag only moves forward, so a write on behalf of an older connection
    /// can never overwrite one made for a newer. This is the half that the
    /// admission check cannot provide: by the time a stale writer is inside
    /// `settle`, the replacement may already have had its say.
    #[test]
    fn an_older_connection_cannot_overwrite_a_newer_one() {
        let gate = BootstrapGate::new(false);

        assert!(gate.settle(7, true), "the newest writer wins");
        assert!(gate.is_armed());

        assert!(
            !gate.settle(6, false),
            "an older connection is refused outright"
        );
        assert!(gate.is_armed(), "and leaves the newer answer standing");

        assert!(
            gate.settle(7, false),
            "the same connection may revise itself"
        );
        assert!(!gate.is_armed());

        assert!(gate.settle(8, true), "and a newer one always may");
        assert!(gate.is_armed());
    }

    /// Pairing has no connection to speak for, and a fresh pairing owes a
    /// bootstrap no matter what any live connection last concluded.
    #[test]
    fn pairing_arms_over_any_connection() {
        let gate = BootstrapGate::new(false);
        assert!(gate.settle(9, false));
        assert!(!gate.is_armed());

        gate.arm_for_pairing();
        assert!(gate.is_armed());
        assert!(
            !gate.settle(9, false),
            "and an ordinary connection cannot undo it"
        );
        assert!(gate.is_armed());
    }

    /// The flag survives the round trip through the tag, which is the only part
    /// readers see.
    #[test]
    fn the_armed_bit_round_trips() {
        let gate = BootstrapGate::new(true);
        assert!(gate.is_armed());
        let gate = BootstrapGate::new(false);
        assert!(!gate.is_armed());
    }
}
