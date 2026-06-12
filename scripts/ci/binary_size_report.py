#!/usr/bin/env python3
"""Build the binary-size PR report and evaluate the regression gate.

Compares the metrics measured on the PR head against the baseline artifact
from the latest successful main run. Binary size is deterministic for a
pinned toolchain, so deltas are exact (no statistical noise): the gate is an
absolute byte budget (Chromium-style) instead of a percentage, because on a
multi-MiB binary a percent threshold lets small-but-real regressions through.

Writes:
  report.md   sticky PR comment body (also used for the job summary)
  gate.txt    PASS | FAIL | OVERRIDDEN, followed by one reason per line

Always exits 0; the workflow enforces the gate from gate.txt so the comment
gets posted even when the budget is exceeded.
"""

import argparse
import json
import os
from pathlib import Path

# Absolute budgets per PR, mirroring Chromium's per-CL allowance. Expected
# jumps (toolchain/deps bumps) go through the `size-increase-ok` label.
FAIL_STRIPPED_DELTA = 64 * 1024
FAIL_TEXT_DELTA = 32 * 1024
WARN_PCT = 1.0
# Attribution rows smaller than this are noise from cargo-bloat's heuristics.
MOVER_MIN_DELTA = 1024
MOVER_LIMIT = 10

MARKER = "<!-- binary-size-report -->"
GRAPHS_URL = "https://oxidezap.github.io/whatsapp-rust/dev/binary-size/"

GATED = {
    "bin size (stripped)": FAIL_STRIPPED_DELTA,
    "bin .text": FAIL_TEXT_DELTA,
}


def human_bytes(n: int) -> str:
    sign = "-" if n < 0 else ""
    n = abs(n)
    for unit, factor in (("MiB", 1024**2), ("KiB", 1024)):
        if n >= factor:
            return f"{sign}{n / factor:.2f} {unit}"
    return f"{sign}{n} B"


def fmt_value(value: int, unit: str) -> str:
    if unit == "bytes":
        return human_bytes(value)
    return f"{value:,}"


def fmt_delta(delta: int, base: int, unit: str) -> str:
    if delta == 0:
        return "0"
    pct = f" ({delta / base:+.2%})" if base else ""
    if unit == "bytes":
        s = human_bytes(delta)
        return f"{'+' + s if delta > 0 else s}{pct}"
    return f"{delta:+,}{pct}"


