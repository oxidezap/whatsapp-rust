use super::*;
use crate::ErrorChainExt;
use crate::features::ContactError;
use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
use std::sync::Arc;
use wacore::iq::spec::IqSpec;
use wacore_binary::{Node, NodeContent, OwnedNodeRef, builder::NodeBuilder};

fn found_response() -> Node {
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("picture")
            .attr("id", "photo-7")
            .attr("url", "https://example.test/avatar.jpg")
            .attr("direct_path", "/avatar.jpg")
            .attr("hash", "synthetic-hash")
            .build()])
        .build()
}

fn refusal(code: u16, nested: bool, backoff: Option<u32>) -> Node {
    let mut error = NodeBuilder::new("error")
        .attr("code", code.to_string())
        .attr("text", "synthetic-refusal")
        .attr("type", "wait")
        .attr("original_metadata", "keep-me");
    if let Some(backoff) = backoff {
        error = error.attr("backoff", backoff.to_string());
    }
    let child = if nested {
        NodeBuilder::new("picture")
            .children([error.build()])
            .build()
    } else {
        error.build()
    };
    NodeBuilder::new("iq")
        .attr("type", if nested { "result" } else { "error" })
        .children([child])
        .build()
}

async fn canonical_roundtrip(
    response: Node,
    existing_id: Option<&'static str>,
) -> (
    Result<ProfilePictureLookup, ContactError>,
    Arc<OwnedNodeRef>,
) {
    canonical_roundtrip_route(
        response,
        existing_id,
        Route::Contact,
        ProfilePictureType::Preview,
    )
    .await
}

#[derive(Debug, Clone, Copy)]
enum Route {
    Contact,
    Group,
    Community,
}

async fn canonical_roundtrip_route(
    response: Node,
    existing_id: Option<&'static str>,
    route: Route,
    size: ProfilePictureType,
) -> (
    Result<ProfilePictureLookup, ContactError>,
    Arc<OwnedNodeRef>,
) {
    let (client, transport) = create_iq_test_client().await;
    let task_client = client.clone();
    let task = tokio::spawn(async move {
        let jid = match route {
            Route::Contact => Jid::pn("15550000001"),
            Route::Group | Route::Community => Jid::group("15550000001-7"),
        };
        let target = match route {
            Route::Contact => ProfilePictureTarget::Contact(&jid),
            Route::Group => ProfilePictureTarget::Group(&jid),
            Route::Community => ProfilePictureTarget::Community(&jid),
        };
        task_client
            .pictures()
            .lookup(ProfilePictureRequest::new(target, size).existing_id(existing_id))
            .await
    });
    let sent = decode_sent_iq(&transport, 0).await;
    let community = matches!(route, Route::Community);
    assert_eq!(
        sent.get().get_attr("xmlns").unwrap().as_str(),
        if community {
            "w:g2"
        } else {
            "w:profile:picture"
        },
        "route={route:?}, size={size:?}, existing_id={existing_id:?}",
    );
    let picture = if community {
        assert_eq!(
            sent.get().get_attr("to").unwrap().as_str(),
            "15550000001-7@g.us",
            "route={route:?}, size={size:?}, existing_id={existing_id:?}",
        );
        assert!(
            sent.get().get_attr("target").is_none(),
            "route={route:?}, size={size:?}, existing_id={existing_id:?}"
        );
        sent.get()
            .get_optional_child("pictures")
            .unwrap()
            .get_optional_child("picture")
            .unwrap()
    } else {
        assert_eq!(
            sent.get().get_attr("to").unwrap().as_str(),
            "s.whatsapp.net",
            "route={route:?}, size={size:?}, existing_id={existing_id:?}",
        );
        assert_eq!(
            sent.get().get_attr("target").unwrap().as_str(),
            if matches!(route, Route::Contact) {
                "15550000001@s.whatsapp.net"
            } else {
                "15550000001-7@g.us"
            },
            "route={route:?}, size={size:?}, existing_id={existing_id:?}",
        );
        sent.get().get_optional_child("picture").unwrap()
    };
    assert_eq!(
        picture.get_attr("type").unwrap().as_str(),
        size.as_str(),
        "route={route:?}, size={size:?}, existing_id={existing_id:?}"
    );
    assert_eq!(
        picture.get_attr("query").unwrap().as_str(),
        "url",
        "route={route:?}, size={size:?}, existing_id={existing_id:?}"
    );
    assert_eq!(
        picture.get_attr("id").map(|s| s.as_str().into_owned()),
        existing_id.map(str::to_owned),
        "route={route:?}, size={size:?}, existing_id={existing_id:?}",
    );
    let id = sent.get().get_attr("id").unwrap().to_string();
    let response = if community && let Some(picture) = response.get_optional_child("picture") {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("pictures")
                .children([picture.clone()])
                .build()])
            .build()
    } else {
        response
    };
    let original = answer_iq(&client, &id, &response).await;
    let result = task.await.unwrap();
    assert_eq!(
        transport.sent().len(),
        1,
        "lookup must not make a hidden fallback IQ: route={route:?}, size={size:?}, existing_id={existing_id:?}"
    );
    (result, original)
}

