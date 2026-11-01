//! Standalone call control against a synthetic host. No authenticated Client or ratchet store.

use portable_atomic::AtomicU64;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use futures::channel::oneshot;
use wacore::message_processing::EncType;
use wacore::messages::MessageUtils;
use wacore::runtime::Runtime;
use wacore::stanza::call::{CAPABILITY_OFFER, OfferDeviceKey, parse_call_stanza};
use wacore::voip_control::fake_backend::FakeMediaBackend;
use wacore::voip_control::registry::CallRegistry;
use wacore::voip_control::{
    AudioSink, AudioSource, CallDirection, MediaOpenContext, MediaSessionKey, MediaSessionSpec,
    MediaSetupError, VoipMediaBackend, VoipMediaSession,
};
use wacore_binary::{Jid, Node, Server, builder::NodeBuilder};

use super::direct::{self, CallAckFuture, DirectCallHost, DirectCallKeys, DirectCallState};
use crate::CallError;

fn own() -> Jid {
    Jid::new("111111111111111", Server::Lid).with_device(1)
}
fn peer() -> Jid {
    Jid::new("222222222222222", Server::Lid)
}

#[derive(Default)]
struct Backend {
    sessions: FakeMediaBackend,
    opened: Mutex<Vec<MediaSessionSpec>>,
    fail_open: AtomicBool,
    opened_signal: tokio::sync::Notify,
}

#[async_trait]
impl VoipMediaBackend for Backend {
    fn reserve(
        &self,
        key: &MediaSessionKey,
        direction: CallDirection,
    ) -> Arc<dyn VoipMediaSession> {
        self.sessions.reserve(key, direction)
    }

    async fn open(
        &self,
        spec: MediaSessionSpec,
        mut context: MediaOpenContext,
    ) -> Result<(), MediaSetupError> {
        if self.fail_open.load(Ordering::Relaxed) {
            return Err(MediaSetupError::Backend(
                "synthetic media setup failure".into(),
            ));
        }
        self.opened.lock().unwrap().push(spec);
        self.opened_signal.notify_one();
        if let Some(connected) = context.on_media_connected.take() {
            connected();
        }
        Ok(())
    }
}

type AckMap = Arc<Mutex<HashMap<String, (u64, oneshot::Sender<Node>)>>>;
struct AckLease {
    map: AckMap,
    id: String,
    generation: u64,
    released: async_channel::Sender<String>,
}
impl Drop for AckLease {
    fn drop(&mut self) {
        let mut map = self.map.lock().unwrap();
        if map
            .get(&self.id)
            .is_some_and(|(generation, _)| *generation == self.generation)
        {
            map.remove(&self.id);
        }
        let _ = self.released.try_send(self.id.clone());
    }
}

struct Host {
    state: DirectCallState,
    backend: Arc<Backend>,
    sent: Mutex<Vec<Node>>,
    plaintexts: Mutex<Vec<Vec<u8>>>,
    decryptions: Mutex<Vec<(Jid, EncType, Vec<u8>)>>,
    next: AtomicU64,
    own_identity: Option<Jid>,
    resolved_peer: Jid,
    resolutions: AtomicU64,
    connected: AtomicBool,
    fail_keys: AtomicBool,
    fail_token: AtomicBool,
    fail_decrypt: AtomicBool,
    fail_send: AtomicBool,
    gate_offer: AtomicBool,
    disconnect_during_keys: AtomicBool,
    disconnect_during_token: AtomicBool,
    ack: AckMap,
    released_tx: async_channel::Sender<String>,
    released_rx: async_channel::Receiver<String>,
}

