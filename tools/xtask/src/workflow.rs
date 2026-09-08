//! First-party workflow logic. External programs remain argument-safe process calls.
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde_json::Value;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use xtask_support::{capture, run};

#[derive(Subcommand)]
pub enum Task {
    /// Read the installed libc version for the benchmark cache key.
    LibcVersion,
    /// Wait for the local self-signed mock service used by E2E/bench jobs.
    WaitForMock,
    /// Reject direct native pool references outside the SQLite platform shim.
    GuardSqliteShim,
    /// Run semver-checks while streaming its merged output into semver.log.
    SemverCheck,
    /// Append an informational semver result and bounded log tail to the job summary.
    SemverSummary {
        #[arg(long)]
        outcome: String,
        #[arg(long, default_value = "semver.log")]
        log: PathBuf,
    },
    /// Validate a release version before publishing and emit version/tag outputs.
    ReleaseVersion,
    /// Create the requested release tag only if it is absent remotely.
    ReleaseTag,
    /// Preserve existing release notes, otherwise create the requested GitHub release.
    GithubRelease,
    /// Validate a main dispatch's compiler before it reaches rustup or the measurement process.
    SizeToolchain,
    /// Download the PR base's validated main measurement, independent of graph publication.
    SizeBaseline {
        #[arg(long, default_value = "size-out")]
        head: PathBuf,
        #[arg(long, default_value = "base-out")]
        out_dir: PathBuf,
    },
    /// Query the live size-increase-ok label and emit the override value.
    SizeOverride,
    /// Update the existing sticky size report, or create it if absent.
    SizeComment {
        #[arg(long, default_value = "size-out/report.md")]
        report: PathBuf,
    },
    /// Enforce the previously written gate after reports have been published.
    SizeGate {
        #[arg(long, default_value = "size-out/gate.txt")]
        file: PathBuf,
    },
    /// Append a file to GITHUB_STEP_SUMMARY.
    AppendSummary { file: PathBuf },
    /// Install matching libc debug symbols with the benchmark CI's bounded retries/cache.
    InstallLibcDebug,
}
fn output(key: &str, value: &str) -> Result<()> {
    ensure!(!value.contains(['\r', '\n']), "multiline output");
    let path = std::env::var_os("GITHUB_OUTPUT").context("GITHUB_OUTPUT must be set")?;
    writeln!(
        std::fs::OpenOptions::new().append(true).open(path)?,
        "{key}={value}"
    )?;
    Ok(())
}
fn summary(bytes: &[u8]) -> Result<()> {
    let path =
        std::env::var_os("GITHUB_STEP_SUMMARY").context("GITHUB_STEP_SUMMARY must be set")?;
    std::fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
fn quiet(command: &mut Command) -> Result<bool> {
    Ok(command
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success())
}
fn env(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("{name} must be set"))
}
fn version_valid(version: &str) -> bool {
    regex::Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$")
        .is_ok_and(|r| r.is_match(version))
}
fn size_toolchain<'a>(
    event: &str,
    requested: &'a str,
    default: &'a str,
) -> Result<(&'a str, bool)> {
    let selected = if event == "workflow_dispatch" && !requested.is_empty() {
        requested
    } else {
        default
    };
    ensure!(
        regex::Regex::new(r"\Anightly-[0-9]{4}-(0[1-9]|1[0-2])-(0[1-9]|[12][0-9]|3[01])\z")?
            .is_match(selected),
        "binary size toolchain must be a dated nightly, nightly-YYYY-MM-DD"
    );
    Ok((selected, selected == default))
}
fn tag() -> Result<String> {
    let tag = env("RELEASE_TAG")?;
    ensure!(
        tag.strip_prefix('v').is_some_and(version_valid),
        "invalid release tag"
    );
    Ok(tag)
}
fn github(path: &str) -> Result<Value> {
    Ok(serde_json::from_slice(
        &capture(Command::new("gh").args(["api", path]))?.stdout,
    )?)
}
fn copy_debs(directory: &Path, cache: &Path) -> Result<()> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("libc6-dbg_") && n.ends_with(".deb"))
        {
            let _ = std::fs::copy(&path, cache.join(path.file_name().context("package name")?));
        }
    }
    Ok(())
}
fn install_libc_debug() -> Result<()> {
    let work = tempfile::NamedTempFile::new()?;
    std::fs::write(work.path(),b"Acquire::Retries \"3\";\nAcquire::http::Timeout \"20\";\nAcquire::https::Timeout \"20\";\nDPkg::Lock::Timeout \"60\";\nAPT::Keep-Downloaded-Packages \"true\";\n")?;
    run(Command::new("sudo")
        .args(["install", "-m", "644"])
        .arg(work.path())
        .arg("/etc/apt/apt.conf.d/99-ci-bounds"))?;
    let cache = PathBuf::from(env("HOME")?).join(".cache/libc6-dbg");
    std::fs::create_dir_all(&cache)?;
    let packages = std::fs::read_dir(&cache)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "deb"))
        .collect::<Vec<_>>();
    if !packages.is_empty() && quiet(Command::new("sudo").args(["dpkg", "-i"]).args(packages))? {
        return Ok(());
    }
    let version = env("LIBC6_VERSION")?;
    let pin = format!("libc6-dbg={version}");
    for attempt in 0..3 {
        let ready = attempt == 0
            || quiet(Command::new("sudo").args(["timeout", "180", "apt-get", "update", "-q"]))?;
        if ready
            && quiet(
                Command::new("sudo").args(["timeout", "120", "apt-get", "install", "-y", &pin]),
            )?
        {
            copy_debs(Path::new("/var/cache/apt/archives"), &cache)?;
            return Ok(());
        }
        if attempt > 0 {
            std::thread::sleep(Duration::from_secs(15));
        }
    }
    if quiet(Command::new("sudo").args([
        "timeout",
        "120",
        "apt-get",
        "install",
        "-y",
        "libc6-dbg",
    ]))? {
        println!("::warning::installed newer libc debug symbols; not cached");
    } else {
        println!("::warning::libc debug pre-install gave up; the benchmark runner installs them");
    }
    Ok(())
}
pub fn run_task(root: &Path, task: Task) -> Result<()> {
    match task {
        Task::SemverCheck => {
            use std::io::Read;
            let (mut reader, writer) = std::io::pipe()?;
            let mut command = Command::new("cargo");
            command
                .args([
                    "semver-checks",
                    "check-release",
                    "--color",
                    "never",
                    "-p",
                    "wacore",
                    "-p",
                    "wacore-binary",
                    "-p",
                    "waproto",
                ])
                .current_dir(root)
                .stdout(writer.try_clone()?)
                .stderr(writer);
            let mut child = command.spawn()?;
            drop(command);
            let mut log = std::fs::File::create(root.join("semver.log"))?;
            let mut stdout = std::io::stdout().lock();
            let mut buffer = [0u8; 8192];
            loop {
                let n = reader.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                log.write_all(&buffer[..n])?;
                stdout.write_all(&buffer[..n])?;
                stdout.flush()?;
            }
            ensure!(
                child.wait()?.success(),
                "semver-checks reported a failure; see semver.log"
            );
        }
        Task::LibcVersion => {
            let v = String::from_utf8(
                capture(Command::new("dpkg-query").args(["-W", "-f=${Version}", "libc6"]))?.stdout,
            )?;
            output("version", v.trim())?;
        }
        Task::WaitForMock => {
            for _ in 0..30 {
                if quiet(Command::new("curl").args(["-sk", "https://localhost:8080/"]))? {
                    println!("Mock server is ready");
                    return Ok(());
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            anyhow::bail!("Mock server failed to become ready");
        }
        Task::GuardSqliteShim => {
            let base = root.join("storages/sqlite-storage/src");
            let mut dirs = vec![base.clone()];
            let re = regex::Regex::new(
                r"tokio::task::spawn_blocking|r2d2::Pool|^\s*use .*tokio::task|^\s*use .*r2d2::\{[^}]*Pool",
            )?;
            let mut violations = Vec::new();
            while let Some(dir) = dirs.pop() {
                for entry in std::fs::read_dir(dir)? {
                    let path = entry?.path();
                    if path.is_dir() {
                        dirs.push(path);
                    } else if path.extension().is_some_and(|e| e == "rs")
                        && path != base.join("pool.rs")
                    {
                        for (i, line) in std::fs::read_to_string(&path)?.lines().enumerate() {
                            if re.is_match(line) {
                                violations.push(format!("{}:{}: {line}", path.display(), i + 1));
                            }
                        }
                    }
                }
            }
            ensure!(
                violations.is_empty(),
                "use crate::pool instead of direct native pool APIs:\n{}",
                violations.join("\n")
            );
        }
        Task::SemverSummary { outcome, log } => {
            let verdict = if outcome == "success" {
                "No breaking changes detected against the last published release."
            } else {
                "Breaking changes detected. This does not block the PR — bump the\nminor version if the break is intended."
            };
            let tail = match std::fs::read(log) {
                Ok(bytes) => String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(200000)..])
                    .into_owned(),
                Err(_) => {
                    "cargo-semver-checks did not run; see the failed setup step above.\n".into()
                }
            };
            summary(format!("## cargo-semver-checks (informational)\n\n{verdict}\n\n<details><summary>Output (tail; full log in the semver-checks-log artifact)</summary>\n\n```\n{tail}```\n\n</details>\n").as_bytes())?;
        }
        Task::ReleaseVersion => {
            let text = std::fs::read_to_string(root.join("Cargo.toml"))?;
            let version = text
                .lines()
                .find(|l| l.starts_with("version"))
                .and_then(|l| l.split('"').nth(1))
                .context("package version missing")?;
            ensure!(
                version_valid(version),
                "refusing release: expected MAJOR.MINOR.PATCH[-prerelease], no build metadata"
            );
            output("version", version)?;
            output("tag", &format!("v{version}"))?;
        }
        Task::ReleaseTag => {
            let tag = tag()?;
            if !quiet(
                Command::new("git")
                    .args([
                        "ls-remote",
                        "--exit-code",
                        "--tags",
                        "origin",
                        &format!("refs/tags/{tag}"),
                    ])
                    .current_dir(root),
            )? {
                run(Command::new("git").args(["tag", &tag]).current_dir(root))?;
                run(Command::new("git")
                    .args(["push", "origin", &tag])
                    .current_dir(root))?;
            }
        }
        Task::GithubRelease => {
            let tag = tag()?;
            if !quiet(
                Command::new("gh")
                    .args(["release", "view", &tag])
                    .current_dir(root),
            )? {
                let notes = root.join(format!(".github/release-notes/{tag}.md"));
                let mut c = Command::new("gh");
                c.args(["release", "create", &tag, "--title", &tag, "--latest"])
                    .current_dir(root);
                if notes.is_file() {
                    c.arg("--notes-file").arg(notes);
                } else {
                    c.arg("--generate-notes");
                }
                run(&mut c)?;
            }
        }
        Task::SizeToolchain => {
            let requested = std::env::var("REQUESTED_TOOLCHAIN").unwrap_or_default();
            let default = env("DEFAULT_SIZE_TOOLCHAIN")?;
            let (selected, publish) =
                size_toolchain(&env("GITHUB_EVENT_NAME")?, &requested, &default)?;
            output("toolchain", selected)?;
            output("publish", if publish { "true" } else { "false" })?;
        }
        Task::SizeBaseline { head, out_dir } => {
            super::size_baseline::download(
                &root.join(&head),
                &root.join(&out_dir),
                &env("GITHUB_REPOSITORY")?,
                &env("BASE_SHA")?,
                |args| Ok(capture(Command::new("gh").args(args).current_dir(root))?.stdout),
            )?;
            output("dir", out_dir.to_str().context("baseline path encoding")?)?;
        }
        Task::SizeOverride => {
            let value = github(&format!(
                "repos/{}/issues/{}/labels",
                env("GITHUB_REPOSITORY")?,
                env("PR_NUMBER")?
            ))?;
            let allow = value
                .as_array()
                .context("labels")?
                .iter()
                .any(|v| v["name"] == "size-increase-ok");
            output("allow", if allow { "true" } else { "false" })?;
        }
        Task::SizeComment { report } => {
            let repo = env("GITHUB_REPOSITORY")?;
            let number = env("PR_NUMBER")?;
            let comments: Value = serde_json::from_slice(
                &capture(Command::new("gh").args([
                    "api",
                    "--paginate",
                    "--slurp",
                    &format!("repos/{repo}/issues/{number}/comments"),
                ]))?
                .stdout,
            )?;
            let id = comments
                .as_array()
                .context("comments")?
                .iter()
                .flat_map(|page| page.as_array().into_iter().flatten())
                .find(|c| {
                    c["body"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("<!-- binary-size-report -->"))
                })
                .and_then(|c| c["id"].as_u64());
            let (method, path) = if let Some(id) = id {
                ("PATCH", format!("repos/{repo}/issues/comments/{id}"))
            } else {
                ("POST", format!("repos/{repo}/issues/{number}/comments"))
            };
            run(Command::new("gh")
                .args(["api", "-X", method, &path, "-F"])
                .arg(format!("body=@{}", report.display())))?;
        }
        Task::SizeGate { file } => {
            let text = std::fs::read_to_string(file)?;
            let status = text.lines().next().context("empty size gate")?;
            ensure!(
                matches!(status, "PASS" | "FAIL" | "OVERRIDDEN"),
                "unknown gate status"
            );
            ensure!(
                status != "FAIL",
                "binary size budget exceeded; see the report"
            );
            println!("Gate status: {status}");
        }
        Task::AppendSummary { file } => summary(&std::fs::read(file)?)?,
        Task::InstallLibcDebug => install_libc_debug()?,
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn size_compiler_override_is_dispatch_only_and_does_not_publish() {
        let default = "nightly-2026-06-16";
        let alternate = "nightly-2026-07-01";
        for event in ["push", "pull_request"] {
            assert_eq!(
                size_toolchain(event, alternate, default).unwrap(),
                (default, true)
            );
        }
        for requested in ["", default] {
            assert_eq!(
                size_toolchain("workflow_dispatch", requested, default).unwrap(),
                (default, true)
            );
        }
        assert_eq!(
            size_toolchain("workflow_dispatch", alternate, default).unwrap(),
            (alternate, false)
        );
        let workflow = include_str!("../../../.github/workflows/binary-size.yml");
        assert!(workflow.contains(&format!("default: {default}")));
        assert!(workflow.contains(&format!("RUSTUP_TOOLCHAIN: {default}")));
        assert!(workflow.contains("REQUESTED_TOOLCHAIN: ${{ inputs.toolchain }}"));
        assert!(workflow.contains("DEFAULT_SIZE_TOOLCHAIN: ${{ env.RUSTUP_TOOLCHAIN }}"));
        assert!(workflow.contains("RUSTUP_TOOLCHAIN: ${{ steps.compiler.outputs.toolchain }}"));
        assert!(workflow.contains("if: steps.compiler.outputs.publish == 'true'"));
    }
    #[test]
    fn size_compiler_input_cannot_supply_flags_paths_or_shell_syntax() {
        for requested in [
            "--help",
            "+nightly-2026-06-16",
            "nightly",
            "stable",
            "../toolchain",
            "nightly-2026-06-16 --profile complete",
            "nightly-2026-06-16;id",
            "$(id)",
            "nightly-2026-06-16\nRUSTFLAGS=-O",
            "nightly-2026-06-16\r",
            "nightly-2026-00-16",
            "nightly-2026-06-00",
            "nightly-2026-13-16",
        ] {
            assert!(
                size_toolchain("workflow_dispatch", requested, "nightly-2026-06-16").is_err(),
                "{requested:?}"
            );
        }
    }
    #[test]
    fn release_preflight_rejects_docker_tag_collisions() {
        assert!(version_valid("0.7.0"));
        assert!(version_valid("0.7.0-rc.1"));
        assert!(!version_valid("0.7.0+build.1"));
        assert!(!version_valid("--force"));
    }
}
