//! Same-host CodSpeed samples of immutable library revisions, without relabeling runs.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, run, sha256, write, write_json};

const BENCH: &str = "benches/appstate_prefetch.rs";
const FIXTURE: &str = "src/client/app_state/prefetch_bench.rs";
const ADDRESS_BENCH: &str = "benches/client_usync.rs";
const ADDRESS_ATTRIBUTE: &str = "#[divan::bench(args = USER_COUNTS)]";
const ATTRIBUTE: &str =
    "#[divan::bench(args = [65536, 1048576], sample_count = 1000, sample_size = 1)]";
// Each role/repetition has a distinct benchmark URI. No sample overwrites another.
const ORDER: [(&str, usize); 8] = [
    ("base", 0),
    ("head", 0),
    ("head", 1),
    ("base", 1),
    ("base", 2),
    ("head", 2),
    ("head", 3),
    ("base", 3),
];

#[derive(Serialize, Deserialize)]
struct Revision {
    role: String,
    sha: String,
    checkout: PathBuf,
    target: PathBuf,
    bench: String,
    address_bench: String,
}
#[derive(Serialize, Deserialize)]
struct Manifest {
    schema_version: u32,
    measured_driver_revision: String,
    benchmark_sha256: String,
    fixture_sha256: String,
    address_benchmark_sha256: String,
    expected_walltime_results: usize,
    order: Vec<(String, usize)>,
    revisions: Vec<Revision>,
    rustc: String,
    cargo_codspeed: String,
}

fn revision(root: &Path) -> Result<String> {
    Ok(String::from_utf8(
        capture(
            Command::new("git")
                .current_dir(root)
                .args(["rev-parse", "HEAD"]),
        )?
        .stdout,
    )?
    .trim()
    .to_owned())
}
fn full_sha(sha: &str) -> Result<()> {
    ensure!(
        sha.len() == 40 && sha.bytes().all(|c| c.is_ascii_hexdigit()),
        "expected an immutable full revision"
    );
    Ok(())
}
fn version_response(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<String> {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(stdout),
        String::from_utf8_lossy(stderr)
    );
    // cargo-codspeed 5.0.2 prints its version on stderr and exits 1. This
    // exception applies only to the exact pinned version response, not builds
    // or measurements, whose exit status must still be successful.
    ensure!(
        matches!(code, Some(0 | 1)) && text.trim() == "cargo-codspeed 5.0.2",
        "unexpected cargo-codspeed version response (status {code:?}): {}",
        text.trim()
    );
    Ok(text.trim().to_owned())
}
fn cargo_codspeed_version() -> Result<String> {
    let output = Command::new("cargo")
        .args(["codspeed", "--version"])
        .output()?;
    version_response(output.status.code(), &output.stdout, &output.stderr)
}
fn workload(source: &str) -> Result<String> {
    forward_workload(
        source,
        ATTRIBUTE,
        2,
        &["main", "four_external_blobs", "eight_clients"],
    )
}
fn address_workload(source: &str) -> Result<String> {
    forward_workload(
        source,
        ADDRESS_ATTRIBUTE,
        5,
        &["main", "protocol_address_per_device"],
    )
}
fn forward_workload(
    source: &str,
    attribute: &str,
    count: usize,
    functions: &[&str],
) -> Result<String> {
    let attributes: Vec<_> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("#[divan::bench"))
        .collect();
    ensure!(
        attributes.len() == count && attributes.iter().all(|found| *found == attribute),
        "benchmark registration changed; review the paired workload before measuring"
    );
    let mut output = source.replace(attribute, "");
    for name in functions {
        let definition = format!("fn {name}(");
        ensure!(
            output.matches(&definition).count() == 1,
            "unexpected prefetch function {name}"
        );
        output = output.replace(&definition, &format!("pub(super) fn {name}("));
    }
    Ok(output)
}
fn address_wrapper() -> String {
    r#"#[path = "address_pair_workload/mod.rs"]
#[allow(dead_code)]
mod workload;
fn main() { workload::main(); }
const CASES: &[&str] = &[
    "32_round0", "128_round0", "512_round0", "32_round1", "128_round1", "512_round1",
    "32_round2", "128_round2", "512_round2", "32_round3", "128_round3", "512_round3",
];
#[divan::bench(args = CASES)]
fn protocol_address_per_device(bencher: divan::Bencher, case: &str) {
    let width = case.split_once('_').expect("fixed sample label").0.parse().expect("fixed width");
    workload::protocol_address_per_device(bencher, width);
}
"#
    .to_owned()
}
fn wrapper() -> String {
    // The forwarded benchmark retains its original fixture, timed closure,
    // sample count and sample size. Parsing the label happens outside timing.
    r#"#[path = "prefetch_pair_workload/mod.rs"]
mod workload;
fn main() { workload::main(); }
const CASES: &[&str] = &[
    "65536_round0", "1048576_round0", "65536_round1", "1048576_round1",
    "65536_round2", "1048576_round2", "65536_round3", "1048576_round3",
];
fn bytes(case: &str) -> usize {
    case.split_once('_').expect("fixed sample label").0.parse().expect("fixed byte size")
}
#[divan::bench(args = CASES, sample_count = 1000, sample_size = 1)]
fn four_external_blobs(bencher: divan::Bencher, case: &str) {
    workload::four_external_blobs(bencher, bytes(case));
}
#[divan::bench(args = CASES, sample_count = 1000, sample_size = 1)]
fn eight_clients(bencher: divan::Bencher, case: &str) {
    workload::eight_clients(bencher, bytes(case));
}
"#
    .to_owned()
}

