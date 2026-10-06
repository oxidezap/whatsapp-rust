//! Preparation is explicit; only an approved immutable baseline can authorize release.
use crate::consumers::{self, Lane};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    process::Command,
};
use xtask_support::{capture, run as execute};

const POLICY: &str = "tools/xtask/compatibility.json";

pub fn controls(root: &Path, lane: Lane, toolchain: &str) -> Result<()> {
    ensure!(
        lane != Lane::Msrv,
        "run mutation controls on native and WASM; MSRV runs the unmodified hosts"
    );
    let stage = tempfile::Builder::new()
        .prefix("whatsapp-compatibility-control-")
        .tempdir_in(root.parent().context("checkout parent")?)?;
    let archive = capture(
        Command::new("git")
            .args(["archive", "--format=tar", "HEAD"])
            .current_dir(root),
    )?;
    tar::Archive::new(archive.stdout.as_slice()).unpack(stage.path())?;
    let check = || -> Result<std::process::Output> {
        let mut command = Command::new("cargo");
        command
            .arg(format!("+{toolchain}"))
            .args([
                "check",
                "--locked",
                "--manifest-path",
                "tests/api-consumer/Cargo.toml",
                "--lib",
                "--features",
                "sdk",
                "--message-format=json",
            ])
            .current_dir(stage.path())
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("CARGO_TARGET_DIR", stage.path().join("target"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_PROFILE_DEV_DEBUG", "0")
            .env("RUSTFLAGS", "");
        if lane == Lane::Wasm {
            command
                .args(["--target", "wasm32-unknown-unknown"])
                .env("RUSTFLAGS", "--cfg getrandom_backend=\"wasm_js\"");
        }
        Ok(command.output()?)
    };
    let positive = check()?;
    ensure!(
        positive.status.success(),
        "unmodified control must compile first: {}",
        String::from_utf8_lossy(&positive.stderr)
    );
    for (file, before, after, code, needle) in [
        (
            "src/client/builder.rs",
            "pub fn with_enc_handler<H>",
            "pub(crate) fn with_enc_handler<H>",
            "E0624",
            "with_enc_handler",
        ),
        (
            "src/types/enc_handler.rs",
            "pub trait EncHandler: wacore::sync_marker::MaybeSendSync {",
            "pub trait EncHandler: wacore::sync_marker::MaybeSendSync {\nfn compatibility_required_method(&self);",
            "E0046",
            "compatibility_required_method",
        ),
    ] {
        let path = stage.path().join(file);
        let original = std::fs::read_to_string(&path)?;
        ensure!(
            original.matches(before).count() == 1,
            "mutation anchor changed: {file}; update the control explicitly"
        );
        let result =
            with_mutated_source(&path, &original, &original.replace(before, after), check)?;
        consumers::verify_mutation(&result, code, needle, "src/encapsulation/mod.rs")?;
        println!("{lane:?} control caught {needle} ({code}) in the external host");
    }
    Ok(())
}

fn with_mutated_source<T>(
    path: &Path,
    original: &str,
    mutated: &str,
    check: impl FnOnce() -> Result<T>,
) -> Result<T> {
    std::fs::write(path, mutated)?;
    let result = check();
    std::fs::write(path, original).context("restore mutation control source")?;
    result
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    phase: Phase,
    baseline: Option<String>,
    profiles: Vec<Profile>,
}
#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum Phase {
    Preparing,
    Frozen,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Profile {
    package: String,
    name: String,
    defaults: bool,
    features: Vec<String>,
    // Standalone WASM build. Crypto leaves get their browser entropy backend
    // from the SDK/core host, exercised by the frozen WASM consumer graph.
    wasm: bool,
}
fn sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
impl Policy {
    fn validate(&self, meta: &Value, release: bool) -> Result<()> {
        ensure!(
            self.phase == Phase::Frozen || self.baseline.is_none(),
            "preparation cannot masquerade as a frozen baseline"
        );
        ensure!(
            self.phase != Phase::Frozen || self.baseline.as_deref().is_some_and(sha),
            "frozen policy requires a full immutable commit SHA"
        );
        ensure!(
            !release || self.phase == Phase::Frozen,
            "release blocked: approve production profiles and freeze the RC baseline after API changes"
        );
        let packages = meta["packages"].as_array().context("packages")?;
        let published = packages
            .iter()
            .filter(|p| !p["publish"].as_array().is_some_and(Vec::is_empty))
            .map(|p| p["name"].as_str().context("package name"))
            .collect::<Result<BTreeSet<_>>>()?;
        let selected = self
            .profiles
            .iter()
            .map(|p| p.package.as_str())
            .collect::<BTreeSet<_>>();
        ensure!(
            selected == published,
            "compatibility profiles must cover exactly the published graph; selected {selected:?}, published {published:?}"
        );
        let mut keys = BTreeSet::new();
        for profile in &self.profiles {
            ensure!(
                keys.insert((&profile.package, &profile.name)),
                "duplicate compatibility profile"
            );
            let package = packages
                .iter()
                .find(|p| p["name"] == profile.package)
                .context("unknown profile package")?;
            for feature in &profile.features {
                ensure!(
                    package["features"].get(feature).is_some(),
                    "{}/{} lost feature {feature}",
                    profile.package,
                    profile.name
                );
            }
        }
        Ok(())
    }
}

fn semver_args(profile: &Profile, baseline: &str) -> Vec<String> {
    let mut args = vec![
        "semver-checks".into(),
        "check-release".into(),
        "--release-type".into(),
        "patch".into(),
        "--baseline-rev".into(),
        baseline.into(),
        "-p".into(),
        profile.package.clone(),
        if profile.defaults {
            "--default-features"
        } else {
            "--only-explicit-features"
        }
        .into(),
    ];
    if !profile.features.is_empty() {
        args.extend(["--features".into(), profile.features.join(",")]);
    }
    args
}

fn manifest_at(root: &Path, baseline: &str, path: &str) -> Result<toml::Value> {
    let bytes = capture(
        Command::new("git")
            .args(["show", &format!("{baseline}:{path}")])
            .current_dir(root),
    )?
    .stdout;
    Ok(toml::from_str(std::str::from_utf8(&bytes)?)?)
}

type FeatureGraph = BTreeMap<String, Option<BTreeSet<String>>>;

fn reachable_features(manifest: &toml::Value, profile: &Profile) -> Result<FeatureGraph> {
    let mut pending = profile
        .features
        .iter()
        .cloned()
        .chain(profile.defaults.then(|| "default".into()))
        .collect::<Vec<_>>();
    let mut graph = FeatureGraph::new();
    while let Some(feature) = pending.pop() {
        if graph.contains_key(&feature) {
            continue;
        }
        let definition = manifest
            .get("features")
            .and_then(|v| v.get(&feature))
            .map(|value| -> Result<BTreeSet<String>> {
                value
                    .as_array()
                    .context("feature definition must be an array")?
                    .iter()
                    .map(|edge| {
                        Ok(edge
                            .as_str()
                            .context("feature edge must be a string")?
                            .to_owned())
                    })
                    .collect()
            })
            .transpose()?;
        if let Some(edges) = &definition {
            // Forwarded and weak dependency features remain edges of this node.
            // Only local features have definitions to traverse in this manifest.
            pending.extend(
                edges
                    .iter()
                    .filter(|edge| !edge.starts_with("dep:") && !edge.contains('/'))
                    .cloned(),
            );
        }
        graph.insert(feature, definition);
    }
    Ok(graph)
}

fn feature_contract(before: &toml::Value, after: &toml::Value, profile: &Profile) -> Result<()> {
    ensure!(
        reachable_features(before, profile)? == reachable_features(after, profile)?,
        "{}/{} changed a reachable feature definition; review the contract explicitly",
        profile.package,
        profile.name
    );
    Ok(())
}

fn baseline_manifest_contracts(
    root: &Path,
    baseline: &str,
    policy: &Policy,
    meta: &Value,
) -> Result<()> {
    let workspace = manifest_at(root, baseline, "Cargo.toml")?;
    for profile in &policy.profiles {
        let package = meta["packages"]
            .as_array()
            .context("packages")?
            .iter()
            .find(|p| p["name"] == profile.package)
            .context("profile package")?;
        let path = Path::new(package["manifest_path"].as_str().context("manifest path")?);
        let relative = path
            .strip_prefix(root)?
            .to_str()
            .context("manifest encoding")?;
        let before = manifest_at(root, baseline, relative)?;
        let after: toml::Value = toml::from_str(&std::fs::read_to_string(path)?)?;
        feature_contract(&before, &after, profile)?;
        let old_floor = baseline_msrv(&before, &workspace)?;
        let current_floor = package["rust_version"].as_str().context("published MSRV")?;
        ensure!(
            old_floor == current_floor,
            "{} changed MSRV from {old_floor} to {current_floor}; review the compatibility policy explicitly",
            profile.package
        );
    }
    Ok(())
}

fn baseline_msrv<'a>(before: &'a toml::Value, workspace: &'a toml::Value) -> Result<&'a str> {
    let package = before
        .get("package")
        .context("baseline manifest must declare a package")?;
    match package.get("rust-version") {
        Some(toml::Value::String(version)) => Ok(version),
        None => anyhow::bail!("baseline package must declare an MSRV"),
        Some(value) => {
            ensure!(
                value.get("workspace").and_then(toml::Value::as_bool) == Some(true),
                "baseline MSRV must be a version string or inherit from the workspace"
            );
            workspace
                .get("workspace")
                .and_then(|workspace| workspace.get("package"))
                .and_then(|package| package.get("rust-version"))
                .and_then(toml::Value::as_str)
                .context("baseline workspace must declare an MSRV")
        }
    }
}

pub fn run(root: &Path, release: bool, lane: Lane, toolchain: &str) -> Result<u8> {
    let policy: Policy = serde_json::from_slice(&std::fs::read(root.join(POLICY))?)?;
    let meta: Value = serde_json::from_slice(
        &capture(
            Command::new("cargo")
                .args(["metadata", "--no-deps", "--format-version", "1"])
                .current_dir(root),
        )?
        .stdout,
    )?;
    policy.validate(&meta, release)?;
    if let Some(baseline) = &policy.baseline {
        let exact = capture(
            Command::new("git")
                .args(["rev-parse", &format!("{baseline}^{{commit}}")])
                .current_dir(root),
        )?;
        ensure!(
            String::from_utf8(exact.stdout)?.trim() == baseline,
            "baseline commit mismatch"
        );
        execute(
            Command::new("git")
                .args(["merge-base", "--is-ancestor", baseline, "HEAD"])
                .current_dir(root),
        )?;
        baseline_manifest_contracts(root, baseline, &policy, &meta)?;
        if lane == Lane::Native {
            for profile in &policy.profiles {
                // Proc-macro signatures do not establish expansion compatibility.
                // Frozen downstream hosts below exercise the emitted implementations.
                if profile.package == "wacore-derive" {
                    continue;
                }
                println!(
                    "compatibility {}/{} against {baseline}",
                    profile.package, profile.name
                );
                execute(
                    Command::new("cargo")
                        .args(semver_args(profile, baseline))
                        .current_dir(root),
                )?;
            }
        }
    } else {
        println!(
            "API preparation: checking candidate profiles and current consumers; no RC compatibility claim or release authorization"
        );
    }
    // Do not gate intentional pre-1.0 changes against the published 0.7.0 API.
    // Every declared profile must still build after freezing: an old consumer
    // does not necessarily enable every feature that promises MSRV/WASM support.
    for profile in &policy.profiles {
        if lane == Lane::Wasm && !profile.wasm {
            continue;
        }
        let mut command = Command::new("cargo");
        command
            .arg(format!("+{toolchain}"))
            .args(["check", "--locked", "--lib", "-p", &profile.package])
            .current_dir(root)
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .env("RUSTFLAGS", "");
        if !profile.defaults {
            command.arg("--no-default-features");
        }
        let mut features = profile.features.clone();
        if lane == Lane::Wasm {
            command
                .args(["--target", "wasm32-unknown-unknown"])
                .env("RUSTFLAGS", "--cfg getrandom_backend=\"wasm_js\"");
            if profile.package == "wacore" {
                features.push("js".into());
            }
        }
        if !features.is_empty() {
            command.args(["--features", &features.join(",")]);
        }
        execute(&mut command)?;
    }
    if let Some(baseline) = &policy.baseline {
        return crate::package_consumers::frozen(root, baseline, lane, toolchain);
    }
    consumers::run(
        root,
        consumers::Task::Run {
            lane,
            toolchain: Some(toolchain.into()),
            manifest: None,
            dry_run: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_control_restores_source_and_preserves_spawn_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.rs");
        let error = with_mutated_source(&path, "original", "mutated", || -> Result<()> {
            assert_eq!(std::fs::read_to_string(&path).unwrap(), "mutated");
            anyhow::bail!("could not spawn cargo")
        })
        .unwrap_err();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "original");
        assert_eq!(error.to_string(), "could not spawn cargo");
    }

    #[test]
    fn baseline_msrv_requires_a_package_and_valid_explicit_or_inherited_version() {
        let workspace: toml::Value =
            toml::from_str("[workspace.package]\nrust-version = '1.94'").unwrap();
        for (manifest, expected) in [
            ("[package]\nrust-version = '1.93'", "1.93"),
            ("[package]\nrust-version.workspace = true", "1.94"),
        ] {
            assert_eq!(
                baseline_msrv(&toml::from_str(manifest).unwrap(), &workspace).unwrap(),
                expected
            );
        }
        for manifest in [
            "[workspace]",
            "[package]",
            "[package]\nrust-version = 194",
            "[package]\nrust-version.workspace = false",
        ] {
            assert!(baseline_msrv(&toml::from_str(manifest).unwrap(), &workspace).is_err());
        }
    }
    fn policy() -> Policy {
        serde_json::from_str(r#"{"phase":"preparing","baseline":null,"profiles":[{"package":"sdk","name":"minimal","defaults":false,"features":["host"],"wasm":true}]}"#).unwrap()
    }
    fn metadata() -> Value {
        serde_json::json!({"packages":[{"name":"sdk","features":{"host":[]},"publish":null}]})
    }
    #[test]
    fn preparation_never_authorizes_release_or_fabricates_a_baseline() {
        let mut policy = policy();
        policy.validate(&metadata(), false).unwrap();
        assert!(policy.validate(&metadata(), true).is_err());
        policy.phase = Phase::Frozen;
        assert!(policy.validate(&metadata(), true).is_err());
        policy.baseline = Some("main".into());
        assert!(policy.validate(&metadata(), true).is_err());
        policy.baseline = Some("a".repeat(40));
        policy.validate(&metadata(), true).unwrap();
    }
    #[test]
    fn rejects_missing_sdk_adapter_or_feature_contract() {
        let policy = policy();
        let mut meta = metadata();
        meta["packages"][0]["features"] = serde_json::json!({});
        assert!(policy.validate(&meta, false).is_err());
        meta = metadata();
        meta["packages"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"name":"adapter","publish":null}));
        assert!(policy.validate(&meta, false).is_err());
    }
    #[test]
    fn version_bumps_cannot_disable_break_detection_and_features_are_explicit() {
        let policy = policy();
        let args = semver_args(&policy.profiles[0], &"a".repeat(40));
        assert!(args.windows(2).any(|w| w == ["--release-type", "patch"]));
        assert!(args.contains(&"--only-explicit-features".into()));
        assert!(!args.contains(&"--all-features".into()));
    }

    #[test]
    fn default_and_opt_in_contracts_cannot_change_silently() {
        let mut policy = policy();
        let before: toml::Value =
            toml::from_str("[features]\ndefault = [\"host\"]\nhost = [\"dep:adapter\"]").unwrap();
        let after: toml::Value = toml::from_str("[features]\ndefault = []\nhost = []").unwrap();
        assert!(feature_contract(&before, &after, &policy.profiles[0]).is_err());
        policy.profiles[0].features.clear();
        policy.profiles[0].defaults = true;
        assert!(feature_contract(&before, &after, &policy.profiles[0]).is_err());
        feature_contract(&before, &before, &policy.profiles[0]).unwrap();
    }

    #[test]
    fn unchanged_default_cannot_hide_changed_child_forwarding() {
        let mut policy = policy();
        let profile = &mut policy.profiles[0];
        profile.features.clear();
        profile.defaults = true;
        let before: toml::Value = toml::from_str(
            r#"
[features]
default = ["sqlite-storage-bundled"]
sqlite-storage-bundled = ["sqlite-storage", "store/bundled-sqlite"]
sqlite-storage = ["dep:store"]
unrelated = ["dep:experimental"]
"#,
        )
        .unwrap();
        let mut after = before.clone();
        after["features"]["sqlite-storage-bundled"] =
            toml::Value::Array(vec!["sqlite-storage".into()]);
        assert!(feature_contract(&before, &after, profile).is_err());
        after = before.clone();
        after["features"]["unrelated"] = toml::Value::Array(vec![]);
        feature_contract(&before, &after, profile).unwrap();
    }

    #[test]
    fn reachable_feature_graph_handles_cycles_and_weak_forwarding() {
        let mut policy = policy();
        let profile = &mut policy.profiles[0];
        profile.features = vec!["a".into()];
        let before: toml::Value = toml::from_str(
            "[features]\na = [\"b\", \"dep:adapter\"]\nb = [\"a\", \"adapter?/host\"]",
        )
        .unwrap();
        let reordered: toml::Value = toml::from_str(
            "[features]\na = [\"dep:adapter\", \"b\"]\nb = [\"adapter?/host\", \"a\"]",
        )
        .unwrap();
        feature_contract(&before, &reordered, profile).unwrap();
        let mut after = before.clone();
        after["features"]["b"] = toml::Value::Array(vec!["a".into()]);
        assert!(feature_contract(&before, &after, profile).is_err());
    }
}
