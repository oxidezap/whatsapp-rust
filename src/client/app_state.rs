//! App-state collection sync and mutation dispatch.

use super::*;

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

/// In-flight dedup registry for app-state collection syncs.
///
/// Reservations carry a per-begin token so a release can only ever remove the
/// reservation it belongs to: a stale task finishing after a reconnect cleared
/// the registry cannot evict the newer generation's reservation for the same
/// collection. Releases run from the guard's `Drop`, so a cancelled sync
/// (timeout, abort, teardown) can never strand a collection as "in flight".
/// The mutex is synchronous and never held across an await.
pub(crate) struct SyncInFlight {
    entries: std::sync::Mutex<HashMap<WAPatchName, u64>>,
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

    /// Reserve `name`, or `None` when a sync for it is already in flight.
    pub(crate) fn try_begin(self: &Arc<Self>, name: WAPatchName) -> Option<SyncInFlightGuard> {
        let token = self.next_token.fetch_add(1, Ordering::Relaxed);
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if entries.contains_key(&name) {
            return None;
        }
        entries.insert(name, token);
        Some(SyncInFlightGuard {
            registry: Arc::clone(self),
            name,
            token,
        })
    }

    /// Reserve `name`, waiting for the current holder to finish.
    ///
    /// [`try_begin`](Self::try_begin) is right for a sync, where an in-flight
    /// one already does the work and skipping is free. A patch send cannot
    /// skip: it must not write the collection's version and mutation MACs while
    /// a sync is writing them, and it needs the base a concurrent sync is about
    /// to move. Cancelling this future simply stops waiting; nothing is
    /// reserved until the guard is returned.
    pub(crate) async fn begin(self: &Arc<Self>, name: WAPatchName) -> SyncInFlightGuard {
        loop {
            // Register the listener before re-checking, so a release landing
            // between the check and the wait cannot be missed.
            let released = self.released.listen();
            if let Some(guard) = self.try_begin(name) {
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
        if entries.get(&self.name) == Some(&self.token) {
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

    /// Sync multiple collections in a single IQ request, re-fetching those with `has_more_patches`.
    /// Matches WA Web's `serverSync()` outer loop (`3JJWKHeu5-P.js:54278-54305`).
    /// Max 5 iterations (WA Web's `C=5` constant).
    ///
    /// `key_wait_deadline` bounds how long a missing app-state decode key may be
    /// awaited. The initial critical bootstrap passes the shared 180s critical-sync
    /// deadline so the explicit `AppStateSyncKeyRequest` fallback can recover a
    /// late/never-auto-shared key on the same connection; other callers pass `None`
    /// for the fixed short default.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.sync_batched", level = "debug", skip_all, fields(count = collections.len()), err(Debug)))]
    pub(crate) async fn sync_collections_batched(
        &self,
        collections: Vec<WAPatchName>,
        key_wait_deadline: Option<wacore::time::Instant>,
    ) -> Result<()> {
        if collections.is_empty() {
            return Ok(());
        }

        // In-flight dedup: filter out collections already being synced. The
        // guards release on every exit path, including cancellation.
        let mut guards = Vec::with_capacity(collections.len());
        let mut pending = Vec::with_capacity(collections.len());
        for name in collections {
            match self.app_state_syncing.try_begin(name) {
                Some(guard) => {
                    guards.push(guard);
                    pending.push(name);
                }
                None => {
                    debug!(target: "Client/AppState", "Skipping {:?} in batch: already in flight", name);
                }
            }
        }

        if pending.is_empty() {
            return Ok(());
        }

        self.sync_collections_batched_inner(pending, key_wait_deadline)
            .await
    }

    async fn sync_collections_batched_inner(
        &self,
        mut pending: Vec<WAPatchName>,
        key_wait_deadline: Option<wacore::time::Instant>,
    ) -> Result<()> {
        use wacore::appstate::patch_decode::CollectionSyncError;
        const MAX_ITERATIONS: usize = 5;
        let mut iteration = 0;

        while !pending.is_empty() && iteration < MAX_ITERATIONS {
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

            // Parse the response once here for pre-download; the same parsed
            // lists are handed to the processor below (no second parse).
            let mut patch_lists =
                wacore::appstate::patch_decode::parse_patch_lists_ref(resp.get())?;

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
            let key_wait = match key_wait_deadline {
                Some(deadline) => deadline.saturating_duration_since(wacore::time::Instant::now()),
                None => APP_STATE_KEY_REQUEST_TIMEOUT,
            };
            if !missing_all.is_empty() && !self.request_keys_and_wait(missing_all, key_wait).await {
                // The re-shared key didn't land in time. Report failure rather than a
                // false success: the initial critical-sync path treats Ok as permission
                // to cancel its retry watchdog and dispatch Connected, which would leave
                // CriticalBlock/CriticalUnblockLow unsynced with no scheduled retry. The
                // collections re-sync on the retry (or a later server_sync) once the
                // share arrives; the keys we DID repair are already persisted.
                return Err(anyhow::anyhow!(
                    "app-state decode key(s) still missing after re-request; deferring batched sync"
                ));
            }

            // Process the already-parsed (and inlined) collections; keys are present.
            let results = proc
                .process_patch_lists(patch_lists, &download, true)
                .await?;

            let mut needs_refetch = Vec::new();

            for (mutations, new_state, list) in results {
                let name = list.name;

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
                            }
                            continue;
                        }
                        CollectionSyncError::Fatal { code, text } => {
                            warn!(target: "Client/AppState", "Collection {:?} fatal error {}: {}", name, code, text);
                            continue;
                        }
                        CollectionSyncError::Retry { code, text } => {
                            warn!(target: "Client/AppState", "Collection {:?} retryable error {}: {}, will refetch", name, code, text);
                            needs_refetch.push(name);
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
                }

                debug!(
                    target: "Client/AppState",
                    "Batched sync: {:?} done (version={}, has_more={})",
                    name, new_state.version, list.has_more_patches
                );
            }

            pending = needs_refetch;
        }

        if !pending.is_empty() {
            warn!(
                target: "Client/AppState",
                "Batched sync: max iterations ({}) reached for {:?}",
                MAX_ITERATIONS, pending
            );
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
            Some(name) => Some(self.app_state_syncing.begin(name).await),
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
                let guard = registry.begin(WAPatchName::Regular).await;
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
