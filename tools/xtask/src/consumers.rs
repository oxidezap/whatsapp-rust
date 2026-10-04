//! Independent API hosts: explicit modes, no workspace feature unification.
use anyhow::{Context, Result, ensure};
use clap::{Subcommand, ValueEnum};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::process::Command;

const REGISTRY: &str = "tools/xtask/consumers.json";
// These are the only test manifests belonging to the root workspace, not API hosts.
const WORKSPACE_TESTS: &[&str] = &["tests/bench-integration/Cargo.toml", "tests/e2e/Cargo.toml"];

#[derive(Subcommand)]
pub enum Task {
    /// Fail on missing registrations, stale paths or invalid modes (no Cargo builds).
    Check,
    /// Execute the registered commands; test includes rustdoc negative controls.
    Run {
        #[arg(long, value_enum)]
        lane: Lane,
        /// Cargo toolchain for the hosts, independent of the xtask build toolchain.
        #[arg(long)]
        toolchain: Option<String>,
        /// Limit local validation to one registered manifest.
        #[arg(long)]
        manifest: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Lane {
    Native,
    Msrv,
    Wasm,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Check,
    Build,
    Test,
    Run,
}
impl Mode {
    fn command(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Build => "build",
            Self::Test => "test",
            Self::Run => "run",
        }
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Consumer {
    manifest: String,
    // Legacy fixtures without a committed lock resolve independently; never use
    // the root lock or add them to its workspace to make --locked work.
    locked: bool,
    /// Temporary forward registration for a separately owned, unmerged domain PR.
    /// Missing fixtures are reported, never executed or claimed covered.
    #[serde(default)]
    integration_pr: Option<String>,
    commands: Vec<Invocation>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    lanes: Vec<Lane>,
    mode: Mode,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    no_default_features: bool,
    #[serde(default)]
    release: bool,
    #[serde(default)]
    bin: Option<String>,
    #[serde(default)]
    lib: bool,
    /// A directed negative binary must fail for this diagnostic, not a missing dependency.
    #[serde(default)]
    expect_failure: Option<ExpectedFailure>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedFailure {
    error_code: String,
    contains: Vec<String>,
}
impl ExpectedFailure {
    fn verify(&self, success: bool, stdout: &str) -> Result<()> {
        ensure!(!success, "removed API unexpectedly compiled");
        let errors = stdout
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|message| {
                message["reason"] == "compiler-message" && message["message"]["level"] == "error"
            })
            .collect::<Vec<_>>();
        ensure!(
            errors.len() == 1,
            "negative control needs exactly one compiler error, got {}",
            errors.len()
        );
        let diagnostic = &errors[0]["message"];
        ensure!(
            diagnostic["code"]["code"] == self.error_code,
            "negative control did not report {}",
            self.error_code
        );
        // The rendered diagnostic also carries expected/found type labels and
        // source spans, needed for directed E0308/E0639 library controls.
        let text = diagnostic["rendered"]
            .as_str()
            .or_else(|| diagnostic["message"].as_str())
            .context("compiler diagnostic text")?;
        for needle in &self.contains {
            ensure!(
                text.contains(needle),
                "negative control's {} diagnostic did not report {needle:?}",
                self.error_code
            );
        }
        Ok(())
    }
}

fn manifest_key(path: &Path) -> Result<String> {
    Ok(path
        .components()
        .map(|part| part.as_os_str().to_str().context("non-UTF8 manifest path"))
        .collect::<Result<Vec<_>>>()?
        .join("/"))
}

fn discover(dir: &Path, root: &Path, manifests: &mut BTreeSet<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        // Only build output and VCS metadata are excluded; hidden host source
        // directories must not bypass registration.
        if name == "target" || name == ".git" {
            continue;
        }
        let kind = entry.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "symlink in consumer tree: {}",
            entry.path().display()
        );
        if kind.is_dir() {
            discover(&entry.path(), root, manifests)?;
        } else if name == "Cargo.toml" {
            manifests.insert(manifest_key(entry.path().strip_prefix(root)?)?);
        }
    }
    Ok(())
}

fn validate(root: &Path, consumers: &[Consumer]) -> Result<()> {
    let mut registered = BTreeSet::new();
    for consumer in consumers {
        let path = Path::new(&consumer.manifest);
        ensure!(
            !consumer.manifest.contains('\\')
                && path.components().all(|c| matches!(c, Component::Normal(_)))
                && path.starts_with("tests")
                && path.file_name().is_some_and(|n| n == "Cargo.toml"),
            "invalid consumer manifest path: {}",
            consumer.manifest
        );
        ensure!(
            registered.insert(consumer.manifest.clone()),
            "duplicate consumer: {}",
            consumer.manifest
        );
        if let Some(pr) = &consumer.integration_pr {
            let number = pr
                .strip_prefix("https://github.com/oxidezap/whatsapp-rust/pull/")
                .context("integration_pr must name this repository's PR")?;
            ensure!(
                !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()),
                "invalid integration_pr: {pr}"
            );
        }
        if root.join(path).is_file() {
            ensure!(
                consumer.integration_pr.is_none(),
                "{} has arrived: remove integration_pr to promote mandatory coverage before validation",
                consumer.manifest
            );
            let text = std::fs::read_to_string(root.join(path))?;
            ensure!(
                text.lines().any(|line| line.trim() == "[workspace]"),
                "{} must remain a standalone workspace",
                consumer.manifest
            );
            ensure!(
                !consumer.locked || root.join(path).with_file_name("Cargo.lock").is_file(),
                "{} requires a committed Cargo.lock",
                consumer.manifest
            );
        } else {
            ensure!(
                consumer.integration_pr.is_some(),
                "registered consumer missing: {}",
                consumer.manifest
            );
        }
        ensure!(
            !consumer.commands.is_empty(),
            "{} has no commands",
            consumer.manifest
        );
        let mut modes = BTreeSet::new();
        let mut has_native = false;
        let mut has_msrv = false;
        for invocation in &consumer.commands {
            ensure!(
                !invocation.lanes.is_empty(),
                "{} has a command without lanes",
                consumer.manifest
            );
            ensure!(
                invocation
                    .lanes
                    .iter()
                    .enumerate()
                    .all(|(i, lane)| !invocation.lanes[..i].contains(lane)),
                "{} has duplicate lanes",
                consumer.manifest
            );
            if invocation.expect_failure.is_none() {
                has_native |= invocation.lanes.contains(&Lane::Native);
                has_msrv |= invocation.lanes.contains(&Lane::Msrv);
            }
            ensure!(
                !invocation.lanes.contains(&Lane::Wasm)
                    || matches!(invocation.mode, Mode::Check | Mode::Build),
                "{}: WASM modes must be check/build, not unconfigured cross-target execution",
                consumer.manifest
            );
            ensure!(
                !invocation.lib || invocation.bin.is_none(),
                "{}: select lib or bin, not both",
                consumer.manifest
            );
            ensure!(
                !invocation.lib || invocation.mode != Mode::Run,
                "{}: cannot run a library",
                consumer.manifest
            );
            if let Some(expected) = &invocation.expect_failure {
                ensure!(
                    invocation.mode == Mode::Check && (invocation.bin.is_some() || invocation.lib),
                    "{}: directed negatives must check an explicit lib or named bin target",
                    consumer.manifest
                );
                ensure!(
                    expected.error_code.len() == 5
                        && expected.error_code.starts_with('E')
                        && expected.error_code[1..].bytes().all(|b| b.is_ascii_digit())
                        && !expected.contains.is_empty()
                        && expected.contains.iter().all(|s| !s.is_empty()),
                    "{}: negative control needs an E#### code and API-specific diagnostics",
                    consumer.manifest
                );
            }
            for lane in &invocation.lanes {
                ensure!(
                    modes.insert(format!("{lane:?}:{:?}", invocation.args(consumer, *lane))),
                    "{}: duplicate command",
                    consumer.manifest
                );
            }
        }
        ensure!(
            has_native && has_msrv,
            "{} needs positive execution in both native and MSRV lanes",
            consumer.manifest
        );
    }
    let mut found = BTreeSet::new();
    discover(&root.join("tests"), root, &mut found)?;
    for path in WORKSPACE_TESTS {
        ensure!(
            found.remove(*path),
            "known workspace test manifest missing: {path}"
        );
    }
    let unregistered = found.difference(&registered).cloned().collect::<Vec<_>>();
    let staged = consumers
        .iter()
        .filter(|c| c.integration_pr.is_some())
        .map(|c| c.manifest.clone())
        .collect::<BTreeSet<_>>();
    let stale = registered
        .difference(&found)
        .filter(|path| !staged.contains(*path))
        .cloned()
        .collect::<Vec<_>>();
    ensure!(
        unregistered.is_empty() && stale.is_empty(),
        "consumer registry drift; unregistered: {unregistered:?}; stale: {stale:?}. Register each tests/ Cargo.toml and meaningful modes in {REGISTRY}"
    );
    Ok(())
}

