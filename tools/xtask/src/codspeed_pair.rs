//! Contracts for the fixed, upload-disabled A02 benchmark comparison.
use anyhow::{Context, Result, ensure};
use clap::{Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::fs::write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use xtask_support::capture;

// Isolated codec comparison: v38 and the candidate retain identical wire semantics.
// The same 67 benchmark lines are added to both sources.
const BASE: &str = "9ea2cbacaf920558d22d56bd501bd3072e0b97fa";
const HEAD: &str = env!("A02_PAIR_HEAD_SHA");
const CONTRACTS: &[&str] = &[
    "runtime.sha256",
    "cpu-contract.txt",
    "rustc.txt",
    "cargo-codspeed.txt",
    "build-env.txt",
];
const ENVIRONMENT: &[&str] = &[
    "MALLOC_ARENA_MAX",
    "MALLOC_MMAP_THRESHOLD_",
    "MALLOC_TRIM_THRESHOLD_",
    "MALLOC_TOP_PAD_",
    "GLIBC_TUNABLES",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "CARGO_INCREMENTAL",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
];
const REQUIRED: &[&str] = &[
    "bench_oneof_owned_encode",
    "bench_oneof_view_encode",
    "bench_oneof_owned_decode",
    "bench_oneof_view_decode",
];

#[derive(Clone, Copy, ValueEnum)]
pub enum Side {
    Base,
    Head,
}
impl Side {
    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Head => "head",
        }
    }
}
#[derive(Subcommand)]
pub enum Task {
    Preflight,
    Capture {
        #[arg(long, value_enum)]
        side: Side,
    },
    Validate,
    /// Validate an existing pair and print its captured counters, without rerunning it.
    Summarize,
    /// Print existing size artifact metadata and attribution without building.
    InspectSize {
        directory: PathBuf,
    },
}

