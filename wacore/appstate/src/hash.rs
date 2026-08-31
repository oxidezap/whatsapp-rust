use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use std::collections::HashMap;
use wacore_libsignal::crypto::CryptographicMac;
use waproto::whatsapp as wa;

use crate::{AppStateError, WAPATCH_INTEGRITY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashState {
    pub version: u64,
    #[serde(with = "BigArray")]
    pub hash: [u8; 128],
    pub index_value_map: HashMap<String, Vec<u8>>,
    /// The collection's aggregate ltHash no longer agrees with the one its peers
    /// compute, so a patch's `snapshotMac` can never match again and comparing it
    /// only freezes the collection on a base already proven unusable.
    ///
    /// Mirrors WA Web's `isCollectionInMacMismatchFatal`
    /// (`WAWebGetCollectionVersion`): set the first time a *patch* fails the
    /// snapshotMac comparison, after which the comparison is skipped entirely
    /// (`WAWebSyncdAntiTampering`, `if (E && k) return null`). `patchMac` — the
    /// only MAC that proves authorship, since the server cannot compute it —
    /// stays enforced on every patch either way.
    ///
    /// Unlike WA Web, which merges the flag forward forever, a snapshot resets it:
    /// a snapshot rebuilds the ltHash from scratch, so the new baseline deserves
    /// to be trusted again.
    #[serde(default)]
    pub mac_mismatch_fatal: bool,
    /// Whether a bootstrap reached its terminal page.
    ///
    /// Set only when the run that persisted this state was not still paging, so
    /// a partial bootstrap -- page five of sixty, then a deferral -- leaves it
    /// false even though the version has moved. That is what makes it safe to
    /// ask before building an outgoing patch, where a cursor behind the head
    /// earns a 409 that the send has to unwind.
    ///
    /// Presence of a record cannot answer that on its own: a collection that has
    /// never synced and one that synced and is legitimately empty are both
    /// version 0 with an all-zero ltHash, byte for byte. WA Web distinguishes
    /// them by whether a record exists at all, because it writes one on the
    /// "no updates" branch; we carry the fact explicitly so a row written by an
    /// older build -- which wrote version 0 for an interrupted bootstrap too --
    /// decodes to false and bootstraps once more rather than asking for patches
    /// its empty ltHash can never accept.
    #[serde(default)]
    pub bootstrapped: bool,
}

impl Default for HashState {
    fn default() -> Self {
        Self {
            version: 0,
            hash: [0; 128],
            index_value_map: HashMap::new(),
            mac_mismatch_fatal: false,
            bootstrapped: false,
        }
    }
}

/// What a fold actually did, as opposed to what it was handed.
///
/// Returned rather than recomputed by callers: the fold's eligibility rule has
/// already changed once, and a diagnostic that restates it is one that can
/// disagree with the hash it is describing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldReport {
    /// How many value MACs entered the ltHash.
    pub folded: usize,
    /// How many of those came from records carrying no index MAC.
    pub unkeyed: usize,
}

impl HashState {
    /// Whether there is a real ltHash here to apply patches on top of.
    ///
    /// This is the question `return_snapshot` answers: a version past zero proves
    /// a baseline was established, since patches only apply over one. Only
    /// version zero is ambiguous -- never synced and synced-but-empty are
    /// byte-identical there -- and that is the case [`HashState::bootstrapped`]
    /// decides. Asking for the flag alone would make every account re-download a
    /// snapshot per collection on upgrade, for something the version already
    /// answers.
    ///
    /// It is deliberately *not* the question to ask before building an outgoing
    /// patch. A run that persisted page five of sixty and then stopped has a
    /// baseline and is still behind the head; [`HashState::bootstrapped`] is what
    /// says a bootstrap actually finished.
    pub fn has_baseline(&self) -> bool {
        self.bootstrapped || self.version > 0
    }
}

/// Result of updating the hash state with mutations.
#[derive(Debug, Clone, Default)]
pub struct HashUpdateResult {
    /// Whether a REMOVE mutation was missing its previous value.
    /// This happens when the server has an entry we don't have locally.
    /// WhatsApp Web tracks this as telemetry for MAC-failure diagnostics;
    /// it must not make MAC validation failures non-fatal.
    pub has_missing_remove: bool,
}

