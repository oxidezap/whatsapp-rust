use super::*;
use crate::{EditRequest, MessageId, MessageRef, SendRequest, StanzaId};
use wacore::proto_helpers::{MessageBuilderExt, MessageExt};

#[tokio::test]
async fn secret_event_edit_selects_event_crypto_and_wire_kind() {
    assert_secret_creation_edit_wire(true).await;
}

#[tokio::test]
async fn secret_poll_edit_selects_poll_crypto_and_wire_kind() {
    assert_secret_creation_edit_wire(false).await;
}

async fn assert_secret_creation_edit_wire(is_event: bool) {
    use wa::message::secret_encrypted_message::SecretEncType;
    use wacore::libsignal::protocol::{
        SenderKeyName, create_sender_key_distribution_message, group_decrypt,
        process_sender_key_distribution_message,
    };
    use wacore::secret_enc_addon::ModificationType;

    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        let (original, creator, secret) = if is_event {
            let created = fixture
                .client
                .events()
                .create(
                    &fixture.group,
                    crate::EventCreationParams {
                        name: "Launch".into(),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            (
                created.send_result().clone(),
                created.creator().clone(),
                created.secret().clone(),
            )
        } else {
            let created = fixture
                .client
                .polls()
                .create_quiz(&fixture.group, "Question", &["Yes".into(), "No".into()], 0)
                .await
                .unwrap();
            (
                created.send_result().clone(),
                created.creator().clone(),
                created.secret().clone(),
            )
        };
        let mut update = (*original.message).clone();
        update.message_context_info = buffa::MessageField::none();
        if is_event {
            update.event_message.get_or_insert_default().name = Some("Updated launch".into());
        } else {
            update.poll_creation_message_v3.get_or_insert_default().name =
                Some("Updated question".into());
        }
        let (kind, modification, stanza_type) = if is_event {
            (
                SecretEncType::EventEdit,
                ModificationType::EventEdit,
                "event",
            )
        } else {
            (SecretEncType::PollEdit, ModificationType::PollEdit, "poll")
        };
        let name = SenderKeyName::from_parts(
            &fixture.group.to_string(),
            fixture.own_sending.to_protocol_address().as_str(),
        );
        let receiver = crate::test_utils::create_test_client().await;
        let mut sender = fixture.client.signal_adapter();
        let mut receiver = receiver.signal_adapter();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let distribution =
            create_sender_key_distribution_message(&name, &mut sender.sender_key_store, &mut rng)
                .await
                .unwrap();
        process_sender_key_distribution_message(
            &name,
            &distribution,
            &mut receiver.sender_key_store,
        )
        .await
        .unwrap();
        let creator = creator.to_non_ad_string();
        let editor = fixture.own_sending.to_non_ad_string();
        let creator_jid: Jid = creator.parse().unwrap();
        let context = wacore::message_edit::MessageEditContext {
            original_msg_id: original.message_id.as_str(),
            original_sender_jid: &creator,
            editor_jid: &editor,
        };
        for raw in [false, true] {
            let index = fixture.transport.sent_count();
            let result = if raw {
                fixture
                    .client
                    .edit_message_encrypted_raw(
                        &fixture.group,
                        original.message_id.as_str(),
                        secret.as_bytes(),
                        update.clone(),
                    )
                    .await
                    .unwrap()
            } else {
                fixture
                    .client
                    .edit_message(
                        EditRequest::new(original.message_ref().unwrap(), update.clone())
                            .with_secret(&creator_jid, &secret),
                    )
                    .await
                    .unwrap()
            };
            let node = fixture.stanza(index).await;
            let enc = node.get().get_optional_child("enc").unwrap();
            let padded = group_decrypt(
                enc.content_bytes().unwrap(),
                &mut receiver.sender_key_store,
                &name,
            )
            .await
            .unwrap();
            let plaintext = wacore::messages::unpad_plaintext(padded, 2).unwrap();
            let wire = waproto::codec::message_decode(&plaintext).unwrap();
            assert_eq!(
                wire.secret_encrypted_message,
                result.message.secret_encrypted_message
            );
            let envelope = wire.secret_encrypted_message.as_option().unwrap();
            let inner = wacore::message_edit::decrypt_secret_encrypted(
                envelope.enc_payload.as_deref().unwrap(),
                envelope.enc_iv.as_deref().unwrap(),
                secret.as_bytes(),
                modification,
                &context,
            );
            assert!(
                inner.is_ok(),
                "{kind:?} must authenticate under its own use case, not {:?}: {inner:?}",
                envelope.secret_enc_type
            );
            assert_eq!(envelope.secret_enc_type, Some(kind));
            let inner = inner.unwrap();
            assert_eq!(
                inner.protocol_message.edited_message.as_option(),
                Some(&update)
            );
            assert_eq!(
                inner.protocol_message.key.id.as_deref(),
                Some(original.message_id.as_str())
            );
            assert_eq!(
                envelope.target_message_key.participant.as_deref(),
                Some(creator.as_str())
            );
            assert!(
                wacore::message_edit::decrypt_message_edit(
                    envelope.enc_payload.as_deref().unwrap(),
                    envelope.enc_iv.as_deref().unwrap(),
                    secret.as_bytes(),
                    &context,
                )
                .is_err(),
                "ordinary Message Edit is not an event/poll crypto namespace"
            );
            assert_eq!(
                node.get().attrs().optional_string("type").as_deref(),
                Some(stanza_type)
            );
            assert_eq!(
                node.get().attrs().optional_string("edit").as_deref(),
                Some("1")
            );
            assert_eq!(
                node.get().attrs().optional_string("id").as_deref(),
                Some(result.stanza_id().as_str())
            );
            if is_event {
                assert_eq!(
                    node.get()
                        .get_optional_child("meta")
                        .unwrap()
                        .attrs()
                        .optional_string("event_type")
                        .as_deref(),
                    Some("edit")
                );
            }
        }
    }
}

#[tokio::test]
async fn bot_secret_edit_uses_lid_editor_and_requires_known_namespace() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let bot: Jid = "770000099@bot".parse().unwrap();
    seed_dm_wire_namespace_state_for_peer_lid(&client, bot.clone()).await;
    let created = client
        .polls()
        .create(&bot, "Question", &["Yes".into(), "No".into()], 1)
        .await
        .unwrap();
    let creator = created.creator().to_non_ad_string();
    assert_eq!(created.creator(), &client.lid().unwrap().to_non_ad());
    let pn = client.pn().unwrap().to_non_ad_string();
    for raw in [false, true] {
        let index = transport.sent_count();
        let result = if raw {
            client
                .edit_message_encrypted_raw(
                    &bot,
                    created.send_result().message_id.as_str(),
                    created.secret().as_bytes(),
                    wa::Message::text("updated"),
                )
                .await
                .unwrap()
        } else {
            client
                .edit_message(
                    EditRequest::new(
                        created.send_result().message_ref().unwrap(),
                        wa::Message::text("updated"),
                    )
                    .with_secret(created.creator(), created.secret()),
                )
                .await
                .unwrap()
        };
        let node = crate::test_utils::decode_sent_iq(&transport, index).await;
        assert_eq!(node.get().attrs().optional_jid("to"), Some(bot.clone()));
        assert_eq!(
            node.get().attrs().optional_string("edit").as_deref(),
            Some("1")
        );
        let recipient = node
            .get()
            .get_optional_child("participants")
            .unwrap()
            .get_optional_child("to")
            .unwrap();
        assert_eq!(recipient.attrs().optional_jid("jid"), Some(bot.clone()));
        assert!(
            recipient
                .get_optional_child("enc")
                .unwrap()
                .content_bytes()
                .is_some()
        );
        let envelope = result.message.secret_encrypted_message.as_option().unwrap();
        let context = wacore::message_edit::MessageEditContext {
            original_msg_id: created.send_result().message_id.as_str(),
            original_sender_jid: &creator,
            editor_jid: &creator,
        };
        let inner = wacore::message_edit::decrypt_message_edit(
            envelope.enc_payload.as_deref().unwrap(),
            envelope.enc_iv.as_deref().unwrap(),
            created.secret().as_bytes(),
            &context,
        )
        .expect("the bot-observed LID editor must authenticate");
        assert_eq!(
            inner
                .protocol_message
                .edited_message
                .conversation
                .as_deref(),
            Some("updated")
        );
        let wrong_editor = wacore::message_edit::MessageEditContext {
            editor_jid: &pn,
            ..context
        };
        assert!(
            wacore::message_edit::decrypt_message_edit(
                envelope.enc_payload.as_deref().unwrap(),
                envelope.enc_iv.as_deref().unwrap(),
                created.secret().as_bytes(),
                &wrong_editor,
            )
            .is_err(),
            "PN is not a bot editor namespace alias"
        );
    }
    let later_editor = Jid::lid("100000000000999");
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(later_editor.clone())))
        .await;
    let result = client
        .edit_message(
            EditRequest::new(
                created.send_result().message_ref().unwrap(),
                wa::Message::text("later edit"),
            )
            .with_secret(created.creator(), created.secret()),
        )
        .await
        .unwrap();
    let envelope = result.message.secret_encrypted_message.as_option().unwrap();
    let editor = later_editor.to_non_ad_string();
    let context = wacore::message_edit::MessageEditContext {
        original_msg_id: created.send_result().message_id.as_str(),
        original_sender_jid: &creator,
        editor_jid: &editor,
    };
    let inner = wacore::message_edit::decrypt_message_edit(
        envelope.enc_payload.as_deref().unwrap(),
        envelope.enc_iv.as_deref().unwrap(),
        created.secret().as_bytes(),
        &context,
    )
    .unwrap();
    assert_eq!(
        inner
            .protocol_message
            .edited_message
            .conversation
            .as_deref(),
        Some("later edit")
    );
    let substituted_creator = wacore::message_edit::MessageEditContext {
        original_sender_jid: &editor,
        ..context
    };
    assert!(
        wacore::message_edit::decrypt_message_edit(
            envelope.enc_payload.as_deref().unwrap(),
            envelope.enc_iv.as_deref().unwrap(),
            created.secret().as_bytes(),
            &substituted_creator,
        )
        .is_err()
    );
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(None))
        .await;
    let before = transport.sent_count();
    assert!(matches!(
        client
            .edit_message(
                EditRequest::new(
                    created.send_result().message_ref().unwrap(),
                    wa::Message::text("unroutable")
                )
                .with_secret(created.creator(), created.secret()),
            )
            .await,
        Err(SendError::NotLoggedIn)
    ));
    assert!(matches!(
        client
            .edit_message_encrypted_raw(
                &bot,
                created.send_result().message_id.as_str(),
                created.secret().as_bytes(),
                wa::Message::text("unroutable"),
            )
            .await,
        Err(SendError::NotLoggedIn)
    ));
    assert_eq!(transport.sent_count(), before);
    client
        .send_message(&bot, wa::Message::text("ordinary"))
        .await
        .unwrap();
    assert!(transport.sent_count() > before);
}

