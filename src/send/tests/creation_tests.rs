use super::*;
use crate::{CreatedEvent, CreatedPoll, EventCreationParams, EventResponseType, MessageSecret};
use wacore::poll::{PollVoteCiphertext, compute_option_hash};

fn assert_redacted(value: &impl std::fmt::Debug, name: &str) {
    assert_eq!(format!("{value:?}"), format!("{name}([REDACTED])"));
    assert_eq!(format!("{value:#?}"), format!("{name}([REDACTED])"));
}

fn assert_poll_debug_and_borrowing(created: &CreatedPoll) {
    let result = created.send_result();
    let count = Arc::strong_count(&result.message);
    let reference = created.poll_ref().unwrap();
    assert_eq!(reference.message().id(), &result.message_id);
    assert!(reference.message().from_me());
    assert!(std::ptr::eq(reference.message().chat(), &result.to));
    assert!(std::ptr::eq(reference.creator(), created.creator()));
    assert!(std::ptr::eq(reference.secret(), created.secret()));
    assert_eq!(Arc::strong_count(&result.message), count);
    assert_redacted(created, "CreatedPoll");
    assert_redacted(&reference, "PollRef");
    assert!(
        format!("{result:?}").contains("message_secret"),
        "nested raw negative control"
    );
    assert!(!format!("{created:?}").contains("message_secret"));
    let cloned = created.clone();
    assert!(Arc::ptr_eq(&result.message, &cloned.send_result().message));
}

async fn check_poll(client: &Arc<Client>, created: &CreatedPoll, group: bool) -> SendResult {
    assert_poll_debug_and_borrowing(created);
    let reference = created.poll_ref().unwrap();
    let vote = client
        .polls()
        .vote(&reference, &["Yes".to_owned()])
        .await
        .unwrap();
    let update = vote.message.poll_update_message.as_option().unwrap();
    let key = update.poll_creation_message_key.as_option().unwrap();
    assert_eq!(
        key.id.as_deref(),
        Some(created.send_result().message_id.as_str())
    );
    assert_ne!(
        vote.message_id,
        created.send_result().message_id,
        "operation != target"
    );
    assert_eq!(key.from_me, Some(true));
    assert_eq!(key.remote_jid, Some(created.send_result().to.to_string()));
    assert_eq!(
        key.participant,
        group.then(|| created.creator().to_string())
    );
    assert!(update.metadata.is_unset());
    let enc = update.vote.as_option().unwrap();
    let cipher = PollVoteCiphertext {
        enc_payload: enc.enc_payload.as_deref().unwrap(),
        enc_iv: enc.enc_iv.as_deref().unwrap(),
    };
    let hashes = client
        .polls()
        .decrypt_vote_ref(cipher, &reference, created.creator())
        .await
        .unwrap();
    assert_eq!(hashes, vec![compute_option_hash("Yes").to_vec()]);
    assert!(
        client
            .polls()
            .decrypt_vote(
                cipher,
                &[0; 32],
                reference.message().id().as_str(),
                reference.creator(),
                reference.creator()
            )
            .await
            .is_err(),
        "wrong-secret negative control"
    );
    assert!(
        client
            .polls()
            .decrypt_vote(
                cipher,
                reference.secret().as_bytes(),
                vote.message_id.as_str(),
                reference.creator(),
                reference.creator()
            )
            .await
            .is_err(),
        "operation-ID negative control"
    );
    vote
}

