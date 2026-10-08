//! Test the bytes Cargo distributes, with no dependency paths back to the checkout.
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, run as execute};

use crate::consumers::{self, Lane};

#[derive(Debug)]
struct Package {
    version: semver::Version,
    directory: PathBuf,
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

fn published(meta: &Value) -> Result<BTreeMap<String, Package>> {
    let mut packages = BTreeMap::new();
    for p in meta["packages"].as_array().context("metadata packages")? {
        if p["publish"].as_array().is_some_and(Vec::is_empty) {
            continue;
        }
        let name = p["name"].as_str().context("package name")?;
        packages.insert(
            name.into(),
            Package {
                version: p["version"].as_str().context("version")?.parse()?,
                directory: Path::new(p["manifest_path"].as_str().context("manifest")?)
                    .parent()
                    .context("manifest parent")?
                    .to_owned(),
            },
        );
    }
    ensure!(!packages.is_empty(), "no published packages");
    Ok(packages)
}

// Check build dependencies too: a package that builds only inside this workspace
// is not publishable. Dev dependencies do not determine publication order.
fn publication_order(meta: &Value, packages: &BTreeMap<String, Package>) -> Result<Vec<String>> {
    let mut edges = BTreeMap::<String, BTreeSet<String>>::new();
    for p in meta["packages"].as_array().context("packages")? {
        let name = p["name"].as_str().context("name")?;
        if !packages.contains_key(name) {
            continue;
        }
        let dependencies = edges.entry(name.into()).or_default();
        for dep in p["dependencies"].as_array().context("dependencies")? {
            if dep["kind"] == "dev" {
                continue;
            }
            let target = dep["name"].as_str().context("dependency name")?;
            if let Some(package) = packages.get(target) {
                let requirement: semver::VersionReq =
                    dep["req"].as_str().context("requirement")?.parse()?;
                ensure!(
                    requirement.matches(&package.version),
                    "{name} requires {target} {requirement}, release contains {}",
                    package.version
                );
                dependencies.insert(target.into());
            } else {
                ensure!(
                    dep.get("path").is_none(),
                    "{name} depends on unpublished tooling {target}"
                );
            }
        }
    }
    let mut order = Vec::new();
    while !edges.is_empty() {
        let next = edges
            .iter()
            .find(|(_, dependencies)| dependencies.is_empty())
            .map(|(name, _)| name.clone())
            .context("cycle in publication graph")?;
        edges.remove(&next);
        for dependencies in edges.values_mut() {
            dependencies.remove(&next);
        }
        order.push(next);
    }
    Ok(order)
}

fn copy_tree(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        if matches!(
            entry.file_name().to_str(),
            Some("target" | ".git" | "Cargo.lock")
        ) {
            continue;
        }
        let kind = entry.file_type()?;
        ensure!(
            !kind.is_symlink(),
            "symlink in consumer sources: {}",
            entry.path().display()
        );
        if kind.is_dir() {
            copy_tree(&entry.path(), &to.join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn external_dependencies(
    value: &mut toml::Value,
    packages: &BTreeMap<String, Package>,
) -> Result<()> {
    let Some(table) = value.as_table_mut() else {
        return Ok(());
    };
    for (section, content) in table {
        if matches!(
            section.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies"
        ) {
            for (alias, dependency) in content.as_table_mut().context("dependency table")? {
                let Some(spec) = dependency.as_table_mut() else {
                    continue;
                };
                let name = spec
                    .get("package")
                    .and_then(toml::Value::as_str)
                    .unwrap_or(alias);
                if let Some(package) = packages.get(name) {
                    spec.remove("path");
                    spec.insert(
                        "version".into(),
                        toml::Value::String(format!("={}", package.version)),
                    );
                } else {
                    ensure!(
                        !spec.contains_key("path"),
                        "consumer path dependency {name} is not a published artifact"
                    );
                }
            }
        } else {
            external_dependencies(content, packages)?;
        }
    }
    Ok(())
}

fn assert_resolution(
    meta: &Value,
    packages: &BTreeMap<String, Package>,
    staged: &Path,
    checkout: &Path,
) -> Result<()> {
    for p in meta["packages"].as_array().context("resolved packages")? {
        let path = Path::new(p["manifest_path"].as_str().context("resolved manifest")?);
        ensure!(
            !path.starts_with(checkout),
            "resolution escaped into checkout: {}",
            path.display()
        );
        if let Some(package) = p["name"].as_str().and_then(|name| packages.get(name)) {
            ensure!(
                path.starts_with(staged) && p["version"] == package.version.to_string(),
                "production dependency did not resolve to packaged bytes: {}",
                path.display()
            );
        }
    }
    Ok(())
}

pub fn run(root: &Path, lane: Lane, toolchain: &str) -> Result<u8> {
    let stage = tempfile::Builder::new()
        .prefix("whatsapp-package-consumers-")
        .tempdir_in(root.parent().context("checkout parent")?)?;
    let (root, staged) = qualification_paths(root, stage.path())?;
    let meta = metadata(&root)?;
    let packages = published(&meta)?;
    let order = publication_order(&meta, &packages)?;
    // Keep the owner alive until the consumers finish.
    run_staged(&root, &root, &staged, lane, toolchain, &packages, &order)
}

pub fn frozen(root: &Path, baseline: &str, lane: Lane, toolchain: &str) -> Result<u8> {
    let stage = tempfile::Builder::new()
        .prefix("whatsapp-frozen-consumers-")
        .tempdir_in(root.parent().context("checkout parent")?)?;
    let (root, staged) = qualification_paths(root, stage.path())?;
    let root = &root;
    let source = tempfile::tempdir_in(root.parent().context("checkout parent")?)?;
    let archive = capture(
        Command::new("git")
            .args([
                "archive",
                "--format=tar",
                baseline,
                "tests",
                "tools/xtask/consumers.json",
            ])
            .current_dir(root),
    )?;
    tar::Archive::new(archive.stdout.as_slice()).unpack(source.path())?;
    let meta = metadata(root)?;
    let packages = published(&meta)?;
    let order = publication_order(&meta, &packages)?;
    run_staged(
        root,
        source.path(),
        &staged,
        lane,
        toolchain,
        &packages,
        &order,
    )
}

fn qualification_paths(root: &Path, stage: &Path) -> Result<(PathBuf, PathBuf)> {
    let root = root.canonicalize()?;
    let stage = stage.canonicalize()?;
    ensure!(
        !stage.starts_with(&root),
        "package qualification must run outside the checkout"
    );
    Ok((root, stage))
}

fn run_staged(
    root: &Path,
    consumer_source: &Path,
    stage: &Path,
    lane: Lane,
    toolchain: &str,
    packages: &BTreeMap<String, Package>,
    order: &[String],
) -> Result<u8> {
    let commit = String::from_utf8(
        capture(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(root),
        )?
        .stdout,
    )?;
    let commit = commit.trim();
    println!("Packaging commit {commit} for {lane:?}");
    let artifacts = stage.join("artifacts");
    let extracted = stage.join("packages");
    let mut command = Command::new("cargo");
    command
        .args(["package", "--no-verify", "--locked", "--target-dir"])
        .arg(&artifacts)
        .current_dir(root);
    for name in order {
        command.args(["-p", name]);
    }
    execute(&mut command)?;
    std::fs::create_dir_all(&extracted)?;
    let mut patches = toml::Table::new();
    for name in order {
        let package = &packages[name];
        let stem = format!("{name}-{}", package.version);
        let archive = artifacts.join("package").join(format!("{stem}.crate"));
        let bytes = std::fs::read(&archive)
            .with_context(|| format!("missing package {}", archive.display()))?;
        println!(
            "artifact {stem}: {}",
            xtask_support::hash_input(archive.to_str().context("archive path")?, false)?
        );
        let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()));
        for entry in tar.entries()? {
            let mut entry = entry?;
            ensure!(
                entry.header().entry_type().is_file() || entry.header().entry_type().is_dir(),
                "links in package {name}"
            );
            ensure!(
                entry.path()?.starts_with(&stem),
                "unexpected package root for {name}"
            );
            ensure!(
                entry.unpack_in(&extracted)?,
                "unsafe archive path in {name}"
            );
        }
        let path = extracted.join(&stem);
        let vcs: Value =
            serde_json::from_slice(&std::fs::read(path.join(".cargo_vcs_info.json"))?)?;
        ensure!(
            vcs["git"]["sha1"] == commit && vcs["git"]["dirty"] != true,
            "{name} was not packaged from clean commit {commit}"
        );
        ensure!(
            path.join("Cargo.toml").is_file(),
            "package manifest missing: {name}"
        );
        patches.insert(
            name.clone(),
            toml::Value::Table(toml::Table::from_iter([(
                "path".into(),
                toml::Value::String(path.to_str().context("package path")?.into()),
            )])),
        );
        // Cargo's normalized manifest is authoritative; never rewrite its requirements.
        let packaged: toml::Value =
            toml::from_str(&std::fs::read_to_string(path.join("Cargo.toml"))?)?;
        ensure!(
            packaged["package"]["version"].as_str() == Some(&package.version.to_string()),
            "packaged version drift"
        );
        println!("qualified source: {}", package.directory.display());
    }
    copy_tree(&consumer_source.join("tests"), &stage.join("tests"))?;
    let registry = "tools/xtask/consumers.json";
    let consumers: Value = serde_json::from_slice(&std::fs::read(consumer_source.join(registry))?)?;
    std::fs::create_dir_all(stage.join("tools/xtask"))?;
    std::fs::copy(consumer_source.join(registry), stage.join(registry))?;
    std::fs::create_dir_all(stage.join(".cargo"))?;
    let config = toml::Table::from_iter([(
        "patch".into(),
        toml::Value::Table(toml::Table::from_iter([(
            "crates-io".into(),
            toml::Value::Table(patches),
        )])),
    )]);
    std::fs::write(stage.join(".cargo/config.toml"), toml::to_string(&config)?)?;
    for consumer in consumers.as_array().context("consumer registry")? {
        let manifest = stage.join(consumer["manifest"].as_str().context("consumer manifest")?);
        let mut contents: toml::Value = toml::from_str(&std::fs::read_to_string(&manifest)?)?;
        external_dependencies(&mut contents, packages)?;
        std::fs::write(&manifest, toml::to_string(&contents)?)?;
        let resolved: Value = serde_json::from_slice(
            &capture(
                Command::new("cargo")
                    .arg(format!("+{toolchain}"))
                    // Inspect every optional edge before the isolated builds.
                    // This resolves metadata only; each build still uses its
                    // registered profile, never an all-features compilation.
                    .args([
                        "metadata",
                        "--all-features",
                        "--format-version",
                        "1",
                        "--manifest-path",
                    ])
                    .arg(&manifest)
                    .current_dir(stage)
                    .env_remove("CARGO_ENCODED_RUSTFLAGS")
                    .env("RUSTFLAGS", ""),
            )?
            .stdout,
        )?;
        assert_resolution(&resolved, packages, &extracted, root)?;
    }
    consumers::run(
        stage,
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
    #[cfg(unix)]
    #[test]
    fn symlinked_checkout_and_stage_accept_packages_but_reject_checkout_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("checkout");
        let stage = dir.path().join("stage");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&stage).unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
        let (checkout, staged) =
            qualification_paths(&alias.join("checkout"), &alias.join("stage")).unwrap();
        for (path, accepted) in [
            (stage.join("Cargo.toml"), true),
            (root.join("Cargo.toml"), false),
        ] {
            let meta = serde_json::json!({"packages":[{"name":"sdk", "version":"1.0.0-rc.1", "manifest_path":path}]});
            assert_eq!(
                assert_resolution(&meta, &packages(), &staged, &checkout).is_ok(),
                accepted
            );
        }
        assert!(qualification_paths(&root, &alias.join("checkout")).is_err());
    }
    fn packages() -> BTreeMap<String, Package> {
        BTreeMap::from([(
            "sdk".into(),
            Package {
                version: "1.0.0-rc.1".parse().unwrap(),
                directory: "/checkout".into(),
            },
        )])
    }
    #[test]
    fn rewrites_target_dependencies_and_keeps_feature_contract() {
        let mut value: toml::Value = toml::from_str(r#"[target.'cfg(unix)'.dependencies]
renamed = { package = "sdk", path = "../..", default-features = false, features = ["host"], optional = true }
"#).unwrap();
        external_dependencies(&mut value, &packages()).unwrap();
        let dep = &value["target"]["cfg(unix)"]["dependencies"]["renamed"];
        assert!(dep.get("path").is_none());
        assert_eq!(dep["version"].as_str(), Some("=1.0.0-rc.1"));
        assert_eq!(dep["default-features"].as_bool(), Some(false));
        assert_eq!(dep["features"][0].as_str(), Some("host"));
        assert_eq!(dep["optional"].as_bool(), Some(true));
    }
    #[test]
    fn refuses_tooling_and_registry_fallback() {
        let mut value: toml::Value =
            toml::from_str("[dependencies]\noracle = { path = \"../../tools/oracle-core\" }")
                .unwrap();
        assert!(external_dependencies(&mut value, &packages()).is_err());
        for path in ["/checkout/sdk/Cargo.toml", "/registry/sdk/Cargo.toml"] {
            let meta = serde_json::json!({"packages":[{"name":"sdk", "version":"1.0.0-rc.1", "manifest_path":path}]});
            assert!(
                assert_resolution(
                    &meta,
                    &packages(),
                    Path::new("/staged"),
                    Path::new("/checkout")
                )
                .is_err()
            );
        }
    }
    #[test]
    fn validates_prerelease_version_graph_and_build_dependencies() {
        let meta =
            |dep: Value| serde_json::json!({"packages":[{"name":"sdk", "dependencies":[dep]}]});
        assert!(
            publication_order(
                &meta(serde_json::json!({"name":"host-tool", "kind":"build", "path":"/tools"})),
                &packages()
            )
            .is_err()
        );
        assert!(
            publication_order(
                &meta(serde_json::json!({"name":"sdk", "req":"^0.7", "kind":null})),
                &packages()
            )
            .is_err()
        );
        assert!(
            publication_order(
                &meta(serde_json::json!({"name":"sdk", "req":"=1.0.0-rc.1", "kind":null})),
                &packages()
            )
            .is_err()
        );
    }
}
