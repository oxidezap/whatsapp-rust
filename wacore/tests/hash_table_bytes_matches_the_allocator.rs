//! `wacore::stats::hash_table_bytes` claims to report what a hash table really
//! allocates. That claim is only worth having if it is checked against the
//! allocator rather than against another formula, so this test installs a
//! counting `GlobalAlloc` and compares.
//!
//! Its own integration test, not a unit test: a global allocator applies to the
//! whole binary, and the library's other tests must not run under one.
//!
//! One test function, not two: the counter is global, so a second test running
//! on another harness thread would have its allocations land in this one's
//! measurement.

use std::alloc::{GlobalAlloc, Layout, System};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use wacore::stats::hash_table_bytes;

static LIVE: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: every method forwards to `System` unchanged; the counters are the
// only added effect and touch no allocation state.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        LIVE.fetch_add(new_size, Ordering::Relaxed);
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// A 24-byte entry, the shape of the `(CompactString, u64)`-ish pairs these
/// reports actually hold.
type Entry = (u64, [u8; 16]);

#[test]
fn reported_bytes_match_what_the_table_allocates() {
    // One throwaway table first: `RandomState` seeds a thread-local on first
    // use, and that allocation would otherwise land on the first measurement.
    drop(HashMap::<u64, [u8; 16]>::from_iter([(0, [0u8; 16])]));

    // Sizes either side of hashbrown's small-table cases (4 and 8 buckets) and
    // across several resize boundaries.
    for entries in [1usize, 3, 4, 7, 8, 9, 100, 1000, 1024, 1792, 1793] {
        let before = LIVE.load(Ordering::Relaxed);
        // `with_capacity`, so the table is one allocation of the final size
        // rather than a series of doublings whose freed intermediates the
        // counter would have to net out.
        let mut map: HashMap<u64, [u8; 16]> = HashMap::with_capacity(entries);
        for i in 0..entries {
            map.insert(i as u64, [0u8; 16]);
        }
        let allocated = LIVE.load(Ordering::Relaxed) - before;
        let capacity = map.capacity();
        drop(map);

        let reported = hash_table_bytes(capacity, size_of::<Entry>());

        // The control array carries a fixed `Group::WIDTH` tail (16 bytes on
        // x86-64) that does not scale with the map, so the report is allowed
        // to sit just under the true figure — but never over it, and never by
        // a margin that grows with the entry count.
        assert!(
            reported <= allocated,
            "{entries} entries: reported {reported} B over the real {allocated} B"
        );
        assert!(
            allocated - reported <= 64,
            "{entries} entries: reported {reported} B, real {allocated} B \u{2014} the \
             gap must stay a fixed tail, not a per-entry error"
        );

        // And the bug this replaced: multiplying `capacity()` by the entry
        // size misses both the free eighth hashbrown keeps and the control
        // bytes, so it lands under the truth by more than that fixed tail.
        let previous_formula = capacity * size_of::<Entry>();
        assert!(
            previous_formula < reported,
            "{entries} entries: the corrected figure ({reported} B) must exceed \
             the old one ({previous_formula} B)"
        );
    }
}