async fn check_event(client: &Arc<Client>, created: &CreatedEvent, group: bool) -> SendResult {
    let result = created.send_result();
    let count = Arc::strong_count(&result.message);
    let reference = created.event_ref().unwrap();
    assert_eq!(reference.message().id(), &result.message_id);
    assert!(std::ptr::eq(reference.secret(), created.secret()));
    assert_eq!(Arc::strong_count(&result.message), count);
    assert_redacted(created, "CreatedEvent");
    assert_redacted(&reference, "EventRef");
    assert!(format!("{result:?}").contains("message_secret"));
    let response = client
        .events()
        .respond(&reference, EventResponseType::Going, Some(2))
        .await
        .unwrap();
    let enc = response
        .message
        .enc_event_response_message
        .as_option()
        .unwrap();
    let key = enc.event_creation_message_key.as_option().unwrap();
    assert_eq!(key.id.as_deref(), Some(result.message_id.as_str()));
    assert_eq!(key.from_me, Some(true));
    assert_eq!(
        key.participant,
        group.then(|| created.creator().to_string())
    );
    assert_ne!(response.message_id, result.message_id);
    let decoded = wacore::event::decrypt_event_response_with_secret(
        enc.enc_payload.as_deref().unwrap(),
        enc.enc_iv.as_deref().unwrap(),
        created.secret().as_bytes(),
        result.message_id.as_str(),
        &created.creator().to_string(),
        &created.creator().to_string(),
    )
    .unwrap();
    assert_eq!(decoded.response, Some(EventResponseType::Going));
    assert_eq!(decoded.extra_guest_count, Some(2));
    assert!(
        wacore::event::decrypt_event_response_with_secret(
            enc.enc_payload.as_deref().unwrap(),
            enc.enc_iv.as_deref().unwrap(),
            &[0; 32],
            result.message_id.as_str(),
            &created.creator().to_string(),
            &created.creator().to_string(),
        )
        .is_err()
    );
    response
}

#[tokio::test]
async fn dm_created_poll_quiz_and_event_roundtrip() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (peer, _) = seed_dm_wire_namespace_state(&client).await;
    let options = ["Yes".to_owned(), "No".to_owned()];
    let poll = client
        .polls()
        .create(&peer, "Question", &options, 2)
        .await
        .unwrap();
    assert_eq!(poll.creator(), &client.pn().unwrap().to_non_ad());
    assert!(poll.send_result().message.poll_creation_message.is_set());
    let vote = check_poll(&client, &poll, false).await;
    let quiz = client
        .polls()
        .create_quiz(&peer, "Quiz", &options, 0)
        .await
        .unwrap();
    let creation = quiz
        .send_result()
        .message
        .poll_creation_message_v3
        .as_option()
        .unwrap();
    assert_eq!(creation.poll_type, Some(wa::message::PollType::QUIZ));
    assert_eq!(
        creation
            .correct_answer
            .as_option()
            .unwrap()
            .option_name
            .as_deref(),
        Some("Yes")
    );
    let quiz_vote = check_poll(&client, &quiz, false).await;
    let event = client
        .events()
        .create(
            &peer,
            EventCreationParams::builder().name("Launch".into()).build(),
        )
        .await
        .unwrap();
    let response = check_event(&client, &event, false).await;
    let frames = crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0; 32]);
    for result in [
        poll.send_result(),
        &vote,
        quiz.send_result(),
        &quiz_vote,
        event.send_result(),
        &response,
    ] {
        assert!(
            frames.iter().any(|frame| {
                let bytes = wacore_binary::util::unpack(frame).unwrap().into_owned();
                let node = wacore_binary::OwnedNodeRef::new(bytes).unwrap();
                node.get().tag == "message"
                    && node.get().attrs().optional_string("id").as_deref()
                        == Some(result.message_id.as_str())
            }),
            "returned ID must identify the real wire envelope"
        );
    }
    let ptr = Arc::as_ptr(&poll.send_result().message);
    let (send, creator, secret) = poll.into_parts();
    assert_eq!(Arc::as_ptr(&send.message), ptr);
    assert_eq!(creator, client.pn().unwrap().to_non_ad());
    assert_eq!(
        send.message
            .message_context_info
            .as_option()
            .unwrap()
            .message_secret
            .as_deref(),
        Some(secret.as_bytes().as_slice())
    );
}