#[tokio::test]
async fn picture_lookup_found_unchanged_absent_and_unauthorized_roundtrip() {
    let (found, _) = canonical_roundtrip(found_response(), None).await;
    let picture = found.unwrap().into_found().unwrap();
    assert_eq!(picture.id, "photo-7");
    assert_eq!(picture.direct_path.as_deref(), Some("/avatar.jpg"));
    assert_eq!(picture.hash.as_deref(), Some("synthetic-hash"));
    let empty = NodeBuilder::new("iq").attr("type", "result").build();
    assert_eq!(
        canonical_roundtrip(empty.clone(), Some("photo-7"))
            .await
            .0
            .unwrap(),
        ProfilePictureLookup::Unchanged
    );
    assert_eq!(
        canonical_roundtrip(empty, None).await.0.unwrap(),
        ProfilePictureLookup::NotFound
    );
    for nested in [false, true] {
        for code in [401, 403, 404] {
            let result = canonical_roundtrip(refusal(code, nested, None), None)
                .await
                .0
                .unwrap();
            assert_eq!(
                result,
                if code == 404 {
                    ProfilePictureLookup::NotFound
                } else {
                    ProfilePictureLookup::NotAuthorized
                }
            );
        }
    }
}

#[tokio::test]
async fn picture_lookup_rate_limit_keeps_source_original_stanza_and_optional_backoff() {
    for nested in [false, true] {
        for backoff in [None, Some(73)] {
            let (result, original) = canonical_roundtrip(refusal(429, nested, backoff), None).await;
            let error = result.unwrap_err();
            let rejection = error.server_rejection().unwrap();
            assert_eq!(rejection.code, 429);
            assert_eq!(rejection.text, "synthetic-refusal");
            assert_eq!(rejection.error_type, Some("wait"));
            assert_eq!(rejection.backoff, backoff);
            assert!(
                error
                    .sources()
                    .any(|e| e.downcast_ref::<IqError>().is_some())
            );
            let ContactError::Iq(IqError::ServerError { response, .. }) = error else {
                panic!("typed rejection lost")
            };
            assert!(Arc::ptr_eq(response.as_arc(), &original));
            assert_eq!(response.get(), original.get());
        }
    }
    assert!(matches!(
        canonical_roundtrip(refusal(500, false, None), None).await.0,
        Err(ContactError::Iq(IqError::ServerError { code: 500, .. }))
    ));
    assert!(matches!(
        canonical_roundtrip(refusal(500, true, None), None).await.0,
        Err(ContactError::Iq(IqError::ParseError(_)))
    ));
}

