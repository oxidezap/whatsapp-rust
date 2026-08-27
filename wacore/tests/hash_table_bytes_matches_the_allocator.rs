//! `wacore::stats::hash_table_bytes` claims to report what a hash table really
//! allocates. That claim is only worth having if it is checked against the
//! allocator rather than against another formula, so this test installs a
//! counting `GlobalAlloc` and compares.
//!
//! Its own integration test, not a unit test: a global allocator applies to the
//! whole binary, and the library's other tests must not run under one.
//!
//! What a table holds is measured by **dropping** it and reading how much the
//! counter falls, not by bracketing its construction. The counter is
//! process-wide, so a window around a construction also catches whatever else
//! the process allocated in it — a lazily seeded global, the harness, a
//! neighbouring test — and under `--all-features` that is not nothing. A drop
//! frees exactly the table's own allocation and nothing else, so unrelated
//! allocations in the window cancel instead of inflating the figure.
//!
//! Still one test function, not several: the counter is process-wide and the
//! harness runs functions on separate threads.

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

/// Bytes the allocator reclaims when `map` is dropped — that is, exactly what
/// its table held.
///
/// `HashMap<u64, [u8; 16]>` owns a single allocation and its entries own none,
/// so the fall in the counter across the drop is the table and nothing else.
fn bytes_released_by_dropping(map: HashMap<u64, [u8; 16]>) -> usize {
    let before = LIVE.load(Ordering::Relaxed);
    drop(map);
    before - LIVE.load(Ordering::Relaxed)
}

#[test]
fn reported_bytes_match_what_the_table_allocates() {
    // One throwaway table first, so `RandomState`'s thread-local seeding is
    // done before anything is measured.
    drop(HashMap::<u64, [u8; 16]>::from_iter([(0, [0u8; 16])]));

    // Sizes either side of hashbrown's small-table cases (4 and 8 buckets) and
    // across several resize boundaries.
    for entries in [1usize, 3, 4, 7, 8, 9, 100, 1000, 1024, 1792, 1793] {
        // `with_capacity`, so the table is one allocation of the final size
        // rather than a series of doublings.
        let mut map: HashMap<u64, [u8; 16]> = HashMap::with_capacity(entries);
        for i in 0..entries {
            map.insert(i as u64, [0u8; 16]);
        }
        let capacity = map.capacity();
        let allocated = bytes_released_by_dropping(map);

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

    // A table that has seen removals reports a `capacity()` below the
    // canonical one for its buckets: hashbrown leaves tombstones that consume
    // growth slots without shrinking the array. The figure is a floor there
    // rather than exact — the safe direction, and the property worth pinning
    // is that it never reads *over* what the table holds.
    let mut map: HashMap<u64, [u8; 16]> = HashMap::with_capacity(1000);
    for i in 0..1000u64 {
        map.insert(i, [0u8; 16]);
    }
    for i in 0..900u64 {
        map.remove(&i);
    }
    let reported = hash_table_bytes(map.capacity(), size_of::<Entry>());
    let allocated = bytes_released_by_dropping(map);

    assert!(
        reported <= allocated,
        "after removals the report must stay a floor: {reported} B reported, \
         {allocated} B held"
    );
}
