//! Minimal MLow codec hot-loop driver for an unbiased profiler pass (callgrind for CPU + malloc
//! attribution). Not a benchmark -- no timing, no sampling: it just runs `encode` or `decode` N
//! times over a small stream so an external profiler can attribute instructions and allocations to
//! the real per-stage functions.
//!
//!   cargo build -p wacore --release --example voip_profile --features voip
//!   valgrind --tool=callgrind --collect-atstart=no --toggle-collect='*hot_encode*' \
//!     --callgrind-out-file=cg.enc target/release/examples/voip_profile encode 30
//!   callgrind_annotate cg.enc | head -60

use std::hint::black_box;
use wacore::voip::{MlowDecoder, MlowEncoder};

// With `--features dhat-heap`, dhat is the global allocator and writes dhat-heap.json (per-call-site
// allocation counts + bytes) on exit. Without it, the example runs under the system allocator so an
// external CPU profiler (callgrind) sees no profiler overhead.
#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const SAMPLES: usize = 960;

fn tone(phase: usize) -> Vec<f32> {
    (0..SAMPLES)
        .map(|i| 0.3 * (((i + phase) as f32) * 0.07).sin())
        .collect()
}

#[inline(never)]
fn hot_encode(enc: &mut MlowEncoder, frames: &[Vec<f32>], n: usize) {
    for k in 0..n {
        black_box(enc.encode(black_box(&frames[k % frames.len()])).unwrap());
    }
}

#[inline(never)]
fn hot_decode(dec: &mut MlowDecoder, packets: &[Vec<u8>], n: usize) {
    for k in 0..n {
        black_box(dec.decode(black_box(&packets[k % packets.len()])));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("encode");
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    // A handful of distinct frames so the encoder/decoder sees a stream, not one repeated frame.
    let frames: Vec<Vec<f32>> = (0..8).map(|i| tone(i * SAMPLES)).collect();

    match mode {
        "encode" => {
            let mut enc = MlowEncoder::new();
            let _ = enc.encode(&frames[0]); // prime past the first-frame path
            hot_encode(&mut enc, &frames, n);
        }
        "decode" => {
            let mut enc = MlowEncoder::new();
            let _ = enc.encode(&frames[0]);
            let packets: Vec<Vec<u8>> = frames.iter().map(|f| enc.encode(f).unwrap()).collect();
            let mut dec = MlowDecoder::new();
            let _ = dec.decode(&packets[0]); // prime
            hot_decode(&mut dec, &packets, n);
        }
        other => eprintln!("usage: voip_profile <encode|decode> <n>; got {other:?}"),
    }
}