#[tokio::test]
async fn group_created_references_preserve_pn_lid_and_decrypt_real_wire() {
    use wacore::libsignal::protocol::{
        create_sender_key_distribution_message, group_decrypt,
        process_sender_key_distribution_message,
    };
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    for fixture in [
        GroupSendFixture::new().await,
        GroupSendFixture::new_lid(2).await,
    ] {
        fixture.send_text("prime chain").await;
        let receiver = crate::test_utils::create_test_client().await;
        let name = SenderKeyName::from_parts(
            &fixture.group.to_string(),
            fixture.own_sending.to_protocol_address().as_str(),
        );
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
        let poll = fixture
            .client
            .polls()
            .create_quiz(
                &fixture.group,
                "Quiz",
                &["Yes".to_owned(), "No".to_owned()],
                0,
            )
            .await
            .unwrap();
        assert_eq!(poll.creator(), &fixture.own_sending.to_non_ad());
        let vote = check_poll(&fixture.client, &poll, true).await;
        let event = fixture
            .client
            .events()
            .create(
                &fixture.group,
                EventCreationParams::builder().name("Launch".into()).build(),
            )
            .await
            .unwrap();
        assert_eq!(event.creator(), &fixture.own_sending.to_non_ad());
        let response = check_event(&fixture.client, &event, true).await;
        let raw_vote = fixture
            .client
            .polls()
            .vote_raw(
                &fixture.group,
                poll.send_result().message_id.as_str(),
                poll.creator(),
                poll.secret().as_bytes(),
                &["Yes".to_owned()],
            )
            .await
            .unwrap();
        let raw_response = fixture
            .client
            .events()
            .respond_raw(
                &fixture.group,
                event.send_result().message_id.as_str(),
                event.creator(),
                event.secret().as_bytes(),
                EventResponseType::Going,
                None,
            )
            .await
            .unwrap();
        for (key, parent) in [
            (
                raw_vote
                    .message
                    .poll_update_message
                    .poll_creation_message_key
                    .as_option()
                    .unwrap(),
                poll.send_result(),
            ),
            (
                raw_response
                    .message
                    .enc_event_response_message
                    .event_creation_message_key
                    .as_option()
                    .unwrap(),
                event.send_result(),
            ),
        ] {
            assert_eq!(key.from_me, Some(true));
            assert_eq!(key.id.as_deref(), Some(parent.message_id.as_str()));
            assert_eq!(
                key.participant,
                Some(fixture.own_sending.to_non_ad_string())
            );
        }
        for (i, result) in [
            poll.send_result(),
            &vote,
            event.send_result(),
            &response,
            &raw_vote,
            &raw_response,
        ]
        .iter()
        .enumerate()
        {
            let stanza = fixture.stanza(i + 1).await;
            assert_eq!(
                stanza.get().attrs().optional_string("id").as_deref(),
                Some(result.message_id.as_str())
            );
            let enc = stanza.get().get_optional_child("enc").unwrap();
            let padded = group_decrypt(
                enc.content_bytes().unwrap(),
                &mut receiver.sender_key_store,
                &name,
            )
            .await
            .unwrap();
            let bytes = wacore::messages::unpad_plaintext(padded, 2).unwrap();
            let decoded = waproto::codec::message_decode(&bytes).unwrap();
            if i == 0 || i == 2 {
                let expected = if i == 0 {
                    poll.secret()
                } else {
                    event.secret()
                };
                assert_eq!(
                    decoded
                        .message_context_info
                        .as_option()
                        .unwrap()
                        .message_secret
                        .as_deref(),
                    Some(expected.as_bytes().as_slice())
                );
            } else if i == 1 || i == 4 {
                assert_eq!(
                    decoded.poll_update_message,
                    result.message.poll_update_message
                );
            } else {
                assert_eq!(
                    decoded.enc_event_response_message,
                    result.message.enc_event_response_message
                );
            }
        }
    }
}

#[tokio::test]
async fn poll_vote_reference_sender_is_not_the_crypto_creator() {
    let fixture = GroupSendFixture::new_lid(2).await;
    let created = fixture
        .client
        .polls()
        .create(&fixture.group, "Question", &["Yes".into(), "No".into()], 1)
        .await
        .unwrap();
    let sender = fixture.client.pn().unwrap().to_non_ad();
    assert_ne!(&sender, created.creator());
    let message = crate::MessageRef::new(
        &fixture.group,
        created.send_result().message_id.clone(),
        Some(&sender),
        true,
    )
    .unwrap();
    let expected_key = message.to_raw_key();
    let target = crate::PollRef::new(message, created.creator(), created.secret()).unwrap();
    let result = fixture
        .client
        .polls()
        .vote(&target, &["Yes".into()])
        .await
        .unwrap();
    let update = result.message.poll_update_message.as_option().unwrap();
    let enc = update.vote.as_option().unwrap();
    let ciphertext = PollVoteCiphertext {
        enc_payload: enc.enc_payload.as_deref().unwrap(),
        enc_iv: enc.enc_iv.as_deref().unwrap(),
    };
    let hashes = fixture
        .client
        .polls()
        .decrypt_vote_ref(ciphertext, &target, created.creator())
        .await
        .unwrap();
    assert_eq!(hashes, vec![compute_option_hash("Yes").to_vec()]);
    assert!(
        wacore::poll::decrypt_poll_vote_with_secret(
            ciphertext,
            created.secret().as_bytes(),
            created.send_result().message_id.as_str(),
            &sender.to_non_ad_string(),
            &created.creator().to_non_ad_string(),
        )
        .is_err(),
        "stanza sender must not replace captured creator in crypto"
    );
    assert_eq!(
        update.poll_creation_message_key.participant, expected_key.participant,
        "the A06 addressing reference supplies the group participant"
    );
}

