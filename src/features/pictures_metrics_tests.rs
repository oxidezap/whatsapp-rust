use super::*;
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use wacore::iq::spec::IqStreamSpec;

fn counts(handle: &PrometheusHandle) -> [u64; 3] {
    ["ok", "error", "timeout"]
        .map(|label| metric(handle, &format!("wa_iq_total{{result=\"{label}\"}}")))
}

fn metric(handle: &PrometheusHandle, name: &str) -> u64 {
    handle
        .render()
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{name} ")))
        .map(|value| value.parse().unwrap())
        .unwrap_or(0)
}

fn assert_delta(handle: &PrometheusHandle, before: [u64; 3], delta: [u64; 3]) {
    let expected = std::array::from_fn(|index| before[index] + delta[index]);
    assert_eq!(counts(handle), expected, "one final outcome per exchange");
}

#[derive(Clone, Copy)]
enum ParseKind {
    Success,
    ContextualFailure,
    CoreTimeout,
    DomainFailure,
}

#[derive(Debug, thiserror::Error)]
#[error("synthetic GraphQL domain failure {code}")]
struct DomainFailure {
    code: u16,
}

struct CheckedParseSpec {
    handle: PrometheusHandle,
    before: [u64; 3],
    duration_before: u64,
    kind: ParseKind,
    direct: bool,
    marker: Arc<()>,
}

impl IqSpec for CheckedParseSpec {
    type Response = ();

    fn build_iq(&self) -> wacore::request::InfoQuery<'static> {
        ProfilePictureSpec::preview(&Jid::pn("15550000001")).build_iq()
    }

    fn encode_iq_direct(&self, id: &str, out: &mut Vec<u8>) -> Result<bool, anyhow::Error> {
        if !self.direct {
            return Ok(false);
        }
        let node = wacore::request::RequestUtils::new("metrics-fixture".to_owned())
            .build_iq_node(self.build_iq(), Some(id.to_owned()));
        wacore_binary::marshal_to_vec(&node, out)?;
        Ok(true)
    }

    fn parse_response(&self, _: &wacore_binary::NodeRef<'_>) -> Result<(), anyhow::Error> {
        // Detect an early success even if another error increment later masks it.
        assert_eq!(counts(&self.handle), self.before);
        // The existing timer measures send/wait, not the spec parser.
        assert_eq!(
            metric(&self.handle, "wa_iq_duration_seconds_count"),
            self.duration_before + 1
        );
        match self.kind {
            ParseKind::Success => Ok(()),
            ParseKind::ContextualFailure => {
                Err(anyhow::Error::new(FixtureParseError(self.marker.clone()))
                    .context("ordinary parser context"))
            }
            ParseKind::CoreTimeout => Err(wacore::request::IqError::Timeout.into()),
            ParseKind::DomainFailure => {
                Err(anyhow::Error::new(DomainFailure { code: 429 })
                    .context("GraphQL domain context"))
            }
        }
    }
}

struct StreamSpec;

impl IqSpec for StreamSpec {
    type Response = ();

    fn build_iq(&self) -> wacore::request::InfoQuery<'static> {
        ProfilePictureSpec::preview(&Jid::pn("15550000001")).build_iq()
    }

    fn parse_response(&self, _: &wacore_binary::NodeRef<'_>) -> Result<(), anyhow::Error> {
        panic!("streaming must not call the tree parser")
    }
}

