//! Pure, synchronous patch and snapshot processing logic for app state.
//!
//! This module provides runtime-agnostic processing of app state patches and snapshots.
//! All functions are synchronous and take callbacks for key lookup, making them
//! suitable for use in both async and sync contexts.

use crate::AppStateError;
use crate::decode::{Mutation, decode_record};
use crate::hash::{HashState, generate_patch_mac};
use crate::keys::ExpandedAppStateKeys;
use log::{debug, trace};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use waproto::whatsapp as wa;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppStateMutationMAC {
    pub index_mac: Vec<u8>,
    pub value_mac: Vec<u8>,
}

/// Result of processing a snapshot.
#[derive(Debug, Clone)]
pub struct ProcessedSnapshot {
    /// The updated hash state after processing.
    pub state: HashState,
    /// The decoded mutations from the snapshot.
    pub mutations: Vec<Mutation>,
    /// The mutation MACs to store (for later patch processing).
    pub mutation_macs: Vec<AppStateMutationMAC>,
}

/// Result of processing a single patch.
#[derive(Debug, Clone)]
pub struct PatchProcessingResult {
    /// The updated hash state after processing.
    pub state: HashState,
    /// The decoded mutations from the patch.
    pub mutations: Vec<Mutation>,
    /// The mutation MACs that were added.
    pub added_macs: Vec<AppStateMutationMAC>,
    /// The index MACs that were removed.
    pub removed_index_macs: Vec<Vec<u8>>,
}

/// Process a snapshot and decode all its records.
///
/// This is a pure, synchronous function that processes a snapshot without
/// any async operations. Key lookup is done via a callback.
///
/// # Arguments
/// * `snapshot` - The snapshot to process
/// * `initial_state` - The initial hash state (will be mutated in place)
/// * `get_keys` - Callback to get expanded keys for a key ID
/// * `validate_macs` - Whether to validate MACs during processing
/// * `collection_name` - The collection name (for MAC validation)
///
/// # Returns
/// A `ProcessedSnapshot` containing the new state and decoded mutations.
pub fn process_snapshot<F>(
    snapshot: &wa::SyncdSnapshot,
    initial_state: &mut HashState,
    mut get_keys: F,
    validate_macs: bool,
    collection_name: &str,
) -> Result<ProcessedSnapshot, AppStateError>
where
    F: FnMut(&[u8]) -> Result<Arc<ExpandedAppStateKeys>, AppStateError>,
{
    let version = snapshot.version.version.unwrap_or(0);
    initial_state.version = version;

    // Update hash state directly from records (no cloning needed)
    initial_state.update_hash_from_records(&snapshot.records);

    debug!(
        target: "AppState",
        "Snapshot {} v{}: {} records, ltHash ends with ...{}",
        collection_name,
        version,
        snapshot.records.len(),
        hex::encode(&initial_state.hash[120..])
    );

    // Validate snapshot MAC if requested. A snapshot that omits `mac`/`key_id` is
    // treated as a validation FAILURE, not skipped: WA Web's anti-tampering
    // compares against the (possibly undefined) mac and fires the recovery path on
    // mismatch, so a missing mac must not silently accept unverified records.
    if validate_macs {
        let (Some(mac_expected), Some(key_id)) = (
            snapshot.mac.as_ref(),
            snapshot.key_id.as_option().and_then(|k| k.id.as_deref()),
        ) else {
            return Err(AppStateError::SnapshotMACMismatch);
        };
        let keys = get_keys(key_id)?;
        let computed = initial_state.generate_snapshot_mac(collection_name, &keys.snapshot_mac);
        trace!(
            target: "AppState",
            "Snapshot {} v{} MAC validation: computed={}, expected={}",
            collection_name,
            version,
            hex::encode(&computed),
            hex::encode(mac_expected)
        );
        if computed != *mac_expected {
            return Err(AppStateError::SnapshotMACMismatch);
        }
    }

    // Decode all records and collect MACs in a single pass
    let mut mutations = Vec::with_capacity(snapshot.records.len());
    let mut mutation_macs = Vec::with_capacity(snapshot.records.len());

    for rec in &snapshot.records {
        let key_id = rec.key_id.id.as_ref().ok_or(AppStateError::MissingKeyId)?;
        let keys = get_keys(key_id)?;

        let (mutation, macs) = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            rec,
            &keys,
            key_id,
            validate_macs,
        )?;

        mutation_macs.push(AppStateMutationMAC {
            index_mac: macs.index_mac,
            value_mac: macs.value_mac,
        });

        mutations.push(mutation);
    }

    Ok(ProcessedSnapshot {
        state: initial_state.clone(),
        mutations,
        mutation_macs,
    })
}