#[tokio::test]
async fn bot_creation_requires_known_crypto_namespace_not_a_pn_fallback() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let bot: Jid = "770000099@bot".parse().unwrap();
    seed_dm_wire_namespace_state_for_peer_lid(&client, bot.clone()).await;
    let own_lid = client.lid().unwrap().to_non_ad();
    let options = ["Yes".into(), "No".into()];
    let created = client
        .polls()
        .create(&bot, "Question", &options, 1)
        .await
        .unwrap();
    assert_eq!(created.creator(), &own_lid);
    assert!(
        transport.sent_count() > 0,
        "known-namespace creation reaches wire"
    );
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(None))
        .await;
    let before = transport.sent_count();
    assert!(matches!(
        client.polls().create(&bot, "Question", &options, 1).await,
        Err(crate::PollError::Send(SendError::NotLoggedIn))
    ));
    assert_eq!(transport.sent_count(), before);
    client
        .send_message(&bot, wa::Message::text("ordinary"))
        .await
        .unwrap();
    assert!(
        transport.sent_count() > before,
        "ordinary send does not claim creation metadata"
    );
    let pn = client.pn().unwrap().to_non_ad_string();
    let hashes = [compute_option_hash("Yes").to_vec()];
    let (payload, iv) = wacore::poll::encrypt_poll_vote_with_secret(
        &hashes,
        created.secret().as_bytes(),
        created.send_result().message_id.as_str(),
        &pn,
        &pn,
    )
    .unwrap();
    assert!(
        wacore::poll::decrypt_poll_vote_with_secret(
            PollVoteCiphertext {
                enc_payload: &payload,
                enc_iv: &iv
            },
            created.secret().as_bytes(),
            created.send_result().message_id.as_str(),
            &own_lid.to_non_ad_string(),
            &pn,
        )
        .is_err(),
        "blind PN fallback changes the captured original crypto namespace"
    );
}

#[tokio::test]
async fn malformed_raw_inputs_fail_before_wire_and_debug_has_negative_controls() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let peer = Jid::pn("15550000001");
    for length in [0, 31, 33] {
        assert!(matches!(
            client
                .polls()
                .vote_raw(&peer, "P", &peer, &vec![177; length], &[])
                .await,
            Err(crate::PollError::InvalidSecret(_))
        ));
        let error = client
            .events()
            .respond_raw(
                &peer,
                "E",
                &peer,
                &vec![177; length],
                EventResponseType::Going,
                None,
            )
            .await
            .unwrap_err();
        let SendError::InvalidSecret(length_error) = &error else {
            panic!("expected typed length error, got {error:?}");
        };
        assert_eq!(length_error.actual, length);
        assert!(!format!("{error:#?}").contains("177"));
    }
    assert!(
        client
            .events()
            .create(
                &peer,
                EventCreationParams::builder().name(String::new()).build()
            )
            .await
            .is_err()
    );
    assert!(
        client
            .polls()
            .create_quiz(&peer, "Q", &["A".into(), "B".into()], 2)
            .await
            .is_err()
    );
    assert_eq!(transport.sent_count(), 0);
    let secret = MessageSecret::from_bytes([177; 32]);
    let raw = SendResult {
        message_id: crate::MessageId::new("SECRET-CONTROL").unwrap(),
        to: peer.clone(),
        recipient_fanout: None,
        message: Arc::new(wa::Message {
            message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
                message_secret: Some(vec![177; 32]),
                ..Default::default()
            }),
            ..Default::default()
        }),
    };
    assert!(format!("{raw:?}").contains("177"));
    assert!(!format!("{secret:?}").contains("177"));
    let poll = CreatedPoll::new(raw.clone(), peer.clone(), secret.clone());
    let event = CreatedEvent::new(raw, peer, secret);
    assert_redacted(&poll, "CreatedPoll");
    assert_redacted(&event, "CreatedEvent");
}
