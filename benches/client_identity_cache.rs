//! LID↔PN identity lookups: the mapping every Signal address resolution pays.
//!
//! Both DM and group sends resolve each device's Signal address through
//! `LidPnCache`, and the msg-secret path re-checks the alternate JID on a
//! miss (`e18d9b10b`). `store_shapes` measures the DB scan behind a cold
//! start; what is measured here is the warm in-process hit and miss, which is
//! what every send after the first pays.
//!
//! Driven on a bare poll loop rather than a Tokio runtime: the cache's locks
//! are `async_lock` primitives that resolve without yielding when
//! uncontended, so every future here completes on its first poll, and a
//! runtime in the loop would put its own scheduling on the clock instead of
//! the lookup's. Same pattern as `benches/jid_keyed_cache.rs`.
//!
//! Fixtures are built once and each iteration works a batch: one lookup is
//! tens of nanoseconds, below cloud wall-clock noise. Probes cycle rather
//! than repeat so one process's hash seed cannot amplify a lucky bucket into
//! a baseline shift (same note as `jid_keyed_cache`).
//!
//! What it does not cover: the persistent warm-up scan (see `store_shapes`
//! `lid_pn_warm_up_scan`), the usync fetch that learns a miss, and the
//! `SenderKeyDeviceMap` gate, which is `pub(crate)` and therefore measured
//! only through `client_group_send`'s warm-send rows.

use divan::{black_box, counter::ItemsCount};
use std::future::Future;
use std::pin::pin;
use std::sync::OnceLock;
use std::task::{Context, Poll, Waker};
use wacore::types::{LearningSource, LidPnEntry};
use whatsapp_rust::lid_pn_cache::LidPnCache;

fn main() {
    divan::main();
}

/// Drive a future to completion by polling it. Every future in this file
/// resolves on the first poll; the spin is a fallback that hangs visibly
/// rather than returning a wrong answer. Same helper as `jid_keyed_cache`.
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

/// Mapping counts worth distinguishing: a 1:1-heavy account, a group-heavy
/// one, a large community member set.
const ENTRY_COUNTS: [usize; 3] = [64, 512, 4096];

/// Lookups per measured iteration.
const BATCH: usize = 1024;

/// Fictional but well-shaped identifiers: 15-digit LIDs, E.164-like PNs.
fn entry(i: usize) -> LidPnEntry {
    LidPnEntry::new(
        format!("100000000{i:06}"),
        format!("1555000{i:05}"),
        LearningSource::Usync,
    )
}

struct Fixture {
    cache: LidPnCache,
    phones_hit: Vec<String>,
    lids_hit: Vec<String>,
    phones_miss: Vec<String>,
    lids_miss: Vec<String>,
}

fn fixture(n: usize) -> &'static Fixture {
    static BUILT: OnceLock<Vec<Fixture>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        ENTRY_COUNTS
            .iter()
            .map(|&m| {
                let cache = LidPnCache::new();
                block_on(async {
                    for i in 0..m {
                        cache.add(&entry(i)).await;
                    }
                });
                Fixture {
                    cache,
                    phones_hit: (0..m).map(|i| format!("1555000{i:05}")).collect(),
                    lids_hit: (0..m).map(|i| format!("100000000{i:06}")).collect(),
                    phones_miss: (0..m).map(|i| format!("1555999{i:05}")).collect(),
                    lids_miss: (0..m).map(|i| format!("199999999{i:06}")).collect(),
                }
            })
            .collect()
    });
    let idx = ENTRY_COUNTS
        .iter()
        .position(|&m| m == n)
        .expect("known width");
    &built[idx]
}

/// PN → LID: the direction a PN-addressed send resolves per device.
#[divan::bench(args = ENTRY_COUNTS)]
fn lid_lookup_by_phone_hit(bencher: divan::Bencher, n: usize) {
    let fx = fixture(n);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut found = 0usize;
        for i in 0..BATCH {
            found += block_on(fx.cache.get_current_lid(black_box(&fx.phones_hit[i % n]))).is_some()
                as usize;
        }
        black_box(found)
    });
}

#[divan::bench(args = ENTRY_COUNTS)]
fn lid_lookup_by_phone_miss(bencher: divan::Bencher, n: usize) {
    let fx = fixture(n);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut absent = 0usize;
        for i in 0..BATCH {
            absent += block_on(fx.cache.get_current_lid(black_box(&fx.phones_miss[i % n])))
                .is_none() as usize;
        }
        black_box(absent)
    });
}

/// LID → PN: the direction group sends and the msg-secret alternate-JID check
/// use.
#[divan::bench(args = ENTRY_COUNTS)]
fn pn_lookup_by_lid_hit(bencher: divan::Bencher, n: usize) {
    let fx = fixture(n);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut found = 0usize;
        for i in 0..BATCH {
            found += block_on(fx.cache.get_phone_number(black_box(&fx.lids_hit[i % n]))).is_some()
                as usize;
        }
        black_box(found)
    });
}

#[divan::bench(args = ENTRY_COUNTS)]
fn pn_lookup_by_lid_miss(bencher: divan::Bencher, n: usize) {
    let fx = fixture(n);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut absent = 0usize;
        for i in 0..BATCH {
            absent += block_on(fx.cache.get_phone_number(black_box(&fx.lids_miss[i % n]))).is_none()
                as usize;
        }
        black_box(absent)
    });
}
