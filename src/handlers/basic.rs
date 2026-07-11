use super::traits::StanzaHandler;
use crate::client::Client;
use async_trait::async_trait;
use std::sync::Arc;
use wacore_binary::OwnedNodeRef;

/// Handler for `<success>` stanzas.
#[derive(Default)]
pub struct SuccessHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for SuccessHandler {
    fn tag(&self) -> &'static str {
        "success"
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.success", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        client.handle_success(node.get()).await;
        true
    }
}

/// Handler for `<failure>` stanzas.
#[derive(Default)]
pub struct FailureHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for FailureHandler {
    fn tag(&self) -> &'static str {
        "failure"
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.failure", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        client.handle_connect_failure(node.get()).await;
        true
    }
}

/// Handler for `<stream:error>` stanzas.
#[derive(Default)]
pub struct StreamErrorHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for StreamErrorHandler {
    fn tag(&self) -> &'static str {
        "stream:error"
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.stream_error", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        client.handle_stream_error(node.get()).await;
        true
    }
}

/// Handler for `<ack>` stanzas.
#[derive(Default)]
pub struct AckHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for AckHandler {
    fn tag(&self) -> &'static str {
        "ack"
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        client.handle_ack_response_arc(&node);
        true
    }
}