#[tokio::test]
async fn picture_lookup_direct_spec_preserves_nested_rejection() {
    let (client, transport) = create_iq_test_client().await;
    let c = client.clone();
    let task = tokio::spawn(async move {
        c.execute(ProfilePictureSpec::preview(&Jid::pn("15550000001")))
            .await
    });
    let sent = decode_sent_iq(&transport, 0).await;
    let original = answer_iq(
        &client,
        &sent.get().get_attr("id").unwrap().to_string(),
        &refusal(429, true, Some(12)),
    )
    .await;
    let error = task.await.unwrap().unwrap_err();
    assert_eq!(error.server_rejection().unwrap().backoff, Some(12));
    let IqError::ServerError { response, .. } = error else {
        panic!("typed rejection lost")
    };
    assert!(Arc::ptr_eq(response.as_arc(), &original));
}

#[tokio::test]
async fn picture_lookup_all_routes_and_sizes_preserve_outcomes_and_rejections() {
    for route in [Route::Contact, Route::Group, Route::Community] {
        for size in [ProfilePictureType::Preview, ProfilePictureType::Full] {
            let (found, _) = canonical_roundtrip_route(found_response(), None, route, size).await;
            assert_eq!(
                found.unwrap().found().unwrap().id,
                "photo-7",
                "route={route:?}, size={size:?}"
            );
            for existing_id in [None, Some("photo-7")] {
                for picture in [
                    None,
                    Some(NodeBuilder::new("picture").build()),
                    Some(NodeBuilder::new("picture").attr("id", "photo-7").build()),
                ] {
                    let response = NodeBuilder::new("iq")
                        .attr("type", "result")
                        .children(picture)
                        .build();
                    let (outcome, _) =
                        canonical_roundtrip_route(response, existing_id, route, size).await;
                    let outcome = outcome.unwrap_or_else(|e| {
                        panic!("route={route:?}, size={size:?}, existing_id={existing_id:?}: {e:?}")
                    });
                    assert_eq!(
                        outcome,
                        if existing_id.is_some() {
                            ProfilePictureLookup::Unchanged
                        } else {
                            ProfilePictureLookup::NotFound
                        },
                        "route={route:?}, size={size:?}, existing_id={existing_id:?}",
                    );
                    assert!(
                        outcome.into_found().is_none(),
                        "no invented URL or local bytes: route={route:?}, size={size:?}, existing_id={existing_id:?}"
                    );
                }
            }
            for (status, expected) in [
                ("304", ProfilePictureLookup::Unchanged),
                ("204", ProfilePictureLookup::NotFound),
            ] {
                let response = NodeBuilder::new("iq")
                    .attr("type", "result")
                    .children([NodeBuilder::new("picture").attr("status", status).build()])
                    .build();
                assert_eq!(
                    canonical_roundtrip_route(response, None, route, size)
                        .await
                        .0
                        .unwrap(),
                    expected,
                    "route={route:?}, size={size:?}, status={status}",
                );
            }
            for nested in [false, true] {
                for code in [401, 403, 404] {
                    assert_eq!(
                        canonical_roundtrip_route(refusal(code, nested, None), None, route, size)
                            .await
                            .0
                            .unwrap(),
                        if code == 404 {
                            ProfilePictureLookup::NotFound
                        } else {
                            ProfilePictureLookup::NotAuthorized
                        },
                        "route={route:?}, size={size:?}, nested={nested}, code={code}",
                    );
                }
                for backoff in [None, Some(73)] {
                    let (result, original) =
                        canonical_roundtrip_route(refusal(429, nested, backoff), None, route, size)
                            .await;
                    let context = format!(
                        "route={route:?}, size={size:?}, nested={nested}, code=429, backoff={backoff:?}"
                    );
                    let error = result.expect_err(&context);
                    let rejection = error.server_rejection().expect(&context);
                    assert_eq!(rejection.code, 429, "{context}");
                    assert_eq!(rejection.text, "synthetic-refusal", "{context}");
                    assert_eq!(rejection.error_type, Some("wait"), "{context}");
                    assert_eq!(rejection.backoff, backoff, "{context}");
                    let ContactError::Iq(IqError::ServerError { response, .. }) = error else {
                        panic!("typed rejection lost: {context}")
                    };
                    assert!(Arc::ptr_eq(response.as_arc(), &original), "{context}");
                }
            }
        }
    }
}

