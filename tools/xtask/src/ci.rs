//! CI decisions derived from metadata rather than duplicated shell lists.
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, write};

#[derive(Subcommand)]
pub enum Task {
    /// Additional first-party workflow tasks, including release preflight and reports.
    Workflow {
        #[command(subcommand)]
        task: super::workflow::Task,
    },
    /// Run both protobuf serde representations, keeping the first failure.
    TestWaprotoFeatures,
    /// Run combined compatible features for each package, keeping the first failure.
    TestFeaturePackages { packages: Vec<String> },
    /// Publishable workspace crates as a JSON array.
    FeatureMatrixCrates {
        #[arg(long)]
        github_output: bool,
    },
    /// Mutually compatible native test features, comma separated.
    TestFeatures { package: String },
    /// Preserve a nextest JUnit report and return the test process's exit status.
    NextestTimed {
        label: String,
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Check/write workflow literals from the canonical Bartender image pin.
    SyncBartenderImage {
        #[arg(long, conflicts_with = "write")]
        check: bool,
        #[arg(long)]
        write: bool,
    },
    /// Measure the release binary using Cargo, strip, size, bloat and llvm-lines.
    MeasureBinarySize {
        #[arg(long, default_value = ".size-out")]
        out_dir: PathBuf,
        #[arg(long)]
        skip_build: bool,
    },
    /// Write the size gate and report; budget failure stays in gate.txt for the workflow.
    BinarySizeReport {
        #[arg(long)]
        head: PathBuf,
        #[arg(long)]
        base: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
    },
}
fn metadata(root: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(
        &capture(
            Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(root),
        )?
        .stdout,
    )?)
}
fn github_output(key: &str, value: &str) -> Result<()> {
    use std::io::Write;
    ensure!(!value.contains(['\r', '\n']), "multiline Actions output");
    let path = std::env::var_os("GITHUB_OUTPUT").context("GITHUB_OUTPUT must be set")?;
    writeln!(
        std::fs::OpenOptions::new().append(true).open(path)?,
        "{key}={value}"
    )?;
    Ok(())
}
fn feature_names(meta: &Value, package: &str) -> Result<String> {
    let package = meta["packages"]
        .as_array()
        .context("packages")?
        .iter()
        .find(|p| p["name"] == package)
        .context("unknown package")?;
    let mut features = package["features"]
        .as_object()
        .context("features")?
        .keys()
        .filter(|s| {
            !s.starts_with("danger-skip-")
                && !matches!(
                    s.as_str(),
                    "default" | "dhat-heap" | "js" | "getrandom" | "tracing-pii"
                )
        })
        .cloned()
        .collect::<Vec<_>>();
    features.sort();
    Ok(features.join(","))
}
fn sync_bartender(root: &Path, update: bool) -> Result<()> {
    let image = std::fs::read_to_string(root.join(".github/bartender-image.txt"))?;
    let image = image.trim();
    let valid =
        regex::Regex::new(r"^ghcr\.io/whiskeysockets-devtools/bartender@sha256:[0-9a-f]{64}$")?;
    ensure!(valid.is_match(image), "invalid canonical Bartender image");
    let pattern = regex::Regex::new(
        r#"(?m)^(\s*image:\s+)(["']?)(ghcr\.io/whiskeysockets-devtools/bartender[^\s"']*)(["']?)([ \t]*)$"#,
    )?;
    let mut changes = Vec::new();
    for name in ["codspeed.yml", "copilot-setup-steps.yml", "e2e.yml"] {
        let path = root.join(".github/workflows").join(name);
        let text = std::fs::read_to_string(&path)?;
        let matches = pattern.captures_iter(&text).collect::<Vec<_>>();
        ensure!(matches.len() == 1, "{name}: expected exactly one image pin");
        let found = &matches[0];
        ensure!(found[2] == found[4], "{name}: mismatched image quotes");
        if !update {
            ensure!(
                &found[3] == image,
                "{name}: image drift; run cargo xt ci sync-bartender-image --write"
            );
        }
        let text = pattern
            .replace(&text, |c: &regex::Captures<'_>| {
                format!("{}{}{image}{}{}", &c[1], &c[2], &c[2], &c[5])
            })
            .into_owned();
        changes.push((path, text));
    }
    if update {
        for (path, text) in changes {
            write(&path, text.as_bytes())?;
        }
    }
    println!("Bartender image pins are synchronized (3 references).");
    Ok(())
}
fn timed(root: &Path, label: &str, args: &[String]) -> Result<u8> {
    let dir = PathBuf::from(
        std::env::var_os("TEST_TIMINGS_DIR").context("TEST_TIMINGS_DIR must be set")?,
    );
    let profile = std::env::var("NEXTEST_PROFILE").context("NEXTEST_PROFILE must be set")?;
    timed_report(root, &dir, &profile, label, || {
        Ok(Command::new("cargo")
            .args(["nextest", "run"])
            .args(args)
            .current_dir(root)
            .status()?)
    })
}
fn timed_report(
    root: &Path,
    directory: &Path,
    profile: &str,
    label: &str,
    execute: impl FnOnce() -> Result<std::process::ExitStatus>,
) -> Result<u8> {
    for name in [label, profile] {
        ensure!(
            !name.is_empty() && Path::new(name).file_name().and_then(|s| s.to_str()) == Some(name),
            "invalid report label/profile"
        );
    }
    let report = root.join(format!("target/nextest/{profile}/junit.xml"));
    std::fs::create_dir_all(directory)?;
    if report.exists() {
        std::fs::remove_file(&report)?;
    }
    let status = execute()?;
    if report.is_file() {
        std::fs::copy(&report, directory.join(format!("{label}.xml")))?;
        std::fs::remove_file(report)?;
    }
    Ok(xtask_support::exit_code(status))
}

