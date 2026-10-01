//! Isolated high-level mutation roundtrips; no server or real account is used.
use super::{GroupError, NewsletterError, ProfileError};
use crate::ErrorChainExt;
use crate::client::Client;
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::request::IqError;
use crate::runtime_impl::TokioRuntime;
use crate::store::persistence_manager::PersistenceManager;
use crate::test_utils::{answer_iq, create_test_backend, decode_sent_iq};
use crate::transport::mock::{CapturingMockTransport, CapturingMockTransportFactory};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use wacore::handshake::NoiseCipher;
use wacore_binary::{Jid, Node, builder::NodeBuilder};

#[derive(Clone, Copy, Debug)]
enum Route {
    Own,
    Group,
    Channel,
}

#[derive(Debug, Default)]
struct CountingHttp(AtomicUsize);

#[async_trait::async_trait]
impl HttpClient for CountingHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse, anyhow::Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        anyhow::bail!("picture mutations must not add an HTTP upload")
    }
}

async fn fixture() -> (Arc<Client>, Arc<CapturingMockTransport>, Arc<CountingHttp>) {
    let http = Arc::new(CountingHttp::default());
    let pm = Arc::new(
        PersistenceManager::new(create_test_backend().await)
            .await
            .unwrap(),
    );
    let factory = CapturingMockTransportFactory::new();
    let transport = factory.transport();
    let (client, _) = Client::builder()
        .with_runtime(TokioRuntime)
        .with_persistence_manager(pm)
        .with_transport_factory(factory)
        .with_http_client_arc(http.clone())
        .build()
        .await
        .unwrap()
        .into_parts();
    let socket = crate::socket::NoiseSocket::with_observers(
        Arc::new(TokioRuntime),
        transport.clone() as Arc<dyn crate::transport::Transport>,
        NoiseCipher::new(&[0; 32]).unwrap(),
        NoiseCipher::new(&[0; 32]).unwrap(),
        crate::socket::noise_socket::SendObservers::with_stats(client.stats.clone())
            .with_sent_frames(client.sent_frame_tap.clone()),
    );
    *client.noise_socket.lock().unwrap() = Some(Arc::new(socket));
    client.set_connected_for_test(true);
    client.is_running.store(true, Ordering::Release);
    client.enter_live_mode_for_tests();
    (client, transport, http)
}

async fn mutate(client: &Client, route: Route, data: Option<Vec<u8>>) -> anyhow::Result<String> {
    let group = Jid::group("15550000001-7");
    let channel: Jid = "120363000000000001@newsletter".parse().unwrap();
    match (route, data) {
        (Route::Own, Some(data)) => Ok(client.profile().set_profile_picture(data).await?.id),
        (Route::Own, None) => Ok(client.profile().remove_profile_picture().await?.id),
        (Route::Group, Some(data)) => {
            Ok(client.groups().set_profile_picture(group, data).await?.id)
        }
        (Route::Group, None) => Ok(client.groups().remove_profile_picture(group).await?.id),
        (Route::Channel, data) => {
            let metadata = match data {
                Some(data) => client.newsletter().set_picture(&channel, &data).await?,
                None => client.newsletter().remove_picture(&channel).await?,
            };
            assert_eq!(metadata.jid, channel);
            assert_eq!(metadata.picture_url.as_deref(), Some("/synthetic-picture"));
            Ok(metadata.name)
        }
    }
}

fn success(route: Route, remove: bool) -> Node {
    let children = match route {
        Route::Channel => {
            vec![NodeBuilder::new("result").bytes(serde_json::to_vec(&serde_json::json!({
            "data": {"xwa2_newsletter_update": {
                "id": "120363000000000001@newsletter",
                "state": {"type": "ACTIVE"},
                "thread_metadata": {
                    "name": {"text": "Synthetic channel"},
                    "picture": {"direct_path": "/synthetic-picture", "id": "photo-7"}
                }
            }}
        })).unwrap()).build()]
        }
        _ if remove => vec![],
        _ => vec![NodeBuilder::new("picture").attr("id", "photo-7").build()],
    };
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children(children)
        .build()
}

#[tokio::test]
async fn picture_setters_reject_empty_before_any_send_or_upload() {
    for route in [Route::Own, Route::Group, Route::Channel] {
        let (client, transport, http) = fixture().await;
        let error = mutate(&client, route, Some(Vec::new())).await.unwrap_err();
        match route {
            Route::Own => assert!(matches!(
                error.downcast_ref::<ProfileError>(),
                Some(ProfileError::EmptyPicture)
            )),
            Route::Group => assert!(matches!(
                error.downcast_ref::<GroupError>(),
                Some(GroupError::EmptyPicture)
            )),
            Route::Channel => assert!(matches!(
                error.downcast_ref::<NewsletterError>(),
                Some(NewsletterError::EmptyPicture)
            )),
        }
        assert!(
            transport.sent().is_empty(),
            "{route:?} must not remove accidentally"
        );
        assert_eq!(http.0.load(Ordering::Relaxed), 0);
    }
}

