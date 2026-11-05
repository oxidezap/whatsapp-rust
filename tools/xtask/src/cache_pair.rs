//! Same-host samples of immutable cache implementations and identical workloads.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, run, sha256, write, write_json};

use super::prefetch_pair::{
    cargo_codspeed_version, ensure_locked_dependencies, full_sha, revision,
};

const BENCH: &str = "benches/client_cache_maint.rs";
const FIXTURE: &str = "src/client/bench_retry.rs";
const IMPLEMENTATION: &str = "src/portable_cache/implementation.rs";
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
struct Artifact {
    role: String,
    sha: String,
    checkout: PathBuf,
    target: PathBuf,
    name: String,
    implementation_sha256: String,
    fixture_sha256: String,
}

#[derive(Serialize, Deserialize)]
struct Manifest {
    driver_revision: String,
    cargo_codspeed: String,
    rustc: String,
    benchmark_sha256: String,
    forwarded_sha256: String,
    wrapper_sha256: String,
    expected_results: usize,
    artifacts: Vec<Artifact>,
}

fn fixture_runtime(source: &str) -> &str {
    source
        .split_once("\n#[cfg(test)]")
        .map_or(source, |(runtime, _)| runtime)
        .trim_end()
}

fn forward(source: &str) -> Result<String> {
    for line in source
        .lines()
        .filter(|line| line.trim_start().starts_with("#[divan::bench"))
    {
        ensure!(
            line.trim_end().ends_with(")]"),
            "multiline registration needs review"
        );
    }
    let mut forwarded = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("#[divan::bench"))
        .collect::<Vec<_>>()
        .join("\n");
    ensure!(
        !forwarded.contains("#[divan::bench"),
        "multiline registration needs review"
    );
    for name in [
        "cache_upsert_mix",
        "cache_upsert_no_change",
        "client_retry_count",
    ] {
        let signature = format!("fn {name}");
        ensure!(
            forwarded.matches(&signature).count() == 1,
            "missing unique workload {name}"
        );
        forwarded = forwarded.replace(&signature, &format!("pub(super) fn {name}"));
    }
    Ok(forwarded)
}

fn wrapper() -> String {
    let mut mixes = Vec::new();
    let mut no_change = Vec::new();
    let mut retry = Vec::new();
    for round in 0..4 {
        for managed in [false, true] {
            for hit in [0, 50, 100] {
                mixes.push(format!("{managed}_{hit}_round{round}"));
            }
            for hit in [false, true] {
                no_change.push(format!("{managed}_{hit}_round{round}"));
            }
        }
        for seed in [0, 3, 5] {
            retry.push(format!("{seed}_round{round}"));
        }
    }
    format!(
        r#"
#[path = "cache_pair_workload/mod.rs"]
#[allow(dead_code, unused_imports)]
mod workload;
fn main() {{ divan::main(); }}
const MIX: &[&str] = &{mixes:?};
const NO_CHANGE: &[&str] = &{no_change:?};
const RETRY: &[&str] = &{retry:?};
#[divan::bench(args = MIX)]
fn upsert_mix(b: divan::Bencher, case: &str) {{
    let mut parts = case.split('_');
    let managed = parts.next().expect("managed").parse::<bool>().expect("bool");
    let hit = parts.next().expect("hit").parse().expect("percentage");
    if managed {{ workload::cache_upsert_mix::<true>(b, hit); }}
    else {{ workload::cache_upsert_mix::<false>(b, hit); }}
}}
#[divan::bench(args = NO_CHANGE)]
fn upsert_no_change(b: divan::Bencher, case: &str) {{
    let mut parts = case.split('_');
    let managed = parts.next().expect("managed").parse::<bool>().expect("bool");
    let hit = parts.next().expect("hit").parse().expect("bool");
    if managed {{ workload::cache_upsert_no_change::<true>(b, hit); }}
    else {{ workload::cache_upsert_no_change::<false>(b, hit); }}
}}
#[divan::bench(args = RETRY)]
fn client_retry(b: divan::Bencher, case: &str) {{
    let seed = case.split('_').next().expect("seed").parse().expect("count");
    workload::client_retry_count(b, seed);
}}
"#
    )
}

