use super::{make_group_message_info, make_placeholder_response, make_web_msg};
use crate::client::Client;
use buffa::Message as _;
use std::sync::Arc;
use wacore::types::events::{ChannelEventHandler, InboundMessage};
use wacore::types::jid::JidExt as _;
use wacore::types::message::{ChatMessageId, MessageInfo, SenderMessageId};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

async fn client_with_session() -> (Arc<Client>, Arc<MessageInfo>) {
    let (client, _) = crate::test_utils::create_iq_test_client().await;
    use crate::store::commands::DeviceCommand;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(
            "12025550100:2@s.whatsapp.net".parse().unwrap(),
        )))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetAccount(Some(
            wa::ADVSignedDeviceIdentity::default(),
        )))
        .await;
    crate::test_utils::seed_peer_session(&client, &"12025550100@s.whatsapp.net".parse().unwrap())
        .await;
    let info = make_group_message_info(
        "120363000000000001@g.us",
        "12025550101@s.whatsapp.net",
        "RECOVER_CONTENT",
    );
    (client, info)
}

fn pending_key(info: &MessageInfo) -> ChatMessageId {
    ChatMessageId::new(info.source.chat.clone(), info.id.clone())
}

fn gate_key(info: &MessageInfo) -> SenderMessageId {
    SenderMessageId::new(
        info.source.chat.clone(),
        info.id.clone(),
        info.source.sender.clone(),
    )
}

async fn send_automatic(client: &Arc<Client>, info: &Arc<MessageInfo>) -> String {
    client
        .send_pdo_placeholder_resend_request(info)
        .await
        .unwrap();
    client
        .pdo_requested
        .get(&gate_key(info))
        .await
        .unwrap()
        .request_id
        .clone()
}

type PlaceholderResponse = wa::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse;

fn top_level_response(info: &MessageInfo) -> PlaceholderResponse {
    let mut response =
        make_placeholder_response(&info.source.chat.to_string(), false, &info.id, None);
    let mut web =
        waproto::codec::web_message_info_decode(response.web_message_info_bytes.as_ref().unwrap())
            .unwrap();
    web.participant = Some(info.source.sender.to_string());
    response.web_message_info_bytes = Some(web.encode_to_vec());
    response
}

fn drain(rx: &async_channel::Receiver<Arc<wacore::types::events::Event>>) -> Vec<InboundMessage> {
    let mut messages = Vec::new();
    while let Ok(event) = rx.try_recv() {
        messages.extend(event.messages().cloned());
    }
    messages
}

#[tokio::test]
async fn top_level_participant_preserves_current_pending_author() {
    let (client, mut info) = client_with_session().await;
    Arc::make_mut(&mut info).push_name = "Pending author".into();
    let id = send_automatic(&client, &info).await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    client
        .handle_placeholder_resend_response(&top_level_response(&info), &id)
        .await;
    let delivered = drain(&rx);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].info.source.sender, info.source.sender);
    assert_eq!(delivered[0].info.push_name, info.push_name);
    assert_eq!(
        delivered[0].info.unavailable_request_id.as_deref(),
        Some(id.as_str())
    );
    assert!(
        client
            .pdo_pending_requests
            .get(&pending_key(&info))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn top_level_participant_does_not_bypass_stale_owner() {
    let (client, info) = client_with_session().await;
    let old = send_automatic(&client, &info).await;
    client
        .pdo_pending_requests
        .remove(&pending_key(&info))
        .await;
    let latest = client
        .retry_pdo_placeholder_resend_request(&info)
        .await
        .unwrap()
        .unwrap();
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    let response = top_level_response(&info);
    client
        .handle_placeholder_resend_response(&response, &old)
        .await;
    assert!(
        drain(&rx).is_empty(),
        "obsolete content must not be published"
    );
    assert_eq!(
        client
            .pdo_pending_requests
            .get(&pending_key(&info))
            .await
            .unwrap()
            .1
            .request_id,
        latest
    );
    client
        .handle_placeholder_resend_response(&response, &latest)
        .await;
    let delivered = drain(&rx);
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].info.source.sender, info.source.sender);
}

