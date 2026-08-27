//! Client-level group send, swept across group size.
//!
//! `wacore`'s `send_receive_benchmark` covers the pure stanza preparation, and
//! PR #1279 pinned it flat in group size. Everything *around* it — the group
//! metadata `Arc`, the per-group device memo, the sender-key device map, the
//! SKDM target filter, the signal-cache flush, marshalling and the noise
//! socket — lives in the `whatsapp-rust` crate, which had no benchmarks. An
//! external whole-client profile attributed ~670 instructions per additional
//! member per message to that remainder; this target is what makes the claim
//! testable in-process.
//!
//! What it does not cover, stated once so no number here is over-read:
//!
//! - **Receive.** The external profile's `group-send` scenario is server-paced,
//!   so each of its "messages" includes decoding an inbound stanza as well as
//!   producing the outbound one. Everything below is send-only.
//! - **LID addressing.** The fixture is PN-addressed, so
//!   `GroupInfo::phone_jid_for_lid_user` is never reached. A LID sweep needs a
//!   registry seeded with LID↔PN mappings, which is a different fixture rather
//!   than a parameter on this one. The *hit-rate* question is covered for LID
//!   groups by `repeat_lid_group_sends_hit_both_device_memos_on_every_send`
//!   in `src/send/mod.rs`, which is what settles whether that function is on
//!   a warm send's bill at all — it runs only inside the uncached resolve.
//! - **The SQLite backend.** The fixture stores through `InMemoryBackend`, so
//!   the storage engine's own per-send cost is excluded by construction.
//! - **First contact.** The fixture reaches its steady state before measuring,
//!   so the cold fan-out (an SKDM per member, `pkmsg` payloads) is setup, not
//!   measurement. That path is not swept here.
//! - **The socket write.** The transport drops the frame it is handed; the
//!   marshal, noise encrypt and framing before it are measured.

use divan::black_box;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use whatsapp_rust::bench_support::{GROUP_SIZES, GroupSendHarness};

fn main() {
    divan::main();
}

/// Per-send iteration budget for any benchmark that advances the sender-key
/// chain.
///
/// A group send rotates its sender key once the chain iteration passes 1000
/// (`SENDER_KEY_ROTATION_THRESHOLD`), and a rotation re-distributes to every
/// member — tens of millions of instructions at 512 members. Amortised over a
/// long run that shows up as growth in group size that has nothing to do with
/// a steady-state send: measured, a 1000-iteration sweep at 512 members read
/// the per-send cost ~10% high against a sub-threshold sweep, purely from the
/// one rotation it crossed. `sample_count × sample_size` is kept well under the
/// threshold so no run can reach it.
const SAMPLE_COUNT: u32 = 20;
const SAMPLE_SIZE: u32 = 20;

