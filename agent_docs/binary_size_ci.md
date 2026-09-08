# Binary Size CI

`binary-size.yml` tracks the size of the `demo` example — the default-feature binary entry point that links the whole library, since the package no longer ships a bin — (default features, real release profile) on every PR and main push, so heavy new dependencies and monomorphization regressions surface before they accumulate.

## What is measured

All metrics come from one release build with symbols kept (`CARGO_PROFILE_RELEASE_STRIP=false`, which matches what cargo-bloat injects, so its run reuses the build):

- **bin size (stripped)** — shipping-size proxy, measured on a `strip --strip-all` copy.
- **bin .text** — invariant to strip; the signal for monomorphization bloat.
- **bin allocated (text+data+bss)** — catches static data tables that don't show in .text.
- **.text per crate** — `cargo bloat --crates` attribution (workspace crates + std as individual series, the rest aggregated as "other deps").
- **llvm-lines** (wacore, whatsapp-rust lib) — LLVM IR lines and monomorphization copies, pre-link and cheap.
- **deps crates (Cargo.lock)** — new-dependency canary.

Do NOT switch any metric to rlib size: rlibs carry un-monomorphized generics plus metadata, so cross-crate instantiation bloat (the dominant class found in the 2026-06 audit) never shows up there.

## How regressions are caught

- **PR gate** (`cargo xt ci binary-size-report`): absolute per-PR budget — stripped Δ ≤ 64 KiB, .text Δ ≤ 32 KiB. Absolute instead of percentage because sizes are deterministic for a pinned toolchain and 1% of a multi-MiB binary would hide real regressions. The sticky PR comment shows all deltas plus per-crate top movers.
- **Escape hatch**: the `size-increase-ok` label downgrades a failed gate to a warning. Use it for toolchain/dependency bumps and accepted feature costs; the increase still lands in the series.
- **Post-merge safety net**: the push job stores the series at `dev/binary-size` on gh-pages via github-action-benchmark (`alert-threshold: 102%` comments on the offending commit). Graphs: <https://oxidezap.github.io/whatsapp-rust/dev/binary-size/>.

## Baseline semantics and pitfalls

- The PR baseline must match the event's `pull_request.base.sha`. The selector searches this repository's Binary Size runs on main, including pushes and manual dispatches. It requires successful measurement and upload in the same job and run attempt, an unexpired artifact from that upload, matching commit metadata, and the same rustc version as the head. Graph publication may fail without invalidating a completed measurement.
- Missing, expired, malformed or mismatched baselines fail visibly. There is no fallback to older main and no passing gate without a comparison. Run Binary Size on the required main commit, then retry the PR job. A dispatch on a newer main commit cannot supply an older PR's baseline.
- Metric names are series keys. Renaming one orphans its history in the chart, so keep names stable.
- Sizes are only comparable under the same pinned toolchain. A toolchain change requires a base measurement with the matching compiler; `size-increase-ok` cannot override missing or incomparable measurements. The selector skips other-compiler candidates at the same base SHA, not metadata or provenance failures.
- `cargo bloat` exits 0 even on analysis errors; the measure script validates its JSON instead of trusting the exit code.
- Fork PRs run with a read-only token: they get the job summary and the gate, but no PR comment.
- CI invokes the dispatcher with `cargo run --locked --quiet -p whatsapp-xtask --`. The build, cargo-bloat and cargo-llvm-lines also use `--locked`. Measurement refuses lockfile changes, and CI checks the tracked worktree before uploading. Fix manifest/lock inconsistencies explicitly rather than letting measurement rewrite them and break the subsequent gh-pages checkout.
- Local measurement uses `cargo run --locked --quiet -p whatsapp-xtask -- ci measure-binary-size --out-dir size-out`. Add `--skip-build` to reuse an existing release build. Reports require `--base <artifact-directory>` or `BASE_DIR`; set `BASE_SHA` to also enforce the expected commit locally.

For a compiler-changing PR, dispatch a matching baseline on main. For example,
if the PR selects `nightly-2026-07-01`:

```sh
gh workflow run binary-size.yml --repo oxidezap/whatsapp-rust --ref main -f toolchain=nightly-2026-07-01
```

Wait for measurement and upload to succeed, then retry the PR job. If the PR's
base SHA is older than current main, rebase first. The dispatch measures current
main only, not an arbitrary ref. This workflow support must already be on main.

The optional input defaults to `nightly-2026-06-16`. Only dated nightly identifiers
are accepted because the build uses nightly-only flags. Input reaches the
validator through an environment variable, never shell interpolation. Bootstrap
and tool installation retain the default compiler; the measurement step sets
`RUSTUP_TOOLCHAIN` to the validated selection for Cargo, cargo-bloat,
cargo-llvm-lines and `rustc --version`. Metadata records that actual rustc version.
Alternate-compiler dispatches upload artifacts but skip gh-pages publication, so
they do not mix compilers in the normal graph series. Pushes and default-compiler
dispatches retain normal publication.

The stale-baseline failure on PR #1470 illustrates why publication status is not
measurement status. The parent `47e1b5b41` uploaded valid measurements, then graph
publication failed on a dirty `Cargo.lock`. The old successful-run selector used
`2b9a8d799` instead and charged the PR for growth already introduced by group
resync. Parent and PR artifacts had identical stripped and `.text` sizes. The
default-feature demo does not enable VoIP, so this says nothing about the size
cost of a VoIP parser in a VoIP-enabled binary.

## Per-crate opt-level

`[profile.release.package.*]` in the root `Cargo.toml` builds the off-hot-path crates at `opt-level = "s"`/`"z"` instead of 3. Crates that run per connection / per sync / per request rather than per message — persistence (Diesel/SQLite), media HTTP (ureq), TLS PKI — trade a little runtime speed for size; the per-message/per-frame crypto/protocol crates (libsignal, wacore-binary, waproto, prost, aes, sha2, hkdf, flate2, curve25519, and wacore-noise — whose `NoiseCipher` runs the transport AEAD on every frame) stay at 3. The overrides take effect under fat LTO and cut ~530 KiB off the stripped `demo` (~5%).

Pitfall: `cargo bloat`'s per-crate `.text` is attribution guesswork, and `opt-level` changes shift where LTO accounts for inlined/monomorphized code. After this change a few `.text <crate>` series move the "wrong" way (e.g. `whatsapp_rust_sqlite_storage` and `wacore_appstate` rise) even though the total falls. Trust `bin .text` and `bin size (stripped)` from `size`/`strip` — those are exact; the gate keys off them.