impl Host {
    fn new() -> Arc<Self> {
        let registry = Arc::new(CallRegistry::new());
        let backend = Arc::new(Backend::default());
        assert!(registry.install_backend(backend.clone()));
        let (released_tx, released_rx) = async_channel::bounded(64);
        Arc::new(Self {
            state: DirectCallState::new(registry),
            backend,
            sent: Mutex::new(Vec::new()),
            plaintexts: Mutex::new(Vec::new()),
            decryptions: Mutex::new(Vec::new()),
            next: AtomicU64::new(0),
            own_identity: Some(own()),
            resolved_peer: peer(),
            resolutions: AtomicU64::new(0),
            connected: AtomicBool::new(true),
            fail_keys: AtomicBool::new(false),
            fail_token: AtomicBool::new(false),
            fail_decrypt: AtomicBool::new(false),
            fail_send: AtomicBool::new(false),
            gate_offer: AtomicBool::new(false),
            disconnect_during_keys: AtomicBool::new(false),
            disconnect_during_token: AtomicBool::new(false),
            ack: Arc::new(Mutex::new(HashMap::new())),
            released_tx,
            released_rx,
        })
    }

    fn assert_empty(&self) {
        assert_eq!(self.state.registry().memory_stats().entries, 0);
        assert!(self.state.pending_outgoing_calls.lock().unwrap().is_empty());
        assert!(self.ack.lock().unwrap().is_empty());
    }

    fn forget_ack(&self, id: &str) {
        self.ack.lock().unwrap().remove(id);
    }
}

