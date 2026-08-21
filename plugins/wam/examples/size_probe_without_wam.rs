//! Half of the pair that measures what installing the WAM plugin costs.
//!
//! This one builds a client with the plugin host on and no plugin installed;
//! `size_probe_with_wam.rs` is the same program with one line added. Building
//! both under the release profile and subtracting is what
//! `agent_docs/subsystem_boundary.md` asks for, and a pair is the only way to
//! get it: the `demo` example cannot depend on a plugin crate that depends on
//! it, so the comparison has to happen on this side of the edge.
//!
//! Neither is a working client, nothing here connects, and neither is built
//! by an ordinary `cargo build`, since examples of a non-default member are not
//! in the default build.
#![allow(clippy::print_stdout)]

use std::sync::Arc;

use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::wacore::store::InMemoryBackend;
use whatsapp_rust::{Client, TokioRuntime};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let persistence = Arc::new(PersistenceManager::new(Arc::new(InMemoryBackend::new())).await?);
    let client = Client::builder()
        .with_runtime(TokioRuntime)
        .with_persistence_manager(persistence)
        .build()
        .await?
        .into_client();
    println!("{:?}", client.plugin_stats().map(|s| s.plugins.len()));
    Ok(())
}