#[tokio::test]
async fn picture_group_batch_keeps_one_iq_per_entry_outcomes_and_limit() {
    use crate::features::{GroupError, GroupProfilePictureOutcome, PictureType};
    let (client, transport) = create_iq_test_client().await;
    let c = client.clone();
    let task = tokio::spawn(async move {
        c.groups()
            .get_profile_pictures(
                (1..=4)
                    .map(|n| Jid::group(format!("15550000001-{n}")))
                    .collect(),
                PictureType::Image,
            )
            .await
    });
    let sent = decode_sent_iq(&transport, 0).await;
    assert_eq!(sent.get().get_attr("xmlns").unwrap().as_str(), "w:g2");
    assert_eq!(sent.get().get_attr("to").unwrap().as_str(), "g.us");
    let pictures = sent.get().get_optional_child("pictures").unwrap();
    let entries: Vec<_> = pictures.get_children_by_tag("picture").collect();
    assert_eq!(entries.len(), 4);
    for (n, entry) in entries.iter().enumerate() {
        assert_eq!(entry.get_attr("type").unwrap().as_str(), "image");
        assert_eq!(
            entry.get_attr("sub_group_jid").unwrap().as_str(),
            format!("15550000001-{}@g.us", n + 1)
        );
        assert!(entry.get_attr("parent_group_jid").is_none());
    }
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("pictures")
            .children([
                NodeBuilder::new("picture")
                    .attr("sub_group_jid", "15550000001-1@g.us")
                    .attr("id", "photo-1")
                    .attr("url", "https://example.test/batch.jpg")
                    .build(),
                NodeBuilder::new("picture")
                    .attr("sub_group_jid", "15550000001-2@g.us")
                    .attr("status", "304")
                    .build(),
                NodeBuilder::new("picture")
                    .attr("sub_group_jid", "15550000001-3@g.us")
                    .attr("status", "204")
                    .build(),
                NodeBuilder::new("picture")
                    .attr("sub_group_jid", "15550000001-4@g.us")
                    .attr("status", "500")
                    .build(),
            ])
            .build()])
        .build();
    answer_iq(
        &client,
        &sent.get().get_attr("id").unwrap().to_string(),
        &response,
    )
    .await;
    let results = task.await.unwrap().unwrap();
    assert_eq!(results.len(), 4);
    assert!(matches!(
        results[0].outcome(),
        GroupProfilePictureOutcome::Found { .. }
    ));
    assert_eq!(results[1].outcome(), GroupProfilePictureOutcome::Unchanged);
    assert_eq!(results[2].outcome(), GroupProfilePictureOutcome::NotFound);
    assert_eq!(
        results[3].outcome(),
        GroupProfilePictureOutcome::Error { code: 500 }
    );
    assert_eq!(transport.sent().len(), 1);
    let oversized =
        vec![Jid::group("15550000001-7"); wacore::iq::groups::BATCH_PROFILE_PICTURES_LIMIT + 1];
    assert!(matches!(
        client
            .groups()
            .get_profile_pictures(oversized, PictureType::Preview)
            .await,
        Err(GroupError::InvalidRequest(_))
    ));
    assert_eq!(
        transport.sent().len(),
        1,
        "limit must reject before sending"
    );
}