async fn check_short_circuited_retry(evict_gate: bool, evict_pending: bool) {
    let (client, info) = client_with_session().await;
    let peer: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
    let session = client
        .session_lock_for(peer.to_protocol_address().as_str())
        .await;
    let lock = session.lock().await;
    let automatic = tokio::spawn({
        let client = client.clone();
        let info = info.clone();
        async move { client.send_pdo_placeholder_resend_request(&info).await }
    });
    let owner = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some((_, owner)) = client.pdo_pending_requests.get(&pending_key(&info)).await {
                break owner;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("automatic request must reserve pending before waiting for its session");
    assert_eq!(
        owner.outcome.load(std::sync::atomic::Ordering::Acquire),
        super::super::PDO_IN_FLIGHT
    );
    if evict_gate {
        client.pdo_requested.remove(&gate_key(&info)).await;
    }
    if evict_pending {
        client.pdo_pending_requests.remove(&pending_key(&info)).await;
    }
    let retry = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::select! {
            result = client.retry_pdo_placeholder_resend_request(&info) => result.unwrap(),
            replacement = async {
                loop {
                    if let Some((_, memo)) = client.pdo_pending_requests.get(&pending_key(&info)).await {
                        break memo.request_id.clone();
                    }
                    tokio::task::yield_now().await;
                }
            }, if evict_pending => panic!(
                "retry replaced the retained unsent reservation with {replacement}"
            ),
        }
    })
    .await
    .expect("retained unsent reservation must short-circuit before the held session lock");
    assert_eq!(retry, None);
    assert_eq!(
        client
            .pdo_requested
            .get(&gate_key(&info))
            .await
            .as_ref()
            .map(|memo| memo.request_id.as_str()),
        Some(owner.request_id.as_str()),
        "a short-circuited retry must retain the automatic reservation"
    );
    drop(lock);
    automatic.await.unwrap().unwrap();
    assert_eq!(
        client
            .pdo_requested
            .get(&gate_key(&info))
            .await
            .unwrap()
            .request_id,
        owner.request_id
    );
    client
        .handle_placeholder_resend_response(&top_level_response(&info), &owner.request_id)
        .await;
    assert!(
        client
            .pdo_pending_requests
            .get(&pending_key(&info))
            .await
            .is_none()
    );
    client
        .send_pdo_placeholder_resend_request(&info)
        .await
        .unwrap();
    assert!(
        client
            .pdo_pending_requests
            .get(&pending_key(&info))
            .await
            .is_none(),
        "redelivery must not send a second automatic request"
    );
    assert_eq!(
        client
            .pdo_requested
            .get(&gate_key(&info))
            .await
            .unwrap()
            .request_id,
        owner.request_id
    );
}

#[tokio::test]
async fn short_circuited_retry_preserves_inflight_automatic_gate() {
    check_short_circuited_retry(false, false).await;
}

#[tokio::test]
async fn short_circuited_retry_restores_evicted_inflight_automatic_gate() {
    check_short_circuited_retry(true, false).await;
}

#[tokio::test]
async fn retained_inflight_gate_short_circuits_retry_after_pending_eviction() {
    check_short_circuited_retry(false, true).await;
}

#[tokio::test]
async fn previous_response_while_retry_waits_for_session_is_delivered() {
    let (client, info) = client_with_session().await;
    let old = send_automatic(&client, &info).await;
    client
        .pdo_pending_requests
        .remove(&pending_key(&info))
        .await;
    let peer: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
    let session = client
        .session_lock_for(peer.to_protocol_address().as_str())
        .await;
    let lock = session.lock().await;
    let mut retry = Box::pin(client.retry_pdo_placeholder_resend_request(&info));
    tokio::select! {
        result = &mut retry => panic!("session-wait retry unexpectedly finished: {result:?}"),
        result = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while client.pdo_pending_requests.get(&pending_key(&info)).await.is_none() {
                tokio::task::yield_now().await;
            }
        }) => { result.unwrap(); }
    }
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    let response = make_placeholder_response(
        &info.source.chat.to_string(),
        false,
        &info.id,
        Some(&info.source.sender.to_string()),
    );
    client
        .handle_placeholder_resend_response(&response, &old)
        .await;
    let delivered = drain(&rx);
    drop(retry);
    drop(lock);
    assert_eq!(delivered.len(), 1, "reserving a retry did not send it");
    assert_eq!(
        delivered[0].info.unavailable_request_id.as_deref(),
        Some(old.as_str())
    );
    assert!(drain(&rx).is_empty());
}