fn env_path(key: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os(key).with_context(|| format!("missing {key}"))?,
    ))
}
fn output(program: &str, args: &[&str]) -> Result<Vec<u8>> {
    Ok(capture(Command::new(program).args(args))?.stdout)
}
fn version(result: &Output) -> Result<&'static str> {
    // 5.0.1 propagates clap's DisplayVersion through anyhow, then exits 1.
    ensure!(
        result.status.code() == Some(1)
            && result.stdout.is_empty()
            && std::str::from_utf8(&result.stderr)?.trim() == "cargo-codspeed 5.0.1",
        "unexpected cargo-codspeed version response: {result:?}"
    );
    Ok("cargo-codspeed 5.0.1\n")
}
fn contracts(out: &Path) -> Result<()> {
    std::fs::create_dir_all(out)?;
    write(
        out.join("runtime.sha256"),
        output(
            "sha256sum",
            &[
                "/lib/x86_64-linux-gnu/libc.so.6",
                "/lib/x86_64-linux-gnu/libm.so.6",
            ],
        )?,
    )?;
    let cpu = std::fs::read_to_string("/proc/cpuinfo")?;
    let mut lines: Vec<_> = cpu
        .lines()
        .filter(|line| {
            ["vendor_id", "model name", "flags"]
                .iter()
                .any(|prefix| line.starts_with(prefix))
        })
        .collect();
    lines.sort_unstable();
    lines.dedup();
    ensure!(!lines.is_empty(), "missing CPU contract");
    write(
        out.join("cpu-contract.txt"),
        format!("{}\n", lines.join("\n")),
    )?;
    write(out.join("rustc.txt"), output("rustc", &["-Vv"])?)?;
    write(
        out.join("cargo-codspeed.txt"),
        version(
            &Command::new("cargo")
                .args(["codspeed", "--version"])
                .output()?,
        )?,
    )?;
    let env: BTreeMap<_, _> = ENVIRONMENT
        .iter()
        .map(|key| {
            (
                *key,
                std::env::var_os(key).map(|value| format!("{value:?}")),
            )
        })
        .collect();
    write(out.join("build-env.txt"), serde_json::to_vec_pretty(&env)?)?;
    Ok(())
}
fn equal(left: &Path, right: &Path, name: &str) -> Result<()> {
    ensure!(
        std::fs::read(left.join(name))? == std::fs::read(right.join(name))?,
        "contract differs: {name}"
    );
    Ok(())
}
fn preflight_contracts(preflight: &Path, measured: &Path) -> Result<()> {
    for name in CONTRACTS {
        if *name != "build-env.txt" {
            equal(preflight, measured, name)?;
        }
    }
    type Environment = BTreeMap<String, Option<String>>;
    let mut expected: Environment =
        serde_json::from_slice(&std::fs::read(preflight.join("build-env.txt"))?)?;
    let actual: Environment =
        serde_json::from_slice(&std::fs::read(measured.join("build-env.txt"))?)?;
    // The pinned Valgrind Debian wrapper adds its debug directory; traced
    // execs prepend vgpreload_core. Retain every other pre-build setting.
    for name in ["LD_PRELOAD", "LD_LIBRARY_PATH"] {
        let original = expected
            .get(name)
            .context("missing loader environment contract")?
            .as_deref()
            .map(serde_json::from_str::<String>)
            .transpose()?
            .unwrap_or_default();
        let value = match name {
            "LD_PRELOAD" => {
                format!("/usr/libexec/valgrind/vgpreload_core-amd64-linux.so:{original}")
            }
            _ if original.is_empty() => "/usr/lib/debug".to_owned(),
            _ => format!("{original}:/usr/lib/debug"),
        };
        expected.insert(name.to_owned(), Some(format!("{value:?}")));
    }
    ensure!(
        expected == actual,
        "measurement environment differs from the pinned runner contract"
    );
    Ok(())
}
fn profile(directory: &Path) -> Result<PathBuf> {
    let profiles: Vec<_> = std::fs::read_dir(directory)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path.file_name().is_some_and(|name| {
                    let name = name.to_string_lossy();
                    name.starts_with("profile.") && name.ends_with(".out")
                })
        })
        .collect();
    ensure!(
        profiles.len() == 1,
        "expected one profile under {}, got {}",
        directory.display(),
        profiles.len()
    );
    Ok(profiles[0].clone())
}
fn measured(profile: &Path) -> Result<Vec<String>> {
    let log = std::fs::read_to_string(profile.join("runner.log"))?;
    let mut names: Vec<_> = log
        .lines()
        .filter_map(|line| {
            line.split_once("Measured: ")
                .map(|(_, name)| name.to_owned())
        })
        .collect();
    names.sort();
    ensure!(!names.is_empty(), "no measured benchmarks");
    for required in REQUIRED {
        ensure!(
            names.iter().any(|name| name.contains(required)),
            "missing measured benchmark: {required}"
        );
    }
    let instrumented = counters(profile)?;
    ensure!(
        names.iter().all(|name| instrumented.contains_key(name)),
        "missing nonempty instrumented benchmark profile"
    );
    Ok(names)
}