#[tokio::test]
async fn canonical_send_moves_body_and_options_to_real_newsletter_wire() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let chat: Jid = "120363000000000001@newsletter".parse().unwrap();
    let message = wa::Message::text_with_context("hello", wa::ContextInfo::default());
    let request = SendRequest::new(&chat, message).with_options(
        SendOptions::default()
            .with_message_id(MessageId::new("ünïcødé_CONTENT").unwrap())
            .with_ephemeral_expiration(86400)
            .with_stanza_type_override(StanzaType::Media)
            .with_extra_stanza_nodes(vec![
                NodeBuilder::new("meta").attr("test", "request").build(),
            ]),
    );
    let allocation = Arc::as_ptr(&request.message);
    let result = client.send(request).await.unwrap();
    assert_eq!(Arc::as_ptr(&result.message), allocation);
    assert_eq!(Arc::strong_count(&result.message), 1);
    assert_eq!(result.message_id.as_str(), "ünïcødé_CONTENT");
    assert_eq!(result.stanza_id().as_str(), "ünïcødé_CONTENT");
    let node = crate::test_utils::decode_sent_iq(&transport, 0).await;
    let node = node.get();
    assert_eq!(
        node.attrs().optional_string("id").as_deref(),
        Some(result.message_id.as_str())
    );
    assert_eq!(
        node.attrs().optional_string("type").as_deref(),
        Some("media")
    );
    assert_eq!(node.attrs().optional_jid("to"), Some(chat.clone()));
    assert_eq!(
        node.get_optional_child("meta")
            .unwrap()
            .attrs()
            .optional_string("test")
            .as_deref(),
        Some("request")
    );
    let decoded = waproto::codec::message_decode(
        node.get_optional_child("plaintext")
            .unwrap()
            .content_bytes()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(&decoded, &*result.message);
    assert_eq!(
        decoded.extended_text_message.context_info.expiration,
        Some(86400)
    );
    let reference = result.newsletter_ref().unwrap();
    assert_eq!(reference.message_id(), Some(&result.message_id));
    assert!(reference.server_id().is_none());
    assert_eq!(Arc::strong_count(&result.message), 1);
}

