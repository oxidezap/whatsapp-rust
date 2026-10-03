//! Offline public invitation acceptance: real Noise sends, retained wire frames, fake media.

use super::*;
use crate::transport::mock::CapturingMockTransport;
use crate::voip::EncodedAudioFrame;
use bytes::Bytes;
use wacore::types::group_call::{GroupCallRelay, GroupCallRelayEndpoint};
use wacore::voip_control::fake_backend::FakeMediaBackend;
use wacore::voip_control::group::GroupStateApply;

async fn fixture(
    relay: bool,
    media: &str,
) -> (Arc<Client>, Arc<CapturingMockTransport>, IncomingCall, u64) {
    let client =
        crate::test_utils::create_test_client_with_voip_backend(Arc::new(FakeMediaBackend::new()))
            .await;
    client
        .persistence_manager()
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            Jid::new("100001", Server::Lid).with_device(1),
        )))
        .await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_transport(&client, transport.clone());
    client.set_connected_for_test(true);
    let creator = Jid::new("200002", Server::Lid).with_device(1);
    let mut incoming = IncomingCall::new_for_test(
        creator.clone(),
        "INVITE-STANZA".into(),
        wacore::time::from_secs(1_700_000_000).expect("time"),
        CallAction::Offer {
            call_id: "INVITED-CALL".into(),
            call_creator: creator.clone(),
            caller_pn: None,
            caller_country_code: None,
            device_class: None,
            joinable: true,
            is_video: false,
            audio: Vec::new(),
            group_jid: None,
        },
    );
    let mut group = GroupCallUpdate::builder()
        .call_id(incoming.action.call_id().to_string())
        .call_creator(creator.clone())
        .transaction_id(15)
        .media(media.to_string())
        .connected_limit(32)
        .joinable(true)
        .av_upgradable(true)
        .rekey_requested(false)
        .participants(Vec::new())
        .build();
    if relay {
        group.relay = Some(sample_relay());
    }
    incoming.group = Some(Box::new(group.clone()));
    let mut session = wacore::voip_control::CallSession::new_incoming(
        incoming.action.call_id(),
        creator.clone(),
        creator,
    );
    session.group = Some(group);
    let generation = client
        .call_registry()
        .insert_ringing_group_if_inactive(session)
        .expect("snapshot")
        .expect("ringing generation");
    incoming.set_ringing_generation(generation);
    (client, transport, incoming, generation)
}

fn install_transport(client: &Client, transport: Arc<dyn crate::transport::Transport>) {
    use wacore::handshake::NoiseCipher;
    *client.noise_socket.lock().expect("socket") = Some(Arc::new(crate::socket::NoiseSocket::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        transport,
        NoiseCipher::new(&[0; 32]).expect("key"),
        NoiseCipher::new(&[0; 32]).expect("key"),
    )));
}

fn sample_relay() -> GroupCallRelay {
    GroupCallRelay::builder()
        .transaction_id(1)
        .self_pid(0)
        .uuid("TEST-RELAY".to_string())
        .participant_uuid("TEST-PARTICIPANT".to_string())
        .attribute_padding(false)
        .warp_mi_tag_len(4)
        .key(vec![7; 32])
        .tokens(vec![vec![9; 16]])
        .endpoints(vec![
            GroupCallRelayEndpoint::builder()
                .relay_id(1)
                .token_id(0)
                .auth_token_id(0)
                .relay_name("test-relay".to_string())
                .is_fna(false)
                .ipv4("203.0.113.7".to_string())
                .port(3478)
                .build(),
        ])
        .build()
}

fn start(
    client: Arc<Client>,
    incoming: IncomingCall,
) -> tokio::task::JoinHandle<Result<CallHandle, CallError>> {
    tokio::spawn(async move {
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();
        client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await
    })
}

fn start_observing_cleanup(
    client: Arc<Client>,
    incoming: IncomingCall,
) -> (
    tokio::task::JoinHandle<Result<CallHandle, CallError>>,
    async_channel::Receiver<()>,
) {
    let (completed, completion) = async_channel::bounded(1);
    let setup = tokio::spawn(async move {
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();
        client
            .voip()
            .accept(&incoming)
            .observe_cleanup(completed)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await
    });
    (setup, completion)
}