/// Process a single patch and decode its mutations.
///
/// This is a pure, synchronous function that processes a patch without
/// any async operations. Key and previous value lookup are done via callbacks.
///
/// # Arguments
/// * `patch` - The patch to process
/// * `state` - The current hash state (will be mutated in place)
/// * `get_keys` - Callback to get expanded keys for a key ID
/// * `get_prev_value_mac` - Callback to get previous value MAC for an index MAC
/// * `validate_macs` - Whether to validate MACs during processing
/// * `collection_name` - The collection name (for MAC validation)
///
/// # Returns
/// A `PatchProcessingResult` containing the new state and decoded mutations.
pub fn process_patch<F, G>(
    patch: &wa::SyncdPatch,
    state: &mut HashState,
    mut get_keys: F,
    mut get_prev_value_mac: G,
    validate_macs: bool,
    collection_name: &str,
) -> Result<PatchProcessingResult, AppStateError>
where
    F: FnMut(&[u8]) -> Result<Arc<ExpandedAppStateKeys>, AppStateError>,
    G: FnMut(&[u8]) -> Result<Option<Vec<u8>>, AppStateError>,
{
    // Capture original state before modification - needed for MAC validation logic
    // If original state was empty (version=0, hash all zeros), we cannot validate
    // snapshotMac because we don't have the baseline state the patch was built against.
    // This matches WhatsApp Web behavior which throws a retryable error in this case.
    let original_version = state.version;
    let original_hash_is_empty = state.hash == [0u8; 128];
    let had_no_prior_state = original_version == 0 && original_hash_is_empty;

    let patch_version = patch.version.version.unwrap_or(0);

    // WA Web: validatePatchVersion — strict monotonic version check.
    // Patch version must be exactly local_version + 1.  If not, WA Web throws
    // "syncd-version-check-error-local-version-{greater|less}-than-expected".
    // Skip this check when we have no prior state (version=0, empty hash),
    // since we don't have a baseline to validate against.
    let expected_version = original_version.saturating_add(1);
    if !had_no_prior_state && patch_version != expected_version {
        return Err(AppStateError::PatchVersionMismatch {
            expected: expected_version,
            got: patch_version,
        });
    }

    state.version = patch_version;

    // index_mac -> most-recent in-patch value MAC tail, filled as we iterate. Replaces a
    // reverse scan over patch.mutations[..idx] (O(n^2) total) with an O(1) lookup, mirroring
    // WA Web's WAWebSyncdAntiTampering Map. Recording the current value only after the lookup
    // keeps the old strictly-prior semantics: a mutation never matches itself, and a SET that
    // overwrites the same index earlier in the patch takes precedence over the DB value.
    let mut in_patch: HashMap<&[u8], &[u8]> = HashMap::with_capacity(patch.mutations.len());
    let (hash_update_result, result) = state.update_hash(&patch.mutations, |index_mac, idx| {
        // WA Web resolves every previous value against the store map fetched before
        // the loop; the in-patch overlay only models SET-overwrite collapse and must
        // never feed a REMOVE (a REMOVE preceded by a SET on the same index would
        // otherwise subtract the in-patch value instead of the store's).
        let is_remove = matches!(
            patch.mutations[idx].operation,
            Some(wa::syncd_mutation::SyncdOperation::REMOVE)
        );
        let prev = if !is_remove && let Some(value_mac) = in_patch.get(index_mac) {
            Some(value_mac.to_vec())
        } else {
            get_prev_value_mac(index_mac).map_err(|e| anyhow::anyhow!(e))?
        };
        if let Some(rec) = patch.mutations[idx].record.as_option()
            && let Some(index) = rec.index.as_option().and_then(|i| i.blob.as_deref())
            && let Some(value) = rec.value.as_option().and_then(|v| v.blob.as_deref())
            && value.len() >= 32
        {
            in_patch.insert(index, &value[value.len() - 32..]);
        }
        Ok(prev)
    });
    result.map_err(|_| AppStateError::MismatchingLTHash)?;

    debug!(
        target: "AppState",
        "Patch {} v{}: {} mutations, ltHash ends with ...{}, hasMissingRemove={}",
        collection_name,
        state.version,
        patch.mutations.len(),
        hex::encode(&state.hash[120..]),
        hash_update_result.has_missing_remove
    );

    // Validate MACs if requested
    if validate_macs && let Some(key_id) = patch.key_id.id.as_ref() {
        let keys = get_keys(key_id)?;
        validate_patch_macs(
            patch,
            state,
            &keys,
            collection_name,
            had_no_prior_state,
            hash_update_result.has_missing_remove,
        )?;
    }

    // Anti-tampering parity: a repeated index within the same operation of one patch
    // is fatal in WA Web (validateNoSameIndexForMultipleMutations -> SyncdFatalError),
    // and the cryptographic patch/snapshot MACs above don't catch it (a duplicate-index
    // patch still MACs correctly). Runs only on the validated inbound path.
    if validate_macs {
        detect_duplicate_index_in_patch(&patch.mutations)?;
    }

    // Decode all mutations and collect MACs in a single pass
    let mut mutations = Vec::with_capacity(patch.mutations.len());
    let mut added_macs = Vec::with_capacity(patch.mutations.len());
    let mut removed_index_macs = Vec::with_capacity(patch.mutations.len());

    for m in &patch.mutations {
        if m.record.is_set() {
            let op = m
                .operation
                .unwrap_or(wa::syncd_mutation::SyncdOperation::SET);

            let key_id = m
                .record
                .key_id
                .id
                .as_ref()
                .ok_or(AppStateError::MissingKeyId)?;
            let keys = get_keys(key_id)?;

            let (mutation, macs) = decode_record(op, &m.record, &keys, key_id, validate_macs)?;

            match op {
                wa::syncd_mutation::SyncdOperation::SET => {
                    added_macs.push(AppStateMutationMAC {
                        index_mac: macs.index_mac,
                        value_mac: macs.value_mac,
                    });
                }
                wa::syncd_mutation::SyncdOperation::REMOVE => {
                    removed_index_macs.push(macs.index_mac);
                }
            }

            mutations.push(mutation);
        }
    }

    Ok(PatchProcessingResult {
        state: state.clone(),
        mutations,
        added_macs,
        removed_index_macs,
    })
}

