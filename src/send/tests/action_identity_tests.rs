use super::*;
use crate::{MessageId, MessageRef, MessageRefError};
use wacore::types::message::{MessageInfo, MessageSource};

async fn mark_cag(fixture: &GroupSendFixture) {
    let mut routing = (*fixture
        .client
        .get_group_cache()
        .get(&fixture.group)
        .await
        .unwrap())
    .clone();
    routing.is_community_announce = Some(true);
    fixture
        .client
        .get_group_cache()
        .insert(fixture.group.clone(), Arc::new(routing))
        .await;
}

#[tokio::test]
async fn typed_cag_reactions_and_comments_keep_parent_author_and_comment_secret() {
    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        mark_cag(&fixture).await;
        let modifier = fixture.client.lid().unwrap().to_non_ad_string();
        for from_me in [false, true] {
            let author = if from_me {
                &fixture.own_sending
            } else {
                &fixture.member
            };
            let id = if from_me { "OWN_POST" } else { "RECEIVED_POST" };
            let secret = [177; 32];
            fixture
                .client
                .persistence_manager
                .backend()
                .put_msg_secrets(vec![wacore::store::traits::MsgSecretEntry::new(
                    &fixture.group,
                    author,
                    id,
                    secret,
                    0,
                    1_700_000_000,
                )])
                .await
                .unwrap();
            let target = MessageRef::new(
                &fixture.group,
                MessageId::new(id).unwrap(),
                (!from_me).then_some(author),
                from_me,
            )
            .unwrap();
            let reaction = fixture.client.send_reaction(&target, "👍").await.unwrap();
            assert!(
                !reaction.message.reaction_message.is_set(),
                "CAG must never get plaintext reactions"
            );
            let envelope = reaction.message.enc_reaction_message.as_option().unwrap();
            let author_string = author.to_non_ad_string();
            assert_eq!(
                envelope.target_message_key.participant.as_deref(),
                Some(author_string.as_str())
            );
            assert_eq!(envelope.target_message_key.from_me, Some(from_me));
            assert_eq!(envelope.target_message_key.id.as_deref(), Some(id));
            let inner = wacore::reaction::decrypt_reaction_with_secret(
                envelope.enc_payload.as_deref().unwrap(),
                envelope.enc_iv.as_deref().unwrap(),
                &secret,
                id,
                &author_string,
                &modifier,
            )
            .unwrap();
            assert_eq!(inner.text.as_deref(), Some("👍"));
            assert!(
                wacore::reaction::decrypt_reaction_with_secret(
                    envelope.enc_payload.as_deref().unwrap(),
                    envelope.enc_iv.as_deref().unwrap(),
                    &secret,
                    id,
                    "999999999999999@lid",
                    &modifier,
                )
                .is_err()
            );

            let raw_key = fixture.client.message_ref_addon_key(&target).await.unwrap();
            let comments = [
                fixture
                    .client
                    .comments()
                    .send_text(&target, "comment")
                    .await
                    .unwrap(),
                fixture
                    .client
                    .comments()
                    .send_text_raw(&fixture.group, raw_key, "comment")
                    .await
                    .unwrap(),
                fixture
                    .client
                    .comments()
                    .send_message(&target, wa::Message::text("body"))
                    .await
                    .unwrap(),
            ];
            for (index, result) in comments.iter().enumerate() {
                let envelope = result.message.enc_comment_message.as_option().unwrap();
                assert_eq!(
                    envelope.target_message_key.participant.as_deref(),
                    Some(author_string.as_str())
                );
                assert_eq!(envelope.target_message_key.from_me, Some(from_me));
                assert_eq!(envelope.target_message_key.id.as_deref(), Some(id));
                assert_ne!(result.message_id.as_str(), id);
                let body = wacore::comment::decrypt_comment_with_secret(
                    envelope.enc_payload.as_deref().unwrap(),
                    envelope.enc_iv.as_deref().unwrap(),
                    &secret,
                    id,
                    &author_string,
                    &modifier,
                )
                .unwrap();
                if index < 2 {
                    assert_eq!(body.extended_text_message.text.as_deref(), Some("comment"));
                } else {
                    assert_eq!(body.conversation.as_deref(), Some("body"));
                }
                let minted = result
                    .message
                    .message_context_info
                    .message_secret
                    .as_deref()
                    .unwrap();
                assert_eq!(minted.len(), 32);
                // The drain worker may already have persisted earlier comments;
                // exercise the buffered-first reader, not transient map occupancy.
                let (comment_author, readable) = fixture
                    .client
                    .resolve_outgoing_addon_parent(
                        &fixture.group,
                        &wa::MessageKey {
                            remote_jid: Some(fixture.group.to_string()),
                            id: Some(result.message_id.to_string()),
                            participant: Some(modifier.clone()),
                            from_me: Some(true),
                        },
                    )
                    .await
                    .unwrap();
                assert_eq!(comment_author.to_non_ad_string(), modifier);
                assert_eq!(readable.as_slice(), minted);
                fixture.client.msg_secret_buffer.wait_flushed().await;
                let persisted = fixture
                    .client
                    .persistence_manager
                    .backend()
                    .get_stored_msg_secret(
                        &fixture.group.to_non_ad_string(),
                        &modifier,
                        result.message_id.as_str(),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(persisted.secret.as_bytes().as_slice(), minted);
            }
        }
    }
}

