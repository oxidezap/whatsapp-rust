//! Client-level receive: one inbound message from the decoded stanza to the
//! dispatched event.
//!
//! `wacore`'s `send_receive_benchmark` covers the pure decrypt, and both DM and
//! group receives there are within a few microseconds of their crypto floor.
//! Everything *around* the decrypt — classification, the signal cache
//! checkout, the chat-lane and dedup bookkeeping, plaintext handling, the
//! event bus, the delivery receipt — lives in the `whatsapp-rust` crate, and
//! this target is what puts a number on it.
//!
//! The direct receive and `_burst` cases enter at `handle_incoming_message`
//! and exclude the queue hop. The `worker_*` cases include enqueue and the
//! production chat-lane worker; `worker_lanes` also includes lane shutdown.
//!
//! These fixtures exclude:
//! - **The SQLite backend.** The fixture stores through `InMemoryBackend`.
//! - **First contact.** Session and sender key are established in setup.
//! - **The socket write.** The delivery receipt is marshalled, noise-encrypted
//!   and framed; the transport drops the frame.

use divan::black_box;
use std::sync::OnceLock;
use whatsapp_rust::bench_support::ReceiveHarness;

fn main() {
    divan::main();
}

// Few, large samples: the fixture's flush worker fires on its own 25 ms clock
// and a coalesced flush that lands mid-sample is amortised over a long sample
// rather than moving a short one.
#[allow(dead_code)]
const SAMPLE_COUNT: u32 = 20;
#[allow(dead_code)]
const SAMPLE_SIZE: u32 = 50;

/// One harness for both benches, so the second does not pay a second fixture.
/// The peer's chains advance per built stanza and each bench receives what it
/// built, in order, so sharing does not cross their streams.
fn harness() -> &'static ReceiveHarness {
    static HARNESS: OnceLock<ReceiveHarness> = OnceLock::new();
    HARNESS.get_or_init(ReceiveHarness::new)
}

/// A 1:1 text message on an acknowledged session.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn dm_receive(bencher: divan::Bencher) {
    let harness = harness();
    let before = harness.messages_delivered();
    let mut received = 0u64;
    bencher
        .with_inputs(|| harness.dm_stanza())
        .bench_local_values(|node| {
            received += 1;
            harness.receive(black_box(node));
        });
    // A decrypt failure is fast and silent; it must not pass for a receive.
    assert_eq!(harness.messages_delivered() - before, received);
}

/// A group text message under an installed sender key.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn group_receive(bencher: divan::Bencher) {
    let harness = harness();
    let before = harness.messages_delivered();
    let mut received = 0u64;
    bencher
        .with_inputs(|| harness.group_stanza())
        .bench_local_values(|node| {
            received += 1;
            harness.receive(black_box(node));
        });
    assert_eq!(harness.messages_delivered() - before, received);
}

const BURST_SIZE: usize = 50;

/// Limit 2 (Harness control): A 1:1 text message received in a burst under a single
/// runtime entry (`block_on`), matching the exact decrypt, event, and receipt flush work
/// while isolating the per-message processing cost from the `block_on` future passing artifact.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = 1)]
fn dm_receive_burst(bencher: divan::Bencher) {
    let harness = harness();
    let before = harness.messages_delivered();
    let mut round_delivered = 0u64;
    bencher
        .counter(divan::counter::ItemsCount::new(BURST_SIZE as u64))
        .with_inputs(|| {
            (0..BURST_SIZE)
                .map(|_| harness.dm_stanza())
                .collect::<Vec<_>>()
        })
        .bench_local_values(|batch| {
            round_delivered += batch.len() as u64;
            harness.receive_burst(black_box(&batch));
        });
    assert_eq!(harness.messages_delivered() - before, round_delivered);
    assert!(round_delivered > 0);
}

/// Limit 2 (Harness control): A group text message received in a burst under a single
/// runtime entry (`block_on`).
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = 1)]
fn group_receive_burst(bencher: divan::Bencher) {
    let harness = harness();
    let before = harness.messages_delivered();
    let mut round_delivered = 0u64;
    bencher
        .counter(divan::counter::ItemsCount::new(BURST_SIZE as u64))
        .with_inputs(|| {
            (0..BURST_SIZE)
                .map(|_| harness.group_stanza())
                .collect::<Vec<_>>()
        })
        .bench_local_values(|batch| {
            round_delivered += batch.len() as u64;
            harness.receive_burst(black_box(&batch));
        });
    assert_eq!(harness.messages_delivered() - before, round_delivered);
    assert!(round_delivered > 0);
}

/// Multi-lane harness initialized with 256 distinct groups and installed sender keys.
fn multilane_harness() -> &'static whatsapp_rust::bench_support::MultiLaneReceiveHarness {
    static HARNESS: OnceLock<whatsapp_rust::bench_support::MultiLaneReceiveHarness> =
        OnceLock::new();
    HARNESS.get_or_init(|| whatsapp_rust::bench_support::MultiLaneReceiveHarness::new(256))
}

const LANE_COUNTS: &[usize] = &[1, 32, 256];
const WORKER_BURST_SIZE: usize = 256;
#[allow(dead_code)]
const WORKER_SAMPLE_COUNT: u32 = 10;

/// Limit 3 (Production worker): A single warm chat lane processing bursts through
/// `MessageHandler::handle_inline` into its worker task without closing the worker
/// between samples.
#[divan::bench(sample_count = WORKER_SAMPLE_COUNT, sample_size = 1)]
fn worker_hot_burst_warm(bencher: divan::Bencher) {
    let harness = harness();
    let before = harness.messages_delivered();
    let mut round_delivered = 0u64;
    bencher
        .counter(divan::counter::ItemsCount::new(BURST_SIZE as u64))
        .with_inputs(|| {
            (0..BURST_SIZE)
                .map(|_| harness.group_stanza())
                .collect::<Vec<_>>()
        })
        .bench_local_values(|batch| {
            round_delivered += batch.len() as u64;
            harness.enqueue_and_drain(black_box(&batch));
        });
    assert_eq!(harness.messages_delivered() - before, round_delivered);
    assert!(round_delivered > 0);
    harness.close_lanes();
}

/// Limit 3 (Production worker): Production chat lane worker pipeline across 1, 32,
/// and 256 active lanes with valid sender key ratchets, fixed total message count (256),
/// and closing all worker tasks per round to verify task lifecycle and memory reclamation.
#[divan::bench(args = LANE_COUNTS, sample_count = WORKER_SAMPLE_COUNT, sample_size = 1)]
fn worker_lanes(bencher: divan::Bencher, lanes: usize) {
    let harness = multilane_harness();
    let before = harness.messages_delivered();
    let mut round_delivered = 0u64;
    bencher
        .counter(divan::counter::ItemsCount::new(WORKER_BURST_SIZE as u64))
        .with_inputs(|| harness.generate_burst(lanes, WORKER_BURST_SIZE))
        .bench_local_values(|batch| {
            round_delivered += batch.len() as u64;
            harness.enqueue_and_drain(black_box(&batch));
            harness.close_lanes();
        });
    assert_eq!(harness.messages_delivered() - before, round_delivered);
    assert_eq!(harness.active_lanes(), 0);
}
