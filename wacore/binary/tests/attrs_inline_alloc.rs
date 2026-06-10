//! Locks the inline-Attrs property: building a node whose attributes fit the
//! inline capacity must not touch the heap for the attribute storage. A plain
//! `Vec` backing (the regression this guards) pays one allocation per node on
//! the encode hot path.

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
fn building_fanout_shaped_node_does_not_heap_allocate() {
    // The per-recipient fanout shape: short static keys, inline-able values.
    // Tag and keys are borrowed statics; CompactString keeps short values inline.
    let before = ALLOCS.load(Ordering::Relaxed);
    let node = NodeBuilder::new("enc")
        .attr("v", "2")
        .attr("type", "msg")
        .build();
    let after = ALLOCS.load(Ordering::Relaxed);
    assert_eq!(
        after - before,
        0,
        "a node with <= 2 attrs must keep them inline (no heap allocation)"
    );
    assert_eq!(node.attrs.len(), 2);
}

#[test]
fn larger_attr_lists_still_work_by_spilling() {
    let node = NodeBuilder::new("message")
        .attr("to", "5511999990000@s.whatsapp.net")
        .attr("id", "3EB0A9252A8F12B7E2")
        .attr("type", "text")
        .attr("t", "1760000000")
        .attr("phash", "2:abcdefgh")
        .build();
    assert_eq!(node.attrs.len(), 5);
    assert_eq!(
        node.attrs.get("type").map(|v| v.as_str().into_owned()),
        Some("text".to_string())
    );
}
