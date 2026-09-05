# Rust tasks

Use `cargo xt --help`. Tasks are host-only, outside default-members. The
runtime libraries do not depend on this crate.

| Command | Purpose |
|---|---|
| `cargo xt proto-desc` / `tables-desc` / `wire-desc` | Regenerate protobuf descriptors and hash sidecars |
| `cargo xt mlow regenerate --oracle-repo PATH --check` | Run the pinned unwasm Rust tasks and verify the primary corpus |
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
| `cargo xt ci binary-size-report --head DIR --base DIR --out-dir DIR` | Render report and absolute-budget gate |
| `cargo xt ci workflow --help` | Workflow setup, readiness, summary and release tasks |
| `cargo xt sha256 FILE` / `cargo xt sha256 --hex HEX` | Reproducible content hashes |

The C reference remains an independent external oracle, selected through
`MLOW_REFERENCE`; its task refuses dirty/stale builds as before. Compilers,
Cargo, Git and deployment tools remain external programs; task logic does not
invoke Python or maintained shell scripts.

`xtask-support` is pinned to the unwasm repository. It shares descriptors,
checked I/O, hashes, binary readers and CBOR/zstd without linking the wasm engine
into these tasks. The unwasm command validates cached runs itself, so neither
repository imports another language's implementation or duplicates manifest
validation. Stable oracle builds clear inherited nightly-only rustflags.

Metadata commands write only machine-readable results to stdout. Timed tests
require `NEXTEST_PROFILE` and `TEST_TIMINGS_DIR`. GitHub workflow tasks use the
same scoped environment inputs as their previous inline implementations.
