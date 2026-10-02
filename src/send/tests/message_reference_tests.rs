use super::*;
use crate::{MessageId, MessageRef, MessageRefError, NewsletterMessageRef, ServerMessageId};
use wacore::types::events::InboundMessage;
use wacore::types::message::{MessageInfo, MessageSource};

fn transport_nodes(
    transport: &Arc<crate::transport::mock::CapturingMockTransport>,
) -> Vec<Arc<wacore_binary::OwnedNodeRef>> {
    crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0; 32])
        .into_iter()
        .map(|payload| {
            let bytes = wacore_binary::util::unpack(&payload).unwrap().into_owned();
            Arc::new(wacore_binary::OwnedNodeRef::new(bytes).unwrap())
        })
        .collect()
}

fn sent_message<'a>(
    nodes: &'a [Arc<wacore_binary::OwnedNodeRef>],
    result: &SendResult,
) -> &'a wacore_binary::OwnedNodeRef {
    nodes
        .iter()
        .find(|node| {
            node.get().tag == "message"
                && node.get().attrs().optional_string("id").as_deref()
                    == Some(result.message_id.as_str())
        })
        .expect("the returned operation id must identify a message frame")
}

fn participant_targets(node: &wacore_binary::OwnedNodeRef) -> Vec<Jid> {
    node.get()
        .get_optional_child("participants")
        .unwrap()
        .children()
        .unwrap()
        .iter()
        .map(|entry| {
            assert!(entry.get_optional_child("enc").is_some());
            entry.attrs().optional_jid("jid").unwrap()
        })
        .collect()
}

fn message_operation_key(message: &wa::Message, index: usize) -> &wa::MessageKey {
    let key = match index {
        0 | 1 => &message.protocol_message.as_option().unwrap().key,
        2 => &message.reaction_message.as_option().unwrap().key,
        3 => &message.pin_in_chat_message.as_option().unwrap().key,
        4 => &message.keep_in_chat_message.as_option().unwrap().key,
        _ => unreachable!(),
    };
    key.as_option().unwrap()
}

fn operation_key(result: &SendResult, index: usize) -> &wa::MessageKey {
    message_operation_key(&result.message, index)
}

#[tokio::test]
async fn legacy_and_reference_message_transport_match() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (peer, _) = seed_dm_wire_namespace_state(&client).await;
    let target =
        MessageRef::new(&peer, MessageId::new("OWN_ORIGINAL").unwrap(), None, true).unwrap();
    let raw_key = wa::MessageKey {
        remote_jid: Some(peer.to_string()),
        from_me: Some(true),
        id: Some("OWN_ORIGINAL".into()),
        participant: None,
    };
    let raw = [
        client
            .edit_message(&peer, "OWN_ORIGINAL", wa::Message::text("changed"))
            .await
            .unwrap(),
        client
            .revoke_message(&peer, "OWN_ORIGINAL", RevokeType::Sender)
            .await
            .unwrap(),
        client
            .send_reaction(&peer, raw_key.clone(), "👍")
            .await
            .unwrap(),
        client
            .pin_message(&peer, raw_key.clone(), PinDuration::Days7)
            .await
            .unwrap(),
        client.keep_message(&peer, raw_key, true).await.unwrap(),
    ];
    let typed = vec![
        client
            .edit_message_ref(&target, wa::Message::text("changed"))
            .await
            .unwrap(),
        client.revoke_message_ref(&target).await.unwrap(),
        client.send_reaction_ref(&target, "👍").await.unwrap(),
        client
            .pin_message_ref(&target, PinDuration::Days7)
            .await
            .unwrap(),
        client.keep_message_ref(&target, true).await.unwrap(),
    ];
    let status = info(&Jid::status_broadcast(), &peer, false);
    let status_ref = MessageRef::from_info(&status).unwrap();
    let status_key = wa::MessageKey {
        remote_jid: Some(Jid::status_broadcast().to_string()),
        from_me: Some(false),
        id: Some("CONTENT_TARGET".into()),
        participant: Some(peer.to_string()),
    };
    let raw_status = client
        .send_reaction(Jid::status_broadcast(), status_key, "👍")
        .await
        .unwrap();
    let typed_status = client.send_reaction_ref(&status_ref, "👍").await.unwrap();
    client
        .mark_as_played(status_ref.chat(), Some(&peer), &["CONTENT_TARGET"])
        .await
        .unwrap();
    client.mark_message_played(&status_ref).await.unwrap();
    let nodes = transport_nodes(&transport);
    let receipts: Vec<_> = nodes
        .iter()
        .filter(|node| {
            node.get().tag == "receipt"
                && node.get().attrs().optional_string("id").as_deref() == Some("CONTENT_TARGET")
        })
        .collect();
    assert_eq!(receipts.len(), 2);
    for receipt in receipts {
        assert_eq!(
            receipt.get().attrs().optional_string("type").as_deref(),
            Some("played")
        );
        assert_eq!(
            receipt.get().attrs().optional_jid("to"),
            Some(Jid::status_broadcast())
        );
        assert_eq!(
            receipt.get().attrs().optional_jid("participant"),
            Some(peer.clone())
        );
        assert_eq!(receipt.get().attrs().optional_string("class"), None);
        assert!(receipt.get().get_optional_child("list").is_none());
    }
    let control: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.get().tag != "message"
                && !(n.get().tag == "receipt"
                    && n.get().attrs().optional_string("id").as_deref() == Some("CONTENT_TARGET"))
        })
        .collect();
    assert!(control.iter().all(|node| node.get().tag == "iq"));
    for (index, (raw, typed)) in raw.iter().zip(&typed).enumerate() {
        assert_eq!(operation_key(raw, index), operation_key(typed, index));
        let raw = sent_message(&nodes, raw);
        let typed = sent_message(&nodes, typed);
        for attr in ["to", "type", "edit", "class"] {
            assert_eq!(
                raw.get().attrs().optional_string(attr),
                typed.get().attrs().optional_string(attr)
            );
        }
        assert_eq!(participant_targets(raw), participant_targets(typed));
    }
    let raw_node = sent_message(&nodes, &raw_status);
    let typed_node = sent_message(&nodes, &typed_status);
    for node in [raw_node, typed_node] {
        assert_eq!(
            node.get().attrs().optional_jid("to"),
            Some(Jid::status_broadcast())
        );
        assert_eq!(node.get().attrs().optional_string("class"), None);
        assert_eq!(participant_targets(node), vec![peer.clone()]);
    }
    assert_eq!(
        raw_status.message.reaction_message.as_option().unwrap().key,
        typed_status
            .message
            .reaction_message
            .as_option()
            .unwrap()
            .key
    );
}

