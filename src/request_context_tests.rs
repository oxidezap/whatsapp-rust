use super::*;
use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
use wacore::iq::spec::IqSpec;
use wacore_binary::{Jid, NodeRef, builder::NodeBuilder};

struct ContextSpec {
    direct: bool,
    explicit: bool,
    builds: Arc<std::sync::atomic::AtomicUsize>,
}

impl IqSpec for ContextSpec {
    type Response = (Node, bool);

    fn build_iq(&self) -> InfoQuery<'static> {
        self.builds.fetch_add(1, Ordering::Relaxed);
        let mut query = InfoQuery::get_ref(
            "w:test",
            &Jid::pn("15550000001"),
            Some(wacore_binary::NodeContent::Nodes(vec![
                NodeBuilder::new("query")
                    .attr("reference", "request-only")
                    .build(),
            ])),
        )
        .with_target_ref(&Jid::lid("10000000001"));
        if self.explicit {
            query.id = Some("explicit-context-id".into());
        }
        query
    }

    fn encode_iq_direct(&self, id: &str, out: &mut Vec<u8>) -> Result<bool, anyhow::Error> {
        if !self.direct {
            return Ok(false);
        }
        let node = RequestUtils::new(String::new()).build_iq_node(self.build_iq(), Some(id.into()));
        wacore_binary::marshal_to_vec(&node, out)?;
        Ok(true)
    }

    fn parse_response(&self, _: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        panic!("contextual spec must not use the legacy parser")
    }
    fn uses_contextual_response(&self) -> bool {
        true
    }
    fn handles_iq_errors(&self) -> bool {
        true
    }
    fn parse_response_with_context(
        &self,
        response: &NodeRef<'_>,
        request: &NodeRef<'_>,
    ) -> Result<Self::Response, anyhow::Error> {
        assert_eq!(
            request
                .get_optional_child("query")
                .unwrap()
                .get_attr("reference")
                .unwrap()
                .as_str(),
            "request-only"
        );
        assert!(response.get_optional_child("query").is_none());
        Ok((
            request.to_owned(),
            response.get_attr("type").unwrap().as_str() == "error",
        ))
    }
}

#[tokio::test]
async fn execute_passes_exact_sent_id_destination_target_and_request_references() {
    for direct in [false, true] {
        for explicit in [false, true] {
            for error in [false, true] {
                let (client, transport) = create_iq_test_client().await;
                let builds = Arc::new(std::sync::atomic::AtomicUsize::new(0));
                let spec = ContextSpec {
                    direct,
                    explicit,
                    builds: builds.clone(),
                };
                let c = client.clone();
                let task = tokio::spawn(async move { c.execute(spec).await });
                let sent = decode_sent_iq(&transport, 0).await;
                let id = sent.get().get_attr("id").unwrap().to_string();
                if explicit && !direct {
                    assert_eq!(id, "explicit-context-id");
                }
                let response = NodeBuilder::new("iq")
                    .attr("type", if error { "error" } else { "result" })
                    .build();
                answer_iq(&client, &id, &response).await;
                let (context, rejected) = task.await.unwrap().unwrap();
                assert_eq!(
                    wacore_binary::marshal::marshal(&context).unwrap(),
                    wacore_binary::marshal::marshal(&sent.to_owned_node()).unwrap()
                );
                assert_eq!(rejected, error);
                assert_eq!(
                    builds.load(Ordering::Relaxed),
                    1,
                    "capture must not rebuild a stateful spec"
                );
                assert!(client.response_waiters_guard().is_empty());
            }
        }
    }
}

#[tokio::test]
async fn generated_group_adapter_checks_sender_and_retains_public_error() {
    use wacore::iq::groups::{GroupSubject, SetGroupSubjectIq};
    for (from, error, wrong_id) in [
        ("123@g.us", false, false),
        ("456@g.us", false, false),
        ("123@g.us", true, false),
        ("456@g.us", true, false),
        ("123@g.us", false, true),
        ("123@g.us", true, true),
    ] {
        let (client, transport) = create_iq_test_client().await;
        let c = client.clone();
        let task = tokio::spawn(async move {
            c.execute(SetGroupSubjectIq::new(
                &Jid::group("123"),
                GroupSubject::new("synthetic").unwrap(),
            ))
            .await
        });
        let sent = decode_sent_iq(&transport, 0).await;
        let id = sent.get().get_attr("id").unwrap().to_string();
        let mut response = NodeBuilder::new("iq")
            .attr("id", if wrong_id { "not-the-sent-id" } else { &id })
            .attr("from", from)
            .attr("type", if error { "error" } else { "result" });
        if error {
            response = response.children([NodeBuilder::new("error")
                .attr("code", "403")
                .attr("text", "forbidden")
                .attr("type", "cancel")
                .build()]);
        }
        answer_iq(&client, &id, &response.build()).await;
        let result = task.await.unwrap();
        if from != "123@g.us" || wrong_id {
            assert!(matches!(result, Err(IqError::ParseError(_))));
        } else if error {
            assert!(matches!(
                result,
                Err(IqError::ServerError { code: 403, .. })
            ));
        } else {
            result.unwrap();
        }
    }
}