/// Count of *records carrying an index* at or below which
/// [`HashState::update_hash_from_records`] dedups with a linear scan instead of
/// a sort; above it the sort wins despite ordering every entry. Note this counts
/// records, not distinct indices: a snapshot of 200 records over 100 indices
/// takes the sorted branch. Mirrors `MAC_DEDUP_SCAN_LIMIT` in `wacore`'s
/// appstate sync, which measured the same trade-off over the same kind of key
/// (#856).
const MAC_DEDUP_SCAN_LIMIT: usize = 64;

impl HashState {
    pub fn update_hash<F>(
        &mut self,
        mutations: &[wa::SyncdMutation],
        mut get_prev_set_value_mac: F,
    ) -> (HashUpdateResult, anyhow::Result<()>)
    where
        F: FnMut(&[u8], usize) -> anyhow::Result<Option<Vec<u8>>>,
    {
        // WA Web index-mode (WAWebSyncdAntiTampering, gate `d`): when every mutation
        // carries an index, a SET whose index is also REMOVEd in the same patch must
        // NOT subtract its previous value; the REMOVE owns that subtraction. Without
        // the guard the previous value is subtracted twice and the SET's value is
        // orphaned in the MAC store, leaving the ltHash and the store in permanent
        // disagreement. The legacy non-index path keeps the old math, like WA Web.
        // fn item, not a closure: the borrowed return needs HRTB (issue #825 class).
        fn index_mac_of(mutation: &wa::SyncdMutation) -> Option<&[u8]> {
            mutation
                .record
                .as_option()
                .and_then(|r| r.index.as_option())
                .and_then(|idx| idx.blob.as_deref())
        }
        let index_mode = mutations.iter().all(|m| index_mac_of(m).is_some());
        // Membership set over REMOVE index_macs, which are HMAC outputs (uniformly
        // random). A linear-scan Vec beats a SipHash HashSet at the patch sizes seen
        // in practice — the same trade-off as detect_duplicate_index_in_patch and
        // collect_unique_index_macs (#856). Only `.contains()` is queried below, so an
        // unconditional push is membership-equivalent to the set (a malformed duplicate
        // REMOVE is rejected by the duplicate-index guard regardless).
        let mut removed_in_patch: Vec<&[u8]> = Vec::new();
        if index_mode {
            for mutation in mutations {
                if mutation
                    .operation
                    .is_some_and(|op| op == wa::syncd_mutation::SyncdOperation::REMOVE)
                    && let Some(index_mac) = index_mac_of(mutation)
                {
                    removed_in_patch.push(index_mac);
                }
            }
        }

        // Borrow the MAC tails instead of copying; mirrors `update_hash_from_records`.
        let mut added: Vec<&[u8]> = Vec::with_capacity(mutations.len());
        let mut removed: Vec<Vec<u8>> = Vec::with_capacity(mutations.len());
        let mut result = HashUpdateResult::default();

        for (i, mutation) in mutations.iter().enumerate() {
            // SyncdOperation is an open enum; an unknown op can't be mapped to
            // add or subtract, so bail before touching the hash.
            let op = match mutation.operation {
                None => wa::syncd_mutation::SyncdOperation::SET,
                Some(v) => {
                    let Some(op) = v.as_known() else {
                        return (
                            result,
                            Err(anyhow::anyhow!(AppStateError::UnsupportedSyncdOperation(
                                v.to_i32()
                            ))),
                        );
                    };
                    op
                }
            };
            let is_set = op == wa::syncd_mutation::SyncdOperation::SET;
            if is_set
                && mutation.record.is_set()
                && let Some(blob) = &mutation.record.value.blob
                && blob.len() >= 32
            {
                added.push(&blob[blob.len() - 32..]);
            }
            if let Some(index_mac) = index_mac_of(mutation) {
                if is_set && removed_in_patch.contains(&index_mac) {
                    continue;
                }
                match get_prev_set_value_mac(index_mac, i) {
                    Ok(Some(prev)) => removed.push(prev),
                    Ok(None) => {
                        if op == wa::syncd_mutation::SyncdOperation::REMOVE {
                            result.has_missing_remove = true;
                            log::trace!(
                                target: "AppState",
                                "REMOVE mutation missing previous value (hasMissingRemove=true)"
                            );
                        }
                    }
                    Err(e) => return (result, Err(anyhow::anyhow!(e))),
                }
            }
        }

        // One call, not one per direction: `subtract_then_add_in_place` already
        // walks the subtract list then the add list, so splitting it in two ran
        // each list past an empty counterpart and set the 128-byte derivation
        // buffer up twice over.
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &removed, &added);
        (result, Ok(()))
    }

    /// Fold a snapshot's records into the ltHash.
    ///
    /// Takes the records directly rather than cloning into `SyncdMutation`:
    /// a snapshot is all SETs with no previous values to look up.
    ///
    /// A snapshot is keyed by index, not a list: WA Web builds
    /// `new Map(records.map(r => [hex(r.index.blob), valueMac(r.value.blob)]))`
    /// and folds `[...map.values()]` (`WAWebSyncdAntiTampering`), so when a
    /// snapshot carries the same index twice only the last record counts.
    /// Folding both instead leaves the ltHash one value MAC heavier than the one
    /// the server signed, and the snapshotMac can then never match — the
    /// collection is unsyncable for good, every recovery path answering
    /// `snapshot MAC mismatch`. The store already keys the same way
    /// (`put_mutation_macs`, `on_conflict((name, index_mac, device_id))`), so
    /// before this the ltHash and the store disagreed about the same snapshot.
    ///
    /// One deliberate difference from that Map: a record whose value blob is
    /// shorter than a MAC is dropped before the dedup, so the winner is the last
    /// *usable* record for an index rather than the last one. WA Web slices with
    /// a negative index there and folds the truncated buffer instead. Both are
    /// answers to a record the server should never send; ours refuses to fold
    /// something that is not a MAC.
    /// Answers what it folded rather than how many records it was given: a
    /// record whose value blob is too short to hold a MAC is dropped, and a
    /// repeated index contributes once. Diagnostics read this rather than
    /// restating the rule, which is how a count and a fold drift apart.
    pub fn update_hash_from_records(&mut self, records: &[wa::SyncdRecord]) -> FoldReport {
        // Borrow the MAC tails; no Vec<u8> allocation per MAC.
        let mut added: Vec<&[u8]> = Vec::with_capacity(records.len());
        let mut indexed: Vec<(&[u8], &[u8])> = Vec::with_capacity(records.len());
        // Counted here rather than recomputed by a caller, for the reason the
        // fold reports its own total: this arm is a known divergence from WA
        // Web, which keys every record through `hex(index.blob)` and so
        // collapses records carrying no index into a single entry where this
        // folds each of them. Harmless while at most one record is unkeyed,
        // and a silent wrong ltHash the moment two are -- with a folded count
        // that still equals the record count, which is exactly the shape a
        // diagnostic has to be able to see.
        let mut unkeyed = 0usize;

        for record in records {
            let Some(value_mac) = record
                .value
                .blob
                .as_ref()
                .filter(|blob| blob.len() >= 32)
                .map(|blob| &blob[blob.len() - 32..])
            else {
                continue;
            };

            match record.index.as_option().and_then(|idx| idx.blob.as_deref()) {
                // Keyed: the last record for this index is the one that counts.
                Some(index_mac) => indexed.push((index_mac, value_mac)),
                // Unkeyed records cannot collide with a keyed one, so they
                // always fold. Whether they collide with *each other* is the
                // divergence noted above, and `unkeyed` is what says so.
                None => {
                    unkeyed += 1;
                    added.push(value_mac)
                }
            }
        }

        if indexed.len() <= MAC_DEDUP_SCAN_LIMIT {
            // Index MACs are HMAC outputs (uniformly random), so at these sizes a
            // linear scan beats a SipHash set — the same trade-off as
            // collect_unique_index_macs (#856). Walking backwards makes the first
            // hit the last record, which is the one WA Web's Map keeps.
            let mut seen: Vec<&[u8]> = Vec::with_capacity(indexed.len());
            for (index_mac, value_mac) in indexed.iter().rev() {
                if !seen.contains(index_mac) {
                    seen.push(index_mac);
                    added.push(value_mac);
                }
            }
        } else {
            // Above the scan limit the sort wins despite ordering every entry.
            // `sort_by` is stable, so within one index the encounter order — and
            // therefore which record is last — survives.
            indexed.sort_by(|a, b| a.0.cmp(b.0));
            let mut rest = indexed.as_slice();
            while let Some((index_mac, _)) = rest.first() {
                let run = rest.partition_point(|(candidate, _)| candidate == index_mac);
                added.push(rest[run - 1].1);
                rest = &rest[run..];
            }
        }

        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &[] as &[&[u8]], &added);
        FoldReport {
            folded: added.len(),
            unkeyed,
        }
    }

    pub fn generate_snapshot_mac(&self, name: &str, key: &[u8]) -> Vec<u8> {
        let version_be = u64_to_be(self.version);
        let mut mac =
            CryptographicMac::new("HmacSha256", key).expect("HmacSha256 is a valid algorithm");
        mac.update(&self.hash);
        mac.update(&version_be);
        mac.update(name.as_bytes());
        mac.finalize()
    }
}

