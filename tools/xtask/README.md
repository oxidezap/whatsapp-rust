# Rust tasks

Use `cargo xt --help`. Tasks are host-only, outside default-members. The
runtime libraries do not depend on this crate.

| Command | Purpose |
|---|---|
| `cargo xt proto-desc` / `tables-desc` / `wire-desc` | Regenerate protobuf descriptors and hash sidecars |
| `cargo xt mlow regenerate --check` | Run the local WhatsApp wasm oracle and verify the primary corpus |
| `cargo xt mlow verify` | Re-derive all locked J/S outputs |
| `cargo xt mlow specs --check` | Check generated specs against their recipes |
| `cargo xt oracle fetch` | Restore every hash-pinned WhatsApp wasm capture |
| `cargo xt mobile-apk --help` | Pin, fetch, import or verify exact APK bytes without executing them |
| `cargo xt oracle --help` | Capture-specific diagnostic patches |
| `cargo xt mlow pack SOURCE OUTPUT` | Lossless canonical CBOR + zstd |
| `cargo xt mlow pack-legacy --check` | Verify independent historical C auditors |
| `cargo xt mlow c-reference --check` | Build/run the C auditor harness and compare packet/PCM pairs |
| `cargo xt ci feature-matrix-crates` | Publishable crates from Cargo metadata |
| `cargo xt ci test-features PACKAGE` | Compatible native feature set |
| `cargo xt ci nextest-timed LABEL -- ARGS` | Run nextest, preserve JUnit and the child exit status |
| `cargo xt ci test-waproto-features` | Run both serde variants, retaining the first failure |
| `cargo xt ci test-feature-packages PACKAGES...` | Test native feature sets for each package |
| `cargo xt ci sync-bartender-image --check` | Check canonical service image pins (`--write` updates) |
| `cargo xt ci measure-binary-size --out-dir DIR` | Measure release binary and attribution |
| `cargo xt ci measure-binary-size --gate-only --out-dir DIR` | Measure the same release binary and dependency count without bloat/llvm-lines |
| `cargo xt ci binary-size-report --head DIR --base DIR --out-dir DIR` | Render report and absolute-budget gate |
| `cargo xt ci workflow --help` | Workflow setup, readiness, summary and release tasks |
| `cargo xt sha256 FILE` / `cargo xt sha256 --hex HEX` | Reproducible content hashes |

The C reference remains an independent external oracle, selected through
`MLOW_REFERENCE`; its task refuses dirty/stale builds as before. Compilers,
Cargo, Git and deployment tools remain external programs; task logic does not
invoke Python or maintained shell scripts.

`xtask-support` is local and contains descriptors, checked I/O, binary readers
and canonical CBOR/zstd. Capture transport comes from whatspec's commit-pinned
`wa-store`; static wasm analysis comes from commit-pinned `unwasm-core`.
`oracle-core`, both dependencies and all recipes remain host-only workspace
tools outside `default-members`.

Metadata commands write only machine-readable results to stdout. Timed tests
require `NEXTEST_PROFILE` and `TEST_TIMINGS_DIR`. GitHub workflow tasks use the
same scoped environment inputs as their previous inline implementations.

Binary-size measurement is complete by default. `--gate-only` retains stripped
bytes, `.text`, allocated sections, dependency count and provenance, and marks
heavy diagnostics as omitted. It still builds the release demo. PRs collect
full diagnostics only after a failed budget, using `--skip-build` with a separate
output directory; the original gate remains authoritative. Main measurements
keep full attribution and LLVM-line metrics.

`cargo xt ci consumers run --lane native` runs the standalone public API profiles
in `tests/api-consumer`; `--lane msrv --toolchain 1.95.0` and `--lane wasm`
select the other gates. The single lock is mandatory. Directed negatives share
one validator for error code, message and primary source. There is no staging
exemption for missing fixtures. Add new contracts to an existing domain module.

`cargo xt ci compatibility` runs declared profiles and checkout consumers by
default. During preparation, main pushes and pull requests targeting main can
delegate native/MSRV checkout consumers with `--preparing-consumers api-hosts`.
The task verifies the event, exact head, compiler and unfiltered owner workflow
before delegating. After package and mutation checks, Supply Chain verifies the
matching API host job and successful consumer step for this exact head and latest
workflow attempt. GitHub records the reusable host workflow revision independently
of the checked-out source. The verifier reads the owner configuration and compiler
pin at that execution revision, including pull-request merge revisions. The
consumer driver checks the source head and tracked files before every command
and after the full suite. Failed, cancelled, skipped, missing or ambiguous evidence fails
the job; pending evidence has a bounded wait. Delegation makes no local execution
claim and needs only read access to Actions. Scheduled, dispatched and local
runs keep the full default. Frozen baseline and release qualification always run
their original consumers. Packaged artifacts and mutation controls still run in
Supply Chain, and WASM keeps both maintained compiler qualifications.