fn info(chat: &Jid, sender: &Jid, from_me: bool) -> MessageInfo {
    MessageInfo {
        id: "CONTENT_TARGET".into(),
        source: MessageSource {
            chat: chat.clone(),
            sender: sender.clone(),
            is_from_me: from_me,
            is_group: chat.is_group(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[tokio::test]
async fn received_context_and_own_result_references_borrow_metadata() {
    let (client, _) = crate::test_utils::create_iq_test_client().await;
    let chat: Jid = "120363000000000001@g.us".parse().unwrap();
    let sender: Jid = "100000000000001@lid".parse().unwrap();
    let parent = wa::MessageKey {
        id: Some("PARENT_POST".into()),
        ..Default::default()
    };
    let inbound = InboundMessage::builder()
        .message(Arc::new(wa::Message::text("comment")))
        .info(Arc::new(info(&chat, &sender, false)))
        .ephemeral_expiration(86400)
        .comment_target(Box::new(parent))
        .build();
    let before = Arc::strong_count(&inbound.message);
    let reference = inbound.message_ref().unwrap();
    assert!(std::ptr::eq(
        reference.source().unwrap(),
        &inbound.info.source
    ));
    assert!(std::ptr::eq(
        reference.comment_target().unwrap(),
        inbound.comment_target.as_deref().unwrap()
    ));
    assert_eq!(reference.ephemeral_expiration(), Some(86400));
    assert_eq!(reference.id().as_str(), "CONTENT_TARGET");
    assert_eq!(reference.to_raw_key().participant, Some(sender.to_string()));
    assert_eq!(Arc::strong_count(&inbound.message), before);
    let ctx = crate::bot::MessageContext::from_inbound(&inbound, client.clone());
    let ctx_ref = ctx.message_ref().unwrap();
    assert_eq!(ctx_ref.to_raw_key(), ctx.message_key());
    assert!(std::ptr::eq(
        ctx_ref.comment_target().unwrap(),
        ctx.comment_target.as_deref().unwrap()
    ));
    assert!(Arc::ptr_eq(&ctx.message, &inbound.message));
    let dm: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
    let received = info(&dm, &dm, false);
    let dm_ref = MessageRef::from_info(&received).unwrap();
    assert_eq!(dm_ref.sender(), Some(&dm));
    assert_eq!(dm_ref.to_raw_key().participant, None);
    assert_eq!(dm_ref.to_raw_key().from_me, Some(false));
    let own = SendResult {
        message_id: "OWN_CONTENT".into(),
        to: dm,
        message: Arc::new(wa::Message::text("own")),
        recipient_fanout: None,
    };
    let own_ref = own.message_ref().unwrap();
    assert!(own_ref.from_me());
    assert_eq!(own_ref.sender(), None);
    assert_eq!(own_ref.to_raw_key(), own.message_key());
    assert_eq!(Arc::strong_count(&own.message), 1);
    let own_status = SendResult {
        to: Jid::status_broadcast(),
        ..own
    };
    let status_ref = own_status.message_ref().unwrap();
    assert!(status_ref.from_me());
    assert_eq!(status_ref.sender(), None);
}

#[tokio::test]
async fn own_send_reference_and_channel_reference_are_distinct() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (peer, _) = seed_dm_wire_namespace_state(&client).await;
    let channel: Jid = "120363000000000001@newsletter".parse().unwrap();
    for chat in [&peer, &channel] {
        // This is the content envelope's client id, set by the existing send
        // option, not an original target id invented for a later operation.
        let result = client
            .send_message_with_options(
                chat,
                wa::Message::text("post"),
                SendOptions::default().with_message_id("OWN_CONTENT"),
            )
            .await
            .unwrap();
        let body = result.message.clone();
        assert_eq!(result.message_id, "OWN_CONTENT");
        assert_eq!(&result.to, chat);
        assert_eq!(result.message.conversation.as_deref(), Some("post"));
        assert_eq!(result.stanza_id().unwrap().as_str(), "OWN_CONTENT");
        if chat.is_newsletter() {
            assert_eq!(
                result.message_ref().unwrap_err(),
                MessageRefError::ExpectedChat
            );
            let sent_post = result.newsletter_ref().unwrap();
            assert_eq!(sent_post.message_id().unwrap().as_str(), "OWN_CONTENT");
            assert_eq!(sent_post.server_id(), None);
            assert_eq!(sent_post.from_me(), Some(true));
            let post = NewsletterMessageRef::new(
                sent_post.chat(),
                Some(MessageId::new("OWN_CONTENT").unwrap()),
                Some(ServerMessageId::new(0)),
            )
            .unwrap();
            assert_eq!(post.server_id().unwrap().get(), 0);
            assert!(std::ptr::eq(sent_post.chat(), &result.to));
        } else {
            let reference = result.message_ref().unwrap();
            assert!(reference.from_me());
            assert_eq!(reference.to_raw_key(), result.message_key());
            assert_eq!(
                result.newsletter_ref().unwrap_err(),
                MessageRefError::ExpectedNewsletter
            );
            assert!(std::ptr::eq(reference.chat(), &result.to));
        }
        assert!(Arc::ptr_eq(&body, &result.message));
        assert_eq!(Arc::strong_count(&body), 2);
        let cloned = result.clone();
        assert!(Arc::ptr_eq(&cloned.message, &result.message));
        drop(cloned);
        assert_eq!(Arc::strong_count(&body), 2);
    }
    let nodes = transport_nodes(&transport);
    let messages: Vec<_> = nodes
        .iter()
        .filter(|node| {
            node.get().tag == "message"
                && node.get().attrs().optional_string("id").as_deref() == Some("OWN_CONTENT")
        })
        .collect();
    assert_eq!(messages.len(), 2);
    for chat in [&peer, &channel] {
        let matching: Vec<_> = messages
            .iter()
            .filter(|node| node.get().attrs().optional_jid("to").as_ref() == Some(chat))
            .collect();
        assert_eq!(matching.len(), 1);
        let node = matching[0];
        if chat.is_newsletter() {
            assert_eq!(node.get().attrs().optional_u64("server_id"), None);
            let bytes = node
                .get()
                .get_optional_child("plaintext")
                .unwrap()
                .content_bytes()
                .unwrap();
            assert_eq!(
                waproto::codec::message_decode(bytes)
                    .unwrap()
                    .conversation
                    .as_deref(),
                Some("post")
            );
        } else {
            assert_eq!(participant_targets(node), vec![peer.clone()]);
        }
    }
    let control: Vec<_> = nodes
        .iter()
        .filter(|node| node.get().tag != "message")
        .collect();
    assert!(control.iter().all(|node| node.get().tag == "iq"));
}

#[tokio::test]
async fn own_dm_reference_operations_keep_content_and_operation_ids_separate() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (peer, _) = seed_dm_wire_namespace_state(&client).await;
    let own = MessageRef::new(&peer, MessageId::new("OWN_ORIGINAL").unwrap(), None, true).unwrap();
    let results = [
        client
            .edit_message_ref(&own, wa::Message::text("changed"))
            .await
            .unwrap(),
        client.revoke_message_ref(&own).await.unwrap(),
        client.send_reaction_ref(&own, "👍").await.unwrap(),
        client
            .pin_message_ref(&own, PinDuration::Days7)
            .await
            .unwrap(),
        client.keep_message_ref(&own, true).await.unwrap(),
    ];
    let nodes = transport_nodes(&transport);
    let control: Vec<_> = nodes.iter().filter(|n| n.get().tag != "message").collect();
    assert!(control.iter().all(|node| node.get().tag == "iq"));
    for (i, result) in results.iter().enumerate() {
        let node = sent_message(&nodes, result);
        assert_eq!(
            node.get().attrs().optional_string("id").as_deref(),
            Some(result.stanza_id().unwrap().as_str())
        );
        assert_eq!(node.get().attrs().optional_jid("to"), Some(peer.clone()));
        let key = match i {
            0 | 1 => result
                .message
                .protocol_message
                .as_option()
                .unwrap()
                .key
                .as_option()
                .unwrap(),
            2 => result
                .message
                .reaction_message
                .as_option()
                .unwrap()
                .key
                .as_option()
                .unwrap(),
            3 => result
                .message
                .pin_in_chat_message
                .as_option()
                .unwrap()
                .key
                .as_option()
                .unwrap(),
            4 => result
                .message
                .keep_in_chat_message
                .as_option()
                .unwrap()
                .key
                .as_option()
                .unwrap(),
            _ => unreachable!(),
        };
        assert_eq!(key.id.as_deref(), Some("OWN_ORIGINAL"));
        assert_eq!(key.from_me, Some(true));
        assert_eq!(key.participant, None);
        assert_ne!(key.id.as_deref(), Some(result.message_id.as_str()));
    }
}

#[tokio::test]
async fn newsletter_reference_operations_reach_transport_with_distinct_ids() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let chat: Jid = "120363000000000001@newsletter".parse().unwrap();
    let target = NewsletterMessageRef::new(
        &chat,
        Some(MessageId::new("CLIENT_CONTENT").unwrap()),
        Some(ServerMessageId::new(u64::MAX)),
    )
    .unwrap();
    let ack_id = client
        .newsletter()
        .send_reaction_ref(&target, "👍")
        .await
        .unwrap();
    let vote_id = client
        .newsletter()
        .send_poll_vote_ref(&target, &[[7; 32]])
        .await
        .unwrap();
    client
        .newsletter()
        .edit_message_ref(&target, wa::Message::text("edited"))
        .await
        .unwrap();
    client
        .newsletter()
        .revoke_message_ref(&target)
        .await
        .unwrap();
    for (index, id) in [&ack_id, &vote_id].iter().enumerate() {
        let node = crate::test_utils::decode_sent_iq(&transport, index).await;
        let node = node.get();
        assert_eq!(
            node.attrs().optional_string("server_id").as_deref(),
            Some(u64::MAX.to_string().as_str())
        );
        assert_eq!(
            node.attrs().optional_string("id").as_deref(),
            Some(id.as_str())
        );
        assert_ne!(id.as_str(), "CLIENT_CONTENT");
    }
    for (index, edit) in [(2, "3"), (3, "8")] {
        let node = crate::test_utils::decode_sent_iq(&transport, index).await;
        let node = node.get();
        assert_eq!(
            node.attrs().optional_string("id").as_deref(),
            Some("CLIENT_CONTENT")
        );
        assert_eq!(node.attrs().optional_string("edit").as_deref(), Some(edit));
        assert_eq!(node.attrs().optional_string("server_id"), None);
        let plaintext = node.get_optional_child("plaintext").unwrap();
        if index == 2 {
            let decoded =
                waproto::codec::message_decode(plaintext.content_bytes().unwrap()).unwrap();
            assert_eq!(decoded.conversation.as_deref(), Some("edited"));
        } else {
            assert!(plaintext.content_bytes().is_none_or(<[u8]>::is_empty));
        }
    }
    let vote = crate::test_utils::decode_sent_iq(&transport, 1).await;
    assert_eq!(
        vote.get()
            .get_optional_child("votes")
            .unwrap()
            .children()
            .unwrap()[0]
            .content_bytes(),
        Some([7; 32].as_slice())
    );
    let ack = wacore::types::events::ServerAck::builder()
        .id(vote_id.to_string())
        .class("message".to_string())
        .build();
    assert!(vote_id.matches_message_ack(&ack));
    assert_eq!(ack.stanza_id().unwrap(), vote_id);
    assert!(
        !crate::StanzaId::new("CLIENT_CONTENT")
            .unwrap()
            .matches_message_ack(&ack)
    );
}

#[tokio::test]
async fn missing_newsletter_ids_and_invalid_chat_operations_send_nothing() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let channel: Jid = "120363000000000001@newsletter".parse().unwrap();
    let client_only =
        NewsletterMessageRef::new(&channel, Some(MessageId::new("CLIENT").unwrap()), None).unwrap();
    let server_only = NewsletterMessageRef::new(&channel, None, Some(0.into())).unwrap();
    assert!(matches!(
        client
            .newsletter()
            .send_reaction_ref(&client_only, "👍")
            .await,
        Err(crate::NewsletterError::MessageRef(
            MessageRefError::MissingServerMessageId
        ))
    ));
    assert!(matches!(
        client
            .newsletter()
            .send_poll_vote_ref(&client_only, &[])
            .await,
        Err(crate::NewsletterError::MessageRef(
            MessageRefError::MissingServerMessageId
        ))
    ));
    assert!(matches!(
        client
            .newsletter()
            .edit_message_ref(&server_only, wa::Message::text("bad"))
            .await,
        Err(crate::NewsletterError::MessageRef(
            MessageRefError::MissingMessageId
        ))
    ));
    assert!(matches!(
        client.newsletter().revoke_message_ref(&server_only).await,
        Err(crate::NewsletterError::MessageRef(
            MessageRefError::MissingMessageId
        ))
    ));
    let dm: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
    let received = MessageRef::new(&dm, MessageId::new("DM").unwrap(), Some(&dm), false).unwrap();
    assert!(matches!(
        client
            .edit_message_ref(&received, wa::Message::default())
            .await,
        Err(SendError::MessageRef(MessageRefError::NotFromMe))
    ));
    assert!(matches!(
        client.revoke_message_ref(&received).await,
        Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
    ));
    let status = Jid::status_broadcast();
    let reference =
        MessageRef::new(&status, MessageId::new("STATUS").unwrap(), Some(&dm), false).unwrap();
    assert!(matches!(
        client.pin_message_ref(&reference, PinDuration::Days7).await,
        Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
    ));
    assert!(matches!(
        client.keep_message_ref(&reference, true).await,
        Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
    ));
    assert_eq!(transport.sent_count(), 0);
    let own = MessageRef::new(&dm, MessageId::new("OWN").unwrap(), None, true).unwrap();
    let err = client.mark_message_read(&own).await.unwrap_err();
    assert_eq!(
        err.downcast_ref::<MessageRefError>(),
        Some(&MessageRefError::ExpectedIncoming)
    );
    assert_eq!(transport.sent_count(), 0);
}

