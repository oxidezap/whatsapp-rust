# Native WAL reader retention evaluation

## Architecture and hypothesis

At `6f07e3ab`, the default file-backed WAL store eagerly opens one writer and
one `query_only` reader. The pools share the existing process-wide r2d2
management threads. A separate semaphore bounds reader blocking jobs; their
pooled checkouts protect connections until the blocking work finishes, even
when an async caller is cancelled. `SharedSqlite` and sibling device handles
share these same pools and permits.

r2d2 0.8.10 already has a ten-minute idle timeout, a thirty-second reaper, and
on-demand connection creation. Its default `min_idle = None` means `max_size`,
so an idle reader **is already closed and immediately replaced**. The baseline
does not retain its warm page cache indefinitely, but pays for a replacement
connection even when no reads arrive. Reducing the page-cache cap is not the
same as eliminating this fixed connection cost.

The candidate changes only the reader pool's minimum idle count to zero. It
uses the existing timeout/reaper and recreates readers on demand, without a
new task, timer, cache, connection pool, or maintenance control plane. Initial
reader checkouts still validate every configured connection before the
constructor returns. Writer pragmas, migrations, transactions, timeouts and
serialization remain unchanged. Non-WAL, shared-cache, in-memory and browser
paths still have no extra reader pool.

## Reproduction

The ignored native measurement lives in
`src/sqlite_store/reader_lifecycle_tests.rs`. Each arm must run in its own
process, with no other tests in that process:

```sh
CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=off cargo test --locked --release \
  -p whatsapp-rust-sqlite-storage --lib --no-run

SQLITE_READER_MODE=baseline CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=off \
  cargo test --locked --release -p whatsapp-rust-sqlite-storage --lib \
  sqlite_store::reader_lifecycle_tests::measure_reader_retention \
  -- --exact --ignored --nocapture --test-threads=1

SQLITE_READER_MODE=adaptive CARGO_BUILD_JOBS=1 CARGO_PROFILE_RELEASE_LTO=off \
  cargo test --locked --release -p whatsapp-rust-sqlite-storage --lib \
  sqlite_store::reader_lifecycle_tests::measure_reader_retention \
  -- --exact --ignored --nocapture --test-threads=1
```

Both arms default to 50 independent stores against one seeded WAL file, a
512 KiB cache cap, and 4,000 fictitious 1 KiB session records. The writer stays
open. The point-read profile prices the ordinary small reader cache; setting
`SQLITE_READER_WARM=scan` first scans the records to fill reader page caches.
`SQLITE_READER_COUNT=1` covers the single-store shape. Both arms shorten only
the reader idle timeout to one second, retaining r2d2's actual thirty-second
reaper. These accelerated runs attach an explicit reader-pool replica; they
are not production-constructor measurements. The baseline also reaps/replaces
readers, avoiding an unfair comparison against a permanently warm baseline.

`SQLITE_READER_IDLE_SECS=600` instead uses the actual
`SqliteStore::with_config(default)` constructor, with no timeout override.
Build separate baseline and candidate binaries for that comparison: apply
`evaluation/reader-retention/production-baseline.patch` to restore only the
pre-change reader-builder body, build and preserve the baseline executable,
reverse the patch, then build the candidate executable with identical flags.
Run each executable separately with `SQLITE_READER_COUNT=50`, the matching
`SQLITE_READER_MODE`, and `SQLITE_READER_IDLE_SECS=600`; never compile during
a measurement. The evaluator asserts the effective minimum-idle policy,
600-second timeout, eager reader count, retained writer count, observed idle
reader count and reacquisition count. A mismatched binary/mode fails rather
than silently measuring a different policy.

Checkpoints report SQLite's process-global `sqlite3_memory_used()` (live native
SQLite allocations, not total process heap), procfs `RssAnon` and total RSS
separately, and actual writer/reader pool connection counts. No allocator trim
is forced. Neither an estimated cache cap nor file-backed resident pages are
presented as anonymous RSS. Independent allocator retention can keep anonymous
pages resident after SQLite frees its allocations.

