//! Rust repository automation: `cargo xt`.
#![allow(clippy::print_stdout, clippy::print_stderr)]
mod ci;
mod mlow;
mod size;
mod workflow;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;

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
    /// Codec oracle regeneration and fixture packaging.
    Mlow {
        #[command(subcommand)]
        task: mlow::Task,
    },
    /// CI metadata, timed tests, image pins, binary measurements and reporting.
    Ci {
        #[command(subcommand)]
        task: ci::Task,
    },
}
fn main() -> Result<std::process::ExitCode> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
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
        Task::Mlow { task } => {
            mlow::run(&root, task)?;
            0
        }
        Task::Ci { task } => ci::run(&root, task)?,
    };
    Ok(std::process::ExitCode::from(status))
}
fn descriptor(root: &Path, stem: &str, source_info: bool) -> Result<()> {
    xtask_support::descriptor(
        &root.join(format!("{stem}.proto")),
        &root.join(format!("{stem}.desc")),
        source_info,
    )
}
