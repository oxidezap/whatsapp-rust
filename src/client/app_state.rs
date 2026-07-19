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
    runtime: &dyn wacore::runtime::Runtime,
    mut requests: futures::stream::FuturesUnordered<F>,
    timeout: Duration,
) -> Result<AppStateKeyRequestDelivery, anyhow::Error>
where
    F: std::future::Future<Output = (u16, std::result::Result<(), E>)>,
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
    ) -> std::collections::HashMap<String, Vec<u8>> {
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
        let mut seen_paths: std::collections::HashSet<&str> = std::collections::HashSet::new();
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
            return std::collections::HashMap::new();
        }

        let mut pre_downloaded = std::collections::HashMap::with_capacity(jobs.len());
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

    /// Public entry point for processing [`MajorSyncTask`] from the sync channel.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.appstate.sync_task", level = "debug", skip_all)
    )]
    pub async fn process_sync_task(self: &Arc<Self>, task: crate::sync_task::MajorSyncTask) {
        match task {
            crate::sync_task::MajorSyncTask::HistorySync {
                message_id,
                notification,
            } => {
                self.process_history_sync_task(message_id, *notification)
                    .await;
                self.finish_history_sync_task();
            }
            crate::sync_task::MajorSyncTask::AppStateSync { name, full_sync } => {
                if let Err(e) = self.process_app_state_sync_task(name, full_sync).await {
                    log::warn!("App state sync task for {name:?} failed: {e}");
                }
            }
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.fetch", level = "debug", skip_all, fields(name = ?name), err(Debug)))]
    pub(crate) async fn fetch_app_state_with_retry(&self, name: WAPatchName) -> anyhow::Result<()> {
        // In-flight dedup: skip if this collection is already being synced.
        // Matches WA Web's WAWebSyncdCollectionsStateMachine which tracks in-flight syncs
        // and queues new requests to a pending set.
        {
            let mut syncing = self.app_state_syncing.lock().await;
            if !syncing.insert(name) {
                debug!(target: "Client/AppState", "Skipping sync for {:?}: already in flight", name);
                return Ok(());
            }
        }

        let result = self.fetch_app_state_with_retry_inner(name).await;

        // Always remove from in-flight set when done
        self.app_state_syncing.lock().await.remove(&name);

        result
    }

    async fn fetch_app_state_with_retry_inner(&self, name: WAPatchName) -> anyhow::Result<()> {
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
                            // TEMP-DIAG(#1053): raised from debug! so CI shows it.
                            log::info!(target: "Client/AppState", "App state key missing for {:?}; waiting up to 10s for key share then retrying", name);
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
    ) -> anyhow::Result<()> {
        if collections.is_empty() {
            return Ok(());
        }

        // In-flight dedup: filter out collections already being synced
        let pending = {
            let mut syncing = self.app_state_syncing.lock().await;
            let mut filtered = Vec::with_capacity(collections.len());
            for name in collections {
                if syncing.insert(name) {
                    filtered.push(name);
                } else {
                    debug!(target: "Client/AppState", "Skipping {:?} in batch: already in flight", name);
                }
            }
            filtered
        };

        if pending.is_empty() {
            return Ok(());
        }

        // Track all collections for cleanup
        let all_collections: Vec<WAPatchName> = pending.clone();

        let result = self
            .sync_collections_batched_inner(pending, key_wait_deadline)
            .await;

        // Always clean up in-flight set
        {
            let mut syncing = self.app_state_syncing.lock().await;
            for name in &all_collections {
                syncing.remove(name);
            }
        }

        result
    }

    async fn sync_collections_batched_inner(
        &self,
        mut pending: Vec<WAPatchName>,
        key_wait_deadline: Option<wacore::time::Instant>,
    ) -> anyhow::Result<()> {
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
            let mut was_snapshot = std::collections::HashSet::new();
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

            let download = |ext: &wa::ExternalBlobReference| -> anyhow::Result<Vec<u8>> {
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
    ) -> anyhow::Result<()> {
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

            let download = |ext: &wa::ExternalBlobReference| -> anyhow::Result<Vec<u8>> {
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

        // TEMP-DIAG(#1053)
        log::info!(target: "Client/AppState", "TEMP-DIAG sync task complete for {:?} (final version={})", name, state.version);
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
        F: std::future::Future<Output = AppStateKeyRequestSchedule>,
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
                    self.send_message_impl(
                        peer,
                        msg,
                        Some(self.generate_message_id()),
                        true,
                        false,
                        None,
                        Vec::new(),
                        None,
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
    /// Builds the IQ stanza and sends it. Returns the updated hash state.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.send_patch", level = "debug", skip_all, fields(name = %collection_name, count = mutations.len()), err(Debug)))]
    pub(crate) async fn send_app_state_patch(
        &self,
        collection_name: &str,
        mutations: Vec<wa::SyncdMutation>,
    ) -> Result<()> {
        let proc = self.get_app_state_processor().await;
        let (patch_bytes, base_version) = proc.build_patch(collection_name, mutations).await?;

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

        self.send_iq(iq).await?;

        // Re-sync to get the latest state from the server after our patch was accepted.
        // This matches whatsmeow's behavior: fetchAppState after successful send.
        if let Ok(patch_name) = collection_name.parse::<WAPatchName>()
            && let Err(e) = self.fetch_app_state_with_retry(patch_name).await
        {
            log::warn!("Failed to re-sync {collection_name} after patch send: {e}");
        }

        Ok(())
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
