//! Client-level inbound non-message stanzas: `<receipt>`, `<presence>`,
//! `<notification>` and `<ack>`, from the decoded stanza to the dispatched
//! event.
//!
//! `client_receive` covers the `<message>` path, where a Signal decrypt
//! dominates everything around it. These four are the rest of the read loop —
//! no crypto, so what they measure *is* the machinery: classification, the
//! handler's boxed future, the parse, and the event bus. On an offline drain
//! they are also the bulk of the queue.
//!
//! **The metric to read here is bytes and allocations per stanza, not wall
//! time.** Every row's body is a few hundred nanoseconds of branch-heavy work
//! against a fixture that shares a runtime with an ack worker, so timings move
//! more between runs than any change worth making moves them; the allocation
//! columns (`divan::AllocProfiler` is wired in below) are exact. The regression
//! this target exists to catch is a per-stanza heap block growing back — an
//! `.await` on an unboxed handler arm re-inflates the notification future the
//! way it was before PR "perf(handlers)".
//!
//! Every stanza runs twice: once with nothing subscribed to its event kind and
//! once with a subscriber. That is not redundancy — the subscriber is what
//! decides how much of the pipeline runs. `has_handler_for` gates the receipt
//! parse, steers `processes_inline`, and is what makes an `Arc<Event>` exist at
//! all.
//!
//! What it does not cover, stated once so no number here is over-read:
//!
//! - **The read loop's task spawn.** Stanzas enter at `process_node`, which is
//!   what the spawned task calls; the spawn itself is not measured.
//! - **The socket write.** The transport `<ack>` a `<receipt>` or
//!   `<notification>` owes is marshalled and noise-encrypted by a worker; the
//!   sink transport drops the frame.
//! - **Stanza construction.** Each row builds its stanza once, before
//!   sampling, and re-submits it. Nothing on these paths dedupes, so a repeat
//!   is the same work as a fresh one.

use divan::black_box;
use std::sync::Arc;
use std::sync::OnceLock;
use whatsapp_rust::bench_support::ReceiveHarness;

fn main() {
    divan::main();
}

/// Byte and allocation counts per row. The point of the target: see the module
/// header.
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

/// Few, large samples, matching `client_receive`: the fixture's flush worker
/// fires on its own 25 ms clock and a coalesced flush that lands mid-sample is
/// amortised over a long sample rather than moving a short one.
const SAMPLE_COUNT: u32 = 20;
const SAMPLE_SIZE: u32 = 50;

/// A reconnect drains its offline queue in one go. 500 is the size at which the
/// per-item cost of a drain is clearly separated from the fixed cost of
/// entering it.
const BURST: usize = 500;

/// One harness for every row, so no row pays for a second fixture. Unlike the
/// message rows these stanzas carry no ratchet state, so sharing cannot cross
/// their streams.
fn harness() -> &'static ReceiveHarness {
    static HARNESS: OnceLock<ReceiveHarness> = OnceLock::new();
    HARNESS.get_or_init(ReceiveHarness::new)
}

/// Submit one stanza per iteration, asserting afterwards that the subscriber
/// saw exactly the events it should have: `expected_events` per iteration, so a
/// stanza silently dropped by a parse failure (which is fast) cannot pass for
/// a processed one.
fn bench_one(
    bencher: divan::Bencher,
    node: Arc<wacore_binary::OwnedNodeRef>,
    subscribed: bool,
    expected_events: u64,
) {
    let harness = harness();
    let subscription = subscribed.then(|| harness.subscribe_stanza_events());
    let before = harness.stanza_events();
    let mut submitted = 0u64;
    bencher.bench_local(|| {
        submitted += 1;
        harness.process_nowait(black_box(Arc::clone(&node)));
    });
    let delivered = harness.stanza_events() - before;
    // Dropping it before the flush keeps the assertion below about what this
    // row submitted.
    drop(subscription);
    harness.flush();
    assert_eq!(delivered, submitted * expected_events);
}

/// Same, for the drain shape: `BURST` stanzas under one `block_on`.
fn bench_burst(
    bencher: divan::Bencher,
    node: Arc<wacore_binary::OwnedNodeRef>,
    expected_events: u64,
) {
    let harness = harness();
    let burst: Vec<_> = std::iter::repeat_n(node, BURST).collect();
    let subscription = harness.subscribe_stanza_events();
    let before = harness.stanza_events();
    let mut submitted = 0u64;
    bencher.bench_local(|| {
        submitted += BURST as u64;
        harness.process_burst(black_box(&burst));
    });
    let delivered = harness.stanza_events() - before;
    drop(subscription);
    harness.flush();
    assert_eq!(delivered, submitted * expected_events);
}

/// A delivery `<receipt>` nobody is listening for: the read-loop shape, where
/// the whole point is that the parse stops at the subscriber gate.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn receipt(bencher: divan::Bencher) {
    bench_one(bencher, harness().receipt_stanza(), false, 0);
}

#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn receipt_subscribed(bencher: divan::Bencher) {
    bench_one(bencher, harness().receipt_stanza(), true, 1);
}

#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn presence(bencher: divan::Bencher) {
    bench_one(bencher, harness().presence_stanza(), false, 0);
}

#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn presence_subscribed(bencher: divan::Bencher) {
    bench_one(bencher, harness().presence_stanza(), true, 1);
}

/// The row the boxed-arm regression shows up in: the notification handler's
/// future is sized for whichever arm the compiler keeps inline, whatever type
/// this stanza actually carries.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn notification(bencher: divan::Bencher) {
    bench_one(bencher, harness().notification_stanza(), false, 0);
}

#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn notification_subscribed(bencher: divan::Bencher) {
    bench_one(bencher, harness().notification_stanza(), true, 1);
}

/// A server `<ack>` no waiter is parked on — the shape every fire-and-forget
/// send draws back, and the cheapest stanza the client handles.
#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn ack(bencher: divan::Bencher) {
    bench_one(bencher, harness().ack_stanza(), false, 0);
}

#[divan::bench(sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn ack_subscribed(bencher: divan::Bencher) {
    bench_one(bencher, harness().ack_stanza(), true, 1);
}

// The drain rows all run subscribed: a reconnect that drains a queue into a
// consumer is the case where the per-item cost is actually paid, and the
// no-subscriber shape is already covered per stanza above. Divide every column
// by `BURST` for the per-item figure.
#[divan::bench(sample_count = 10, sample_size = 1)]
fn burst_receipt(bencher: divan::Bencher) {
    bench_burst(bencher, harness().receipt_stanza(), 1);
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn burst_presence(bencher: divan::Bencher) {
    bench_burst(bencher, harness().presence_stanza(), 1);
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn burst_notification(bencher: divan::Bencher) {
    bench_burst(bencher, harness().notification_stanza(), 1);
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn burst_ack(bencher: divan::Bencher) {
    bench_burst(bencher, harness().ack_stanza(), 1);
}
