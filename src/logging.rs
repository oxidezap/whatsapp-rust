//! Logging must not become a second delivery channel for protocol payloads.
use std::fmt;
use wacore::stanza::wire_tags::StanzaTag;
use wacore_binary::{Node, NodeRef};

/// Only closed stanza categories and numeric error codes belong in diagnostics.
/// Attributes, children and content may contain linking secrets even at trace.
/// Consumers that need the payload receive it through the existing events.
pub(crate) struct StanzaSummary<'a> {
    tag: &'a str,
    code: Option<u32>,
}

impl<'a> From<&'a Node> for StanzaSummary<'a> {
    fn from(node: &'a Node) -> Self {
        Self {
            tag: &node.tag,
            code: node.attrs.get("code").and_then(|s| s.as_str().parse().ok()),
        }
    }
}

impl<'a> From<&'a NodeRef<'_>> for StanzaSummary<'a> {
    fn from(node: &'a NodeRef<'_>) -> Self {
        Self {
            tag: node.tag.as_ref(),
            code: node.get_attr("code").and_then(|s| s.as_str().parse().ok()),
        }
    }
}

impl fmt::Display for StanzaSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tag = StanzaTag::try_from(self.tag).map_or("unknown", |tag| tag.as_str());
        write!(f, "<{tag}")?;
        if let Some(code) = self.code {
            write!(f, " code={code}")?;
        }
        f.write_str(" payload=omitted>")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::log_capture;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn disconnect_errors_do_not_reintroduce_stanza_payloads() {
        if log_capture::delegated_to_child(
            "logging::tests::disconnect_errors_do_not_reintroduce_stanza_payloads",
        ) {
            return;
        }
        let capture = log_capture::session();
        let node = NodeBuilder::new("failure")
            .attr("appeal_token", "synthetic-disconnect-secret")
            .attr("from", "19990000005@s.whatsapp.net")
            .bytes(b"synthetic-disconnect-key".to_vec())
            .build();
        let facade = crate::request::IqError::Disconnected(Box::new(node.clone()));
        let core = wacore::request::IqError::Disconnected(Box::new(node.clone()));
        let errors: [&dyn std::error::Error; 2] = [&facade, &core];
        for error in errors {
            log::warn!(target: "whatsapp_rust::logging", "{error}");
            log::error!(target: "whatsapp_rust::logging", "{error:?}");
            log::debug!(target: "whatsapp_rust::logging", "{error:#?}");
        }
        let records = capture.records_for("whatsapp_rust::logging");
        assert_eq!(records.len(), 6);
        for (_, message) in records {
            assert!(message.contains("disconnect") || message.contains("Disconnected"));
            for marker in [
                "synthetic-disconnect-secret",
                "19990000005",
                "synthetic-disconnect-key",
            ] {
                assert!(
                    !message.contains(marker),
                    "disconnect error leaked its retained stanza"
                );
            }
        }
        assert!(matches!(facade, crate::request::IqError::Disconnected(raw) if *raw == node));
        assert!(matches!(core, wacore::request::IqError::Disconnected(raw) if *raw == node));
    }

    #[tokio::test]
    async fn device_reset_logs_follow_the_identifier_privacy_policy() {
        if log_capture::delegated_to_child(
            "logging::tests::device_reset_logs_follow_the_identifier_privacy_policy",
        ) {
            return;
        }
        let capture = log_capture::session();
        let client = crate::test_utils::create_test_client().await;
        let user = "19990000008";
        let record = wacore::store::traits::DeviceListRecord {
            user: user.into(),
            devices: Box::new([]),
            timestamp: 0,
            phash: None,
            raw_id: Some(1),
        };
        client
            .clear_device_record(user, "s.whatsapp.net", &record)
            .await;
        let records = capture.records_for("whatsapp_rust::client::device_registry");
        assert!(
            records
                .iter()
                .any(|(_, message)| message.contains("Clearing device record"))
        );
        if !cfg!(feature = "tracing-pii") {
            assert!(records.iter().all(|(_, message)| !message.contains(user)));
        }
    }

    #[tokio::test]
    async fn error_handlers_log_diagnostics_without_raw_stanzas() {
        if log_capture::delegated_to_child(
            "logging::tests::error_handlers_log_diagnostics_without_raw_stanzas",
        ) {
            return;
        }
        let capture = log_capture::session();
        let client = crate::test_utils::create_test_client().await;
        for tag in ["iq", "stream:error", "failure"] {
            let node = NodeBuilder::new(tag)
                .attr("type", "get")
                .attr("code", "599")
                .attr("from", "19990000003@s.whatsapp.net")
                .attr("appeal_token", "synthetic-handler-secret")
                .children([NodeBuilder::new("unrecognized")
                    .bytes(b"synthetic-handler-key".to_vec())
                    .build()])
                .build();
            match tag {
                "iq" => {
                    crate::handlers::iq::IqHandler::handle_scoped(
                        &client,
                        &crate::test_utils::node_to_owned_ref(&node),
                        &client.connection_shutdown_signal(),
                    )
                    .await
                }
                "stream:error" => client.handle_stream_error(&node.as_node_ref()).await,
                _ => client.handle_connect_failure(&node.as_node_ref()).await,
            }
        }
        let ack = NodeBuilder::new("stream:error")
            .children([NodeBuilder::new("ack")
                .attr("id", "synthetic-ack-secret")
                .attr("class", "19990000009@s.whatsapp.net")
                .build()])
            .build();
        client.handle_stream_error(&ack.as_node_ref()).await;
        let records: Vec<_> = [
            "whatsapp_rust::handlers::iq",
            "whatsapp_rust::client::node_io",
        ]
        .iter()
        .flat_map(|target| capture.records_for(target))
        .collect();
        for diagnostic in [
            "Received unhandled IQ",
            "Unknown stream error",
            "Unknown connect failure",
            "Stream error carrying <ack>",
        ] {
            assert!(
                records
                    .iter()
                    .any(|(_, message)| message.contains(diagnostic)),
                "missing {diagnostic}"
            );
        }
        for (_, message) in records {
            for marker in [
                "19990000003",
                "synthetic-handler-secret",
                "synthetic-handler-key",
                "synthetic-ack-secret",
                "19990000009",
            ] {
                assert!(!message.contains(marker), "sensitive stanza value logged");
            }
        }
    }

    #[test]
    fn stanza_logs_omit_attributes_and_nested_payloads_at_every_level() {
        if log_capture::delegated_to_child(
            "logging::tests::stanza_logs_omit_attributes_and_nested_payloads_at_every_level",
        ) {
            return;
        }
        let capture = log_capture::session();
        let node = NodeBuilder::new("stream:error")
            .attr("code", "599")
            .attr("from", "19990000001@s.whatsapp.net")
            .attr("appeal_token", "synthetic-appeal-token")
            .children([NodeBuilder::new("key")
                .bytes(b"synthetic-key-bytes".to_vec())
                .build()])
            .build();
        for level in [
            log::Level::Error,
            log::Level::Warn,
            log::Level::Info,
            log::Level::Debug,
            log::Level::Trace,
        ] {
            log::log!(target: "whatsapp_rust::logging", level, "{}", StanzaSummary::from(&node));
            log::log!(target: "whatsapp_rust::logging", level, "{}", StanzaSummary::from(&node.as_node_ref()));
        }
        let records = capture.records_for("whatsapp_rust::logging");
        assert_eq!(records.len(), 10);
        for (_, message) in records {
            assert_eq!(message, "<stream:error code=599 payload=omitted>");
        }
        for tag in ["chatstate", "xmlstreamend"] {
            let node = NodeBuilder::new(tag).build();
            assert_eq!(
                StanzaSummary::from(&node).to_string(),
                format!("<{tag} payload=omitted>")
            );
        }
        let unknown = NodeBuilder::new("synthetic-secret-tag")
            .attr("code", "synthetic-secret-code")
            .build();
        assert_eq!(
            StanzaSummary::from(&unknown).to_string(),
            "<unknown payload=omitted>"
        );
    }
}
