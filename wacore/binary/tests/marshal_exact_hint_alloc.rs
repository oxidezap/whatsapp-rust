//! Locks the exact-marshal allocation property: for a typical stanza the
//! two-pass `marshal_exact` must allocate ONLY the output buffer. The string
//! hint cache built during the size pass keeps its entries inline (SmallVec);
//! a heap-backed cache (the regression this guards) paid ~1.3 KiB per
//! outgoing stanza on the send hot path.
//!
//! Single test fn on purpose: the counting allocator is process-global, so a
//! concurrently running sibling test would bleed its allocations into the
//! measurement (seen once in CI).

// Host-only allocation-count harness; std's 64-bit atomic is fine (never built
// for embedded targets).
#![allow(clippy::disallowed_types)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::{marshal, marshal_exact};

struct CountingAlloc;
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[test]
fn marshal_exact_allocates_only_the_output_buffer() {
    // A live delivery-receipt shape: the strings comfortably fit the hint
    // cache's inline capacity, so the size pass must not touch the heap.
    let node = NodeBuilder::new("receipt")
        .attr("to", "5511999990000@s.whatsapp.net")
        .attr("id", "3EB0A9252A8F12B7E2")
        .attr("type", "read")
        .attr("t", "1760000000")
        .build();

    // Min-delta over many windows: the counter is process-global and harness
    // threads bleed sporadic allocations, but if the plan pass is truly
    // alloc-free at least one window lands on exactly the output buffer.
    let mut min_delta = u64::MAX;
    for _ in 0..100 {
        let before = ALLOCS.load(Ordering::Relaxed);
        let payload = marshal_exact(&node).expect("marshal_exact");
        let after = ALLOCS.load(Ordering::Relaxed);
        assert!(!payload.is_empty());
        min_delta = min_delta.min(after - before);
    }
    assert_eq!(
        min_delta, 1,
        "marshal_exact of a small stanza must allocate only the output buffer"
    );

    // Spill correctness: a stanza with more distinct strings than the inline
    // capacity still encodes byte-identically to the one-pass path.
    let mut builder = NodeBuilder::new("iq");
    for i in 0..40 {
        let key: &'static str = Box::leak(format!("k{i:02}").into_boxed_str());
        builder = builder.attr(key, format!("value-{i:02}"));
    }
    let big = builder.build();
    assert_eq!(
        marshal_exact(&big).expect("exact"),
        marshal(&big).expect("one-pass"),
        "spilled hint cache must not change the encoding"
    );
}
