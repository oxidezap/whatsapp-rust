//! Batches unknown-device users during offline sync for deferred usync.
//! WA Web: `OfflinePendingDeviceCache` + `doPendingDeviceSync()`.

use std::collections::HashSet;
use wacore_binary::Jid;

pub(crate) struct PendingDeviceSync {
    pending: async_lock::Mutex<HashSet<Jid>>,
}

impl PendingDeviceSync {
    pub(crate) fn new() -> Self {
        Self {
            pending: async_lock::Mutex::new(HashSet::new()),
        }
    }

    /// Insert a user. Returns `true` if newly inserted, `false` if already present.
    ///
    /// Takes `&Jid` and clones only on a real insert, so the dedup path (the common
    /// case during a retry storm) does no allocation.
    pub(crate) async fn add(&self, jid: &Jid) -> bool {
        let mut pending = self.pending.lock().await;
        if pending.contains(jid) {
            false
        } else {
            pending.insert(jid.clone());
            true
        }
    }

    pub(crate) async fn take_all(&self) -> Vec<Jid> {
        self.pending.lock().await.drain().collect()
    }

    pub(crate) async fn clear(&self) {
        self.pending.lock().await.clear();
    }
}