#[async_trait]
impl DirectCallHost for Host {
    fn call_state(&self) -> &DirectCallState {
        &self.state
    }
    fn runtime(&self) -> Arc<dyn Runtime> {
        Arc::new(crate::runtime_impl::TokioRuntime)
    }
    fn own_lid(&self) -> Option<Jid> {
        self.own_identity.clone()
    }
    fn own_pn(&self) -> Option<Jid> {
        Some(Jid::new("15550001111", Server::Pn).with_device(1))
    }
    async fn namespace_alias(&self, _peer: &Jid) -> Option<Jid> {
        None
    }
    async fn learn_identity(
        &self,
        _lid: &Jid,
        _pn: &Jid,
        _offline: bool,
        _has_username: bool,
    ) -> Result<(), CallError> {
        Ok(())
    }
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    fn generate_request_id(&self) -> String {
        format!("DIRECT-{}", self.next.fetch_add(1, Ordering::Relaxed))
    }
    async fn resolve_peer(&self, _peer: &Jid) -> Result<Jid, CallError> {
        self.resolutions.fetch_add(1, Ordering::Relaxed);
        Ok(self.resolved_peer.clone())
    }
    async fn prepare_keys(
        &self,
        _peer: &Jid,
        plaintext: &[u8],
    ) -> Result<DirectCallKeys, CallError> {
        if self.fail_keys.load(Ordering::Relaxed) {
            return Err(CallError::Setup("synthetic key commit failure".into()));
        }
        if self.disconnect_during_keys.load(Ordering::Relaxed) {
            self.connected.store(false, Ordering::Release);
            direct::disconnect(self);
            self.connected.store(true, Ordering::Release);
        }
        self.plaintexts.lock().unwrap().push(plaintext.to_vec());
        Ok(DirectCallKeys {
            device_keys: vec![OfferDeviceKey {
                device_jid: peer().with_device(2),
                ciphertext: vec![0x21, 0x22],
                enc_type: "msg".into(),
            }],
            ring_devices: vec![peer().with_device(0), peer().with_device(2)],
            device_identity: None,
        })
    }
    async fn decrypt(
        &self,
        peer: &Jid,
        enc_type: EncType,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, CallError> {
        self.decryptions
            .lock()
            .unwrap()
            .push((peer.clone(), enc_type, ciphertext.to_vec()));
        if self.fail_decrypt.load(Ordering::Relaxed) {
            return Err(CallError::Decrypt("synthetic decrypt failure".into()));
        }
        let mut message = waproto::whatsapp::Message::default();
        let mut call = waproto::whatsapp::message::Call::default();
        call.call_key = Some(vec![0x55; 32]);
        message.call = buffa::MessageField::some(call);
        Ok(MessageUtils::encode_and_pad(&message))
    }
    async fn read_token(&self, _peer: &Jid) -> Result<Option<Vec<u8>>, CallError> {
        if self.fail_token.load(Ordering::Relaxed) {
            return Err(CallError::Setup("synthetic token store failure".into()));
        }
        if self.disconnect_during_token.load(Ordering::Relaxed) {
            self.connected.store(false, Ordering::Release);
            direct::disconnect(self);
            self.connected.store(true, Ordering::Release);
        }
        Ok(Some(vec![0x31, 0x32]))
    }
    async fn issue_token(&self, _peer: &Jid) {}
    async fn send_node(&self, node: Node) -> Result<(), CallError> {
        let is_offer = node.as_node_ref().get_optional_child("offer").is_some();
        if is_offer {
            let id = node
                .attrs
                .get("id")
                .expect("offer correlation id")
                .as_str()
                .into_owned();
            assert!(
                self.ack.lock().unwrap().contains_key(&id),
                "ACK observation precedes the transport write"
            );
        }
        self.sent.lock().unwrap().push(node);
        if self.fail_send.load(Ordering::Relaxed) {
            return Err(CallError::Setup("synthetic transport failure".into()));
        }
        if is_offer && self.gate_offer.load(Ordering::Relaxed) {
            futures::future::pending::<()>().await;
        }
        Ok(())
    }
    fn watch_ack(&self, id: &str) -> Result<CallAckFuture, CallError> {
        let (sender, receiver) = oneshot::channel();
        let generation = self.next.fetch_add(1, Ordering::Relaxed);
        let mut map = self.ack.lock().unwrap();
        if map.contains_key(id) {
            return Err(CallError::Response("duplicate synthetic ACK id".into()));
        }
        map.insert(id.to_owned(), (generation, sender));
        let lease = AckLease {
            map: self.ack.clone(),
            id: id.to_owned(),
            generation,
            released: self.released_tx.clone(),
        };
        Ok(Box::pin(async move {
            let _lease = lease;
            receiver
                .await
                .map_err(|_| CallError::Response("synthetic ACK channel closed".into()))
        }))
    }
}

fn ports() -> (Arc<dyn AudioSource>, Arc<dyn AudioSink>) {
    let (_source, source) = async_channel::bounded::<Vec<i16>>(1);
    let (sink, _output) = async_channel::bounded::<Vec<i16>>(1);
    (Arc::new(source), Arc::new(sink))
}

fn relay() -> Node {
    NodeBuilder::new("relay")
        .children([
            NodeBuilder::new("key")
                .bytes(b"cmVsYXkta2V5".to_vec())
                .build(),
            NodeBuilder::new("warp_mi_tag_len")
                .bytes(b"4".to_vec())
                .build(),
            NodeBuilder::new("token")
                .attr("id", "0")
                .bytes(vec![0x41; 16])
                .build(),
            NodeBuilder::new("auth_token")
                .attr("id", "1")
                .bytes(vec![0x42; 16])
                .build(),
            NodeBuilder::new("te2")
                .attr("relay_id", "1")
                .attr("relay_name", "test-relay")
                .attr("token_id", "0")
                .attr("auth_token_id", "1")
                .bytes(vec![203, 0, 113, 7, 0x0d, 0x96])
                .build(),
        ])
        .build()
}

fn incoming_node() -> Node {
    NodeBuilder::new("call")
        .attr("from", peer())
        .attr("id", "INCOMING-STANZA")
        .attr("t", "1700000000")
        .children([
            NodeBuilder::new("offer")
                .attr("call-id", "DIRECT-INCOMING")
                .attr("call-creator", peer())
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "msg")
                        .bytes(vec![0x61, 0x62])
                        .build(),
                    NodeBuilder::new("capability")
                        .attr("ver", "1")
                        .bytes(CAPABILITY_OFFER.to_vec())
                        .build(),
                ])
                .build(),
            relay(),
        ])
        .build()
}

fn incoming() -> wacore::types::call::IncomingCall {
    parse_call_stanza(&incoming_node().as_node_ref())
        .expect("valid synthetic offer")
        .expect("call")
}

