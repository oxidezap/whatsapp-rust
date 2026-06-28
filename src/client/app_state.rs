//! App-state collection sync and mutation dispatch.

use super::*;

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
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.sync_batched", level = "debug", skip_all, fields(count = collections.len()), err(Debug)))]
    pub(crate) async fn sync_collections_batched(
        &self,
        collections: Vec<WAPatchName>,
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

        let result = self.sync_collections_batched_inner(pending).await;

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

            // Pre-download all external blobs for all collections in the response
            let mut pre_downloaded: std::collections::HashMap<String, Vec<u8>> =
                std::collections::HashMap::new();

            // Parse the response once here for pre-download; the same parsed
            // lists are handed to the processor below (no second parse).
            let mut patch_lists =
                wacore::appstate::patch_decode::parse_patch_lists_ref(resp.get())?;

            let proc = self.get_app_state_processor().await;
            {
                for pl in &patch_lists {
                    // Download external snapshot
                    if let Some(ext) = &pl.snapshot_ref
                        && let Some(path) = &ext.direct_path
                    {
                        match self.download(ext).await {
                            Ok(bytes) => {
                                pre_downloaded.insert(path.clone(), bytes);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to download external snapshot for {:?}: {e}",
                                    pl.name
                                );
                            }
                        }
                    }

                    // Download external mutations
                    for patch in &pl.patches {
                        if let Some(ext) = &patch.external_mutations
                            && let Some(path) = &ext.direct_path
                        {
                            match self.download(ext).await {
                                Ok(bytes) => {
                                    pre_downloaded.insert(path.clone(), bytes);
                                }
                                Err(e) => {
                                    let v =
                                        patch.version.as_ref().and_then(|v| v.version).unwrap_or(0);
                                    warn!(
                                        "Failed to download external mutations for patch v{}: {e}",
                                        v
                                    );
                                }
                            }
                        }
                    }
                }
            }

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
            if !missing_all.is_empty() && !self.request_keys_and_wait(missing_all).await {
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
                self.request_missing_keys_with_dedup(missing).await;

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

            // Pre-download all external blobs (snapshot and patch mutations); keyed by
            // directPath.
            let mut pre_downloaded: std::collections::HashMap<String, Vec<u8>> =
                std::collections::HashMap::new();
            {
                // Download external snapshot if present
                if let Some(ext) = &pl.snapshot_ref
                    && let Some(path) = &ext.direct_path
                {
                    match self.download(ext).await {
                        Ok(bytes) => {
                            debug!(target: "Client/AppState", "Downloaded external snapshot ({} bytes)", bytes.len());
                            pre_downloaded.insert(path.clone(), bytes);
                        }
                        Err(e) => {
                            warn!("Failed to download external snapshot: {e}");
                        }
                    }
                }

                // Download external mutations for each patch that has them
                for patch in &pl.patches {
                    if let Some(ext) = &patch.external_mutations
                        && let Some(path) = &ext.direct_path
                    {
                        let patch_version =
                            patch.version.as_ref().and_then(|v| v.version).unwrap_or(0);
                        match self.download(ext).await {
                            Ok(bytes) => {
                                debug!(target: "Client/AppState", "Downloaded external mutations for patch v{} ({} bytes)", patch_version, bytes.len());
                                pre_downloaded.insert(path.clone(), bytes);
                            }
                            Err(e) => {
                                warn!(
                                    "Failed to download external mutations for patch v{}: {e}",
                                    patch_version
                                );
                            }
                        }
                    }
                }
            }

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
            if !missing.is_empty() && !self.request_keys_and_wait(missing).await {
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
            self.request_missing_keys_with_dedup(missing).await;

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

    /// Shared missing-key repair step for both sync paths: request the given keys and,
    /// only if a fresh request actually went out (the per-key dedup didn't suppress it),
    /// wait briefly for the primary to re-share. Returns true iff the caller should
    /// refetch (a request was sent and we waited); false means nothing was requested
    /// (empty or deduped), so the caller proceeds without stalling.
    /// Request the missing decode keys, wait briefly for the re-share, then VERIFY they
    /// actually landed. Returns true only when every requested key is now stored (the
    /// caller may process); false means the share didn't arrive in time and the caller
    /// must NOT process -- doing so would abort with KeyNotFound -- and should skip the
    /// collection so it re-syncs on a later cycle. Empty input returns true (nothing to
    /// wait for). Waits even when the per-key dedup suppressed the send: a deduped
    /// request means an earlier one is still in flight, so the key may yet land here,
    /// and a re-verify that fails can't be masked by treating "request sent" as success
    /// or by a wake from an unrelated key share.
    async fn request_keys_and_wait(&self, missing: Vec<Vec<u8>>) -> bool {
        if missing.is_empty() {
            return true;
        }
        let count = missing.len();
        let listener = self.initial_keys_synced_notifier.listen();
        self.request_missing_keys_with_dedup(missing.clone()).await;
        debug!(target: "Client/AppState", "Requested {count} missing app-state key(s); waiting up to 10s for the re-share");
        let _ = rt_timeout(&*self.runtime, Duration::from_secs(10), listener).await;
        let backend = self.persistence_manager.backend();
        for id in &missing {
            if backend.get_sync_key(id).await.ok().flatten().is_none() {
                return false;
            }
        }
        true
    }

    /// Request missing app-state keys with dedup stamps.
    /// On send failure, removes stamps so keys can be retried next sync.
    /// Returns true iff a fresh key request was actually sent (some ids passed the
    /// per-key dedup and the send succeeded), so the caller knows whether a re-share
    /// is plausibly inbound and worth waiting for.
    async fn request_missing_keys_with_dedup(&self, missing: Vec<Vec<u8>>) -> bool {
        if missing.is_empty() {
            return false;
        }
        let mut to_request: Vec<Vec<u8>> = Vec::with_capacity(missing.len());
        let mut guard = self.app_state_key_requests.lock().await;
        let now = wacore::time::Instant::now();
        for key_id in missing {
            let hex_id = hex::encode(&key_id);
            let should = guard
                .get(&hex_id)
                .map(|t| t.elapsed() > std::time::Duration::from_secs(24 * 3600))
                .unwrap_or(true);
            if should {
                guard.insert(hex_id, now);
                to_request.push(key_id);
            }
        }
        guard.retain(|_, t| t.elapsed() < std::time::Duration::from_secs(24 * 3600));
        drop(guard);
        if to_request.is_empty() {
            return false;
        }
        if let Err(e) = self.request_app_state_keys(&to_request).await {
            warn!("Failed to send app state key request: {e}");
            let mut guard = self.app_state_key_requests.lock().await;
            for key_id in &to_request {
                guard.remove(&hex::encode(key_id));
            }
            return false;
        }
        true
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.request_keys", level = "debug", skip_all, fields(count = raw_key_ids.len()), err(Debug)))]
    async fn request_app_state_keys(&self, raw_key_ids: &[Vec<u8>]) -> Result<(), anyhow::Error> {
        if raw_key_ids.is_empty() {
            return Ok(());
        }
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        // Address the request to the PRIMARY (device 0), not our own device JID: this is
        // a peer message and `device_snapshot.pn` carries OUR device number, so sending
        // it as-is encrypts to ourselves (no self-session exists) and fails with
        // "session ... not found". The primary is the app-state key source and we hold a
        // session with it from pairing. Mirrors whatsmeow's `ownID.ToNonAD()`.
        let own_jid = match device_snapshot.pn.clone() {
            Some(j) => j.to_non_ad(),
            None => {
                return Err(anyhow::anyhow!(
                    "no own JID available for app-state key request"
                ));
            }
        };
        let key_ids: Vec<wa::message::AppStateSyncKeyId> = raw_key_ids
            .iter()
            .map(|k| wa::message::AppStateSyncKeyId {
                key_id: Some(k.clone()),
            })
            .collect();
        let msg = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::AppStateSyncKeyRequest as i32),
                app_state_sync_key_request: Some(wa::message::AppStateSyncKeyRequest { key_ids }),
                ..Default::default()
            })),
            ..Default::default()
        };
        self.send_message_impl(
            own_jid,
            &msg,
            Some(self.generate_message_id()),
            true,
            false,
            None,
            vec![],
            None,
        )
        .await?;
        Ok(())
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
                && let Some(act) = &val.nct_salt_sync_action
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
            && let Some(act) = &val.push_name_setting
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
                    crate::types::events::SelfPushNameUpdated {
                        from_server: true,
                        old_name: old.clone(),
                        new_name: new_name.clone(),
                    },
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
