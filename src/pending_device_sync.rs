//! Batches unknown-device users during offline sync for deferred usync.
//! WA Web: `OfflinePendingDeviceCache` + `doPendingDeviceSync()`.

use std::collections::HashSet;
use wacore_binary::Jid;

pub(crate) struct PendingDeviceSync {
    /// Sync lock: no critical section here does anything but a set operation.
    pending: std::sync::Mutex<HashSet<Jid>>,
}

impl PendingDeviceSync {
    pub(crate) fn new() -> Self {
        Self {
            pending: std::sync::Mutex::new(HashSet::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<Jid>> {
        self.pending.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Insert a user. Returns `true` if newly inserted, `false` if already present.
    ///
    /// Takes `&Jid` and clones only on a real insert, so the dedup path (the common
    /// case during a retry storm) does no allocation.
    pub(crate) fn add(&self, jid: &Jid) -> bool {
        let mut pending = self.lock();
        if pending.contains(jid) {
            false
        } else {
            pending.insert(jid.clone());
            true
        }
    }

    pub(crate) fn take_all(&self) -> Vec<Jid> {
        self.lock().drain().collect()
    }

    pub(crate) fn clear(&self) {
        self.lock().clear();
    }
}