After idle, each store performs one point read; latency includes reader
recreation where applicable. A fixed active workload then overlaps 2,000 point
reads with 200 upserts of 256 existing 1 KiB session records, reporting total
wall time and point-read p50/p99. The timing includes real SQLite and the normal
store API dispatch, not mocks or simulated instruction counts.

## Original replica results

The original measurements below used the explicit reader-pool replica, including
the real elapsed 600-second run. They remain archived under their original
`6f07e3ab` prototype identity. The production-builder follow-up is separate;
see the artifact manifest for revision and factory provenance.

Measured on the primary Linux aarch64 VPS, `nightly-2026-06-16`, optimized
release with LTO disabled for this bounded storage evaluator. No compiler or
other scheduled heavy evaluation ran during measurements. Raw outputs and
exact environment are retained in [`evaluation/reader-retention/`](evaluation/reader-retention/).
This is a storage-only experiment, **not connected WhatsApp-session RSS** and
not a measurement of total live Rust/process heap.

Post-idle checkpoints (bytes; writer count is unchanged):

| Shape | Baseline SQLite heap | Candidate SQLite heap | Baseline RssAnon | Candidate RssAnon | Readers, baseline → candidate |
| --- | ---: | ---: | ---: | ---: | --- |
| 50 stores, point reads, accelerated ABBA | 10,181,248 | 5,249,648 | 18,382,848–19,353,600 | 17,571,840–17,575,936 | 50 → 0 |
| 50 stores, full scans, accelerated | 9,089,264 | 4,157,648 | 19,869,696 | 17,829,888 | 50 → 0 |
| 1 store, point read, accelerated | 204,064 | 105,432 | 6,029,312 | 5,902,336 | 1 → 0 |
| **50 stores, replica with real 600-second timeout** | **9,865,264** | **4,933,648** | **19,173,376** | **17,645,568** | **50 → 0** |

The incremental native SQLite heap saving is approximately **98,632 bytes
(96.3 KiB) per reader**. The actual-interval run saves 4,931,616 bytes of live
SQLite heap across 50 stores. Its post-idle RssAnon is 1,527,808 bytes lower;
adjusting for the arms' 102,400-byte pre-idle difference leaves a 1,425,408-byte
difference in phase growth. The point-read candidate's own anonymous residency
stays approximately flat rather than falling by the freed heap amount. The
baseline grows during reader replacement; avoiding that allocation churn is
useful, but allocator-retained holes remain resident. After dropping every
store, SQLite's allocation counter returns to zero while process RssAnon stays
nonzero in **both** arms.

The scan arms have identical 38,676,752-byte warm SQLite heap checkpoints.
Both already reclaim warm caches during the existing idle reap; the candidate
must not be credited for the entire peak-to-idle reduction.

### Resume and active tradeoff

For 50 stores at the real ten-minute interval, first-read p50/p99 goes from
**0.087/0.419 ms to 0.510/2.156 ms**. Each arm samples 50 first reads, not a
large population-wide latency distribution. The single-store accelerated run
has one first-read observation per arm: 0.283 ms baseline versus 0.999 ms
candidate. These timings include normal Tokio blocking dispatch; idle blocking
threads may also have retired. Warm read p50/p99 in the real-interval run is
0.041/0.109 ms versus 0.036/0.129 ms.

The fixed active read/write workload takes 237.6 ms baseline versus 247.1 ms
candidate at the real interval. The accelerated ABBA runs span 216.2–359.1 ms
across arms; active point-read medians overlap. There is **no claimed active
speedup**, nor a precision claim that excludes a small regression. The visible
tradeoff is paying connection initialization on the first read after prolonged
idle; warm operations retain their separate WAL reader queue.

## Production-builder follow-up

A separate review-correction pair on `19eb64ca` + the candidate uses
`SqliteStore::with_config(default)` for **both** arms. The baseline executable
restores only the original reader-builder body; the candidate uses the shipped
builder unchanged. Same optimized profile, bundled SQLite, seeded file, 50
stores, point warmup and fixed active workload. No compiler ran during either
measurement. Pool-policy and observed-count assertions pass in both arms.

