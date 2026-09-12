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