/// One fixture per (benchmark, group size), leaked for the process lifetime.
///
/// Building one is not cheap — at 512 members it establishes 513 Signal
/// sessions and runs a cold fan-out to all of them, ~2.7 billion instructions.
/// Divan generates `with_inputs` values per iteration, so a fresh harness per
/// iteration would spend essentially all of its wall clock in setup; under
/// CodSpeed's Valgrind instrumentation that is the difference between a viable
/// CI job and an unviable one.
///
/// Keyed by benchmark and not by size alone, so no benchmark inherits state
/// another one left behind. Two things leak across a shared fixture and both
/// are silent: `warm_group_send` advances the sender-key chain toward the
/// rotation threshold, and its sends re-arm the coalesced signal-flush worker,
/// which on this single-threaded runtime can then only fire inside some later
/// measured closure. Sixteen setups instead of four is the price of results
/// that do not depend on the order benchmarks run in.
fn shared(label: &'static str, group_size: usize) -> &'static GroupSendHarness {
    static FIXTURES: OnceLock<Mutex<HashMap<(&'static str, usize), &'static GroupSendHarness>>> =
        OnceLock::new();
    let mut map = FIXTURES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("fixture registry");
    map.entry((label, group_size))
        .or_insert_with(|| Box::leak(Box::new(GroupSendHarness::new(group_size))))
}

/// One warm group send through the real `Client::send_message`, from the group
/// cache lookup to the frame the noise socket writes.
///
/// The harness is built once and reused across iterations, which is the whole
/// point: a real client resolves the group once and holds the `Arc` across
/// sends. Building an N-participant fixture inside the measured body is exactly
/// the error PR #1279 found and removed from the `wacore` sweep, where it was
/// the entirety of the apparent per-member growth. The same trap has a second
/// mouth here: a fixture whose participant list omits self makes
/// `ensure_self_in_group` deep-clone the metadata on every send, worth another
/// 45 instructions per member; a third is an unacknowledged companion session,
/// worth 11% of the absolute cost. Both are measured in `bench_support`.
///
/// Each iteration advances the sender-key chain by one, as a real repeat send
/// does; no iteration re-distributes to a member (they are all warm), and each
/// re-distributes to our own companion, which is the warm steady state WA Web's
/// `!isMeDevice` guard produces.
#[divan::bench(args = GROUP_SIZES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn warm_group_send(bencher: divan::Bencher, group_size: usize) {
    let harness = shared("warm_group_send", group_size);
    bencher.bench(|| harness.warm_send());
}

/// SKDM target resolution alone, on the memoized (steady-state) path.
///
/// Separated from `warm_group_send` because a send is dominated by one Ed25519
/// signature — 68% of it in `wacore`'s measurement — which would bury a
/// per-member term of a few hundred instructions. If the client's group send
/// grows with N, this is the most likely place, so it is measured without the
/// crypto on top.
#[divan::bench(args = GROUP_SIZES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn skdm_target_resolution_warm(bencher: divan::Bencher, group_size: usize) {
    let harness = shared("skdm_target_resolution_warm", group_size);
    bencher.bench(|| black_box(harness.resolve_skdm_targets()));
}

/// The same resolution with the warm memo invalidated first, so
/// `filter_skdm_targets` runs its one-hash-per-device scan.
///
/// This is the shape of the cost an external profile attributed to
/// `filter_skdm_targets`/`skdm_device_map`, and the difference against
/// `skdm_target_resolution_warm` is what the memo is worth per send. Measuring
/// both is what separates "the filter is expensive" from "the filter runs when
/// it should not" — only the second is a bug, and only a hit-rate observation
/// can tell them apart. Which one the steady state takes is not asserted here
/// (a benchmark measures how much an outcome costs, not how often it happens):
/// it is pinned by `skdm_warm_memo_hits_on_every_repeat_send` and, per term,
/// by `repeat_group_sends_hit_both_device_memos_on_every_send`. At group sizes
/// no unit test builds, read `GroupSendHarness::memo_stats` after a run of
/// `warm_send`.
#[divan::bench(args = GROUP_SIZES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn skdm_target_resolution_memo_cold(bencher: divan::Bencher, group_size: usize) {
    let harness = shared("skdm_target_resolution_memo_cold", group_size);
    bencher.bench(|| black_box(harness.resolve_skdm_targets_memo_cold()));
}

/// A signal-cache flush from the state a warm send leaves behind.
///
/// `Client::flush_signal_cache` was the largest single name in the external
/// profile's per-message hash-lookup table (42.8 calls at 512 members), which
/// would only make sense if it walked something proportional to the group. The
/// sweep answers whether the walk is over the whole cached set or only the
/// dirty part: the fixture's cached session set is N+2 entries wide at every
/// size, while what a warm send dirties is not.
#[divan::bench(args = GROUP_SIZES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn flush_signal_cache_steady(bencher: divan::Bencher, group_size: usize) {
    let harness = shared("flush_signal_cache_steady", group_size);
    bencher.bench(|| harness.flush_signal_cache());
}
