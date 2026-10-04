# Verify a change

Use the pinned toolchain from `rust-toolchain.toml`. The workflows and Rust
`cargo xt` tasks are the authoritative command definitions.

```sh
cargo fmt --all
cargo nextest run -p <touched-crate> --lib
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p <touched-crate> --doc
```

CI uses nextest's `ci` profile. Nextest does not run doctests; examples need the
separate rustdoc step. Feature-sensitive changes also need the relevant minimal
feature profile, MSRV and WASM checks. `cargo xt ci test-features PACKAGE` prints
the shareable native feature set. Keep external consumers outside the root
dev-dependency graph so feature unification cannot hide missing requirements.

## Runtime and E2E tests

`tests/e2e/src/lib.rs` supplies `TestClient`. Start the digest-pinned mock server
specified in `.github/workflows/e2e.yml`, including `CHATSTATE_TTL_SECS=3`, then
run `cargo nextest run --profile e2e -p e2e-tests`. Each client uses an isolated
backend and `unique_push_name`; sharing an account must be explicit.

Wait for events or bounded state transitions, never fixed sleeps. A reconnect
returns before background teardown; use `wait_for_disconnected`, not a suppressed
`Disconnected` event. Disconnect every client and assert offline state if the
next step depends on it. Use narrow fault hooks to reproduce races deterministically.
Offline receive processing and commit drains share one permit in production;
fixtures must reproduce that exclusion before asserting hook deduplication.
Hooks remain at-least-once across failures and require consumer idempotence.

Native call fixtures are documented in `src/test_support.rs`. The bench-only
receive layout audit is `audit_receive_future_and_struct_layouts`; run it with
`--features bench-harness -- --ignored --nocapture`. Use `client_receive` and
`examples/receive_footprint.rs` for CPU and requested-heap measurements.
`benches/connected_idle.md` describes connected retention measurement.

## Layout and binary budgets

Exact size assertions protect representation contracts. Other layout tests use
upper bounds or compositional comparisons, with pointer-width gates where needed.
On failure, run the focused test, identify which field/compiler/dependency moved,
check the compositional property, and explain any changed bound beside the test.
Do not raise a limit merely to pass CI. Run feature-sensitive client layouts in
default, plugins/lifecycle and the native feature set printed by xtask. Run the
32-bit sender-key layout tests on a supported 32-bit target after changing them.

`.github/workflows/binary-size.yml` measures the linked default-feature demo,
not rlibs. Local measurement uses `cargo xt ci measure-binary-size --out-dir DIR`;
comparison uses `cargo xt ci binary-size-report --head DIR --base DIR --out-dir DIR`.
Base SHA, toolchain, successful measurement/upload and artifact metadata must
match. Missing or incomparable baselines fail; never substitute older main.
The gate reports stripped bytes, text, allocated sections and dependencies.
Explain accepted costs in the PR; use the existing label policy only for a
reviewed increase. A toolchain change needs a matching baseline dispatch on main.

For plugin cost, build `plugins/wam`'s `size_probe_without_wam` and
`size_probe_with_wam` in release with `CARGO_PROFILE_RELEASE_STRIP=false`
and the same lock, toolchain and flags. Keep
symbols in the original binaries, compare `.text` with `size`, then run
`strip --strip-all` on copies and compare their byte lengths. The first probe
already enables the host; the delta measures installing WAM. This comparison
supplements the shipping demo gate and does not measure runtime memory.

## Unsafe and capture-backed checks

Changes to `Yokeable`/`StableDeref` or zlib `set_len` require the Miri gate.
Locally use `cargo miri test -p wacore-binary --lib`; large fixtures need a small
Miri twin rather than removing coverage.

Use `cargo xt oracle conformance`, adding `--slow` for serialized signaling.
Restore exact captures with `cargo xt oracle fetch`; `wasm.lock.json` and
`mlow.lock.json` pin inputs/outputs. Run host execution in release. See
`tools/oracle-core/AGENTS.md` and its README for selectors, ABI and commands.
Missing captures may skip locally; a skip establishes no conformance.

JS IQ schemas, wasm signaling, MLOW bytes, media callbacks, packetization and
crypto round trips have different owners. A schema check is not execution of
all IQs. MediaWatch/compare-media infrastructure does not establish callback ABI
coverage, and ignored full-call scenarios do not prove end-to-end parity.
Provenance lives beside `wacore/src/voip/mlow/testdata` and the capture locks.

For binary debugging, use the parser and examples in `wacore/binary`. Use fictitious identities
in fixtures and retain hashes when deriving vectors from captured artifacts.