#[test]
fn picture_lookup_request_wire_size_route_timeout_and_advanced_options() {
    let jid = Jid::group("15550000001-7");
    let shared = Jid::group("15550000001-8");
    for size in [ProfilePictureType::Preview, ProfilePictureType::Full] {
        for target in [
            ProfilePictureTarget::Contact(&jid),
            ProfilePictureTarget::Group(&jid),
            ProfilePictureTarget::Community(&jid),
        ] {
            let request = ProfilePictureRequest::new(target, size)
                .existing_id(Some("photo-7"))
                .common_gid(Some(&shared))
                .invite(Some("invite-7"))
                .persona_id(Some("persona-7"))
                .timeout(Some(Duration::from_millis(1250)));
            let spec = request.spec(None);
            let query = spec.build_iq();
            let community = matches!(target, ProfilePictureTarget::Community(_));
            let old = if community {
                ProfilePictureSpec::community(&jid, size)
            } else {
                ProfilePictureSpec::new(&jid, size)
            };
            assert_eq!(query.namespace, old.build_iq().namespace);
            assert_eq!(query.to, old.build_iq().to);
            assert_eq!(query.target, old.build_iq().target);
            assert_eq!(query.timeout, Some(Duration::from_millis(1250)));
            let Some(NodeContent::Nodes(nodes)) = query.content else {
                panic!("picture content")
            };
            let picture = if community {
                nodes[0].get_optional_child("picture").unwrap()
            } else {
                &nodes[0]
            };
            assert_eq!(picture.attrs.get("type").unwrap().as_str(), size.as_str());
            assert_eq!(picture.attrs.get("id").unwrap().as_str(), "photo-7");
            assert_eq!(picture.attrs.get("query").unwrap().as_str(), "url");
            if community {
                assert_eq!(
                    picture.attrs.get("parent_group_jid").unwrap().as_str(),
                    jid.to_string()
                );
                assert!(!picture.attrs.contains_key("invite"));
                assert!(!picture.attrs.contains_key("persona_id"));
                assert!(!picture.attrs.contains_key("common_gid"));
            } else {
                assert_eq!(picture.attrs.get("invite").unwrap().as_str(), "invite-7");
                assert_eq!(
                    picture.attrs.get("persona_id").unwrap().as_str(),
                    "persona-7"
                );
                assert_eq!(
                    picture.attrs.get("common_gid").unwrap().as_str(),
                    shared.to_string()
                );
            }
        }
    }
    let request = ProfilePictureRequest::new(
        ProfilePictureTarget::Contact(&jid),
        ProfilePictureType::Preview,
    )
    .common_gid(Some(&shared));
    let spec = request.spec(Some(vec![1, 2, 3]));
    assert!(spec.common_gid.is_none());
    let Some(NodeContent::Nodes(nodes)) = spec.build_iq().content else {
        panic!("content")
    };
    let token = nodes[0].get_optional_child("tctoken").unwrap();
    assert_eq!(token.content, Some(NodeContent::Bytes(vec![1, 2, 3])));
}

#[test]
fn picture_lookup_special_jids_do_not_discover_privacy_tokens() {
    let pn = Jid::pn("15550000001");
    assert!(token_eligible(ProfilePictureTarget::Contact(&pn), false));
    assert!(!token_eligible(ProfilePictureTarget::Contact(&pn), true));
    for raw in [
        "15550000001-7@g.us",
        "15550000001@newsletter",
        "13135550001@s.whatsapp.net",
        "status@broadcast",
        "15550000001@broadcast",
    ] {
        let jid: Jid = raw.parse().unwrap();
        assert!(
            !token_eligible(ProfilePictureTarget::Contact(&jid), false),
            "{raw}"
        );
    }
    assert!(!token_eligible(ProfilePictureTarget::Group(&pn), false));
    assert!(!token_eligible(ProfilePictureTarget::Community(&pn), false));
}

#[tokio::test]
async fn picture_lookup_regular_jid_is_not_short_circuited_when_disconnected() {
    let client = crate::test_utils::create_test_client().await;
    let err = client
        .pictures()
        .lookup(ProfilePictureRequest::new(
            ProfilePictureTarget::Contact(&Jid::pn("15550000001")),
            ProfilePictureType::Full,
        ))
        .await
        .unwrap_err();
    assert!(matches!(err, ContactError::Iq(IqError::NotConnected)));
}