/// Reject a patch that repeats an index within the same operation.
///
/// Mirrors WA Web `WAWebSyncdValidateMutations.validateNoSameIndexForMultipleMutations`,
/// which keeps one Set per operation (SET, REMOVE) and throws a fatal
/// `SAME_INDEX_FOR_MULTIPLE_MUTATIONS_IN_PATCH` when an index reappears in the same one.
/// WA Web keys on the decrypted index; the raw index_mac blob is a deterministic
/// function of that index, so keying on it is equivalent for detection.
fn detect_duplicate_index_in_patch(mutations: &[wa::SyncdMutation]) -> Result<(), AppStateError> {
    // index_macs are HMAC outputs (uniformly random), so a HashSet only buys
    // SipHash setup plus an allocation for no distribution benefit. A linear scan
    // wins at the patch sizes seen in practice — the same trade-off measured for
    // collect_unique_index_macs (#856). Set and Remove are deduped independently:
    // a Set and a Remove may legitimately carry the same index within one patch.
    let mut seen_set: Vec<&[u8]> = Vec::new();
    let mut seen_remove: Vec<&[u8]> = Vec::new();
    for m in mutations {
        let Some(rec) = m.record.as_option() else {
            continue;
        };
        let Some(index_mac) = rec.index.as_option().and_then(|i| i.blob.as_deref()) else {
            continue;
        };
        let op = m
            .operation
            .unwrap_or(wa::syncd_mutation::SyncdOperation::SET);
        let seen = match op {
            wa::syncd_mutation::SyncdOperation::SET => &mut seen_set,
            wa::syncd_mutation::SyncdOperation::REMOVE => &mut seen_remove,
        };
        if seen.contains(&index_mac) {
            return Err(AppStateError::DuplicateIndexInPatch);
        }
        seen.push(index_mac);
    }
    Ok(())
}

