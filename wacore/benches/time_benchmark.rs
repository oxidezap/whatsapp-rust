//! Timestamp paths used by incoming messages, status expiry, clocks and logs.

use divan::{Bencher, black_box, counter::ItemsCount};
use std::fmt::Write;
use wacore::time::{ClockReading, Instant, from_millis, from_secs, now_utc};
use wacore::types::message::MessageInfo;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

const BATCH: usize = 256;
const MILLIS: [i64; 8] = [-1, 0, 1, 999, 1000, 1_700_000_000_123, i64::MIN, i64::MAX];
const MESSAGE_MILLIS: [i64; 4] = [
    1_794_888_000_123,
    1_794_888_001_456,
    1_794_974_400_789,
    1_795_060_800_000,
];

#[divan::bench]
fn decode_message_timestamps(bencher: Bencher) {
    decode_timestamps(bencher, &MESSAGE_MILLIS);
}

#[divan::bench]
fn decode_timestamp_boundaries(bencher: Bencher) {
    decode_timestamps(bencher, &MILLIS);
}

fn decode_timestamps(bencher: Bencher, inputs: &[i64]) {
    bencher
        .counter(ItemsCount::new(BATCH * inputs.len() * 2))
        .bench(|| {
            for _ in 0..BATCH {
                for &millis in black_box(inputs) {
                    black_box(from_millis(black_box(millis)));
                    black_box(from_secs(black_box(millis / 1000)));
                }
            }
        });
}

#[divan::bench]
fn capture_wall_clock(bencher: Bencher) {
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        for _ in 0..BATCH {
            black_box(now_utc());
        }
    });
}

#[divan::bench]
fn validate_clock_reading(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(BATCH * MILLIS.len()))
        .bench(|| {
            for _ in 0..BATCH {
                for &millis in black_box(&MILLIS) {
                    let _ = black_box(ClockReading::new(black_box(millis), Instant::ZERO));
                }
            }
        });
}

#[divan::bench]
fn status_expiry(bencher: Bencher) {
    let mut info = MessageInfo {
        timestamp: from_millis(1_700_000_000_123).unwrap(),
        ..MessageInfo::default()
    };
    info.source.chat = "status@broadcast".parse().unwrap();
    let clocks = [1_700_000_000_122, 1_700_086_400_123, 1_700_086_400_124];
    bencher
        .counter(ItemsCount::new(BATCH * clocks.len()))
        .bench(|| {
            for _ in 0..BATCH {
                for &millis in black_box(&clocks) {
                    black_box(
                        black_box(&info)
                            .is_expired_status_with(|| from_millis(black_box(millis)).unwrap()),
                    );
                }
            }
        });
}

#[divan::bench]
fn serialize_message_info(bencher: Bencher) {
    let info = MessageInfo {
        timestamp: from_millis(1_700_000_000_123).unwrap(),
        ..MessageInfo::default()
    };
    bencher
        .with_inputs(|| Vec::with_capacity(4096))
        .bench_refs(|out| {
            out.clear();
            serde_json::to_writer(&mut *out, black_box(&info)).unwrap();
            black_box(out.as_slice());
        });
}

#[divan::bench]
fn format_log_clock(bencher: Bencher) {
    let at = from_millis(1_700_000_000_123).unwrap();
    bencher
        .with_inputs(|| String::with_capacity(16))
        .bench_refs(|out| {
            out.clear();
            write!(out, "{}", black_box(at).display_hms()).unwrap();
            black_box(out.as_bytes());
        });
}
