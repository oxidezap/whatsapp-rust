//! Request registration ownership without channels, timers or an executor.

use std::collections::HashMap;
use std::num::NonZeroU64;

struct Entry<T> {
    generation: NonZeroU64,
    value: T,
}

pub struct PendingRequests<T> {
    entries: HashMap<String, Entry<T>>,
    last_generation: u64,
}

impl<T> Default for PendingRequests<T> {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            last_generation: 0,
        }
    }
}

impl<T> PendingRequests<T> {
    fn next_generation(&mut self) -> NonZeroU64 {
        self.last_generation = self.last_generation.wrapping_add(1).max(1);
        NonZeroU64::new(self.last_generation).expect("generation is nonzero")
    }

    pub fn try_insert(&mut self, id: String, value: T) -> Option<NonZeroU64> {
        if self.entries.contains_key(&id) {
            return None;
        }
        let generation = self.next_generation();
        self.entries.insert(id, Entry { generation, value });
        Some(generation)
    }

    pub fn insert(&mut self, id: String, value: T) -> Option<T> {
        let generation = self.next_generation();
        self.entries
            .insert(id, Entry { generation, value })
            .map(|e| e.value)
    }

    pub fn get(&self, id: &str) -> Option<&T> {
        self.entries.get(id).map(|e| &e.value)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut T> {
        self.entries.get_mut(id).map(|entry| &mut entry.value)
    }

    pub fn remove(&mut self, id: &str) -> Option<T> {
        self.entries.remove(id).map(|e| e.value)
    }

    pub fn remove_guarded(&mut self, id: &str, generation: NonZeroU64) -> Option<T> {
        if self
            .entries
            .get(id)
            .is_some_and(|entry| entry.generation == generation)
        {
            self.remove(id)
        } else {
            None
        }
    }

    pub fn retain(&mut self, mut keep: impl FnMut(&str, &T) -> bool) {
        self.entries.retain(|id, entry| keep(id, &entry.value));
    }

    // Old guards can outlive a disconnect. Clearing must retain the sequence.
    pub fn clear(&mut self) {
        self.entries = HashMap::new();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_cancellation_cannot_remove_a_reused_id() {
        let mut pending = PendingRequests::default();
        let old = pending.try_insert("id".into(), 10).unwrap();
        assert!(pending.try_insert("id".into(), 11).is_none());
        assert_eq!(pending.remove("id"), Some(10));
        let new = pending.try_insert("id".into(), 12).unwrap();
        assert_eq!(pending.remove_guarded("id", old), None);
        assert_eq!(pending.remove_guarded("id", new), Some(12));
    }

    #[test]
    fn disconnect_preserves_registration_identity() {
        let mut pending = PendingRequests::default();
        let old = pending.try_insert("id".into(), 10).unwrap();
        pending.clear();
        pending.try_insert("id".into(), 20).unwrap();
        assert_eq!(pending.remove_guarded("id", old), None);
        assert_eq!(pending.get("id"), Some(&20));
    }

    #[test]
    fn generation_wrap_skips_zero() {
        let mut pending = PendingRequests {
            last_generation: u64::MAX,
            ..Default::default()
        };
        assert_eq!(pending.try_insert("id".into(), ()), NonZeroU64::new(1));
    }

    #[test]
    fn replacement_and_expiry_return_the_right_values() {
        let mut pending = PendingRequests::default();
        assert_eq!(pending.insert("a".into(), 1), None);
        assert_eq!(pending.insert("a".into(), 2), Some(1));
        pending.insert("b".into(), 3);
        pending.retain(|_, value| *value == 3);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending.get("b"), Some(&3));
    }
}
