//! Binary-size measurements and absolute-budget reporting, shared by all CI entry points.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, read_json, run, write, write_json};
const CRATES: &[&str] = &[
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
];
const GATED: &[(&str, i64)] = &[("bin size (stripped)", 64 * 1024), ("bin .text", 32 * 1024)];
#[derive(Clone, Deserialize, Serialize)]
struct Metric {
    name: String,
    unit: String,
    value: i64,
}
fn command(root: &Path, program: &str, args: &[&str]) -> Command {
    let mut c = Command::new(program);
    c.args(args)
        .current_dir(root)
        .env("CARGO_PROFILE_RELEASE_STRIP", "false");
    c
}
fn text(root: &Path, program: &str, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8(
        capture(&mut command(root, program, args))?.stdout,
    )?)
}
fn metric(name: impl Into<String>, unit: &str, value: i64) -> Metric {
    Metric {
        name: name.into(),
        unit: unit.into(),
        value,
    }
}
fn sections(sysv: &str, berkeley: &str) -> Result<(i64, i64)> {
    let text = sysv
        .lines()
        .find_map(|line| {
            let p = line.split_whitespace().collect::<Vec<_>>();
            (p.first() == Some(&".text"))
                .then(|| p.get(1).and_then(|n| n.parse::<i64>().ok()))
                .flatten()
        })
        .context(".text missing in size output")?;
    let allocated = berkeley
        .lines()
        .nth(1)
        .and_then(|l| l.split_whitespace().nth(3))
        .context("allocated size missing")?
        .parse()?;
    Ok((text, allocated))
}
fn llvm_total(output: &str) -> Result<(i64, i64)> {
    for line in output.lines() {
        let p = line.split_whitespace().collect::<Vec<_>>();
        if p.last() == Some(&"(TOTAL)") {
            let n = p
                .iter()
                .filter_map(|n| n.parse::<i64>().ok())
                .collect::<Vec<_>>();
            ensure!(n.len() >= 2, "incomplete LLVM total");
            return Ok((n[0], n[1]));
        }
    }
    anyhow::bail!("no TOTAL row in cargo llvm-lines output")
}
pub fn measure(root: &Path, out: &Path, skip: bool) -> Result<()> {
    std::fs::create_dir_all(out)?;
    if !skip {
        run(&mut command(
            root,
            "cargo",
            &["build", "--release", "--locked", "--example", "demo"],
        ))?;
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or(root.join("target"));
    let binary = target.join("release/examples/demo");
    ensure!(
        binary.is_file(),
        "demo binary missing: {}",
        binary.display()
    );
    let temporary = tempfile::NamedTempFile::new_in(out)?;
    std::fs::copy(&binary, temporary.path())?;
    run(Command::new("strip")
        .arg("--strip-all")
        .arg(temporary.path()))?;
    let stripped = temporary.as_file().metadata()?.len();
    let bin = binary.to_str().context("binary path encoding")?;
    let (text_size, allocated) = sections(
        &text(root, "size", &["-A", "-d", bin])?,
        &text(root, "size", &["-d", bin])?,
    )?;
    let bloat: Value = serde_json::from_str(&text(
        root,
        "cargo",
        &[
            "bloat",
            "--release",
            "--example",
            "demo",
            "--crates",
            "--message-format",
            "json",
            "-n",
            "0",
        ],
    )?)?;
    let rows = bloat["crates"]
        .as_array()
        .filter(|r| !r.is_empty())
        .context("cargo bloat returned no crate data")?;
    let mut sizes = BTreeMap::new();
    for c in rows {
        sizes.insert(
            c["name"].as_str().context("crate name")?,
            c["size"].as_i64().context("crate size")?,
        );
    }
    let mut metrics = vec![
        metric("bin size (stripped)", "bytes", i64::try_from(stripped)?),
        metric("bin .text", "bytes", text_size),
        metric("bin allocated (text+data+bss)", "bytes", allocated),
    ];
    for name in CRATES {
        if let Some(size) = sizes.get(name) {
            metrics.push(metric(format!(".text {name}"), "bytes", *size));
        }
    }
    metrics.push(metric(
        ".text other deps",
        "bytes",
        sizes
            .iter()
            .filter(|(name, _)| !CRATES.contains(name))
            .map(|(_, v)| v)
            .sum(),
    ));
    for (package, label) in [("wacore", "wacore"), ("whatsapp-rust", "whatsapp-rust lib")] {
        let (lines, copies) = llvm_total(&text(
            root,
            "cargo",
            &["llvm-lines", "-p", package, "--lib", "--release"],
        )?)?;
        metrics.push(metric(format!("llvm-lines {label}"), "lines", lines));
        metrics.push(metric(
            format!("llvm-lines {label} copies"),
            "copies",
            copies,
        ));
    }
    let count = std::fs::read_to_string(root.join("Cargo.lock"))?
        .lines()
        .filter(|l| l.starts_with("name = "))
        .count();
    metrics.push(metric(
        "deps crates (Cargo.lock)",
        "crates",
        i64::try_from(count)?,
    ));
    let meta = json!({"commit":text(root,"git",&["rev-parse","HEAD"])?.trim(),"rustc":text(root,"rustc",&["--version"])?.trim()});
    write_json(&out.join("size-metrics.json"), &metrics)?;
    write_json(&out.join("size-attribution.json"), &bloat)?;
    write_json(&out.join("size-meta.json"), &meta)?;
    for m in metrics {
        println!("{}: {} {}", m.name, m.value, m.unit);
    }
    Ok(())
}
fn bytes(n: i64) -> String {
    let sign = if n < 0 { "-" } else { "" };
    let a = n.unsigned_abs();
    for (unit, factor) in [("MiB", 1024 * 1024), ("KiB", 1024)] {
        if a >= factor {
            return format!("{sign}{:.2} {unit}", a as f64 / factor as f64);
        }
    }
    format!("{sign}{a} B")
}
fn commas(n: i64) -> String {
    let digits = n.unsigned_abs().to_string();
    let mut result = String::new();
    if n < 0 {
        result.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}
fn value(n: i64, unit: &str) -> String {
    if unit == "bytes" { bytes(n) } else { commas(n) }
}
fn delta(n: i64, base: i64, unit: &str) -> String {
    if n == 0 {
        return "0".into();
    }
    let sign = if n > 0 { "+" } else { "" };
    let mut s = format!("{sign}{}", value(n, unit));
    if base != 0 {
        s.push_str(&format!(" ({:+.2}%)", n as f64 / base as f64 * 100.0));
    }
    s
}
fn metrics(directory: &Path) -> Result<Vec<Metric>> {
    Ok(serde_json::from_value(read_json(
        &directory.join("size-metrics.json"),
    )?)?)
}
fn movers(head: &Path, base: &Path) -> Result<Vec<String>> {
    let get = |path: &Path| -> Result<BTreeMap<String, i64>> {
        let v = read_json(&path.join("size-attribution.json"))?;
        v["crates"]
            .as_array()
            .context("attribution crates")?
            .iter()
            .map(|r| {
                Ok((
                    r["name"].as_str().context("name")?.into(),
                    r["size"].as_i64().context("size")?,
                ))
            })
            .collect()
    };
    let h = get(head)?;
    let b = get(base)?;
    let keys = h
        .keys()
        .chain(b.keys())
        .collect::<std::collections::BTreeSet<_>>();
    let mut rows = Vec::new();
    for key in keys {
        let hv = h.get(key).copied();
        let bv = b.get(key).copied();
        let d = hv.unwrap_or(0) - bv.unwrap_or(0);
        if d.unsigned_abs() >= 1024 {
            rows.push((d.unsigned_abs(), key.clone(), bv, hv, d));
        }
    }
    rows.sort_by(|a, b| b.cmp(a));
    Ok(rows
        .into_iter()
        .take(10)
        .map(|(_, name, b, h, d)| {
            format!(
                "| {name} | {} | {} | {} |",
                b.map(bytes).unwrap_or("(absent)".into()),
                h.map(bytes).unwrap_or("(removed)".into()),
                delta(d, b.unwrap_or(0), "bytes")
            )
        })
        .collect())
}
fn render(
    head: &[Metric],
    base: Option<&[Metric]>,
    allow: bool,
) -> Result<(Vec<String>, Vec<String>, String)> {
    for (name, _) in GATED {
        ensure!(
            head.iter().any(|m| m.name == *name),
            "head metrics missing gated row: {name}"
        );
    }
    let mut lines = vec![
        "<!-- binary-size-report -->".into(),
        "## 📦 Binary size report".into(),
        String::new(),
    ];
    let mut failures = Vec::new();
    if let Some(base) = base {
        let mut main_rows = Vec::new();
        let mut crate_rows = Vec::new();
        for m in head {
            let row = if let Some(b) = base.iter().find(|b| b.name == m.name) {
                let d = m.value - b.value;
                let icon = if let Some((_, budget)) = GATED
                    .iter()
                    .find(|(name, budget)| *name == m.name && d > *budget)
                {
                    failures.push(format!(
                        "{}: {} exceeds the {} per-PR budget",
                        m.name,
                        delta(d, b.value, &m.unit),
                        bytes(*budget)
                    ));
                    " 🚨"
                } else if b.value != 0 && (d as f64 / b.value as f64).abs() * 100.0 >= 1.0 {
                    if d > 0 { " ⚠️" } else { " 🎉" }
                } else if d > 0 {
                    " 🔺"
                } else if d < 0 {
                    " 🔽"
                } else {
                    ""
                };
                format!(
                    "| {} | {} | {} | {}{icon} |",
                    m.name,
                    value(b.value, &m.unit),
                    value(m.value, &m.unit),
                    delta(d, b.value, &m.unit)
                )
            } else {
                format!("| {} | (new) | {} | |", m.name, value(m.value, &m.unit))
            };
            if m.name.starts_with(".text ") {
                crate_rows.push(row);
            } else {
                main_rows.push(row);
            }
        }
        lines.extend([
            "| Metric | main | PR | Δ |".into(),
            "|---|---:|---:|---:|".into(),
        ]);
        lines.extend(main_rows);
        lines.extend([
            "".into(),
            "<details>".into(),
            "<summary>.text per crate</summary>".into(),
            "".into(),
            "| Crate | main | PR | Δ |".into(),
            "|---|---:|---:|---:|".into(),
        ]);
        lines.extend(crate_rows);
        lines.extend(["".into(), "</details>".into()]);
    } else {
        lines.extend(["No baseline available yet (no successful run on main); reporting absolute values only.".into(),"".into(),"| Metric | PR |".into(),"|---|---:|".into()]);
        for m in head {
            if !m.name.starts_with(".text ") {
                lines.push(format!("| {} | {} |", m.name, value(m.value, &m.unit)));
            }
        }
    }
    let status = if failures.is_empty() {
        "PASS"
    } else if allow {
        "OVERRIDDEN"
    } else {
        "FAIL"
    };
    Ok((lines, failures, status.into()))
}
pub fn report(head_dir: &Path, base_dir: Option<&Path>, out: &Path) -> Result<()> {
    let head = metrics(head_dir)?;
    let meta = read_json(&head_dir.join("size-meta.json"))?;
    let base = base_dir
        .filter(|p| p.join("size-metrics.json").exists())
        .map(metrics)
        .transpose()?;
    let base_meta = base_dir.and_then(|p| read_json(&p.join("size-meta.json")).ok());
    let allow = std::env::var("ALLOW_SIZE_INCREASE").as_deref() == Ok("true");
    let (mut lines, failures, status) = render(&head, base.as_deref(), allow)?;
    if let Some(base_dir) = base_dir
        && base.is_some()
        && let Ok(rows) = movers(head_dir, base_dir)
        && !rows.is_empty()
    {
        lines.extend([
            "".into(),
            "<details>".into(),
            "<summary>Top movers (cargo-bloat attribution)</summary>".into(),
            "".into(),
            "| Crate | main | PR | Δ |".into(),
            "|---|---:|---:|---:|".into(),
        ]);
        lines.extend(rows);
        lines.extend(["".into(), "</details>".into()]);
    }
    if !failures.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "🚨 Per-PR size budget exceeded (Δ stripped ≤ {}, Δ .text ≤ {}):",
            bytes(64 * 1024),
            bytes(32 * 1024)
        ));
        lines.extend(failures.iter().map(|f| format!("- {f}")));
        lines.push(String::new());
        lines.push(if allow{"The `size-increase-ok` label is set, so the gate is not enforced for this PR."}else{"If this increase is expected (toolchain or dependency bump, accepted feature cost), add the `size-increase-ok` label and re-run the failed job."}.into());
    }
    let baseline = base_meta
        .as_ref()
        .and_then(|v| v["commit"].as_str())
        .unwrap_or("n/a");
    let head = meta["commit"].as_str().context("head commit missing")?;
    lines.push(String::new());
    lines.push(format!("Baseline: `{}` (latest main run) · Head: `{}` · [Graphs](https://oxidezap.github.io/whatsapp-rust/dev/binary-size/)",baseline.chars().take(9).collect::<String>(),head.chars().take(9).collect::<String>()));
    write(&out.join("report.md"), (lines.join("\n") + "\n").as_bytes())?;
    let mut gate = vec![status.clone()];
    gate.extend(failures);
    write(&out.join("gate.txt"), (gate.join("\n") + "\n").as_bytes())?;
    println!("gate: {status}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_budget_boundary_and_override_are_preserved() {
        let base = vec![
            metric("bin size (stripped)", "bytes", 100000),
            metric("bin .text", "bytes", 50000),
        ];
        let mut head = base.clone();
        head[0].value += 65536;
        assert_eq!(render(&head, Some(&base), false).unwrap().2, "PASS");
        head[0].value += 1;
        assert_eq!(render(&head, Some(&base), false).unwrap().2, "FAIL");
        assert_eq!(render(&head, Some(&base), true).unwrap().2, "OVERRIDDEN");
        assert!(render(&head[..1], Some(&base), true).is_err());
    }
    #[test]
    fn parses_tool_outputs_and_refuses_missing_measurements() {
        assert_eq!(
            sections(
                ".text 456 0\n",
                "text data bss dec hex file\n456 4 8 468 1d4 demo\n"
            )
            .unwrap(),
            (456, 468)
        );
        assert!(sections(".data 4 0", "nonsense").is_err());
        assert_eq!(llvm_total(" 123 45 (TOTAL)\n").unwrap(), (123, 45));
        assert!(llvm_total("empty").is_err());
    }
}
