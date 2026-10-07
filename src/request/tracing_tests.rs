use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use tracing_subscriber::{Layer, layer::Context, prelude::*, registry::LookupSpan};
use wacore::store::commands::DeviceCommand;
use wacore_binary::builder::NodeBuilder;

#[derive(Clone, Debug)]
pub(crate) struct Record {
    pub name: String,
    pub level: tracing::Level,
    pub fields: BTreeMap<String, String>,
    pub spans: Vec<(String, BTreeMap<String, String>)>,
}
#[derive(Clone, Default)]
pub(crate) struct Capture(pub Arc<Mutex<Vec<Record>>>);
#[derive(Default, Clone)]
struct Fields(BTreeMap<String, String>);
impl tracing::field::Visit for Fields {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.insert(field.name().into(), format!("{value:?}"));
    }
}
impl<S: tracing::Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::Id,
        ctx: Context<'_, S>,
    ) {
        let mut fields = Fields::default();
        attrs.record(&mut fields);
        ctx.span(id).unwrap().extensions_mut().insert(fields);
    }
    fn on_record(&self, id: &tracing::Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) {
        values.record(
            ctx.span(id)
                .unwrap()
                .extensions_mut()
                .get_mut::<Fields>()
                .unwrap(),
        );
    }
    fn on_close(&self, id: tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).unwrap();
        self.0.lock().unwrap().push(Record {
            name: span.name().into(),
            level: *span.metadata().level(),
            fields: span.extensions().get::<Fields>().unwrap().0.clone(),
            spans: vec![],
        });
    }
    fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let spans = ctx
            .event_scope(event)
            .into_iter()
            .flat_map(|scope| scope.from_root())
            .map(|span| {
                (
                    span.name().into(),
                    span.extensions().get::<Fields>().unwrap().0.clone(),
                )
            })
            .collect();
        self.0.lock().unwrap().push(Record {
            name: event.metadata().target().into(),
            level: *event.metadata().level(),
            fields: fields.0,
            spans,
        });
    }
}
impl Capture {
    pub fn dispatch(&self, level: tracing::Level) -> tracing::Dispatch {
        tracing::Dispatch::new(
            tracing_subscriber::registry().with(
                self.clone()
                    .with_filter(tracing_subscriber::filter::LevelFilter::from_level(level)),
            ),
        )
    }
    pub fn records(&self) -> Vec<Record> {
        self.0.lock().unwrap().clone()
    }
}
pub(crate) async fn set_identity(client: &crate::Client, n: u64) {
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(
            format!("1555000{n:04}.0:1@s.whatsapp.net").parse().unwrap(),
        )))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            format!("19999000000{n:04}.0:1@lid").parse().unwrap(),
        )))
        .await;
}

#[tokio::test]
async fn iq_rejections_are_debug_events_and_preserve_typed_errors() {
    use tracing::instrument::WithSubscriber;
    use wacore::iq::spec::IqSpec;
    for level in [tracing::Level::DEBUG, tracing::Level::INFO] {
        for node_path in [false, true] {
            for code in [401, 404, 429, 500] {
                let (client, transport) = crate::test_utils::create_iq_test_client().await;
                set_identity(&client, 1).await;
                let capture = Capture::default();
                let dispatch = capture.dispatch(level);
                let request_client = client.clone();
                let request = tokio::spawn(
                    async move {
                        if node_path {
                            request_client
                                .send_iq_node(
                                    NodeBuilder::new("iq").attr("type", "get").build(),
                                    None,
                                )
                                .await
                        } else {
                            request_client
                                .send_iq(wacore::iq::keepalive::KeepaliveSpec::new().build_iq())
                                .await
                        }
                    }
                    .with_subscriber(dispatch),
                );
                let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
                let id = sent.attrs().optional_string("id").unwrap().into_owned();
                let response = crate::test_utils::answer_iq(
                    &client,
                    &id,
                    &NodeBuilder::new("iq")
                        .attr("id", id.as_str())
                        .attr("type", "error")
                        .children([NodeBuilder::new("error")
                            .attr("code", code.to_string())
                            .attr("text", "synthetic rejection")
                            .children([NodeBuilder::new("private-payload-sentinel").build()])
                            .build()])
                        .build(),
                )
                .await;
                let err = request.await.unwrap().unwrap_err();
                assert!(
                    matches!(&err, super::IqError::ServerError { code: actual, response: retained, .. } if *actual == code && Arc::ptr_eq(retained.as_arc(), &response))
                );
                let records = capture.records();
                let errors: Vec<_> = records
                    .iter()
                    .filter(|r| r.fields.contains_key("error"))
                    .collect();
                assert!(
                    errors.iter().all(|r| r.level == tracing::Level::DEBUG),
                    "{records:?}"
                );
                if level == tracing::Level::DEBUG {
                    assert_eq!(errors.len(), 1, "{records:?}");
                    let span = errors[0].spans.last().unwrap();
                    assert_eq!(span.0, if node_path { "wa.iq.node" } else { "wa.iq" });
                    assert_eq!(span.1.get("lid"), client.identity_tags().lid.as_ref());
                    assert_eq!(span.1.get("pn"), client.identity_tags().pn.as_ref());
                    assert!(errors[0].fields["error"].contains(&code.to_string()));
                } else {
                    assert!(errors.is_empty(), "{records:?}");
                }
                assert!(!format!("{records:?}").contains("private-payload-sentinel"));
            }
        }
    }
}

