//! Client cache maintenance: the write and sweep terms around the read path.
//!
//! `jid_keyed_cache` pins the per-message `get` hit/miss/create; what it
//! cannot see is what keeps that cache (and every sibling `Cache<Jid, _>`)
//! bounded: plain `insert` throughput, insertion at capacity forcing FIFO
//! eviction, and the `run_pending_tasks` sweep the keepalive maintenance tick
//! runs (`fa36dc61e`). The sweep row is the regression for re-adding unbounded
//! growth: with no TTL configured it must stay near-free, with expired
//! entries it must scale with the expired set, not the resident one.
//!
//! Driven on a bare poll loop rather than a Tokio runtime, for the reason
//! `jid_keyed_cache`'s header gives: uncontended `async_lock` futures complete
//! on the first poll, and a runtime would bill its scheduling to the lookup.
//!
//! What it does not cover: the `TypedCache` custom-store path
//! (`src/cache_store.rs:96` `to_string` per op), which needs a backend and
//! belongs in `store_shapes`/`store_contention`, and the offline receipt
//! grouping itself (`group_delivery_receipts` is crate-private; its burst
//! shape is covered by `inbound_stanza`'s `burst_*` rows and its alloc floor
//! by its unit test).

use divan::{black_box, counter::ItemsCount};
use std::future::Future;
use std::pin::pin;
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use wacore_binary::jid::Jid;
use whatsapp_rust::cache::Cache;

fn main() {
    divan::main();
}

/// Same first-poll driver as `jid_keyed_cache`; see its header.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut cx) {
            return value;
        }
        std::hint::spin_loop();
    }
}

const CACHE_SIZES: [usize; 3] = [64, 512, 4096];
const BATCH: usize = 512;

fn key(i: usize) -> Jid {
    Jid::pn(format!("5511999{i:06}"))
}

/// Warm caches per size, built once; values are cheap so the clock keeps the
/// cache bookkeeping, not the payload.
fn warm(n: usize) -> &'static Cache<Jid, Arc<()>> {
    static BUILT: OnceLock<Vec<Cache<Jid, Arc<()>>>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        CACHE_SIZES
            .iter()
            .map(|&m| {
                let cache: Cache<Jid, Arc<()>> = Cache::builder().max_capacity(m as u64).build();
                block_on(async {
                    for i in 0..m {
                        cache.insert(key(i), Arc::new(())).await;
                    }
                });
                cache
            })
            .collect()
    });
    let idx = CACHE_SIZES
        .iter()
        .position(|&m| m == n)
        .expect("known size");
    &built[idx]
}

/// Plain insert throughput into a warm cache that never evicts: capacity is
/// `n + BATCH`, so the batch fits beside the resident set and every
/// iteration starts and ends with the same `n` entries.
#[divan::bench(args = CACHE_SIZES)]
fn cache_insert(bencher: divan::Bencher, n: usize) {
    static ROOMY: OnceLock<Vec<Cache<Jid, Arc<()>>>> = OnceLock::new();
    let built = ROOMY.get_or_init(|| {
        CACHE_SIZES
            .iter()
            .map(|&m| {
                let cache: Cache<Jid, Arc<()>> =
                    Cache::builder().max_capacity((m + BATCH) as u64).build();
                block_on(async {
                    for i in 0..m {
                        cache.insert(key(i), Arc::new(())).await;
                    }
                });
                cache
            })
            .collect()
    });
    let idx = CACHE_SIZES
        .iter()
        .position(|&m| m == n)
        .expect("known size");
    let cache = &built[idx];
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        block_on(async {
            for i in 0..BATCH {
                cache.insert(black_box(key(n + i)), Arc::new(())).await;
            }
        });
        // Back to the steady state so every iteration measures the same work
        // instead of growing the fixture without bound.
        block_on(async {
            for i in 0..BATCH {
                cache.invalidate(black_box(&key(n + i))).await;
            }
        });
        black_box(cache.entry_count())
    });
}

/// Insertion at capacity: every insert evicts one entry, so the FIFO scan is
/// on the bill. The per-iteration cost must not grow with what is retained.
#[divan::bench(args = CACHE_SIZES)]
fn cache_insert_at_capacity(bencher: divan::Bencher, n: usize) {
    static FULL: OnceLock<Vec<Cache<Jid, Arc<()>>>> = OnceLock::new();
    let built = FULL.get_or_init(|| {
        CACHE_SIZES
            .iter()
            .map(|&m| {
                let cache: Cache<Jid, Arc<()>> = Cache::builder().max_capacity(m as u64).build();
                block_on(async {
                    for i in 0..m {
                        cache.insert(key(i), Arc::new(())).await;
                    }
                });
                cache
            })
            .collect()
    });
    let idx = CACHE_SIZES
        .iter()
        .position(|&m| m == n)
        .expect("known size");
    let cache = &built[idx];
    let next = std::sync::atomic::AtomicUsize::new(n);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        block_on(async {
            for _ in 0..BATCH {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                cache.insert(black_box(key(i)), Arc::new(())).await;
            }
        });
        black_box(cache.entry_count())
    });
}

/// The maintenance-tick sweep with nothing expirable: no TTL means no table
/// walk, so this must stay flat in cache size.
#[divan::bench(args = CACHE_SIZES)]
fn sweep_idle_no_ttl(bencher: divan::Bencher, n: usize) {
    let cache = warm(n);
    bencher.counter(ItemsCount::new(1usize)).bench(|| {
        block_on(cache.run_pending_tasks());
        black_box(cache.entry_count())
    });
}

/// The sweep with expired entries present: scales with the expired set.
///
/// Entries carry a 10 ms TTL and each iteration sleeps past it before
/// sweeping, so the sweep always reclaims `n` expired entries — the assert
/// proves it, and a sweep that stopped expiring would fail instead of
/// silently measuring the live-entry walk. The sleep is a syscall, ~zero
/// instructions, so CodSpeed's instruction counts stay a clean signal; wall
/// time includes it, which is why this row runs few samples. Same 10/20 ms
/// pairing the `portable_cache` expiry tests use.
#[divan::bench(args = [64, 512], sample_count = 5, sample_size = 3)]
fn sweep_with_expired(bencher: divan::Bencher, n: usize) {
    static BUILT: OnceLock<Vec<Cache<Jid, Arc<()>>>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        [64usize, 512usize]
            .iter()
            .map(|&m| {
                let cache: Cache<Jid, Arc<()>> = Cache::builder()
                    .max_capacity((m * 2) as u64)
                    .time_to_live(Duration::from_millis(10))
                    .build();
                cache
            })
            .collect()
    });
    let idx = [64usize, 512usize]
        .iter()
        .position(|&m| m == n)
        .expect("known size");
    let cache = &built[idx];
    bencher.counter(ItemsCount::new(n)).bench(|| {
        block_on(async {
            for i in 0..n {
                cache.insert(black_box(key(i)), Arc::new(())).await;
            }
        });
        std::thread::sleep(Duration::from_millis(20));
        block_on(cache.run_pending_tasks());
        let remaining = cache.entry_count();
        assert_eq!(remaining, 0, "the sweep must reclaim every expired entry");
        black_box(remaining)
    });
}
