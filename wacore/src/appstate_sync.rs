use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_lock::Mutex;
use thiserror::Error;

use crate::appstate::hash::HashState;
use crate::appstate::hash::generate_index_mac;
use crate::appstate::keys::ExpandedAppStateKeys;
use crate::appstate::patch_decode::{CollectionSyncError, PatchList};
use crate::appstate::processor::AppStateMutationMAC;
use crate::appstate::{
    collect_key_id_refs_from_patch_list, expand_app_state_keys, process_patch, process_snapshot,
};
use crate::store::traits::Backend;
use waproto::whatsapp as wa;

// Re-export Mutation from appstate for convenience
pub use crate::appstate::Mutation;

/// Index MAC carried by a mutation's record, if present.
fn mutation_index_mac(m: &wa::SyncdMutation) -> Option<&[u8]> {
    m.record.as_option()?.index.as_option()?.blob.as_deref()
}

/// A mutation's index MAC as the fixed-size HMAC-SHA256 array; `None` for a
/// missing OR malformed (non-32-byte) blob. A malformed MAC could never match a
/// stored row, so dropping it from the batch lookup is equivalent to the miss
/// the lookup would return anyway.
fn mutation_index_mac_array(m: &wa::SyncdMutation) -> Option<IndexMac> {
    mutation_index_mac(m).and_then(|b| b.try_into().ok())
}

/// An appstate mutation index MAC: a full HMAC-SHA256 output, so always 32
/// bytes. Inline (`Copy`) so batch lookups sort/hash flat arrays with zero
/// per-MAC heap allocations.
pub type IndexMac = [u8; 32];

/// Distinct index MACs of a patch's mutations, feeding the batched
/// previous-value-MAC backend lookup. Both callers pass the result straight to a
/// `get_mutation_macs` HashMap fetch, so the order is unspecified.
///
/// Small patches dedup with a linear scan; larger patches sort + dedup the
/// inline arrays in place. Either way the only allocation is the returned `Vec`
/// itself: the comparator touches contiguous 32-byte elements (never re-walking
/// the boxed record/index/blob `MessageField` chain, the part buffa makes
/// pricier than prost's `Option` derefs).
pub fn collect_unique_index_macs(mutations: &[wa::SyncdMutation]) -> Vec<IndexMac> {
    if mutations.len() <= MAC_DEDUP_SCAN_LIMIT {
        let mut out: Vec<IndexMac> = Vec::with_capacity(mutations.len());
        for m in mutations {
            if let Some(mac) = mutation_index_mac_array(m)
                && !out.contains(&mac)
            {
                out.push(mac);
            }
        }
        return out;
    }

    let mut macs: Vec<IndexMac> = Vec::with_capacity(mutations.len());
    macs.extend(mutations.iter().filter_map(mutation_index_mac_array));
    macs.sort_unstable();
    macs.dedup();
    macs
}

/// Mutation count at or below which [`collect_unique_index_macs`] dedups with a
/// linear scan instead of a sort; above it the sort wins despite allocating
/// every MAC before deduping.
const MAC_DEDUP_SCAN_LIMIT: usize = 64;

