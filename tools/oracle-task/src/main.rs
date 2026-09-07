//! Capture-backed repository tasks, launched by `cargo xt` in release mode.
#![allow(clippy::print_stdout, clippy::print_stderr)]
mod conformance;
mod derive_mlow;
mod media;
mod mlow;
mod oracle;
mod oracle_patch;
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(bin_name = "cargo xt")]
struct Args {
    #[command(subcommand)]
    task: Task,
}
#[derive(Subcommand)]
enum Task {
    /// Codec oracle regeneration and fixture packaging.
    Mlow {
        #[command(subcommand)]
        task: mlow::Task,
    },
    /// Capture acquisition, diagnostics, media and conformance.
    Oracle {
        #[command(subcommand)]
        task: oracle::Task,
    },
}
fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    match Args::parse().task {
        Task::Mlow { task } => mlow::run(&root, task),
        Task::Oracle { task } => oracle::run(&root, task),
    }
}