pub fn prepare(root: &Path, base: &str, head: &str, output: &Path) -> Result<()> {
    full_sha(base)?;
    full_sha(head)?;
    ensure!(
        revision(root)? == head,
        "measurement driver must be checked out at the declared head"
    );
    let cargo_codspeed = cargo_codspeed_version()?;
    let output = root.join(output);
    ensure!(
        !output.exists(),
        "refusing to reuse a previous paired measurement directory"
    );
    std::fs::create_dir_all(&output)?;
    let output = output.canonicalize()?;
    let mut revisions = Vec::new();
    let mut inputs = Vec::new();
    for (role, sha) in [("base", base), ("head", head)] {
        let checkout = output.join(role);
        run(Command::new("git")
            .current_dir(root)
            .args(["worktree", "add", "--detach"])
            .arg(&checkout)
            .arg(sha))?;
        ensure!(revision(&checkout)? == sha, "unexpected checkout revision");
        inputs.push((
            std::fs::read(checkout.join(BENCH))?,
            std::fs::read(checkout.join(FIXTURE))?,
            std::fs::read(checkout.join(ADDRESS_BENCH))?,
        ));
        revisions.push(Revision {
            role: role.to_owned(),
            sha: sha.to_owned(),
            checkout,
            target: output.join("target").join(role),
            bench: format!("prefetch_pair_{role}"),
            address_bench: format!("address_pair_{role}"),
        });
    }
    ensure!(
        inputs[0] == inputs[1],
        "base/head prefetch workloads differ; do not compare different fixtures"
    );
    let source = String::from_utf8(inputs[0].0.clone())?;
    let rewritten = workload(&source)?;
    let addresses = address_workload(std::str::from_utf8(&inputs[0].2)?)?;
    write(
        &output.join("results/original_benchmark.rs"),
        source.as_bytes(),
    )?;
    write(&output.join("results/original_fixture.rs"), &inputs[0].1)?;
    write(
        &output.join("results/original_address_benchmark.rs"),
        &inputs[0].2,
    )?;
    write(
        &output.join("results/forwarded_addresses.rs"),
        addresses.as_bytes(),
    )?;
    write(
        &output.join("results/address_wrapper.rs"),
        address_wrapper().as_bytes(),
    )?;
    write(
        &output.join("results/forwarded_workload.rs"),
        rewritten.as_bytes(),
    )?;
    write(
        &output.join("results/paired_wrapper.rs"),
        wrapper().as_bytes(),
    )?;
    for item in &revisions {
        write(
            &item.checkout.join("benches/address_pair_workload/mod.rs"),
            addresses.as_bytes(),
        )?;
        write(
            &item
                .checkout
                .join(format!("benches/{}.rs", item.address_bench)),
            address_wrapper().as_bytes(),
        )?;
        write(
            &item.checkout.join("benches/prefetch_pair_workload/mod.rs"),
            rewritten.as_bytes(),
        )?;
        write(
            &item.checkout.join(format!("benches/{}.rs", item.bench)),
            wrapper().as_bytes(),
        )?;
        let path = item.checkout.join("Cargo.toml");
        let mut manifest = std::fs::read_to_string(&path)?;
        manifest.push_str(&format!(
            "\n[[bench]]\nname = {:?}\nharness = false\nrequired-features = [\"bench-harness\"]\n",
            item.bench
        ));
        manifest.push_str(&format!(
            "\n[[bench]]\nname = {:?}\nharness = false\nrequired-features = [\"bench-harness\"]\n",
            item.address_bench
        ));
        write(&path, manifest.as_bytes())?;
        let patch = capture(Command::new("git").current_dir(&item.checkout).args([
            "diff",
            "--",
            "Cargo.toml",
        ]))?;
        write(
            &output.join(format!("results/{}-registration.patch", item.role)),
            &patch.stdout,
        )?;
        run(Command::new("cargo")
            .current_dir(&item.checkout)
            .env("CARGO_TARGET_DIR", &item.target)
            .args([
                "codspeed",
                "build",
                "-m",
                "walltime",
                "-p",
                "whatsapp-rust",
                "--features",
                "bench-harness",
                "--bench",
                &item.bench,
                "--bench",
                &item.address_bench,
            ]))?;
    }
    write_json(
        &output.join("results/manifest.json"),
        &Manifest {
            schema_version: 2,
            measured_driver_revision: head.to_owned(),
            benchmark_sha256: sha256(&inputs[0].0),
            fixture_sha256: sha256(&inputs[0].1),
            address_benchmark_sha256: sha256(&inputs[0].2),
            expected_walltime_results: 56,
            order: ORDER
                .iter()
                .map(|(role, round)| ((*role).to_owned(), *round))
                .collect(),
            revisions,
            rustc: String::from_utf8(capture(Command::new("rustc").arg("-Vv"))?.stdout)?,
            cargo_codspeed,
        },
    )
}

