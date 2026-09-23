//! Short-lived offer liveness across concurrent signaling stanzas, including
//! builds without the VoIP call registry.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Tracks only offers still inside the handler; completed offers retain no entry.
#[derive(Default)]
pub(crate) struct PendingOffers {
    entries: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl PendingOffers {
    /// Register an offer before any receipt or identity-learning await.
    pub(crate) fn register(&self, call_id: &str) -> PendingOffer<'_> {
        let alive = Arc::new(AtomicBool::new(true));
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(previous) = entries.insert(call_id.to_owned(), alive.clone()) {
            previous.store(false, Ordering::Release);
        }
        PendingOffer {
            tracker: self,
            call_id: call_id.to_owned(),
            alive,
        }
    }

    /// Cancel an in-flight offer when a terminate for that call is processed.
    pub(crate) fn terminate(&self, call_id: &str) {
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(alive) = entries.remove(call_id) {
            alive.store(false, Ordering::Release);
        }
    }

    /// Number of in-flight offers retained by this client.
    pub(crate) fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

/// Removing only our own entry prevents an older offer from erasing a retry.
pub(crate) struct PendingOffer<'a> {
    tracker: &'a PendingOffers,
    call_id: String,
    alive: Arc<AtomicBool>,
}

impl PendingOffer<'_> {
    pub(crate) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

impl Drop for PendingOffer<'_> {
    fn drop(&mut self) {
        let mut entries = self
            .tracker
            .entries
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if entries
            .get(&self.call_id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.alive))
        {
            entries.remove(&self.call_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacing_an_offer_does_not_let_the_old_guard_remove_the_new_one() {
        let pending = PendingOffers::default();
        let old = pending.register("CALL-TEST");
        let current = pending.register("CALL-TEST");
        assert!(!old.is_alive());
        drop(old);
        assert_eq!(pending.len(), 1);
        assert!(current.is_alive());
        pending.terminate("CALL-TEST");
        assert!(!current.is_alive());
        drop(current);
        assert_eq!(pending.len(), 0);
    }
}