/// Validate the snapshot and patch MACs for a patch.
///
/// This is a pure function that validates the MACs without any I/O.
///
/// # Arguments
/// * `patch` - The patch to validate
/// * `state` - The hash state AFTER applying the patch mutations
/// * `keys` - The expanded app state keys for MAC computation
/// * `collection_name` - The collection name
/// * `had_no_prior_state` - If true, skip ALL MAC validation. On an empty collection
///   this is only reached for the genesis patch (version 1), which has no prior baseline
///   to validate against. The empty + non-genesis case (a patch without a snapshot that
///   can't anchor the ltHash) is rejected upstream in `process_patch_list`, which marks
///   the collection retryable so it re-syncs via snapshot, matching WhatsApp Web.
/// * `has_missing_remove` - If true, a REMOVE mutation was missing its previous value.
///   WhatsApp Web reports this as MAC-failure telemetry, but it does not make
///   aggregate MAC mismatches acceptable.
pub fn validate_patch_macs(
    patch: &wa::SyncdPatch,
    state: &HashState,
    keys: &ExpandedAppStateKeys,
    collection_name: &str,
    had_no_prior_state: bool,
    has_missing_remove: bool,
) -> Result<(), AppStateError> {
    // Skip ALL MAC validation only for the genesis patch (version 1) seeding an
    // empty baseline: there is no prior snapshotMac/patchMac baseline to validate
    // against. The empty + non-genesis case (a patch without a snapshot that can't
    // anchor the ltHash) is rejected upstream in `process_patch_list` as a retryable
    // resync, so it never reaches here.
    if had_no_prior_state {
        return Ok(());
    }

    if let Some(snap_mac) = patch.snapshot_mac.as_ref() {
        let computed_snap = state.generate_snapshot_mac(collection_name, &keys.snapshot_mac);
        trace!(
            target: "AppState",
            "Patch {} v{} snapshotMAC: computed={}, expected={}",
            collection_name,
            state.version,
            hex::encode(&computed_snap),
            hex::encode(snap_mac)
        );
        if computed_snap != *snap_mac {
            debug!(
                target: "AppState",
                "Patch {} v{} snapshotMAC MISMATCH! ltHash=...{}, hasMissingRemove={}",
                collection_name,
                state.version,
                hex::encode(&state.hash[120..]),
                has_missing_remove
            );
            return Err(AppStateError::PatchSnapshotMACMismatch);
        }
    }

    if let Some(patch_mac) = patch.patch_mac.as_ref() {
        let version = patch.version.version.unwrap_or(0);
        let computed_patch = generate_patch_mac(patch, collection_name, &keys.patch_mac, version);
        if computed_patch != *patch_mac {
            debug!(
                target: "AppState",
                "Patch {} v{} patchMAC MISMATCH, hasMissingRemove={}",
                collection_name,
                state.version,
                has_missing_remove
            );
            return Err(AppStateError::PatchMACMismatch);
        }
    }

    Ok(())
}