#[tokio::test]
async fn group_reference_operations_encrypt_operation_specific_keys() {
    use wacore::libsignal::protocol::{
        create_sender_key_distribution_message, group_decrypt,
        process_sender_key_distribution_message,
    };
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        let mut routing = (*fixture
            .client
            .get_group_cache()
            .get(&fixture.group)
            .await
            .unwrap())
        .clone();
        routing.is_community_announce = Some(false);
        fixture
            .client
            .get_group_cache()
            .insert(fixture.group.clone(), Arc::new(routing))
            .await;
        fixture.send_text("prime sender chain").await;
        let receiver = crate::test_utils::create_test_client().await;
        let name = SenderKeyName::from_parts(
            &fixture.group.to_string(),
            fixture.own_sending.to_protocol_address().as_str(),
        );
        let mut sender_stores = fixture.client.signal_adapter();
        let mut receiver_stores = receiver.signal_adapter();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let distribution = create_sender_key_distribution_message(
            &name,
            &mut sender_stores.sender_key_store,
            &mut rng,
        )
        .await
        .unwrap();
        process_sender_key_distribution_message(
            &name,
            &distribution,
            &mut receiver_stores.sender_key_store,
        )
        .await
        .unwrap();
        let original = info(&fixture.group, &fixture.member, false);
        let received = MessageRef::from_info(&original).unwrap();
        let own = MessageRef::new(
            &fixture.group,
            MessageId::new("OWN_ORIGINAL").unwrap(),
            None,
            true,
        )
        .unwrap();
        let calls = [
            fixture
                .client
                .edit_message_ref(&own, wa::Message::text("changed"))
                .await
                .unwrap(),
            fixture.client.revoke_message_ref(&own).await.unwrap(),
            fixture.client.revoke_message_ref(&received).await.unwrap(),
            fixture
                .client
                .send_reaction_ref(&received, "👍")
                .await
                .unwrap(),
            fixture
                .client
                .pin_message_ref(&received, PinDuration::Days7)
                .await
                .unwrap(),
            fixture
                .client
                .keep_message_ref(&received, true)
                .await
                .unwrap(),
            fixture.client.unpin_message_ref(&received).await.unwrap(),
        ];
        for (i, result) in calls.iter().enumerate() {
            let stanza = fixture.stanza(i + 1).await;
            let stanza = stanza.get();
            assert_eq!(
                stanza.attrs().optional_string("id").as_deref(),
                Some(result.stanza_id().unwrap().as_str())
            );
            let enc = stanza.get_optional_child("enc").unwrap();
            assert_eq!(
                enc.attrs().optional_string("type").as_deref(),
                Some("skmsg")
            );
            let padded = group_decrypt(
                enc.content_bytes().unwrap(),
                &mut receiver_stores.sender_key_store,
                &name,
            )
            .await
            .unwrap();
            let bytes = wacore::messages::unpad_plaintext(padded, 2).unwrap();
            let decoded = waproto::codec::message_decode(&bytes).unwrap();
            let key = match i {
                0..=2 => decoded
                    .protocol_message
                    .as_option()
                    .unwrap()
                    .key
                    .as_option()
                    .unwrap(),
                3 => decoded
                    .reaction_message
                    .as_option()
                    .unwrap()
                    .key
                    .as_option()
                    .unwrap(),
                4 | 6 => decoded
                    .pin_in_chat_message
                    .as_option()
                    .unwrap()
                    .key
                    .as_option()
                    .unwrap(),
                5 => decoded
                    .keep_in_chat_message
                    .as_option()
                    .unwrap()
                    .key
                    .as_option()
                    .unwrap(),
                _ => unreachable!(),
            };
            let expected_participant = match i {
                0 => Some(fixture.own_sending.to_non_ad_string()),
                1 => None,
                _ => Some(fixture.member.to_string()),
            };
            assert_eq!(key.participant, expected_participant);
            assert_eq!(key.from_me, Some(i < 2));
            assert_eq!(key.remote_jid, Some(fixture.group.to_string()));
            assert_eq!(
                key.id.as_deref(),
                Some(if i < 2 {
                    "OWN_ORIGINAL"
                } else {
                    "CONTENT_TARGET"
                })
            );
            assert_ne!(key.id.as_deref(), Some(result.message_id.as_str()));
        }
    }
}

