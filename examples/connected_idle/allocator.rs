//! Rust-requested live/peak bytes. SQLite's C malloc and allocator metadata are
//! deliberately not claimed here; Linux RssAnon is the complementary process metric.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

pub struct Counting;

fn add(bytes: usize) {
    let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
    PEAK.fetch_max(live, Ordering::Relaxed);
}

// SAFETY: every operation delegates to System with unchanged pointers/layouts;
// only allocation-free atomic accounting is added, and failed allocations do
// not alter the counters. Zeroed requests retain System's calloc/lazy-page
// behavior instead of the trait default's eager memset through alloc.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            add(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let next = unsafe { System.realloc(ptr, layout, new_size) };
        if !next.is_null() {
            if new_size >= layout.size() {
                add(new_size - layout.size());
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        next
    }
}

pub fn live() -> usize {
    LIVE.load(Ordering::Relaxed)
}

pub fn peak() -> usize {
    PEAK.load(Ordering::Relaxed)
}

pub fn reset_peak() {
    PEAK.store(live(), Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_growth_shrink_release_and_peak_are_counted() {
        // Bounds allow the test runner's small concurrent allocations. The only
        // allocator test in this binary, so no other large test runs overlap.
        const SIZE: usize = 4 * 1024 * 1024;
        let baseline = live();
        reset_peak();
        let mut bytes = vec![0u8; SIZE];
        std::hint::black_box(&bytes);
        assert!(live() >= baseline + SIZE);
        bytes.resize(2 * SIZE, 1);
        std::hint::black_box(&bytes);
        assert!(live() >= baseline + 2 * SIZE);
        assert!(peak() >= baseline + 2 * SIZE);
        bytes.truncate(SIZE / 2);
        bytes.shrink_to_fit();
        assert!(live() < baseline + SIZE);
        drop(bytes);
        assert!(live() < baseline + SIZE / 2);
        assert!(peak() >= baseline + 2 * SIZE);
    }
}
