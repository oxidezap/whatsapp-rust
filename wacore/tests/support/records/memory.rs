// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use crate::records::record::{
    Fence, FenceLost, Lease, LeaseStore, Namespace, RecordExists, RecordStore, WriteOp,
};
use async_trait::async_trait;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use wacore::store::error::{Result, StoreError};

type Records = BTreeMap<(Arc<str>, Namespace, String), Vec<u8>>;

#[derive(Default)]
struct State {
    records: Records,
    leases: HashMap<Arc<str>, LeaseRow>,
}

#[derive(Clone)]
struct LeaseRow {
    holder: Arc<str>,
    generation: u64,
    expires_at_ms: u64,
}

/// In-process [`RecordStore`] and [`LeaseStore`]. The reference behaviour for
/// the conformance suite, and the fallback where no database is configured.
#[derive(Default)]
pub struct MemoryRecordStore {
    state: Mutex<State>,
}

impl MemoryRecordStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| StoreError::Validation("memory record store lock poisoned".into()))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RecordStore for MemoryRecordStore {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        let state = self.lock()?;
        Ok(state
            .records
            .get(&(Arc::from(scope), ns, key.to_owned()))
            .cloned())
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        let state = self.lock()?;
        let scope: Arc<str> = Arc::from(scope);
        let start = (scope.clone(), ns, prefix.to_owned());
        let keys = state
            .records
            .range(start..)
            .map(|((s, n, k), _)| (s, n, k))
            .take_while(|(s, n, k)| **s == scope && **n == ns && k.starts_with(prefix))
            .map(|(_, _, k)| k.clone());
        Ok(match limit {
            Some(limit) => keys.take(limit).collect(),
            None => keys.collect(),
        })
    }

    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()> {
        let mut state = self.lock()?;
        let current = state.leases.get(&fence.scope).map(|row| row.generation);
        if current != Some(fence.generation) {
            return Err(FenceLost {
                scope: fence.scope.clone(),
                generation: fence.generation,
            }
            .into_store_error());
        }
        // Validation happens before any mutation, so the batch stays atomic.
        // `present` tracks the batch's running effect on key presence, so an
        // Insert sees earlier Put/Insert/Delete ops of the same batch.
        let mut present: HashMap<(Namespace, &str), bool> = HashMap::new();
        for op in ops {
            match op {
                WriteOp::Insert { ns, key, .. } => {
                    let exists = match present.get(&(*ns, key.as_str())) {
                        Some(&known) => known,
                        None => {
                            state
                                .records
                                .contains_key(&(fence.scope.clone(), *ns, key.clone()))
                        }
                    };
                    if exists {
                        return Err(RecordExists {
                            ns: *ns,
                            key: key.clone(),
                        }
                        .into_store_error());
                    }
                    present.insert((*ns, key.as_str()), true);
                }
                WriteOp::Put { ns, key, .. } => {
                    present.insert((*ns, key.as_str()), true);
                }
                WriteOp::Delete { ns, key } => {
                    present.insert((*ns, key.as_str()), false);
                }
                // Update never changes presence.
                WriteOp::Update { .. } => {}
            }
        }
        for op in ops {
            match op {
                WriteOp::Put { ns, key, value } | WriteOp::Insert { ns, key, value } => {
                    state
                        .records
                        .insert((fence.scope.clone(), *ns, key.clone()), value.clone());
                }
                WriteOp::Update { ns, key, value } => {
                    if let Some(slot) =
                        state
                            .records
                            .get_mut(&(fence.scope.clone(), *ns, key.clone()))
                    {
                        slot.clone_from(value);
                    }
                }
                WriteOp::Delete { ns, key } => {
                    state
                        .records
                        .remove(&(fence.scope.clone(), *ns, key.clone()));
                }
            }
        }
        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl LeaseStore for MemoryRecordStore {
    async fn acquire(
        &self,
        scope: &str,
        holder: &str,
        now_ms: u64,
        ttl_ms: u64,
    ) -> Result<Option<Lease>> {
        // An empty holder is how a released lease is marked.
        if holder.is_empty() {
            return Err(StoreError::Validation(
                "lease holder must not be empty".into(),
            ));
        }
        let mut state = self.lock()?;
        let scope: Arc<str> = Arc::from(scope);
        let expires_at_ms = now_ms.saturating_add(ttl_ms);
        let generation = match state.leases.get(&scope) {
            Some(row) if &*row.holder == holder => row.generation,
            Some(row) if row.expires_at_ms > now_ms => return Ok(None),
            Some(row) => row.generation + 1,
            None => 1,
        };
        let holder: Arc<str> = Arc::from(holder);
        state.leases.insert(
            scope.clone(),
            LeaseRow {
                holder: holder.clone(),
                generation,
                expires_at_ms,
            },
        );
        Ok(Some(Lease {
            fence: Fence { scope, generation },
            holder,
            expires_at_ms,
        }))
    }

    async fn renew(&self, lease: &Lease, now_ms: u64, ttl_ms: u64) -> Result<Option<Lease>> {
        let mut state = self.lock()?;
        let Some(row) = state.leases.get_mut(&lease.fence.scope) else {
            return Ok(None);
        };
        if row.generation != lease.fence.generation || row.holder != lease.holder {
            return Ok(None);
        }
        row.expires_at_ms = now_ms.saturating_add(ttl_ms);
        Ok(Some(Lease {
            expires_at_ms: row.expires_at_ms,
            ..lease.clone()
        }))
    }

    async fn release(&self, lease: &Lease) -> Result<()> {
        let mut state = self.lock()?;
        if let Some(row) = state.leases.get_mut(&lease.fence.scope)
            && row.generation == lease.fence.generation
            && row.holder == lease.holder
        {
            // Expire it but keep the generation: the next holder must still
            // advance past it, or a delayed write from this lease could land.
            row.expires_at_ms = 0;
            row.holder = Arc::from("");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_the_conformance_suite() {
        futures::executor::block_on(crate::records::conformance::run_all(
            &MemoryRecordStore::new(),
        ));
    }
}
