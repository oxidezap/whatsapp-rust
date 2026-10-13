use super::traits::StanzaHandler;
use crate::client::Client;
use async_trait::async_trait;
use log::{debug, warn};
use std::sync::Arc;
use wacore::stanza::wire_tags::StanzaTag;
use wacore::xml::DisplayableNodeRef;

/// Handler for `<iq>` (Info/Query) stanzas.
///
/// Processes various query types including:
/// - Ping/pong exchanges
/// - Pairing requests
/// - Feature queries
/// - Settings updates
#[derive(Default)]
pub(crate) struct IqHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for IqHandler {
    fn tag(&self) -> &'static str {
        StanzaTag::Iq.as_str()
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        let shutdown = client.connection_shutdown_signal();
        Self::handle_scoped(&client, &node, &shutdown).await;
        true
    }
}

impl IqHandler {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.iq", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_scoped(
        client: &Arc<Client>,
        node: &Arc<wacore_binary::OwnedNodeRef>,
        shutdown: &wacore::runtime::ShutdownSignal,
    ) {
        let nr = node.get();
        if !client.handle_iq_scoped(nr, shutdown).await {
            if nr.get_attr("type").is_some_and(|s| s.as_str() == "result") {
                debug!(
                    "Received late IQ response (waiter already removed): {}",
                    DisplayableNodeRef(nr)
                );
            } else {
                warn!("Received unhandled IQ: {}", DisplayableNodeRef(nr));
            }
        }
    }
}
