# Receive future footprint

Measured on native aarch64 with pinned nightly-2026-06-16, default features plus
`bench-harness`. Baseline was `6f07e3ab`; identical diagnostic/benchmark scaffolding
was kept for the production baseline and candidate, built in-place. The selected
candidate delays the outer message Arc until dispatch, shares probe-materialized
plaintext, and boxes only the secret-envelope continuation. Processing stays
inline; lane lifecycle, Signal locks and commit ordering are unchanged.
The raw matched measurements use snapshot `6f10080`; a later benchmark-only
single-poll enqueue guard prevents worker scheduling if enqueue yields. No
historical enqueue timing is claimed to isolate creation with that newer guard.

## Future and requested-heap results

`audit_receive_future_and_struct_layouts` uses `size_of_val`, including the
runtime-erased task's pointee, rather than the size of a boxed pointer.

| Native test future | Before | After |
| --- | ---: | ---: |
| Lane task | 11296 B | 6544 B |
| Scoped receive | 11120 B | 6368 B |
| Session batch | 10736 B | 5984 B |
| Plaintext handling | 8864 B | 4112 B |
| Owned dispatch wrapper | 7440 B | 1960 B |
| Duplicate probe | 2000 B | 96 B |

Five DHAT repetitions per lane count, fixture/encrypted inputs outside profiling:

| Active lanes | Cold peak before/after | Drained retained before/after |
| --- | ---: | ---: |
| 1 | 252807 / 248039 B | 98032 / 93264 B |
| 32 | 685707 / 533131 B | 533508 / 380932 B |
| 256 | 3834843 / 2614235 B | 3687012 / 2466404 B |

These cold/drained values were exact over five repetitions. Optimized-profile
requested heap falls 4768 B per lane (31.83% cold peak at 256 lanes). The 16 B
per-lane difference from test-profile state is configuration-specific. Cumulative
high water after the warm batch falls similarly; it is not an independent warm
peak and may retain the cold maximum. Post-close retained heap is unchanged
within small variable-length receipt strings. This is **not allocator RSS or connected-idle
memory**. Lane allocation counts are unchanged.

Fifty ordinary DMs retain the same 273151 total requested bytes / 1962 blocks.
Plain text, pure SKDM and suppressed text also keep baseline allocation totals
and counts. The delayed Arc avoids allocating a shared outer message for the
last two paths. A concrete plaintext-batch entry future shrinks 9008 -> 4240 B;
both are below Tokio 1.53.1's release `block_on` boxing threshold, 16384 B.
Runtime-entry future moves remain a harness artifact, not per-message clones.

An empty/malformed secret carrier adds **1976 B of total requested allocation /
one boxed continuation per message**: a 50-message batch totals 82924 -> 181724 B
and 481 -> 531 blocks. Batch peak separately rises 2096 -> 3736 B (+1640 B);
total allocation and peak live heap differ because allocation lifetimes overlap
differently. This control does not measure successful decryption or every resend
shape: substituted envelopes can
create a different final inner Arc, and an unresolved duplicate probe can retry
materialization during dispatch. These costs are not claimed absent.

## Runtime and binary tradeoffs

Five paired AB/BA rounds, OS timer, 30 samples per function/run, no compiler
while sampling. Full Signal DM50 paired median delta +0.93% (range -9.61..+4.02%);
group50 -3.12% (-30.93..+3.20%). No general speed or no-regression claim.

Crypto-bypass plaintext controls expose small skipped-path regressions: SKDM50
35.67 -> 36.97 us, paired median +3.70% (+0.45..+6.21%); suppressed50 138.9 ->
143.9 us, +2.42% (+1.73..+31.30%). Malformed-secret50 138.9 -> 136.2 us, -2.46%
(-3.24..+5.73%). These include receipt flush and once-per-batch runtime entry,
not just the allocation instruction. The host has two available CPUs and a
1.8-core quota; large scheduling variability remains.

A linked task-local wasm32 cdylib retained creation/poll/drop of a real
`Client::run` future (including its connection read and direct message handler).
Same probe lock, toolchain, local matching std sysroot, getrandom wasm-js flag,
opt3/FatLTO/one codegen unit/abort/stripped: before/after **5944208 B**, code section
**5688786 B**, 4162 functions. Both validate; code hashes differ. No total/code
section growth in this probe. This is not connected WASM execution or ESP proof;
ESP remains unmeasured.

Native matched benchmark `.text` grows **16880 B** (7045588 -> 7062468 B), rodata
unchanged. It is not the default-feature shipping demo; its cost remains explicit
and CI's shipping-size gate still applies. Do not infer no native growth from
WASM equality. The memory benefit, not a universal walltime win, motivates this
change.

## Reproduction

Run serially with `CARGO_BUILD_JOBS=1`, never compile during sampling:

```sh
cargo test --locked -p whatsapp-rust --lib --features bench-harness -- --test-threads=2
cargo test --locked -p whatsapp-rust --lib --features bench-harness audit_receive_future_and_struct_layouts -- --ignored --nocapture
cargo clippy --locked -p whatsapp-rust --all-targets --features bench-harness -- -D warnings
cargo bench --locked -p whatsapp-rust --features bench-harness --bench client_receive --no-run
cargo build --locked -p whatsapp-rust --features bench-harness --profile bench --example receive_footprint
```

Save the built executables for both revisions. Run the bench executable directly
with `--bench --color never --timer os --sample-count 30`; the `--bench` flag is
required by this Divan fork. Alternate versions across five paired rounds.
Run `receive_footprint` separately; never interpret DHAT walltime as latency.
At the measurement snapshot, the library suite passed 2258 tests, the ignored
layout audit and touched-crate all-target clippy passed. The newer enqueue guard
has a separate 1/32/256-lane no-worker-poll regression test. Fixture inputs use
fictitious identities.
