# MLOW hot-path measurements

## Latest local result

The complete allocation, LPC, top-K and direct-VAD batch reduces steady-state
DHAT blocks from 468.53 to 339.88 per 60 ms packet, and allocated bytes from
396,606.48 to 372,347.76. These include reallocations. The public f32 APIs retain
their contracts. The additive `encode_i16_into` API is used by the call engine
and now passes the original PCM directly to VAD.

CPU improvements are small compared with the allocation reduction. Local
instruction counts and native timings are reported separately below; neither
is substituted for a published CodSpeed comparison. No post-change live
product-call performance measurement was captured. The supplied live-call
profile covers the original baseline only.

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
5. Cache the immutable LPC windows, preserving the target's original f32 formulas.
6. Replace LPC Levinson's two temporary vectors with 17-element f64 arrays.
7. Select CELP survivors with bounded insertion into the caller's index buffer,
   retaining the original NaN behavior and lowest-index ties.
8. Pass the original i16 PCM to VAD, removing the exact conversion back from f32
   and avoiding 1,920 bytes of conversion scratch for an i16-only encoder.

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

The following measurements cover the detached experiment based on `77d8ae91`.
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

The supplied real-call CPU profile over seconds 15 through 33 contains
340,285 µs in the engine, 303,519 µs in encode, 19,901 µs in decode,
15,659 µs in CELP top-K and 31,361 µs in FFT. The bridge uses `6502b871`
with different code generation from release, so these numbers support
attribution only, not absolute production CPU estimates. Reconstructing the
sample ancestry and weighting by `timeDeltas` reproduces 89.20% encode within
the engine, 5.16% CELP top-K within encode and 10.33% FFT within encode.
The V8 heap profile contains 33 samples totaling 20,770,808 bytes; it does not
attribute Rust allocations inside wasm linear memory.

Local CodSpeed profiles ran one row per invocation without upload. Profiles
are retained under `target/` with prefixes
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
from 540 to 411. The final DHAT and warm-heap measurements are below.
The windows contain 1,440 bytes of shared
immutable array storage; the two LPC arrays contain 272 bytes of local storage.
These sizes do not establish runtime peak stack usage.

The experiment passed independent legacy-reference comparisons over one
million random float/tie vectors and exhaustive seven-value alphabets through
length five, including signed zeros, infinities, NaNs and k=0 through 8.
Bitwise long/short LPC-window checks, golden and LPC C/wasm fixtures, and
targeted all-targets Clippy also passed in the experiment. No golden constants,
FFT, decoder, framing, bridge or release profiles changed. Existing public APIs
remain compatible; this PR adds `MlowEncoder::encode_i16_into`, which the engine
uses internally. The rejected pitch scratch remains reverted.

In the managed worktree, all 136 selected MLOW library tests passed with
`cargo nextest run -p wacore --features voip-mlow --lib -E 'test(voip::mlow)'`.
Targeted all-targets Clippy with `voip-mlow,bench-internals`, formatting and
diff checks passed. The two code patches match the experiment exactly.

These checks are completed evidence, not pending work. The published CI
checkpoint below names its exact revision and must not be read as a measurement
of later changes. Final PR checks and review status are linked from PR #1500.

## Direct i16 input to VAD

Every i16 sample is represented exactly after conversion to f32 and division by
32768. Multiplication by 32768, rounding and clamping previously reconstructed
that same i16 value for VAD. The new path borrows the original samples instead.
A debug invariant checks that the two input representations agree. The f32 API
retains its conversion buffer, including after alternating between APIs.

The isolated VAD experiment compares against the LPC/top-K version, using the
same i16 benchmark input in both runs.

| Measure | Before direct VAD | After direct VAD |
| --- | ---: | ---: |
| i16 reused-output Ir | 7,918,025 | 7,883,980 |
| i16 reused-output Ct | 559,080,303 | 557,129,194 |
| i16 reused-output Cl | 1,963,888,738 | 1,955,523,737 |
| Engine outbound Ir | 8,279,386 | 8,248,696 |
| i16 native median, CPU 2, 500 samples | 296.3 µs | 294.4 µs |
| Outbound native median, CPU 2, 500 samples | 308.2 µs | 305.5 µs |
| i16-only warm encoder heap | 157,725 B | 155,805 B |

This small deterministic reduction and the smaller retained heap justify the
internal dataflow change. It adds no public API beyond the engine-used i16
method already in the PR. Together, the engine staging removal and direct VAD
remove 5,760 bytes of retained PCM conversion storage from an i16-only call.
They avoid 5,760 bytes of staging writes and 7,680 bytes of staging reads per
non-silent packet. Required normalization and VAD's input read still happen.

## Final allocation attribution

The same 100-packet DHAT stream records 33,988 blocks and 37,234,776 bytes on
`hot_encode` stacks, excluding setup and priming. The baseline records 46,853
blocks and 39,660,648 bytes. This is a 27.46% block reduction and 6.12% byte
reduction; it is not zero-allocation encoding.

| Measured allocation site | Base calls/packet | Final | Base bytes/packet | Final |
| --- | ---: | ---: | ---: | ---: |
| Perceptual Levinson scratch | 72 | 0 | 14,112 | 0 |
| Perceptual response vectors | 24 | 0 | 2,352 | 0 |
| Perceptual response outer vectors | 6 | 0 | 576 | 0 |
| Candidate pulse storage | 3 | 0 | 3,840 | 0 |
| CELP trimmed pulse output | 11.65 | 0 | 98.72 | 0 |
| LPC window vectors | 6 | 0 | 3,808 | 0 |
| LPC Levinson scratch | 6 | 0 | 816 | 0 |
| CELP subframe output container | 3 | 3 | 1,056 | 2,400 |