async fn cache_group_subtype(fixture: &GroupSendFixture, flag: Option<bool>) {
    let mut routing = (*fixture
        .client
        .get_group_cache()
        .get(&fixture.group)
        .await
        .unwrap())
    .clone();
    routing.is_community_announce = flag;
    fixture
        .client
        .get_group_cache()
        .insert(fixture.group.clone(), Arc::new(routing))
        .await;
}

#[tokio::test]
async fn review_broadcast_list_reaction_rejects_before_identity_or_send() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(None))
        .await;
    let chat: Jid = "123456789@broadcast".parse().unwrap();
    assert!(chat.is_broadcast_list());
    assert!(!chat.is_status_broadcast());
    let target =
        MessageRef::new(&chat, MessageId::new("LIST_TARGET").unwrap(), None, true).unwrap();
    assert!(matches!(
        client.send_reaction_ref(&target, "👍").await,
        Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
    ));
    assert_eq!(transport.sent_count(), 0);
}

#[tokio::test]
async fn review_cached_cag_typed_edit_rejects_without_message_send() {
    let fixture = GroupSendFixture::new().await;
    cache_group_subtype(&fixture, Some(true)).await;
    let target = MessageRef::new(
        &fixture.group,
        MessageId::new("OWN_CAG_TARGET").unwrap(),
        None,
        true,
    )
    .unwrap();
    assert!(matches!(
        fixture
            .client
            .edit_message_ref(&target, wa::Message::text("changed"))
            .await,
        Err(SendError::InvalidRequest(_))
    ));
    assert_eq!(fixture.transport.sent_count(), 0);
}

