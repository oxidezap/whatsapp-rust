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
/// 5000, so none of these evict; what changes across them is how deep the map
/// is when the lookup lands.
const CHAT_COUNTS: [usize; 3] = [16, 256, 4096];

/// Lookups per measured iteration.
const BATCH: usize = 1024;

fn chat_jid(i: usize) -> Jid {
    Jid::pn(format!("5511999{i:06}"))
}

/// A warm cache plus the two probes into it, one of each outcome.
struct Probed {
    cache: Cache<Jid, Arc<()>>,
    hit: Jid,
    miss: Jid,
}

/// One warm cache per chat count, plus a key that hits and one that misses.
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
                    hit: chat_jid(n / 2),
                    miss: Jid::pn("5511000000000"),
                }
            })
            .collect()
    });
    &built[CHAT_COUNTS
        .iter()
        .position(|&n| n == count)
        .expect("known chat count")]
}

/// The per-message path: an existing chat resolves its lane.
#[divan::bench(args = CHAT_COUNTS)]
fn bench_chat_lane_get_hit(bencher: divan::Bencher, count: usize) {
    let Probed { cache, hit, .. } = fixture(count);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut found = 0usize;
        for _ in 0..BATCH {
            found += block_on(cache.get(black_box(hit))).is_some() as usize;
        }
        black_box(found)
    });
}

/// First message from a chat: the lookup misses and a lane has to be created.
#[divan::bench(args = CHAT_COUNTS)]
fn bench_chat_lane_get_miss(bencher: divan::Bencher, count: usize) {
    let Probed { cache, miss, .. } = fixture(count);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut absent = 0usize;
        for _ in 0..BATCH {
            absent += block_on(cache.get(black_box(miss))).is_none() as usize;
        }
        black_box(absent)
    });
}

/// Creating the lane, which is what a miss leads to. Overwrites one key rather
/// than growing the cache, so the arms stay comparable and nothing evicts.
#[divan::bench(args = CHAT_COUNTS)]
fn bench_chat_lane_insert(bencher: divan::Bencher, count: usize) {
    let Probed { cache, hit, .. } = fixture(count);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        for _ in 0..BATCH {
            block_on(cache.insert(black_box(hit).clone(), Arc::new(())));
        }
        black_box(cache.entry_count())
    });
}
