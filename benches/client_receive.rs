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
//! What it does not cover, stated once so no number here is over-read:
//!
//! - **The chat lane.** A stanza enters at `handle_incoming_message`, which
//!   is what the lane worker awaits per message; the queue hop itself is not
//!   measured.
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
const SAMPLE_COUNT: u32 = 20;
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
