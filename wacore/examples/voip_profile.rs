//! Minimal MLow codec hot-loop driver for an unbiased profiler pass (callgrind for CPU + malloc
//! attribution). Not a benchmark -- no timing, no sampling: it runs `encode`, `encode-into`, or `decode` N
//! times over a small stream so an external profiler can attribute instructions and allocations to
//! the real per-stage functions.
//! See `agent_docs/mlow_hotpath_performance.md` for the `encoder-live` heap measurement mode.
//!
//!   cargo build -p wacore --release --example voip_profile --features voip-mlow
//!   valgrind --tool=callgrind --collect-atstart=no --toggle-collect='*hot_encode*' \
//!     --callgrind-out-file=cg.enc target/release/examples/voip_profile encode 30
//!   callgrind_annotate cg.enc | head -60

// A CLI driver: the usage line is its output, not a diagnostic.
#![allow(clippy::print_stderr)]

use std::hint::black_box;
use wacore::voip::mlow::MlowError;
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
fn hot_encode(enc: &mut MlowEncoder, frames: &[Vec<f32>], n: usize) -> Result<(), MlowError> {
    for k in 0..n {
        black_box(enc.encode(black_box(&frames[k % frames.len()]))?);
    }
    Ok(())
}

#[inline(never)]
fn hot_encode_into(enc: &mut MlowEncoder, frames: &[Vec<f32>], n: usize) -> Result<(), MlowError> {
    let mut output = Vec::with_capacity(2048);
    for k in 0..n {
        enc.encode_into(black_box(&frames[k % frames.len()]), &mut output)?;
        black_box(output.as_slice());
    }
    Ok(())
}

#[inline(never)]
fn hot_decode(dec: &mut MlowDecoder, packets: &[Vec<u8>], n: usize) {
    for k in 0..n {
        black_box(dec.decode(black_box(&packets[k % packets.len()])));
    }
}

/// Keep setup outside the hot-loop symbols so profiler filters can isolate steady-state processing.
fn main() -> Result<(), MlowError> {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("encode");
    let n: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(30);
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();
    // A handful of distinct frames so the encoder/decoder sees a stream, not one repeated frame.
    let frames: Vec<Vec<f32>> = (0..8).map(|i| tone(i * SAMPLES)).collect();

    match mode {
        #[cfg(feature = "dhat-heap")]
        "encoder-live" | "encoder-live-i16" => {
            let integer_frames: Vec<Vec<i16>> = frames
                .iter()
                .map(|f| f.iter().map(|s| (s * 32768.0) as i16).collect())
                .collect();
            let mut warm = MlowEncoder::new();
            let _ = warm.encode(&frames[0])?;
            drop(warm);
            let before = dhat::HeapStats::get().curr_bytes;
            let mut enc = MlowEncoder::new();
            for (frame, integers) in frames.iter().zip(&integer_frames) {
                if mode == "encoder-live-i16" {
                    let mut output = Vec::new();
                    enc.encode_i16_into(integers, &mut output)?;
                } else {
                    let _ = enc.encode(frame)?;
                }
            }
            let live = dhat::HeapStats::get().curr_bytes - before;
            eprintln!("encoder live heap: {live} bytes");
            black_box(&enc);
        }
        "encode" => {
            let mut enc = MlowEncoder::new();
            let _ = enc.encode(&frames[0])?; // prime past the first-frame path
            hot_encode(&mut enc, &frames, n)?;
        }
        "encode-into" => {
            let mut enc = MlowEncoder::new();
            let _ = enc.encode(&frames[0])?;
            hot_encode_into(&mut enc, &frames, n)?;
        }
        "decode" => {
            let mut enc = MlowEncoder::new();
            let _ = enc.encode(&frames[0])?;
            let packets: Vec<Vec<u8>> = frames
                .iter()
                .map(|f| enc.encode(f))
                .collect::<Result<_, _>>()?;
            let mut dec = MlowDecoder::new();
            let _ = dec.decode(&packets[0]); // prime
            hot_decode(&mut dec, &packets, n);
        }
        other => eprintln!(
            "usage: voip_profile <encode|encode-into|decode> <n>, or encoder-live[-i16] with dhat-heap; got {other:?}"
        ),
    }
    Ok(())
}
