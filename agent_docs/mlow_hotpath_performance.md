# MLOW hot-path measurements

## Baseline

Base commit `6502b871e35664ffb80044ba7c6317a6427754e2`, verified against
`origin/main` before profiling. `CONTRIBUTING.md` is absent at this commit.

The existing `voip_benchmark` executable supplies every benchmark below.
Local tools are cargo-codspeed 5.0.1, codspeed-runner 5.0.2 and
Valgrind 3.26.0.codspeed7. The simulation build uses the repository's bench
profile and `voip-mlow,bench-internals`. Each local simulation invocation
selects exactly one fully qualified benchmark name.

```sh
cargo codspeed build -m simulation -m memory -p wacore \
  --features voip-mlow,bench-internals --bench voip_benchmark

MALLOC_ARENA_MAX=1 MALLOC_MMAP_THRESHOLD_=131072 \
MALLOC_TRIM_THRESHOLD_=131072 MALLOC_TOP_PAD_=131072 \
codspeed run --skip-upload --skip-setup -m simulation \
  --profile-folder "$PWD/target/mlow-base-encode" -- \
  target/codspeed/analysis/wacore/voip_benchmark \
  --exact wacore/benches/voip_benchmark.rs::mlow_encode
```

Create the profile directory before running. On CachyOS, automatic executor
setup is unsupported. The command above requires the CodSpeed Valgrind fork
on `PATH`. Its pinned Capstone dependency and Valgrind were built locally,
with `-std=gnu17` for Valgrind's compatibility with GCC 16 and glibc headers.
No project release-profile setting changed.

Two isolated encode simulations recorded 8,017,287 and 8,017,119 instructions,
a difference of 0.0021%. Raw profiles retain instruction counts and the `Ct`
and `Cl` cycle-estimation events. These are local measurements, not a
published CodSpeed PR comparison.

