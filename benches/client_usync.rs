//! USync device-resolution hot path: the per-user work every send pays before
//! any crypto runs.
//!
//! `client_group_send` / `client_dm_send` measure the whole warm send; what
//! they cannot isolate is the addressing normalization underneath it:
//! `Jid::to_non_ad` per user (`src/usync.rs:61`), the second `into_non_ad`
//! pass over the same list (`:79`), the `to_protocol_address()` `String` per
//! device (`wacore/src/send/encrypt.rs`), and the `DeviceListSpec` build for
//! a registry miss. Each row here pins one of those terms so a change to it
//! reads as a delta instead of hiding inside a whole-send number.
//!
//! Pure CPU, no runtime: every function measured is synchronous. Fixtures are
//! built once per input width and each iteration works a batch, since one JID
//! conversion is nanoseconds, below cloud wall-clock noise (same note as
//! `wacore/binary/benches/jid_benchmark.rs`).
//!
//! What it does not cover, so no number here is over-read: the registry read
//! itself (see `store_shapes` / `store_contention`), the network IQ, and the
//! encrypt that follows. `into_non_ad` on an already-normalized list is the
//! documented no-op second pass, kept as the regression that justifies
//! removing it.

use divan::{black_box, counter::ItemsCount};
use std::sync::OnceLock;
use wacore::iq::usync::DeviceListSpec;
use wacore::types::jid::JidExt;
use wacore_binary::jid::{Jid, Server};

fn main() {
    divan::main();
}

/// User counts worth distinguishing: a DM burst, a mid-size group, a large
/// group sweep. What changes across them is the per-user slope, not the
/// fixture.
const USER_COUNTS: [usize; 3] = [32, 128, 512];

/// Work per measured iteration: enough conversions that the batch clears timer
/// noise, small enough that the fixture stays in cache.
const BATCH: usize = 512;

/// Fixed-width fictional users from the reserved 555-01xx block, alternating
/// PN/LID servers the way a migrated group's member set looks. Six digits
/// cover the largest fixture; widths never change mid-run.
fn users(n: usize) -> &'static Vec<Jid> {
    static BUILT: OnceLock<Vec<Vec<Jid>>> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        USER_COUNTS
            .iter()
            .map(|&m| {
                (0..m)
                    .map(|i| {
                        let server = if i % 2 == 0 { Server::Pn } else { Server::Lid };
                        Jid::new(format!("1555000{i:05}"), server)
                    })
                    .collect()
            })
            .collect()
    });
    let idx = USER_COUNTS
        .iter()
        .position(|&m| m == n)
        .expect("known width");
    &built[idx]
}

/// `Jid::to_non_ad` per user: the first pass `get_user_devices` always pays.
#[divan::bench(args = USER_COUNTS)]
fn normalize_borrow(bencher: divan::Bencher, n: usize) {
    let input = users(n);
    bencher.counter(ItemsCount::new(BATCH * n)).bench(|| {
        let mut done = 0usize;
        for _ in 0..BATCH {
            for jid in input.iter() {
                black_box(jid.to_non_ad());
                done += 1;
            }
        }
        black_box(done)
    });
}

/// `Jid::into_non_ad` over the already-normalized list: the second pass, a
/// functional no-op that still walks and branches per user.
#[divan::bench(args = USER_COUNTS)]
fn normalize_owned_second_pass(bencher: divan::Bencher, n: usize) {
    bencher.counter(ItemsCount::new(BATCH * n)).bench(|| {
        let mut done = 0usize;
        for _ in 0..BATCH {
            // Cloned outside the timed region in spirit: `with_inputs` would
            // exclude it, but the clone itself is part of what the second
            // pass costs a caller that could have passed borrows instead.
            let mut owned: Vec<Jid> = users(n).clone();
            for jid in owned.drain(..) {
                black_box(jid.into_non_ad());
                done += 1;
            }
        }
        black_box(done)
    });
}

/// `to_protocol_address()` per device: the `user:device@server` `String` each
/// device encrypt allocates to key its session lookup.
#[divan::bench(args = USER_COUNTS)]
fn protocol_address_per_device(bencher: divan::Bencher, n: usize) {
    let input = users(n);
    bencher.counter(ItemsCount::new(BATCH * n)).bench(|| {
        let mut done = 0usize;
        for _ in 0..BATCH {
            for jid in input.iter() {
                black_box(jid.with_device(0).to_protocol_address());
                done += 1;
            }
        }
        black_box(done)
    });
}

/// `DeviceListSpec::new` for a full miss: the query object a registry miss
/// builds before the IQ goes out. Pure construction, no IO.
#[divan::bench(args = USER_COUNTS)]
fn usync_spec_build(bencher: divan::Bencher, n: usize) {
    bencher.counter(ItemsCount::new(n)).bench(|| {
        let spec = DeviceListSpec::new(black_box(users(n).clone()), "bench-sid");
        black_box(spec.jids.len())
    });
}

/// `sort_dedup_by_user` over the miss list: paid once per fetch before the
/// network, O(N log N) over users.
#[divan::bench(args = USER_COUNTS)]
fn sort_dedup_users(bencher: divan::Bencher, n: usize) {
    bencher.counter(ItemsCount::new(n)).bench(|| {
        let mut list = users(n).clone();
        list.extend(users(n).iter().cloned());
        wacore::types::jid::sort_dedup_by_user(black_box(&mut list));
        black_box(list.len())
    });
}