#[tokio::test]
async fn lifecycle_events_follow_each_clients_current_identity() {
    use tracing::instrument::WithSubscriber;
    let capture = Capture::default();
    let dispatch = capture.dispatch(tracing::Level::DEBUG);
    async {
        let a = crate::test_utils::create_test_client().await;
        let b = crate::test_utils::create_test_client().await;
        // Unpaired fields are absent, including after another client was observed.
        a.reconnect().await;
        set_identity(&a, 1).await;
        set_identity(&b, 2).await;
        futures::join!(a.reconnect(), b.reconnect_immediately());
        set_identity(&a, 3).await;
        a.reconnect().await;
        a.pause().await;
        let run = a.run();
        let stop = async {
            tokio::task::yield_now().await;
            a.shutdown().await
        };
        futures::join!(run, stop);
        b.shutdown().await;
        futures::join!(a.run(), b.run());
        b.logout().await;
        let records = capture.records();
        let stopped = records
            .iter()
            .find(|r| {
                r.fields
                    .get("message")
                    .is_some_and(|m| m.contains("run loop has shut down"))
            })
            .unwrap();
        assert_eq!(stopped.fields.get("lid"), a.identity_tags().lid.as_ref());
        assert!(stopped.spans.is_empty());
        for name in [
            "wa.conn.logout",
            "wa.conn.pause",
            "wa.conn.shutdown",
            "wa.conn.reconnect_immediately",
            "wa.conn.cleanup",
        ] {
            assert!(
                records
                    .iter()
                    .any(|r| r.name == name && r.fields.contains_key("lid")),
                "{name}: {records:?}"
            );
        }
        let reconnects: Vec<_> = records
            .iter()
            .filter(|r| r.name == "wa.conn.reconnect")
            .collect();
        assert_eq!(reconnects.len(), 3);
        assert!(!reconnects[0].fields.contains_key("lid"));
        assert!(!reconnects[0].fields.contains_key("pn"));
        assert_eq!(
            reconnects[1].fields["lid"],
            "199990000000001.0:1@lid"
                .parse::<wacore_binary::Jid>()
                .unwrap()
                .to_string()
        );
        assert_eq!(
            reconnects[2].fields.get("lid"),
            a.identity_tags().lid.as_ref()
        );
        let run_events: Vec<_> = records
            .iter()
            .filter(|r| {
                r.fields
                    .get("message")
                    .is_some_and(|m| m.contains("called after shutdown"))
            })
            .collect();
        assert_eq!(run_events.len(), 2, "{records:?}");
        for (event, client) in run_events.iter().zip([&a, &b]) {
            assert_eq!(event.level, tracing::Level::WARN);
            assert_eq!(event.fields.get("lid"), client.identity_tags().lid.as_ref());
            assert_eq!(event.fields.get("pn"), client.identity_tags().pn.as_ref());
            assert!(event.spans.is_empty());
        }
        #[cfg(not(feature = "tracing-pii"))]
        assert!(!format!("{records:?}").contains("1555000"));
        a.persistence_manager
            .process_command(DeviceCommand::SetId(None))
            .await;
        a.persistence_manager
            .process_command(DeviceCommand::SetLid(None))
            .await;
        a.run().await;
        let cleared = capture.records().pop().unwrap();
        assert!(!cleared.fields.contains_key("lid"));
        assert!(!cleared.fields.contains_key("pn"));
    }
    .with_subscriber(dispatch)
    .await;
}

#[tokio::test]
async fn disconnected_iqs_remain_typed_and_quiet_at_info() {
    use tracing::instrument::WithSubscriber;
    use wacore::iq::spec::IqSpec;
    for level in [tracing::Level::DEBUG, tracing::Level::INFO] {
        let capture = Capture::default();
        async {
            let client = crate::test_utils::create_test_client().await;
            let node_result = client
                .send_iq_node(NodeBuilder::new("iq").attr("type", "get").build(), None)
                .await;
            let query_result = client
                .send_iq(wacore::iq::keepalive::KeepaliveSpec::new().build_iq())
                .await;
            assert!(matches!(node_result, Err(super::IqError::NotConnected)));
            assert!(matches!(query_result, Err(super::IqError::NotConnected)));
        }
        .with_subscriber(capture.dispatch(level))
        .await;
        let records = capture.records();
        let errors: Vec<_> = records
            .iter()
            .filter(|r| r.fields.contains_key("error"))
            .collect();
        assert!(
            errors.iter().all(|r| r.level == tracing::Level::DEBUG),
            "{records:?}"
        );
        assert_eq!(
            errors.len(),
            if level == tracing::Level::DEBUG { 2 } else { 0 }
        );
    }
}

