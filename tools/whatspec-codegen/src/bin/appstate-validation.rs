//! Independently pinned app-state boundary; keeps unrelated domain pins stable.
#![allow(clippy::print_stdout, clippy::print_stderr)]
extern crate self as wa_ir;
#[path = "../appstate_validation_ir.rs"]
mod model;
pub use model::*;
#[path = "../emit/appstate_validation.rs"]
mod emit;
#[allow(dead_code)]
#[path = "../naming.rs"]
mod naming;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
fn main() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fixtures = root.join("tools/whatspec-codegen/fixtures/appstate-validation");
    let check = std::env::args().skip(1).any(|arg| arg == "--check");
    let lock: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fixtures.join("lock.json"))?)?;
    let revision = lock["rev"].as_str().context("missing app-state revision")?;
    ensure!(
        revision.len() == 40 && revision.bytes().all(|b| b.is_ascii_hexdigit()),
        "app-state pin must be an immutable commit SHA"
    );
    let mut inputs = Vec::new();
    for name in ["validation.json", "validation_batch.json"] {
        let bytes = std::fs::read(fixtures.join(name))?;
        ensure!(
            lock["files"][name].as_str()
                == Some(
                    &Sha256::digest(&bytes)
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                ),
            "app-state IR digest mismatch: {name}"
        );
        inputs.push(String::from_utf8(bytes)?);
    }
    let protobuf: AppstateValidationIr = serde_json::from_str(&inputs[0])?;
    let batch: AppstateBatchValidationIr = serde_json::from_str(&inputs[1])?;
    ensure!(
        protobuf.wa_version == batch.wa_version,
        "mixed app-state snapshots"
    );
    let artifacts = [
        (
            "validation_generated.rs",
            emit::generate_appstate_validation(&protobuf).map_err(anyhow::Error::msg)?,
        ),
        (
            "validation_batch_generated.rs",
            emit::generate_appstate_batch_validation(&batch).map_err(anyhow::Error::msg)?,
        ),
        (
            "validation_value.rs",
            include_str!("../emit/appstate_validation_value.rs.txt").into(),
        ),
    ];
    for (name, source) in artifacts {
        let source = rustfmt(&root, &source)?;
        let path = root.join("wacore/appstate/src").join(name);
        if check {
            ensure!(
                std::fs::read_to_string(&path).with_context(|| format!("reading {name}"))?
                    == source,
                "stale generated {name}"
            );
        } else {
            std::fs::write(&path, source)?;
        }
    }
    println!(
        "app-state boundary {} at {}: {}",
        protobuf.wa_version,
        lock["rev"].as_str().unwrap_or("unpublished"),
        if check { "verified" } else { "generated" }
    );
    Ok(())
}

fn rustfmt(root: &std::path::Path, source: &str) -> Result<String> {
    let dir = root.join("target/whatspec-codegen");
    std::fs::create_dir_all(&dir)?;
    // Per-process, because CI's `--check` and a developer regenerating in the
    // same checkout would otherwise overwrite each other's scratch between the
    // write and the read-back, and the loser commits the winner's bytes.
    let path = dir.join(format!("scratch-{}.rs", std::process::id()));
    std::fs::write(&path, source)?;
    let out = std::process::Command::new("rustfmt")
        .current_dir(root)
        .arg("--edition")
        .arg("2024")
        .arg(&path)
        .output()
        .context("running rustfmt (install it with `rustup component add rustfmt`)")?;
    ensure!(
        out.status.success(),
        "rustfmt rejected generated source: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let formatted = std::fs::read_to_string(&path).context("reading back the formatted source")?;
    let _ = std::fs::remove_file(&path);
    Ok(formatted)
}
