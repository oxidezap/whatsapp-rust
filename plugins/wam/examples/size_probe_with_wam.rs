//! The other half of the size probe: the same client with the WAM plugin
//! installed. See `size_probe_without_wam.rs` for what the pair is for.
#![allow(clippy::print_stdout)]

use std::sync::Arc;

use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::wacore::store::InMemoryBackend;
use whatsapp_rust::{Client, TokioRuntime};
use whatsapp_rust_plugin_wam::WamPlugin;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let persistence = Arc::new(PersistenceManager::new(Arc::new(InMemoryBackend::new())).await?);
    let client = Client::builder()
        .with_runtime(TokioRuntime)
        .with_persistence_manager(persistence)
        .with_plugin(WamPlugin::default())
        .build()
        .await?
        .into_client();
    println!("{:?}", client.plugin_stats().map(|s| s.plugins.len()));
    Ok(())
}
