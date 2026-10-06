//! External host contract and intentional removed-API controls.
//!
//! The old unlimited policy name is no longer available:
//! ```compile_fail,E0599
//! use whatsapp_rust::EventDelivery;
//! let _ = EventDelivery::Concurrent;
//! ```
//!
//! Permanent chatstate registration must migrate to an owned Subscription:
//! ```compile_fail,E0599
//! use std::sync::Arc;
//! use whatsapp_rust::Client;
//! fn old_registration(client: &Arc<Client>) {
//!     client.register_chatstate_handler(Arc::new(|_| {}));
//! }
//! ```
//!
//! An incoming chatstate does not offer an outgoing conversion:
//! ```compile_fail,E0277
//! use whatsapp_rust::wacore::{iq::chatstate::ChatstateStanza, protocol::ProtocolNode};
//! fn requires_outgoing<T: ProtocolNode>() {}
//! requires_outgoing::<ChatstateStanza>();
//! ```

#[cfg(all(test, feature = "native"))]
#[path = "../../../event_delivery_public.rs"]
mod contracts;

/// Incoming values can be sent back without translating between SDK/core enums.
/// The caller decides whether relaying a remote contact's state is appropriate.
pub async fn relay_presence_and_activity(
    client: &whatsapp_rust::Client,
    availability: &whatsapp_rust::types::events::PresenceUpdate,
    activity: &whatsapp_rust::types::events::ChatPresenceUpdate,
) -> Result<(), Box<dyn std::error::Error>> {
    let status: wacore::types::presence::PresenceStatus = availability.status;
    let state: whatsapp_rust::ChatActivity = activity.state;
    client.presence().set(status).await?;
    client
        .chatstate()
        .send(&activity.source.chat, state)
        .await?;
    Ok(())
}

#[cfg(test)]
mod presence_contracts {
    use wacore::types::presence::ReceiptType;

    #[test]
    fn incoming_chatstate_has_a_typed_parser_and_activity_can_be_relayed() {
        use whatsapp_rust::wacore::iq::chatstate::{ChatstateSource, ChatstateStanza};
        use whatsapp_rust::{ChatActivity, NodeBuilder};

        let node = NodeBuilder::new("chatstate")
            .attr("from", "12025550111@s.whatsapp.net")
            .children([NodeBuilder::new("composing").attr("media", "audio").build()])
            .build();
        let incoming = ChatstateStanza::parse(&node.as_node_ref()).unwrap();
        assert!(matches!(incoming.source, ChatstateSource::User { .. }));
        assert_eq!(incoming.state, ChatActivity::RecordingAudio);
        let outgoing = incoming.state.into_child_node();
        assert_eq!(outgoing.tag, "composing");
        assert_eq!(outgoing.attrs.get("media").unwrap().as_str(), "audio");
    }

    #[test]
    fn persisted_receipts_keep_existing_json_and_roundtrip() {
        for (variant, name) in [
            (ReceiptType::Delivered, "Delivered"),
            (ReceiptType::Sent, "Sent"),
            (ReceiptType::Sender, "Sender"),
            (ReceiptType::Retry, "Retry"),
            (ReceiptType::EncRekeyRetry, "EncRekeyRetry"),
            (ReceiptType::Read, "Read"),
            (ReceiptType::ReadSelf, "ReadSelf"),
            (ReceiptType::Played, "Played"),
            (ReceiptType::PlayedSelf, "PlayedSelf"),
            (ReceiptType::ServerError, "ServerError"),
            (ReceiptType::Inactive, "Inactive"),
            (ReceiptType::PeerMsg, "PeerMsg"),
            (ReceiptType::HistorySync, "HistorySync"),
        ] {
            let json = serde_json::to_value(&variant).unwrap();
            assert_eq!(json, serde_json::json!(name));
            assert_eq!(
                serde_json::from_value::<ReceiptType>(json).unwrap(),
                variant
            );
            assert_eq!(ReceiptType::parse(variant.as_wire_str()), variant);
        }
        for payload in [
            "",
            "ReadSelf",
            "Delivered",
            "Other",
            "read-self",
            "delivery",
            "new-receipt",
        ] {
            let receipt = ReceiptType::Other(payload.into());
            let json = serde_json::to_value(&receipt).unwrap();
            assert_eq!(json, serde_json::json!({"Other": payload}));
            assert_eq!(
                serde_json::from_value::<ReceiptType>(json).unwrap(),
                receipt
            );
        }
        assert_eq!(ReceiptType::parse(""), ReceiptType::Delivered);
    }
}