#[tokio::test]
async fn picture_set_and_explicit_remove_keep_each_route_and_result() {
    // Deliberately not a JPEG: the API validates non-emptiness, not image format.
    let bytes = vec![1, 2, 3];
    for route in [Route::Own, Route::Group, Route::Channel] {
        for remove in [false, true] {
            let (client, transport, http) = fixture().await;
            let c = client.clone();
            let data = (!remove).then(|| bytes.clone());
            let task = tokio::spawn(async move { mutate(&c, route, data).await });
            let sent = decode_sent_iq(&transport, 0).await;
            let node = sent.get();
            assert_eq!(node.get_attr("to").unwrap(), "s.whatsapp.net");
            match route {
                Route::Own | Route::Group => {
                    assert_eq!(node.get_attr("xmlns").unwrap(), "w:profile:picture");
                    assert_eq!(node.get_attr("type").unwrap(), "set");
                    assert_eq!(
                        node.get_attr("target").map(|a| a.to_string()),
                        matches!(route, Route::Group).then(|| "15550000001-7@g.us".to_string())
                    );
                    if remove {
                        assert!(node.get_optional_child("picture").is_none());
                    } else {
                        let picture = node.get_optional_child("picture").unwrap();
                        assert_eq!(picture.get_attr("type").unwrap(), "image");
                        assert_eq!(picture.content_bytes().unwrap(), bytes);
                    }
                }
                Route::Channel => {
                    assert_eq!(node.get_attr("xmlns").unwrap(), "w:mex");
                    assert_eq!(node.get_attr("type").unwrap(), "get");
                    let query = node.get_optional_child("query").unwrap();
                    assert_eq!(
                        query.get_attr("query_id").unwrap(),
                        wacore::iq::mex_operations::update_newsletter::DOC_ID
                    );
                    let payload: serde_json::Value =
                        serde_json::from_slice(query.content_bytes().unwrap()).unwrap();
                    assert_eq!(
                        payload["variables"]["newsletter_id"],
                        "120363000000000001@newsletter"
                    );
                    assert_eq!(
                        payload["variables"]["updates"],
                        serde_json::json!({"picture": if remove { "" } else { "AQID" }})
                    );
                }
            }
            let id = node.get_attr("id").unwrap().to_string();
            answer_iq(&client, &id, &success(route, remove)).await;
            let result = task.await.unwrap().unwrap();
            assert_eq!(
                result,
                match route {
                    Route::Channel => "Synthetic channel",
                    _ if remove => "",
                    _ => "photo-7",
                }
            );
            assert_eq!(transport.sent().len(), 1);
            assert_eq!(http.0.load(Ordering::Relaxed), 0);
        }
    }
}

#[tokio::test]
async fn picture_mutation_rejections_preserve_source_metadata_and_response_arc() {
    for route in [Route::Own, Route::Group, Route::Channel] {
        for remove in [false, true] {
            let (client, transport, _) = fixture().await;
            let c = client.clone();
            let task =
                tokio::spawn(async move { mutate(&c, route, (!remove).then(|| vec![1])).await });
            let sent = decode_sent_iq(&transport, 0).await;
            let response = NodeBuilder::new("iq")
                .attr("type", "error")
                .children([NodeBuilder::new("error")
                    .attr("code", "429")
                    .attr("text", "synthetic-throttle")
                    .attr("type", "wait")
                    .attr("backoff", "73")
                    .attr("extra", "preserved")
                    .build()])
                .build();
            let original = answer_iq(
                &client,
                &sent.get().get_attr("id").unwrap().to_string(),
                &response,
            )
            .await;
            let error = task.await.unwrap().unwrap_err();
            let rejection = error.server_rejection().unwrap();
            assert_eq!(rejection.code, 429);
            assert_eq!(rejection.backoff, Some(73));
            assert_eq!(rejection.error_type, Some("wait"));
            assert_eq!(rejection.text, "synthetic-throttle");
            let iq = error
                .chain()
                .find_map(|e| e.downcast_ref::<IqError>())
                .unwrap();
            let IqError::ServerError { response, .. } = iq else {
                panic!("typed rejection lost")
            };
            assert!(Arc::ptr_eq(response.as_arc(), &original));
            assert_eq!(transport.sent().len(), 1);
        }
    }
}