fn lookup_app_state_key(
    keys_map: &HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>,
    key_id: &[u8],
) -> Result<Arc<ExpandedAppStateKeys>, crate::appstate::AppStateError> {
    // Return the Arc (refcount bump) instead of deep-cloning the 160-byte
    // ExpandedAppStateKeys; the callback runs once per mutation (up to ~1000/patch).
    keys_map
        .get(key_id)
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
fn download_external_blobs(pl: &mut PatchList, download: &BlobDownloadFn<'_>) -> Result<()> {
    let name = pl.name;
    if pl.snapshot.is_none()
        && let Some(ext) = &pl.snapshot_ref
    {
        let data =
            download(ext).with_context(|| format!("download external snapshot for {name:?}"))?;
        let snapshot = waproto::codec::syncd_snapshot_decode(&data)
            .with_context(|| format!("decode external snapshot for {name:?}"))?;
        pl.snapshot = Some(snapshot);
    }

    for patch in &mut pl.patches {
        if let Some(ext) = patch.external_mutations.as_option() {
            let v = patch
                .version
                .as_option()
                .and_then(|x| x.version)
                .unwrap_or(0);
            let data = download(ext)
                .with_context(|| format!("download external mutations for {name:?} v{v}"))?;
            let ext_mutations = waproto::codec::syncd_mutations_decode(&data)
                .with_context(|| format!("decode external mutations for {name:?} v{v}"))?;
            patch.mutations = ext_mutations.mutations;
            // Consumed: the reference is what marks a patch as still external,
            // so leaving it would make the next pass (`missing_key_ids_after_inline`
            // runs before `process_one_patch_list`) download and decode the
            // blob — routinely multi-MB — a second time.
            patch.external_mutations = buffa::MessageField::none();
        }
    }
    Ok(())
}

/// External-blob resolver as a trait object, so the large download/decode/apply
/// bodies below instantiate once instead of per closure type. Returns `Bytes`
/// so a resolver serving from a prefetch map hands out a refcount instead of
/// copying the blob.
pub type BlobDownloadFn<'a> =
    dyn Fn(&wa::ExternalBlobReference) -> Result<bytes::Bytes> + Send + Sync + 'a;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppStateSyncError {
    #[error("app state key not found: {0}")]
    KeyNotFound(String),
    #[error("store error")]
    Store(#[from] crate::store::error::StoreError),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// One outstanding ask, and the id the answer will carry back.
///
/// The id is what lets a reply that cannot be decoded still be recognised as
/// the answer to a request: the collection name is inside the payload, so a
/// truncated or corrupt one names nothing.
struct RecoveryRequest {
    asked_at: crate::time::Instant,
    request_id: Option<String>,
    /// Set once an answer carrying this id has been taken up, so a repeat of
    /// that answer is refused before it costs a decode.
    answering: bool,
}

/// What became of a collection the primary sent back.
///
/// Discarding is neither a failure nor a success, and a caller that could not
/// tell the two apart would log a recovery that did not happen.
#[derive(Debug)]
pub enum RecoveryOutcome {
    /// The collection was replaced; these are its records.
    Applied(Vec<Mutation>),
    /// The collection had already moved past what the primary offered, so it was
    /// left alone.
    Stale { held: u64, offered: u64 },
    /// The caller stopped standing behind the write before it began, so nothing
    /// was touched.
    Retired,
}

#[derive(Clone)]
pub struct AppStateProcessor {
    pub backend: Arc<dyn Backend>,
    pub runtime: Arc<dyn crate::runtime::Runtime>,
    /// Expanded app-state keys, keyed by the raw key id: the lookup runs once
    /// per mutation, and a base64 key meant encoding (and allocating) the id for
    /// every one of them.
    key_cache: Arc<Mutex<KeyCache>>,
    /// Collections a recovery has been asked of the primary for.
    ///
    /// Held here rather than beside the connection because it has to outlive the
    /// sync that raised it: the phone answers whenever it answers, and by then
    /// the run that asked is long over. What each entry carries, and why, is on
    /// [`RecoveryRequest`].
    recovery_requested: Arc<Mutex<HashMap<String, RecoveryRequest>>>,
}

/// Expanded app-state keys, bounded and keyed by raw key id.
///
/// An expanded key is a pure function of its key id — HKDF over bytes the
/// backend stores and never rewrites — so a cached entry can never go stale and
/// the only reason to drop one is memory. That makes a small capacity the whole
/// bound this needs: entries survive reconnects (a reconnect used to empty the
/// map, paying a backend read plus an HKDF expansion again for keys that had not
/// changed), and an account that references many distinct key ids over a long
/// connection still cannot grow it without limit.
///
/// [`CAPACITY`](Self::CAPACITY) is well above what a sync touches — WA Web sends
/// a handful of key ids per collection — so eviction is the pathological case,
/// not the steady state, and oldest-first is enough: an evicted key costs one
/// backend read to come back.
#[derive(Default)]
struct KeyCache {
    keys: HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>,
    /// Insertion order, oldest first. Only ids currently in `keys`.
    order: VecDeque<Vec<u8>>,
}

impl KeyCache {
    const CAPACITY: usize = 32;

    fn get(&self, key_id: &[u8]) -> Option<Arc<ExpandedAppStateKeys>> {
        self.keys.get(key_id).cloned()
    }

    fn insert(&mut self, key_id: Vec<u8>, expanded: Arc<ExpandedAppStateKeys>) {
        if self.keys.insert(key_id.clone(), expanded).is_none() {
            self.order.push_back(key_id);
            while self.order.len() > Self::CAPACITY
                && let Some(oldest) = self.order.pop_front()
            {
                self.keys.remove(&oldest);
            }
        }
    }

    fn len(&self) -> usize {
        self.keys.len()
    }
}

impl AppStateProcessor {
    pub fn new(backend: Arc<dyn Backend>, runtime: Arc<dyn crate::runtime::Runtime>) -> Self {
        Self {
            runtime,
            backend,
            key_cache: Arc::new(Mutex::new(KeyCache::default())),
            recovery_requested: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// How long one outstanding request suppresses another for the same
    /// collection.
    ///
    /// Nothing rate-limits the escalation on its own: both call sites reach it
    /// on every failed apply, and the retry machinery re-runs those applies
    /// across rounds and again after a reconnect. Each request asks the primary
    /// to serialize and send a whole collection, so a stuck collection would
    /// have the phone rebuilding it in a loop.
    ///
    /// Bounded on both sides by what has to happen inside it. Longer than the
    /// reply takes -- WA Web waits 60s for one -- and shorter than the retry
    /// budget that produces the asks, or the escalation would fire once and
    /// never again: the sync retries at 1, 2, 4, 8, 16, 32, 64 and 128 seconds,
    /// so a window of 300s covers every one of them and the loop exits at 255s
    /// with the first request still unanswered and nothing left to re-ask.
    /// At 120s the rounds at 127s and 255s cross it, so a phone that stays
    /// silent is asked again within the same stuck sync rather than after some
    /// unrelated trigger.
    const RECOVERY_REQUEST_TTL: core::time::Duration = core::time::Duration::from_secs(120);

    /// How long a request that is *being answered* survives instead.
    ///
    /// The window above is sized for a phone that never replied. Once a reply
    /// has been taken up, replacing the request would strand the handler holding
    /// it: it takes by id at the end and would find nothing, dropping a decoded
    /// recovery for a collection that is stuck. An answer can legitimately take
    /// far longer than the ask's own window -- the key repair and then the
    /// collection reservation, which waits up to 450 seconds on its own.
    ///
    /// Measured from the claim, not from the ask: expiry is lazy, so a phone
    /// that answers long after the window closed still claims successfully, and
    /// a ceiling counted from `asked_at` would already be spent the moment the
    /// handler took the reply up.
    ///
    /// Still an upper bound rather than none: a task that dies without ending
    /// its claim would otherwise keep the collection from ever being asked
    /// about again.
    const RECOVERY_ANSWER_TTL: core::time::Duration = core::time::Duration::from_secs(900);

    /// Records that the primary is being asked for this collection, answering
    /// whether the request is a new one.
    ///
    /// `false` means one is already outstanding and the caller should not send:
    /// the reply that is already coming answers this ask too.
    pub async fn mark_recovery_requested(&self, collection: &str) -> bool {
        let mut outstanding = self.recovery_requested.lock().await;
        if let Some(request) = outstanding.get(collection)
            && request.asked_at.elapsed()
                < if request.answering {
                    Self::RECOVERY_ANSWER_TTL
                } else {
                    Self::RECOVERY_REQUEST_TTL
                }
        {
            return false;
        }
        outstanding.insert(
            collection.to_string(),
            RecoveryRequest {
                asked_at: crate::time::Instant::now(),
                request_id: None,
                answering: false,
            },
        );
        true
    }

    /// How many recovery requests are outstanding, for the memory report.
    pub async fn outstanding_recovery_requests(&self) -> usize {
        self.recovery_requested.lock().await.len()
    }

    /// Records the id the primary's answer will carry.
    ///
    /// Set before the send, like the marker itself: the id is minted by the
    /// caller rather than by the send, and a reply that beats the send's return
    /// would otherwise find the request recorded with no id at all -- which is
    /// unrecognisable, and so unable to free the ask it answers.
    pub async fn note_recovery_request_id(&self, collection: &str, request_id: &str) {
        if let Some(request) = self.recovery_requested.lock().await.get_mut(collection) {
            request.request_id = Some(request_id.to_string());
        }
    }

    /// The collection an answer's id was asked about, claimed for this answer.
    ///
    /// The payload names a collection too, and that name is the reply's own
    /// claim about itself; this is what the ask actually was. Comparing the two
    /// is what stops a reply carrying one collection's id and another's name
    /// from overwriting a collection nobody asked about.
    ///
    /// Claiming rather than reading, because one ask has one answer: a response
    /// repeating the result, or a second copy arriving before the first is
    /// consumed, would otherwise each inflate and decode a whole collection
    /// against the same request. `None` once it is claimed, and every path that
    /// ends an answer removes the request outright.
    pub async fn claim_recovery_request_by_id(&self, request_id: &str) -> Option<String> {
        let mut outstanding = self.recovery_requested.lock().await;
        let (name, request) = outstanding
            .iter_mut()
            .find(|(_, request)| request.request_id.as_deref() == Some(request_id))?;
        if request.answering {
            return None;
        }
        request.answering = true;
        // The clock restarts here, because from here the entry is protecting a
        // handler rather than recording an unanswered ask -- and a reply can
        // arrive long after the ask's own window closed.
        request.asked_at = crate::time::Instant::now();
        Some(name.clone())
    }

    /// Takes the request an id answers, whatever its payload turned out to be.
    ///
    /// For a reply that cannot be decoded: the collection name lives inside the
    /// payload, so a truncated or corrupt one names nothing, and leaving the
    /// marker would suppress every retry for the rest of the window over an ask
    /// that has already been answered -- badly, but answered.
    pub async fn take_recovery_request_by_id(&self, request_id: &str) -> Option<String> {
        let mut outstanding = self.recovery_requested.lock().await;
        let collection = outstanding
            .iter()
            .find(|(_, request)| request.request_id.as_deref() == Some(request_id))
            .map(|(name, _)| name.clone())?;
        outstanding.remove(&collection);
        Some(collection)
    }

    /// Whether a recovery for this collection is outstanding, without taking it.
    ///
    /// The cheap check a reply is refused by before anything is reserved or
    /// spawned on its behalf.
    pub async fn has_recovery_request(&self, collection: &str) -> bool {
        self.recovery_requested
            .lock()
            .await
            .contains_key(collection)
    }

    pub async fn take_recovery_request(&self, collection: &str) -> bool {
        self.recovery_requested
            .lock()
            .await
            .remove(collection)
            .is_some()
    }

    pub async fn get_app_state_key(
        &self,
        key_id: &[u8],
    ) -> std::result::Result<Arc<ExpandedAppStateKeys>, AppStateSyncError> {
        use base64::Engine;
        use base64::engine::general_purpose::STANDARD_NO_PAD;
        if let Some(cached) = self.key_cache.lock().await.get(key_id) {
            return Ok(cached);
        }
        let key_opt = self.backend.get_sync_key(key_id).await?;
        let key = key_opt
            .ok_or_else(|| AppStateSyncError::KeyNotFound(STANDARD_NO_PAD.encode(key_id)))?;
        let expanded = Arc::new(expand_app_state_keys(&key.key_data));
        self.key_cache
            .lock()
            .await
            .insert(key_id.to_vec(), expanded.clone());
        Ok(expanded)
    }

    /// Rebuild a collection from what the primary device sent back.
    ///
    /// The escalation for a snapshot this side cannot validate. The server's
    /// snapshot is signed with a MAC we recompute, and when the two disagree
    /// there is no way forward through the server: the same bytes arrive on
    /// every retry and fail the same way, so the collection stays at version 0
    /// for ever. WA Web answers that by asking the phone, and this is the
    /// second half of that exchange.
    ///
    /// What arrives is not another snapshot. The records carry their
    /// `SyncActionData` **in the clear**, so nothing here decrypts and the
    /// app-state key is needed only to re-derive each index MAC -- which is why
    /// a collection whose value encryption we could not follow is still
    /// recoverable. The version and the ltHash are the primary's own, written
    /// as given rather than folded from the records: reproducing them would be
    /// this side recomputing the very thing it just failed to agree about.
    ///
    /// Trusting them is trusting the paired phone, over an end-to-end encrypted
    /// message, about an account it owns -- which is a weaker claim than the one
    /// already made by every key in the store.
    /// `still_current` is asked once more, immediately before the first write.
    /// Everything above it is reads and CPU and can be abandoned freely, but the
    /// three writes below replace a collection, and what makes that safe is the
    /// caller's exclusion of the other writers -- a reservation that a
    /// disconnect drops wholesale. The checks the caller made before calling are
    /// therefore stale by exactly the store lookups and the record work in
    /// between, which is the part of this that can take real time.
    pub async fn apply_snapshot_recovery(
        &self,
        recovery: wa::SyncdSnapshotRecovery,
        expected_collection: &str,
        still_current: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<RecoveryOutcome> {
        // The response says which collection it is, and the request said which
        // one was asked for. WA Web compares the two and refuses on mismatch;
        // so does this, because the reply is not correlated by anything else.
        let name = recovery
            .collection_name
            .as_deref()
            .ok_or_else(|| anyhow!("snapshot recovery names no collection"))?;
        if name != expected_collection {
            return Err(anyhow!(
                "snapshot recovery is for {name}, not the {expected_collection} that was asked for"
            ));
        }

        let version = recovery
            .version
            .as_option()
            .and_then(|v| v.version)
            .ok_or_else(|| anyhow!("snapshot recovery for {name} carries no version"))?;

        // Asked here, before the ltHash is read and before a single record is
        // looked at. Nothing waits for this reply, so the collection can have
        // moved on while the phone was answering -- a later server sync that did
        // validate, or a recovery that already landed -- and rolling it back
        // would undo real state and leave the next patch conflicting with a
        // baseline nobody is at. Checking first also means a stale reply costs
        // one read rather than a key lookup and an HMAC per record, and cannot
        // be turned into an error by a payload nobody is going to apply.
        let held = self
            .backend
            .get_version(name)
            .await?
            .map(|state| state.version)
            .unwrap_or(0);
        if snapshot_is_stale(held, version) {
            return Ok(RecoveryOutcome::Stale {
                held,
                offered: version,
            });
        }

        let lthash = recovery
            .collection_lthash
            .as_deref()
            .ok_or_else(|| anyhow!("snapshot recovery for {name} carries no ltHash"))?;
        let hash: [u8; 128] = lthash.try_into().map_err(|_| {
            anyhow!(
                "snapshot recovery for {name} carries a {}-byte ltHash, not 128",
                lthash.len()
            )
        })?;

        // The keys first, because looking one up may reach the store and the
        // work below may not. Distinct ids only: a collection is usually keyed
        // by one or two, and the cache behind this makes a repeat cheap anyway.
        let mut keys_by_id: HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>> = HashMap::new();
        for (i, record) in recovery.mutation_records.iter().enumerate() {
            let key_id = record
                .key_id
                .as_deref()
                .ok_or_else(|| anyhow!("recovery record {i} of {name} carries no key id"))?;
            if !keys_by_id.contains_key(key_id) {
                let keys = self.get_app_state_key(key_id).await?;
                keys_by_id.insert(key_id.to_vec(), keys);
            }
        }

        // And the rest off the caller's thread. A primary may send a whole
        // collection, and this is a JSON parse, an action clone and an HMAC per
        // record -- reached from the inbound message path, where holding a
        // worker stalls every message queued behind it. The recovery is taken by
        // value so the closure can consume it rather than deep-clone what it
        // needs.
        let owned_name = name.to_string();
        let (mutations, macs) = crate::runtime::blocking(&*self.runtime, move || {
            // One winner per index -- the last record for it -- which is the rule the
            // snapshot path already applies, for the reason it gives: the MAC store
            // keeps a single value per index, so a loser that was dispatched anyway
            // would hand a consumer a mute, a contact or a label the collection we
            // just persisted does not describe, and event delivery is concurrent, so
            // it could be the one that sticks.
            //
            // Keyed by the index *and the key id*, because that pair is what the
            // stored index MAC is derived from: two records naming the same index
            // under different app-state keys hash to different MACs and so occupy
            // different rows, and collapsing them would leave part of the primary's
            // ltHash with nothing standing for it -- the next patch then fails on
            // the collection this recovery just repaired. The snapshot path
            // deduplicates on the index MAC itself for the same reason; here the MAC
            // has not been derived yet, and the pair it comes from decides the same
            // thing for two HMACs less.
            //
            // Note what keying on the pair settles: a loser shares its winner's key
            // id by construction, so there is no such thing as a superseded record
            // whose key nobody else needs. Skipping losers below would save no
            // lookup, and a record referencing a key this side never received fails
            // the recovery whether or not it goes on to win.
            let winners: Vec<bool> = {
                let mut winners = vec![true; recovery.mutation_records.len()];
                // Hashed, not scanned: a whole collection is thousands of records
                // and every one of them is a membership test, so a linear `seen`
                // makes this quadratic in a payload whose size the primary chose.
                let mut seen: HashSet<(&[u8], &[u8])> = HashSet::new();
                // Backwards, so the first hit for an index is its last record.
                for (i, record) in recovery.mutation_records.iter().enumerate().rev() {
                    // A record missing its value, index or key id is not silently
                    // dropped here: it falls through to the loop below, which
                    // refuses the whole recovery over it.
                    let Some(index) = record.value.as_option().and_then(|v| v.index.as_deref())
                    else {
                        continue;
                    };
                    let Some(key_id) = record.key_id.as_deref() else {
                        continue;
                    };
                    if !seen.insert((index, key_id)) {
                        winners[i] = false;
                    }
                }
                winners
            };

            let mut mutations = Vec::with_capacity(recovery.mutation_records.len());
            let mut macs = Vec::with_capacity(recovery.mutation_records.len());
            for (i, record) in recovery.mutation_records.into_iter().enumerate() {
                if !winners[i] {
                    continue;
                }
                let action = record.value.into_option().ok_or_else(|| {
                    anyhow!("recovery record {i} of {owned_name} carries no value")
                })?;
                // The payload itself, not just the envelope around it. A record
                // whose `SyncActionData` omits its value produces a mutation
                // with nothing in it, and an index-specific dispatcher then
                // claims the index and emits no event -- while the MAC and the
                // version are committed, so the next sync starts past it and the
                // update is gone. The snapshot path does not count a value-less
                // record as part of the collection either.
                if action.value.is_unset() {
                    return Err(anyhow!(
                        "recovery record {i} of {owned_name} carries no action value"
                    ));
                }
                let key_id = record.key_id.ok_or_else(|| {
                    anyhow!("recovery record {i} of {owned_name} carries no key id")
                })?;
                // The record's own MAC is the value MAC the store keeps: the
                // primary sends what it computed over the encrypted form, so
                // nothing here has to re-encrypt a record to arrive at it.
                let value_mac = record
                    .mac
                    .ok_or_else(|| anyhow!("recovery record {i} of {owned_name} carries no MAC"))?;
                // The store keeps this as a value MAC and a later patch
                // subtracts it from the ltHash to overwrite or remove the index.
                // A MAC of the wrong length would be subtracted just the same,
                // producing a state the server disagrees with and stranding the
                // collection behind another MAC failure -- the very thing this
                // recovery exists to end.
                if value_mac.len() != 32 {
                    return Err(anyhow!(
                        "recovery record {i} of {owned_name} carries a {}-byte MAC, not 32",
                        value_mac.len()
                    ));
                }
                let index = action.index.ok_or_else(|| {
                    anyhow!("recovery record {i} of {owned_name} carries no index")
                })?;
                let keys = keys_by_id
                    .get(&key_id)
                    .expect("every key id was fetched above");

                macs.push(AppStateMutationMAC {
                    index_mac: generate_index_mac(&index, &keys.index),
                    value_mac,
                });
                // Not `unwrap_or_default()`. An index that does not parse would
                // become an empty vec, `dispatch_app_state_mutation` returns
                // early on one, and the record would reach no consumer -- while
                // its MAC was committed and the version written with a baseline,
                // so nothing would ever ask for the collection again. Silently
                // losing a record is exactly the failure this recovery exists to
                // end, and every other field here is already required.
                let parsed_index: Vec<String> = serde_json::from_slice(&index).map_err(|e| {
                    anyhow!("recovery record {i} of {owned_name} has an unreadable index: {e}")
                })?;
                // `[]` parses, and then names nothing:
                // `dispatch_app_state_mutation` returns early on an empty index,
                // so the record would reach no consumer while its MAC was
                // committed and the version written -- the same silent loss the
                // unreadable index above is refused for, one step further along.
                if parsed_index.is_empty() {
                    return Err(anyhow!(
                        "recovery record {i} of {owned_name} carries an empty index"
                    ));
                }

                mutations.push(Mutation {
                    action_value: action.value.into_option(),
                    index: parsed_index,
                    // A recovered collection is what exists, so every record is
                    // a SET -- there is nothing left to remove. WA Web forces
                    // the same.
                    operation: wa::syncd_mutation::SyncdOperation::SET,
                });
            }
            Ok::<_, anyhow::Error>((mutations, macs))
        })
        .await?;

        // Asked again here, with the reads and the record work behind us and the
        // first write in front. Whatever the caller checked before calling was
        // true a key lookup and a whole collection's worth of HMACs ago.
        if !still_current() {
            return Ok(RecoveryOutcome::Retired);
        }

        // Same order the snapshot path commits in, and for the same reason: the
        // version goes last, so a store error part-way leaves a collection that
        // will be recovered again rather than one whose version says it is
        // current over MACs that are gone.
        self.backend.clear_mutation_macs(name).await?;
        if !macs.is_empty() {
            self.backend.put_mutation_macs(name, version, &macs).await?;
        }
        self.backend
            .set_version(
                name,
                HashState {
                    version,
                    hash,
                    bootstrapped: true,
                    // The snapshot that could not be validated is the reason we
                    // are here; the collection the primary just handed over is
                    // not in that state.
                    mac_mismatch_fatal: false,
                    ..Default::default()
                },
            )
            .await?;

        Ok(RecoveryOutcome::Applied(mutations))
    }

    /// Drop every cached key, so the next access re-expands from the backend.
    ///
    /// Not part of the reconnect path: the cache bounds itself, and an
    /// expanded key is a pure function of a key id that never changes, so
    /// emptying it across reconnects only bought DB reads and HKDF expansions.
    /// Kept for a caller that genuinely wants the memory back now.
    pub async fn clear_key_cache(&self) {
        *self.key_cache.lock().await = KeyCache::default();
    }

    /// Expanded app-state keys held in memory, for `Client::memory_report()`.
    /// Bounded by `KeyCache::CAPACITY`.
    pub async fn cached_key_count(&self) -> usize {
        self.key_cache.lock().await.len()
    }

    /// Every key a patch list references, expanded, as the one map its blocking
    /// snapshot and patch closures look keys up in.
    ///
    /// Returned rather than read back out of `key_cache`: that cache is bounded,
    /// so a list referencing more keys than it holds would have evicted its first
    /// keys by the time its last was fetched, and the closures would then report
    /// a key the backend has as missing. Each key still passes through the cache
    /// on its way here, so the next list reuses whatever fits.
    ///
    /// A key the backend does not have is left out rather than failing the list:
    /// the closure that needs it reports it missing, and the caller asks the
    /// primary for it from that. Any other failure is the backend's, and is
    /// returned as such: swallowed, it would surface as that same missing-key
    /// report and ask the primary for a key this side already holds.
    async fn prefetch_keys(
        &self,
        pl: &PatchList,
    ) -> Result<HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>> {
        let key_ids = collect_key_id_refs_from_patch_list(pl.snapshot.as_ref(), &pl.patches);
        let mut keys = HashMap::with_capacity(key_ids.len());
        for key_id in key_ids {
            match self.get_app_state_key(key_id).await {
                Ok(expanded) => {
                    keys.insert(key_id.to_vec(), expanded);
                }
                Err(AppStateSyncError::KeyNotFound(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(keys)
    }

    /// Process an already-parsed single PatchList: download external blobs via
    /// `download`, then decode + apply. Lets a caller that parsed the response for
    /// pre-download avoid re-parsing it.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_parsed", level = "debug", skip_all, fields(name = ?pl.name), err(Debug)))]
    pub async fn process_parsed_patch_list(
        &self,
        mut pl: PatchList,
        download: &BlobDownloadFn<'_>,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)> {
        download_external_blobs(&mut pl, download)?;
        self.process_patch_list(pl, validate_macs).await
    }

    /// Process already-parsed patch lists, downloading any external blobs via
    /// `download`. Lets callers that already parsed the IQ response (e.g. to
    /// pre-download blobs) avoid re-parsing it.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_lists", level = "debug", skip_all, fields(count = patch_lists.len()), err(Debug)))]
    pub async fn process_patch_lists(
        &self,
        patch_lists: Vec<PatchList>,
        download: &BlobDownloadFn<'_>,
        validate_macs: bool,
    ) -> Result<Vec<(Vec<Mutation>, HashState, PatchList)>> {
        let mut results = Vec::with_capacity(patch_lists.len());

        for pl in patch_lists {
            results.push(
                self.process_one_patch_list(pl, download, validate_macs)
                    .await?,
            );
        }

        Ok(results)
    }

    /// One collection's share of [`Self::process_patch_lists`], exposed on its own.
    ///
    /// Each call persists what it applies, so a caller that must not write past
    /// some boundary — a connection being replaced, a deadline running out —
    /// needs to be able to stop between collections rather than hand over a
    /// batch it can no longer take back.
    pub async fn process_one_patch_list(
        &self,
        mut pl: PatchList,
        download: &BlobDownloadFn<'_>,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)> {
        // Skip collections with errors — caller handles them via pl.error
        if pl.error.is_some() {
            let state = self
                .backend
                .get_version(pl.name.as_str())
                .await?
                .unwrap_or_default();
            return Ok((Vec::new(), state, pl));
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
            let state = self
                .backend
                .get_version(pl.name.as_str())
                .await?
                .unwrap_or_default();
            return Ok((Vec::new(), state, pl));
        }

        self.process_patch_list(pl, validate_macs).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.process_list", level = "debug", skip_all, fields(name = ?pl.name), err(Debug)))]
    pub async fn process_patch_list(
        &self,
        mut pl: PatchList,
        validate_macs: bool,
    ) -> Result<(Vec<Mutation>, HashState, PatchList)> {
        // Arc so each blocking closure's handoff is a refcount bump, not a map copy.
        let keys_map = Arc::new(self.prefetch_keys(&pl).await?);

        let stored = self.backend.get_version(pl.name.as_str()).await?;
        let had_baseline = stored.as_ref().is_some_and(|s| s.has_baseline());
        let mut state = stored.unwrap_or_default();
        let mut new_mutations: Vec<Mutation> = Vec::new();
        let collection_name = pl.name.as_str();

        // Process snapshot if present, unless it is stale. WA Web's
        // WAWebSyncdCollectionHandler (ot()/CollectionVersionStore) applies a snapshot only
        // when it is strictly newer than the persisted version; a stale or replayed snapshot
        // (persisted >= incoming) is discarded ("skip applying syncd old version") so it can't
        // roll the collection backward. No-op on the benign first-sync path, where snapshots
        // are requested only at version 0.
        let snapshot_fresh = pl.snapshot.as_ref().is_some_and(|snapshot| {
            let snapshot_version = snapshot.version.as_option().and_then(|v| v.version).unwrap_or(0);
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
            let keys_map = Arc::clone(&keys_map);
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
            // Carried as itself rather than flattened to a string: a snapshot MAC
            // that does not match is the one error with an answer -- asking the
            // primary for the collection -- and a caller can only tell it apart
            // from a missing key or a bad decode by downcasting to it.
            .map_err(anyhow::Error::new)?;

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
            state.bootstrapped |= !pl.has_more_patches;
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
                .and_then(|p| p.version.as_option())
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
            let db_prev: HashMap<IndexMac, Vec<u8>> = self
                .backend
                .get_mutation_macs(collection_name, &need_db_lookup)
                .await?;

            let state_clone = state.clone();
            let keys = keys_map.clone();
            let coll = collection_name_owned.clone();

            // Offload CPU-intensive patch processing to a blocking thread
            let (result, patch) = crate::runtime::blocking(&*self.runtime, move || {
                let get_prev_value_mac =
                    |index_mac: &[u8]| -> Result<Option<Vec<u8>>, crate::appstate::AppStateError> {
                        Ok(<&IndexMac>::try_from(index_mac)
                            .ok()
                            .and_then(|k| db_prev.get(k))
                            .cloned())
                    };

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

            // Persist state and MACs: one backend call per patch, so a
            // transactional backend commits the version with the MACs it
            // pairs with instead of paying three round trips.
            state.bootstrapped |= !pl.has_more_patches;
            self.backend
                .commit_patch(
                    collection_name,
                    state.clone(),
                    &result.removed_index_macs,
                    &result.added_macs,
                )
                .await?;
        }
        pl.patches = processed_patches;

        // Handle case where we only have a snapshot and no patches
        if pl.patches.is_empty() && pl.snapshot.is_some() {
            state.bootstrapped |= !pl.has_more_patches;
            self.backend
                .set_version(collection_name, state.clone())
                .await?;
        } else if pl.patches.is_empty()
            && !pl.has_more_patches
            && pl.snapshot_ref.is_none()
            && !had_baseline
            && pl.error.is_none()
        {
            // A bootstrap the server answered with nothing to apply, and nothing
            // still to come. WA Web records it -- `if (isBootstrap(v))
            // updateCollectionVersionAndLtHash(0, EMPTY_LT_HASH)` on its "sync X
            // but there are no updates" branch -- and the record is what stops
            // the next sync asking for the snapshot again. Without it an account
            // whose collection is legitimately empty re-requests one forever.
            //
            // `has_more_patches` and an undownloaded `snapshot_ref` both mean this
            // is a page rather than the whole answer. Recording zero for either
            // would end the bootstrap early: the collection would never ask for a
            // snapshot again, and a non-genesis patch over its empty ltHash is
            // refused for good.
            state.bootstrapped |= !pl.has_more_patches;
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
        let mut state = self
            .backend
            .get_version(collection_name)
            .await?
            .unwrap_or_default();
        let base_version = state.version;

        // Pre-fetch previous value MACs in one backend round-trip, mirroring
        // the inbound patch path: one batched query instead of a
        // spawn_blocking + single-row SELECT per mutation.
        let need_db_lookup = collect_unique_index_macs(&mutations);
        let db_prev: HashMap<IndexMac, Vec<u8>> = self
            .backend
            .get_mutation_macs(collection_name, &need_db_lookup)
            .await?;

        // Update hash state
        let (_, hash_result) = state.update_hash(&mutations, |index_mac, _| {
            Ok(<&IndexMac>::try_from(index_mac)
                .ok()
                .and_then(|k| db_prev.get(k))
                .cloned())
        });
        hash_result?;

        state.version += 1;

        // Generate snapshot MAC
        let snapshot_mac = state.generate_snapshot_mac(collection_name, &keys.snapshot_mac);

        // Build the patch — matching whatsmeow: no Version or DeviceIndex fields
        let mut patch = wa::SyncdPatch {
            snapshot_mac: Some(snapshot_mac),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            mutations,
            ..Default::default()
        };

        // Generate and set patch MAC
        let patch_mac = generate_patch_mac(&patch, collection_name, &keys.patch_mac, state.version);
        patch.patch_mac = Some(patch_mac);

        // Encode to protobuf
        let patch_bytes = waproto::codec::syncd_patch_to_vec(&patch);

        Ok((patch_bytes, base_version))
    }

    pub async fn get_missing_key_ids(&self, pl: &PatchList) -> Result<Vec<Vec<u8>>> {
        let key_ids = collect_key_id_refs_from_patch_list(pl.snapshot.as_ref(), &pl.patches);
        let mut missing = Vec::with_capacity(key_ids.len());
        for id in key_ids {
            if self.backend.get_sync_key(id).await?.is_none() {
                missing.push(id.to_vec());
            }
        }
        Ok(missing)
    }

    /// Inline the patch list's external blobs, then report which referenced decode keys
    /// are absent. Inlining first is load-bearing: the SNAPSHOT's `key_id` lives inside
    /// its external blob, so [`get_missing_key_ids`](Self::get_missing_key_ids) alone (called before download)
    /// can't see it and would miss the snapshot's key — letting processing later abort
    /// with `KeyNotFound`. Used by the sync paths to request missing keys up front.
    /// Idempotent: `download_external_blobs` no-ops once the blobs are inlined, and the
    /// supplied `download` closure should read from the already-prefetched cache.
    pub async fn missing_key_ids_after_inline(
        &self,
        pl: &mut PatchList,
        download: &BlobDownloadFn<'_>,
    ) -> Result<Vec<Vec<u8>>> {
        download_external_blobs(pl, download)?;
        self.get_missing_key_ids(pl).await
    }
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
    use crate::appstate::patch_decode::WAPatchName;

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
        let download = |_: &wa::ExternalBlobReference| -> Result<bytes::Bytes> {
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
        let download = |_: &wa::ExternalBlobReference| -> Result<bytes::Bytes> {
            Ok(bytes::Bytes::from_static(&[0xFF, 0xFF, 0xFF]))
        };
        assert!(download_external_blobs(&mut pl, &download).is_err());
    }

    #[test]
    fn external_mutation_download_failure_propagates() {
        // The patch-level external_mutations path must propagate failures as well.
        let mut pl = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                external_mutations: buffa::MessageField::some(wa::ExternalBlobReference {
                    direct_path: Some("/mutations".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };
        let download = |_: &wa::ExternalBlobReference| -> Result<bytes::Bytes> {
            Err(anyhow!("simulated failure"))
        };
        assert!(download_external_blobs(&mut pl, &download).is_err());
    }

    /// The missing-key probe inlines before `process_one_patch_list` inlines
    /// again; the second pass must find nothing left to fetch.
    #[test]
    fn external_mutations_are_fetched_once() {
        let mut pl = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                external_mutations: buffa::MessageField::some(wa::ExternalBlobReference {
                    direct_path: Some("/mutations".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };
        let fetches = std::sync::atomic::AtomicUsize::new(0);
        // An empty buffer decodes as a `SyncdMutations` with no mutations.
        let download = |_: &wa::ExternalBlobReference| -> Result<bytes::Bytes> {
            fetches.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(bytes::Bytes::new())
        };
        download_external_blobs(&mut pl, &download).expect("first inline");
        download_external_blobs(&mut pl, &download).expect("second inline");
        assert_eq!(fetches.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(pl.patches[0].external_mutations.is_unset());
    }

    #[test]
    fn no_external_refs_is_ok() {
        let mut pl = pl_with_snapshot_ref(None);
        let download =
            |_: &wa::ExternalBlobReference| -> Result<bytes::Bytes> { Ok(bytes::Bytes::new()) };
        assert!(download_external_blobs(&mut pl, &download).is_ok());
    }
}

#[cfg(test)]
mod dedup_tests {
    use super::*;

    fn mutation(index_mac: &[u8]) -> wa::SyncdMutation {
        wa::SyncdMutation {
            record: buffa::MessageField::some(wa::SyncdRecord {
                index: buffa::MessageField::some(wa::SyncdIndex {
                    blob: Some(index_mac.to_vec()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Builds `n` mutations whose index MACs repeat every `distinct` values, so
    /// the distinct set is the first `distinct` MACs.
    fn build(n: usize, distinct: usize) -> Vec<wa::SyncdMutation> {
        (0..n)
            .map(|i| {
                let mut mac = vec![0u8; 32];
                mac[..8].copy_from_slice(&((i % distinct) as u64).to_le_bytes());
                mutation(&mac)
            })
            .collect()
    }

    fn mac_bytes(i: usize) -> IndexMac {
        let mut mac = [0u8; 32];
        mac[..8].copy_from_slice(&(i as u64).to_le_bytes());
        mac
    }

    fn expected(distinct: usize) -> Vec<IndexMac> {
        (0..distinct).map(mac_bytes).collect()
    }

    /// Dedups to the distinct index MACs across small and large N, dropping
    /// repeats. Order is unspecified (callers feed a HashMap lookup), so compare
    /// as sorted sets.
    #[test]
    fn dedups_to_distinct_macs() {
        for &n in &[8usize, 64, 65, 1000] {
            let distinct = (n / 2).max(1);
            let mut got = collect_unique_index_macs(&build(n, distinct));
            got.sort_unstable();
            let mut want = expected(distinct);
            want.sort_unstable();
            assert_eq!(got, want, "n = {n}");
        }
    }

    #[test]
    fn skips_mutations_without_index_blob() {
        let mutations = vec![
            mutation(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            wa::SyncdMutation::default(),
            mutation(b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            mutation(b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        ];
        let mut macs = collect_unique_index_macs(&mutations);
        macs.sort_unstable();
        assert_eq!(
            macs,
            vec![
                *b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                *b"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
            ]
        );
    }
}

#[cfg(test)]
mod key_cache_tests {
    use super::*;

    fn expanded(byte: u8) -> Arc<ExpandedAppStateKeys> {
        Arc::new(expand_app_state_keys(&[byte; 32]))
    }

    /// The cap is the whole bound: entries are never invalidated, only crowded
    /// out, and the oldest is the one that goes.
    #[test]
    fn the_key_cache_keeps_the_newest_entries_up_to_its_capacity() {
        let mut cache = KeyCache::default();
        for i in 0..(KeyCache::CAPACITY + 4) {
            cache.insert(vec![i as u8], expanded(i as u8));
        }

        assert_eq!(cache.len(), KeyCache::CAPACITY);
        for evicted in 0..4u8 {
            assert!(
                cache.get(&[evicted]).is_none(),
                "key {evicted} is one of the four oldest and must have been evicted"
            );
        }
        for kept in 4..(KeyCache::CAPACITY + 4) {
            assert!(
                cache.get(&[kept as u8]).is_some(),
                "key {kept} must be kept"
            );
        }
    }

    /// Re-expanding a key already held must not consume a second slot: the same
    /// key id can be looked up any number of times.
    #[test]
    fn re_inserting_a_key_id_does_not_grow_the_cache() {
        let mut cache = KeyCache::default();
        for _ in 0..(KeyCache::CAPACITY * 2) {
            cache.insert(vec![7], expanded(7));
        }

        assert_eq!(cache.len(), 1);
        assert!(cache.get(&[7]).is_some());
    }
}