#[tokio::test]
async fn outgoing_uses_host_keys_and_registers_ack_before_send() {
    let host = Host::new();
    let (source, sink) = ports();
    let handle = direct::call_pcm(
        host.clone(),
        &Jid::new("15550002222", Server::Pn),
        source,
        sink,
    )
    .await
    .expect("call");
    {
        let sent = host.sent.lock().unwrap();
        let offer = sent[0].as_node_ref();
        assert_eq!(offer.get_attr("to").unwrap().as_str(), peer().to_string());
        let action = offer.get_optional_child("offer").expect("offer child");
        let destinations = action
            .get_optional_child("destination")
            .expect("full ring set retains addressed form");
        let destination = &destinations.children().unwrap()[0];
        assert_eq!(
            destination.get_attr("jid").unwrap().as_str(),
            peer().with_device(2).to_string()
        );
        assert_eq!(
            destination
                .get_optional_child("enc")
                .unwrap()
                .content_bytes()
                .unwrap(),
            [0x21, 0x22]
        );
        assert_eq!(
            action
                .get_optional_child("privacy")
                .unwrap()
                .content_bytes()
                .unwrap(),
            [0x31, 0x32]
        );
    }
    {
        let payloads = host.plaintexts.lock().unwrap();
        let padded = &payloads[0];
        let proto =
            waproto::codec::message_decode(MessageUtils::unpad_message_ref(padded, 2).unwrap())
                .unwrap();
        assert_eq!(
            proto
                .call
                .as_option()
                .unwrap()
                .call_key
                .as_ref()
                .unwrap()
                .len(),
            32
        );
    }
    assert!(!handle.media_connected());
    assert!(matches!(
        handle.terminate().await,
        super::CallTermination::PeerNotified
    ));
    handle.wait_ended().await;
    host.released_rx
        .recv()
        .await
        .expect("ended call cancels ACK wait without timeout");
    host.assert_empty();
}

#[tokio::test]
async fn failed_keys_or_token_store_never_send_or_reserve_media() {
    for fail_keys in [true, false] {
        let host = Host::new();
        host.fail_keys.store(fail_keys, Ordering::Relaxed);
        host.fail_token.store(!fail_keys, Ordering::Relaxed);
        let (source, sink) = ports();
        assert!(
            direct::call_pcm(host.clone(), &peer(), source, sink)
                .await
                .is_err()
        );
        assert!(host.sent.lock().unwrap().is_empty());
        assert!(host.backend.sessions.sessions().is_empty());
        host.assert_empty();
    }
}

#[tokio::test]
async fn failed_offer_write_releases_registration_and_ack_without_background_timeout() {
    let host = Host::new();
    host.fail_send.store(true, Ordering::Relaxed);
    let (source, sink) = ports();
    assert!(
        direct::call_pcm(host.clone(), &peer(), source, sink)
            .await
            .is_err()
    );
    host.assert_empty();
    assert!(
        host.backend.sessions.sessions()[0]
            .record()
            .closed
            .is_some()
    );
}

#[tokio::test]
async fn cancellation_while_offer_write_is_pending_retires_only_its_generation() {
    let host = Host::new();
    host.gate_offer.store(true, Ordering::Relaxed);
    let (source, sink) = ports();
    let peer = peer();
    let mut starting = Box::pin(direct::call_pcm(host.clone(), &peer, source, sink));
    assert!(futures::poll!(starting.as_mut()).is_pending());
    assert_eq!(host.state.pending_outgoing_calls.lock().unwrap().len(), 1);
    drop(starting);
    host.assert_empty();
    assert!(
        host.backend.sessions.sessions()[0]
            .record()
            .closed
            .is_some()
    );
}