#[tokio::test]
async fn cancelled_caller_does_not_undo_a_transport_accepted_retry() {
    struct RetainedWrite {
        capture: Arc<crate::transport::mock::CapturingMockTransport>,
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }
    #[async_trait::async_trait]
    impl crate::transport::Transport for RetainedWrite {
        async fn send(&self, bytes: bytes::Bytes) -> anyhow::Result<()> {
            crate::transport::Transport::send(self.capture.as_ref(), bytes).await?;
            self.entered.send(()).await.unwrap();
            self.release.recv().await.unwrap();
            Ok(())
        }
        async fn disconnect(&self) {}
    }
    let (client, info) = client_with_session().await;
    let old = client
        .retry_pdo_placeholder_resend_request(&info)
        .await
        .unwrap()
        .unwrap();
    client
        .pdo_pending_requests
        .remove(&pending_key(&info))
        .await;
    let capture = Arc::new(crate::transport::mock::CapturingMockTransport::new());
    let (entered, arrived) = async_channel::bounded(1);
    let (release, wait) = async_channel::bounded(1);
    let socket = Arc::new(crate::socket::NoiseSocket::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        Arc::new(RetainedWrite {
            capture: capture.clone(),
            entered,
            release: wait,
        }),
        wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
        wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
    ));
    *client.noise_socket.lock().unwrap() = Some(socket.clone());
    let mut retry = Box::pin(client.retry_pdo_placeholder_resend_request(&info));
    tokio::select! {
        result = &mut retry => panic!("retained write unexpectedly finished: {result:?}"),
        result = tokio::time::timeout(std::time::Duration::from_secs(5), arrived.recv()) => {
            result.unwrap().unwrap();
        }
    }
    let node = crate::test_utils::decode_sent_iq(&capture, 0).await;
    let latest = node
        .get()
        .attrs()
        .optional_string("id")
        .unwrap()
        .to_string();
    assert_ne!(latest, old);
    drop(retry);
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    let response = make_placeholder_response(
        &info.source.chat.to_string(),
        false,
        &info.id,
        Some(&info.source.sender.to_string()),
    );
    // The write remains inside the socket task, and a response can arrive before
    // its completion notification, just as it can over a real duplex socket.
    client
        .handle_placeholder_resend_response(&response, &latest)
        .await;
    let delivered = drain(&rx);
    release.send(()).await.unwrap();
    // A second actual socket job is a deterministic barrier past the retained
    // write. It also proves dropping the first caller did not abort the writer.
    let mut barrier = Box::pin(socket.encrypt_and_send(bytes::Bytes::from_static(b"barrier")));
    assert!(futures::poll!(&mut barrier).is_pending());
    tokio::time::timeout(std::time::Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    release.send(()).await.unwrap();
    barrier.await.unwrap();
    assert_eq!(delivered.len(), 1);
    assert_eq!(
        delivered[0].info.unavailable_request_id.as_deref(),
        Some(latest.as_str())
    );
    let mut stale = response;
    let mut web =
        waproto::codec::web_message_info_decode(stale.web_message_info_bytes.as_ref().unwrap())
            .unwrap();
    web.message.as_option_mut().unwrap().conversation = Some("Obsolete distinct content".into());
    stale.web_message_info_bytes = Some(web.encode_to_vec());
    client
        .handle_placeholder_resend_response(&stale, &old)
        .await;
    assert!(drain(&rx).is_empty());
}

#[tokio::test]
async fn stale_dm_response_matches_retained_alias_in_both_directions() {
    for chat_is_lid in [false, true] {
        let (client, group_info) = client_with_session().await;
        let pn: Jid = "12025550101@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "777000000000101@lid".parse().unwrap();
        let (chat, alias) = if chat_is_lid {
            (lid.clone(), pn.clone())
        } else {
            (pn.clone(), lid.clone())
        };
        let mut info = (*group_info).clone();
        info.source.chat = chat.clone();
        info.source.sender = chat;
        info.source.sender_alt = Some(alias.clone());
        info.source.is_group = false;
        let info = Arc::new(info);
        let old = send_automatic(&client, &info).await;
        let cached_chat = if chat_is_lid {
            pn
        } else {
            info.source.chat.clone()
        };
        let key = ChatMessageId::new(cached_chat, info.id.clone());
        client.pdo_pending_requests.remove(&key).await;
        let latest = client
            .retry_pdo_placeholder_resend_request(&info)
            .await
            .unwrap()
            .unwrap();
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let response = make_placeholder_response(&alias.to_string(), false, &info.id, None);
        client
            .handle_placeholder_resend_response(&response, &old)
            .await;
        assert!(
            drain(&rx).is_empty(),
            "retained alias must suppress obsolete content"
        );
        assert_eq!(
            client
                .pdo_pending_requests
                .get(&key)
                .await
                .unwrap()
                .1
                .request_id,
            latest
        );
        client
            .handle_placeholder_resend_response(&response, &latest)
            .await;
        assert_eq!(drain(&rx).len(), 1);
    }
}