/// Validate a snapshot MAC.
///
/// This is a pure function that validates the snapshot MAC without any I/O.
pub fn validate_snapshot_mac(
    snapshot: &wa::SyncdSnapshot,
    state: &HashState,
    keys: &ExpandedAppStateKeys,
    collection_name: &str,
) -> Result<(), AppStateError> {
    // A missing snapshot mac is a validation failure, not a skip (matches WA Web
    // and process_snapshot's enforced gate).
    let Some(mac_expected) = snapshot.mac.as_ref() else {
        return Err(AppStateError::SnapshotMACMismatch);
    };
    let computed = state.generate_snapshot_mac(collection_name, &keys.snapshot_mac);
    if computed != *mac_expected {
        return Err(AppStateError::SnapshotMACMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{generate_content_mac, generate_index_mac};
    use crate::keys::expand_app_state_keys;
    use crate::lthash::WAPATCH_INTEGRITY;
    use buffa::Message;
    use wacore_libsignal::crypto::aes_256_cbc_encrypt_into;

    fn create_encrypted_record(
        op: wa::syncd_mutation::SyncdOperation,
        index_mac: &[u8],
        keys: &ExpandedAppStateKeys,
        key_id: &[u8],
        timestamp: i64,
    ) -> wa::SyncdRecord {
        // The `index_mac` arg is the index identity bytes; the stored index blob is
        // their HMAC, so the record stays valid under unconditional index-MAC checks.
        let action_data = wa::SyncActionData {
            index: Some(index_mac.to_vec()),
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(timestamp),
                ..Default::default()
            }),
            ..Default::default()
        };
        let plaintext = action_data.encode_to_vec();

        let iv = vec![0u8; 16];
        let mut ciphertext = Vec::new();
        aes_256_cbc_encrypt_into(&plaintext, &keys.value_encryption, &iv, &mut ciphertext)
            .expect("test data should be valid");

        let mut value_with_iv = iv;
        value_with_iv.extend_from_slice(&ciphertext);
        let value_mac = generate_content_mac(op, &value_with_iv, key_id, &keys.value_mac);
        let mut value_blob = value_with_iv;
        value_blob.extend_from_slice(&value_mac);

        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(generate_index_mac(index_mac, &keys.index)),
            }),
            value: buffa::MessageField::some(wa::SyncdValue {
                blob: Some(value_blob),
            }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.to_vec()),
            }),
        }
    }

    #[test]
    fn test_process_snapshot_basic() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![1; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            1234567890,
        );

        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            records: vec![record],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));

        let mut state = HashState::default();
        let result = process_snapshot(&snapshot, &mut state, get_keys, false, "regular")
            .expect("test data should be valid");

        assert_eq!(result.state.version, 1);
        assert_eq!(result.mutations.len(), 1);
        assert_eq!(result.mutation_macs.len(), 1);
        // Exact MAC bytes (not just counts): catches empty/swapped MACs.
        assert_eq!(
            result.mutation_macs[0].index_mac,
            generate_index_mac(&index_mac, &keys.index)
        );
        assert!(!result.mutation_macs[0].value_mac.is_empty());
        assert_ne!(
            result.mutation_macs[0].index_mac,
            result.mutation_macs[0].value_mac
        );
        assert_eq!(
            result.mutations[0]
                .action_value
                .as_ref()
                .and_then(|v| v.timestamp),
            Some(1234567890)
        );
    }

    #[test]
    fn process_snapshot_rejects_missing_mac_when_validating() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &[1u8; 32],
            &keys,
            &key_id,
            1234567890,
        );
        // Snapshot WITHOUT a `mac` field — must fail validation, not be accepted.
        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            records: vec![record],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };
        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let mut state = HashState::default();
        let err = process_snapshot(&snapshot, &mut state, get_keys, true, "regular")
            .expect_err("missing snapshot mac must fail when validating");
        assert!(matches!(err, AppStateError::SnapshotMACMismatch));
    }

    #[test]
    fn process_snapshot_rejects_missing_key_id_when_validating() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &[1u8; 32],
            &keys,
            &key_id,
            1234567890,
        );
        // mac present but top-level key_id absent — the other branch of the gate.
        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            records: vec![record],
            mac: Some(vec![9u8; 32]),
            key_id: buffa::MessageField::none(),
        };
        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let mut state = HashState::default();
        let err = process_snapshot(&snapshot, &mut state, get_keys, true, "regular")
            .expect_err("missing snapshot key_id must fail when validating");
        assert!(matches!(err, AppStateError::SnapshotMACMismatch));
    }

    /// Deterministic reproduction of the fresh-pairing race that PR #972 works
    /// around. The critical `critical_unblock_low` snapshot (the account's saved
    /// contacts + push name) can arrive before the encrypted app-state key-share
    /// has been processed, when a heavy history sync saturates the stream at
    /// pairing time. The SAME snapshot fails to decode with `KeyNotFound` while
    /// the key is still in flight, and decodes cleanly the instant the key lands
    /// — proving the failure is purely a key-ORDERING race, not a bad snapshot.
    ///
    /// Mirrors the field symptom: `critical_unblock_low v3: N records` failing
    /// with "didn't find app state key" (`AppStateProcessor::get_app_state_key`
    /// -> `backend.get_sync_key` returning `None` -> this `get_keys` closure
    /// returning `KeyNotFound`).
    #[test]
    fn critical_snapshot_fails_key_not_found_until_key_share_lands() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"appstate-sync-key-1".to_vec();

        // A critical_unblock_low-style snapshot carrying a contact record.
        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &[1u8; 32],
            &keys,
            &key_id,
            1_700_000_000,
        );
        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(3) }),
            records: vec![record],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        // Leg 1 — key-share NOT yet processed: the decode fails with KeyNotFound,
        // exactly the "didn't find app state key" the paired companion hits.
        let key_missing = |_: &[u8]| -> Result<Arc<ExpandedAppStateKeys>, AppStateError> {
            Err(AppStateError::KeyNotFound)
        };
        let mut state = HashState::default();
        let err = process_snapshot(
            &snapshot,
            &mut state,
            key_missing,
            false,
            "critical_unblock_low",
        )
        .expect_err("must fail while the key-share is still in flight");
        assert!(
            matches!(err, AppStateError::KeyNotFound),
            "expected KeyNotFound (the 'didn't find app state key' failure), got {err:?}"
        );

        // Leg 2 — key-share lands: the SAME snapshot decodes cleanly. The failure
        // was ordering, not the snapshot — so the fix is about ensuring the key is
        // present (event-driven), never about the snapshot or a longer fixed wait.
        let key_present = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let mut state2 = HashState::default();
        let result = process_snapshot(
            &snapshot,
            &mut state2,
            key_present,
            false,
            "critical_unblock_low",
        )
        .expect("the same snapshot must decode once the key is present");
        assert_eq!(result.state.version, 3);
        assert_eq!(result.mutations.len(), 1, "the contact record must apply");
    }

    #[test]
    fn test_process_patch_basic() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![1; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            1234567890,
        );

        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(record),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(None);

        let mut state = HashState::default();
        let result = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect("test data should be valid");

        assert_eq!(result.state.version, 2);
        assert_eq!(result.mutations.len(), 1);
        assert_eq!(result.added_macs.len(), 1);
        // Exact MAC bytes (not just counts): catches empty/swapped MACs.
        assert_eq!(
            result.added_macs[0].index_mac,
            generate_index_mac(&index_mac, &keys.index)
        );
        assert!(!result.added_macs[0].value_mac.is_empty());
        assert_ne!(
            result.added_macs[0].index_mac,
            result.added_macs[0].value_mac
        );
        assert!(result.removed_index_macs.is_empty());
    }

    #[test]
    fn validate_patch_macs_rejects_snapshot_mismatch_even_with_missing_remove() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
            snapshot_mac: Some(vec![0u8; 32]),
            ..Default::default()
        };
        let state = HashState {
            version: 2,
            hash: [3u8; 128],
            index_value_map: HashMap::new(),
        };

        let err = validate_patch_macs(&patch, &state, &keys, "regular", false, true)
            .expect_err("hasMissingRemove is telemetry, not a snapshotMAC bypass");

        assert!(matches!(err, AppStateError::PatchSnapshotMACMismatch));
    }

    #[test]
    fn validate_patch_macs_rejects_patch_mismatch_even_with_missing_remove() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
            patch_mac: Some(vec![0u8; 32]),
            ..Default::default()
        };
        let state = HashState {
            version: 2,
            hash: [5u8; 128],
            index_value_map: HashMap::new(),
        };

        let err = validate_patch_macs(&patch, &state, &keys, "regular", false, true)
            .expect_err("hasMissingRemove is telemetry, not a patchMAC bypass");

        assert!(matches!(err, AppStateError::PatchMACMismatch));
    }

    #[test]
    fn process_patch_rejects_duplicate_set_index() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![1u8; 32];

        // Two SET mutations colliding on the same index within one patch.
        let mk = |ts| wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::SET),
            record: buffa::MessageField::some(create_encrypted_record(
                wa::syncd_mutation::SyncdOperation::SET,
                &index_mac,
                &keys,
                &key_id,
                ts,
            )),
        };
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            mutations: vec![mk(1), mk(2)],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(None);
        let mut state = HashState::default();
        let err = process_patch(&patch, &mut state, get_keys, get_prev, true, "regular")
            .expect_err("duplicate SET index must be rejected when validating");
        assert!(matches!(err, AppStateError::DuplicateIndexInPatch));
    }

    #[test]
    fn process_patch_allows_same_index_across_set_and_remove() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![2u8; 32];

        // SET and REMOVE share an index legitimately: WA Web tracks the two
        // operations in separate sets, so this is not tampering.
        let set = wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::SET),
            record: buffa::MessageField::some(create_encrypted_record(
                wa::syncd_mutation::SyncdOperation::SET,
                &index_mac,
                &keys,
                &key_id,
                1,
            )),
        };
        let remove = wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::REMOVE),
            record: buffa::MessageField::some(create_encrypted_record(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                &index_mac,
                &keys,
                &key_id,
                2,
            )),
        };
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            mutations: vec![set, remove],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(None);
        let mut state = HashState::default();
        let result = process_patch(&patch, &mut state, get_keys, get_prev, true, "regular");
        assert!(
            result.is_ok(),
            "SET+REMOVE on the same index must be allowed: {result:?}"
        );
    }

    #[test]
    fn process_patch_allows_distinct_indices_when_validating() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();

        let mk = |index: &[u8], ts| wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::SET),
            record: buffa::MessageField::some(create_encrypted_record(
                wa::syncd_mutation::SyncdOperation::SET,
                index,
                &keys,
                &key_id,
                ts,
            )),
        };
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            mutations: vec![mk(&[3u8; 32], 1), mk(&[4u8; 32], 2)],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(None);
        let mut state = HashState::default();
        let result = process_patch(&patch, &mut state, get_keys, get_prev, true, "regular");
        assert!(result.is_ok(), "distinct indices must pass: {result:?}");
    }

    #[test]
    fn test_process_patch_with_overwrite() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![1; 32];

        // Create initial record
        let initial_record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            1000,
        );
        let initial_value_blob = initial_record
            .value
            .blob
            .as_ref()
            .expect("test data should be valid");
        let initial_value_mac = initial_value_blob[initial_value_blob.len() - 32..].to_vec();

        // Process initial snapshot to get starting state
        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            records: vec![initial_record],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let mut snapshot_state = HashState::default();
        let snapshot_result =
            process_snapshot(&snapshot, &mut snapshot_state, get_keys, false, "regular")
                .expect("test data should be valid");

        // Create overwrite record
        let overwrite_record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            2000,
        );

        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(overwrite_record.clone()),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        // process_patch looks up by the stored index MAC (HMAC of the index bytes).
        let stored_index_mac = generate_index_mac(&index_mac, &keys.index);
        let get_prev = |idx: &[u8]| {
            if idx == stored_index_mac.as_slice() {
                Ok(Some(initial_value_mac.clone()))
            } else {
                Ok(None)
            }
        };

        let mut patch_state = snapshot_result.state.clone();
        let result = process_patch(
            &patch,
            &mut patch_state,
            get_keys,
            get_prev,
            false,
            "regular",
        )
        .expect("test data should be valid");

        assert_eq!(result.state.version, 2);
        assert_eq!(result.mutations.len(), 1);
        assert_eq!(
            result.mutations[0]
                .action_value
                .as_ref()
                .and_then(|v| v.timestamp),
            Some(2000)
        );

        // Verify the hash was updated correctly (old value removed, new added)
        let new_value_blob = overwrite_record
            .value
            .into_option()
            .expect("test data should be valid")
            .blob
            .expect("test data should be valid");
        let new_value_mac = new_value_blob[new_value_blob.len() - 32..].to_vec();

        let expected_hash = WAPATCH_INTEGRITY.subtract_then_add(
            &snapshot_result.state.hash,
            &[initial_value_mac],
            &[new_value_mac],
        );

        assert_eq!(result.state.hash.as_slice(), expected_hash.as_slice());
    }

    /// Two SETs of the SAME index in one patch: the second must use the first SET's value
    /// as its "previous value" (in-patch last-write-wins), NOT the DB. Locks the O(1) map
    /// against a regression to a global last-write map (which would remove the wrong value
    /// at position 0) or to no in-patch lookup at all (which would leave both values in the
    /// ltHash). DB returns None here, so a correct run must still cancel the first value.
    #[test]
    fn test_process_patch_in_patch_overwrite_last_write_wins() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![1; 32];

        let first = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            1000,
        );
        let second = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            2000,
        );

        let tail = |rec: &wa::SyncdRecord| {
            let blob = rec.value.as_option().unwrap().blob.as_ref().unwrap();
            blob[blob.len() - 32..].to_vec()
        };
        let first_tail = tail(&first);
        let second_tail = tail(&second);
        assert_ne!(
            first_tail, second_tail,
            "distinct timestamps must yield distinct value MACs"
        );

        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            mutations: vec![
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                    record: buffa::MessageField::some(first),
                },
                wa::SyncdMutation {
                    operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                    record: buffa::MessageField::some(second),
                },
            ],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(None);

        // Fresh state -> had_no_prior_state skips version/MAC checks.
        let mut state = HashState::default();
        let result = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect("two in-patch SETs should process");

        assert_eq!(result.mutations.len(), 2);
        assert_eq!(result.added_macs.len(), 2);

        // Net: first value added then removed by the overwrite -> only the second remains.
        const EMPTY: &[Vec<u8>] = &[];
        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0u8; 128],
            EMPTY,
            std::slice::from_ref(&second_tail),
        );
        assert_eq!(
            result.state.hash.as_slice(),
            expected.as_slice(),
            "in-patch overwrite must leave only the second SET's value in the ltHash"
        );

        // Guard the exact regression: if both values stayed (no in-patch lookup), this differs.
        let both_kept =
            WAPATCH_INTEGRITY.subtract_then_add(&[0u8; 128], EMPTY, &[first_tail, second_tail]);
        assert_ne!(
            result.state.hash.as_slice(),
            both_kept.as_slice(),
            "both SET values must not remain: in-patch overwrite regressed"
        );
    }

    /// SET+REMOVE on the same index in one patch: WA Web index-mode pre-collects the
    /// REMOVEd indices and suppresses the SET's subtraction (the REMOVE owns it, and
    /// it subtracts the STORE value, never the in-patch one). Net must be
    /// base + set_tail - store_prev, which also agrees with the persisted MAC store
    /// (delete removed_index_macs then put added_macs leaves the index present with
    /// the SET's value).
    #[test]
    fn test_process_patch_set_plus_remove_same_index_wa_web_index_mode() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![3; 32];
        let store_prev = vec![9u8; 32];

        let set = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            2000,
        );
        let remove = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::REMOVE,
            &index_mac,
            &keys,
            &key_id,
            1000,
        );

        let tail = |rec: &wa::SyncdRecord| {
            let blob = rec.value.as_option().unwrap().blob.as_ref().unwrap();
            blob[blob.len() - 32..].to_vec()
        };
        let set_tail = tail(&set);

        let build_patch = |mutations: Vec<wa::SyncdMutation>| wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
            mutations,
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };
        let set_mutation = wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::SET),
            record: buffa::MessageField::some(set),
        };
        let remove_mutation = wa::SyncdMutation {
            operation: Some(wa::syncd_mutation::SyncdOperation::REMOVE),
            record: buffa::MessageField::some(remove),
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| Ok(Some(store_prev.clone()));

        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0u8; 128],
            std::slice::from_ref(&store_prev),
            std::slice::from_ref(&set_tail),
        );

        // Both orderings must yield the same hash: the math is per-index, not
        // per-position (WA Web accumulates adds/subtracts in maps).
        for (label, mutations) in [
            (
                "set-then-remove",
                vec![set_mutation.clone(), remove_mutation.clone()],
            ),
            ("remove-then-set", vec![remove_mutation, set_mutation]),
        ] {
            let patch = build_patch(mutations);
            let mut state = HashState::default();
            let result = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
                .unwrap_or_else(|e| panic!("{label} should process: {e:?}"));

            assert_eq!(
                result.state.hash.as_slice(),
                expected.as_slice(),
                "{label}: net must be base + set_tail - store_prev (WA Web index-mode)"
            );

            // The MAC store ends with the index present (delete-then-put), so the
            // hash above is the only self-consistent answer. The wire blob is the
            // HMAC of the index identity, recomputed by decode_record.
            let wire_index_mac = generate_index_mac(&index_mac, &keys.index);
            assert_eq!(result.added_macs.len(), 1, "{label}");
            assert_eq!(result.added_macs[0].index_mac, wire_index_mac, "{label}");
            assert_eq!(result.added_macs[0].value_mac, set_tail, "{label}");
            assert_eq!(
                result.removed_index_macs,
                vec![wire_index_mac.clone()],
                "{label}"
            );
        }
    }

    /// WA Web: validatePatchVersion checks `localVersion !== patchVersion - 1`.
    /// If the patch version is not exactly local_version + 1, it rejects with
    /// "syncd-version-check-error-local-version-{greater|less}-than-expected".
    #[test]
    fn test_patch_version_rollback_rejected() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![99; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            5000,
        );

        // Current state is at version 5
        let mut state = HashState {
            version: 5,
            ..Default::default()
        };

        // Patch claims version 3 (rollback: 3 < 5 + 1)
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(3) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(record),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| -> Result<Option<Vec<u8>>, AppStateError> { Ok(None) };

        let err = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect_err("rollback patch should be rejected");

        assert!(
            matches!(
                err,
                AppStateError::PatchVersionMismatch {
                    expected: 6,
                    got: 3
                }
            ),
            "expected PatchVersionMismatch {{ expected: 6, got: 3 }}, got: {err:?}"
        );
    }

    /// WA Web: version gap (e.g., local=5, patch=8) also triggers
    /// "syncd-version-check-error-local-version-less-than-expected".
    #[test]
    fn test_patch_version_gap_rejected() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![99; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            6000,
        );

        // Current state is at version 5
        let mut state = HashState {
            version: 5,
            ..Default::default()
        };

        // Patch claims version 8 (gap: 8 != 5 + 1)
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(8) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(record),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| -> Result<Option<Vec<u8>>, AppStateError> { Ok(None) };

        let err = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect_err("version gap should be rejected");

        assert!(
            matches!(
                err,
                AppStateError::PatchVersionMismatch {
                    expected: 6,
                    got: 8
                }
            ),
            "expected PatchVersionMismatch {{ expected: 6, got: 8 }}, got: {err:?}"
        );
    }

    /// Consecutive patch (local=5, patch=6) should succeed.
    #[test]
    fn test_patch_version_consecutive_accepted() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![99; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            7000,
        );

        // Current state at version 5
        let mut state = HashState {
            version: 5,
            ..Default::default()
        };

        // Patch version 6 (exactly local + 1)
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(6) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(record),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| -> Result<Option<Vec<u8>>, AppStateError> { Ok(None) };

        let result = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect("consecutive version should be accepted");
        assert_eq!(result.state.version, 6);
    }

    /// When local version is 0 (no prior state), any patch version should be
    /// accepted — we can't validate version continuity without a baseline.
    /// WA Web: "empty lthash" is retryable, but the patch still applies.
    #[test]
    fn test_patch_version_check_skipped_when_no_prior_state() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index_mac = vec![99; 32];

        let record = create_encrypted_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &index_mac,
            &keys,
            &key_id,
            8000,
        );

        // Fresh state — version 0, empty hash
        let mut state = HashState::default();

        // Patch version 42 — should be accepted since no prior state
        let patch = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(42) }),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET),
                record: buffa::MessageField::some(record),
            }],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };

        let get_keys = |_: &[u8]| Ok(Arc::new(keys.clone()));
        let get_prev = |_: &[u8]| -> Result<Option<Vec<u8>>, AppStateError> { Ok(None) };

        let result = process_patch(&patch, &mut state, get_keys, get_prev, false, "regular")
            .expect("no-prior-state should skip version check");
        assert_eq!(result.state.version, 42);
    }
}