impl Invocation {
    fn args(&self, consumer: &Consumer, lane: Lane) -> Vec<String> {
        let mut args = vec![
            self.mode.command().into(),
            "--manifest-path".into(),
            consumer.manifest.clone(),
        ];
        if consumer.locked {
            args.push("--locked".into());
        }
        if self.release {
            args.push("--release".into());
        }
        if self.no_default_features {
            args.push("--no-default-features".into());
        }
        if !self.features.is_empty() {
            args.extend(["--features".into(), self.features.join(",")]);
        }
        if let Some(bin) = &self.bin {
            args.extend(["--bin".into(), bin.clone()]);
        }
        if self.lib {
            args.push("--lib".into());
        }
        if lane == Lane::Wasm {
            args.extend(["--target".into(), "wasm32-unknown-unknown".into()]);
        }
        args
    }
}

pub fn run(root: &Path, task: Task) -> Result<u8> {
    let consumers: Vec<Consumer> = serde_json::from_slice(&std::fs::read(root.join(REGISTRY))?)?;
    validate(root, &consumers)?;
    let present = consumers
        .iter()
        .filter(|c| root.join(&c.manifest).is_file())
        .count();
    for consumer in &consumers {
        if let Some(pr) = &consumer.integration_pr {
            println!(
                "NOT YET INTEGRATED: {} ({pr}); no commands executed or coverage claimed",
                consumer.manifest
            );
        }
    }
    let Task::Run {
        lane,
        toolchain,
        manifest,
        dry_run,
    } = task
    else {
        println!(
            "Consumer registry covers all {} standalone tests/ manifests.",
            present
        );
        return Ok(0);
    };
    ensure!(
        lane != Lane::Msrv || toolchain.is_some(),
        "MSRV execution requires --toolchain (CI pins the published floor)"
    );
    if let Some(manifest) = &manifest {
        ensure!(
            consumers.iter().any(|c| &c.manifest == manifest),
            "unknown consumer: {manifest}"
        );
    }
    let mut count = 0;
    let mut first_failure = 0;
    for consumer in &consumers {
        if !root.join(&consumer.manifest).is_file() {
            continue;
        }
        if manifest.as_ref().is_some_and(|m| m != &consumer.manifest) {
            continue;
        }
        for invocation in &consumer.commands {
            if !invocation.lanes.contains(&lane) {
                continue;
            }
            count += 1;
            let mut args = Vec::new();
            if let Some(toolchain) = &toolchain {
                args.push(format!("+{toolchain}"));
            }
            args.extend(invocation.args(consumer, lane));
            println!("[{lane:?}] cargo {}", args.join(" "));
            if dry_run {
                continue;
            }
            let mut command = Command::new("cargo");
            command
                .args(&args)
                .current_dir(root)
                .env("CARGO_BUILD_JOBS", "1")
                .env("CARGO_TARGET_DIR", root.join("target/consumers"))
                // Do not leak host nightly flags into the MSRV or WASM hosts.
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env(
                    "RUSTFLAGS",
                    if lane == Lane::Wasm {
                        "--cfg getrandom_backend=\"wasm_js\""
                    } else {
                        ""
                    },
                );
            let code = if let Some(expected) = &invocation.expect_failure {
                // Structured diagnostics tie the code and API fragments to one
                // primary error, independent of ANSI or unrelated stderr text.
                command
                    .args(["--message-format", "json"])
                    .env("CARGO_TERM_COLOR", "never");
                let output = command
                    .output()
                    .with_context(|| format!("execute {}", consumer.manifest))?;
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprint!("{stderr}");
                match expected.verify(
                    output.status.success(),
                    &String::from_utf8_lossy(&output.stdout),
                ) {
                    Ok(()) => 0,
                    Err(error) => {
                        eprintln!("{error:#}");
                        1
                    }
                }
            } else {
                xtask_support::exit_code(
                    command
                        .status()
                        .with_context(|| format!("execute {}", consumer.manifest))?,
                )
            };
            if code != 0 {
                eprintln!(
                    "consumer failed: {} ({:?}, {lane:?})",
                    consumer.manifest, invocation.mode
                );
                if first_failure == 0 {
                    first_failure = code;
                }
            }
        }
    }
    ensure!(
        count > 0,
        "no registered commands for requested lane/manifest"
    );
    println!("{count} consumer commands selected; first failure: {first_failure}");
    Ok(first_failure)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Vec<Consumer>) {
        let root = tempfile::tempdir().unwrap();
        for path in WORKSPACE_TESTS
            .iter()
            .copied()
            .chain(["tests/fixtures/host/Cargo.toml"])
        {
            let path = root.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "[workspace]\n").unwrap();
        }
        let consumers = serde_json::from_str(r#"[{"manifest":"tests/fixtures/host/Cargo.toml","locked":false,"commands":[{"lanes":["native","msrv"],"mode":"test"}]}]"#).unwrap();
        (root, consumers)
    }
    #[test]
    fn detects_new_manifests_even_without_consumer_in_their_name() {
        let (root, consumers) = fixture();
        validate(root.path(), &consumers).unwrap();
        assert_eq!(
            manifest_key(Path::new("tests/fixtures/host/Cargo.toml")).unwrap(),
            "tests/fixtures/host/Cargo.toml"
        );
        let added = root.path().join("tests/.new-domain/deep");
        std::fs::create_dir_all(&added).unwrap();
        std::fs::write(added.join("Cargo.toml"), "[workspace]").unwrap();
        let error = validate(root.path(), &consumers).unwrap_err().to_string();
        assert!(
            error.contains("unregistered") && error.contains("tests/.new-domain/deep/Cargo.toml")
        );
    }
    #[test]
    fn rejects_stale_duplicate_nonstandalone_and_empty_registrations() {
        let (root, mut consumers) = fixture();
        consumers[0].commands.clear();
        assert!(validate(root.path(), &consumers).is_err());
        let (root, mut consumers) = fixture();
        consumers[0].manifest = "tests/absent/Cargo.toml".into();
        assert!(validate(root.path(), &consumers).is_err());
        let (root, mut consumers) = fixture();
        consumers.push(serde_json::from_str(&serde_json::json!({"manifest":consumers[0].manifest,"locked":false,"commands":[{"mode":"test","lanes":["native"]}]}).to_string()).unwrap());
        assert!(validate(root.path(), &consumers).is_err());
        let (root, consumers) = fixture();
        std::fs::write(root.path().join(&consumers[0].manifest), "[package]").unwrap();
        assert!(validate(root.path(), &consumers).is_err());
    }
    #[test]
    fn builds_explicit_modes_and_never_uses_all_features_or_test_filters() {
        let (_, consumers) = fixture();
        let invocation: Invocation = serde_json::from_str(r#"{"lanes":["wasm"],"mode":"build","features":["requests"],"release":true,"no_default_features":true}"#).unwrap();
        let args = invocation.args(&consumers[0], Lane::Wasm);
        assert_eq!(
            args,
            [
                "build",
                "--manifest-path",
                "tests/fixtures/host/Cargo.toml",
                "--release",
                "--no-default-features",
                "--features",
                "requests",
                "--target",
                "wasm32-unknown-unknown"
            ]
        );
        let args = consumers[0].commands[0].args(&consumers[0], Lane::Msrv);
        assert_eq!(args[0], "test"); // No --lib: rustdoc negative controls must run too.
        assert!(
            !args
                .iter()
                .any(|arg| arg == "--all-features" || arg == "--lib")
        );
    }
    #[test]
    fn negative_controls_require_the_specific_failure_and_reject_success() {
        let expected = ExpectedFailure {
            error_code: "E0599".into(),
            contains: vec!["VideoStateChanged".into(), "CallEvent".into()],
        };
        let diagnostic = |code: &str, message: &str| {
            serde_json::json!({"reason":"compiler-message","message":{"level":"error","code":{"code":code},"message":message}}).to_string()
        };
        let correct = diagnostic(
            "E0599",
            "no variant named `VideoStateChanged` found for enum `CallEvent`",
        );
        assert!(expected.verify(false, &correct).is_ok());
        assert!(expected.verify(true, &correct).is_err());
        assert!(
            expected
                .verify(false, &diagnostic("E0432", "unresolved import"))
                .is_err()
        );
        assert!(
            expected
                .verify(false, &diagnostic("E0599", "unrelated method absent"))
                .is_err()
        );
        let split = format!(
            "{}\n{}",
            diagnostic("E0599", "unrelated method absent"),
            diagnostic("E0432", "VideoStateChanged CallEvent")
        );
        assert!(expected.verify(false, &split).is_err());
        let typed = ExpectedFailure {
            error_code: "E0308".into(),
            contains: vec!["expected `MessageId`".into(), "found `String`".into()],
        };
        let typed_output = serde_json::json!({"reason":"compiler-message","message":{"level":"error","code":{"code":"E0308"},"message":"mismatched types","rendered":"error[E0308]: mismatched types\nexpected `MessageId`, found `String`"}}).to_string();
        assert!(typed.verify(false, &typed_output).is_ok());
        assert!(
            expected
                .verify(
                    false,
                    &format!("{correct}\n{}", diagnostic("E0432", "another error"))
                )
                .is_err()
        );
    }
    #[test]
    fn forward_registration_is_explicit_and_checks_every_mode_when_fixture_arrives() {
        let (root, mut consumers) = fixture();
        let incoming: Consumer = serde_json::from_str(r#"{"manifest":"tests/fixtures/incoming/Cargo.toml","locked":true,"integration_pr":"https://github.com/oxidezap/whatsapp-rust/pull/1624","commands":[{"lanes":["native","msrv"],"mode":"test"},{"lanes":["wasm"],"mode":"check"}]}"#).unwrap();
        consumers.push(incoming);
        validate(root.path(), &consumers).unwrap();
        let path = root.path().join(&consumers[1].manifest);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[workspace]").unwrap();
        assert!(validate(root.path(), &consumers).is_err()); // Its lock is now mandatory.
        std::fs::write(path.with_file_name("Cargo.lock"), "").unwrap();
        let error = validate(root.path(), &consumers).unwrap_err().to_string();
        assert!(error.contains("remove integration_pr")); // Cannot leave an integrated exemption.
        consumers[1].integration_pr = None; // Promotion is mandatory, not a documentation-only convention.
        validate(root.path(), &consumers).unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(validate(root.path(), &consumers).is_err());
    }
    #[test]
    fn refuses_silent_native_or_msrv_coverage_gaps() {
        let (root, mut consumers) = fixture();
        consumers[0].commands[0].lanes = vec![Lane::Native];
        assert!(validate(root.path(), &consumers).is_err());
        consumers[0].commands[0].lanes = vec![Lane::Msrv];
        assert!(validate(root.path(), &consumers).is_err());
    }
    #[test]
    fn refuses_wasm_test_run_and_missing_locks() {
        let (root, mut consumers) = fixture();
        consumers[0].commands[0].lanes = vec![Lane::Native, Lane::Wasm];
        assert!(validate(root.path(), &consumers).is_err());
        consumers[0].commands[0].lanes = vec![Lane::Native];
        consumers[0].locked = true;
        assert!(validate(root.path(), &consumers).is_err());
    }
}
