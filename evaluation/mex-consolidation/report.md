# MEX validation report

Implementation contract and migration: [contract.md](contract.md).
Base `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`; final implementation commit
is identified by the published PR head (this report does not self-hash).

## Matched measurement

The same `tests/fixtures/mex_consumer/src/bin/size_probe.rs` is built before and
after the library change. Standalone consumer's unchanged Cargo release defaults,
no dependency default features, nightly-2026-06-16, jobs=1, standard Cargo through
configured mbx; CARGO_TARGET_DIR unset. Native flags come from .cargo/config.toml;
WASM uses `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`, target
wasm32-unknown-unknown. No wasm-opt, profile or size budget changes. The probe
keeps the canonical executor's poll reachable, but never connects/sends.

Consumer lock SHA256:
`53dc77dd2d75e7da4ec542f3a556312a8d1bc6eb5f93ee367f5fbee4812fc059`.
Probe SHA256:
`4f8d17b94531a82d1f12f5c878262c19a3c9bb289e842d7188f7d9ffffb6f3da`.

Both base and head builds exit 0; lock/probe hashes were rechecked unchanged:

| Metric (bytes) | Base | Head | Delta |
| --- | ---: | ---: | ---: |
| Native stripped | 3,052,328 | 3,052,232 | -96 |
| Native .text | 2,997,898 | 2,997,782 | -116 |
| Native allocated text+data+bss | 3,049,482 | 3,049,462 | -20 |
| WASM module | 564,836 | 570,139 | +5,303 |

Measured head implementation `590fe2313ad4852651dac01997b8a3f1b57e8cfa`.
The subsequent lint correction changes only two qualifications in an integration
test, not the linked library or probe. These are consumer-specific measurements,
not evidence of RAM savings or universal binary improvement: WASM grew ~0.94%.
No assertion or budget was changed. The existing CI demo size gate also passed
at the first implementation head: stripped/.text -640 bytes, allocated unchanged,
607 dependencies unchanged (matched base d9f78b806, synthetic PR merge 3abe00242).
See https://github.com/oxidezap/whatsapp-rust/actions/runs/37071472592.

## Checks

- Source audit: no productive ExtensionError construction before removal.
- fmt and diff whitespace check: passed after implementation.
- Added public positive custom/raw construction and independent negative
  constructor/executor/error doctests in both root rustdoc and renamed-dependency
  standalone package. Existing typed linkage/privacy, optional-key omission and
  direct duplicate-key serializer controls retained.
- Default-feature scoped nextest: **180 passed** across MEX, business, groups,
  community and newsletter (2,244 unrelated tests filtered out). Includes
  mutation wire bytes, optional query execution, actual get_username GraphQL/IQ
  404 vs 403/500, original parse/JSON sources and GraphQL-vs-IQ classification.
- Public MEX/error classification nextest: **32 passed**, repeated after lint fix.
- Standalone Cargo-renamed dependency fixture: **5 tests + 6 negative doctests
  passed**, both nightly and MSRV **1.94.1** (`RUSTFLAGS=''` for stable).
- Root doctests: **34 passed, 16 ignored** (retained pre-existing ignored examples).
  The new negative entry-point controls also run here in ordinary CI. The
  separate Cargo-renamed package remains supplemental manual validation.
- Strict workspace/all-target Clippy: first attempt **exit 101**, two unnecessary
  NodeBuilder qualifications in the new integration test. Both corrected using
  the already-imported type; repeat **exit 0**, and root all-feature/all-target
  Clippy **exit 0**, both with `-D warnings`. Original failures are retained.
- Production no-default-feature native and WASM consumers: release builds
  **exit 0**. WASM emitted the same two existing unused-qualification warnings
  at base/head; those were not suppressed.
- Extra `nextest --no-default-features --lib` probe: **failed, exit 101/226
  compile errors**. Native unit fixtures unconditionally use feature-gated
  runtime/SQLite/transport/HTTP APIs (e.g. untouched enc_handler.rs imports
  TokioRuntime, client/builder.rs imports runtime_impl). This is not a passing
  no-default unit suite; no global fixture migration or feature-policy rewrite
  was made. The supported no-default production consumer paths above pass.

## Forge evidence and remaining gates

PR: https://github.com/oxidezap/whatsapp-rust/pull/1605 (ready, not draft).
At implementation head 590fe23, all configured substantive bot reviews completed:
Greptile and CodeRabbit approved, cubic reported no issues, Codex code/security
completed without findings. No extra reviewer or pipeline was requested; no
addressable review threads were created.

First head CI: **33 passed, 4 failed, 5 skipped**, all terminal. Format, rustdoc,
MSRV, build/tests, WASM/native fixture cfg probe, all feature matrices, generated
artifacts, E2E, Miri, CodSpeed and binary size passed. The two strict Clippy jobs
failed for the same two test qualifications fixed above. These were actual
failures, not ignored checks. The new final head will run the configured checks
again; its forge results are recorded in PR comments rather than self-updating
this source report on every CI round.

Cargo Deny failed the unchanged host oracle tooling audit for **Wasmtime 48.0.3**:
RUSTSEC-2026-0325, RUSTSEC-2026-0326 and RUSTSEC-2026-0327. The runtime dependency
audit passed; this PR changes no lockfile/tool pin/deny policy. This required
failure remains visible for the integration owner, not described as green.

Informational semver failed against published 0.7.0; its full log was read and
preserved. Reported existing core/protobuf breaks include removed fields/methods,
added exhaustive variants, constants and sync_action_data_to_vec's changed arity.
The explicitly authorized MEX breaks in this PR are the four entries enumerated
in contract.md and exercised by independent negative compile fixtures; no aliases
were restored to manufacture a semver pass and no version was bumped.

Raw logs, exits and measurement outputs stay in this directory (ignored).
Base build PID 18695, initial head validation PID 1901297 and lint-fix validation
PID 2372811 all ended (completion markers plus process checks) before subsequent
source/config changes. Resources at head validation
planning: 22GiB available RAM, 181GiB free disk, visible compilers ~4.2GiB RSS;
one jobs=1 lane planned ~3GiB RAM/~15GiB additional disk, preserving 5GiB reserve.