#[tokio::test]
async fn review_live_newsletter_context_does_not_infer_ownership() {
    let (client, _) = crate::test_utils::create_iq_test_client().await;
    let own: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
    let node = NodeBuilder::new("message")
        .attr("from", "120363000000000001@newsletter")
        .attr("id", "LIVE_CLIENT_CONTENT")
        .attr("server_id", 0u64)
        .attr("t", "123")
        .build();
    let parsed = wacore::messages::parse_message_info(&node.as_node_ref(), &own, None).unwrap();
    assert!(!parsed.source.is_from_me);
    let inbound = InboundMessage::builder()
        .info(Arc::new(parsed))
        .message(Arc::new(wa::Message::default()))
        .build();
    let context = crate::bot::MessageContext::from_inbound(&inbound, client);
    for (target, owner_chat) in [
        (inbound.newsletter_ref().unwrap(), &inbound.info.source.chat),
        (context.newsletter_ref().unwrap(), &context.info.source.chat),
    ] {
        assert_eq!(target.from_me(), None);
        assert_eq!(target.message_id().unwrap().as_str(), "LIVE_CLIENT_CONTENT");
        assert_eq!(target.server_id().unwrap().get(), 0);
        assert!(std::ptr::eq(target.chat(), owner_chat));
        assert_eq!(target.chat(), &inbound.info.source.chat);
    }
}