def load_metrics(path: Path) -> dict:
    data = json.loads(path.read_text())
    return {m["name"]: m for m in data}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--head", required=True, help="dir with the PR head outputs")
    parser.add_argument("--base", help="dir with the baseline (main) outputs")
    parser.add_argument("--out-dir", default=".")
    args = parser.parse_args()

    head_dir, out_dir = Path(args.head), Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    head = load_metrics(head_dir / "size-metrics.json")
    head_meta = json.loads((head_dir / "size-meta.json").read_text())
    # Otherwise dropping a gated row from the measure script would silently
    # disarm the budget. The baseline stays lenient: a PR introducing a new
    # gated metric can't have it in main's artifact yet.
    missing = sorted(name for name in GATED if name not in head)
    if missing:
        raise SystemExit(f"head metrics missing gated rows: {', '.join(missing)}")

    base = base_meta = None
    if args.base:
        base_dir = Path(args.base)
        if (base_dir / "size-metrics.json").exists():
            base = load_metrics(base_dir / "size-metrics.json")
            meta_path = base_dir / "size-meta.json"
            if meta_path.exists():
                base_meta = json.loads(meta_path.read_text())

    failures = []
    lines = [MARKER, "## 📦 Binary size report", ""]

    if base is None:
        lines.append("No baseline available yet (no successful run on main); reporting absolute values only.")
        lines.extend(["", "| Metric | PR |", "|---|---:|"])
        for name, m in head.items():
            if name.startswith(".text "):
                continue
            lines.append(f"| {name} | {fmt_value(m['value'], m['unit'])} |")
        status = "PASS"
    else:
        main_rows, crate_rows = [], []
        for name, m in head.items():
            unit = m["unit"]
            b = base.get(name)
            if b is None:
                row = f"| {name} | (new) | {fmt_value(m['value'], unit)} | |"
            else:
                delta = m["value"] - b["value"]
                icon = ""
                if name in GATED and delta > GATED[name]:
                    failures.append(
                        f"{name}: {fmt_delta(delta, b['value'], unit)} exceeds the "
                        f"{human_bytes(GATED[name])} per-PR budget"
                    )
                    icon = " 🚨"
                elif b["value"] and abs(delta) / b["value"] * 100 >= WARN_PCT:
                    icon = " ⚠️" if delta > 0 else " 🎉"
                elif delta > 0:
                    icon = " 🔺"
                elif delta < 0:
                    icon = " 🔽"
                row = (f"| {name} | {fmt_value(b['value'], unit)} | "
                       f"{fmt_value(m['value'], unit)} | "
                       f"{fmt_delta(delta, b['value'], unit)}{icon} |")
            (crate_rows if name.startswith(".text ") else main_rows).append(row)

        lines.extend(["| Metric | main | PR | Δ |", "|---|---:|---:|---:|"])
        lines.extend(main_rows)
        lines.extend(["", "<details>", "<summary>.text per crate</summary>", "",
                      "| Crate | main | PR | Δ |", "|---|---:|---:|---:|"])
        lines.extend(crate_rows)
        lines.extend(["", "</details>"])

        movers = top_movers(head_dir, Path(args.base))
        if movers:
            lines.extend(["", "<details>", "<summary>Top movers (cargo-bloat attribution)</summary>",
                          "", "| Crate | main | PR | Δ |", "|---|---:|---:|---:|"])
            lines.extend(movers)
            lines.extend(["", "</details>"])

        status = "FAIL" if failures else "PASS"

    if failures:
        lines.extend(["", f"🚨 Per-PR size budget exceeded (Δ stripped ≤ {human_bytes(FAIL_STRIPPED_DELTA)}, Δ .text ≤ {human_bytes(FAIL_TEXT_DELTA)}):"])
        lines.extend(f"- {f}" for f in failures)
        if os.environ.get("ALLOW_SIZE_INCREASE") == "true":
            status = "OVERRIDDEN"
            lines.append("")
            lines.append("The `size-increase-ok` label is set, so the gate is not enforced for this PR.")
        else:
            lines.append("")
            lines.append("If this increase is expected (toolchain or dependency bump, accepted feature cost), add the `size-increase-ok` label and re-run the failed job.")

    footer = [
        "",
        f"Baseline: `{(base_meta or {}).get('commit', 'n/a')[:9]}` (latest main run)"
        f" · Head: `{head_meta['commit'][:9]}` · [Graphs]({GRAPHS_URL})",
    ]
    lines.extend(footer)

    (out_dir / "report.md").write_text("\n".join(lines) + "\n")
    (out_dir / "gate.txt").write_text("\n".join([status, *failures]) + "\n")
    print(f"gate: {status}")
    for f in failures:
        print(f"  {f}")


def top_movers(head_dir: Path, base_dir: Path):
    try:
        head_crates = {c["name"]: c["size"] for c in
                       json.loads((head_dir / "size-attribution.json").read_text())["crates"]}
        base_crates = {c["name"]: c["size"] for c in
                       json.loads((base_dir / "size-attribution.json").read_text())["crates"]}
    except (OSError, KeyError, json.JSONDecodeError):
        return []

    rows = []
    for name in head_crates.keys() | base_crates.keys():
        h, b = head_crates.get(name), base_crates.get(name)
        delta = (h or 0) - (b or 0)
        if abs(delta) < MOVER_MIN_DELTA:
            continue
        rows.append((abs(delta), name, b, h, delta))
    rows.sort(reverse=True)

    out = []
    for _, name, b, h, delta in rows[:MOVER_LIMIT]:
        base_s = human_bytes(b) if b is not None else "(absent)"
        head_s = human_bytes(h) if h is not None else "(removed)"
        out.append(f"| {name} | {base_s} | {head_s} | {fmt_delta(delta, b or 0, 'bytes')} |")
    return out


if __name__ == "__main__":
    main()
