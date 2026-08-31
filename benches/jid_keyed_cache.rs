//! The `Cache<Jid, _>` lookup every inbound message performs.
//!
//! `chat_lanes` (`src/client.rs`) is the hottest structure this repository
//! keys by `Jid`: `handlers::message` resolves one lane per incoming message
//! before anything else happens, so its cost is paid per message rather than
//! per send. The lane itself cannot be driven from a bench target -- `ChatLane`
//! and the field holding it are crate-private, and reaching them needs a
//! connected `Client` -- so what is measured here is the cache it lives in,
//! built the same way (`max_capacity` + `evict_guard`) and keyed the same way,
//! with a value cheap enough that what is left on the clock is the `Jid` hash,
//! the `Jid` equality on a hit, and the cache's own bookkeeping.
//!
//! Driven on a bare poll loop rather than a Tokio runtime: the cache's locks
//! are `async_lock` primitives that resolve without yielding when uncontended,
//! so every future here completes on its first poll, and a runtime in the loop
//! would put its own scheduling on the clock instead of the lookup's.
//!
//! Fixtures are built once and leaked, and each iteration works a batch: a
//! single lookup is a few nanoseconds, below this repository's documented
//! wall-clock noise on cloud hardware. See the same note in
//! `wacore/binary/benches/jid_benchmark.rs`.

use divan::black_box;
use divan::counter::ItemsCount;
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll, Waker};
use wacore_binary::jid::Jid;
use whatsapp_rust::cache::Cache;

fn main() {
    divan::main();
}

/// Drive a future to completion by polling it. Every future in this file
/// resolves on the first poll, so the busy loop is a fallback that never runs;
/// it is here so a future that did park would hang visibly rather than return
/// a wrong answer.
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

/// Live-chat counts worth distinguishing. The default `chat_lanes_capacity` is
/// 5000, so none of these evict on the lookup benches; what changes across them
/// is how deep the map is when the lookup lands.
const CHAT_COUNTS: [usize; 3] = [16, 256, 4096];

/// Lookups per measured iteration.
const BATCH: usize = 1024;

/// Fixed-width so every key costs the same to hash and compare, whichever
/// rotation a bench is on. Six digits covers the largest fixture with room to
/// spare, and never widens mid-run.
fn chat_jid(i: usize) -> Jid {
    Jid::pn(format!("5511999{i:06}"))
}

/// A key of the same width that no fixture ever inserts, for the miss probes.
fn absent_jid(i: usize) -> Jid {
    Jid::pn(format!("5511000{i:06}"))
}

/// `count + 1` keys per chat count, built once. One more than the cache holds,
/// so a rotation through them always lands on the key that is currently absent
/// -- which is what lets the creation bench stay on the miss path without
/// building a key inside the measured loop.
fn keys(count: usize) -> &'static [Jid] {
    static KEYS: OnceLock<Vec<Vec<Jid>>> = OnceLock::new();
    let built = KEYS.get_or_init(|| {
        CHAT_COUNTS
            .iter()
            .map(|&n| (0..=n).map(chat_jid).collect())
            .collect()
    });
    &built[index_of(count)]
}

fn index_of(count: usize) -> usize {
    CHAT_COUNTS
        .iter()
        .position(|&n| n == count)
        .expect("known chat count")
}

/// A warm cache plus a set of probes into it for each outcome.
///
/// Both sets are cycled rather than repeated, for the reason
/// `bench_jid_hashmap_get` cycles its own: the cache hashes with a per-process
/// `RandomState`, so one fixed probe measures whichever bucket and collision
/// chain that process happened to draw. Repeating it a thousand times only
/// amplifies the draw, and CodSpeed compares two separate processes -- the
/// baseline could differ from the branch with no code change at all.
struct Probed {
    cache: Cache<Jid, Arc<()>>,
    hits: Vec<Jid>,
    misses: Vec<Jid>,
}

/// One warm cache per chat count, plus the probe sets that hit and miss it.
fn fixture(count: usize) -> &'static Probed {
    static BUILT: OnceLock<Vec<Probed>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        CHAT_COUNTS
            .iter()
            .map(|&n| {
                let cache: Cache<Jid, Arc<()>> = Cache::builder()
                    .max_capacity(5_000)
                    .evict_guard(|v: &Arc<()>| Arc::strong_count(v) <= 1)
                    .build();
                block_on(async {
                    for i in 0..n {
                        cache.insert(chat_jid(i), Arc::new(())).await;
                    }
                });
                Probed {
                    cache,
                    hits: (0..n).map(chat_jid).collect(),
                    misses: (0..n).map(absent_jid).collect(),
                }
            })
            .collect()
    });
    &built[index_of(count)]
}