pub fn generate_patch_mac(patch: &wa::SyncdPatch, name: &str, key: &[u8], version: u64) -> Vec<u8> {
    let mut mac =
        CryptographicMac::new("HmacSha256", key).expect("HmacSha256 is a valid algorithm");

    // Feed directly to HMAC without collecting into Vec<Vec<u8>>
    if let Some(sm) = &patch.snapshot_mac {
        mac.update(sm);
    }
    for m in &patch.mutations {
        if m.record.is_set()
            && let Some(blob) = &m.record.value.blob
            && blob.len() >= 32
        {
            mac.update(&blob[blob.len() - 32..]);
        }
    }
    mac.update(&u64_to_be(version));
    mac.update(name.as_bytes());

    mac.finalize()
}

fn u64_to_be(val: u64) -> [u8; 8] {
    val.to_be_bytes()
}

pub fn generate_content_mac(
    operation: wa::syncd_mutation::SyncdOperation,
    data: &[u8],
    key_id: &[u8],
    key: &[u8],
) -> [u8; 32] {
    let op_byte = [operation as u8 + 1];
    // WA Web (WAWebSyncdMutationKeyApi.Crypto) packs the associated-data length as
    // a single u8 at the low byte of an 8-byte zero buffer:
    //   octetLength = new Uint8Array(8); octetLength[7] = ad.length & 0xff
    // We mirror that exactly so the HMAC input is bytewise identical.
    let mut key_data_length = [0u8; 8];
    key_data_length[7] = ((key_id.len() + 1) & 0xff) as u8;
    let mut mac =
        CryptographicMac::new("HmacSha512", key).expect("HmacSha512 is a valid algorithm");
    mac.update(&op_byte);
    mac.update(key_id);
    mac.update(data);
    mac.update(&key_data_length);
    let mut out = [0u8; 64];
    mac.finalize_into(&mut out)
        .expect("64 bytes is enough for HmacSha512");
    let mut result = [0u8; 32];
    result.copy_from_slice(&out[..32]);
    result
}