impl IqStreamSpec for StreamSpec {
    fn consume_response(&self, _: &mut wacore_binary::NodeStream<'_>) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

// The exporter is already a dev dependency. Isolate its global recorder so this
// also works with parallel cargo-test lib cases, not only nextest's isolation.
#[test]
fn picture_iq_outcomes_counter_regression() {
    const CHILD_ENV: &str = "WHATSAPP_PICTURE_IQ_METRICS_CHILD";
    if std::env::var_os(CHILD_ENV).is_none() {
        let module = module_path!().split_once("::").unwrap().1;
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("{module}::picture_iq_outcomes_counter_regression"),
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CHILD_ENV, "1")
            .status()
            .unwrap();
        assert!(status.success(), "isolated metrics fixture must pass");
        return;
    }
    let handle = PrometheusBuilder::new().install_recorder().unwrap();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(check_outcomes(handle));
}

async fn check_outcomes(handle: PrometheusHandle) {
    for nested in [false, true] {
        let before = counts(&handle);
        let (result, original) = canonical_roundtrip(refusal(429, nested, Some(17)), None).await;
        let ContactError::Iq(IqError::ServerError {
            response, backoff, ..
        }) = result.unwrap_err()
        else {
            panic!("preserved IQ rejection")
        };
        assert_eq!(backoff, Some(17));
        assert!(Arc::ptr_eq(response.as_arc(), &original));
        assert_delta(&handle, before, [0, 1, 0]);
    }
    let before = counts(&handle);
    assert!(
        canonical_roundtrip(found_response(), None)
            .await
            .0
            .unwrap()
            .into_found()
            .is_some()
    );
    assert_delta(&handle, before, [1, 0, 0]);

    for direct in [false, true] {
        for kind in [
            ParseKind::Success,
            ParseKind::ContextualFailure,
            ParseKind::CoreTimeout,
            ParseKind::DomainFailure,
        ] {
            let (client, transport) = create_iq_test_client().await;
            let before = counts(&handle);
            let marker = Arc::new(());
            let spec = CheckedParseSpec {
                handle: handle.clone(),
                before,
                duration_before: metric(&handle, "wa_iq_duration_seconds_count"),
                kind,
                direct,
                marker: marker.clone(),
            };
            let c = client.clone();
            let task = tokio::spawn(async move { c.execute(spec).await });
            let sent = decode_sent_iq(&transport, 0).await;
            // A business-domain code inside a result is not an IQ rejection.
            let payload = NodeBuilder::new("iq")
                .attr("type", "result")
                .children([NodeBuilder::new("data")
                    .bytes(b"{\"errors\":[{\"code\":429}]}".to_vec())
                    .build()])
                .build();
            answer_iq(
                &client,
                &sent.get().get_attr("id").unwrap().to_string(),
                &payload,
            )
            .await;
            let result = task.await.unwrap();
            if matches!(kind, ParseKind::Success) {
                result.unwrap();
                assert_delta(&handle, before, [1, 0, 0]);
            } else {
                let IqError::ParseError(error) = result.unwrap_err() else {
                    panic!("ordinary parse error")
                };
                match kind {
                    ParseKind::ContextualFailure => {
                        assert_eq!(error.to_string(), "ordinary parser context");
                        assert!(Arc::ptr_eq(
                            &error.downcast_ref::<FixtureParseError>().unwrap().0,
                            &marker
                        ));
                    }
                    ParseKind::CoreTimeout => assert!(matches!(
                        error.downcast_ref::<wacore::request::IqError>(),
                        Some(wacore::request::IqError::Timeout)
                    )),
                    ParseKind::DomainFailure => {
                        assert_eq!(error.downcast_ref::<DomainFailure>().unwrap().code, 429)
                    }
                    ParseKind::Success => unreachable!(),
                }
                assert_delta(&handle, before, [0, 1, 0]);
            }
            assert!(client.response_waiters_guard().is_empty());
        }
    }

    // Raw IQ APIs still judge the envelope, not arbitrary embedded payloads.
    for node_api in [false, true] {
        for nested in [false, true] {
            let (client, transport) = create_iq_test_client().await;
            let before = counts(&handle);
            let c = client.clone();
            let task = tokio::spawn(async move {
                if node_api {
                    c.send_iq_node(
                        NodeBuilder::new("iq")
                            .attr("id", "explicit-metrics-id")
                            .attr("type", "get")
                            .attr("xmlns", "w:test")
                            .build(),
                        None,
                    )
                    .await
                } else {
                    let mut query = ProfilePictureSpec::preview(&Jid::pn("15550000001")).build_iq();
                    query.id = Some("explicit-metrics-id".to_owned());
                    c.send_iq(query).await
                }
            });
            let sent = decode_sent_iq(&transport, 0).await;
            let id = sent.get().get_attr("id").unwrap().to_string();
            assert_eq!(id, "explicit-metrics-id");
            let original = answer_iq(&client, &id, &refusal(429, nested, None)).await;
            let result = task.await.unwrap();
            if nested {
                assert!(Arc::ptr_eq(&result.unwrap(), &original));
                assert_delta(&handle, before, [1, 0, 0]);
            } else {
                assert!(matches!(
                    result,
                    Err(IqError::ServerError { code: 429, .. })
                ));
                assert_delta(&handle, before, [0, 1, 0]);
            }
        }
    }
    // The direct spec now has the same typed embedded rejection policy as the facade.
    let (client, transport) = create_iq_test_client().await;
    let before = counts(&handle);
    let c = client.clone();
    let task = tokio::spawn(async move {
        c.execute(ProfilePictureSpec::preview(&Jid::pn("15550000001")))
            .await
    });
    let sent = decode_sent_iq(&transport, 0).await;
    answer_iq(
        &client,
        &sent.get().get_attr("id").unwrap().to_string(),
        &refusal(429, true, None),
    )
    .await;
    assert!(matches!(
        task.await.unwrap(),
        Err(IqError::ServerError { code: 429, .. })
    ));
    assert_delta(&handle, before, [0, 1, 0]);

    // Streaming remains independently counted once after its own consumption.
    for error in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let before = counts(&handle);
        let c = client.clone();
        let task = tokio::spawn(async move { c.execute_streaming(StreamSpec).await });
        let sent = decode_sent_iq(&transport, 0).await;
        let id = sent.get().get_attr("id").unwrap().to_string();
        let response = if error {
            refusal(429, false, None)
        } else {
            found_response()
        };
        let original = crate::test_utils::node_to_owned_ref(&response);
        let crate::client::ResponseWaiter::Stream(sink) =
            client.response_waiters_guard().remove(&id).unwrap()
        else {
            panic!("stream waiter")
        };
        sink(crate::client::StreamedResponse::Node(&original));
        let result = task.await.unwrap();
        assert_eq!(result.is_err(), error);
        assert_delta(&handle, before, if error { [0, 1, 0] } else { [1, 0, 0] });
    }

    for cancel in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let before = counts(&handle);
        let c = client.clone();
        let task = tokio::spawn(async move {
            c.pictures()
                .lookup(
                    ProfilePictureRequest::new(
                        ProfilePictureTarget::Group(&Jid::group("15550000001-7")),
                        ProfilePictureType::Full,
                    )
                    .timeout(Some(Duration::from_millis(if cancel {
                        5000
                    } else {
                        100
                    }))),
                )
                .await
        });
        decode_sent_iq(&transport, 0).await;
        if cancel {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            assert_delta(&handle, before, [0, 0, 0]);
        } else {
            assert!(matches!(
                task.await.unwrap(),
                Err(ContactError::Iq(IqError::Timeout))
            ));
            assert_delta(&handle, before, [0, 0, 1]);
        }
        assert!(client.response_waiters_guard().is_empty());
    }
}
