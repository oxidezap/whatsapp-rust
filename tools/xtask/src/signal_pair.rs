//! Matched CPU/allocation workloads, with explicit ports for retired adapter methods.
use crate::prefetch_pair::{cargo_codspeed_version, full_sha, revision};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
use xtask_support::{capture, run, sha256, write, write_json};

const PACKAGE: &str = "wacore/libsignal";
const WORKLOAD: &str = "benches/classic_fixture/mod.rs";
const ACCESS: &str = "benches/classic_fixture/access.rs";
const BENCH: &str = "benches/classic_benchmark.rs";
// These are calls to the baseline's public API, not an implementation of its algorithms.
const BASE_ACCESS: &str = r#"// SPDX-License-Identifier: MIT
use wacore_libsignal::protocol::{PublicKey, SessionState, MessageKeyGenerator, SenderKeyRecord, PrivateKey, PreparedVerifyingKey};
pub(super) struct CryptoContext {
    signing: Option<PrivateKey>,
    verifying: PreparedVerifyingKey,
}
pub(super) fn capture_crypto(record: &mut SenderKeyRecord) -> CryptoContext {
    let state = record.sender_key_state().unwrap();
    let signing = state.signing_key_private().ok();
    if let Some(key) = &signing { key.precompute_signing_cache(); }
    let verifying = PreparedVerifyingKey::new(&state.signing_key_public().unwrap());
    verifying.precompute();
    CryptoContext { signing, verifying }
}
pub(super) fn restore_crypto(record: &mut SenderKeyRecord, context: &CryptoContext) {
    let state = record.sender_key_state().unwrap();
    if let Some(key) = &context.signing { state.prewarm_signing_key(key.clone()).unwrap(); }
    state.prewarm_verifying_key(context.verifying.clone()).unwrap();
}
pub(super) fn same_identity(state: &SessionState) -> bool {
    state.session_with_self().unwrap()
}
pub(super) fn take(state: &mut SessionState, ratchet: &PublicKey, index: u32) -> bool {
    let key = state.get_message_keys(ratchet, index).unwrap();
    let found = key.is_some();
    std::hint::black_box(key);
    found
}
pub(super) fn insert(state: &mut SessionState, ratchet: &PublicKey, index: u32, seed: [u8; 32]) {
    state.set_message_keys(ratchet, MessageKeyGenerator::new_from_seed(&seed, index)).unwrap();
}
"#;
#[derive(Serialize, Deserialize)]
struct Artifact {
    role: String,
    revision: String,
    checkout: PathBuf,
    target: PathBuf,
    bench: String,
    port_sha256: String,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    driver_revision: String,
    workload_sha256: String,
    benchmark_sha256: String,
    cases: usize,
    expected_results: usize,
    instruments: [String; 2],
    rustc: String,
    cargo_codspeed: String,
    artifacts: Vec<Artifact>,
}
fn case_count(source: &str) -> Result<usize> {
    let marker = "pub const CASES: &[Case] = &[";
    ensure!(
        source.matches(marker).count() == 1,
        "classic case catalog changed"
    );
    let catalog = source
        .split_once(marker)
        .context("missing classic catalog")?
        .1
        .split_once("];")
        .context("unterminated classic catalog")?
        .0;
    let count = catalog
        .lines()
        .filter(|line| line.trim().starts_with("Case::"))
        .count();
    ensure!(count > 0, "empty classic case catalog");
    Ok(count)
}
fn baseline_port(package: &Path, cargo: &mut String) -> Result<Vec<u8>> {
    let parsed: toml::Value = toml::from_str(cargo)?;
    let features = parsed
        .get("features")
        .and_then(toml::Value::as_table)
        .context("baseline feature table missing")?;
    if features.contains_key("bench-internals") {
        return std::fs::read(package.join(ACCESS))
            .context("baseline declares bench-internals but has no matching benchmark API port");
    }
    ensure!(
        cargo.matches("[features]\n").count() == 1,
        "baseline feature table changed"
    );
    *cargo = cargo.replacen("[features]\n", "[features]\nbench-internals = []\n", 1);
    Ok(BASE_ACCESS.as_bytes().to_vec())
}
pub fn prepare(root: &Path, base: &str, head: &str, output: &Path) -> Result<()> {
    full_sha(base)?;
    full_sha(head)?;
    ensure!(
        revision(root)? == head,
        "measurement driver is not at declared head"
    );
    let version = cargo_codspeed_version()?;
    let output = root.join(output);
    ensure!(
        !output.exists(),
        "refusing to reuse previous Signal measurement artifacts"
    );
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let package = root.join(PACKAGE);
    let workload = std::fs::read(package.join(WORKLOAD))?;
    let benchmark = std::fs::read(package.join(BENCH))?;
    let head_port = std::fs::read(package.join(ACCESS))?;
    let cases = case_count(std::str::from_utf8(&workload)?)?;
    for (name, contents) in [
        ("workload.rs", workload.as_slice()),
        ("benchmark.rs", benchmark.as_slice()),
        ("head-api-port.rs", head_port.as_slice()),
    ] {
        write(&output.join("results").join(name), contents)?;
    }
    let mut artifacts = Vec::new();
    for (role, sha) in [("base", base), ("head", head)] {
        let checkout = output.join(role);
        run(Command::new("git")
            .current_dir(root)
            .args(["worktree", "add", "--detach"])
            .arg(&checkout)
            .arg(sha))?;
        ensure!(
            revision(&checkout)? == sha,
            "unexpected comparison revision"
        );
        let package = checkout.join(PACKAGE);
        let manifest_path = package.join("Cargo.toml");
        let mut cargo = std::fs::read_to_string(&manifest_path)?;
        let port = if role == "base" {
            baseline_port(&package, &mut cargo)?
        } else {
            head_port.clone()
        };
        write(&output.join(format!("results/{role}-api-port.rs")), &port)?;
        write(&package.join(WORKLOAD), &workload)?;
        write(&package.join(ACCESS), &port)?;
        let bench = format!("classic_pair_{role}");
        write(&package.join(format!("benches/{bench}.rs")), &benchmark)?;
        cargo.push_str(&format!("\n[[bench]]\nname = {bench:?}\nharness = false\n"));
        write(&manifest_path, cargo.as_bytes())?;
        let patch = capture(Command::new("git").current_dir(&checkout).args([
            "diff",
            "--",
            "wacore/libsignal/Cargo.toml",
        ]))?;
        write(
            &output.join(format!("results/{role}-registration.patch")),
            &patch.stdout,
        )?;
        let target = output.join("target").join(role);
        run(Command::new("cargo")
            .current_dir(&checkout)
            .env("CARGO_TARGET_DIR", &target)
            .args([
                "codspeed",
                "build",
                "-m",
                "simulation",
                "-m",
                "memory",
                "-p",
                "wacore-libsignal",
                "--features",
                "legacy-session-interop,bench-internals",
                "--bench",
                &bench,
            ]))?;
        artifacts.push(Artifact {
            role: role.to_owned(),
            revision: sha.to_owned(),
            checkout,
            target,
            bench,
            port_sha256: sha256(&port),
        });
    }
    write_json(
        &output.join("results/manifest.json"),
        &Manifest {
            schema_version: 1,
            driver_revision: head.to_owned(),
            workload_sha256: sha256(&workload),
            benchmark_sha256: sha256(&benchmark),
            cases,
            expected_results: cases * 4,
            instruments: ["Simulation".to_owned(), "Memory".to_owned()],
            rustc: String::from_utf8(capture(Command::new("rustc").arg("-Vv"))?.stdout)?,
            cargo_codspeed: version,
            artifacts,
        },
    )
}
pub fn measure(root: &Path, output: &Path) -> Result<()> {
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(
        root.join(output).join("results/manifest.json"),
    )?)?;
    ensure!(
        manifest.schema_version == 1 && revision(root)? == manifest.driver_revision,
        "Signal measurement driver changed after preparation"
    );
    ensure!(
        manifest.artifacts.len() == 2 && manifest.expected_results == manifest.cases * 4,
        "Signal measurement catalog changed"
    );
    for artifact in manifest.artifacts {
        ensure!(
            revision(&artifact.checkout)? == artifact.revision,
            "Signal comparison revision changed"
        );
        let package = artifact.checkout.join(PACKAGE);
        for (file, expected) in [
            (WORKLOAD, &manifest.workload_sha256),
            (ACCESS, &artifact.port_sha256),
        ] {
            ensure!(
                sha256(&std::fs::read(package.join(file))?) == *expected,
                "Signal workload changed: {file}"
            );
        }
        ensure!(
            sha256(&std::fs::read(
                package.join(format!("benches/{}.rs", artifact.bench))
            )?) == manifest.benchmark_sha256,
            "Signal benchmark changed"
        );
        println!(
            "CodSpeed classic Signal: {} {}",
            artifact.role, artifact.revision
        );
        run(Command::new("cargo")
            .current_dir(&artifact.checkout)
            .env("CARGO_TARGET_DIR", &artifact.target)
            .args([
                "codspeed",
                "run",
                "-p",
                "wacore-libsignal",
                "--bench",
                &artifact.bench,
            ]))?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counts_the_full_feature_catalog_and_rejects_missing_registration() {
        const SOURCE: &str =
            include_str!("../../../wacore/libsignal/benches/classic_fixture/mod.rs");
        assert_eq!(case_count(SOURCE).unwrap(), 41);
        assert!(case_count("const OTHER: &[Case] = &[];").is_err());
    }
    #[test]
    fn rewritten_base_keeps_its_feature_and_own_port() {
        let package = tempfile::tempdir().unwrap();
        let original = include_str!("../../../wacore/libsignal/Cargo.toml");
        let expected =
            include_bytes!("../../../wacore/libsignal/benches/classic_fixture/access.rs");
        write(&package.path().join(ACCESS), expected).unwrap();
        let mut cargo = original.to_owned();
        assert_eq!(baseline_port(package.path(), &mut cargo).unwrap(), expected);
        assert_eq!(cargo, original);
    }
    #[test]
    fn historical_base_gets_only_the_missing_feature_and_public_api_port() {
        let package = tempfile::tempdir().unwrap();
        let mut cargo =
            "# bench-internals is not declared here\n[features]\nlegacy-session-interop = []\n"
                .to_owned();
        assert_eq!(
            baseline_port(package.path(), &mut cargo).unwrap(),
            BASE_ACCESS.as_bytes()
        );
        let parsed: toml::Value = toml::from_str(&cargo).unwrap();
        assert!(
            parsed["features"]["bench-internals"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(parsed["features"].get("legacy-session-interop").is_some());
    }
    #[test]
    fn rewritten_base_without_its_port_does_not_fall_back_to_retired_methods() {
        let package = tempfile::tempdir().unwrap();
        let original = "[features]\nbench-internals = []\n";
        let mut cargo = original.to_owned();
        assert!(baseline_port(package.path(), &mut cargo).is_err());
        assert_eq!(cargo, original);
    }
}
