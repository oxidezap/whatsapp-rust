use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_lock::Mutex;
use async_trait::async_trait;
use prost::Message;
use thiserror::Error;

use crate::appstate::hash::HashState;
use crate::appstate::keys::ExpandedAppStateKeys;
use crate::appstate::patch_decode::{
    CollectionSyncError, PatchList, WAPatchName, parse_patch_list, parse_patch_list_ref,
    parse_patch_lists, parse_patch_lists_ref,
};
use crate::appstate::{
    collect_key_ids_from_patch_list, expand_app_state_keys, process_patch, process_snapshot,
};
use crate::store::traits::Backend;
use wacore_binary::{Node, NodeRef};
use waproto::whatsapp as wa;

// Re-export Mutation from appstate for convenience
pub use crate::appstate::Mutation;

/// Index MAC carried by a mutation's record, if present.
fn mutation_index_mac(m: &wa::SyncdMutation) -> Option<&[u8]> {
    m.record.as_ref()?.index.as_ref()?.blob.as_deref()
}

/// Unique index MACs of a patch's mutations, in first-seen order, feeding the
/// batched previous-value-MAC backend lookup.
///
/// Small patches use a cache-friendly linear scan (a HashSet measured 6-120%
/// slower at small N here). Patches carry up to ~1000 mutations, where the
/// scan's O(n²) compares dominate, so above [`MAC_DEDUP_SCAN_LIMIT`] dedup runs
/// through a sort of position indices — O(n log n) with only a `Vec<u32>` of
/// scratch, far cheaper than a `HashSet` of 32-byte MACs — then re-emits in
/// first-seen order.
pub fn collect_unique_index_macs(mutations: &[wa::SyncdMutation]) -> Vec<Vec<u8>> {
    if mutations.len() <= MAC_DEDUP_SCAN_LIMIT {
        let mut out: Vec<Vec<u8>> = Vec::with_capacity(mutations.len());
        for m in mutations {
            if let Some(mac) = mutation_index_mac(m)
                && !out.iter().any(|v| v.as_slice() == mac)
            {
                out.push(mac.to_vec());
            }
        }
        return out;
    }

    // Indices in `order` always carry a MAC, so the default branch is dead; it
    // only keeps the lookup `unwrap`-free.
    let mac_at = |i: u32| mutation_index_mac(&mutations[i as usize]).unwrap_or_default();

    // Positions of mutations carrying a MAC, in first-seen order. Pre-sized to
    // one allocation (the only scratch this path adds over the returned Vec).
    let mut order: Vec<u32> = Vec::with_capacity(mutations.len());
    order.extend(
        mutations
            .iter()
            .enumerate()
            .filter_map(|(i, m)| mutation_index_mac(m).map(|_| i as u32)),
    );

    // Group equal MACs (ties broken by position so each run's first occurrence
    // leads it), drop all but each run's leader, then restore first-seen order.
    order.sort_unstable_by(|&a, &b| mac_at(a).cmp(mac_at(b)).then(a.cmp(&b)));
    order.dedup_by(|&mut a, &mut b| mac_at(a) == mac_at(b));
    order.sort_unstable();

    order.into_iter().map(|i| mac_at(i).to_vec()).collect()
}

/// Mutation count above which [`collect_unique_index_macs`] switches from the
/// cache-friendly O(n²) linear scan to the O(n log n) index sort. Chosen well
/// below the ~1000-mutation patch ceiling and above the small-N range where the
/// scan beats sorting.
const MAC_DEDUP_SCAN_LIMIT: usize = 64;

fn lookup_app_state_key(
    keys_map: &HashMap<String, Arc<ExpandedAppStateKeys>>,
    key_id: &[u8],
) -> Result<Arc<ExpandedAppStateKeys>, crate::appstate::AppStateError> {
    use base64::Engine;
    use base64::engine::general_purpose::STANDARD_NO_PAD;
    let id_b64 = STANDARD_NO_PAD.encode(key_id);
    // Return the Arc (refcount bump) instead of deep-cloning the 160-byte
    // ExpandedAppStateKeys; the callback runs once per mutation (up to ~1000/patch).
    keys_map
        .get(&id_b64)
        .map(Arc::clone)
        .ok_or(crate::appstate::AppStateError::KeyNotFound)
}