#[tokio::test]
async fn review_raw_newsletter_reaction_preserves_status_delegation() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let chat = Jid::status_broadcast();
    let id = client
        .newsletter()
        .send_reaction(&chat, 42, "👍")
        .await
        .unwrap();
    let node = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_eq!(node.get().tag, "message");
    assert_eq!(node.get().attrs().optional_jid("to"), Some(chat.clone()));
    assert_eq!(node.get().attrs().optional_u64("server_id"), Some(42));
    assert_eq!(
        node.get().attrs().optional_string("id").as_deref(),
        Some(id.as_str())
    );
    assert_eq!(
        node.get()
            .get_optional_child("reaction")
            .unwrap()
            .attrs()
            .optional_string("code")
            .as_deref(),
        Some("👍")
    );
    assert_eq!(
        NewsletterMessageRef::new(&chat, None, Some(ServerMessageId::new(42))).unwrap_err(),
        MessageRefError::ExpectedNewsletter
    );
    assert_eq!(transport.sent_count(), 1);
}

#[tokio::test]
async fn review_own_group_send_reference_keys_encrypt_original_author() {
    use wacore::libsignal::protocol::{
        create_sender_key_distribution_message, group_decrypt,
        process_sender_key_distribution_message,
    };
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        cache_group_subtype(&fixture, Some(false)).await;
        let content = fixture
            .client
            .send_message(&fixture.group, wa::Message::text("own original"))
            .await
            .unwrap();
        let target = content.message_ref().unwrap();
        assert!(target.from_me());
        assert_eq!(target.sender(), None);
        assert_eq!(target.to_raw_key().participant, None);
        assert!(std::ptr::eq(target.chat(), &content.to));
        assert_eq!(Arc::strong_count(&content.message), 1);
        let receiver = crate::test_utils::create_test_client().await;
        let name = SenderKeyName::from_parts(
            &fixture.group.to_string(),
            fixture.own_sending.to_protocol_address().as_str(),
        );
        let mut sender_stores = fixture.client.signal_adapter();
        let mut receiver_stores = receiver.signal_adapter();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let distribution = create_sender_key_distribution_message(
            &name,
            &mut sender_stores.sender_key_store,
            &mut rng,
        )
        .await
        .unwrap();
        process_sender_key_distribution_message(
            &name,
            &distribution,
            &mut receiver_stores.sender_key_store,
        )
        .await
        .unwrap();
        let results = [
            fixture
                .client
                .edit_message_ref(&target, wa::Message::text("edited"))
                .await
                .unwrap(),
            fixture.client.revoke_message_ref(&target).await.unwrap(),
            fixture
                .client
                .send_reaction_ref(&target, "👍")
                .await
                .unwrap(),
            fixture
                .client
                .pin_message_ref(&target, PinDuration::Days7)
                .await
                .unwrap(),
            fixture.client.unpin_message_ref(&target).await.unwrap(),
            fixture
                .client
                .keep_message_ref(&target, true)
                .await
                .unwrap(),
            fixture
                .client
                .keep_message_ref(&target, false)
                .await
                .unwrap(),
        ];
        let nodes = transport_nodes(&fixture.transport);
        for (index, result) in results.iter().enumerate() {
            let node = sent_message(&nodes, result);
            let enc = node.get().get_optional_child("enc").unwrap();
            assert_eq!(
                enc.attrs().optional_string("type").as_deref(),
                Some("skmsg")
            );
            let padded = group_decrypt(
                enc.content_bytes().unwrap(),
                &mut receiver_stores.sender_key_store,
                &name,
            )
            .await
            .unwrap();
            let decoded = waproto::codec::message_decode(
                &wacore::messages::unpad_plaintext(padded, 2).unwrap(),
            )
            .unwrap();
            let kind = match index {
                0 | 1 => index,
                2 => 2,
                3 | 4 => 3,
                5 | 6 => 4,
                _ => unreachable!(),
            };
            let decoded_key = message_operation_key(&decoded, kind);
            assert_eq!(decoded_key, operation_key(result, kind));
            assert_eq!(decoded_key.id.as_deref(), Some(content.message_id.as_str()));
            assert_ne!(decoded_key.id.as_deref(), Some(result.message_id.as_str()));
            assert_eq!(decoded_key.remote_jid, Some(fixture.group.to_string()));
            assert_eq!(decoded_key.from_me, Some(true));
            assert_eq!(
                decoded_key.participant,
                if index == 1 {
                    None
                } else {
                    Some(fixture.own_sending.to_non_ad_string())
                }
            );
        }
        assert_eq!(target.sender(), None);
        assert_eq!(target.to_raw_key().participant, None);
        assert_eq!(Arc::strong_count(&content.message), 1);
    }
}

