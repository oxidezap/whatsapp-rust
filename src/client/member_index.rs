//! The membership index behind the device-list memos.

use std::collections::hash_map::RandomState;
use std::hash::BuildHasher;
use std::sync::OnceLock;

/// The set of user identifiers a device-list memo was built over, stored as
/// sorted 64-bit fingerprints rather than the identifiers themselves.
///
/// The only question ever asked of it is the scoped-invalidation probe "did
/// this topology write touch a member of that memo?", and its two answers are
/// not symmetric: a spurious `true` recomputes a memo that did not need it, a
/// spurious `false` serves a device list that is missing a device. A hash
/// collision can only produce the former, so the identifiers themselves earn
/// nothing here — which is why a memo over a 1024-member group can index its
/// membership in 8 bytes per identifier instead of a `HashSet` of strings
/// whose buckets alone cost an order of magnitude more.
///
/// Construction must therefore stay push-only: every identifier that reaches
/// the builder is indexed, and no control flow may depend on two fingerprints
/// being equal (see [`crate::client::device_registry`]'s DM member walk, which
/// dedups on the strings for exactly that reason).
pub(crate) struct MemberIndex {
    /// Sorted and deduplicated, so a probe is a binary search. Boxed rather
    /// than a `Vec` because it never changes after construction and the memo
    /// holds it for the life of the group.
    fingerprints: Box<[u64]>,
}

/// One `RandomState` for the whole process. The fingerprints of two different
/// indexes are never compared, so nothing requires a shared seed — but sharing
/// one keeps `MemberIndex` a single pointer wide, and drawing it randomly once
/// per process keeps a remote peer from choosing identifiers that collide.
fn fingerprint_hasher() -> &'static RandomState {
    static HASHER: OnceLock<RandomState> = OnceLock::new();
    HASHER.get_or_init(RandomState::new)
}

pub(crate) fn fingerprint(user: &str) -> u64 {
    fingerprint_hasher().hash_one(user)
}

impl MemberIndex {
    pub(crate) fn builder(capacity: usize) -> MemberIndexBuilder {
        MemberIndexBuilder {
            fingerprints: Vec::with_capacity(capacity),
        }
    }

    /// Index one set of identifiers in a single call.
    pub(crate) fn from_users<'a>(users: impl IntoIterator<Item = &'a str>) -> Self {
        let users = users.into_iter();
        let mut builder = Self::builder(users.size_hint().0);
        for user in users {
            builder.insert(user);
        }
        builder.build()
    }

    /// Whether `user` may be a member. `true` is "possibly", `false` is
    /// definite — see the type's note on which direction is safe.
    #[cfg(test)]
    pub(crate) fn contains(&self, user: &str) -> bool {
        self.fingerprints.binary_search(&fingerprint(user)).is_ok()
    }

    /// Whether the two indexes share a fingerprint, with the same asymmetry
    /// as [`contains`](Self::contains): `true` is "possibly". A merge walk
    /// over the two sorted slices, so a topology change that touched hundreds
    /// of users is checked against a memo in one pass.
    pub(crate) fn intersects(&self, other: &MemberIndex) -> bool {
        let (mut left, mut right) = (self.fingerprints.iter(), other.fingerprints.iter());
        let (mut l, mut r) = (left.next(), right.next());
        while let (Some(a), Some(b)) = (l, r) {
            match a.cmp(b) {
                std::cmp::Ordering::Equal => return true,
                std::cmp::Ordering::Less => l = left.next(),
                std::cmp::Ordering::Greater => r = right.next(),
            }
        }
        false
    }

    pub(crate) fn len(&self) -> usize {
        self.fingerprints.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.fingerprints.is_empty()
    }
}

impl wacore::stats::HeapSize for MemberIndex {
    fn heap_bytes(&self) -> usize {
        self.fingerprints.len() * size_of::<u64>()
    }
}

impl std::fmt::Debug for MemberIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemberIndex")
            .field("members", &self.fingerprints.len())
            .finish()
    }
}

pub(crate) struct MemberIndexBuilder {
    fingerprints: Vec<u64>,
}

impl MemberIndexBuilder {
    pub(crate) fn insert(&mut self, user: &str) {
        self.fingerprints.push(fingerprint(user));
    }

