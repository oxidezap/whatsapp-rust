//! Manual startup cache allocation diagnostic. Run with `--profile profiling
//! --features bench-harness --example lid_pn_footprint`. Storage-shaped input
//! strings are allocated before DHAT starts; conversion to shared identifiers
//! and cache publication are measured. Retained bytes are requested Rust heap,
//! excluding the input strings allocated before profiling, SQLite and RSS.
//! DHAT JSON profiles are written to the platform's temporary directory.
#![allow(clippy::print_stdout)]
use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};
use wacore::types::{LearningSource, LidPnEntry};
use whatsapp_rust::lid_pn_cache::LidPnCache;

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::hint::spin_loop();
    }
}

fn main() {
    for n in [64, 512, 3443, 4096] {
        let cache = LidPnCache::new();
        let entries: Vec<_> = (0..n)
            .map(|i| (format!("100000000{i:06}"), format!("1555000{i:05}")))
            .collect();
        let profiler = dhat::Profiler::builder()
            .file_name(std::env::temp_dir().join(format!("lid-pn-{n}.json")))
            .build();
        run(cache.warm_up(
            entries
                .into_iter()
                .map(|(lid, pn)| LidPnEntry::with_timestamp(lid, pn, 1, LearningSource::Other)),
        ));
        let s = dhat::HeapStats::get();
        println!(
            "n={n} allocations={} total_bytes={} retained={} peak={}",
            s.total_blocks, s.total_bytes, s.curr_bytes, s.max_bytes
        );
        drop(profiler);
        assert_eq!(run(cache.lid_count()), n);
    }
}