#[tokio::test]
async fn review_filtered_operations_do_not_replace_host_identity() {
    use tracing::{Instrument, instrument::WithSubscriber};
    let capture = Capture::default();
    let subscriber = tracing_subscriber::registry().with(capture.clone().with_filter(
        tracing_subscriber::filter::filter_fn(|m| {
            !m.is_span() || m.module_path() == Some(module_path!())
        }),
    ));
    let dispatch = tracing::Dispatch::new(subscriber);
    async {
        let a = crate::test_utils::create_test_client().await;
        let b = crate::test_utils::create_test_client().await;
        set_identity(&a, 1).await;
        set_identity(&b, 2).await;
        // A host can reuse both name and target while filtering library callsites.
        let host = tracing::info_span!(target: "whatsapp_rust::client::lifecycle", "wa.conn.reconnect", lid = %"host-lid", pn = %"host-pn");
        async {
            a.reconnect().await;
            b.reconnect_immediately().await;
            a.shutdown().await;
            b.logout().await;
            let _ = a.send_iq_node(NodeBuilder::new("iq").build(), None).await;
            tracing::info!(target: "host", "host event after client operations");
        }.instrument(host).await;
    }.with_subscriber(dispatch).await;
    let records = capture.records();
    let event = records.iter().find(|r| r.name == "host").unwrap();
    assert_eq!(event.spans.len(), 1);
    assert_eq!(
        event.spans[0].1.get("lid").map(String::as_str),
        Some("host-lid")
    );
    assert_eq!(
        event.spans[0].1.get("pn").map(String::as_str),
        Some("host-pn")
    );
}

#[test]
fn ready_review_maintenance_emits_without_scheduled_identity() {
    use crate::test_utils::log_capture;
    if log_capture::delegated_to_child(
        "request::tracing_tests::ready_review_maintenance_emits_without_scheduled_identity",
    ) {
        return;
    }
    let logs = log_capture::session();
    let capture = Capture::default();
    tracing::dispatcher::with_default(&capture.dispatch(tracing::Level::WARN), || {
        crate::Client::log_engine_maintenance_error(
            None,
            &crate::store::error::StoreError::Validation("late-enabled-maintenance".into()),
        );
    });
    let records = capture.records();
    assert_eq!(
        records.len(),
        1,
        "warning must reach a tracing-only subscriber"
    );
    assert!(
        logs.records_for("Client/Keepalive").is_empty(),
        "tracing emission must not also write through log"
    );
    assert_eq!(records[0].level, tracing::Level::WARN);
    assert!(records[0].fields["message"].contains("late-enabled-maintenance"));
    assert!(!records[0].fields.contains_key("lid"));
    assert!(!records[0].fields.contains_key("pn"));
}

#[cfg(all(feature = "client-lifecycle", not(target_arch = "wasm32")))]
#[tokio::test]
async fn ready_review_dropped_cleanup_is_attributed_at_info() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tracing::instrument::WithSubscriber;
    use wacore::runtime::{AbortHandle, BoxFuture, Runtime};
    struct DropNextSpawn(AtomicBool);
    impl Runtime for DropNextSpawn {
        fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
            if self.0.swap(false, Ordering::SeqCst) {
                drop(future);
                AbortHandle::noop()
            } else {
                crate::TokioRuntime.spawn(future)
            }
        }
        fn sleep(&self, duration: std::time::Duration) -> BoxFuture<'static, ()> {
            crate::TokioRuntime.sleep(duration)
        }
        fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
            crate::TokioRuntime.spawn_blocking(f)
        }
        fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
            None
        }
    }
    struct Lifecycle;
    impl crate::client::ClientLifecycle for Lifecycle {}
    let runtime = Arc::new(DropNextSpawn(AtomicBool::new(false)));
    let pm = Arc::new(
        crate::store::persistence_manager::PersistenceManager::new(
            crate::test_utils::create_test_backend().await,
        )
        .await
        .unwrap(),
    );
    let client = crate::Client::builder()
        .with_runtime_arc(runtime.clone())
        .with_persistence_manager(pm)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client(crate::test_utils::MockHttpClient)
        .with_lifecycle(Lifecycle)
        .build()
        .await
        .unwrap()
        .into_client();
    set_identity(&client, 1).await;
    let capture = Capture::default();
    runtime.0.store(true, Ordering::SeqCst);
    let outcome = client
        .cleanup_connection_state()
        .with_subscriber(capture.dispatch(tracing::Level::INFO))
        .await;
    assert!(matches!(outcome, crate::client::DrainOutcome::Unobserved));
    let records = capture.records();
    let event = records
        .iter()
        .find(|r| r.level == tracing::Level::ERROR)
        .expect("cleanup failure event");
    assert_eq!(event.fields.get("lid"), client.identity_tags().lid.as_ref());
    assert_eq!(event.fields.get("pn"), client.identity_tags().pn.as_ref());
    assert!(event.spans.is_empty());
}