#[tokio::test]
async fn disconnect_ends_a_dormant_call_and_drops_its_response_observer() {
    let host = Host::new();
    let (source, sink) = ports();
    let handle = direct::call_pcm(host.clone(), &peer(), source, sink)
        .await
        .expect("call");
    host.connected.store(false, Ordering::Release);
    direct::disconnect(&*host);
    handle.wait_ended().await;
    host.released_rx
        .recv()
        .await
        .expect("disconnect wakes response wait");
    host.assert_empty();
    let (source, sink) = ports();
    assert!(matches!(
        direct::call_pcm(host.clone(), &peer(), source, sink).await,
        Err(CallError::Connect(_))
    ));
}

#[tokio::test]
async fn incoming_decrypts_through_host_and_opens_the_same_neutral_backend() {
    let host = Host::new();
    let (source, sink) = ports();
    let offer = incoming();
    let handle = direct::accept_pcm(host.clone(), &offer, source, sink)
        .await
        .expect("accept");
    {
        let decryptions = host.decryptions.lock().unwrap();
        assert_eq!(decryptions.len(), 1);
        assert_eq!(decryptions[0].0, peer());
        assert_eq!(decryptions[0].1, EncType::Message);
        assert_eq!(decryptions[0].2, [0x61, 0x62]);
    }
    {
        let opened = host.backend.opened.lock().unwrap();
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].call_key, [0x55; 32]);
        assert_eq!(opened[0].direction, CallDirection::Incoming);
    }
    assert!(handle.media_connected());
    assert!(matches!(
        handle.terminate().await,
        super::CallTermination::PeerNotified
    ));
    host.assert_empty();
}

#[tokio::test]
async fn failed_incoming_decryption_does_not_open_media_or_leave_a_ringing_call() {
    let host = Host::new();
    host.fail_decrypt.store(true, Ordering::Relaxed);
    let (source, sink) = ports();
    assert!(
        direct::accept_pcm(host.clone(), &incoming(), source, sink)
            .await
            .is_err()
    );
    assert!(host.backend.opened.lock().unwrap().is_empty());
    let sent = host.sent.lock().unwrap();
    let actions: Vec<_> = sent
        .iter()
        .filter_map(|node| {
            node.as_node_ref()
                .children()
                .and_then(|children| children.first())
                .map(|child| child.tag.to_string())
        })
        .collect();
    assert_eq!(actions, ["preaccept", "terminate"]);
    drop(sent);
    host.assert_empty();
}

#[tokio::test]
async fn dropping_old_ack_registration_cannot_cancel_a_reused_id() {
    let host = Host::new();
    let first = host.watch_ack("REUSED").unwrap();
    host.forget_ack("REUSED");
    let second = host.watch_ack("REUSED").unwrap();
    drop(first);
    assert!(host.ack.lock().unwrap().contains_key("REUSED"));
    drop(second);
    host.assert_empty();
}

#[tokio::test]
async fn cancelling_an_old_start_cannot_remove_a_replacement_with_the_same_call_id() {
    let host = Host::new();
    host.gate_offer.store(true, Ordering::Relaxed);
    let (source, sink) = ports();
    let peer = peer();
    let mut starting = Box::pin(direct::call_pcm(host.clone(), &peer, source, sink));
    assert!(futures::poll!(starting.as_mut()).is_pending());
    let id = host
        .state
        .pending_outgoing_calls
        .lock()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let registry = host.state.registry();
    let old = registry.generation_of(&id).expect("original generation");
    let replacement = registry.insert(wacore::voip_control::CallSession::new_outgoing(
        &id,
        peer.clone(),
        own(),
    ));
    assert_ne!(replacement, old);
    drop(starting);
    assert_eq!(registry.generation_of(&id), Some(replacement));
    assert!(host.state.pending_outgoing_calls.lock().unwrap().is_empty());
    assert!(host.ack.lock().unwrap().is_empty());
    direct::disconnect(&*host);
    host.assert_empty();
}