pub fn measure(root: &Path, output: &Path) -> Result<()> {
    let output = root.join(output);
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(output.join("results/manifest.json"))?)?;
    ensure!(
        manifest.schema_version == 2 && revision(root)? == manifest.measured_driver_revision,
        "measurement driver changed after preparation"
    );
    ensure!(
        manifest.order
            == ORDER
                .iter()
                .map(|(role, round)| ((*role).to_owned(), *round))
                .collect::<Vec<_>>(),
        "paired sample order changed"
    );
    for (role, round) in &manifest.order {
        let item = manifest
            .revisions
            .iter()
            .find(|item| &item.role == role)
            .context("missing paired revision")?;
        ensure!(
            revision(&item.checkout)? == item.sha,
            "paired revision changed after building"
        );
        ensure!(
            sha256(&std::fs::read(item.checkout.join(BENCH))?) == manifest.benchmark_sha256,
            "prefetch workload changed after building"
        );
        ensure!(
            sha256(&std::fs::read(item.checkout.join(FIXTURE))?) == manifest.fixture_sha256,
            "prefetch fixture changed after building"
        );
        ensure!(
            sha256(&std::fs::read(item.checkout.join(ADDRESS_BENCH))?)
                == manifest.address_benchmark_sha256,
            "address workload changed after building"
        );
        println!(
            "CodSpeed paired prefetch: {role} {}, repetition {round}",
            item.sha
        );
        run(Command::new("cargo")
            .current_dir(&item.checkout)
            .env("CARGO_TARGET_DIR", &item.target)
            .args([
                "codspeed",
                "run",
                "-p",
                "whatsapp-rust",
                &format!("round{round}"),
            ]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../benches/appstate_prefetch.rs");
    #[test]
    fn version_probe_accepts_only_the_pinned_response() {
        assert!(version_response(Some(0), b"cargo-codspeed 5.0.2\n", b"").is_ok());
        assert!(version_response(Some(1), b"", b"cargo-codspeed 5.0.2\n").is_ok());
        assert!(version_response(Some(2), b"", b"cargo-codspeed 5.0.2\n").is_err());
        assert!(version_response(None, b"", b"cargo-codspeed 5.0.2\n").is_err());
        assert!(version_response(Some(1), b"", b"network failure\n").is_err());
        assert!(version_response(Some(0), b"cargo-codspeed 5.0.3\n", b"").is_err());
    }
    #[test]
    fn forwards_existing_workload_and_rejects_changed_sampling() {
        let forwarded = workload(SOURCE).unwrap();
        assert!(!forwarded.contains("#[divan::bench"));
        assert_eq!(
            forwarded.replace("pub(super) fn ", "fn "),
            SOURCE.replace(ATTRIBUTE, "")
        );
        assert!(workload(&SOURCE.replace("sample_count = 1000", "sample_count = 999")).is_err());
    }
    #[test]
    fn forwards_address_work_without_changing_sampling_or_batch_size() {
        const ADDRESSES: &str = include_str!("../../../benches/client_usync.rs");
        let forwarded = address_workload(ADDRESSES).unwrap();
        assert_eq!(
            forwarded.replace("pub(super) fn ", "fn "),
            ADDRESSES.replace(ADDRESS_ATTRIBUTE, "")
        );
        assert!(address_workload(&ADDRESSES.replace("args = USER_COUNTS", "args = [32]")).is_err());
        assert!(
            address_workload(&ADDRESSES.replace(
                "fn protocol_address_per_device(",
                "fn renamed_address_work("
            ))
            .is_err()
        );
    }
    #[test]
    fn alternates_roles_without_reusing_sample_uris() {
        let samples: std::collections::BTreeSet<_> = ORDER.into_iter().collect();
        assert_eq!(samples.len(), 8);
        for role in ["base", "head"] {
            assert_eq!(ORDER.iter().filter(|sample| sample.0 == role).count(), 4);
        }
        assert!(full_sha("HEAD").is_err());
        assert!(full_sha("main").is_err());
    }
}