#[tokio::test]
async fn typed_comment_invalid_origin_and_missing_secret_send_nothing() {
    let fixture = GroupSendFixture::new_lid(2).await;
    mark_cag(&fixture).await;
    let target = MessageRef::new(
        &fixture.group,
        MessageId::new("NOT_CAPTURED").unwrap(),
        Some(&fixture.member),
        false,
    )
    .unwrap();
    assert!(matches!(
        fixture.client.send_reaction(&target, "👍").await,
        Err(SendError::InvalidRequest(_))
    ));
    assert!(matches!(
        fixture
            .client
            .comments()
            .send_text(&target, "comment")
            .await,
        Err(SendError::InvalidRequest(_))
    ));
    for chat in [
        Jid::pn("15550000001"),
        Jid::status_broadcast(),
        "123456789@broadcast".parse().unwrap(),
    ] {
        let target =
            MessageRef::new(&chat, MessageId::new("INVALID_ORIGIN").unwrap(), None, true).unwrap();
        for result in [
            fixture
                .client
                .comments()
                .send_text(&target, "comment")
                .await,
            fixture
                .client
                .comments()
                .send_message(&target, wa::Message::text("body"))
                .await,
        ] {
            assert!(matches!(
                result,
                Err(SendError::MessageRef(MessageRefError::UnsupportedOrigin))
            ));
        }
    }
    assert_eq!(fixture.transport.sent_count(), 0);
}

#[tokio::test]
async fn context_revoke_uses_reference_scope_and_event_name_rejects_before_send() {
    let fixture = GroupSendFixture::new_lid(2).await;
    let info = MessageInfo {
        id: "RECEIVED_POST".into(),
        source: MessageSource {
            chat: fixture.group.clone(),
            sender: fixture.member.clone(),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let ctx = crate::bot::MessageContext::builder()
        .message(Arc::new(wa::Message::text("post")))
        .info(info)
        .client(fixture.client.clone())
        .ephemeral_expiration(86400)
        .build();
    let result = ctx
        .revoke_message(&ctx.message_ref().unwrap())
        .await
        .unwrap();
    let key = &result.message.protocol_message.key;
    assert_eq!(key.from_me, Some(false));
    assert_eq!(
        key.participant.as_deref(),
        Some(fixture.member.to_string().as_str())
    );
    assert_eq!(key.id.as_deref(), Some("RECEIVED_POST"));
    let before = fixture.transport.sent_count();
    let params = crate::EventCreationParams::builder()
        .name("  ".into())
        .build();
    assert!(matches!(
        fixture.client.events().create(&fixture.group, params).await,
        Err(SendError::InvalidRequest(_))
    ));
    assert_eq!(fixture.transport.sent_count(), before);
}
