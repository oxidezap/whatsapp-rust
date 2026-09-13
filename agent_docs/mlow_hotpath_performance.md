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

## Combined local results before the LPC and top-K additions

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

## Incremental LPC and CELP measurements

The following evidence was supplied by the calling agent from the detached
experiment based on `77d8ae91`. The two tested code diffs were copied exactly.
The main baseline remains `6502b871e35664ffb80044ba7c6317a6427754e2`.
Earlier tables retain the four-batch results and are not measurements of this
new head.

Immutable LPC windows now use `OnceLock<LpcWindows>` with 264-, 64- and
32-element f32 arrays generated with the original formulas on each target.
After initialization this removes six allocations and 952 trigonometric
evaluations per 60 ms packet. LPC Levinson uses two 17-element f64 arrays,
removing six more allocations per packet without changing arithmetic.
CELP top-K uses the caller's bounded index array for insertion selection,
with an argmax path for k=1. NaNs retain the legacy scan because its
first-unselected NaN behavior cannot be expressed by a comparator. Strict
comparisons, lowest-index ties, repeated index zero after candidate exhaustion
and the untouched destination tail are preserved.

The caller read a real-call CPU profile over seconds 15 through 33. It contains
340,285 µs in the engine, 303,519 µs in encode, 19,901 µs in decode,
15,659 µs in CELP top-K and 31,361 µs in FFT. The bridge uses `6502b871`
with different code generation from release, so these numbers support
attribution only, not absolute production CPU estimates.

Local CodSpeed profiles ran one row per invocation without upload. The caller
retained them under `/home/jlucaso/projects/whatsapp-rust/target/` with prefixes
`lpc-windows-*`, `lpc-levinson-*` and `celp-topk-*`.

| Encode implementation | Ir | Ct | Cl |
| --- | ---: | ---: | ---: |
| Prior PR | 8,020,308 | 562,982,939 | 1,976,744,162 |
| Plus cached LPC windows | 7,980,057 | 560,486,094 | 1,966,750,461 |
| Plus LPC Levinson arrays | 7,978,828 | 560,401,404 | 1,966,478,862 |
| Plus CELP top-K | 7,921,074 | 559,414,269 | 1,964,267,338 |

Outbound instructions fall incrementally from 8,372,742 to 8,279,386.
CELP-stage instructions fall from 672,220 to 652,555. Against main, aggregate
encode instructions are 8,017,287 to 7,921,074, outbound 8,373,354 to
8,279,386 and CELP-stage 675,307 to 652,555. Aggregate encode Ct falls from
560,717,713 to 559,414,269, while Cl remains above main's 1,948,318,127.

| CPU 2 pinned native median | Prior PR | With additions |
| --- | ---: | ---: |
| Encode | 304.8 µs | 300.0 µs |
| Reused-output encode | 304.9 µs | 298.2 µs |
| Two-peer call-second | 11.28 ms | 11.03 ms |
| Outbound | 311.2 µs | 313.9 µs |

Native outbound is noisy and flat. These measurements do not show a native
outbound improvement.

Divan encode falls from 338 allocation calls and 376 KB to 326 calls and
371.4 KB. Outbound falls from 423 calls to 411, and LPC-stage from seven to
three. Against main, aggregate encode calls fall from 451 to 326 and outbound
from 540 to 411. The earlier DHAT and warm-heap measurements have not been
remeasured for these additions. The windows contain 1,440 bytes of shared
immutable array storage; the two LPC arrays contain 272 bytes of local storage.
These sizes do not establish runtime peak stack usage.

The experiment passed independent legacy-reference comparisons over one
million random float/tie vectors and exhaustive seven-value alphabets through
length five, including signed zeros, infinities, NaNs and k=0 through 8.
Bitwise long/short LPC-window checks, golden and LPC C/wasm fixtures, and
targeted all-targets Clippy also passed in the experiment. No golden constants,
FFT, decoder, framing, bridge, release profiles or public APIs changed.
The engine still uses `encode_i16_into`. The rejected pitch scratch remains
reverted.

In the managed worktree, all 136 selected MLOW library tests passed with
`cargo nextest run -p wacore --features voip-mlow --lib -E 'test(voip::mlow)'`.
Targeted all-targets Clippy with `voip-mlow,bench-internals`, formatting and
diff checks passed. The two code patches match the experiment exactly.

New-head cloud CodSpeed results, final stack measurements, full validation,
PR metadata and thread resolution remain with the outer executor. The
published comparison below predates these additions and cannot establish
their CI improvement.

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

## Published CI comparison

[PR #1500](https://github.com/oxidezap/whatsapp-rust/pull/1500) uses branch
`perf/voip-mlow-hotpath-batch` and the title
`fix(wacore): reduce MLOW encoding allocations and PCM staging`.
The completed [CodSpeed check](https://github.com/oxidezap/whatsapp-rust/runs/103640418292)
compares baseline `6502b871e35664ffb80044ba7c6317a6427754e2` with
head `e2664f1c7cdbd4b9f00570ddf4fb47ef3909699a`.
These results apply to that head, before this report update.

The check completed on September 12, 2026 at 23:42:38 UTC with a neutral
conclusion. It reports one regressed benchmark, 789 untouched benchmarks,
two new benchmarks and 12 skipped benchmarks. Skipped rows use baseline
results. CodSpeed warns that some significant changes compare different
runtime environments, which may affect accuracy.

The following are the actual rows in the check's Performance Changes table.

| Mode | Benchmark | Base | Head | Efficiency |
| --- | --- | ---: | ---: | ---: |
| Simulation | h264_depacketize_fua_stream | 233.2 µs | 358.1 µs | -34.87% |
| Memory | mlow_encode_i16_reused_output | N/A | 85.3 KB | N/A |
| Simulation | mlow_encode_i16_reused_output | N/A | 7.1 ms | N/A |

The memory row is CodSpeed's reported metric, not DHAT allocated bytes per
packet or warm encoder live heap. The new i16 rows have no baseline and
therefore establish no improvement. The summary does not publish individual
values for the 789 untouched rows. The H.264 regression remains reported;
the environment warning alone does not establish its cause.

## Remaining work

The largest remaining byte costs are pitch scratch, followed by CELP subframe
temporaries. Decoder work remains much smaller than encoder work. ACB basis
scratch, correlation storage, pulse entropy buffers, playout allocation and
jitter operations have not been changed. The next CPU investigation should
start from the retained FFT/CELP/pitch profiles rather than propose another FFT
algorithm without evidence.

Runtime peak stack measurements for baseline and head on native 64-bit and
wasm32 remain outstanding. Array sizes above are storage sizes, not measured
stack peaks. Review-thread resolution and any further PR metadata changes
remain with the outer executor. The completed CodSpeed comparison above
does not close the stack evidence gap or establish a combined CPU improvement.
