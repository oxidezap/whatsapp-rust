#!/usr/bin/env python3
"""Measure binary-size metrics for the size-tracking CI job.

Builds the `whatsapp-rust` bin with the real release profile (fat LTO,
codegen-units=1) but with symbols kept, then derives every metric from that
single build:

  - stripped file size (shipping proxy), measured on a stripped copy
  - .text section size (invariant to strip; the monomorphization signal)
  - total allocated size (text+data+bss; catches static data from new deps)
  - per-crate .text attribution via `cargo bloat --crates`
  - LLVM IR lines/copies via `cargo llvm-lines` (pre-link monomorphization)
  - crate count from Cargo.lock (new-dependency canary)

Outputs in --out-dir:
  size-metrics.json      customSmallerIsBetter array for github-action-benchmark
  size-attribution.json  raw `cargo bloat --crates` JSON for per-crate diffing
  size-meta.json         commit/toolchain provenance for the PR report footer
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

BIN_NAME = "whatsapp-rust"
# Workspace crates tracked as individual series; everything else is summed
# into "other deps" so the series set stays stable as dependencies churn.
WORKSPACE_CRATES = [
    "whatsapp_rust",
    "wacore",
    "wacore_binary",
    "wacore_libsignal",
    "wacore_appstate",
    "wacore_noise",
    "waproto",
    "whatsapp_rust_sqlite_storage",
    "whatsapp_rust_tokio_transport",
    "whatsapp_rust_ureq_http_client",
    "std",
]
LLVM_LINES_TARGETS = [
    ("wacore", "wacore"),
    ("whatsapp-rust", "whatsapp-rust lib"),
]


def run(cmd, **kwargs):
    print(f"+ {' '.join(cmd)}", file=sys.stderr, flush=True)
    return subprocess.run(cmd, check=True, **kwargs)


def build_env():
    env = os.environ.copy()
    # cargo-bloat needs symbols; it injects this same value into its own
    # build, so setting it here keeps the fingerprint identical and the
    # `cargo bloat` invocation below reuses this build instead of recompiling.
    env["CARGO_PROFILE_RELEASE_STRIP"] = "false"
    return env


def measure_stripped_size(bin_path: Path, out_dir: Path) -> int:
    stripped = out_dir / f"{BIN_NAME}-stripped"
    shutil.copy2(bin_path, stripped)
    run(["strip", "--strip-all", str(stripped)])
    size = stripped.stat().st_size
    stripped.unlink()
    return size


def measure_sections(bin_path: Path) -> tuple[int, int]:
    sysv = run(["size", "-A", "-d", str(bin_path)], capture_output=True, text=True).stdout
    text_size = None
    for line in sysv.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[0] == ".text":
            text_size = int(parts[1])
            break
    if text_size is None:
        raise RuntimeError("could not find .text in `size -A` output")

    berkeley = run(["size", "-d", str(bin_path)], capture_output=True, text=True).stdout
    lines = berkeley.strip().splitlines()
    # header: text data bss dec hex filename
    allocated = int(lines[1].split()[3])
    return text_size, allocated


def run_cargo_bloat(env) -> dict:
    proc = run(
        ["cargo", "bloat", "--release", "--bin", BIN_NAME, "--crates",
         "--message-format", "json", "-n", "0"],
        capture_output=True, text=True, env=env,
    )
    # cargo-bloat exits 0 even on analysis errors, so validate the payload.
    try:
        data = json.loads(proc.stdout)
    except json.JSONDecodeError as e:
        print(proc.stdout[:2000], file=sys.stderr)
        raise RuntimeError("cargo bloat did not produce valid JSON") from e
    if "crates" not in data or not data["crates"]:
        raise RuntimeError(f"cargo bloat JSON has no crate data: {list(data)}")
    return data


def run_llvm_lines(package: str, env) -> tuple[int, int]:
    proc = run(
        ["cargo", "llvm-lines", "-p", package, "--lib", "--release"],
        capture_output=True, text=True, env=env,
    )
    for line in proc.stdout.splitlines():
        parts = line.split()
        # "  634926  16990  (TOTAL)" (data rows carry extra percent columns)
        if parts and parts[-1] == "(TOTAL)":
            nums = [p for p in parts if p.isdigit()]
            return int(nums[0]), int(nums[1])
    raise RuntimeError(f"no (TOTAL) row in cargo llvm-lines output for {package}")


def count_lockfile_crates() -> int:
    count = 0
    for line in Path("Cargo.lock").read_text().splitlines():
        if line.startswith("name = "):
            count += 1
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", default=".size-out")
    parser.add_argument("--skip-build", action="store_true",
                        help="reuse an existing release build (local iteration)")
    args = parser.parse_args()

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    env = build_env()

    if not args.skip_build:
        run(["cargo", "build", "--release", "--locked", "--bin", BIN_NAME], env=env)

    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    bin_path = target_dir / "release" / BIN_NAME
    if not bin_path.exists():
        raise RuntimeError(f"binary not found at {bin_path}")

    stripped_size = measure_stripped_size(bin_path, out_dir)
    text_size, allocated = measure_sections(bin_path)
    bloat = run_cargo_bloat(env)
    lock_crates = count_lockfile_crates()

    metrics = [
        {"name": "bin size (stripped)", "unit": "bytes", "value": stripped_size},
        {"name": "bin .text", "unit": "bytes", "value": text_size},
        {"name": "bin allocated (text+data+bss)", "unit": "bytes", "value": allocated},
    ]

    crate_sizes = {c["name"]: c["size"] for c in bloat["crates"]}
    other = 0
    for name, size in crate_sizes.items():
        if name not in WORKSPACE_CRATES:
            other += size
    for name in WORKSPACE_CRATES:
        if name in crate_sizes:
            metrics.append({"name": f".text {name}", "unit": "bytes",
                            "value": crate_sizes[name]})
    metrics.append({"name": ".text other deps", "unit": "bytes", "value": other})

    for package, label in LLVM_LINES_TARGETS:
        lines, copies = run_llvm_lines(package, env)
        metrics.append({"name": f"llvm-lines {label}", "unit": "lines", "value": lines})
        metrics.append({"name": f"llvm-lines {label} copies", "unit": "copies",
                        "value": copies})

    metrics.append({"name": "deps crates (Cargo.lock)", "unit": "crates",
                    "value": lock_crates})

    commit = run(["git", "rev-parse", "HEAD"], capture_output=True,
                 text=True).stdout.strip()
    rustc = run(["rustc", "--version"], capture_output=True, text=True).stdout.strip()
    meta = {"commit": commit, "rustc": rustc}

    (out_dir / "size-metrics.json").write_text(json.dumps(metrics, indent=1) + "\n")
    (out_dir / "size-attribution.json").write_text(json.dumps(bloat) + "\n")
    (out_dir / "size-meta.json").write_text(json.dumps(meta, indent=1) + "\n")

    for m in metrics:
        print(f"{m['name']}: {m['value']} {m['unit']}")


if __name__ == "__main__":
    main()
