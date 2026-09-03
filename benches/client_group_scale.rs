//! Client-level group device resolution, swept across the number of groups
//! an account is active in.
//!
//! `client_group_send` sweeps one group across its size and holds the memo
//! warm by construction, so it cannot see what an account in many groups
//! gets from the per-group device memo. This target builds a client holding
//! `GROUP_COUNTS` groups of `SCALE_GROUP_MEMBERS` members each and asks the
//! two questions that decide whether a warm group send re-resolves its
//! members:
//!
//! - `warm_resolve_pass`: one pass over every group through the memoized
//!   resolver, the shape of a bot answering in each of its groups in turn.
//!   With the memo bounded at 64 entries and evicting oldest-first this was
//!   a cliff: every group past the 64th missed on every pass, and a miss at
//!   64 members re-resolves every member.
//! - `resolve_after_unrelated_refresh`: one group's device-list refresh
//!   through the real registry write path, then a pass over every *other*
//!   group. Whether those are re-stamped or recomputed is decided by the
//!   topology log, which used to overflow on any refresh past ~85 members.
//!
//! Not covered, so no number here is over-read: no Signal sessions and no
//! sends (the resolver is what is measured, not the encrypt), the
//! `InMemoryBackend` (a registry miss into SQLite is a separate cost, and a
//! larger one), and a single member device per member.

use divan::black_box;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use whatsapp_rust::bench_support::{GROUP_COUNTS, GroupScaleHarness, SCALE_GROUP_MEMBERS};

fn main() {
    divan::main();
}

/// Few, long samples: every sample is a full pass over the groups, and the
/// fixture's memo state is what the benchmark is about, so the runs must not
/// be so many that the sampled steady state drifts from the warm one.
const SAMPLE_COUNT: u32 = 10;
const SAMPLE_SIZE: u32 = 5;

/// One fixture per (benchmark, group count), leaked for the process lifetime,
/// for the reasons `client_group_send` gives: building one seeds tens of
/// thousands of registry and mapping entries, and the refresh benchmark
/// mutates its fixture's topology in a way the warm one must not inherit.
fn shared(label: &'static str, groups: usize) -> &'static GroupScaleHarness {
    static FIXTURES: OnceLock<Mutex<HashMap<(&'static str, usize), &'static GroupScaleHarness>>> =
        OnceLock::new();
    let mut map = FIXTURES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("fixture registry");
    map.entry((label, groups)).or_insert_with(|| {
        Box::leak(Box::new(GroupScaleHarness::new(
            groups,
            SCALE_GROUP_MEMBERS,
        )))
    })
}

/// One warm pass over every group, per group.
#[divan::bench(args = GROUP_COUNTS, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn warm_resolve_pass(bencher: divan::Bencher, groups: usize) {
    let harness = shared("warm", groups);
    bencher
        .counter(divan::counter::ItemsCount::new(groups))
        .bench(|| black_box(harness.resolve_all()));
}

/// Refresh group 0's device lists, then one pass over every other group.
#[divan::bench(args = GROUP_COUNTS, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn resolve_after_unrelated_refresh(bencher: divan::Bencher, groups: usize) {
    let harness = shared("refresh", groups);
    bencher
        .counter(divan::counter::ItemsCount::new(groups - 1))
        .bench(|| {
            harness.refresh_group_registry(0);
            black_box(harness.resolve_range(1, harness.group_count()))
        });
}