/// Download and inline any external snapshot/mutation blobs referenced by `pl`,
/// resolving each reference via `download`.
///
/// A download/decode failure for a referenced blob is propagated as an error, not
/// swallowed: WA Web (WAWebSyncdCollectionHandler `Fe()`) throws on a failed
/// external fetch and lets the collection error out, rather than applying an empty
/// patch and advancing the version. Swallowing it here would silently drop the
/// blob's mutations and still persist the new version, losing that data permanently.
fn download_external_blobs<FDownload>(pl: &mut PatchList, download: &FDownload) -> Result<()>
where
    FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>>,
{
    let name = pl.name;
    if pl.snapshot.is_none()
        && let Some(ext) = &pl.snapshot_ref
    {
        let data =
            download(ext).with_context(|| format!("download external snapshot for {name:?}"))?;
        let snapshot = wa::SyncdSnapshot::decode(data.as_slice())
            .with_context(|| format!("decode external snapshot for {name:?}"))?;
        pl.snapshot = Some(snapshot);
    }

    for patch in &mut pl.patches {
        if let Some(ext) = &patch.external_mutations {
            let v = patch.version.as_ref().and_then(|x| x.version).unwrap_or(0);
            let data = download(ext)
                .with_context(|| format!("download external mutations for {name:?} v{v}"))?;
            let ext_mutations = wa::SyncdMutations::decode(data.as_slice())
                .with_context(|| format!("decode external mutations for {name:?} v{v}"))?;
            patch.mutations = ext_mutations.mutations;
        }
    }
    Ok(())
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppStateSyncError {
    #[error("app state key not found: {0}")]
    KeyNotFound(String),
    #[error("store error")]
    Store(#[from] crate::store::error::StoreError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct AppStateProcessor {
    pub backend: Arc<dyn Backend>,
    pub runtime: Arc<dyn crate::runtime::Runtime>,
    key_cache: Arc<Mutex<HashMap<String, Arc<ExpandedAppStateKeys>>>>,
}

impl AppStateProcessor {
    pub fn new(backend: Arc<dyn Backend>, runtime: Arc<dyn crate::runtime::Runtime>) -> Self {
        Self {
            runtime,
            backend,
            key_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn get_app_state_key(
        &self,
        key_id: &[u8],
    ) -> std::result::Result<Arc<ExpandedAppStateKeys>, AppStateSyncError> {
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD_NO_PAD;
        let id_b64 = STANDARD_NO_PAD.encode(key_id);
        if let Some(cached) = self.key_cache.lock().await.get(&id_b64).cloned() {
            return Ok(cached);
        }
        let key_opt = self.backend.get_sync_key(key_id).await?;
        let key = key_opt.ok_or_else(|| AppStateSyncError::KeyNotFound(id_b64.clone()))?;
        let expanded = Arc::new(expand_app_state_keys(&key.key_data));
        self.key_cache.lock().await.insert(id_b64, expanded.clone());
        Ok(expanded)
    }

    /// Clear the in-memory key cache (e.g. on reconnect).
    /// Keys will be re-fetched from the database backend on next access.
    pub async fn clear_key_cache(&self) {
        *self.key_cache.lock().await = HashMap::new();
    }

    /// Pre-fetch and cache all keys needed for a patch list.
    async fn prefetch_keys(&self, pl: &PatchList) -> Result<()> {
        let key_ids = collect_key_ids_from_patch_list(pl.snapshot.as_ref(), &pl.patches);
        for key_id in key_ids {
            // This will fetch and cache if not already cached
            let _ = self.get_app_state_key(&key_id).await;
        }
        Ok(())
    }

    pub async fn decode_patch_list_ref<FDownload>(
        &self,
        stanza_root: &NodeRef<'_>,
        download: FDownload,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let pl = parse_patch_list_ref(stanza_root)?;
        self.process_parsed_patch_list(pl, download, validate_macs)
            .await
    }

    /// Process an already-parsed single PatchList: download external blobs via
    /// `download`, then decode + apply. Lets a caller that parsed the response for
    /// pre-download avoid re-parsing it. See [`decode_patch_list_ref`].
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_parsed", level = "debug", skip_all, fields(name = ?pl.name), err(Debug)))]
    pub async fn process_parsed_patch_list<FDownload>(
        &self,
        mut pl: PatchList,
        download: FDownload,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        download_external_blobs(&mut pl, &download)?;
        self.process_patch_list(pl, validate_macs).await
    }

    pub async fn decode_patch_list<FDownload>(
        &self,
        stanza_root: &Node,
        download: FDownload,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let pl = parse_patch_list(stanza_root)?;
        self.process_parsed_patch_list(pl, download, validate_macs)
            .await
    }

    pub async fn decode_multi_patch_list_ref<FDownload>(
        &self,
        stanza_root: &NodeRef<'_>,
        download: &FDownload,
        validate_macs: bool,
    ) -> Result<Vec<(Vec<Mutation>, HashState, PatchList)>>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let patch_lists = parse_patch_lists_ref(stanza_root)?;
        self.process_patch_lists(patch_lists, download, validate_macs)
            .await
    }

    /// Decode a multi-collection IQ response into per-collection results.
    /// Each collection is parsed and processed independently.
    pub async fn decode_multi_patch_list<FDownload>(
        &self,
        stanza_root: &Node,
        download: &FDownload,
        validate_macs: bool,
    ) -> Result<Vec<(Vec<Mutation>, HashState, PatchList)>>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let patch_lists = parse_patch_lists(stanza_root)?;
        self.process_patch_lists(patch_lists, download, validate_macs)
            .await
    }

    /// Process already-parsed patch lists, downloading any external blobs via
    /// `download`. Lets callers that already parsed the IQ response (e.g. to
    /// pre-download blobs) avoid re-parsing it. See [`decode_multi_patch_list_ref`].
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_lists", level = "debug", skip_all, fields(count = patch_lists.len()), err(Debug)))]
    pub async fn process_patch_lists<FDownload>(
        &self,
        patch_lists: Vec<PatchList>,
        download: &FDownload,
        validate_macs: bool,
    ) -> Result<Vec<(Vec<Mutation>, HashState, PatchList)>>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let mut results = Vec::with_capacity(patch_lists.len());

        for mut pl in patch_lists {
            // Skip collections with errors — caller handles them via pl.error
            if pl.error.is_some() {
                let state = self.backend.get_version(pl.name.as_str()).await?;
                results.push((Vec::new(), state, pl));
                continue;
            }

            // A failed external-blob fetch must not advance the version with an empty
            // patch (silent data loss). Mark the collection retryable and skip it, so
            // the caller re-fetches it instead of persisting partial state.
            if let Err(e) = download_external_blobs(&mut pl, download) {
                log::warn!(target: "AppState", "External blob fetch failed for {:?}, will refetch: {e:#}", pl.name);
                pl.error = Some(CollectionSyncError::Retry {
                    code: 0,
                    text: e.to_string(),
                });
                let state = self.backend.get_version(pl.name.as_str()).await?;
                results.push((Vec::new(), state, pl));
                continue;
            }

            let (mutations, state, pl) = self.process_patch_list(pl, validate_macs).await?;
            results.push((mutations, state, pl));
        }

        Ok(results)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_list", level = "debug", skip_all, fields(name = ?pl.name), err(Debug)))]
    pub async fn process_patch_list(
        &self,
        mut pl: PatchList,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)> {
        // Pre-fetch all keys we'll need
        self.prefetch_keys(&pl).await?;

        let mut state = self.backend.get_version(pl.name.as_str()).await?;
        let mut new_mutations: Vec<Mutation> = Vec::new();
        let collection_name = pl.name.as_str();

        // Process snapshot if present, unless it is stale. WA Web's
        // WAWebSyncdCollectionHandler (ot()/CollectionVersionStore) applies a snapshot only
        // when it is strictly newer than the persisted version; a stale or replayed snapshot
        // (persisted >= incoming) is discarded ("skip applying syncd old version") so it can't
        // roll the collection backward. No-op on the benign first-sync path, where snapshots
        // are requested only at version 0.
        let snapshot_fresh = pl.snapshot.as_ref().is_some_and(|snapshot| {
            let snapshot_version = snapshot.version.as_ref().and_then(|v| v.version).unwrap_or(0);
            if snapshot_is_stale(state.version, snapshot_version) {
                log::warn!(
                    target: "AppState",
                    "Skipping stale snapshot for {collection_name}: incoming v{snapshot_version} <= persisted v{}",
                    state.version
                );
                return false;
            }
            true
        });
        if snapshot_fresh && let Some(snapshot) = pl.snapshot.take() {
            let keys_map = self.key_cache.lock().await.clone();
            let collection_name_owned = collection_name.to_string();

            // Offload CPU-intensive snapshot processing to a blocking thread. The
            // snapshot moves into the closure (its 'static bound used to force a
            // multi-MB deep clone on bootstrap) and comes back via the return tuple
            // because the caller still reads pl.snapshot (get_missing_key_ids).
            let result = crate::runtime::blocking(&*self.runtime, move || {
                let mut snapshot_state = HashState::default();
                let result = process_snapshot(
                    &snapshot,
                    &mut snapshot_state,
                    |key_id| lookup_app_state_key(&keys_map, key_id),
                    validate_macs,
                    &collection_name_owned,
                )?;
                Ok::<_, crate::appstate::AppStateError>((result, snapshot_state, snapshot))
            })
            .await
            .map_err(|e| anyhow!("{}", e))?;

            let (snapshot_result, snapshot_state, snapshot) = result;
            pl.snapshot = Some(snapshot);
            state = snapshot_state;

            // Snapshot owns the whole collection: move its Vec into the empty
            // accumulator rather than extend, which would allocate + copy a second
            // collection-sized buffer at the memory peak. is_empty falls back to extend.
            if new_mutations.is_empty() {
                new_mutations = snapshot_result.mutations;
            } else {
                new_mutations.extend(snapshot_result.mutations);
            }

            // A snapshot is a fresh baseline, so wipe the collection's prior mutation
            // MACs first (unconditionally, even if the snapshot has none) — leftover
            // index->value entries would corrupt the next patch's ltHash.
            //
            // Commit the version LAST. If clear/put fails on a transient store error,
            // an already-advanced version would make the retry treat this same snapshot
            // as stale (snapshot_is_stale) and skip it, stranding the old MACs forever.
            self.backend.clear_mutation_macs(collection_name).await?;
            if !snapshot_result.mutation_macs.is_empty() {
                self.backend
                    .put_mutation_macs(
                        collection_name,
                        state.version,
                        &snapshot_result.mutation_macs,
                    )
                    .await?;
            }
            self.backend
                .set_version(collection_name, state.clone())
                .await?;
        }

        // WA Web AntiTampering: an unsynced collection (empty ltHash) can only be
        // seeded by a snapshot or the genesis patch (version 1). If no snapshot was
        // applied and the first patch is non-genesis, applying it would anchor the
        // aggregate ltHash to nothing and persist unverified mutations, then advance
        // the version so the next sync no longer requests a snapshot. Mark the
        // collection retryable instead; the version stays 0, so the refetch re-requests
        // a snapshot. (whatsmeow/WA Web force a snapshot re-sync here.)
        if state.version == 0 && state.hash == [0u8; 128] {
            let first_version = pl
                .patches
                .first()
                .and_then(|p| p.version.as_ref())
                .and_then(|v| v.version)
                .unwrap_or(0);
            if !pl.patches.is_empty() && first_version != 1 {
                log::warn!(
                    target: "AppState",
                    "Collection {collection_name} has empty ltHash and a non-genesis first patch v{first_version} without a snapshot; will refetch"
                );
                pl.error = Some(CollectionSyncError::Retry {
                    code: 0,
                    text: "empty lthash".to_string(),
                });
                return Ok((new_mutations, state, pl));
            }
            // Reached here with an empty baseline and no snapshot applied (a snapshot
            // would have advanced the version off 0): a genesis patch (v1), or no
            // patches. Any mutation MACs still on disk are from a prior, now-reset
            // state -- e.g. a version blob that no longer decoded and reset to 0 -- so
            // wipe them before the genesis patch runs, or its ltHash would be anchored
            // to stale index->value entries (REMOVE/overwrite lookups would subtract
            // MACs that aren't part of this fresh baseline). The snapshot branch above
            // already clears for the snapshot path.
            self.backend.clear_mutation_macs(collection_name).await?;
        }

        // Snapshot the key cache once for all patches (prefetch_keys already populated
        // it); Arc so the per-patch closure handoff is a refcount bump, not a map copy.
        let keys_map = Arc::new(self.key_cache.lock().await.clone());
        let collection_name_owned = collection_name.to_string();

        // Each patch moves into its blocking closure and comes back via the return
        // tuple: the 'static bound used to force a full deep clone per patch
        // (multi-MB once external mutations are inlined), and the caller still
        // reads pl.patches afterwards (get_missing_key_ids).
        let patches = std::mem::take(&mut pl.patches);
        let mut processed_patches = Vec::with_capacity(patches.len());
        for patch in patches {
            let need_db_lookup = collect_unique_index_macs(&patch.mutations);

            // Fetch previous value MACs in one backend round-trip instead of a
            // spawn_blocking + query per mutation (N+1).
            let db_prev: HashMap<Vec<u8>, Vec<u8>> = self
                .backend
                .get_mutation_macs(collection_name, &need_db_lookup)
                .await?;

            let state_clone = state.clone();
            let keys = keys_map.clone();
            let coll = collection_name_owned.clone();

            // Offload CPU-intensive patch processing to a blocking thread
            let (result, patch) = crate::runtime::blocking(&*self.runtime, move || {
                let get_prev_value_mac = |index_mac: &[u8]| -> Result<
                    Option<Vec<u8>>,
                    crate::appstate::AppStateError,
                > { Ok(db_prev.get(index_mac).cloned()) };

                let mut state = state_clone;
                let result = process_patch(
                    &patch,
                    &mut state,
                    |key_id| lookup_app_state_key(&keys, key_id),
                    get_prev_value_mac,
                    validate_macs,
                    &coll,
                )?;
                Ok::<_, crate::appstate::AppStateError>((result, patch))
            })
            .await
            .map_err(|e| anyhow!("{}", e))?;
            processed_patches.push(patch);

            // Update local state with the result from the blocking task
            state = result.state;

            new_mutations.extend(result.mutations);

            // Persist state and MACs
            self.backend
                .set_version(collection_name, state.clone())
                .await?;
            if !result.removed_index_macs.is_empty() {
                self.backend
                    .delete_mutation_macs(collection_name, &result.removed_index_macs)
                    .await?;
            }
            if !result.added_macs.is_empty() {
                self.backend
                    .put_mutation_macs(collection_name, state.version, &result.added_macs)
                    .await?;
            }
        }
        pl.patches = processed_patches;

        // Handle case where we only have a snapshot and no patches
        if pl.patches.is_empty() && pl.snapshot.is_some() {
            self.backend
                .set_version(collection_name, state.clone())
                .await?;
        }

        Ok((new_mutations, state, pl))
    }

    /// Build and encode a SyncdPatch for sending mutations to the server.
    ///
    /// Takes a list of pre-encoded mutations (from `encode_record`) and produces
    /// the protobuf-encoded patch bytes ready for inclusion in an IQ stanza.
    ///
    /// # Returns
    /// A tuple of (patch_bytes, updated_hash_state).
    /// Encode mutations into a SyncdPatch protobuf blob.
    ///
    /// Returns `(patch_bytes, base_version)` where `base_version` is the collection
    /// version before the patch (for the IQ `version` attribute). Does NOT persist
    /// state — the caller must only persist after the server acknowledges the patch.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.build_patch", level = "debug", skip_all, fields(name = %collection_name, count = mutations.len()), err(Debug)))]
    pub async fn build_patch(
        &self,
        collection_name: &str,
        mutations: Vec<wa::SyncdMutation>,
    ) -> Result<(Vec<u8>, u64)> {
        use crate::appstate::hash::generate_patch_mac;

        // Get active key
        let key_id = self
            .backend
            .get_latest_sync_key_id()
            .await?
            .ok_or_else(|| anyhow!("No app state sync key available"))?;
        let keys = self.get_app_state_key(&key_id).await?;

        // Get current hash state — save base version for the caller
        let mut state = self.backend.get_version(collection_name).await?;
        let base_version = state.version;

        // Pre-fetch previous value MACs in one backend round-trip, mirroring
        // the inbound patch path: one batched query instead of a
        // spawn_blocking + single-row SELECT per mutation.
        let need_db_lookup = collect_unique_index_macs(&mutations);
        let db_prev: std::collections::HashMap<Vec<u8>, Vec<u8>> = self
            .backend
            .get_mutation_macs(collection_name, &need_db_lookup)
            .await?;

        // Update hash state
        let (_, hash_result) = state.update_hash(&mutations, |index_mac, _| {
            Ok(db_prev.get(index_mac).cloned())
        });
        hash_result?;

        state.version += 1;

        // Generate snapshot MAC
        let snapshot_mac = state.generate_snapshot_mac(collection_name, &keys.snapshot_mac);

        // Build the patch — matching whatsmeow: no Version or DeviceIndex fields
        let mut patch = wa::SyncdPatch {
            snapshot_mac: Some(snapshot_mac),
            key_id: Some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            mutations,
            ..Default::default()
        };

        // Generate and set patch MAC
        let patch_mac = generate_patch_mac(&patch, collection_name, &keys.patch_mac, state.version);
        patch.patch_mac = Some(patch_mac);

        // Encode to protobuf
        let patch_bytes = patch.encode_to_vec();

        Ok((patch_bytes, base_version))
    }

    pub async fn get_missing_key_ids(&self, pl: &PatchList) -> Result<Vec<Vec<u8>>> {
        let key_ids = collect_key_ids_from_patch_list(pl.snapshot.as_ref(), &pl.patches);
        let mut missing = Vec::with_capacity(key_ids.len());
        for id in key_ids {
            if self.backend.get_sync_key(&id).await?.is_none() {
                missing.push(id);
            }
        }
        Ok(missing)
    }

    /// Inline the patch list's external blobs, then report which referenced decode keys
    /// are absent. Inlining first is load-bearing: the SNAPSHOT's `key_id` lives inside
    /// its external blob, so [`get_missing_key_ids`] alone (called before download)
    /// can't see it and would miss the snapshot's key — letting processing later abort
    /// with `KeyNotFound`. Used by the sync paths to request missing keys up front.
    /// Idempotent: `download_external_blobs` no-ops once the blobs are inlined, and the
    /// supplied `download` closure should read from the already-prefetched cache.
    pub async fn missing_key_ids_after_inline<FDownload>(
        &self,
        pl: &mut PatchList,
        download: &FDownload,
    ) -> Result<Vec<Vec<u8>>>
    where
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>>,
    {
        download_external_blobs(pl, download)?;
        self.get_missing_key_ids(pl).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.sync_collection", level = "debug", skip_all, fields(name = ?name), err(Debug)))]
    pub async fn sync_collection<D, FDownload>(
        &self,
        driver: &D,
        name: WAPatchName,
        validate_macs: bool,
        download: FDownload,
    ) -> Result<Vec<Mutation>>
    where
        D: AppStateSyncDriver + Sync,
        FDownload: Fn(&wa::ExternalBlobReference) -> Result<Vec<u8>> + Send + Sync,
    {
        let mut all = Vec::new();
        // Bound re-fetches so a server that keeps returning a retryable collection
        // (e.g. an empty-ltHash patch without a snapshot) can't loop forever.
        const MAX_RETRIES: usize = 5;
        let mut retries = 0;
        loop {
            let state = self.backend.get_version(name.as_str()).await?;
            let node = driver.fetch_collection(name, state.version).await?;
            let (mut muts, _new_state, list) = self
                .decode_patch_list(&node, &download, validate_macs)
                .await?;
            all.append(&mut muts);
            // A retryable error (or conflict-with-more) left the version unadvanced;
            // re-fetch (now requesting a snapshot, since the version is still 0)
            // rather than reporting success. Mirrors the batched path's needs_refetch.
            // Fatal / conflict-without-more fall through and end the loop.
            if matches!(
                list.error,
                Some(CollectionSyncError::Retry { .. })
                    | Some(CollectionSyncError::Conflict { has_more: true })
            ) {
                retries += 1;
                if retries >= MAX_RETRIES {
                    break;
                }
                continue;
            }
            if !list.has_more_patches {
                break;
            }
        }
        Ok(all)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait AppStateSyncDriver {
    async fn fetch_collection(&self, name: WAPatchName, after_version: u64) -> Result<Node>;
}

/// A snapshot is stale when the collection already holds a version at or beyond the
/// incoming snapshot's; WA Web discards it ("skip applying syncd old version") rather
/// than rolling the collection backward. The `persisted_version > 0` guard keeps the
/// benign first-sync path (snapshots are requested only at version 0) unaffected.
fn snapshot_is_stale(persisted_version: u64, snapshot_version: u64) -> bool {
    persisted_version > 0 && snapshot_version <= persisted_version
}

#[cfg(test)]
mod snapshot_guard_tests {
    use super::snapshot_is_stale;

    #[test]
    fn first_sync_is_never_stale() {
        // Benign path: nothing persisted yet (version 0), so any snapshot applies.
        assert!(!snapshot_is_stale(0, 1));
        assert!(!snapshot_is_stale(0, 0));
    }

    #[test]
    fn newer_snapshot_applies() {
        assert!(!snapshot_is_stale(5, 6));
    }

    #[test]
    fn equal_or_older_snapshot_is_stale() {
        // WA Web's `a.version >= t` skips equal versions too.
        assert!(snapshot_is_stale(5, 5));
        assert!(snapshot_is_stale(5, 3));
        assert!(snapshot_is_stale(5, 0));
    }
}

#[cfg(test)]
mod external_blob_tests {
    use super::*;

    fn pl_with_snapshot_ref(snapshot_ref: Option<wa::ExternalBlobReference>) -> PatchList {
        PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: Vec::new(),
            snapshot: None,
            snapshot_ref,
            error: None,
        }
    }

    #[test]
    fn external_snapshot_download_failure_propagates() {
        // A referenced blob that fails to download must error, not be swallowed
        // (which would apply an empty patch and advance the version).
        let mut pl = pl_with_snapshot_ref(Some(wa::ExternalBlobReference {
            direct_path: Some("/blob".into()),
            ..Default::default()
        }));
        let download = |_: &wa::ExternalBlobReference| -> Result<Vec<u8>> {
            Err(anyhow!("simulated failure"))
        };
        assert!(download_external_blobs(&mut pl, &download).is_err());
    }

    #[test]
    fn external_snapshot_decode_failure_propagates() {
        // Download succeeds but the bytes aren't a valid SyncdSnapshot: the decode
        // error must propagate too, not just download errors.
        let mut pl = pl_with_snapshot_ref(Some(wa::ExternalBlobReference {
            direct_path: Some("/blob".into()),
            ..Default::default()
        }));
        let download =
            |_: &wa::ExternalBlobReference| -> Result<Vec<u8>> { Ok(vec![0xFF, 0xFF, 0xFF]) };
        assert!(download_external_blobs(&mut pl, &download).is_err());
    }

    #[test]
    fn external_mutation_download_failure_propagates() {
        // The patch-level external_mutations path must propagate failures as well.
        let mut pl = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                external_mutations: Some(wa::ExternalBlobReference {
                    direct_path: Some("/mutations".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };
        let download = |_: &wa::ExternalBlobReference| -> Result<Vec<u8>> {
            Err(anyhow!("simulated failure"))
        };
        assert!(download_external_blobs(&mut pl, &download).is_err());
    }

    #[test]
    fn no_external_refs_is_ok() {
        let mut pl = pl_with_snapshot_ref(None);
        let download = |_: &wa::ExternalBlobReference| -> Result<Vec<u8>> { Ok(Vec::new()) };
        assert!(download_external_blobs(&mut pl, &download).is_ok());
    }
}

#[cfg(test)]
mod dedup_tests {
    use super::*;

    fn mutation(index_mac: &[u8]) -> wa::SyncdMutation {
        wa::SyncdMutation {
            record: Some(wa::SyncdRecord {
                index: Some(wa::SyncdIndex {
                    blob: Some(index_mac.to_vec()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Builds `n` mutations whose index MACs repeat every `distinct` values, so
    /// the expected output is the first `distinct` MACs in first-seen order.
    fn build(n: usize, distinct: usize) -> Vec<wa::SyncdMutation> {
        (0..n)
            .map(|i| {
                let mut mac = vec![0u8; 32];
                mac[..8].copy_from_slice(&((i % distinct) as u64).to_le_bytes());
                mutation(&mac)
            })
            .collect()
    }

    fn mac_bytes(i: usize) -> Vec<u8> {
        let mut mac = vec![0u8; 32];
        mac[..8].copy_from_slice(&(i as u64).to_le_bytes());
        mac
    }

    fn expected(distinct: usize) -> Vec<Vec<u8>> {
        (0..distinct).map(mac_bytes).collect()
    }

    /// Both dedup paths must yield identical first-seen-order unique results;
    /// the index-sort path (large N) and scan path (small N) cannot diverge.
    #[test]
    fn scan_and_sort_paths_agree() {
        // Small N exercises the linear scan; large N (> limit) the index sort.
        for &n in &[8usize, MAC_DEDUP_SCAN_LIMIT, MAC_DEDUP_SCAN_LIMIT + 1, 1000] {
            let distinct = (n / 2).max(1);
            assert_eq!(
                collect_unique_index_macs(&build(n, distinct)),
                expected(distinct),
                "n = {n}"
            );
        }
    }

    /// The index-sort path must re-emit in first-seen order, not byte-sort order.
    /// Force that path (> limit MACs) with first appearances running opposite to
    /// byte order, plus trailing duplicates that must be dropped — so a bug in
    /// the order-restoration step can't pass by coinciding with the sort order.
    #[test]
    fn sort_path_restores_first_seen_order() {
        let distinct = MAC_DEDUP_SCAN_LIMIT + 20;
        // First-seen order is descending i; byte order is ascending (i < 256).
        let mut mutations: Vec<wa::SyncdMutation> = (0..distinct)
            .rev()
            .map(|i| mutation(&mac_bytes(i)))
            .collect();
        for i in [distinct - 1, distinct / 2, 0] {
            mutations.push(mutation(&mac_bytes(i)));
        }
        let want: Vec<Vec<u8>> = (0..distinct).rev().map(mac_bytes).collect();
        assert_eq!(collect_unique_index_macs(&mutations), want);
    }

    #[test]
    fn skips_mutations_without_index_blob() {
        let mutations = vec![
            mutation(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            wa::SyncdMutation::default(),
            mutation(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            mutation(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ];
        let macs = collect_unique_index_macs(&mutations);
        assert_eq!(
            macs,
            vec![
                b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_vec(),
                b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_vec()
            ]
        );
    }
}