The last row grows because its elements now contain inline pulses. Its extra
1,344 bytes offset part of the savings; the table reconciles to 128.65 fewer
blocks and 24,258.72 fewer bytes per packet. Pitch C/E/H remain unchanged at
178,176 allocated bytes per packet combined. The f32 encoder's warm live heap
remains 157,725 bytes; an i16-only encoder now retains 155,805 bytes, measured
with `voip_profile encoder-live-i16` and `dhat-heap`.

## Runtime stack measurements

Measurements compare baseline `6502b871` with the full batch, including direct
VAD. The diagnostic package uses opt-level 3, fat LTO, one codegen unit, aborting
panics and debug level 1. It drives the actual public encoder methods after
eight warmup packets, then measures 128 sequential packets for each of four
input families: varied tone, silence, deterministic noise, and silence followed
by alternating i16 extrema. `encode`, `encode_into` and the head-only
`encode_i16_into` reach the same peak within each target.

| Target and runtime metric | Base | Final | Delta |
| --- | ---: | ---: | ---: |
| x86-64, touched-stack high-water | 25,096 B | 28,088 B | +2,992 B |
| i686, touched-stack high-water | 24,224 B | 27,256 B | +3,032 B |
| wasm32, shadow-stack pointer peak | 19,376 B | 24,432 B | +5,056 B |
| wasm32, touched-memory high-water | 19,376 B | 24,272 B | +4,896 B |

Native measurement paints 256 KiB in a no-inline Rust function, returns from
that function, and uses GDB to scan the sentinel after the measured calls.
Setup, painting and warmup are outside the interval. The first changed 64-bit
word gives an eight-byte-resolution touched-memory high-water, not a proof
about every reserved but untouched native stack slot.

The wasm observer is inserted after every `global.set $__stack_pointer` in the
compiled module. It tracks the minimum pointer while the measurement marker is
active, without allocating guest stack or changing codec arithmetic. A separate
sentinel scan corroborates touched memory. The final pointer peak exceeds the
touched peak by 160 bytes, so the two measures are deliberately not conflated.
The deepest pointer is observed in CELP's `smpl_get_maxi_k`. Original and
instrumented modules produced identical packet-stream digests, and baseline/head
digests match within each target and input family.

Dynamic depth at entry to `smpl_analyze_frame_st` after its prologue is separate
from its descendants' peak: x86-64 is 14,312 to 14,296 bytes, i686 is 14,348 to
14,076 bytes, and wasm32 is 13,568 to 13,296 bytes. A smaller entry depth does
not imply a smaller whole-chain peak.

The observed increases are bounded absolute costs of about 2.92 KiB, 2.96 KiB
and 4.94 KiB. The arrays remain; no per-frame heap allocation was reintroduced.
These are workload/build-specific measurements, not universal task-stack size
recommendations. Caller and runtime frames need their own budget. i686 supplies
a representative 32-bit measurement, not an ESP32 hardware measurement.

The local diagnostic source, manifests, GDB commands, wasm observer and initial
logs are archived in `.cache/mlow-evidence/local-profiles-and-stack-probe.tar.gz`.
Final logs are `.cache/mlow-evidence/native64-final.log`, `native32-final.log`
and `wasm-final.json`. The observer verifies all expected stack-write sites
exist, and validates the transformed module before execution.

## Correctness and portability

- The complete wacore suite passed after the functional changes, including engine,
  golden, C/Go/wasm fixtures and the million-vector top-K comparison.
- The 2,048-packet encode/decode stream passed. It is now a normal, non-ignored
  test so regular CI also exercises long i16 continuity.
- New i16 equivalence coverage exercises every i16 value, reused oversized output,
  invalid lengths between valid frames and reset boundaries.
- Golden checksums and C/Go/wasm fixture expectations were not changed.
- Targeted MLOW Clippy and workspace all-targets Clippy passed.
- wasm32 builds with `--no-default-features --features voip-mlow,js`.
  Omitting the existing `js` feature fails in getrandom before reaching the codec.
- The equivalent diagnostic executable's native text grows from 778,159 to
  780,391 bytes, an increase of 2,232 bytes. After stripping custom sections,
  its wasm module grows from 446,143 to 447,230 bytes, an increase of 1,087 bytes.
  These are executable measurements with the shared diagnostic driver, not
  codec-only text sizes or downstream wasm-opt results.
- Runtime stack measurements are complete for native 64-bit, native 32-bit
  and wasm32, with their measurement boundaries documented above.

## Published CI checkpoint before LPC, top-K and direct VAD

[PR #1500](https://github.com/oxidezap/whatsapp-rust/pull/1500) uses branch
`perf/voip-mlow-hotpath-batch` and the title
`perf(voip): reduce MLOW encoding allocations and PCM staging`.
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

## Remaining hotspots and validation limits

The largest remaining byte costs are pitch scratch, followed by CELP subframe
temporaries. Decoder work remains much smaller than encoder work. ACB basis
scratch, correlation storage, pulse entropy buffers, playout allocation and
jitter operations have not been changed. The next CPU investigation should
start from the retained FFT/CELP/pitch profiles rather than propose another FFT
algorithm without evidence.

The completed CodSpeed checkpoint above predates later batches. The PR body
records the final-head comparison and its exact run link when available.
Conformance rederivation is blocked by unavailable pinned wasm captures,
including `9Nbh3eMuVjD.wasm`; existing committed codec fixtures pass.
The informational semver check reports pre-existing changes against published
0.7.0 releases. These failures are not reported as green checks.

No post-change live WhatsApp call was validated. A follow-up live comparison
should repeat the supplied symbolized profile after updating the consumer pin.
The next core CPU investigation should attribute residual analysis and CELP
work after the top-K change. Downstream optimization-level experiments belong
in the consumer and do not change this repository's release profile.
