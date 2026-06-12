# Binary Size CI

`binary-size.yml` tracks the size of the `whatsapp-rust` bin (default features, real release profile) on every PR and main push, so heavy new dependencies and monomorphization regressions surface before they accumulate.

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

- **PR gate** (`scripts/ci/binary_size_report.py`): absolute per-PR budget — stripped Δ ≤ 64 KiB, .text Δ ≤ 32 KiB. Absolute instead of percentage because sizes are deterministic for a pinned toolchain and 1% of a multi-MiB binary would hide real regressions. The sticky PR comment shows all deltas plus per-crate top movers.
- **Escape hatch**: the `size-increase-ok` label downgrades a failed gate to a warning. Use it for toolchain/dependency bumps and accepted feature costs; the increase still lands in the series.
- **Post-merge safety net**: the push job stores the series at `dev/binary-size` on gh-pages via github-action-benchmark (`alert-threshold: 102%` comments on the offending commit). Graphs: <https://oxidezap.github.io/whatsapp-rust/dev/binary-size/>.

## Baseline semantics and pitfalls

- The PR baseline is the `size-metrics` artifact from the **latest successful main run**, not the merge-base. A stale PR can therefore show deltas inherited from main; rebase to clear them.
- Metric names are series keys. Renaming one orphans its history in the chart, so keep names stable.
- Sizes are only comparable under the same pinned toolchain. A `rust-toolchain.toml` bump legitimately moves every metric — expect a gate hit and use the label.
- `cargo bloat` exits 0 even on analysis errors; the measure script validates its JSON instead of trusting the exit code.
- Fork PRs run with a read-only token: they get the job summary and the gate, but no PR comment.
- Local run: `python3 scripts/ci/measure_binary_size.py --out-dir size-out` (add `--skip-build` to reuse an existing release build).