pub fn generate_index_mac(index_json_bytes: &[u8], key: &[u8; 32]) -> Vec<u8> {
    let mut mac =
        CryptographicMac::new("HmacSha256", key).expect("HmacSha256 is a valid algorithm");
    mac.update(index_json_bytes);
    mac.finalize()
}

pub fn validate_index_mac(
    index_json_bytes: &[u8],
    expected_mac: &[u8],
    key: &[u8; 32],
) -> Result<(), AppStateError> {
    if generate_index_mac(index_json_bytes, key).as_slice() != expected_mac {
        Err(AppStateError::MismatchingIndexMAC)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mutation(
        operation: wa::syncd_mutation::SyncdOperation,
        index_mac: Vec<u8>,
        value_mac: Option<Vec<u8>>,
    ) -> wa::SyncdMutation {
        let value_blob = value_mac.map(|mac| {
            let mut blob = vec![0u8; 16];
            blob.extend_from_slice(&mac);
            blob
        });

        let value = if let Some(b) = value_blob {
            buffa::MessageField::some(wa::SyncdValue { blob: Some(b) })
        } else {
            buffa::MessageField::none()
        };

        wa::SyncdMutation {
            operation: Some(operation.into()),
            record: buffa::MessageField::some(wa::SyncdRecord {
                index: buffa::MessageField::some(wa::SyncdIndex {
                    blob: Some(index_mac),
                }),
                value,
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(b"test_key_id".to_vec()),
                }),
            }),
        }
    }

    #[test]
    fn test_update_hash_with_set_overwrite_and_remove() {
        const INDEX_MAC_1: &[u8] = &[1; 32];
        const VALUE_MAC_1: &[u8] = &[10; 32];

        const INDEX_MAC_2: &[u8] = &[2; 32];
        const VALUE_MAC_2: &[u8] = &[20; 32];

        const VALUE_MAC_3_OVERWRITE: &[u8] = &[30; 32];

        let mut prev_macs = HashMap::<Vec<u8>, Vec<u8>>::new();

        let mut state = HashState::default();
        let initial_mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_1.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_2.to_vec(),
                Some(VALUE_MAC_2.to_vec()),
            ),
        ];

        let get_prev_mac_closure = |_: &[u8], _: usize| Ok(None);
        let (hash_result, result) = state.update_hash(&initial_mutations, get_prev_mac_closure);
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);

        const EMPTY: &[Vec<u8>] = &[];
        let expected_hash_after_add = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            EMPTY,
            &[VALUE_MAC_1.to_vec(), VALUE_MAC_2.to_vec()],
        );
        assert_eq!(state.hash.as_slice(), expected_hash_after_add.as_slice());

        prev_macs.insert(INDEX_MAC_1.to_vec(), VALUE_MAC_1.to_vec());
        prev_macs.insert(INDEX_MAC_2.to_vec(), VALUE_MAC_2.to_vec());

        let update_and_remove_mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_3_OVERWRITE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC_2.to_vec(),
                None,
            ),
        ];

        let get_prev_mac_closure_phase2 =
            |index_mac: &[u8], _: usize| Ok(prev_macs.get(index_mac).cloned());
        let (hash_result, result) =
            state.update_hash(&update_and_remove_mutations, get_prev_mac_closure_phase2);
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);

        let expected_final_hash = WAPATCH_INTEGRITY.subtract_then_add(
            &expected_hash_after_add,
            &[VALUE_MAC_1.to_vec(), VALUE_MAC_2.to_vec()],
            &[VALUE_MAC_3_OVERWRITE.to_vec()],
        );

        assert_eq!(
            state.hash.as_slice(),
            expected_final_hash.as_slice(),
            "The final hash state after overwrite and remove is incorrect."
        );
    }

    /// WA Web index-mode: a SET whose index is also REMOVEd in the same patch must
    /// not subtract its previous value; only the REMOVE subtracts (the store value).
    #[test]
    fn test_update_hash_set_plus_remove_same_index_subtracts_once() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const PREV_VALUE: &[u8] = &[10; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(PREV_VALUE.to_vec()),
            ),
        ];

        let mut state = HashState::default();
        let mut lookups = 0usize;
        let (hash_result, result) = state.update_hash(&mutations, |_, _| {
            lookups += 1;
            Ok(Some(PREV_VALUE.to_vec()))
        });
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);
        assert_eq!(lookups, 1, "the suppressed SET must not query the store");

        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            &[PREV_VALUE.to_vec()],
            &[NEW_VALUE.to_vec()],
        );
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// Index-mode is gated on every mutation carrying an index (WA Web's `d`):
    /// with one index-less mutation in the patch, the SET subtracts as before.
    #[test]
    fn test_update_hash_suppression_disabled_without_full_index_coverage() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const PREV_VALUE: &[u8] = &[10; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mut index_less = create_mutation(
            wa::syncd_mutation::SyncdOperation::SET,
            vec![],
            Some(vec![30; 32]),
        );
        if let Some(rec) = index_less.record.as_option_mut() {
            rec.index = Default::default();
        }

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(PREV_VALUE.to_vec()),
            ),
            index_less,
        ];

        let mut state = HashState::default();
        let (_, result) = state.update_hash(&mutations, |_, _| Ok(Some(PREV_VALUE.to_vec())));
        assert!(result.is_ok());

        // Legacy math: SET and REMOVE each subtract the previous value.
        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            &[PREV_VALUE.to_vec(), PREV_VALUE.to_vec()],
            &[NEW_VALUE.to_vec(), vec![30; 32]],
        );
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// SET+REMOVE same index against an empty store: the SET still adds, the REMOVE
    /// finds nothing and flags has_missing_remove, matching WA Web index-mode which
    /// has no fallback query.
    #[test]
    fn test_update_hash_set_plus_remove_same_index_empty_store() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
        ];

        let mut state = HashState::default();
        let (hash_result, result) = state.update_hash(&mutations, |_, _| Ok(None));
        assert!(result.is_ok());
        assert!(hash_result.has_missing_remove);

        const EMPTY: &[Vec<u8>] = &[];
        let expected = WAPATCH_INTEGRITY.subtract_then_add(&[0; 128], EMPTY, &[NEW_VALUE.to_vec()]);
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// Known-answer test for generate_patch_mac to guard byte ordering and input
    /// concatenation.  The expected MAC was computed by feeding:
    ///   snapshot_mac ‖ mutation1_tail(32) ‖ mutation2_tail(32)
    ///   ‖ u64_to_be(42) ‖ b"regular_high"
    /// into HMAC-SHA256 with key = [0xAA; 32].
    #[test]
    fn test_generate_patch_mac_known_answer() {
        let key = [0xAAu8; 32];
        let name = "regular_high";
        let version: u64 = 42;

        // Build a patch with snapshot_mac and two mutations with >=32 byte blobs.
        let snapshot_mac = vec![0x11u8; 32];
        let mut blob1 = vec![0u8; 16]; // 16 prefix bytes
        blob1.extend_from_slice(&[0x22u8; 32]); // 32-byte tail taken by generate_patch_mac
        let mut blob2 = vec![0u8; 16];
        blob2.extend_from_slice(&[0x33u8; 32]);

        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion {
                version: Some(version),
            }),
            snapshot_mac: Some(snapshot_mac.clone()),
            mutations: vec![
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::SET.into()),
                    record: buffa::MessageField::some(wa::SyncdRecord {
                        value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob1) }),
                        ..Default::default()
                    }),
                },
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::SET.into()),
                    record: buffa::MessageField::some(wa::SyncdRecord {
                        value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob2) }),
                        ..Default::default()
                    }),
                },
            ],
            ..Default::default()
        };

        // Compute expected MAC manually using the same HMAC-SHA256 inputs.
        let mut expected_mac =
            CryptographicMac::new("HmacSha256", &key).expect("HmacSha256 is a valid algorithm");
        expected_mac.update(&snapshot_mac); // snapshot_mac
        expected_mac.update(&[0x22u8; 32]); // mutation 1 tail
        expected_mac.update(&[0x33u8; 32]); // mutation 2 tail
        expected_mac.update(&42u64.to_be_bytes()); // version
        expected_mac.update(b"regular_high"); // name
        let expected = expected_mac.finalize();

        let actual = generate_patch_mac(&patch, name, &key, version);
        assert_eq!(
            actual, expected,
            "generate_patch_mac output must match manual HMAC-SHA256 computation"
        );
    }

    fn record_with(index_mac: &[u8], value_mac: &[u8]) -> wa::SyncdRecord {
        let mut blob = vec![0u8; 16];
        blob.extend_from_slice(value_mac);
        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(index_mac.to_vec()),
            }),
            value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(b"test_key_id".to_vec()),
            }),
        }
    }

    /// Production symptom: `regular_low` never syncs again, every recovery path
    /// answering `snapshot MAC mismatch`, because a snapshot carrying the same
    /// index twice folds both value MACs into the ltHash.
    ///
    /// WA Web keys the records by index before folding
    /// (`WAWebSyncdAntiTampering`: `new Map(records.map(r => [hex(r.index.blob),
    /// valueMac(r.value.blob)]))`, then `add(EMPTY_LT_HASH, [...d.values()])`),
    /// so the last record for an index is the only one that counts.
    #[test]
    fn a_repeated_index_in_a_snapshot_folds_only_its_last_value() {
        const INDEX_A: &[u8] = &[1u8; 32];
        const INDEX_B: &[u8] = &[2u8; 32];
        const STALE: &[u8] = &[0xAAu8; 32];
        const WINNER: &[u8] = &[0xBBu8; 32];
        const OTHER: &[u8] = &[0xCCu8; 32];

        let records = vec![
            record_with(INDEX_A, STALE),
            record_with(INDEX_A, WINNER),
            record_with(INDEX_B, OTHER),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(
            &mut expected,
            &[] as &[&[u8]],
            &[WINNER, OTHER],
        );

        assert_eq!(
            state.hash, expected,
            "a repeated index must contribute only its last value MAC, as WA Web's Map does"
        );
    }

    /// Review of #1365: a bootstrap that persisted page five of sixty and then
    /// stopped has a baseline and is still behind the head. Reading a positive
    /// version as a finished bootstrap let the send guard skip its preflight and
    /// build the user's patch on that cursor.
    #[test]
    fn a_partial_bootstrap_has_a_baseline_but_has_not_bootstrapped() {
        let partial = HashState {
            version: 5,
            ..Default::default()
        };
        assert!(
            partial.has_baseline(),
            "page five established a baseline, so patches apply over it"
        );
        assert!(
            !partial.bootstrapped,
            "but the run never reached its terminal page"
        );
    }

    /// The upgrade case the baseline question exists for: a row written before
    /// the flag existed, sitting on real data.
    #[test]
    fn a_legacy_row_past_zero_still_counts_as_having_a_baseline() {
        let legacy = HashState {
            version: 7,
            ..Default::default()
        };
        assert!(
            legacy.has_baseline(),
            "asking such a row for a snapshot would re-download one per collection \
             on upgrade, for something the version already answers"
        );
    }

    /// Version zero is the ambiguous case, and the only one the flag decides.
    #[test]
    fn at_version_zero_only_the_flag_separates_never_synced_from_empty() {
        let never = HashState::default();
        let empty_but_synced = HashState {
            bootstrapped: true,
            ..Default::default()
        };
        assert_eq!(never.version, empty_but_synced.version);
        assert_eq!(never.hash, empty_but_synced.hash);
        assert!(!never.has_baseline());
        assert!(empty_but_synced.has_baseline());
    }

    /// The branch production actually takes. `MAC_DEDUP_SCAN_LIMIT` counts
    /// records carrying an index, not distinct indices, so any real snapshot --
    /// `regular_low` on an active account is hundreds of records -- sorts rather
    /// than scans. The linear branch the other tests exercise is the one a real
    /// account almost never reaches.
    #[test]
    fn a_repeated_index_is_deduped_above_the_scan_limit_too() {
        const DUPLICATED: &[u8] = &[0xEEu8; 32];
        const STALE: &[u8] = &[0x11u8; 32];
        const WINNER: &[u8] = &[0x22u8; 32];

        // Comfortably past the limit, so this takes the sorted branch.
        let filler = MAC_DEDUP_SCAN_LIMIT * 2;
        let mut records = Vec::with_capacity(filler + 2);
        let mut expected_macs: Vec<[u8; 32]> = Vec::with_capacity(filler + 1);

        // The stale copy first, and the winner last, with the whole filler set
        // between them: the sort has to keep encounter order within the index.
        records.push(record_with(DUPLICATED, STALE));
        for i in 0..filler {
            let mut index_mac = [0u8; 32];
            index_mac[..8].copy_from_slice(&(i as u64).to_le_bytes());
            let mac = [(i % 251) as u8; 32];
            records.push(record_with(&index_mac, &mac));
            expected_macs.push(mac);
        }
        records.push(record_with(DUPLICATED, WINNER));
        expected_macs.push(*<&[u8; 32]>::try_from(WINNER).unwrap());

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let added: Vec<&[u8]> = expected_macs.iter().map(|m| m.as_slice()).collect();
        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &added);

        assert_eq!(
            state.hash, expected,
            "above the scan limit the sort must still keep the last record for a \
             repeated index, and only that one"
        );
    }

    /// Three records under one index, the winner in last place. Guards
    /// `rest[run - 1]` against picking the middle of a run.
    #[test]
    fn the_last_of_a_run_of_three_is_the_one_that_folds() {
        const INDEX: &[u8] = &[7u8; 32];
        const FIRST: &[u8] = &[0xA1u8; 32];
        const MIDDLE: &[u8] = &[0xA2u8; 32];
        const LAST: &[u8] = &[0xA3u8; 32];

        let records = vec![
            record_with(INDEX, FIRST),
            record_with(INDEX, MIDDLE),
            record_with(INDEX, LAST),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &[LAST]);

        assert_eq!(
            state.hash, expected,
            "a run of three must fold its last record, not its first or middle"
        );
    }

    /// The ordinary snapshot: every index distinct. Guards the dedup from
    /// changing what it must not change.
    #[test]
    fn distinct_indices_in_a_snapshot_all_fold() {
        const MAC_1: &[u8] = &[0x11u8; 32];
        const MAC_2: &[u8] = &[0x22u8; 32];

        let records = vec![
            record_with(&[1u8; 32], MAC_1),
            record_with(&[2u8; 32], MAC_2),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(
            &mut expected,
            &[] as &[&[u8]],
            &[MAC_1, MAC_2],
        );

        assert_eq!(state.hash, expected);
    }

    /// A record with no index cannot be keyed, so it folds unconditionally —
    /// the arm the dedup must leave alone.
    #[test]
    fn a_record_without_an_index_still_folds() {
        const MAC: &[u8] = &[0x44u8; 32];
        let mut blob = vec![0u8; 16];
        blob.extend_from_slice(MAC);
        let record = wa::SyncdRecord {
            value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) }),
            ..Default::default()
        };

        let mut state = HashState::default();
        state.update_hash_from_records(&[record]);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &[MAC]);

        assert_eq!(state.hash, expected);
    }

    /// The fold says how many records reached it without an index, because
    /// that arm is a known divergence from WA Web and the count that would
    /// otherwise expose it does not: two unkeyed records fold twice here and
    /// once there -- WA Web keys every record through `hex(index.blob)`, so
    /// records carrying no index collapse into one entry -- while `folded`
    /// still equals the number of records, which is what a snapshot MAC
    /// mismatch looks like when nothing else is wrong.
    ///
    /// Pins the reporting rather than the divergence: correcting the fold to
    /// match WA Web is a change to a live account's ltHash and wants evidence
    /// that such records exist, which is what this number is for.
    #[test]
    fn the_fold_reports_records_that_carried_no_index() {
        let unkeyed = |mac: u8| {
            let mut blob = vec![0u8; 16];
            blob.extend_from_slice(&[mac; 32]);
            wa::SyncdRecord {
                value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) }),
                ..Default::default()
            }
        };

        let mut state = HashState::default();
        let report = state.update_hash_from_records(&[
            record_with(&[0x01; 32], &[0x11; 32]),
            unkeyed(0x22),
            unkeyed(0x33),
        ]);

        assert_eq!(
            report.folded, 3,
            "every record carried a foldable value MAC"
        );
        assert_eq!(report.unkeyed, 2, "two of them carried no index MAC");

        // The number a caller would reach for cannot tell the two apart, which
        // is the whole reason the fold answers instead.
        assert_eq!(
            report.folded, 3,
            "and the folded count still equals the record count"
        );
    }
}