#[tokio::test]
async fn review_own_lid_group_addons_do_not_invent_missing_identity() {
    let fixture = GroupSendFixture::new_lid(2).await;
    cache_group_subtype(&fixture, Some(false)).await;
    let content = fixture
        .client
        .send_message(&fixture.group, wa::Message::text("own"))
        .await
        .unwrap();
    let target = content.message_ref().unwrap();
    fixture
        .client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(None))
        .await;
    let before = fixture.transport.sent_count();
    for result in [
        fixture.client.send_reaction_ref(&target, "👍").await,
        fixture
            .client
            .pin_message_ref(&target, PinDuration::Days7)
            .await,
        fixture.client.unpin_message_ref(&target).await,
        fixture.client.keep_message_ref(&target, true).await,
    ] {
        assert!(matches!(
            result,
            Err(SendError::MessageRef(MessageRefError::MissingSender))
        ));
    }
    assert_eq!(fixture.transport.sent_count(), before);
}

#[tokio::test]
async fn review_typed_edit_validates_origin_before_group_lookup() {
    let fixture = GroupSendFixture::new().await;
    cache_group_subtype(&fixture, None).await;
    let incoming = info(&fixture.group, &fixture.member, false);
    let target = MessageRef::from_info(&incoming).unwrap();
    assert!(matches!(
        fixture
            .client
            .edit_message_ref(&target, wa::Message::text("bad"))
            .await,
        Err(SendError::MessageRef(MessageRefError::NotFromMe))
    ));
    let status = Jid::status_broadcast();
    let target = MessageRef::new(
        &status,
        MessageId::new("STATUS_TARGET").unwrap(),
        None,
        true,
    )
    .unwrap();
    assert!(matches!(
        fixture
            .client
            .edit_message_ref(&target, wa::Message::text("bad"))
            .await,
        Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
    ));
    assert_eq!(fixture.transport.sent_count(), 0);
}

#[tokio::test]
async fn review_unknown_group_subtype_queries_and_rejects_confirmed_cag() {
    let fixture = GroupSendFixture::new().await;
    cache_group_subtype(&fixture, None).await;
    let client = fixture.client.clone();
    let chat = fixture.group.clone();
    let query = tokio::spawn(async move {
        let target =
            MessageRef::new(&chat, MessageId::new("CAG_TARGET").unwrap(), None, true).unwrap();
        client
            .edit_message_ref(&target, wa::Message::text("bad"))
            .await
    });
    let sent = crate::test_utils::decode_sent_iq(&fixture.transport, 0).await;
    assert_eq!(sent.get().tag, "iq");
    assert_eq!(
        sent.get().attrs().optional_string("xmlns").as_deref(),
        Some("w:g2")
    );
    let id = sent.get().attrs().optional_string("id").unwrap();
    let response = NodeBuilder::new("iq")
        .attr("id", &*id)
        .attr("type", "result")
        .children([NodeBuilder::new("group")
            .attr("id", fixture.group.user.as_str())
            .children([NodeBuilder::new("default_sub_group").build()])
            .build()])
        .build();
    crate::test_utils::answer_iq(&fixture.client, &id, &response).await;
    assert!(matches!(
        query.await.unwrap(),
        Err(SendError::InvalidRequest(_))
    ));
    let nodes = transport_nodes(&fixture.transport);
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].get().tag, "iq");
}

#[tokio::test]
async fn review_metadata_errors_preserve_source_and_rejection_allocation() {
    use std::error::Error;
    for edit in [true, false] {
        let fixture = GroupSendFixture::new().await;
        cache_group_subtype(&fixture, Some(false)).await;
        let content = fixture
            .client
            .send_message(&fixture.group, wa::Message::text("own"))
            .await
            .unwrap();
        if edit {
            cache_group_subtype(&fixture, None).await;
        } else {
            fixture
                .client
                .get_group_cache()
                .invalidate(&fixture.group)
                .await;
        }
        let before = fixture.transport.sent_count();
        let client = fixture.client.clone();
        let query = tokio::spawn(async move {
            let target = content.message_ref().unwrap();
            if edit {
                client
                    .edit_message_ref(&target, wa::Message::text("bad"))
                    .await
            } else {
                client.pin_message_ref(&target, PinDuration::Days7).await
            }
        });
        let sent = crate::test_utils::decode_sent_iq(&fixture.transport, before).await;
        assert_eq!(sent.get().tag, "iq");
        assert_eq!(
            sent.get().attrs().optional_string("xmlns").as_deref(),
            Some("w:g2")
        );
        let id = sent.get().attrs().optional_string("id").unwrap();
        let response = NodeBuilder::new("iq")
            .attr("id", &*id)
            .attr("type", "error")
            .attr("scope", "TYPED_REFERENCE_LOOKUP")
            .children([NodeBuilder::new("error")
                .attr("code", "429")
                .attr("text", "rate-overlimit")
                .attr("type", "wait")
                .attr("backoff", "7")
                .build()])
            .build();
        let original = crate::test_utils::answer_iq(&fixture.client, &id, &response).await;
        let error = query.await.unwrap().unwrap_err();
        assert!(error.source().unwrap().downcast_ref::<IqError>().is_some());
        let SendError::Iq(IqError::ServerError {
            code,
            text,
            error_type,
            backoff,
            response,
        }) = error
        else {
            panic!("the exact metadata rejection must remain typed")
        };
        assert_eq!(code, 429);
        assert_eq!(text, "rate-overlimit");
        assert_eq!(error_type.as_deref(), Some("wait"));
        assert_eq!(backoff, Some(7));
        assert!(Arc::ptr_eq(response.as_arc(), &original));
        assert_eq!(
            response.get().attrs().optional_string("scope").as_deref(),
            Some("TYPED_REFERENCE_LOOKUP")
        );
        assert_eq!(fixture.transport.sent_count(), before + 1);
        assert_eq!(
            transport_nodes(&fixture.transport)
                .iter()
                .filter(|node| node.get().tag == "message")
                .count(),
            1
        );
    }
}

