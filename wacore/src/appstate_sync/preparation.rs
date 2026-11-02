//! Owned app-state preparation without storage access or task scheduling.

use super::IndexMac;
use crate::appstate::hash::HashState;
use crate::appstate::keys::ExpandedAppStateKeys;
use crate::appstate::processor::{PatchProcessingResult, ProcessedSnapshot};
use crate::appstate::{AppStateError, Mutation, process_patch, process_snapshot};
use std::{collections::HashMap, sync::Arc};
use waproto::whatsapp as wa;

fn lookup_app_state_key(
    keys_map: &HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>,
    key_id: &[u8],
) -> Result<Arc<ExpandedAppStateKeys>, AppStateError> {
    // Return the Arc (refcount bump) instead of deep-cloning the 160-byte
    // ExpandedAppStateKeys; the callback runs once per mutation (up to ~1000/patch).
    keys_map
        .get(key_id)
        .map(Arc::clone)
        .ok_or(AppStateError::KeyNotFound)
}

pub struct PreparedPatch {
    patch: wa::SyncdPatch,
    result: PatchProcessingResult,
}

impl PreparedPatch {
    pub fn mark_bootstrapped(&mut self) {
        self.result.state.bootstrapped = true;
    }

    pub fn result(&self) -> &PatchProcessingResult {
        &self.result
    }
    pub fn into_parts(self) -> (wa::SyncdPatch, PatchProcessingResult) {
        (self.patch, self.result)
    }
}

pub fn prepare_patch(
    patch: wa::SyncdPatch,
    mut state: HashState,
    keys: &HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>,
    previous_macs: &HashMap<IndexMac, Vec<u8>>,
    validate_macs: bool,
    collection: &str,
    final_page: bool,
) -> Result<PreparedPatch, AppStateError> {
    let mut result = process_patch(
        &patch,
        &mut state,
        |id| lookup_app_state_key(keys, id),
        |index| {
            Ok(<&IndexMac>::try_from(index)
                .ok()
                .and_then(|k| previous_macs.get(k))
                .cloned())
        },
        validate_macs,
        collection,
    )?;
    result.state.bootstrapped |= final_page;
    Ok(PreparedPatch { patch, result })
}

pub struct PreparedSnapshot {
    snapshot: wa::SyncdSnapshot,
    state: HashState,
    result: ProcessedSnapshot,
}

impl PreparedSnapshot {
    pub fn into_parts(self) -> (wa::SyncdSnapshot, HashState, ProcessedSnapshot) {
        (self.snapshot, self.state, self.result)
    }
}

pub fn prepare_snapshot(
    snapshot: wa::SyncdSnapshot,
    keys: &HashMap<Vec<u8>, Arc<ExpandedAppStateKeys>>,
    validate_macs: bool,
    collection: &str,
) -> Result<PreparedSnapshot, AppStateError> {
    let mut state = HashState::default();
    let result = process_snapshot(
        &snapshot,
        &mut state,
        |id| lookup_app_state_key(keys, id),
        validate_macs,
        collection,
    )?;
    Ok(PreparedSnapshot {
        snapshot,
        state,
        result,
    })
}

// Publication is a separate consuming step, called only after all required
// mutation effects and the backend cursor/MAC commit have succeeded.
pub(super) fn publish_patch(
    prepared: PreparedPatch,
    state: &mut HashState,
    committed_state: &mut HashState,
    mutations: &mut Vec<Mutation>,
) -> wa::SyncdPatch {
    let (patch, result) = prepared.into_parts();
    *state = result.state;
    *committed_state = state.clone();
    mutations.extend(result.mutations);
    patch
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_version_does_not_prepare_a_patch_or_change_the_held_cursor() {
        let held = HashState {
            version: 7,
            ..HashState::default()
        };
        let patch = wa::SyncdPatch::default();
        assert!(
            prepare_patch(
                patch,
                held.clone(),
                &HashMap::new(),
                &HashMap::new(),
                true,
                "regular",
                true
            )
            .is_err()
        );
        assert_eq!(held.version, 7);
        assert!(!held.bootstrapped);
    }

    #[test]
    fn publication_moves_only_confirmed_results() {
        let mut state = HashState::default();
        let mut committed = state.clone();
        let mut mutations = Vec::new();
        let mut next = state.clone();
        next.version = 4;
        next.bootstrapped = true;
        let prepared = PreparedPatch {
            patch: wa::SyncdPatch::default(),
            result: PatchProcessingResult {
                state: next,
                mutations: Vec::new(),
                added_macs: Vec::new(),
                removed_index_macs: Vec::new(),
            },
        };
        assert_eq!(state.version, 0);
        publish_patch(prepared, &mut state, &mut committed, &mut mutations);
        assert_eq!(state.version, 4);
        assert_eq!(committed.version, 4);
        assert!(committed.bootstrapped);
    }
}
