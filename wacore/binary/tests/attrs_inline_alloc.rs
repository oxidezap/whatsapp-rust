//! Locks the inline-Attrs property: building a node whose attributes fit the
//! inline capacity must not touch the heap for the attribute storage. A plain
//! `Vec` backing (the regression this guards) pays one allocation per node on
//! the encode hot path.
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
fn attrs_inline_and_spill_behavior() {
    // The per-recipient fanout shape: short static keys, inline-able values.
    // Tag and keys are borrowed statics; CompactString keeps short values inline.
    // Take the minimum delta over many iterations. The counter is process-global,
    // so harness threads can bleed allocations into any single window, but they
    // are sporadic: if inline attrs truly never allocate, at least one of the 100
    // windows is noise-free and the minimum lands on 0. A heap-backed Attrs
    // allocates on every single iteration, so its minimum can never reach 0.
    let mut min_delta = u64::MAX;
    for _ in 0..100 {
        let before = ALLOCS.load(Ordering::Relaxed);
        let node = NodeBuilder::new("enc")
            .attr("v", "2")
            .attr("type", "msg")
            .build();
        let after = ALLOCS.load(Ordering::Relaxed);
        min_delta = min_delta.min(after - before);
        assert_eq!(node.attrs.len(), 2);
        assert!(!node.attrs.0.spilled(), "2 attrs must stay inline");
    }
    assert_eq!(
        min_delta, 0,
        "a node with <= 2 attrs must keep them inline (no heap allocation)"
    );

    // Larger attribute lists keep working by spilling to the heap.
    let node = NodeBuilder::new("message")
        .attr("to", "5511999990000@s.whatsapp.net")
        .attr("id", "3EB0A9252A8F12B7E2")
        .attr("type", "text")
        .attr("t", "1760000000")
        .attr("phash", "2:abcdefgh")
        .build();
    assert_eq!(node.attrs.len(), 5);
    assert!(
        node.attrs.0.spilled(),
        "5 attrs must spill past the inline capacity"
    );
    assert_eq!(
        node.attrs.get("type").map(|v| v.as_str().into_owned()),
        Some("text".to_string())
    );
}