#[tokio::test]
async fn an_error_ack_with_a_valid_relay_does_not_open_media() {
    let host = Host::new();
    let (source, sink) = ports();
    let handle = direct::call_pcm(host.clone(), &peer(), source, sink)
        .await
        .expect("call");
    let mut events = handle.take_events().expect("call events");
    let (id, (_, sender)) = {
        let mut pending = host.ack.lock().unwrap();
        let id = pending.keys().next().unwrap().clone();
        let waiter = pending.remove(&id).unwrap();
        (id, waiter)
    };
    sender
        .send(
            NodeBuilder::new("ack")
                .attr("id", &id)
                .attr("class", "call")
                .attr("type", "offer")
                .attr("error", "503")
                .children([relay()])
                .build(),
        )
        .expect("deliver rejection");
    tokio::select! {
        () = handle.wait_ended() => {},
        () = host.backend.opened_signal.notified() => panic!("an error ACK must not open the otherwise valid relay"),
    }
    assert!(
        matches!(events.try_recv(), Ok(wacore::voip_control::CallEvent::MediaSetupFailed(reason)) if reason.contains("503"))
    );
    host.released_rx
        .recv()
        .await
        .expect("response observer released");
    assert!(host.backend.opened.lock().unwrap().is_empty());
    host.assert_empty();
}

#[tokio::test]
async fn reconnect_during_key_or_token_callback_does_not_send_an_old_offer() {
    for during_keys in [true, false] {
        let host = Host::new();
        host.disconnect_during_keys
            .store(during_keys, Ordering::Relaxed);
        host.disconnect_during_token
            .store(!during_keys, Ordering::Relaxed);
        let (source, sink) = ports();
        assert!(
            direct::call_pcm(host.clone(), &peer(), source, sink)
                .await
                .is_err()
        );
        assert!(
            host.is_connected(),
            "the host already reports its new connection"
        );
        assert!(host.sent.lock().unwrap().is_empty());
        assert!(host.backend.sessions.sessions().is_empty());
        host.assert_empty();
    }
}

#[tokio::test]
async fn incoming_backend_setup_failure_ends_the_call_after_acceptance() {
    let host = Host::new();
    host.backend.fail_open.store(true, Ordering::Relaxed);
    let (source, sink) = ports();
    let result = direct::accept_pcm(host.clone(), &incoming(), source, sink).await;
    assert!(matches!(result, Err(CallError::Setup(_))));
    assert!(host.backend.opened.lock().unwrap().is_empty());
    assert!(
        host.backend.sessions.sessions()[0]
            .record()
            .closed
            .is_some()
    );
    host.assert_empty();
}

#[tokio::test]
async fn ingress_sends_only_the_distinct_offer_receipt_and_retains_an_acceptable_offer() {
    let host = Host::new();
    let node = incoming_node();
    let offer = direct::ingest(host.clone(), &node.as_node_ref())
        .await
        .expect("ingress")
        .expect("offer");
    {
        let sent = host.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        let receipt = sent[0].as_node_ref();
        assert_eq!(receipt.tag.as_ref(), "receipt");
        assert_eq!(receipt.get_attr("id").unwrap().as_str(), "INCOMING-STANZA");
        assert_eq!(
            receipt
                .get_optional_child("offer")
                .unwrap()
                .get_attr("call-id")
                .unwrap()
                .as_str(),
            "DIRECT-INCOMING"
        );
        assert!(
            !sent.iter().any(|node| node.tag == "ack"),
            "the existing host dispatcher remains the generic ACK owner"
        );
    }
    let (source, sink) = ports();
    let handle = direct::accept_pcm(host.clone(), &offer, source, sink)
        .await
        .expect("accept retained offer");
    assert!(handle.media_connected());
    assert!(matches!(
        handle.terminate().await,
        super::CallTermination::PeerNotified
    ));
    host.assert_empty();
}

