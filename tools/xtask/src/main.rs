//! Rust repository automation: `cargo xt`.
#![allow(clippy::print_stdout, clippy::print_stderr)]
mod ci;
mod compatibility;
mod consumers;
mod package_consumers;
mod size;
mod size_baseline;
mod workflow;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "cargo xt",
    bin_name = "cargo xt",
    about = "Rust maintenance tasks for whatsapp-rust"
)]
struct Args {
    #[command(subcommand)]
    task: Task,
}
#[derive(Subcommand)]
enum Task {
    /// SHA-256 of a file or explicit hexadecimal bytes.
    Sha256 {
        value: String,
        #[arg(long)]
        hex: bool,
    },
    /// Regenerate whatsapp.desc and its source/descriptor hashes.
    ProtoDesc,
    /// Regenerate the MLOW runtime table descriptor and hashes.
    TablesDesc,
    /// Regenerate the SQLite wire descriptor and hashes.
    WireDesc,
    /// Codec oracle regeneration and fixture packaging (release worker).
    #[command(disable_help_flag = true)]
    Mlow {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Capture acquisition, diagnostics, media and conformance (release worker).
    #[command(disable_help_flag = true)]
    Oracle {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// CI metadata, timed tests, image pins, binary measurements and reporting.
    Ci {
        #[command(subcommand)]
        task: ci::Task,
    },
}
fn main() -> Result<std::process::ExitCode> {
    let root = workspace_root(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let status = match Args::parse().task {
        Task::Sha256 { value, hex } => {
            println!("{}", xtask_support::hash_input(&value, hex)?);
            0
        }
        Task::ProtoDesc => {
            descriptor(&root, "waproto/src/whatsapp", true)?;
            0
        }
        Task::TablesDesc => {
            descriptor(&root, "wacore/src/voip/mlow/tables", false)?;
            0
        }
        Task::WireDesc => {
            descriptor(&root, "storages/sqlite-storage/proto/wire", false)?;
            0
        }
        Task::Mlow { args } => worker(&root, "mlow", &args)?,
        Task::Oracle { args } => worker(&root, "oracle", &args)?,
        Task::Ci { task } => ci::run(&root, task)?,
    };
    Ok(std::process::ExitCode::from(status))
}

fn workspace_root(manifest: &Path) -> Result<PathBuf> {
    // A preserved bootstrap can run after checkout removes its original crate directory.
    Ok(manifest
        .ancestors()
        .nth(2)
        .context("xtask workspace root")?
        .canonicalize()?)
}

#[cfg(test)]
mod bootstrap_tests {
    #[test]
    fn workspace_root_survives_a_checkout_without_the_bootstrap_crate() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("tools/xtask");
        assert!(!manifest.exists());
        assert_eq!(
            super::workspace_root(&manifest).unwrap(),
            root.path().canonicalize().unwrap()
        );
    }
}
fn descriptor(root: &Path, stem: &str, source_info: bool) -> Result<()> {
    xtask_support::descriptor(
        &root.join(format!("{stem}.proto")),
        &root.join(format!("{stem}.desc")),
        source_info,
    )
}

fn worker(root: &Path, task: &str, args: &[std::ffi::OsString]) -> Result<u8> {
    let status =
        std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args([
                "run",
                "--quiet",
                "--release",
                "--locked",
                "-p",
                "whatsapp-oracle-task",
                "--",
                task,
            ])
            .args(args)
            .current_dir(root)
            .status()?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .unwrap_or(1))
}