async fn wait_frames(transport: &CapturingMockTransport, count: usize) {
    // Keep the same one-second bound even when the protocol timer is intentionally paused.
    let deadline = wacore::time::Instant::now() + Duration::from_secs(1);
    while transport.sent_count() < count {
        assert!(
            wacore::time::Instant::now() < deadline,
            "expected complete wire frames"
        );
        tokio::task::yield_now().await;
    }
}

fn actions(transport: &CapturingMockTransport) -> Vec<String> {
    crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0; 32])
        .iter()
        .map(|frame| {
            let bytes = wacore_binary::util::unpack(frame).expect("frame");
            let node = wacore_binary::OwnedNodeRef::new(bytes.into_owned()).expect("node");
            let node = node.get();
            assert_eq!(node.tag, "call");
            assert_eq!(
                node.get_attr("to").map(ToString::to_string),
                Some("INVITED-CALL@call".to_string())
            );
            node.children().expect("action")[0].tag.as_ref().to_string()
        })
        .collect()
}

fn supply_relay(client: &Client, incoming: &IncomingCall, generation: u64, media: &str) {
    let mut update = (**incoming.group.as_ref().expect("group")).clone();
    update.transaction_id = 13; // Older roster, independent relay transaction 1.
    update.media = media.to_string();
    update.relay = Some(sample_relay());
    assert_eq!(
        client
            .call_registry()
            .apply_group_update_if_current(update, generation),
        GroupStateApply::Applied
    );
}

#[tokio::test(start_paused = true)]
async fn missing_frames_fail_without_advancing_paused_protocol_time() {
    let protocol_start = tokio::time::Instant::now();
    let capture = Arc::new(CapturingMockTransport::new());
    let wait = tokio::spawn(async move { wait_frames(&capture, 1).await });
    assert!(
        wait.await
            .expect_err("missing frames must fail their fixed deadline")
            .is_panic()
    );
    assert_eq!(tokio::time::Instant::now(), protocol_start);
}

#[tokio::test]
async fn invitation_success_sends_exactly_one_answer_pair_with_or_without_initial_relay() {
    for relay in [false, true] {
        let (client, transport, incoming, generation) = fixture(relay, "audio").await;
        let setup = start(client.clone(), incoming.clone());
        wait_frames(&transport, 2).await;
        if !relay {
            assert!(
                !setup.is_finished(),
                "relay-less setup must still wait after accepting"
            );
            supply_relay(&client, &incoming, generation, "audio");
        }
        let handle = setup
            .await
            .expect("setup task")
            .expect("successful fake media join");
        assert_eq!(actions(&transport), ["preaccept", "accept"]);
        assert_eq!(
            client
                .call_registry()
                .generation_of(incoming.action.call_id()),
            Some(generation)
        );
        handle.hangup_local().await;
    }
}

#[tokio::test(start_paused = true)]
async fn invitation_relay_timeout_terminates_and_reaps_the_owned_generation() {
    let (client, transport, incoming, _) = fixture(false, "audio").await;
    let setup = start(client.clone(), incoming.clone());
    wait_frames(&transport, 2).await;
    assert!(!setup.is_finished());
    tokio::time::advance(OFFER_ACK_RELAY_TIMEOUT).await;
    assert!(matches!(
        setup.await.expect("task"),
        Err(CallError::ResponseTimeout)
    ));
    wait_frames(&transport, 3).await;
    assert_eq!(actions(&transport), ["preaccept", "accept", "terminate"]);
    assert!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id())
            .is_none()
    );
    assert!(
        client
            .call_registry()
            .ringing_group_generation(incoming.action.call_id(), incoming.action.call_creator())
            .is_none()
    );
}

