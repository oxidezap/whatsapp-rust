//! Rust-owned adapters for separately pinned original Web reference programs.
#![allow(clippy::print_stdout)]
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
mod engine;
mod group;
mod inputs;
mod retry;
mod wam;
#[derive(Parser)]
struct Args {
    #[command(subcommand)]
    command: Task,
}
#[derive(Subcommand)]
enum Task {
    Group {
        whatspec: PathBuf,
        #[arg(long)]
        check: Option<PathBuf>,
    },
    Verify {
        bundles: PathBuf,
        #[arg(long)]
        lock: PathBuf,
    },
    Retry {
        bundles: PathBuf,
        #[arg(long)]
        lock: PathBuf,
        #[arg(long)]
        check: Option<PathBuf>,
    },
    Wam {
        bundles: PathBuf,
        #[arg(long)]
        lock: PathBuf,
        #[arg(long)]
        check: Option<PathBuf>,
        #[arg(long)]
        modules_out: Option<PathBuf>,
    },
}
fn main() -> Result<()> {
    match Args::parse().command {
        Task::Group { whatspec, check } => group::run(&whatspec, check.as_deref()),
        Task::Retry {
            bundles,
            lock,
            check,
        } => retry::run(&bundles, &lock, check.as_deref()),
        Task::Wam {
            bundles,
            lock,
            check,
            modules_out,
        } => wam::run(&bundles, &lock, check.as_deref(), modules_out.as_deref()),
        Task::Verify { bundles, lock } => {
            println!(
                "{} locked bundles verified",
                inputs::verify(&bundles, &lock)?.len()
            );
            Ok(())
        }
    }
}