#[tokio::test]
async fn an_old_retained_offer_cannot_accept_a_same_id_redelivery() {
    let host = Host::new();
    let first = incoming_node();
    let stale = direct::ingest(host.clone(), &first.as_node_ref())
        .await
        .expect("first ingress")
        .expect("first offer");
    let mut second = incoming_node();
    second.attrs.insert("id", "INCOMING-REPLACEMENT");
    let current = direct::ingest(host.clone(), &second.as_node_ref())
        .await
        .expect("replacement ingress")
        .expect("replacement offer");
    let current_token = current.ringing_generation().expect("retained offer token");
    assert_ne!(stale.ringing_generation(), Some(current_token));
    let sent_before = host.sent.lock().unwrap().len();
    let (source, sink) = ports();
    assert!(matches!(
        direct::accept_pcm(host.clone(), &stale, source, sink).await,
        Err(CallError::CallEndedDuringSetup)
    ));
    assert_eq!(host.sent.lock().unwrap().len(), sent_before);
    assert!(host.decryptions.lock().unwrap().is_empty());
    assert!(host.backend.sessions.sessions().is_empty());
    assert!(
        host.state
            .registry()
            .is_current_ringing_offer("DIRECT-INCOMING", current_token)
    );
    let (source, sink) = ports();
    let handle = direct::accept_pcm(host.clone(), &current, source, sink)
        .await
        .expect("the latest offer remains acceptable");
    assert!(matches!(
        handle.terminate().await,
        super::CallTermination::PeerNotified
    ));
    host.assert_empty();
}

#[tokio::test]
async fn non_user_or_empty_call_targets_are_rejected_before_host_resolution() {
    for target in [
        Jid::new("SYNTHETIC-GROUP", Server::Group),
        Jid::new("SYNTHETIC-CALL", Server::Call),
        Jid::new("status", Server::Broadcast),
        Jid::new("SYNTHETIC-CHANNEL", Server::Newsletter),
        Jid::new("", Server::Pn),
        Jid::new("", Server::Lid),
    ] {
        let host = Host::new();
        let (source, sink) = ports();
        assert!(matches!(
            direct::call_pcm(host.clone(), &target, source, sink).await,
            Err(CallError::Media(_))
        ));
        assert_eq!(
            host.resolutions.load(Ordering::Relaxed),
            0,
            "unsupported targets must not be normalized into a valid peer by the host"
        );
        assert!(host.plaintexts.lock().unwrap().is_empty());
        assert!(host.sent.lock().unwrap().is_empty());
        assert!(host.backend.sessions.sessions().is_empty());
        host.assert_empty();
    }
}

#[tokio::test]
async fn a_host_cannot_resolve_the_media_peer_to_a_non_lid_or_empty_user() {
    for resolved in [
        Jid::new("15550002222", Server::Pn),
        Jid::new("SYNTHETIC-GROUP", Server::Group),
        Jid::new("", Server::Lid),
    ] {
        let mut host = Host::new();
        Arc::get_mut(&mut host).unwrap().resolved_peer = resolved;
        let (source, sink) = ports();
        assert!(matches!(
            direct::call_pcm(host.clone(), &peer(), source, sink).await,
            Err(CallError::Media(_))
        ));
        assert_eq!(host.resolutions.load(Ordering::Relaxed), 1);
        assert!(
            host.plaintexts.lock().unwrap().is_empty(),
            "no envelope encryption may start with an invalid resolved identity"
        );
        assert!(host.sent.lock().unwrap().is_empty());
        assert!(host.backend.sessions.sessions().is_empty());
        host.assert_empty();
    }
}

#[tokio::test]
async fn missing_or_invalid_own_lid_is_rejected_before_resolving_or_encrypting() {
    for identity in [
        None,
        Some(Jid::new("15550001111", Server::Pn)),
        Some(Jid::new("", Server::Lid)),
    ] {
        let mut host = Host::new();
        Arc::get_mut(&mut host).unwrap().own_identity = identity;
        let (source, sink) = ports();
        assert!(matches!(
            direct::call_pcm(host.clone(), &peer(), source, sink).await,
            Err(CallError::Media(_))
        ));
        assert_eq!(host.resolutions.load(Ordering::Relaxed), 0);
        assert!(host.plaintexts.lock().unwrap().is_empty());
        assert!(host.sent.lock().unwrap().is_empty());
        assert!(host.backend.sessions.sessions().is_empty());
        host.assert_empty();
    }
}
