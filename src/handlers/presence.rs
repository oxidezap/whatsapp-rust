//! Handler for incoming `<presence>` stanzas.

use super::traits::StanzaHandler;
use crate::client::Client;
use async_trait::async_trait;
use log::debug;
use std::sync::Arc;
use wacore::stanza::wire_tags::StanzaTag;
use wacore::types::events::{Event, PresenceUpdate};
use wacore::types::presence::PresenceStatus;

/// Handler for `<presence>` stanzas.
///
/// Parses incoming presence updates and dispatches `Event::Presence` via the event bus.
#[derive(Default)]
pub struct PresenceHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for PresenceHandler {
    fn tag(&self) -> &'static str {
        StanzaTag::Presence.as_str()
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.presence", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        let nr = node.get();
        let from_jid = match nr.get_attr("from").and_then(|v| v.to_jid()) {
            Some(jid) => jid,
            None => {
                debug!(target: "PresenceHandler", "Presence stanza missing or invalid 'from' attribute");
                return true;
            }
        };

        let unavailable = nr
            .get_attr("type")
            .is_some_and(|v| v.as_str() == "unavailable");

        // Parse last_seen from 'last' attribute if present
        let last_seen = nr
            .get_attr("last")
            .map(|v| v.as_str())
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(wacore::time::from_secs);

        debug!(
            target: "PresenceHandler",
            "Received presence from {}: unavailable={}",
            from_jid.observe(), unavailable
        );

        client.core.event_bus.dispatch(Event::Presence(
            PresenceUpdate::builder()
                .from(from_jid)
                .status(if unavailable {
                    PresenceStatus::Unavailable
                } else {
                    PresenceStatus::Available
                })
                .maybe_last_seen(last_seen)
                .build(),
        ));

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::types::events::{ChannelEventHandler, EventInterest, EventKind};
    use wacore_binary::builder::NodeBuilder;

    #[tokio::test]
    async fn availability_event_retains_identity_and_last_seen() {
        let client = crate::test_utils::create_test_client().await;
        let (handler, events) = ChannelEventHandler::with_capacity(4);
        let _subscription = client.subscribe(EventInterest::of(&[EventKind::Presence]), handler);
        let from = "12025550100@s.whatsapp.net";
        for (wire_type, status) in [
            (None, PresenceStatus::Available),
            (Some("available"), PresenceStatus::Available),
            (Some("unavailable"), PresenceStatus::Unavailable),
            (Some("future-value"), PresenceStatus::Available),
        ] {
            let mut node = NodeBuilder::new("presence")
                .attr("from", from)
                .attr("last", "1700000000");
            if let Some(value) = wire_type {
                node = node.attr("type", value);
            }
            PresenceHandler
                .handle(
                    client.clone(),
                    crate::test_utils::node_to_owned_ref(&node.build()),
                    &mut false,
                )
                .await;
            let event = events.recv().await.unwrap();
            let Event::Presence(update) = &*event else {
                panic!("presence event")
            };
            assert_eq!(update.from.to_string(), from);
            assert_eq!(update.status, status);
            assert_eq!(update.last_seen.unwrap().timestamp(), 1700000000);
        }
    }
}
