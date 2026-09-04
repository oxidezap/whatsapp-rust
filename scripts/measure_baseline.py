#!/usr/bin/env python3
"""Run baseline benchmarks with CPU pinning and statistical aggregation across rounds."""

import argparse
import hashlib
import os
import platform
from pathlib import Path
import json
import re
import subprocess
from typing import Dict, List, Any

NUM_RE = re.compile(r"([0-9]+(?:\.[0-9]+)?\s+(?:ns|µs|us|ms|s))$")


def parse_time_to_ns(time_str: str) -> float:
    parts = time_str.strip().split()
    if len(parts) != 2:
        raise ValueError(f"Invalid time: {time_str!r}")
    val, unit = float(parts[0]), parts[1]
    if unit == "ns":
        return val
    elif unit == "µs" or unit == "us":
        return val * 1_000.0
    elif unit == "ms":
        return val * 1_000_000.0
    elif unit == "s":
        return val * 1_000_000_000.0
    raise ValueError(f"Unknown time unit: {unit!r}")


def format_ns(ns: float) -> str:
    if ns < 1_000:
        return f"{ns:.2f} ns"
    elif ns < 1_000_000:
        return f"{ns / 1_000.0:.2f} µs"
    elif ns < 1_000_000_000:
        return f"{ns / 1_000_000.0:.3f} ms"
    else:
        return f"{ns / 1_000_000_000.0:.3f} s"


def run_command(cmd: List[str]) -> str:
    res = subprocess.run(cmd, capture_output=True, text=True, check=True)
    return res.stdout