#[tokio::test]
async fn participant_precedence_keeps_key_identity_separate_from_author() {
    let (client, _) = client_with_session().await;
    for chat in ["120363000000000001@g.us", "status@broadcast"] {
        let mut web = make_web_msg(
            chat,
            false,
            "PARTICIPANT_PRECEDENCE",
            Some("12025550101@s.whatsapp.net"),
        );
        web.participant = Some("777000000000101@lid".into());
        let info = client
            .message_info_from_web_message_info(&web)
            .await
            .unwrap();
        assert_eq!(info.source.sender.to_string(), "777000000000101@lid");
        assert_eq!(
            web.key.as_option().unwrap().participant.as_deref(),
            Some("12025550101@s.whatsapp.net")
        );
        web.key.as_option_mut().unwrap().participant = None;
        web.key.as_option_mut().unwrap().from_me = Some(true);
        web.original_self_author_user_jid_string = Some("12025550102@s.whatsapp.net".into());
        web.participant = None;
        let info = client
            .message_info_from_web_message_info(&web)
            .await
            .unwrap();
        assert_eq!(info.source.sender.to_string(), "12025550102@s.whatsapp.net");
    }
}

#[tokio::test]
async fn unpolled_retry_does_not_replace_the_sent_owner() {
    let (client, info) = client_with_session().await;
    let old = send_automatic(&client, &info).await;
    let retry = client.retry_pdo_placeholder_resend_request(&info);
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    let response = make_placeholder_response(
        &info.source.chat.to_string(),
        false,
        &info.id,
        Some(&info.source.sender.to_string()),
    );
    client
        .handle_placeholder_resend_response(&response, &old)
        .await;
    drop(retry);
    let delivered = drain(&rx);
    assert_eq!(delivered.len(), 1);
    assert_eq!(
        delivered[0].info.unavailable_request_id.as_deref(),
        Some(old.as_str())
    );
    assert_eq!(
        client
            .pdo_requested
            .get(&gate_key(&info))
            .await
            .unwrap()
            .request_id,
        old
    );
}

#[tokio::test]
async fn missing_transport_releases_retry_but_keeps_the_sent_owner() {
    let (client, info) = client_with_session().await;
    let old = send_automatic(&client, &info).await;
    client
        .pdo_pending_requests
        .remove(&pending_key(&info))
        .await;
    *client.noise_socket.lock().unwrap() = None;
    assert!(
        client
            .retry_pdo_placeholder_resend_request(&info)
            .await
            .is_err()
    );
    assert!(
        client
            .pdo_pending_requests
            .get(&pending_key(&info))
            .await
            .is_none()
    );
    assert_eq!(
        client
            .pdo_requested
            .get(&gate_key(&info))
            .await
            .unwrap()
            .request_id,
        old
    );
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.subscribe_handler(handler).detach();
    let response = make_placeholder_response(
        &info.source.chat.to_string(),
        false,
        &info.id,
        Some(&info.source.sender.to_string()),
    );
    client
        .handle_placeholder_resend_response(&response, "UNSENT_RETRY")
        .await;
    assert!(drain(&rx).is_empty());
    client
        .handle_placeholder_resend_response(&response, &old)
        .await;
    assert_eq!(drain(&rx).len(), 1);
}

#[tokio::test]
async fn gate_enumeration_requires_matching_id_generation_and_source() {
    let (client, info) = client_with_session().await;
    client
        .pdo_explicit_published
        .store(true, std::sync::atomic::Ordering::Release);
    client
        .pdo_requested
        .insert(
            gate_key(&info),
            super::super::PdoRequestMemo::sent_for_test(&info, "UNVERSIONED".into(), true),
        )
        .await;
    let mut unrelated = info.clone();
    Arc::make_mut(&mut unrelated).id = "OTHER_MESSAGE".into();
    let unrelated_owner =
        super::super::PdoRequestMemo::new(&unrelated, "UNRELATED".into(), true, None)
            .winning_owner(None);
    unrelated_owner
        .outcome
        .store(super::super::PDO_SENT, std::sync::atomic::Ordering::Release);
    client
        .pdo_requested
        .insert(gate_key(&unrelated), unrelated_owner)
        .await;
    let key = pending_key(&info);
    let participant = info.source.sender.to_string();
    assert!(
        !client
            .obsolete_pdo_response(&key, None, false, Some(&participant), "OLD", None)
            .await
    );
    let matching =
        super::super::PdoRequestMemo::new(&info, "LIVE".into(), true, None).winning_owner(None);
    matching
        .outcome
        .store(super::super::PDO_SENT, std::sync::atomic::Ordering::Release);
    client.pdo_requested.insert(gate_key(&info), matching).await;
    assert!(
        client
            .obsolete_pdo_response(&key, None, false, Some(&participant), "OLD", None)
            .await
    );
    assert!(
        !client
            .obsolete_pdo_response(&key, None, false, Some(&participant), "LIVE", None)
            .await
    );
    assert!(
        !client
            .obsolete_pdo_response(&key, None, true, Some(&participant), "OLD", None)
            .await
    );
    assert!(
        !client
            .obsolete_pdo_response(
                &key,
                None,
                false,
                Some("12025550109@s.whatsapp.net"),
                "OLD",
                None
            )
            .await
    );
}