fn counters(profile: &Path) -> Result<BTreeMap<String, BTreeMap<String, u64>>> {
    let mut instrumented = BTreeMap::new();
    for entry in std::fs::read_dir(profile)? {
        let path = entry?.path();
        if !path.is_file()
            || !path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with(|c: char| c.is_ascii_digit()) && name.ends_with(".out")
            })
        {
            continue;
        }
        let text = std::fs::read_to_string(path)?;
        for part in text.split("part:").skip(1) {
            let name = part
                .lines()
                .find_map(|line| line.strip_prefix("desc: Trigger: Client Request: "));
            let events = part.lines().find_map(|line| line.strip_prefix("events: "));
            // Client-request dumps can have a zero header summary. Closing
            // totals contain the costs actually dumped for this benchmark.
            let totals = part.lines().find_map(|line| line.strip_prefix("totals:"));
            if let (Some(name), Some(events), Some(totals)) = (name, events, totals) {
                let costs: Vec<u64> = totals
                    .split_whitespace()
                    .map(str::parse)
                    .collect::<std::result::Result<_, _>>()?;
                let fields: Vec<_> = events.split_whitespace().collect();
                // Callgrind omits trailing zero costs.
                ensure!(
                    !costs.is_empty() && costs.len() <= fields.len(),
                    "profile event columns differ"
                );
                let counts: BTreeMap<_, _> = fields
                    .into_iter()
                    .enumerate()
                    .filter(|(_, field)| !matches!(*field, "sysTime" | "sysCpuTime"))
                    .map(|(index, field)| {
                        (field.to_owned(), costs.get(index).copied().unwrap_or(0))
                    })
                    .collect();
                if counts.get("Ir").is_some_and(|count| *count > 0) {
                    ensure!(
                        instrumented.insert(name.to_owned(), counts).is_none(),
                        "duplicate instrumented benchmark profile: {name}"
                    );
                }
            }
        }
    }
    Ok(instrumented)
}
fn validate(root: &Path) -> Result<()> {
    let base = profile(&root.join("base"))?;
    let head = profile(&root.join("head"))?;
    for (path, sha) in [(&base, BASE), (&head, HEAD)] {
        ensure!(
            std::fs::read_to_string(path.join("source-commit.txt"))?.trim() == sha,
            "unexpected measured source"
        );
        preflight_contracts(&root.join("preflight"), path)?;
        ensure!(
            std::fs::read_to_string(path.join("runner-version.txt"))?.trim()
                == "codspeed-runner 5.4.0",
            "unexpected CodSpeed runner version"
        );
    }
    let base_names = measured(&base)?;
    let head_names = measured(&head)?;
    ensure!(base_names == head_names, "measured benchmark sets differ");
    for side in ["base", "head"] {
        write(
            root.join(side).join("measured.txt"),
            format!("{}\n", base_names.join("\n")),
        )?;
    }
    for name in CONTRACTS.iter().copied().chain(["runner-version.txt"]) {
        equal(&base, &head, name)?;
    }
    Ok(())
}
pub fn run(task: Task) -> Result<()> {
    let workspace = env_path("GITHUB_WORKSPACE")?.canonicalize()?;
    let profiles = workspace.join("_a02_profiles");
    match task {
        Task::Preflight => {
            for program in ["cc", "c++", "cmake", "make", "perl", "pkg-config"] {
                capture(Command::new(program).arg("--version"))?;
            }
            let mut child = Command::new("cc")
                .args(["-x", "c", "-fsyntax-only", "-"])
                .stdin(Stdio::piped())
                .spawn()?;
            {
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .context("compiler stdin")?
                    .write_all(b"#include <stdlib.h>\n#include <pthread.h>\n")?;
            }
            ensure!(child.wait()?.success(), "native headers unavailable");
            for side in ["base", "head", "preflight"] {
                std::fs::create_dir_all(profiles.join(side))?;
            }
            ensure!(
                profiles.join("base").canonicalize()? != profiles.join("head").canonicalize()?,
                "profile directories alias"
            );
            contracts(&profiles.join("preflight"))?;
        }
        Task::Capture { side } => {
            ensure!(
                std::env::var("CODSPEED_SKIP_UPLOAD").as_deref() == Ok("true"),
                "uploads must remain disabled"
            );
            let destination = env_path("CODSPEED_PROFILE_FOLDER")?.canonicalize()?;
            let temp = env_path("TMPDIR")?.canonicalize()?;
            ensure!(
                destination.parent() == Some(temp.as_path())
                    && temp == profiles.join(side.name()).canonicalize()?,
                "profile escaped its side directory"
            );
            ensure!(
                std::env::current_dir()?.canonicalize()?
                    == workspace
                        .join("_a02_pair")
                        .join(side.name())
                        .canonicalize()?,
                "wrong benchmark checkout"
            );
            contracts(&destination)?;
            write(
                destination.join("source-commit.txt"),
                output("git", &["rev-parse", "HEAD"])?,
            )?;
            write(
                destination.join("lock.sha256"),
                output("sha256sum", &["Cargo.lock"])?,
            )?;
            write(
                destination.join("runner-version.txt"),
                output("codspeed", &["--version"])?,
            )?;
            write(
                destination.join("cpuinfo.txt"),
                std::fs::read("/proc/cpuinfo")?,
            )?;
            preflight_contracts(&profiles.join("preflight"), &destination)?;
            if matches!(side, Side::Head) {
                equal(
                    &profile(&profiles.join("base"))?,
                    &destination,
                    "runner-version.txt",
                )?;
            }
        }
        Task::Validate => validate(&profiles)?,
        Task::Summarize => {
            validate(&profiles)?;
            for side in ["base", "head"] {
                let path = profile(&profiles.join(side))?;
                let contracts: BTreeMap<_, _> = CONTRACTS
                    .iter()
                    .copied()
                    .chain(["source-commit.txt", "runner-version.txt"])
                    .map(|name| Ok((name, std::fs::read_to_string(path.join(name))?)))
                    .collect::<Result<_>>()?;
                println!(
                    "{}",
                    serde_json::json!({
                        "side": side,
                        "contracts": contracts,
                        "captured_callgrind_counters": counters(&path)?,
                        "measurement": "existing CodSpeed Simulation artifact; no new execution"
                    })
                );
            }
        }
        Task::InspectSize { directory } => {
            for name in [
                "size-meta.json",
                "size-metrics.json",
                "size-attribution.json",
            ] {
                let path = directory.join(name);
                if name == "size-attribution.json" && !path.exists() {
                    continue;
                }
                let contents: serde_json::Value = serde_json::from_slice(&std::fs::read(&path)?)?;
                println!(
                    "{}",
                    serde_json::json!({"artifact_file":path,"contents":contents})
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn version_accepts_only_the_pinned_display_version_failure() {
        use std::os::unix::process::ExitStatusExt;
        let mut result = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: vec![],
            stderr: b"cargo-codspeed 5.0.1\n\n".to_vec(),
        };
        assert!(version(&result).is_ok());
        result.stderr = b"cargo-codspeed 0.0.0\n".to_vec();
        assert!(version(&result).is_err());
        result.stderr = b"cargo-codspeed 5.0.1\n".to_vec();
        result.status = std::process::ExitStatus::from_raw(127 << 8);
        assert!(version(&result).is_err());
    }
    fn nonempty_profile() -> String {
        REQUIRED.iter().map(|name| format!(
            "part: 1\ndesc: Trigger: Client Request: {name}\nevents: Ir Dr\nsummary: 0\ntotals: 12\n"
        )).collect()
    }
    #[test]
    fn counters_use_closing_totals_and_reject_duplicate_measurements() {
        let directory = tempfile::tempdir().unwrap();
        write(directory.path().join("1.out"), nonempty_profile()).unwrap();
        let costs = counters(directory.path()).unwrap();
        assert_eq!(costs[REQUIRED[0]]["Ir"], 12);
        assert_eq!(costs[REQUIRED[0]]["Dr"], 0);
        write(
            directory.path().join("1.out"),
            format!(
                "part: 1\ndesc: Trigger: Client Request: {}\nevents: Ir sysTime Ct sysCpuTime Cl\nsummary: 0\ntotals: 12 1000 13 2000 14\n",
                REQUIRED[0]
            ),
        ).unwrap();
        let costs = counters(directory.path()).unwrap();
        assert_eq!(costs[REQUIRED[0]]["Ct"], 13);
        assert_eq!(costs[REQUIRED[0]]["Cl"], 14);
        assert!(!costs[REQUIRED[0]].contains_key("sysTime"));
        assert!(!costs[REQUIRED[0]].contains_key("sysCpuTime"));
        write(directory.path().join("2.out"), nonempty_profile()).unwrap();
        assert!(counters(directory.path()).is_err());
    }
    #[test]
    fn pair_rejects_changed_contracts_missing_measurements_and_empty_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let preflight = root.join("preflight");
        std::fs::create_dir(&preflight).unwrap();
        let env = BTreeMap::from([
            ("LD_PRELOAD", None),
            ("LD_LIBRARY_PATH", None),
            ("CARGO_INCREMENTAL", Some("\"0\"")),
        ]);
        for name in CONTRACTS {
            write(preflight.join(name), "same").unwrap();
        }
        write(
            preflight.join("build-env.txt"),
            serde_json::to_vec(&env).unwrap(),
        )
        .unwrap();
        let runtime_env = BTreeMap::from([
            (
                "LD_PRELOAD",
                Some("\"/usr/libexec/valgrind/vgpreload_core-amd64-linux.so:\""),
            ),
            ("LD_LIBRARY_PATH", Some("\"/usr/lib/debug\"")),
            ("CARGO_INCREMENTAL", Some("\"0\"")),
        ]);
        for (side, sha) in [("base", BASE), ("head", HEAD)] {
            let path = root.join(side).join("profile.1.out");
            std::fs::create_dir_all(&path).unwrap();
            write(path.join("source-commit.txt"), sha).unwrap();
            for name in CONTRACTS.iter().copied().chain(["runner-version.txt"]) {
                write(path.join(name), "same").unwrap();
            }
            write(path.join("runner-version.txt"), "codspeed-runner 5.4.0\n").unwrap();
            write(
                path.join("build-env.txt"),
                serde_json::to_vec(&runtime_env).unwrap(),
            )
            .unwrap();
            write(
                path.join("runner.log"),
                REQUIRED
                    .iter()
                    .map(|name| format!("Measured: {name}\n"))
                    .collect::<String>(),
            )
            .unwrap();
            write(path.join("1.out"), nonempty_profile()).unwrap();
        }
        assert!(validate(root).is_ok());
        let head = root.join("head/profile.1.out");
        let mut changed = runtime_env.clone();
        changed.insert("LD_PRELOAD", Some("\"/tmp/other.so\""));
        write(
            head.join("build-env.txt"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        assert!(validate(root).is_err());
        changed = runtime_env.clone();
        changed.insert("CARGO_INCREMENTAL", Some("\"1\""));
        write(
            head.join("build-env.txt"),
            serde_json::to_vec(&changed).unwrap(),
        )
        .unwrap();
        assert!(validate(root).is_err());
        write(
            head.join("build-env.txt"),
            serde_json::to_vec(&runtime_env).unwrap(),
        )
        .unwrap();
        write(head.join("runner-version.txt"), "codspeed-runner 5.3.0").unwrap();
        assert!(validate(root).is_err());
        write(head.join("runner-version.txt"), "codspeed-runner 5.4.0").unwrap();
        write(head.join("runtime.sha256"), "different").unwrap();
        assert!(validate(root).is_err());
        write(head.join("runtime.sha256"), "same").unwrap();
        write(
            head.join("1.out"),
            nonempty_profile()
                .replace("summary: 0", "summary: 12")
                .replace("totals: 12", "totals: 0"),
        )
        .unwrap();
        assert!(validate(root).is_err());
        write(
            head.join("1.out"),
            nonempty_profile().replacen("totals: 12", "totals: 0", 1),
        )
        .unwrap();
        assert!(validate(root).is_err());
        write(head.join("1.out"), nonempty_profile()).unwrap();
        write(head.join("runner.log"), "Measured: unrelated\n").unwrap();
        assert!(validate(root).is_err());
        std::fs::create_dir(root.join("base/profile.2.out")).unwrap();
        assert!(profile(&root.join("base")).is_err());
    }
}