#[tokio::test]
async fn picture_lookup_psa_is_short_circuited_and_timeout_remains_an_error() {
    let (client, transport) = create_iq_test_client().await;
    let psa = Jid::pn("0");
    let request = ProfilePictureRequest::new(
        ProfilePictureTarget::Contact(&psa),
        ProfilePictureType::Full,
    )
    .timeout(Some(Duration::from_millis(1)));
    assert_eq!(
        client.pictures().lookup(request).await.unwrap(),
        ProfilePictureLookup::NotFound
    );
    assert!(transport.sent().is_empty());
    let jid = Jid::pn("15550000001");
    let error = client
        .pictures()
        .lookup(
            ProfilePictureRequest::new(
                ProfilePictureTarget::Contact(&jid),
                ProfilePictureType::Full,
            )
            .timeout(Some(Duration::from_millis(20))),
        )
        .await
        .unwrap_err();
    assert!(error.is_timeout());
    assert!(client.response_waiters_guard().is_empty());
    assert_eq!(transport.sent().len(), 1);
}

// Local integration scenario adapted from oxidezap/client avatar.rs at
// f57c5a1: this is consumer policy, not a new protocol requirement.
async fn consumer_lookup(
    client: &Client,
    jid: &Jid,
    need_bytes: bool,
) -> Result<ProfilePictureLookup, ContactError> {
    let known = if need_bytes {
        None
    } else {
        Some("known-photo")
    };
    let original = client
        .pictures()
        .lookup(
            ProfilePictureRequest::new(
                ProfilePictureTarget::Group(jid),
                ProfilePictureType::Preview,
            )
            .existing_id(known),
        )
        .await?;
    if !matches!(original, ProfilePictureLookup::NotAuthorized) {
        return Ok(original);
    }
    let fallback = client
        .pictures()
        .lookup(
            ProfilePictureRequest::new(
                ProfilePictureTarget::Community(jid),
                ProfilePictureType::Preview,
            )
            .existing_id(known),
        )
        .await;
    match fallback {
        Ok(found @ ProfilePictureLookup::Found(_)) => Ok(found),
        _ => Ok(original),
    }
}

#[tokio::test]
async fn picture_lookup_conditional_consumer_fallback_keeps_original_and_omits_id_for_missing_bytes()
 {
    for need_bytes in [true, false] {
        for fallback_response in [
            found_response(),
            refusal(404, false, None),
            refusal(429, false, Some(5)),
            NodeBuilder::new("iq").attr("type", "result").build(),
        ] {
            let fallback_found = fallback_response.get_optional_child("picture").is_some();
            let (client, transport) = create_iq_test_client().await;
            let c = client.clone();
            let task = tokio::spawn(async move {
                consumer_lookup(&c, &Jid::group("15550000001-7"), need_bytes).await
            });
            let sent = decode_sent_iq(&transport, 0).await;
            assert_eq!(
                sent.get().get_attr("xmlns").unwrap().as_str(),
                "w:profile:picture"
            );
            let picture = sent.get().get_optional_child("picture").unwrap();
            assert_eq!(picture.get_attr("id").is_none(), need_bytes);
            answer_iq(
                &client,
                &sent.get().get_attr("id").unwrap().to_string(),
                &refusal(403, false, None),
            )
            .await;
            let sent = decode_sent_iq(&transport, 1).await;
            assert_eq!(sent.get().get_attr("xmlns").unwrap().as_str(), "w:g2");
            let picture = sent
                .get()
                .get_optional_child("pictures")
                .unwrap()
                .get_optional_child("picture")
                .unwrap();
            assert_eq!(picture.get_attr("id").is_none(), need_bytes);
            answer_iq(
                &client,
                &sent.get().get_attr("id").unwrap().to_string(),
                &fallback_response,
            )
            .await;
            let result = task.await.unwrap().unwrap();
            if fallback_found {
                assert!(result.is_found());
            } else {
                assert_eq!(result, ProfilePictureLookup::NotAuthorized);
            }
            assert_eq!(transport.sent().len(), 2);
        }
    }
    for response in [
        found_response(),
        refusal(404, false, None),
        refusal(429, false, Some(5)),
        NodeBuilder::new("iq").attr("type", "result").build(),
    ] {
        let (client, transport) = create_iq_test_client().await;
        let c = client.clone();
        let task =
            tokio::spawn(
                async move { consumer_lookup(&c, &Jid::group("15550000001-7"), true).await },
            );
        let sent = decode_sent_iq(&transport, 0).await;
        answer_iq(
            &client,
            &sent.get().get_attr("id").unwrap().to_string(),
            &response,
        )
        .await;
        let _ = task.await.unwrap();
        assert_eq!(
            transport.sent().len(),
            1,
            "no fallback except explicitly unauthorized"
        );
    }
}