pub fn prepare(root: &Path, base: &str, head: &str, out: &Path) -> Result<()> {
    full_sha(base)?;
    full_sha(head)?;
    let version = cargo_codspeed_version()?;
    let out = root.join(out);
    ensure!(!out.exists(), "refusing to reuse measurement artifacts");
    std::fs::create_dir_all(&out)?;
    let out = out.canonicalize()?;
    let mut artifacts = Vec::new();
    let mut inputs = Vec::new();
    for (role, sha) in [("base", base), ("head", head)] {
        let checkout = out.join(role);
        run(Command::new("git")
            .current_dir(root)
            .args(["worktree", "add", "--detach"])
            .arg(&checkout)
            .arg(sha))?;
        ensure!(revision(&checkout)? == sha, "wrong measured revision");
        inputs.push((
            std::fs::read(checkout.join(BENCH))?,
            std::fs::read_to_string(checkout.join(FIXTURE))?,
        ));
        artifacts.push(Artifact {
            role: role.to_owned(),
            sha: sha.to_owned(),
            target: out.join("target").join(role),
            name: format!("cache_pair_{role}"),
            implementation_sha256: sha256(&std::fs::read(checkout.join(IMPLEMENTATION))?),
            fixture_sha256: sha256(inputs.last().context("fixture")?.1.as_bytes()),
            checkout,
        });
    }
    ensure!(
        inputs[0].0 == inputs[1].0,
        "base/head benchmark bodies differ"
    );
    ensure!(
        fixture_runtime(&inputs[0].1) == fixture_runtime(&inputs[1].1),
        "runtime fixtures differ"
    );
    let forwarded = forward(std::str::from_utf8(&inputs[0].0)?)?;
    let wrapper = wrapper();
    write(&out.join("results/original_workload.rs"), &inputs[0].0)?;
    write(
        &out.join("results/forwarded_workload.rs"),
        forwarded.as_bytes(),
    )?;
    write(&out.join("results/wrapper.rs"), wrapper.as_bytes())?;
    for artifact in &artifacts {
        write(
            &out.join(format!("results/{}_implementation.rs", artifact.role)),
            &std::fs::read(artifact.checkout.join(IMPLEMENTATION))?,
        )?;
        write(
            &out.join(format!("results/{}_fixture.rs", artifact.role)),
            &std::fs::read(artifact.checkout.join(FIXTURE))?,
        )?;
        write(
            &artifact.checkout.join("benches/cache_pair_workload/mod.rs"),
            forwarded.as_bytes(),
        )?;
        write(
            &artifact
                .checkout
                .join(format!("benches/{}.rs", artifact.name)),
            wrapper.as_bytes(),
        )?;
        let path = artifact.checkout.join("Cargo.toml");
        let mut manifest = std::fs::read_to_string(&path)?;
        manifest.push_str(&format!(
            "\n[[bench]]\nname = {:?}\nharness = false\nrequired-features = [\"bench-harness\"]\n",
            artifact.name
        ));
        write(&path, manifest.as_bytes())?;
        write(
            &out.join(format!("results/{}_Cargo.toml", artifact.role)),
            manifest.as_bytes(),
        )?;
        run(Command::new("cargo")
            .current_dir(&artifact.checkout)
            .env("CARGO_TARGET_DIR", &artifact.target)
            .args([
                "codspeed",
                "build",
                "-m",
                "simulation",
                "-m",
                "memory",
                "-p",
                "whatsapp-rust",
                "--features",
                "bench-harness",
                "--bench",
                &artifact.name,
                "--locked",
            ]))?;
        ensure_locked_dependencies(&artifact.checkout)?;
    }
    write_json(
        &out.join("results/manifest.json"),
        &Manifest {
            driver_revision: revision(root)?,
            cargo_codspeed: version,
            rustc: String::from_utf8(capture(Command::new("rustc").arg("-vV"))?.stdout)?,
            benchmark_sha256: sha256(&inputs[0].0),
            forwarded_sha256: sha256(forwarded.as_bytes()),
            wrapper_sha256: sha256(wrapper.as_bytes()),
            expected_results: 208,
            artifacts,
        },
    )
}

pub fn measure(root: &Path, out: &Path) -> Result<()> {
    let out = root.join(out);
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(out.join("results/manifest.json"))?)?;
    ensure!(
        revision(root)? == manifest.driver_revision && manifest.expected_results == 208,
        "measurement contract changed"
    );
    ensure!(
        cargo_codspeed_version()? == manifest.cargo_codspeed,
        "CodSpeed build tool changed"
    );
    for (role, round) in ORDER {
        let artifact = manifest
            .artifacts
            .iter()
            .find(|a| a.role == role)
            .context("missing role")?;
        ensure!(
            revision(&artifact.checkout)? == artifact.sha,
            "measured revision changed"
        );
        ensure_locked_dependencies(&artifact.checkout)?;
        for (path, expected) in [
            (BENCH.to_owned(), &manifest.benchmark_sha256),
            (FIXTURE.to_owned(), &artifact.fixture_sha256),
            (IMPLEMENTATION.to_owned(), &artifact.implementation_sha256),
            (
                "benches/cache_pair_workload/mod.rs".to_owned(),
                &manifest.forwarded_sha256,
            ),
            (
                format!("benches/{}.rs", artifact.name),
                &manifest.wrapper_sha256,
            ),
        ] {
            ensure!(
                sha256(&std::fs::read(artifact.checkout.join(&path))?) == *expected,
                "source changed after build: {path}"
            );
        }
        println!("Cache pair {role} {}, round {round}", artifact.sha);
        run(Command::new("cargo")
            .current_dir(&artifact.checkout)
            .env("CARGO_TARGET_DIR", &artifact.target)
            .args([
                "codspeed",
                "run",
                "-p",
                "whatsapp-rust",
                "--bench",
                &artifact.name,
                &format!("round{round}"),
            ]))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwarded_work_preserves_every_timed_body() {
        let original = include_str!("../../../benches/client_cache_maint.rs");
        let forwarded = forward(original).unwrap();
        for line in original
            .lines()
            .filter(|l| !l.trim_start().starts_with("#[divan::bench"))
        {
            assert!(
                forwarded.contains(line)
                    || forwarded.contains(&line.replace("fn ", "pub(super) fn "))
            );
        }
        assert!(!forwarded.contains("#[divan::bench"));
        let wrapper = wrapper();
        for round in 0..4 {
            assert_eq!(wrapper.matches(&format!("_round{round}\"")).count(), 13);
        }
    }

    #[test]
    fn fixture_contract_ignores_only_tests() {
        assert_eq!(
            fixture_runtime("runtime\n"),
            fixture_runtime("runtime\n#[cfg(test)]\nmod tests {}")
        );
        assert_ne!(
            fixture_runtime("changed\n"),
            fixture_runtime("runtime\n#[cfg(test)]\nmod tests {}")
        );
        assert!(forward("fn client_retry_count() {}\n").is_err());
    }
}