/// The per-message path: an existing chat resolves its lane.
#[divan::bench(args = CHAT_COUNTS)]
fn bench_chat_lane_get_hit(bencher: divan::Bencher, count: usize) {
    let Probed { cache, hits, .. } = fixture(count);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut found = 0usize;
        for i in 0..BATCH {
            let key = &hits[i % hits.len()];
            found += block_on(cache.get(black_box(key))).is_some() as usize;
        }
        black_box(found)
    });
}

/// First message from a chat: the lookup misses and a lane has to be created.
#[divan::bench(args = CHAT_COUNTS)]
fn bench_chat_lane_get_miss(bencher: divan::Bencher, count: usize) {
    let Probed { cache, misses, .. } = fixture(count);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut absent = 0usize;
        for i in 0..BATCH {
            let key = &misses[i % misses.len()];
            absent += block_on(cache.get(black_box(key))).is_none() as usize;
        }
        black_box(absent)
    });
}

/// A cache held at capacity, and the rotation position that is absent from it.
struct AtCapacity {
    cache: Cache<Jid, Arc<()>>,
    next: AtomicUsize,
    /// Values kept alive so `evict_guard` refuses them, standing in for lanes
    /// whose `enqueue_lock` is still held by message processing. Never read;
    /// holding the `Arc` is the whole point.
    _protected: Vec<Arc<()>>,
}

/// Creating a lane, which is what a first message from a chat actually does.
///
/// Driven through `get_with_by_ref`, not `insert`, because that is what
/// `handlers::message` calls: the miss path behind it hashes and acquires a
/// per-key init lock, re-checks under it, converts the borrowed key to an owned
/// one, inserts, clones the value out and reclaims the lock. A bare `insert`
/// measures none of that, and an `insert` of a key already present measures
/// less still -- it returns from the overwrite branch before `insert_new`.
///
/// The cache is built at `max_capacity = count` and the keys rotate through
/// `count + 1` prebuilt values, so every iteration lands on the one key FIFO
/// eviction has just removed: always a miss, always the same key width, and no
/// allocation inside the measured loop.
///
/// `PROTECTED` picks whether some entries are ineligible for eviction. That is
/// the guard `chat_lanes` actually runs -- a lane whose lock is held must be
/// skipped -- and it makes the eviction scan walk past entries instead of
/// taking the first FIFO candidate, which is the depth-dependent part.
#[divan::bench(args = CHAT_COUNTS, consts = [0usize, 8usize])]
fn bench_chat_lane_create<const PROTECTED: usize>(bencher: divan::Bencher, count: usize) {
    static FULL: OnceLock<Vec<Vec<AtCapacity>>> = OnceLock::new();
    let built = FULL.get_or_init(|| {
        [0usize, 8usize]
            .iter()
            .map(|&protected| {
                CHAT_COUNTS
                    .iter()
                    .map(|&n| {
                        let cache: Cache<Jid, Arc<()>> = Cache::builder()
                            .max_capacity(n as u64)
                            .evict_guard(|v: &Arc<()>| Arc::strong_count(v) <= 1)
                            .build();
                        let mut held = Vec::new();
                        block_on(async {
                            for (i, key) in keys(n).iter().take(n).enumerate() {
                                let value = Arc::new(());
                                if i < protected {
                                    held.push(Arc::clone(&value));
                                }
                                cache.insert(key.clone(), value).await;
                            }
                        });
                        AtCapacity {
                            cache,
                            next: AtomicUsize::new(n - protected),
                            _protected: held,
                        }
                    })
                    .collect()
            })
            .collect()
    });
    let slot = if PROTECTED == 0 { 0 } else { 1 };
    let AtCapacity { cache, next, .. } = &built[slot][index_of(count)];
    // The protected keys are pinned in the cache forever, so probing one is a
    // hit, not a creation. Rotate only over the evictable tail: the cache holds
    // `PROTECTED` pinned plus `count - PROTECTED` of these, one short of the
    // set, so the rotation still lands on the absent key every time.
    let rotating = &keys(count)[PROTECTED..];

    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        for _ in 0..BATCH {
            let i = next.fetch_add(1, Ordering::Relaxed) % rotating.len();
            block_on(cache.get_with_by_ref(black_box(&rotating[i]), async { Arc::new(()) }));
        }
        black_box(cache.entry_count())
    });
}
