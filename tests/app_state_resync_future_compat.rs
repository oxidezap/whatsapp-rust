//! Compile-time guard for tracing-enabled downstream resync callers.
//!
//! Integration tests are separate crates, so this exercises the public future
//! boundary without inheriting the library crate's recursion limit.

use std::future::Future;
use whatsapp_rust::{AppStateResyncMode, Client, WAPatchName};

#[cfg_attr(feature = "tracing", tracing::instrument(skip_all))]
async fn traced_consumer(client: &Client) {
    let _ = client
        .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Incremental)
        .await;
}

fn assert_send<T: Future + Send>(_: T) {}

fn compile_downstream_consumer(client: &Client) {
    assert_send(traced_consumer(client));
}

#[test]
fn tracing_consumer_compiles_at_default_recursion_limit() {
    let _ = compile_downstream_consumer as fn(&Client);
}
