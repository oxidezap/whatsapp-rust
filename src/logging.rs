//! Logging must not become a second delivery channel for protocol payloads.
pub(crate) use wacore_binary::diagnostics::NodeSummary as StanzaSummary;

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
            .attr("code", "515")
            .attr("reason", "replaced")
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
            assert!(message.contains("code=515"));
            assert!(message.contains("reason=replaced"));
            assert!(message.contains("body=bytes(24)"));
            for marker in ["synthetic-disconnect-secret", "synthetic-disconnect-key"] {
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
            "Stream error carrying <ack",
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
    fn stanza_logs_preserve_safe_context_at_every_level() {
        if log_capture::delegated_to_child(
            "logging::tests::stanza_logs_preserve_safe_context_at_every_level",
        ) {
            return;
        }
        let capture = log_capture::session();
        let node = NodeBuilder::new("stream:error")
            .attr("code", "599")
            .attr("reason", "replaced")
            .attr("id", "synthetic-stanza-id")
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
            assert!(message.contains("<stream:error code=599"));
            assert!(message.contains("reason=replaced"));
            assert!(message.contains("id=id#"));
            assert!(message.contains("children=1[<key attrs=0 body=bytes(19)>]"));
            for marker in [
                "synthetic-appeal-token",
                "synthetic-key-bytes",
                "synthetic-stanza-id",
            ] {
                assert!(!message.contains(marker));
            }
            if !cfg!(feature = "tracing-pii") {
                assert!(!message.contains("19990000001"));
            }
        }
        for tag in ["chatstate", "xmlstreamend"] {
            let node = NodeBuilder::new(tag).build();
            assert_eq!(
                StanzaSummary::from(&node).to_string(),
                format!("<{tag} attrs=0 body=none>")
            );
        }
        let unknown = NodeBuilder::new("synthetic-secret-tag")
            .attr("code", "synthetic-secret-code")
            .build();
        assert_eq!(
            StanzaSummary::from(&unknown).to_string(),
            "<unknown attrs=1 body=none>"
        );
    }
    #[test]
    fn safe_views_and_enabled_formatting_do_not_allocate() {
        if log_capture::delegated_to_child(
            "logging::tests::safe_views_and_enabled_formatting_do_not_allocate",
        ) {
            return;
        }
        use std::fmt::Write;
        struct Sink;
        impl Write for Sink {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                std::hint::black_box(s);
                Ok(())
            }
        }
        struct FormatLogger;
        impl log::Log for FormatLogger {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                true
            }
            fn log(&self, record: &log::Record<'_>) {
                write!(Sink, "{}", record.args()).expect("valid diagnostic fixture");
            }
            fn flush(&self) {}
        }
        log::set_max_level(log::LevelFilter::Trace);
        let node = NodeBuilder::new("stream:error")
            .attr("code", "515")
            .attr("from", "19990000001:2@s.whatsapp.net")
            .attr("id", "synthetic-message-id")
            .children([NodeBuilder::new("conflict")
                .attr("reason", "replaced")
                .build()])
            .build();
        let borrowed = node.as_node_ref(); // Conversion is setup, never a log argument.
        let jid: wacore_binary::Jid = "19990000001:2@s.whatsapp.net"
            .parse()
            .expect("valid diagnostic fixture");
        // Warm the process's keyed identifier formatter outside attribution.
        write!(Sink, "{}", jid.observe()).expect("valid diagnostic fixture");
        let (sample, ()) = crate::test_alloc::heap_sample(|| {
            for _ in 0..128 {
                std::hint::black_box(StanzaSummary::from(&node));
                log::info!(logger: &FormatLogger, "{}", StanzaSummary::from(&node));
                write!(
                    Sink,
                    "{} {} {} {}",
                    StanzaSummary::from(&node),
                    StanzaSummary::from(&borrowed),
                    jid.observe(),
                    wacore_binary::jid::ObservedJid::from("19990000001:2@s.whatsapp.net")
                )
                .expect("valid diagnostic fixture");
            }
        });
        assert_eq!(sample.allocations, 0, "{sample:?}");
        assert_eq!(sample.peak, 0);
    }

    #[test]
    fn disabled_and_target_filtered_logs_do_not_format_safe_views() {
        if log_capture::delegated_to_child(
            "logging::tests::disabled_and_target_filtered_logs_do_not_format_safe_views",
        ) {
            return;
        }
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Counter<'a>(&'a AtomicUsize);
        impl std::fmt::Display for Counter<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fetch_add(1, Ordering::Relaxed);
                f.write_str("formatted")
            }
        }
        struct Reject;
        impl log::Log for Reject {
            fn enabled(&self, _: &log::Metadata<'_>) -> bool {
                false
            }
            fn log(&self, _: &log::Record<'_>) {}
            fn flush(&self) {}
        }
        let constructions = AtomicUsize::new(0);
        let formats = AtomicUsize::new(0);
        log::set_max_level(log::LevelFilter::Off);
        log::info!(logger: &Reject, "{}", { constructions.fetch_add(1, Ordering::Relaxed); Counter(&formats) });
        assert_eq!(constructions.load(Ordering::Relaxed), 0);
        log::set_max_level(log::LevelFilter::Trace);
        log::info!(logger: &Reject, "{}", { constructions.fetch_add(1, Ordering::Relaxed); Counter(&formats) });
        assert_eq!(constructions.load(Ordering::Relaxed), 1);
        assert_eq!(formats.load(Ordering::Relaxed), 0);
        #[cfg(feature = "tracing")]
        {
            let dispatch = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
            log::set_max_level(log::LevelFilter::Off);
            tracing::dispatcher::with_default(&dispatch, || {
                tracing::info!(peer = %Counter(&formats), "filtered");
            });
        }
        assert_eq!(formats.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn nested_errors_preserve_classification_without_untrusted_text() {
        if log_capture::delegated_to_child(
            "logging::tests::nested_errors_preserve_classification_without_untrusted_text",
        ) {
            return;
        }
        let capture = log_capture::session();
        use wacore::libsignal::protocol::SignalProtocolError;
        #[derive(Debug, thiserror::Error)]
        #[error("{0}")]
        struct ExternalError(&'static str);
        for error in [
            SignalProtocolError::FfiBindingError("synthetic-external-key".into()),
            SignalProtocolError::InvalidSealedSenderMessage("synthetic-external-key".into()),
            SignalProtocolError::BackendError(
                "load_session",
                Box::new(ExternalError("synthetic-external-key")),
            ),
            SignalProtocolError::ApplicationCallbackError(
                "decrypt",
                Box::new(ExternalError("synthetic-external-key")),
            ),
        ] {
            log::warn!(target: "whatsapp_rust::logging", "{error}");
            log::debug!(target: "whatsapp_rust::logging", "{error:#?}");
        }
        let records = capture.records_for("whatsapp_rust::logging");
        assert_eq!(records.len(), 8);
        for (_, message) in &records {
            assert!(!message.contains("synthetic-external-key"));
        }
        for context in ["ffi callback", "sealed sender", "load_session", "decrypt"] {
            assert!(records.iter().any(|(_, message)| message.contains(context)));
        }
        let io = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "synthetic-private-path-key",
        );
        let nested = crate::socket::error::EncryptSendError::transport(io);
        let output = format!("{nested:?}");
        assert!(output.contains("Transport"));
        assert!(output.contains("PermissionDenied"));
        assert!(!output.contains("synthetic-private-path-key"));
        let raw = anyhow::anyhow!("synthetic-server-plaintext");
        let error = crate::request::IqError::ParseError(raw);
        assert!(!format!("{error:#?}").contains("synthetic-server-plaintext"));
        assert!(
            std::error::Error::source(&error)
                .expect("valid diagnostic fixture")
                .to_string()
                .contains("synthetic-server-plaintext")
        );
        let server = wacore::request::IqError::ServerError {
            code: 429,
            text: "synthetic-server-plaintext".into(),
            error_type: Some("wait".into()),
            backoff: Some(60),
        };
        for output in [server.to_string(), format!("{server:#?}")] {
            assert!(output.contains("429"));
            assert!(output.contains("wait"));
            assert!(output.contains("60"));
            assert!(!output.contains("synthetic-server-plaintext"));
        }
    }
    #[test]
    fn string_identifier_views_validate_before_pii_opt_in() {
        use wacore_binary::jid::ObservedJid;
        let invalid = "synthetic-credential";
        assert!(!ObservedJid::from(invalid).to_string().contains(invalid));
        assert_eq!(
            ObservedJid::from("s.whatsapp.net").to_string(),
            "s.whatsapp.net"
        );
        for value in [
            "19990000001:2@s.whatsapp.net",
            "12345@lid",
            "120000000001@g.us",
        ] {
            let jid: wacore_binary::Jid = value.parse().expect("valid diagnostic fixture");
            assert_eq!(
                ObservedJid::from(value).to_string(),
                jid.observe().to_string()
            );
        }
    }

    #[test]
    fn stanza_summary_bounds_work_and_matches_borrowed_shape() {
        let leaf = NodeBuilder::new("key").bytes(vec![0x5a; 8192]).build();
        let node = NodeBuilder::new("iq")
            .attr("id", "synthetic-id")
            .children(
                (0..64)
                    .map(|_| {
                        NodeBuilder::new("error")
                            .attr("code", "500")
                            .children([leaf.clone()])
                            .build()
                    })
                    .collect::<Vec<_>>(),
            )
            .build();
        let output = StanzaSummary::from(&node).to_string();
        assert_eq!(output, StanzaSummary::from(&node.as_node_ref()).to_string());
        assert!(output.contains("children=64"));
        assert!(output.contains("code=500"));
        assert!(output.contains("body=bytes(8192)"));
        assert!(output.ends_with(", ...]>"));
        assert!(output.len() < 1400);
        assert!(!output.contains("90, 90"));
    }

    #[cfg(feature = "tracing")]
    #[test]
    fn enabled_tracing_formats_borrowed_views_without_allocating() {
        use std::fmt::Write;
        use tracing_subscriber::{
            Layer,
            layer::{Context, SubscriberExt},
        };
        struct Discard;
        impl Write for Discard {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                std::hint::black_box(s);
                Ok(())
            }
        }
        struct Visitor;
        impl tracing::field::Visit for Visitor {
            fn record_debug(&mut self, _: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                write!(Discard, "{value:?}").expect("valid diagnostic fixture");
            }
        }
        struct FormatLayer;
        impl<S: tracing::Subscriber> Layer<S> for FormatLayer {
            fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
                event.record(&mut Visitor);
            }
        }
        let dispatch = tracing::Dispatch::new(tracing_subscriber::registry().with(FormatLayer));
        let node = NodeBuilder::new("failure")
            .attr("code", "401")
            .attr("from", "19990000001@s.whatsapp.net")
            .build();
        tracing::dispatcher::with_default(&dispatch, || {
            let emit = || tracing::info!(node = %StanzaSummary::from(&node), "diagnostic");
            emit(); // Register the callsite and initialize identifier entropy.
            let (sample, ()) = crate::test_alloc::heap_sample(|| {
                for _ in 0..128 {
                    emit();
                }
            });
            assert_eq!(sample.allocations, 0, "{sample:?}");
        });
    }
}