#[tokio::test]
async fn default_request_text_reply_quote_and_forward_preserve_their_preparation() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let chat: Jid = "120363000000000001@newsletter".parse().unwrap();
    let sender = Jid::pn("15550000001");
    let source = wa::Message::text_with_context(
        "source",
        wa::ContextInfo {
            stanza_id: Some("OLD_QUOTE".into()),
            quoted_message: buffa::MessageField::some(wa::Message::text("old quote")),
            forwarding_score: Some(2),
            is_forwarded: Some(true),
            ..Default::default()
        },
    );
    let source = wa::Message {
        message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
            message_secret: Some(vec![177; 32]),
            ..Default::default()
        }),
        ..source
    };
    let info = crate::types::message::MessageInfo {
        id: "SOURCE".into(),
        source: crate::types::message::MessageSource {
            chat: chat.clone(),
            sender,
            ..Default::default()
        },
        ..Default::default()
    };
    let ctx = crate::bot::MessageContext::from_arc(Arc::new(source.clone()), &info, client.clone());
    let request = client
        .send(SendRequest::new(&chat, wa::Message::text("hello")))
        .await
        .unwrap();
    let shortcut = client.send_text(&chat, "hello").await.unwrap();
    let reply = ctx.reply("hello").await.unwrap();
    let quote = ctx.reply_quoting("hello").await.unwrap();
    let forward = client.forward_message(&chat, &source).await.unwrap();
    assert_eq!(request.message, shortcut.message);
    assert_eq!(reply.message, shortcut.message);
    assert!(reply.message.extended_text_message.is_unset());
    let context = quote
        .message
        .extended_text_message
        .context_info
        .as_option()
        .unwrap();
    assert_eq!(context.stanza_id.as_deref(), Some("SOURCE"));
    assert!(context.remote_jid.is_none());
    assert!(context.quoted_message.is_set());
    let context = forward.message.context_info().unwrap();
    assert_eq!(context.is_forwarded, Some(true));
    assert_eq!(context.forwarding_score, Some(3));
    assert!(context.quoted_message.is_unset());
    assert!(context.stanza_id.is_none());
    assert!(forward.message.message_context_info.is_unset());
    let first = wa::Message::text_with_context(
        "first",
        wa::ContextInfo {
            forwarding_score: Some(2),
            is_forwarded: Some(false),
            ..Default::default()
        },
    );
    let first_forward = client.forward_message(&chat, &first).await.unwrap();
    assert_eq!(
        first_forward
            .message
            .context_info()
            .unwrap()
            .forwarding_score,
        Some(2)
    );
    for (index, result) in [
        &request,
        &shortcut,
        &reply,
        &quote,
        &forward,
        &first_forward,
    ]
    .iter()
    .enumerate()
    {
        let node = crate::test_utils::decode_sent_iq(&transport, index).await;
        assert_eq!(
            node.get().attrs().optional_string("id").as_deref(),
            Some(result.message_id.as_str())
        );
        let decoded = waproto::codec::message_decode(
            node.get()
                .get_optional_child("plaintext")
                .unwrap()
                .content_bytes()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(&decoded, &*result.message);
    }
}