    pub(crate) fn build(mut self) -> MemberIndex {
        sort_fingerprints(&mut self.fingerprints);
        dedup_sorted(&mut self.fingerprints);
        MemberIndex {
            // Shrinks the over-reserved build buffer to what the deduplicated
            // membership actually needs, which for a group whose members were
            // each pushed under several aliases is most of it.
            fingerprints: self.fingerprints.into_boxed_slice(),
        }
    }
}

/// In-place heapsort over the fingerprints.
///
/// Deliberately not `sort_unstable`: a fresh instantiation of the standard
/// library's pdqsort cost 15.6 KiB of `.text` when #1353 added one, which is a
/// bad trade for a sort that runs once per memo recompute. This is a few
/// hundred bytes, monomorphic on `u64`, and still O(n log n).
fn sort_fingerprints(values: &mut [u64]) {
    let len = values.len();
    if len < 2 {
        return;
    }
    for root in (0..len / 2).rev() {
        sift_down(values, root, len);
    }
    for end in (1..len).rev() {
        values.swap(0, end);
        sift_down(values, 0, end);
    }
}

fn sift_down(values: &mut [u64], mut root: usize, end: usize) {
    loop {
        let mut child = 2 * root + 1;
        if child >= end {
            return;
        }
        if child + 1 < end && values[child] < values[child + 1] {
            child += 1;
        }
        if values[root] >= values[child] {
            return;
        }
        values.swap(root, child);
        root = child;
    }
}

/// Drop runs of equal values from a sorted slice, monomorphically — same
/// reason as the sort above, `Vec::dedup` would instantiate `dedup_by`.
fn dedup_sorted(values: &mut Vec<u64>) {
    if values.is_empty() {
        return;
    }
    let mut write = 1;
    for read in 1..values.len() {
        if values[read] != values[write - 1] {
            values[write] = values[read];
            write += 1;
        }
    }
    values.truncate(write);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_every_inserted_identifier() {
        let mut builder = MemberIndex::builder(4);
        for user in ["100000000000001", "5511999990001", "100000000000002"] {
            builder.insert(user);
        }
        let index = builder.build();

        for user in ["100000000000001", "5511999990001", "100000000000002"] {
            assert!(index.contains(user), "{user} was inserted");
        }
        // A `false` is the definite answer, so an unrelated identifier must
        // produce one (barring a collision, which this fixture does not hit).
        assert!(!index.contains("100000000000009"));
    }

    #[test]
    fn duplicate_identifiers_collapse() {
        let mut builder = MemberIndex::builder(8);
        for _ in 0..4 {
            builder.insert("100000000000001");
        }
        builder.insert("5511999990001");
        let index = builder.build();

        assert_eq!(index.len(), 2, "aliases pushed repeatedly index once");
        assert!(index.contains("100000000000001"));
        assert!(index.contains("5511999990001"));
    }

    #[test]
    fn empty_index_answers_no() {
        let index = MemberIndex::builder(0).build();
        assert_eq!(index.len(), 0);
        assert!(!index.contains("100000000000001"));
    }

    /// The probe is a binary search, so the sort is load-bearing: an unsorted
    /// slice would answer `false` for a member and serve a stale device list.
    /// Exercised across the sizes that reach both heapsort phases.
    #[test]
    fn every_member_is_found_at_any_size() {
        for size in [1usize, 2, 3, 7, 8, 9, 64, 1000] {
            let users: Vec<String> = (0..size).map(|i| format!("1000000{i:08}")).collect();
            let mut builder = MemberIndex::builder(size);
            for user in &users {
                builder.insert(user);
            }
            let index = builder.build();
            assert_eq!(index.len(), size, "size {size}");
            for user in &users {
                assert!(index.contains(user), "size {size} lost {user}");
            }
        }
    }

    #[test]
    fn sorts_and_dedups_adversarial_orders() {
        // Reversed, all-equal and duplicate-heavy inputs, which are where a
        // hand-written heapsort or dedup goes wrong.
        for mut values in [
            vec![9u64, 8, 7, 6, 5, 4, 3, 2, 1],
            vec![5u64; 9],
            vec![3u64, 1, 3, 1, 2, 2, 3],
            vec![u64::MAX, 0, u64::MAX, 0],
        ] {
            let mut expected = values.clone();
            expected.sort_unstable();
            expected.dedup();

            sort_fingerprints(&mut values);
            dedup_sorted(&mut values);
            assert_eq!(values, expected);
        }
    }
}