| Post-idle metric | Production baseline | Production candidate |
| --- | ---: | ---: |
| Live SQLite heap, bytes | 9,865,264 | 4,933,664 |
| RssAnon, bytes | 18,563,072 | 17,879,040 |
| Total RSS, bytes | 23,285,760 | 22,589,440 |
| Readers / writers | 50 / 50 | 0 / 50 |
| First-read p50 / p99, ms | 0.074 / 0.364 | 0.509 / 1.050 |
| Warm-read p50 / p99, ms | 0.037 / 0.433 | 0.043 / 0.297 |
| Active workload wall time, ms | 427.1 | 256.9 |

The production path confirms **4,931,600 bytes (96.3 KiB per retired reader)**
of live SQLite savings. It does **not** confirm an equivalent RSS drop:
post-idle RssAnon differs by 684,032 bytes; the arms start the idle phase with
a 225,280-byte difference in the opposite direction, giving a 909,312-byte
difference in phase growth. Candidate RssAnon itself increases by 106,496
bytes during idle despite SQLite freeing allocations. All 50 readers reopen
successfully; dropping the stores returns live SQLite heap to zero.

Cold initialization again costs roughly 0.44 ms at the sampled median. Active
wall times now span 216–427 ms across archived comparisons. The apparently
faster candidate in this pair is **not claimed as an active speedup**; warm
medians and tails vary, and these runs cannot exclude a small active regression.
The practical tradeoff remains lower idle connection cost versus a first-read
reopen after ten minutes. The pair takes 632.07 + 631.90 seconds (21.1 minutes)
of measurement time; the separate serial rebuilds take 74 + 70 seconds.
Inflight lease/snapshot, cancellation and failure cases remain separately
identified accelerated real-SQLite tests, not a claimed ten-minute lease test.

## Correctness and scope

Native tests cover eager initialization of every configured production reader.
Accelerated replica tests cover an in-use WAL snapshot surviving retirement of
another reader, committed-state visibility,
concurrent cold reacquisition, `query_only`, busy timeout and foreign keys,
reader-init failure/recovery, cancellation retaining a blocking job's lease,
shared/sibling handle lifetime, and shutdown. Existing storage tests cover
migrations, transaction atomicity, commit barriers, non-WAL/shared-cache
fallback, and writer/reader isolation.

Only the native reader builder and its eager validation change. The browser
still declines the reader pool; the new r2d2 setting and validation are
native-cfg-gated. No WASM/ESP linked-size or runtime result is inferred from
this native evaluator. Applicable shipping-binary size/compatibility checks
remain distinct from this evaluator's binary size.

A separately linked `per_connection_memory` consumer (same native toolchain,
release, `panic=abort`, LTO off for both arms) grows from 1,720,272 to 1,785,808
bytes after explicit `strip --strip-all`: **+65,536 bytes**, exactly the
repository's 64 KiB binary-growth envelope. Its `.text` grows 4,464 bytes and
allocated sections grow 4,528 bytes. The larger file delta is layout/padding,
not 64 KiB of extra instructions. It is recorded rather than waived; this is
not the default-fat-LTO `demo` CI gate, and does not replace that check.
The separate default-feature demo CI comparison subsequently passes: +704
bytes stripped, +640 bytes `.text`, -16 bytes allocated sections, no dependency
increase (baseline `19eb64cac`, tested PR merge head `0c4a0321a`). The WASM
release build and feature-matrix checks also pass; no ESP runtime result is
inferred.

Upstream's subsequent SQLite database/device administration API refactor
retains the same reader builder, pragmas, pools, semaphores and migrations.
The measurements above deliberately retain their original `6f07e3ab` baseline
identity rather than pretending they were taken against a different commit.
After rebasing onto `19eb64ca`, all 132 native library tests, 5 database-API
integration tests and 7 doctests pass; all-target storage clippy passes with
warnings denied. Those current-main outputs are archived separately.