#[tokio::test]
async fn invalid_request_targets_and_raw_ids_or_secrets_send_nothing() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let chat = Jid::pn("15550000001");
    let incoming = MessageRef::new(&chat, MessageId::new("TARGET").unwrap(), None, false).unwrap();
    assert!(matches!(
        client
            .edit_message(EditRequest::new(incoming, wa::Message::text("bad")))
            .await,
        Err(SendError::MessageRef(crate::MessageRefError::NotFromMe))
    ));
    let status = Jid::status_broadcast();
    let target = MessageRef::new(&status, MessageId::new("TARGET").unwrap(), None, true).unwrap();
    assert!(matches!(
        client
            .edit_message(EditRequest::new(target, wa::Message::text("bad")))
            .await,
        Err(SendError::MessageRef(
            crate::MessageRefError::UnsupportedOrigin
        ))
    ));
    assert!(matches!(
        client
            .edit_message_raw(&chat, "", wa::Message::text("bad"), EditOptions::default())
            .await,
        Err(SendError::MessageRef(
            crate::MessageRefError::EmptyMessageId
        ))
    ));
    for length in [0, 31, 33] {
        assert!(matches!(
            client
                .edit_message_encrypted_raw(
                    &chat,
                    "TARGET",
                    &vec![177; length],
                    wa::Message::text("bad")
                )
                .await,
            Err(SendError::InvalidSecret(_))
        ));
    }
    assert_eq!(transport.sent_count(), 0);
}

