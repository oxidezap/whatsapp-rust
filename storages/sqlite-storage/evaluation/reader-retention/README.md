# Reader-retention artifact provenance

These are storage-only measurements, not connected-client process RSS.

| Artifacts | Source identity | Reader construction |
| --- | --- | --- |
| `point*-*.txt`, `scan50-*.txt`, `real600-*.txt` | Working prototype atop `6f07e3ab7f6a94764c23dfd6298c312b58419e7b`, before rebasing | Explicit test reader-pool replica; `real600` uses a real elapsed ten-minute timeout, not the production constructor |
| `correctness.txt` | Same pre-rebase prototype | Native lifecycle/regression suite, 130 passing tests |
| `current-correctness.txt`, `current-database-api.txt`, `current-docs.txt` | Candidate rebased onto `19eb64cac58bc2e36cc9e2bc3c92fe9af3a78729` | 132 library tests, 5 database-API tests, 7 doctests |
| `environment.txt`, `linked-size.txt` | Original `6f07e3ab` comparison, same flags in both arms | Native linked consumer, not the default-feature demo CI size gate |
| `production600-baseline.txt`, `production600-adaptive.txt` | `bb40a2ee` plus test-only production-builder assertions (candidate also has the unrelated lifetime-test correction) | Actual `SqliteStore::with_config(default)` in both arms; baseline restores only the pre-change builder using `production-baseline.patch` |
| `current-clippy.txt` | Original post-rebase lint output, argv not captured | Superseded as command/driver proof by `review-clippy.txt`; checkout prefix redacted |
| `review-correctness.txt`, `review-clippy.txt` | Same review-corrected candidate | Post-review 132 library tests and command-recorded all-target storage clippy |

The original measurement binaries contain 131 tests (one selected measurement
plus 130 filtered out); the rebased binaries contain 133. The difference is
upstream's additional tests, not an omission of the prototype lifecycle tests.
Old outputs are deliberately retained, not regenerated or relabeled as HEAD.

## Production-builder pair

Both arms use aarch64 Linux, `nightly-2026-06-16`, bundled SQLite, release,
`CARGO_BUILD_JOBS=1`, `CARGO_PROFILE_RELEASE_LTO=off`, `opt-level=z`,
`codegen-units=1`, and the same Cargo.lock.
The only production-source difference is the archived reader-builder patch.
Each executable runs separately with no compiler activity:

```text
SQLITE_READER_MODE=baseline|adaptive
SQLITE_READER_COUNT=50
SQLITE_READER_IDLE_SECS=600
<matching-built-executable> sqlite_store::reader_lifecycle_tests::measure_reader_retention --exact --ignored --nocapture --test-threads=1
```

The seeded file, row count, payload sizes, point-read warmup and active
read/write workload are unchanged. `production_builder=true` is printed;
assertions verify the effective minimum-idle setting, ten-minute timeout,
startup readers, retained writers, observed retirement and reacquisition.
Preserved executable SHA-256 values:

```text
baseline  c377803ee5c3ff65e6ea8c788ebe2b547b6c3eb9c7fab98dc2875be65b5384fb
candidate cd41944c3378b2ac4bf2e054ea2cb34debff562e40c3bc24de63e20d474470cf
```

Inflight snapshot/cancellation/failure cases are tested separately using the
explicit accelerated replica and real SQLite; the ten-minute memory run has
no intentionally held reader lease. Do not describe it as a real-ten-minute
inflight-lease test.

Raw local build/verbose lint logs remain in the task-owned evaluation target.
The committed lint output preserves the full command/output while replacing
only the machine-specific checkout prefix with `<repo>`.
