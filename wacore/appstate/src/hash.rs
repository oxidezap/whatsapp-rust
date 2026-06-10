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
}

impl Default for HashState {
    fn default() -> Self {
        Self {
            version: 0,
            hash: [0; 128],
            index_value_map: HashMap::new(),
        }
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
                .as_ref()
                .and_then(|r| r.index.as_ref())
                .and_then(|idx| idx.blob.as_deref())
        }
        let index_mode = mutations.iter().all(|m| index_mac_of(m).is_some());
        let mut removed_in_patch: std::collections::HashSet<&[u8]> =
            std::collections::HashSet::new();
        if index_mode {
            for mutation in mutations {
                if mutation.operation.unwrap_or_default()
                    == wa::syncd_mutation::SyncdOperation::Remove as i32
                    && let Some(index_mac) = index_mac_of(mutation)
                {
                    removed_in_patch.insert(index_mac);
                }
            }
        }

        // Borrow the MAC tails instead of copying; mirrors `update_hash_from_records`.
        let mut added: Vec<&[u8]> = Vec::with_capacity(mutations.len());
        let mut removed: Vec<Vec<u8>> = Vec::with_capacity(mutations.len());
        let mut result = HashUpdateResult::default();

        for (i, mutation) in mutations.iter().enumerate() {
            let op = mutation.operation.unwrap_or_default();
            let is_set = op == wa::syncd_mutation::SyncdOperation::Set as i32;
            if is_set
                && let Some(record) = &mutation.record
                && let Some(value) = &record.value
                && let Some(blob) = &value.blob
                && blob.len() >= 32
            {
                added.push(&blob[blob.len() - 32..]);
            }
            if let Some(index_mac) = index_mac_of(mutation) {
                if is_set && removed_in_patch.contains(index_mac) {
                    continue;
                }
                match get_prev_set_value_mac(index_mac, i) {
                    Ok(Some(prev)) => removed.push(prev),
                    Ok(None) => {
                        if op == wa::syncd_mutation::SyncdOperation::Remove as i32 {
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

        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &removed, &[] as &[Vec<u8>]);
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &[] as &[&[u8]], &added);
        (result, Ok(()))
    }

    /// Update hash state from snapshot records directly (avoids cloning into SyncdMutation).
    ///
    /// This is an optimized version for snapshots where all operations are SET
    /// and there are no previous values to look up.
    pub fn update_hash_from_records(&mut self, records: &[wa::SyncdRecord]) {
        // Collect slices directly — no Vec<u8> allocation per MAC.
        let added: Vec<&[u8]> = records
            .iter()
            .filter_map(|record| {
                record
                    .value
                    .as_ref()
                    .and_then(|v| v.blob.as_ref())
                    .filter(|blob| blob.len() >= 32)
                    .map(|blob| &blob[blob.len() - 32..])
            })
            .collect();

        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &[] as &[&[u8]], &added);
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
        if let Some(record) = &m.record
            && let Some(val) = &record.value
            && let Some(blob) = &val.blob
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

        wa::SyncdMutation {
            operation: Some(operation as i32),
            record: Some(wa::SyncdRecord {
                index: Some(wa::SyncdIndex {
                    blob: Some(index_mac),
                }),
                value: value_blob.map(|b| wa::SyncdValue { blob: Some(b) }),
                key_id: Some(wa::KeyId {
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
                wa::syncd_mutation::SyncdOperation::Set,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_1.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Set,
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
                wa::syncd_mutation::SyncdOperation::Set,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_3_OVERWRITE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Remove,
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
                wa::syncd_mutation::SyncdOperation::Set,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Remove,
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
            wa::syncd_mutation::SyncdOperation::Set,
            vec![],
            Some(vec![30; 32]),
        );
        if let Some(rec) = index_less.record.as_mut() {
            rec.index = None;
        }

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Set,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Remove,
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
                wa::syncd_mutation::SyncdOperation::Set,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::Remove,
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
            version: Some(wa::SyncdVersion {
                version: Some(version),
            }),
            snapshot_mac: Some(snapshot_mac.clone()),
            mutations: vec![
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::Set as i32),
                    record: Some(wa::SyncdRecord {
                        index: None,
                        value: Some(wa::SyncdValue { blob: Some(blob1) }),
                        key_id: None,
                    }),
                },
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::Set as i32),
                    record: Some(wa::SyncdRecord {
                        index: None,
                        value: Some(wa::SyncdValue { blob: Some(blob2) }),
                        key_id: None,
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
}