def parse_divan_output(output: str) -> Dict[str, Dict[str, Any]]:
    results = {}
    path_stack = []  # List of (indent_level, name)

    for line in output.splitlines():
        if not line.strip() or "Timer precision" in line:
            continue

        parts = line.split(" │ ")
        first_col_raw = parts[0]
        cols = [c.strip() for c in parts]

        if "fastest" in first_col_raw:
            root_name = first_col_raw.replace("fastest", "").strip()
            path_stack = [(0, root_name)]
            continue

        pos = -1
        for i, ch in enumerate(first_col_raw):
            if ch in ("├", "╰"):
                pos = i
                break
        depth = (pos // 3 + 1) if pos >= 0 else 0

        m = NUM_RE.search(first_col_raw.rstrip())
        if m and len(cols) >= 6:
            name_raw = first_col_raw[:m.start()].strip(" │├╰─")
            while path_stack and path_stack[-1][0] >= depth:
                path_stack.pop()
            full_name = "::".join([p[1] for p in path_stack] + [name_raw])
            results[full_name] = {
                "name": full_name,
                "fastest_raw": m.group(1),
                "slowest_raw": cols[1],
                "median_raw": cols[2],
                "mean_raw": cols[3],
                "fastest_ns": parse_time_to_ns(m.group(1)),
                "slowest_ns": parse_time_to_ns(cols[1]),
                "median_ns": parse_time_to_ns(cols[2]),
                "mean_ns": parse_time_to_ns(cols[3]),
                "samples": int(cols[4]),
                "iters": int(cols[5]),
            }
        else:
            name_raw = first_col_raw.strip(" │├╰─")
            if name_raw:
                while path_stack and path_stack[-1][0] >= depth:
                    path_stack.pop()
                path_stack.append((depth, name_raw))

    return results


def run_benchmark_rounds(
    bin_path: str,
    filter_args: List[str],
    cpu_cores: str,
    rounds: int,
    raw_dir: Path | None = None,
) -> Dict[str, List[Dict[str, Any]]]:
    all_runs: Dict[str, List[Dict[str, Any]]] = {}
    for r in range(rounds):
        cmd = ["taskset", "-c", cpu_cores, bin_path, "--bench"] + filter_args
        out = run_command(cmd)
        if raw_dir is not None:
            raw_dir.mkdir(parents=True, exist_ok=True)
            (raw_dir / f"round-{r + 1}.txt").write_text(out)
        parsed = parse_divan_output(out)
        if not parsed:
            raise ValueError(f"Round {r + 1}: no benchmark results; check filters and output format")
        if all_runs and set(parsed) != set(all_runs):
            raise ValueError(f"Round {r + 1}: benchmark set changed")
        for name, data in parsed.items():
            if name not in all_runs:
                all_runs[name] = []
            all_runs[name].append(data)
    return all_runs


def aggregate_runs(runs_by_bench: Dict[str, List[Dict[str, Any]]]) -> Dict[str, Any]:
    summary = {}
    for name, runs in runs_by_bench.items():
        medians = [r["median_ns"] for r in runs]
        fastests = [r["fastest_ns"] for r in runs]
        slowests = [r["slowest_ns"] for r in runs]

        medians_sorted = sorted(medians)
        mid = len(medians_sorted) // 2
        median_of_medians = (
            (medians_sorted[mid - 1] + medians_sorted[mid]) / 2.0
            if len(medians_sorted) % 2 == 0
            else medians_sorted[mid]
        )

        min_median = min(medians)
        max_median = max(medians)
        spread_pct = ((max_median - min_median) / median_of_medians * 100.0) if median_of_medians > 0 else 0.0

        summary[name] = {
            "rounds": len(runs),
            "median_of_medians_ns": median_of_medians,
            "median_of_medians_fmt": format_ns(median_of_medians),
            "min_median_fmt": format_ns(min_median),
            "max_median_fmt": format_ns(max_median),
            "spread_pct": round(spread_pct, 2),
            "best_fastest_fmt": format_ns(min(fastests)),
            "worst_slowest_fmt": format_ns(max(slowests)),
            "runs": runs,
        }
    return summary


def main():
    parser = argparse.ArgumentParser(description="Run baseline benchmarks with CPU pinning")
    parser.add_argument("--bin", required=True, help="Path to compiled benchmark binary")
    parser.add_argument("--filter", nargs="*", default=[], help="Filter expressions")
    parser.add_argument("--cpus", required=True, help="Allowed CPUs verified against this host topology")
    parser.add_argument("--rounds", type=int, default=3, help="Number of benchmark rounds (default: 3)")
    parser.add_argument("--raw-dir", type=Path, help="Save raw Divan stdout for each round")
    parser.add_argument("--bench-args", nargs=argparse.REMAINDER, default=[], help="Additional Divan flags; put this option last")
    parser.add_argument("--json", action="store_true", help="Output JSON format")
    args = parser.parse_args()
    if args.rounds < 1:
        parser.error("--rounds must be positive")

    runs = run_benchmark_rounds(args.bin, args.filter + args.bench_args, args.cpus, args.rounds, args.raw_dir)
    agg = aggregate_runs(runs)

    if args.json:
        metadata = {"command": [args.bin, "--bench", *args.filter, *args.bench_args],
                    "cpus": args.cpus, "requested_rounds": args.rounds,
                    "platform": platform.platform(), "cwd": os.getcwd(),
                    "binary_sha256": hashlib.sha256(Path(args.bin).read_bytes()).hexdigest()}
        revision = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True)
        metadata["revision"] = revision.stdout.strip() if revision.returncode == 0 else None
        metadata["git_status"] = subprocess.run(
            ["git", "status", "--short"], capture_output=True, text=True).stdout
        print(json.dumps({"metadata": metadata, "benchmarks": agg}, indent=2))
    else:
        print(f"| Benchmark | Rounds | Median (of medians) | Min Median | Max Median | Spread | Best Fastest |")
        print(f"|---|---:|---:|---:|---:|---:|---:|")
        for name, d in sorted(agg.items()):
            print(
                f"| `{name}` | {d['rounds']} | {d['median_of_medians_fmt']} | "
                f"{d['min_median_fmt']} | {d['max_median_fmt']} | {d['spread_pct']}% | {d['best_fastest_fmt']} |"
            )


if __name__ == "__main__":
    main()