#[tokio::test]
async fn review_own_pn_group_addons_reject_missing_identity() {
    let fixture = GroupSendFixture::new().await;
    cache_group_subtype(&fixture, Some(false)).await;
    let content = fixture
        .client
        .send_message(&fixture.group, wa::Message::text("own"))
        .await
        .unwrap();
    fixture
        .client
        .persistence_manager
        .process_command(DeviceCommand::SetId(None))
        .await;
    let target = content.message_ref().unwrap();
    let before = fixture.transport.sent_count();
    for result in [
        fixture.client.send_reaction_ref(&target, "👍").await,
        fixture
            .client
            .pin_message_ref(&target, PinDuration::Days7)
            .await,
        fixture.client.unpin_message_ref(&target).await,
        fixture.client.keep_message_ref(&target, true).await,
    ] {
        assert!(matches!(result, Err(SendError::NotLoggedIn)));
    }
    assert_eq!(fixture.transport.sent_count(), before);
}

#[tokio::test]
async fn review_own_status_reaction_pn_fallback_matches_raw_fanout() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    seed_dm_wire_namespace_state(&client).await;
    let own = client.persistence_manager.get_device_snapshot();
    let pn = own.pn.as_ref().unwrap();
    let lid = own.lid.as_ref().unwrap();
    client
        .add_lid_pn_mapping(
            &lid.user,
            &pn.user,
            crate::lid_pn_cache::LearningSource::Usync,
        )
        .await
        .unwrap();
    client
        .update_device_list(wacore::store::traits::DeviceListRecord {
            user: pn.user.as_str().into(),
            devices: [
                wacore::store::traits::DeviceInfo::new(0, None),
                wacore::store::traits::DeviceInfo::new(1, None),
            ]
            .into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        })
        .await
        .unwrap();
    crate::test_utils::seed_peer_session(&client, &lid.with_device(1)).await;
    let status = Jid::status_broadcast();
    let target = MessageRef::new(
        &status,
        MessageId::new("OWN_STATUS_TARGET").unwrap(),
        None,
        true,
    )
    .unwrap();
    let mut key = target.to_raw_key();
    key.participant = Some(pn.to_non_ad_string());
    let raw = client.send_reaction(&status, key, "👍").await.unwrap();
    let typed = client.send_reaction_ref(&target, "👍").await.unwrap();
    assert_eq!(operation_key(&raw, 2), operation_key(&typed, 2));
    assert_eq!(
        operation_key(&typed, 2).participant,
        Some(pn.to_non_ad_string())
    );
    let nodes = transport_nodes(&transport);
    let raw_node = sent_message(&nodes, &raw);
    let typed_node = sent_message(&nodes, &typed);
    assert_eq!(
        participant_targets(raw_node),
        participant_targets(typed_node)
    );
    assert!(!participant_targets(typed_node).is_empty());
    for node in [raw_node, typed_node] {
        assert_eq!(node.get().attrs().optional_jid("to"), Some(status.clone()));
        assert_eq!(node.get().attrs().optional_string("class"), None);
    }
}

#[tokio::test]
async fn status_reference_reaction_and_receipts_keep_author_scope() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (author, _) = seed_dm_wire_namespace_state(&client).await;
    let status = info(&Jid::status_broadcast(), &author, false);
    let target = MessageRef::from_info(&status).unwrap();
    let result = client.send_reaction_ref(&target, "👍").await.unwrap();
    assert_eq!(
        result
            .message
            .reaction_message
            .as_option()
            .unwrap()
            .key
            .as_option()
            .unwrap()
            .participant,
        Some(author.to_string())
    );
    client.mark_message_read(&target).await.unwrap();
    client.mark_message_played(&target).await.unwrap();
    let nodes = transport_nodes(&transport);
    let node = sent_message(&nodes, &result);
    assert_eq!(
        node.get().attrs().optional_jid("to"),
        Some(Jid::status_broadcast())
    );
    assert_eq!(participant_targets(node), vec![author.clone()]);
    let receipts: Vec<_> = nodes
        .iter()
        .filter(|n| {
            n.get().tag == "receipt"
                && n.get().attrs().optional_string("id").as_deref() == Some("CONTENT_TARGET")
        })
        .collect();
    assert_eq!(receipts.len(), 2);
    for (receipt_type, expected_class) in [("read", Some("status")), ("played", None)] {
        let matching: Vec<_> = receipts
            .iter()
            .filter(|node| {
                node.get().attrs().optional_string("type").as_deref() == Some(receipt_type)
            })
            .collect();
        assert_eq!(matching.len(), 1);
        let node = matching[0];
        assert_eq!(
            node.get().attrs().optional_jid("to"),
            Some(Jid::status_broadcast())
        );
        assert_eq!(
            node.get().attrs().optional_string("id").as_deref(),
            Some("CONTENT_TARGET")
        );
        assert_eq!(
            node.get().attrs().optional_jid("participant"),
            Some(author.clone())
        );
        assert_eq!(
            node.get().attrs().optional_string("class").as_deref(),
            expected_class
        );
    }
}
