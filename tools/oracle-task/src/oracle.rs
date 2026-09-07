use anyhow::Result;
use clap::Subcommand;
use std::path::{Path, PathBuf};

#[derive(Subcommand)]
pub enum Task {
    /// Fetch every capture pinned by the WhatsApp wasm oracle.
    Fetch,
    /// Export the first COUNT globals without rewriting unrelated sections.
    ExportGlobals {
        source: PathBuf,
        destination: PathBuf,
        count: u32,
    },
    /// Force the uniquely identified outgoing-offer guard.
    ForceOfferGuard {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Neutralize the profiler guard in the pinned D5 capture.
    NeutralizeThreadProfiler {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Tag the nine pinned D5 outgoing-offer failure sites.
    TagOfferErrorSites {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Verify two persisted audio/video oracle traces byte for byte.
    CompareMedia { expected: PathBuf, actual: PathBuf },
    /// Run the capture, generated-IR and pure-Rust VoIP conformance gates.
    Conformance {
        /// Include the serialized, ignored signaling scenarios.
        #[arg(long)]
        slow: bool,
    },
}

pub fn run(root: &Path, task: Task) -> Result<()> {
    match task {
        Task::Fetch => super::derive_mlow::fetch(root, None),
        Task::ExportGlobals {
            source,
            destination,
            count,
        } => super::oracle_patch::globals(&source, &destination, count),
        Task::ForceOfferGuard {
            source,
            destination,
        } => super::oracle_patch::offer_guard(&source, &destination),
        Task::NeutralizeThreadProfiler {
            source,
            destination,
        } => super::oracle_patch::profiler(&source, &destination),
        Task::TagOfferErrorSites {
            source,
            destination,
        } => super::oracle_patch::offer_errors(&source, &destination),
        Task::CompareMedia { expected, actual } => super::media::compare(&expected, &actual),
        Task::Conformance { slow } => super::conformance::run(root, slow),
    }
}