#[tokio::test]
async fn edit_request_moves_content_and_distinguishes_target_from_operation() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (chat, _) = seed_dm_wire_namespace_state(&client).await;
    let target = MessageRef::new(&chat, MessageId::new("ORIGINAL").unwrap(), None, true).unwrap();
    let content = wa::Message::text("edited");
    let string_allocation = content.conversation.as_ref().unwrap().as_ptr();
    let result = client
        .edit_message(EditRequest::new(target.clone(), content))
        .await
        .unwrap();
    let protocol = result.message.protocol_message.as_option().unwrap();
    assert_eq!(protocol.key.id.as_deref(), Some("ORIGINAL"));
    assert_eq!(
        protocol
            .edited_message
            .conversation
            .as_ref()
            .unwrap()
            .as_ptr(),
        string_allocation
    );
    assert_ne!(result.message_id.as_str(), target.id().as_str());
    let node = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_eq!(
        node.get().attrs().optional_string("id").as_deref(),
        Some(result.stanza_id().as_str())
    );
    assert_eq!(
        node.get().attrs().optional_string("edit").as_deref(),
        Some("1")
    );
    assert_eq!(result.message_ref().unwrap().id(), &result.message_id);
    assert_eq!(Arc::strong_count(&result.message), 1);
}

#[tokio::test]
async fn secret_edit_keeps_original_creator_across_actual_group_addressing_modes() {
    use wacore::libsignal::protocol::{
        create_sender_key_distribution_message, group_decrypt,
        process_sender_key_distribution_message,
    };
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    // Two synthetic views of the same account/group model a later routing
    // namespace without rewriting the original creation's cryptographic input.
    let original_view = GroupSendFixture::new().await;
    let current_view = GroupSendFixture::new_lid(2).await;
    assert_eq!(original_view.group, current_view.group);
    assert_eq!(original_view.client.pn(), current_view.client.pn());
    let created = original_view
        .client
        .events()
        .create(
            &original_view.group,
            crate::EventCreationParams {
                name: "Launch".into(),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(created.creator(), &original_view.own_sending.to_non_ad());
    assert_ne!(created.creator(), &current_view.own_sending.to_non_ad());
    current_view.send_text("prime chain").await;
    let name = SenderKeyName::from_parts(
        &current_view.group.to_string(),
        current_view.own_sending.to_protocol_address().as_str(),
    );
    let receiver = crate::test_utils::create_test_client().await;
    let mut sender = current_view.client.signal_adapter();
    let mut receiver = receiver.signal_adapter();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let distribution =
        create_sender_key_distribution_message(&name, &mut sender.sender_key_store, &mut rng)
            .await
            .unwrap();
    process_sender_key_distribution_message(&name, &distribution, &mut receiver.sender_key_store)
        .await
        .unwrap();
    let result = current_view
        .client
        .edit_message(
            EditRequest::new(
                created.send_result().message_ref().unwrap(),
                wa::Message::text("updated"),
            )
            .with_secret(created.creator(), created.secret()),
        )
        .await
        .unwrap();
    let node = current_view.stanza(1).await;
    assert_eq!(
        node.get().attrs().optional_string("id").as_deref(),
        Some(result.stanza_id().as_str())
    );
    assert_eq!(
        node.get()
            .attrs()
            .optional_string("addressing_mode")
            .as_deref(),
        Some("lid")
    );
    let enc = node.get().get_optional_child("enc").unwrap();
    let padded = group_decrypt(
        enc.content_bytes().unwrap(),
        &mut receiver.sender_key_store,
        &name,
    )
    .await
    .unwrap();
    let plaintext = wacore::messages::unpad_plaintext(padded, 2).unwrap();
    let wire = waproto::codec::message_decode(&plaintext).unwrap();
    assert_eq!(
        wire.secret_encrypted_message,
        result.message.secret_encrypted_message
    );
    let envelope = wire.secret_encrypted_message.as_option().unwrap();
    let creator = created.creator().to_string();
    let editor = current_view.own_sending.to_non_ad_string();
    assert_eq!(
        envelope.target_message_key.participant.as_deref(),
        Some(creator.as_str())
    );
    let context = wacore::message_edit::MessageEditContext {
        original_msg_id: created.send_result().message_id.as_str(),
        original_sender_jid: &creator,
        editor_jid: &editor,
    };
    let inner = wacore::message_edit::decrypt_message_edit(
        envelope.enc_payload.as_deref().unwrap(),
        envelope.enc_iv.as_deref().unwrap(),
        created.secret().as_bytes(),
        &context,
    )
    .unwrap();
    assert_eq!(
        inner
            .protocol_message
            .edited_message
            .conversation
            .as_deref(),
        Some("updated")
    );
    let reconstructed = wacore::message_edit::MessageEditContext {
        original_sender_jid: &editor,
        ..context
    };
    assert!(
        wacore::message_edit::decrypt_message_edit(
            envelope.enc_payload.as_deref().unwrap(),
            envelope.enc_iv.as_deref().unwrap(),
            created.secret().as_bytes(),
            &reconstructed
        )
        .is_err()
    );
}

#[tokio::test]
async fn secret_edit_request_uses_captured_creator_and_preserves_borrowed_id_state() {
    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        let created = fixture
            .client
            .events()
            .create(
                &fixture.group,
                crate::EventCreationParams {
                    name: "Launch".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(created.creator(), &fixture.own_sending.to_non_ad());
        let original = created.send_result();
        let before = fixture
            .client
            .recent_message_bytes(&fixture.group, original.message_id.as_str())
            .await
            .unwrap();
        let result = fixture
            .client
            .edit_message(
                EditRequest::new(
                    original.message_ref().unwrap(),
                    wa::Message::text("updated"),
                )
                .with_secret(created.creator(), created.secret())
                .with_options(
                    EditOptions::default()
                        .with_stanza_id(StanzaId::from_message_id(&original.message_id)),
                ),
            )
            .await
            .unwrap();
        assert_eq!(result.message_id, original.message_id);
        let after = fixture
            .client
            .recent_message_bytes(&fixture.group, original.message_id.as_str())
            .await
            .unwrap();
        assert_eq!(
            before, after,
            "borrowed operation id must not overwrite original retry content"
        );
        let envelope = result.message.secret_encrypted_message.as_option().unwrap();
        assert_eq!(
            envelope.target_message_key.id.as_deref(),
            Some(original.message_id.as_str())
        );
        assert_eq!(
            envelope.target_message_key.participant.as_deref(),
            Some(created.creator().to_string().as_str())
        );
        let sender = fixture.own_sending.to_non_ad_string();
        let context = wacore::message_edit::MessageEditContext {
            original_msg_id: original.message_id.as_str(),
            original_sender_jid: &sender,
            editor_jid: &sender,
        };
        let inner = wacore::message_edit::decrypt_message_edit(
            envelope.enc_payload.as_deref().unwrap(),
            envelope.enc_iv.as_deref().unwrap(),
            created.secret().as_bytes(),
            &context,
        )
        .unwrap();
        assert_eq!(
            inner
                .protocol_message
                .edited_message
                .conversation
                .as_deref(),
            Some("updated")
        );
        let wrong = if fixture.own_sending.is_lid() {
            fixture.client.pn().unwrap().to_non_ad_string()
        } else {
            "100000000000999@lid".into()
        };
        let wrong_context = wacore::message_edit::MessageEditContext {
            original_sender_jid: &wrong,
            ..context
        };
        assert!(
            wacore::message_edit::decrypt_message_edit(
                envelope.enc_payload.as_deref().unwrap(),
                envelope.enc_iv.as_deref().unwrap(),
                created.secret().as_bytes(),
                &wrong_context,
            )
            .is_err(),
            "a PN/LID alias is not a cryptographic replacement"
        );
        let node = fixture.stanza(1).await;
        assert_eq!(
            node.get().attrs().optional_string("id").as_deref(),
            Some(result.stanza_id().as_str())
        );
        assert_eq!(
            node.get().attrs().optional_string("edit").as_deref(),
            Some("1")
        );
    }
}