#[tokio::test]
async fn invitation_cancellation_during_relay_wait_terminates_and_reaps() {
    let (client, transport, incoming, _) = fixture(false, "audio").await;
    let setup = start(client.clone(), incoming.clone());
    wait_frames(&transport, 2).await;
    setup.abort();
    assert!(matches!(setup.await, Err(error) if error.is_cancelled()));
    wait_frames(&transport, 3).await;
    assert_eq!(actions(&transport), ["preaccept", "accept", "terminate"]);
    assert!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id())
            .is_none()
    );
}

#[tokio::test]
async fn invitation_known_media_mismatch_emits_no_response() {
    let (client, transport, incoming, generation) = fixture(false, "video").await;
    let result = start(client.clone(), incoming.clone()).await.expect("task");
    assert!(
        matches!(result, Err(CallError::Response(message)) if message == "group offer signaling and roster media modes differ")
    );
    assert!(transport.sent().is_empty());
    assert_eq!(
        client
            .call_registry()
            .ringing_group_generation(incoming.action.call_id(), incoming.action.call_creator()),
        Some(generation)
    );
}

#[tokio::test]
async fn invitation_concurrent_starts_claim_the_ringing_generation_once() {
    let (client, transport, incoming, generation) = fixture(false, "audio").await;
    let first = start(client.clone(), incoming.clone());
    wait_frames(&transport, 2).await;
    let second = start(client.clone(), incoming.clone());
    let result = tokio::time::timeout(Duration::from_secs(1), second)
        .await
        .expect("second start refuses promptly")
        .expect("task");
    assert!(matches!(result, Err(CallError::CallEndedDuringSetup)));
    assert_eq!(actions(&transport), ["preaccept", "accept"]);
    supply_relay(&client, &incoming, generation, "audio");
    let handle = first.await.expect("task").expect("first start owns setup");
    assert_eq!(actions(&transport), ["preaccept", "accept"]);
    handle.hangup_local().await;
}

#[tokio::test]
async fn invitation_cleanup_cannot_terminate_a_same_id_replacement() {
    let (client, transport, incoming, generation) = fixture(false, "audio").await;
    let (setup, completion) = start_observing_cleanup(client.clone(), incoming.clone());
    wait_frames(&transport, 2).await;
    let replacement = {
        let _transition = client
            .lock_answer_transition(incoming.action.call_id())
            .await;
        assert!(
            client
                .call_registry()
                .remove_if_current(incoming.action.call_id(), generation)
        );
        let mut session = wacore::voip_control::CallSession::new_incoming(
            incoming.action.call_id(),
            incoming.from.clone(),
            incoming.action.call_creator().clone(),
        );
        session.group = incoming.group.as_deref().cloned();
        client
            .call_registry()
            .insert_ringing_group_if_inactive(session)
            .expect("snapshot")
            .expect("replacement")
    };
    assert_ne!(replacement, generation);
    assert!(matches!(
        setup.await.expect("task"),
        Err(CallError::CallEndedDuringSetup)
    ));
    tokio::time::timeout(Duration::from_secs(1), completion.recv())
        .await
        .expect("owned cleanup completed before deadline")
        .expect("cleanup completion");
    assert_eq!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id()),
        Some(replacement)
    );
    assert_eq!(actions(&transport), ["preaccept", "accept"]);
}

#[tokio::test]
async fn invitation_media_change_while_waiting_is_revalidated_and_terminated() {
    let (client, transport, incoming, generation) = fixture(false, "audio").await;
    let setup = start(client.clone(), incoming.clone());
    wait_frames(&transport, 2).await;
    let mut changed = (**incoming.group.as_ref().expect("group")).clone();
    changed.transaction_id = 16;
    changed.media = "video".to_string();
    assert_eq!(
        client
            .call_registry()
            .apply_group_update_if_current(changed, generation),
        GroupStateApply::Applied
    );
    supply_relay(&client, &incoming, generation, "audio");
    assert!(matches!(
        setup.await.expect("task"),
        Err(CallError::Response(_))
    ));
    wait_frames(&transport, 3).await;
    assert_eq!(actions(&transport), ["preaccept", "accept", "terminate"]);
    assert!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id())
            .is_none()
    );
}

