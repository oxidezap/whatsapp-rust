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

    /// Takes the set's allocation with it: the queue fills during an offline
    /// drain and is empty the rest of the process lifetime, so `drain` would
    /// pin the high-water table for nothing.
    pub(crate) fn take_all(&self) -> Vec<Jid> {
        std::mem::take(&mut *self.lock()).into_iter().collect()
    }

    /// Release a user whose online refresh has finished, so the next unknown
    /// device from them triggers a refresh again instead of hitting the dedup
    /// for the rest of the connection. A no-op for an entry an offline drain
    /// already took.
    pub(crate) fn remove(&self, jid: &Jid) {
        let mut pending = self.lock();
        pending.remove(jid);
        crate::client::release_after_burst(&mut pending);
    }

    /// Users queued for the next `doPendingDeviceSync`, plus the users with an
    /// online refresh in flight — [`Self::add`] is the dedup for both. Offline
    /// entries leave with [`Self::take_all`]; an online entry leaves when its
    /// refresh finishes ([`Self::remove`]), so outside a drain this holds only
    /// what is in flight.
    ///
    /// The set has no capacity cap, and a cap would be the wrong fix: dropping a
    /// user silently skips a device refresh and leaves the next send to them
    /// addressed to a stale device list. The bound is the drain, not a number —
    /// which is why the count belongs in `memory_report()`.
    pub(crate) fn len(&self) -> usize {
        self.lock().len()
    }

    pub(crate) fn clear(&self) {
        self.lock().clear();
    }
}
