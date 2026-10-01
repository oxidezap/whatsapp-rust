//! Manual receive allocation diagnostic, separate from walltime benchmarks.
//!
//! Run `cargo run -p whatsapp-rust --profile bench --features bench-harness
//! --example receive_footprint`. Input encryption and fixture setup are outside
//! the profiler. A fresh profiler/harness per repetition counts only allocations
//! made during receive, not the pre-existing Signal/session/input graphs.
//! `drained` is retained requested heap with workers still alive; `closed` is
//! after shutdown. `peak_after_warm` is the cumulative high-water mark since cold
//! enqueue, not an isolated warm-phase peak. These are not allocator RSS or
//! connected-idle measurements.
//!
//! Plaintext controls isolate early-Arc/cold-future costs without Signal crypto.
//! Empty SKDM/secret carriers exercise allocation branches, not key installation
//! or successful secret decryption. Their printed harness future size separates
//! runtime-entry copies/possible boxing from per-message application costs.

// This diagnostic's output is its result, not application logging.
#![allow(clippy::print_stdout)]

use std::sync::Arc;
use wacore::types::message::{MessageInfo, MessageSource};
use waproto::whatsapp as wa;
use whatsapp_rust::bench_support::{MultiLaneReceiveHarness, ReceiveHarness};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const REPETITIONS: usize = 5;
const DM_BURST: usize = 50;

fn main() {
    for lanes in [1, 32, 256] {
        for repetition in 0..REPETITIONS {
            let harness = MultiLaneReceiveHarness::new(256);
            let cold = harness.generate_burst(lanes, 256);
            let warm = harness.generate_burst(lanes, 256);
            let before = harness.messages_delivered();
            let profiler = dhat::Profiler::builder().testing().build();
            let target = harness.enqueue(&cold);
            let enqueued = dhat::HeapStats::get();
            harness.drain(target);
            let drained = dhat::HeapStats::get();
            harness.enqueue_and_drain(&warm);
            let warmed = dhat::HeapStats::get();
            harness.close_lanes();
            let closed = dhat::HeapStats::get();
            drop(profiler);
            assert_eq!(harness.messages_delivered() - before, 512);
            assert_eq!(harness.active_lanes(), 0);
            println!(
                "lanes={lanes} repetition={repetition} enqueued={} cold_peak={} drained={} peak_after_warm={} warmed={} closed={} total={} allocations={}",
                enqueued.curr_bytes,
                drained.max_bytes,
                drained.curr_bytes,
                warmed.max_bytes,
                warmed.curr_bytes,
                closed.curr_bytes,
                closed.total_bytes,
                closed.total_blocks,
            );
        }
    }
    for repetition in 0..REPETITIONS {
        let harness = ReceiveHarness::new();
        let batch: Vec<_> = (0..DM_BURST).map(|_| harness.dm_stanza()).collect();
        let before = harness.messages_delivered();
        let profiler = dhat::Profiler::builder().testing().build();
        harness.receive_burst(&batch);
        let stats = dhat::HeapStats::get();
        drop(profiler);
        assert_eq!(harness.messages_delivered() - before, DM_BURST as u64);
        println!(
            "dm_burst repetition={repetition} peak={} retained={} total={} allocations={}",
            stats.max_bytes, stats.curr_bytes, stats.total_bytes, stats.total_blocks,
        );
    }
    for case in ["text", "skdm_only", "suppressed", "malformed_secret"] {
        for repetition in 0..REPETITIONS {
            plaintext_control(case, repetition);
        }
    }
}

fn info(id: String) -> Arc<MessageInfo> {
    let peer = "15550000101@s.whatsapp.net"
        .parse()
        .expect("synthetic peer jid");
    Arc::new(MessageInfo {
        id: id.into(),
        source: MessageSource {
            chat: peer,
            sender: "15550000101@s.whatsapp.net"
                .parse()
                .expect("synthetic sender jid"),
            ..Default::default()
        },
        ..Default::default()
    })
}

fn plaintext_control(case: &str, repetition: usize) {
    let harness = ReceiveHarness::new();
    let text = wa::Message {
        conversation: Some("control".into()),
        ..Default::default()
    };
    let seed = info(format!("SEED-{case}-{repetition}"));
    // Warm receipt/dispatch infrastructure outside every control's profiler.
    harness.plaintext_burst(vec![(
        wacore::messages::MessageUtils::encode_and_pad(&text),
        Arc::clone(&seed),
    )]);
    let mut message = wa::Message::default();
    match case {
        "text" | "suppressed" => message = text,
        "skdm_only" => {
            message
                .sender_key_distribution_message
                .get_or_insert_default();
        }
        "malformed_secret" => {
            message.secret_encrypted_message.get_or_insert_default();
        }
        _ => unreachable!("fixed diagnostic cases"),
    }
    let payloads = (0..DM_BURST)
        .map(|index| {
            let info = if case == "suppressed" {
                Arc::clone(&seed)
            } else {
                info(format!("CONTROL-{case}-{repetition}-{index}"))
            };
            (
                wacore::messages::MessageUtils::encode_and_pad(&message),
                info,
            )
        })
        .collect();
    let harness_future = harness.plaintext_burst_future_bytes();
    let before = harness.messages_delivered();
    let profiler = dhat::Profiler::builder().testing().build();
    let outcomes = harness.plaintext_burst(payloads);
    let stats = dhat::HeapStats::get();
    drop(profiler);
    if case == "skdm_only" {
        assert_eq!(outcomes, (0, DM_BURST));
    } else {
        assert_eq!(outcomes, (DM_BURST, 0));
    }
    let expected = if case == "skdm_only" || case == "suppressed" {
        0
    } else {
        DM_BURST as u64
    };
    assert_eq!(harness.messages_delivered() - before, expected);
    println!(
        "plaintext={case} repetition={repetition} peak={} retained={} total={} allocations={} harness_future={harness_future}",
        stats.max_bytes, stats.curr_bytes, stats.total_bytes, stats.total_blocks,
    );
}