#[tokio::test]
async fn invitation_cancellation_during_each_early_write_keeps_cleanup_ownership() {
    struct GatedWrite {
        capture: Arc<CapturingMockTransport>,
        attempt: std::sync::atomic::AtomicUsize,
        gate: usize,
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }
    #[async_trait::async_trait]
    impl crate::transport::Transport for GatedWrite {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            if self.attempt.fetch_add(1, Ordering::SeqCst) == self.gate {
                self.entered.send(()).await?;
                self.release.recv().await?;
            }
            crate::transport::Transport::send(&*self.capture, data).await
        }
        async fn disconnect(&self) {}
    }
    for gate in [0, 1] {
        let (client, transport, incoming, _) = fixture(false, "audio").await;
        let (entered_tx, entered_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        install_transport(
            &client,
            Arc::new(GatedWrite {
                capture: transport.clone(),
                attempt: std::sync::atomic::AtomicUsize::new(0),
                gate,
                entered: entered_tx,
                release: release_rx,
            }),
        );
        let setup = start(client.clone(), incoming.clone());
        tokio::time::timeout(Duration::from_secs(1), entered_rx.recv())
            .await
            .expect("early write entered before deadline")
            .expect("early write entered");
        setup.abort();
        assert!(matches!(setup.await, Err(error) if error.is_cancelled()));
        // The Noise sender owns its queued write even when its caller is cancelled. Allow it to
        // finish; generation-owned termination follows it without reopening the replacement lane.
        release_tx.send(()).await.expect("release original write");
        wait_frames(&transport, gate + 2).await;
        let expected = if gate == 0 {
            vec!["preaccept", "terminate"]
        } else {
            vec!["preaccept", "accept", "terminate"]
        };
        assert_eq!(actions(&transport), expected);
        assert!(
            client
                .call_registry()
                .generation_of(incoming.action.call_id())
                .is_none()
        );
    }
}

#[tokio::test]
async fn invitation_relay_arriving_before_claim_does_not_accept_before_validation() {
    let (client, transport, incoming, generation) = fixture(false, "audio").await;
    let lane = client
        .lock_answer_transition(incoming.action.call_id())
        .await;
    let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
    let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();
    let voip = client.voip();
    let setup = voip
        .accept(&incoming)
        .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
        .start();
    tokio::pin!(setup);
    assert!(
        futures::poll!(&mut setup).is_pending(),
        "setup blocked on held claim lane"
    );
    supply_relay(&client, &incoming, generation, "audio");
    client
        .persistence_manager()
        .process_command(crate::store::commands::DeviceCommand::SetLid(None))
        .await;
    drop(lane);
    assert!(
        matches!(setup.await, Err(CallError::Media("no own LID"))),
        "known context failure must be validated before acceptance"
    );
    assert_eq!(actions(&transport), ["preaccept", "terminate"]);
    assert!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id())
            .is_none()
    );
}

#[tokio::test]
async fn invitation_accept_write_failure_still_owns_terminal_cleanup() {
    struct FailAccept {
        capture: Arc<CapturingMockTransport>,
        attempt: std::sync::atomic::AtomicUsize,
    }
    #[async_trait::async_trait]
    impl crate::transport::Transport for FailAccept {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            if self.attempt.fetch_add(1, Ordering::SeqCst) == 1 {
                return Err(anyhow::anyhow!("injected accept write failure"));
            }
            crate::transport::Transport::send(&*self.capture, data).await
        }
        async fn disconnect(&self) {}
    }
    let (client, transport, incoming, _) = fixture(false, "audio").await;
    install_transport(
        &client,
        Arc::new(FailAccept {
            capture: transport.clone(),
            attempt: std::sync::atomic::AtomicUsize::new(0),
        }),
    );
    let result = start(client.clone(), incoming.clone()).await.expect("task");
    assert!(matches!(result, Err(CallError::Send(_))));
    // Noise correctly poisons its sender after an ambiguous write failure. Cleanup must still
    // remove this generation, but cannot promise a successful terminal write on that socket.
    assert_eq!(actions(&transport), ["preaccept"]);
    assert!(
        client
            .call_registry()
            .generation_of(incoming.action.call_id())
            .is_none()
    );
}