The baseline GitHub CodSpeed analysis is
[run 6aa3fd64220f2056a184c1b6](https://app.codspeed.io/oxidezap/whatsapp-rust/runs/compare/6aa363317b1d8863bea105e5..6aa3fd64220f2056a184c1b6).
Its check reports 790 untouched benchmarks and 12 skipped benchmarks.

## Initial native measurements

The first Divan pass ran before switching to isolated benchmark invocations.
Times are medians from that pass and have no cross-machine meaning.
Allocation calls exclude Divan's separately reported growth calls.

| Benchmark | Median | Allocation calls | Allocated bytes |
| --- | ---: | ---: | ---: |
| mlow_encode | 880.9 µs | 451 | 395.6 KB |
| mlow_encode_reused_output | 859.6 µs | 450 | 395.1 KB |
| mlow_decode | 43.11 µs | 51 | 25.92 KB |
| engine_outbound_frame | 909.9 µs | 540 | 547.8 KB |
| engine_inbound_packet | 56.81 µs | 53 | 26 KB |
| call_second_two_peers | 34.02 ms | 17,368 | 14.69 MB |
| codec_stages::analyze_frame | 890.9 µs | 449 | 395.1 KB |
| codec_stages::fft512_forward | 12.23 µs | 0 | 0 |
| codec_stages::fft576_roundtrip | 31.36 µs | 0 | 0 |
| codec_stages::perc_model_frame | 63.96 µs | 6 | 2.064 KB |
| codec_stages::lpc_front_end | 26.65 µs | 7 | 3.584 KB |
| codec_stages::celp_subframes_frame | 92.61 µs | 63 | 24.84 KB |
| codec_stages::pitch_search | 55.89 µs | 14 | 85.28 KB |
| codec_stages::lsf_quantize | 8.906 µs | 36 | 2.948 KB |
| codec_stages::entropy_encode | 2.364 µs | 0 | 0 |

## Allocation attribution

DHAT needs both debug information and unstripped output. The ordinary release
profile strips symbols, leaving no usable allocation stacks even if debug
information alone is enabled.

```sh
CARGO_PROFILE_RELEASE_DEBUG=1 CARGO_PROFILE_RELEASE_STRIP=false \
  cargo build -p wacore --release --example voip_profile \
  --features voip-mlow,dhat-heap
```

Running `voip_profile encode 100` and selecting only stacks containing
`voip_profile::hot_encode` records 46,853 blocks and 39,660,648 bytes across
100 packets. This excludes setup and the priming frame. DHAT counts
reallocations as blocks, unlike the separate Divan allocation/growth columns.
The resulting averages are 468.53 blocks and 396,606.48 bytes per 60 ms packet.

| Measured site | Count per packet | Bytes per packet |
| --- | ---: | ---: |
| Pitch C buffer | 3 | 61,056 |
| Pitch E buffer | 3 | 61,056 |
| Pitch H buffer | 3 | 56,064 |
| Pitch energy rows, both resolutions | 6 | 45,024 |
| Pitch extended energy rows | 6 | 10,668 |
| Perceptual Levinson corr_dbl | 24 | 4,704 |
| Perceptual Levinson c0 | 24 | 4,704 |
| Perceptual Levinson c1 | 24 | 4,704 |

The process-wide live heap includes shared codec tables. It is not a valid
per-encoder live-heap estimate.

Local CodSpeed Memory also recorded the isolated encode benchmark, producing
1,358 memory events. It warned that privilege elevation could not adjust
kernel memory tunables. Retain that warning when comparing local memory runs.

## Kept batches

1. Replace the three perceptual Levinson vectors with 33-element f64 arrays.
   Arithmetic and iteration order remain unchanged.
2. Keep CELP pulse output in its existing fixed storage and use explicit pulse
   lengths at both production consumers. Store candidate pulses in 320-element
   i32 arrays.
3. Write perceptual LPC responses into fixed caller storage with the internal
   `smpl_perc_ac2a_into`. Production uses it for both pitch and CELP weighting.
4. Add `MlowEncoder::encode_i16_into`, used by
   `CallEngine::encode_mlow_frame`. It normalizes into the existing encoder
   scratch and removes the engine's extra 960-element f32 buffer. This saves
   3,840 bytes of persistent call storage and one 3,840-byte staging write/read
   per non-silent packet. The exact-zero DTX branch still returns before encoding.

No arithmetic reassociation, precision change, dependency, unsafe code, codec
decision, packet timing or decoder API change is included.

## Rejected pitch experiments

Persistent stage1, downsample, C, E, H and energy buffers passed golden and
pitch fixtures, including a poison-buffer test. They reduced Divan encode
allocation bytes to 134.8 KB but increased local encode instructions from
8,017,287 to 8,348,887. Pitch-stage instructions increased from 502,593 to
632,501. Removing energy-buffer reuse still left 609,652 pitch instructions.
Both versions were reverted, including their experiment-only test.

The profile attributes substantial work to clearing the reused buffers. This
does not establish that every possible pitch scratch design regresses. A future
attempt needs a tighter write-before-read proof and isolated measurements.

## Combined local results

CPU columns below are raw instructions from separate local CodSpeed simulation
invocations. Memory columns are Divan allocated bytes, excluding separately
reported growth. These are different instruments and are labelled accordingly.

| Benchmark | Base Ir | Head Ir | Base allocation bytes | Head allocation bytes |
| --- | ---: | ---: | ---: | ---: |
| mlow_encode | 8,017,287 | 8,020,308 | 395.6 KB | 376.0 KB |
| mlow_encode_reused_output | 8,016,851 | 8,017,956 | 395.1 KB | 375.5 KB |
| engine_outbound_frame | 8,373,354 | 8,372,742 | 547.8 KB | 528.2 KB |
| mlow_decode | 324,532 | 324,563 | 25.92 KB | 25.92 KB |
| engine_inbound_packet | 395,743 | 397,838 | 26.0 KB | 26.0 KB |
| codec_stages::pitch_search | 502,593 | 500,507 | 85.28 KB | 85.28 KB |
| codec_stages::perc_model_frame | 791,015 | 791,009 | 2.064 KB | 2.064 KB |
| codec_stages::celp_subframes_frame | 675,307 | 672,220 | 24.84 KB | 21.56 KB |
| call_second_two_peers | 284,044,856 | 283,961,974 | 14.69 MB | 14.03 MB |

The FFT and entropy instruction totals are identical. LPC is 226,798 to
226,658, LSF is unchanged at 59,907, and analysis is 8,019,831 to 8,018,253.
The new i16 reused-output row records 8,018,834 instructions. Its quantized
input differs from the f32 row, so it is not a direct API speed comparison.

Cycle-estimation events also matter. Encode Ct is 560,717,713 to 562,982,939
and Cl is 1,948,318,127 to 1,976,744,162. Outbound Ct is 598,722,215 to
601,744,466 and Cl is 2,017,557,983 to 2,043,936,499. Instruction counts alone
would hide those increases. No combined CPU improvement is claimed from these
local measurements.

Paired native runs pinned to CPU 2 gave encode medians of 305.2 to 305.5 µs,
reused-output 304.1 to 306.1 µs, outbound 323.7 to 312.0 µs and two-peer
call-second 11.30 to 11.29 ms. Other rows showed substantial time variation even
when their code was unchanged. Treat the native data as a regression check,
not evidence for a decoder speedup or a precise outbound percentage.

| Allocation measure | Base | Head |
| --- | ---: | ---: |
| Divan encode allocation calls | 451 | 338 |
| Divan reused-output allocation calls | 450 | 337 |
| Divan outbound allocation calls | 540 | 423 |
| Divan CELP-stage allocation calls | 63 | 42 |
| Divan two-peer call-second allocation calls | 17,368 | 13,521 |
| DHAT blocks per packet, including reallocations | 468.53 | 351.88 |
| DHAT bytes per packet | 396,606.48 | 376,971.76 |
| Warm encoder live heap, shared tables excluded | 157,725 B | 157,725 B |

DHAT uses the same 100-packet stream and selects only `hot_encode` stacks.
`voip_profile encoder-live`, with `dhat-heap`, warms shared tables with a
discarded encoder, then measures a second encoder after eight packets. The
same profiler-only driver change was applied to the detached baseline worktree.

The three Levinson allocation sites each fall from 24 calls and 4,704 bytes
per packet to zero. Their sum is 72 calls and 14,112 bytes per packet.
The perceptual response vector falls from 24 calls and 2,352 bytes per packet
to zero. Pitch C/E/H remain the largest allocated-byte family.

## Correctness and portability

- 2,397 wacore library tests passed, with three ignored tests reported separately.
- The explicitly invoked 2,048-packet encode/decode stream passed.
- New i16 equivalence coverage exercises every i16 value, reused oversized output,
  invalid lengths between valid frames and reset boundaries.
- Golden checksums and C/Go/wasm fixture expectations were not changed.
- Targeted MLOW Clippy and workspace all-targets Clippy passed.
- wasm32 builds with `--no-default-features --features voip-mlow,js`.
  Omitting the existing `js` feature fails in getrandom before reaching the codec.
- The symbolized release DHAT driver text grows from 1,140,053 to 1,140,885 bytes,
  an increase of 832 bytes. This is an executable measurement, not codec-only size.
- New local arrays hold 792 bytes of f64 Levinson storage, 1,280 bytes per candidate
  pulse array and 512 bytes per response matrix. Native stack-frame and wasm stack
  high-water measurements remain outstanding.

## Remaining work

The largest remaining byte costs are pitch scratch, followed by CELP subframe
temporaries. Decoder work remains much smaller than encoder work. ACB basis
scratch, correlation storage, pulse entropy buffers, playout allocation and
jitter operations have not been changed. The next CPU investigation should
start from the retained FFT/CELP/pitch profiles rather than propose another FFT
algorithm without evidence.

Publication and the final CodSpeed PR comparison are pending. Local results do
not substitute for the repository's CI simulation and memory report.
