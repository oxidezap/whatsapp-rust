//! Locks the one-allocation property of [`Jid::to_non_ad_arc_str`]. The
//! message-secret rows build two of these per message, and the obvious
//! `to_non_ad_string().into()` spelling costs two allocations each: the
//! intermediate `String`, then the `Arc<str>` its bytes are copied into.
//!
//! Single test fn on purpose: the counting allocator is process-global, so a
//! concurrently running sibling test would bleed its allocations into the
//! measurement.

// Host-only allocation-count harness; std's 64-bit atomic is fine (never built
// for embedded targets).
#![allow(clippy::disallowed_types)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use wacore_binary::Jid;

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

/// Smallest allocation delta over many windows: the counter is process-global,
/// so harness threads can bleed into any single window, but a genuine extra
/// allocation is paid on every iteration and its minimum can never drop.
fn min_allocs<T>(iterations: u32, mut op: impl FnMut() -> T) -> u64 {
    let mut min = u64::MAX;
    for _ in 0..iterations {
        let before = ALLOCS.load(Ordering::Relaxed);
        let value = std::hint::black_box(op());
        let after = ALLOCS.load(Ordering::Relaxed);
        drop(value);
        min = min.min(after - before);
    }
    min
}

#[test]
fn to_non_ad_arc_str_costs_one_allocation() {
    // A device-qualified PN: 28 bytes of output, past CompactString's inline
    // limit, so nothing here is inline-able and every allocation is real.
    let jid: Jid = "5511987650001:33@s.whatsapp.net".parse().expect("parse");
    assert_eq!(&*jid.to_non_ad_arc_str(), "5511987650001@s.whatsapp.net");

    let direct = min_allocs(200, || jid.to_non_ad_arc_str());
    assert_eq!(direct, 1, "the Arc<str> itself is the only allocation");

    // The spelling this replaced, measured the same way, so the test states the
    // saving rather than asserting a bare number that could drift with the JID.
    let via_string = min_allocs(200, || Arc::<str>::from(jid.to_non_ad_string()));
    assert_eq!(
        via_string,
        direct + 1,
        "going through String must cost exactly the one extra allocation this removes"
    );
}