#[tokio::test]
async fn picture_lookup_community_nested_rate_limit_retains_original_response() {
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("pictures")
            .children([refusal(429, true, Some(33))
                .get_optional_child("picture")
                .unwrap()
                .clone()])
            .build()])
        .build();
    let (result, original) = canonical_roundtrip_route(
        response,
        None,
        Route::Community,
        ProfilePictureType::Preview,
    )
    .await;
    let error = result.unwrap_err();
    assert_eq!(error.server_rejection().unwrap().backoff, Some(33));
    let ContactError::Iq(IqError::ServerError { response: kept, .. }) = error else {
        panic!("typed rejection")
    };
    assert!(Arc::ptr_eq(kept.as_arc(), &original));
}

#[derive(Debug, thiserror::Error)]
#[error("fixture decode failed")]
struct FixtureParseError(Arc<()>);

struct OrdinaryParseSpec {
    marker: Arc<()>,
    core_timeout: bool,
}

impl IqSpec for OrdinaryParseSpec {
    type Response = ();

    fn build_iq(&self) -> wacore::request::InfoQuery<'static> {
        ProfilePictureSpec::preview(&Jid::pn("15550000001")).build_iq()
    }

    fn parse_response(&self, _: &wacore_binary::NodeRef<'_>) -> Result<(), anyhow::Error> {
        if self.core_timeout {
            Err(wacore::request::IqError::Timeout.into())
        } else {
            Err(anyhow::Error::new(FixtureParseError(self.marker.clone()))
                .context("ordinary parser context"))
        }
    }
}

#[tokio::test]
async fn picture_lookup_execute_leaves_ordinary_and_non_rejection_core_parse_errors_unchanged() {
    for core_timeout in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let marker = Arc::new(());
        let c = client.clone();
        let spec = OrdinaryParseSpec {
            marker: marker.clone(),
            core_timeout,
        };
        let task = tokio::spawn(async move { c.execute(spec).await });
        let sent = decode_sent_iq(&transport, 0).await;
        answer_iq(
            &client,
            &sent.get().get_attr("id").unwrap().to_string(),
            &NodeBuilder::new("iq").attr("type", "result").build(),
        )
        .await;
        let error = task.await.unwrap().unwrap_err();
        let IqError::ParseError(error) = error else {
            panic!("only typed rejections may be lifted")
        };
        if core_timeout {
            assert!(matches!(
                error.downcast_ref::<wacore::request::IqError>(),
                Some(wacore::request::IqError::Timeout)
            ));
        } else {
            assert_eq!(error.to_string(), "ordinary parser context");
            let original = error.downcast_ref::<FixtureParseError>().unwrap();
            assert!(Arc::ptr_eq(&marker, &original.0));
        }
    }
}

#[cfg(feature = "metrics")]
#[path = "pictures_metrics_tests.rs"]
mod metrics;