pub fn run(root: &Path, task: Task) -> Result<u8> {
    match task {
        Task::Workflow { task } => super::workflow::run_task(root, task)?,
        Task::TestWaprotoFeatures => {
            let mut status = 0;
            for feature in ["serde-enum-repr", "serde-snake-case"] {
                let rc = timed(
                    root,
                    feature,
                    &[
                        "-p".into(),
                        "waproto".into(),
                        "--features".into(),
                        feature.into(),
                    ],
                )?;
                if status == 0 {
                    status = rc;
                }
            }
            return Ok(status);
        }
        Task::TestFeaturePackages { packages } => {
            ensure!(!packages.is_empty(), "at least one package is required");
            let meta = metadata(root)?;
            let mut status = 0;
            for package in packages {
                let features = feature_names(&meta, &package)?;
                let rc = timed(
                    root,
                    &package,
                    &[
                        "-p".into(),
                        package.clone(),
                        "--features".into(),
                        features,
                        "--lib".into(),
                        "--tests".into(),
                    ],
                )?;
                if status == 0 {
                    status = rc;
                }
            }
            return Ok(status);
        }
        Task::FeatureMatrixCrates {
            github_output: output,
        } => {
            let meta = metadata(root)?;
            let mut names = meta["packages"]
                .as_array()
                .context("packages")?
                .iter()
                .filter(|p| p["publish"] != json!([]))
                .map(|p| {
                    p["name"]
                        .as_str()
                        .context("package name")
                        .map(str::to_owned)
                })
                .collect::<Result<Vec<_>>>()?;
            names.sort();
            let text = serde_json::to_string(&names)?;
            if output {
                github_output("crates", &text)?;
            } else {
                println!("{text}");
            }
        }
        Task::TestFeatures { package } => {
            println!("{}", feature_names(&metadata(root)?, &package)?)
        }
        Task::NextestTimed { label, args } => return timed(root, &label, &args),
        Task::SyncBartenderImage { write, .. } => sync_bartender(root, write)?,
        Task::MeasureBinarySize {
            out_dir,
            skip_build,
        } => super::size::measure(root, &out_dir, skip_build)?,
        Task::BinarySizeReport {
            head,
            base,
            out_dir,
        } => super::size::report(
            &head,
            base.or_else(|| {
                std::env::var_os("BASE_DIR")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
            })
            .as_deref(),
            &out_dir,
        )?,
    }
    Ok(0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn security_and_backend_features_cannot_join_the_native_suite() {
        let meta = json!({"packages":[{"name":"sample","features":{"default":[],"danger-skip-cert":[],"tracing":[],"tracing-pii":[],"js":[],"alpha":[]}}]});
        assert_eq!(feature_names(&meta, "sample").unwrap(), "alpha,tracing");
        assert!(feature_names(&meta, "missing").is_err());
    }
    #[cfg(unix)]
    #[test]
    fn failing_tests_keep_their_report_but_build_failures_cannot_reuse_stale_xml() {
        use std::os::unix::process::ExitStatusExt;
        let root = tempfile::tempdir().unwrap();
        let destination = root.path().join("timings");
        let source = root.path().join("target/nextest/ci/junit.xml");
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, b"stale").unwrap();
        let status = timed_report(root.path(), &destination, "ci", "failed-test", || {
            assert!(!source.exists());
            std::fs::write(&source, b"fresh failure")?;
            Ok(std::process::ExitStatus::from_raw(7 << 8))
        })
        .unwrap();
        assert_eq!(status, 7);
        assert_eq!(
            std::fs::read(destination.join("failed-test.xml")).unwrap(),
            b"fresh failure"
        );
        std::fs::write(&source, b"stale restored target").unwrap();
        let status = timed_report(root.path(), &destination, "ci", "failed-build", || {
            Ok(std::process::ExitStatus::from_raw(1 << 8))
        })
        .unwrap();
        assert_eq!(status, 1);
        assert!(!source.exists());
        assert!(!destination.join("failed-build.xml").exists());
    }
}
