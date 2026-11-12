// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Outbound: the service builds the stanza, the client only resolves devices
//! and bundles, and the recipient decrypts what was built.

use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use wacore::client::context::{GroupRoutingInfo, SendContextResolver, SessionLockGuard};
use wacore::libsignal::protocol::{DeviceId, IdentityKey, PreKeyBundle, PublicKey};
use wacore::prekeys::PreKeyFetchOutcome;
use wacore::runtime::{AbortHandle, Runtime};
use wacore::types::jid::JidExt as _;
use wacore_binary::{Jid, NodeContent};
use wacore_recordstore::{AesGcmSealer, LeaseStore, MemoryRecordStore, RecordSignalStore};
mod common;
use wacore_signal_ops::{
    FetchedBundle, ReceiveKind, ReceiveRequest, Received, SignalOps, SignalResolver,
    VerifiablePrekeys,
};
use waproto::whatsapp as wa;

mod runtime;
use runtime::InlineRuntime;

type Store = RecordSignalStore<MemoryRecordStore, AesGcmSealer>;
type BoxFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

struct TokioRuntime;

impl Runtime for TokioRuntime {
    fn spawn(&self, future: BoxFuture) -> AbortHandle {
        let task = tokio::spawn(future);
        AbortHandle::new(move || task.abort())
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(duration))
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture {
        Box::pin(async move {
            let _ = tokio::task::spawn_blocking(f).await;
        })
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        Some(Box::pin(tokio::task::yield_now()))
    }
}

async fn device(scope: &str, own_pn: &str) -> SignalOps<Store> {
    device_with_lid(scope, own_pn, None).await
}

async fn device_with_lid(scope: &str, own_pn: &str, own_lid: Option<&str>) -> SignalOps<Store> {
    let records = Arc::new(MemoryRecordStore::new());
    let lease = records
        .acquire(scope, "node-a", 0, 60_000)
        .await
        .unwrap()
        .unwrap();
    let store = Arc::new(RecordSignalStore::new(
        records,
        Arc::new(AesGcmSealer::new(&[5; 32])),
        lease.fence,
    ));
    let ops = SignalOps::create(store).await.unwrap();
    ops.set_own_jids(own_pn, own_lid).await.unwrap();
    ops
}

/// Stands in for the WhatsApp client: answers device-list and prekey
/// queries from a fixed table, as a real client would from the server.
struct FakeClient {
    devices: Vec<Jid>,
    bundles: HashMap<Jid, PreKeyBundle>,
    group: Option<Arc<GroupRoutingInfo>>,
    /// The `<device-identity>` the server sends with each companion's bundle.
    device_identities: HashMap<Jid, Vec<u8>>,
}

#[async_trait]
impl SendContextResolver for FakeClient {
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, _jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        Ok(self.devices.clone())
    }

    async fn fetch_prekeys(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        Ok(jids
            .iter()
            .filter_map(|jid| self.bundles.get(jid).map(|b| (jid.clone(), b.clone())))
            .collect())
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<PreKeyFetchOutcome, anyhow::Error> {
        Ok(PreKeyFetchOutcome {
            bundles: self.fetch_prekeys(jids).await?,
            rejected: Vec::new(),
        })
    }

    async fn resolve_group_routing_info(
        &self,
        _jid: &Jid,
    ) -> Result<Arc<GroupRoutingInfo>, anyhow::Error> {
        self.group
            .clone()
            .ok_or_else(|| anyhow::anyhow!("not a group test"))
    }
}

async fn bundle_for(ops: &SignalOps<Store>) -> PreKeyBundle {
    let identity = ops.public_identity().await;
    let prekey = ops
        .generate_prekeys(&InlineRuntime, 1)
        .await
        .unwrap()
        .remove(0);
    PreKeyBundle::new(
        identity.registration_id,
        DeviceId::from(1),
        Some((
            prekey.id.into(),
            PublicKey::from_djb_public_key_bytes(&prekey.public_key).unwrap(),
        )),
        identity.signed_prekey.id.into(),
        PublicKey::from_djb_public_key_bytes(&identity.signed_prekey.public_key).unwrap(),
        identity.signed_prekey.signature,
        IdentityKey::new(PublicKey::from_djb_public_key_bytes(&identity.identity_key).unwrap()),
    )
    .unwrap()
}

/// Pair `peer` under a fresh account and have `sender` pin that account, as it
/// would after meeting the peer's primary device. Returns the companion's
/// `<device-identity>` entry for a client's prekey responses.
async fn vouch(sender: &SignalOps<Store>, peer: &SignalOps<Store>, jid: &Jid) -> (Jid, Vec<u8>) {
    let paired = common::pair(peer).await;
    sender
        .trust_account_identity(jid, paired.account)
        .await
        .unwrap();
    (jid.clone(), paired.device_identity)
}

/// The bundles `client` holds for `jids`, each with its companion identity.
fn verifiable(client: &FakeClient, jids: &[Jid]) -> VerifiablePrekeys {
    VerifiablePrekeys {
        bundles: jids
            .iter()
            .filter_map(|jid| {
                client.bundles.get(jid).map(|bundle| {
                    (
                        jid.clone(),
                        FetchedBundle {
                            bundle: bundle.clone(),
                            device_identity: client.device_identities.get(jid).cloned(),
                        },
                    )
                })
            })
            .collect(),
        rejected: Vec::new(),
    }
}

#[async_trait]
impl SignalResolver for FakeClient {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        Ok(verifiable(self, jids))
    }
}

struct ChangingGroupClient {
    current: std::sync::RwLock<FakeClient>,
    pause_devices: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
    change_mapping: bool,
    mapping_reads: std::sync::atomic::AtomicUsize,
}

impl ChangingGroupClient {
    fn new(client: FakeClient, change_mapping: bool) -> Self {
        Self {
            current: std::sync::RwLock::new(client),
            pause_devices: std::sync::atomic::AtomicBool::new(false),
            entered: tokio::sync::Notify::new(),
            resume: tokio::sync::Notify::new(),
            change_mapping,
            mapping_reads: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl SendContextResolver for ChangingGroupClient {
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, _jids: &[Jid]) -> anyhow::Result<Vec<Jid>> {
        let snapshot = self.current.read().unwrap().devices.clone();
        if self
            .pause_devices
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            self.entered.notify_one();
            self.resume.notified().await;
        }
        Ok(snapshot)
    }

    async fn fetch_prekeys(&self, jids: &[Jid]) -> anyhow::Result<HashMap<Jid, PreKeyBundle>> {
        let client = self.current.read().unwrap();
        Ok(jids
            .iter()
            .filter_map(|jid| {
                client
                    .bundles
                    .get(jid)
                    .map(|bundle| (jid.clone(), bundle.clone()))
            })
            .collect())
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> anyhow::Result<PreKeyFetchOutcome> {
        Ok(PreKeyFetchOutcome {
            bundles: self.fetch_prekeys(jids).await?,
            rejected: Vec::new(),
        })
    }

    async fn resolve_group_routing_info(
        &self,
        _jid: &Jid,
    ) -> anyhow::Result<Arc<GroupRoutingInfo>> {
        Ok(self
            .current
            .read()
            .unwrap()
            .group
            .clone()
            .expect("group fixture"))
    }

    async fn get_lid_for_phone(&self, user: &str) -> Option<wacore_binary::CompactString> {
        if self.change_mapping && user == "222" {
            let previous = self
                .mapping_reads
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return (previous > 0).then(|| "900222".into());
        }
        None
    }
}

#[async_trait]
impl SignalResolver for ChangingGroupClient {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        Ok(verifiable(&self.current.read().unwrap(), jids))
    }
}

async fn receive_group_component(
    ops: &SignalOps<Store>,
    group: &Jid,
    from: &Jid,
    from_identity: &[u8],
    (kind, version, ciphertext): (String, u8, Vec<u8>),
) -> Result<Received, wacore_signal_ops::OpsError> {
    ops.receive(ReceiveRequest {
        chat: &group.to_string(),
        sender: &from.to_protocol_address(),
        kind: ReceiveKind::try_from(kind.as_str()).expect("enc type"),
        ciphertext: &ciphertext,
        padding_version: version,
        is_from_me: false,
        device_identity: (kind == "pkmsg").then_some(from_identity),
    })
    .await
}

fn parse(stanza: &[u8]) -> wacore_binary::Node {
    wacore_binary::marshal::unmarshal_packed_ref(stanza)
        .unwrap()
        .to_owned()
}

fn enc_parts(enc: &wacore_binary::Node) -> (String, u8, Vec<u8>) {
    let kind = enc.attrs().optional_string("type").unwrap().to_string();
    let version = enc
        .attrs()
        .optional_string("v")
        .map_or(2, |v| v.parse().unwrap());
    let Some(NodeContent::Bytes(bytes)) = &enc.content else {
        panic!("enc without bytes")
    };
    (kind, version, bytes.clone())
}

/// The pairwise `<enc>` addressed to `device`, if the stanza has one.
fn pairwise_enc_for(stanza: &[u8], device: &Jid) -> Option<(String, u8, Vec<u8>)> {
    let node = parse(stanza);
    let participants = node.get_optional_child("participants")?;
    let to = participants.get_children_by_tag("to").find(|to| {
        to.attrs().optional_string("jid").as_deref() == Some(device.to_string().as_str())
    })?;
    Some(enc_parts(to.get_optional_child("enc").expect("enc")))
}

/// The `<enc>` addressed to `device` inside a marshaled stanza.
fn enc_for(stanza: &[u8], device: &Jid) -> (String, u8, Vec<u8>) {
    pairwise_enc_for(stanza, device).expect("an entry for the device")
}

/// The group-wide `skmsg` `<enc>`.
fn skmsg_of(stanza: &[u8]) -> (String, u8, Vec<u8>) {
    let node = parse(stanza);
    let enc = node
        .get_children_by_tag("enc")
        .find(|e| e.attrs().optional_string("type").as_deref() == Some("skmsg"))
        .expect("skmsg");
    enc_parts(enc)
}

fn text(body: &str) -> wa::Message {
    let mut message = wa::Message::default();
    message.conversation = Some(body.to_owned());
    message
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stanza_built_by_the_service_decrypts_at_the_recipient() {
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let alice = device("alice-send", "111:1@s.whatsapp.net").await;
    let bob = device("bob-send", "222:1@s.whatsapp.net").await;
    let alice_identity = common::vouch(&bob, &alice, "111:1@s.whatsapp.net").await;
    let client = FakeClient {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::from([vouch(&alice, &bob, &bob_jid).await]),
        group: None,
    };
    let to = bob_jid.to_non_ad();

    for (round, body) in ["first message", "second message"].into_iter().enumerate() {
        let id = format!("MSG{round}");
        let sent = alice
            .send_direct(&TokioRuntime, &client, &to, &text(body), &id)
            .await
            .unwrap();
        assert!(sent.unreached_devices.is_empty());
        let (kind, version, ciphertext) = enc_for(&sent.stanza, &bob_jid);
        // Until the recipient replies, every message on a new session is a
        // prekey message (libsignal's unacknowledged-prekey state).
        assert_eq!(kind, "pkmsg");

        let sender = alice_jid.to_protocol_address();
        let received = bob
            .receive(ReceiveRequest {
                chat: &alice_jid.to_non_ad().to_string(),
                sender: &sender,
                kind: if kind == "pkmsg" {
                    ReceiveKind::PreKey
                } else {
                    ReceiveKind::Message
                },
                ciphertext: &ciphertext,
                padding_version: version,
                is_from_me: false,
                device_identity: (kind == "pkmsg").then_some(alice_identity.as_slice()),
            })
            .await
            .unwrap();
        let Received::Message(message) = received else {
            panic!("expected a message")
        };
        assert_eq!(message.content.message.conversation.as_deref(), Some(body));

        let kept = alice.sent_message(&to, &id).await.unwrap().unwrap();
        assert_eq!(kept.conversation.as_deref(), Some(body));
    }

    assert_eq!(alice.prune_sent_messages(0).await.unwrap(), 0);
    assert_eq!(alice.prune_sent_messages(u64::MAX).await.unwrap(), 2);
    assert!(alice.sent_message(&to, "MSG0").await.unwrap().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unpaired_device_refuses_to_send() {
    let records = Arc::new(MemoryRecordStore::new());
    let lease = records
        .acquire("unpaired", "node-a", 0, 60_000)
        .await
        .unwrap()
        .unwrap();
    let ops = SignalOps::create(Arc::new(RecordSignalStore::new(
        records,
        Arc::new(AesGcmSealer::new(&[5; 32])),
        lease.fence,
    )))
    .await
    .unwrap();
    let client = FakeClient {
        devices: Vec::new(),
        bundles: HashMap::new(),
        device_identities: HashMap::new(),
        group: None,
    };
    let to: Jid = "222@s.whatsapp.net".parse().unwrap();
    assert!(
        ops.send_direct(&TokioRuntime, &client, &to, &text("x"), "ID")
            .await
            .is_err()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_send_distributes_the_sender_key_once_then_reuses_it() {
    use wacore::types::message::AddressingMode;

    let group: Jid = "120363000000001@g.us".parse().unwrap();
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    // A primary device, so the primary-device gate is met once it is marked.
    let bob_jid: Jid = "222@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-group-send",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    let bob = device("bob-group-send", "222@s.whatsapp.net").await;
    let alice_identity = common::vouch(&bob, &alice, "111:1@s.whatsapp.net").await;
    let client = FakeClient {
        devices: vec![alice_jid.clone(), bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::new(),
        group: Some(Arc::new(GroupRoutingInfo::new(
            vec![alice_jid.to_non_ad(), bob_jid.clone()],
            AddressingMode::Pn,
        ))),
    };
    let alice_addr = alice_jid.to_protocol_address();
    let group_str = group.to_string();
    let receive = |kind: &str, version: u8, ciphertext: Vec<u8>| {
        let sender = alice_addr.clone();
        let chat = group_str.clone();
        let kind = ReceiveKind::try_from(kind).expect("enc type");
        let bob = &bob;
        let identity = alice_identity.as_slice();
        async move {
            bob.receive(ReceiveRequest {
                chat: &chat,
                sender: &sender,
                kind,
                ciphertext: &ciphertext,
                padding_version: version,
                is_from_me: false,
                device_identity: (kind == ReceiveKind::PreKey).then_some(identity),
            })
            .await
            .unwrap()
        }
    };

    let first = alice
        .send_group(&TokioRuntime, &client, &group, &text("hello group"), "G1")
        .await
        .unwrap();
    assert_eq!(first.distribution_targets, vec![bob_jid.clone()]);
    let (kind, version, carrier) = pairwise_enc_for(&first.stanza, &bob_jid).expect("key for bob");
    let Received::Message(_) = receive(&kind, version, carrier).await else {
        panic!("sender key carrier was not delivered")
    };
    let (_, version, skmsg) = skmsg_of(&first.stanza);
    let Received::Message(message) = receive("skmsg", version, skmsg).await else {
        panic!("expected the group message")
    };
    assert_eq!(
        message.content.message.conversation.as_deref(),
        Some("hello group")
    );

    assert!(
        alice
            .mark_sender_key_distributed(
                &group,
                &first.distribution_targets,
                first.distribution_epoch,
            )
            .await
            .unwrap()
    );
    let second = alice
        .send_group(&TokioRuntime, &client, &group, &text("second"), "G2")
        .await
        .unwrap();
    assert!(second.distribution_targets.is_empty());
    assert!(pairwise_enc_for(&second.stanza, &bob_jid).is_none());
    let (_, version, skmsg) = skmsg_of(&second.stanza);
    let Received::Message(message) = receive("skmsg", version, skmsg).await else {
        panic!("expected the group message")
    };
    assert_eq!(
        message.content.message.conversation.as_deref(),
        Some("second")
    );

    // A retry receipt from Bob in the group makes the next send redistribute.
    common::pair(&alice).await;
    alice
        .resend(
            &client,
            wacore_signal_ops::RetryRequest {
                chat: &group,
                message_id: "G2",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: wacore_signal_ops::RetryRoute::Group {
                    addressing_mode: AddressingMode::Pn,
                },
                retry_count: 1,
                bundle: None,
            },
        )
        .await
        .unwrap();
    let after_retry = alice
        .send_group(&TokioRuntime, &client, &group, &text("after retry"), "G2b")
        .await
        .unwrap();
    assert_eq!(after_retry.distribution_targets, vec![bob_jid.clone()]);
    assert!(
        alice
            .mark_sender_key_distributed(
                &group,
                &after_retry.distribution_targets,
                after_retry.distribution_epoch,
            )
            .await
            .unwrap()
    );

    alice.forget_sender_key_devices(&group).await.unwrap();
    let third = alice
        .send_group(&TokioRuntime, &client, &group, &text("third"), "G3")
        .await
        .unwrap();
    assert_eq!(third.distribution_targets, vec![bob_jid.clone()]);
    let (_, version, ciphertext) = skmsg_of(&third.stanza);
    assert!(
        bob.receive(ReceiveRequest {
            chat: &group_str,
            sender: &alice_addr,
            kind: ReceiveKind::SenderKey,
            ciphertext: &ciphertext,
            padding_version: version,
            is_from_me: false,
            device_identity: None,
        })
        .await
        .is_err(),
        "the old sender-key chain must not decrypt a message after rotation"
    );
    let (kind, version, carrier) =
        pairwise_enc_for(&third.stanza, &bob_jid).expect("fresh key for bob");
    assert!(matches!(
        receive(&kind, version, carrier).await,
        Received::Message(_)
    ));
    let (_, version, ciphertext) = skmsg_of(&third.stanza);
    assert!(matches!(
        receive("skmsg", version, ciphertext).await,
        Received::Message(_)
    ));
}

#[tokio::test]
async fn a_send_waiting_on_devices_cannot_give_a_removed_member_the_replacement_chain() {
    use std::task::Poll;
    use wacore::types::message::AddressingMode;
    let group: Jid = "12345@g.us".parse().unwrap();
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222@s.whatsapp.net".parse().unwrap();
    let carol_jid: Jid = "333@s.whatsapp.net".parse().unwrap();
    let alice = Arc::new(
        device_with_lid(
            "alice-removal-race",
            &alice_jid.to_string(),
            Some("900111:1@lid"),
        )
        .await,
    );
    let bob = device("bob-removal-race", &bob_jid.to_string()).await;
    let alice_identity = common::vouch(&bob, &*alice, "111:1@s.whatsapp.net").await;
    let carol = device("carol-removal-race", &carol_jid.to_string()).await;
    let alice_identity_carol = common::vouch(&carol, &*alice, "111:1@s.whatsapp.net").await;
    let client = Arc::new(ChangingGroupClient::new(
        FakeClient {
            devices: vec![alice_jid.clone(), bob_jid.clone(), carol_jid.clone()],
            bundles: HashMap::from([
                (bob_jid.clone(), bundle_for(&bob).await),
                (carol_jid.clone(), bundle_for(&carol).await),
            ]),
            device_identities: HashMap::new(),
            group: Some(Arc::new(GroupRoutingInfo::new(
                vec![alice_jid.to_non_ad(), bob_jid.clone(), carol_jid.clone()],
                AddressingMode::Pn,
            ))),
        },
        false,
    ));
    let first = alice
        .send_group(
            &TokioRuntime,
            client.as_ref(),
            &group,
            &text("before removal"),
            "BEFORE",
        )
        .await
        .unwrap();
    receive_group_component(
        &bob,
        &group,
        &alice_jid,
        &alice_identity,
        enc_for(&first.stanza, &bob_jid),
    )
    .await
    .unwrap();
    receive_group_component(
        &bob,
        &group,
        &alice_jid,
        &alice_identity,
        skmsg_of(&first.stanza),
    )
    .await
    .unwrap();
    assert!(
        alice
            .mark_sender_key_distributed(
                &group,
                &first.distribution_targets,
                first.distribution_epoch,
            )
            .await
            .unwrap()
    );
    client
        .pause_devices
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let in_flight = {
        let alice = alice.clone();
        let client = client.clone();
        let group = group.clone();
        tokio::spawn(async move {
            alice
                .send_group(
                    &TokioRuntime,
                    client.as_ref(),
                    &group,
                    &text("in flight"),
                    "INFLIGHT",
                )
                .await
                .unwrap()
        })
    };
    client.entered.notified().await;
    let mut rotation = Box::pin(alice.forget_sender_key_devices(&group));
    // Poll the rotation while the old device snapshot is suspended. A correct
    // send finishes on its old chain before rotation can invalidate it.
    let rotation_progress =
        futures::future::poll_fn(|cx| Poll::Ready(rotation.as_mut().poll(cx))).await;
    {
        let mut current = client.current.write().unwrap();
        current.devices = vec![alice_jid.clone(), carol_jid.clone()];
        current.group = Some(Arc::new(GroupRoutingInfo::new(
            vec![alice_jid.to_non_ad(), carol_jid.clone()],
            AddressingMode::Pn,
        )));
    }
    client.resume.notify_one();
    let sent = in_flight.await.unwrap();
    match rotation_progress {
        Poll::Ready(result) => result.unwrap(),
        Poll::Pending => rotation.await.unwrap(),
    }
    if let Some(carrier) = pairwise_enc_for(&sent.stanza, &bob_jid) {
        receive_group_component(&bob, &group, &alice_jid, &alice_identity, carrier)
            .await
            .unwrap();
    }
    receive_group_component(
        &bob,
        &group,
        &alice_jid,
        &alice_identity,
        skmsg_of(&sent.stanza),
    )
    .await
    .unwrap();
    let after = alice
        .send_group(
            &TokioRuntime,
            client.as_ref(),
            &group,
            &text("after removal"),
            "AFTER",
        )
        .await
        .unwrap();
    assert!(pairwise_enc_for(&after.stanza, &bob_jid).is_none());
    assert!(
        receive_group_component(
            &bob,
            &group,
            &alice_jid,
            &alice_identity,
            skmsg_of(&after.stanza)
        )
        .await
        .is_err(),
        "a removed participant received the replacement sender key from a stale resolution"
    );
    receive_group_component(
        &carol,
        &group,
        &alice_jid,
        &alice_identity_carol,
        enc_for(&after.stanza, &carol_jid),
    )
    .await
    .unwrap();
    let Received::Message(message) = receive_group_component(
        &carol,
        &group,
        &alice_jid,
        &alice_identity_carol,
        skmsg_of(&after.stanza),
    )
    .await
    .unwrap() else {
        panic!("retained participant did not receive the message")
    };
    assert_eq!(
        message.content.message.conversation.as_deref(),
        Some("after removal")
    );
}

#[tokio::test]
async fn group_encryption_keeps_the_address_selected_before_session_locking() {
    use wacore::types::message::AddressingMode;
    let group: Jid = "12345@g.us".parse().unwrap();
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-mapping-race",
        &alice_jid.to_string(),
        Some("900111:1@lid"),
    )
    .await;
    let bob = device("bob-mapping-race", &bob_jid.to_string()).await;
    let alice_identity = common::vouch(&bob, &alice, "111:1@s.whatsapp.net").await;
    let client = ChangingGroupClient::new(
        FakeClient {
            devices: vec![alice_jid.clone(), bob_jid.clone()],
            bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
            device_identities: HashMap::new(),
            group: Some(Arc::new(GroupRoutingInfo::new(
                vec![alice_jid.to_non_ad(), bob_jid.clone()],
                AddressingMode::Pn,
            ))),
        },
        true,
    );
    let sent = alice
        .send_group(
            &TokioRuntime,
            &client,
            &group,
            &text("frozen address"),
            "FROZEN",
        )
        .await
        .unwrap();
    // A second lookup would change the encryption address to a LID that has
    // no fetched bundle and was never included in the acquired lock set.
    let carrier = pairwise_enc_for(&sent.stanza, &bob_jid)
        .expect("the frozen PN session must deliver the sender key");
    receive_group_component(&bob, &group, &alice_jid, &alice_identity, carrier)
        .await
        .unwrap();
    let Received::Message(received) = receive_group_component(
        &bob,
        &group,
        &alice_jid,
        &alice_identity,
        skmsg_of(&sent.stanza),
    )
    .await
    .unwrap() else {
        panic!("expected the group payload")
    };
    assert_eq!(
        received.content.message.conversation.as_deref(),
        Some("frozen address")
    );
    assert_eq!(
        client
            .mapping_reads
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

/// The first `<enc>` anywhere in the stanza.
fn first_enc(stanza: &[u8]) -> (String, u8, Vec<u8>) {
    fn find(node: &wacore_binary::Node) -> Option<&wacore_binary::Node> {
        if node.tag == "enc" {
            return Some(node);
        }
        node.children()?.iter().find_map(find)
    }
    enc_parts(find(&parse(stanza)).expect("enc"))
}

fn remote_bundle_from(
    bundle: &PreKeyBundle,
    device_identity: Option<Vec<u8>>,
) -> wacore_signal_ops::RemoteBundle {
    let key = |k: &PublicKey| -> [u8; 32] { *k.public_key_bytes() };
    wacore_signal_ops::RemoteBundle {
        registration_id: bundle.registration_id().unwrap(),
        device_id: bundle.device_id().unwrap().into(),
        identity_key: key(bundle.identity_key().unwrap().public_key()),
        signed_prekey_id: bundle.signed_pre_key_id().unwrap().into(),
        signed_prekey: key(bundle.signed_pre_key_public().unwrap()),
        signed_prekey_signature: bundle
            .signed_pre_key_signature()
            .unwrap()
            .try_into()
            .unwrap(),
        prekey: bundle
            .pre_key_id()
            .unwrap()
            .map(|id| (id.into(), key(bundle.pre_key_public().unwrap().unwrap()))),
        device_identity,
    }
}

/// Decrypt the first `<enc>` of a stanza from `from` and return its text.
async fn deliver_message(
    ops: &SignalOps<Store>,
    from: &Jid,
    from_identity: &[u8],
    stanza: Vec<u8>,
) -> wa::Message {
    let (kind, version, ciphertext) = first_enc(&stanza);
    let sender = from.to_protocol_address();
    let chat = from.to_non_ad().to_string();
    let received = ops
        .receive(ReceiveRequest {
            chat: &chat,
            sender: &sender,
            kind: if kind == "pkmsg" {
                ReceiveKind::PreKey
            } else {
                ReceiveKind::Message
            },
            ciphertext: &ciphertext,
            padding_version: version,
            is_from_me: false,
            device_identity: (kind == "pkmsg").then_some(from_identity),
        })
        .await
        .unwrap();
    let Received::Message(message) = received else {
        panic!("expected a message")
    };
    message.content.message
}

async fn deliver(
    ops: &SignalOps<Store>,
    from: &Jid,
    from_identity: &[u8],
    stanza: Vec<u8>,
) -> Option<String> {
    deliver_message(ops, from, from_identity, stanza)
        .await
        .conversation
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_receipt_gets_the_message_re_encrypted() {
    use wacore_signal_ops::{RetryRequest, RetryRoute};

    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let alice = device("alice-retry", "111:1@s.whatsapp.net").await;
    // A retry pkmsg must carry the device identity, which pairing provides.
    common::pair(&alice).await;
    let bob = device("bob-retry", "222:1@s.whatsapp.net").await;
    let bob_paired = common::pair(&bob).await;
    alice
        .trust_account_identity(&bob_jid, bob_paired.account)
        .await
        .unwrap();
    let alice_identity = common::vouch(&bob, &alice, "111:1@s.whatsapp.net").await;
    let client = FakeClient {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::from([(bob_jid.clone(), bob_paired.device_identity.clone())]),
        group: None,
    };
    let to = bob_jid.to_non_ad();
    // Bob never processes the original.
    alice
        .send_direct(&TokioRuntime, &client, &to, &text("lost once"), "R1")
        .await
        .unwrap();

    let retried = alice
        .resend(
            &client,
            RetryRequest {
                chat: &to,
                message_id: "R1",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 1,
                bundle: None,
            },
        )
        .await
        .unwrap();
    let saved = alice.sent_message(&to, "R1").await.unwrap().unwrap();
    let secret = saved
        .message_context_info
        .as_option()
        .and_then(|context| context.message_secret.as_ref())
        .expect("the original send generated a message secret");
    assert_eq!(secret.len(), 32);
    let delivered = deliver_message(&bob, &alice_jid, &alice_identity, retried).await;
    assert_eq!(delivered.conversation.as_deref(), Some("lost once"));
    assert_eq!(
        delivered
            .message_context_info
            .as_option()
            .and_then(|context| context.message_secret.as_ref()),
        Some(secret),
        "retry encryption must preserve the original generated secret"
    );

    // Bob's device was reset: new keys arrive with the next retry receipt.
    let fresh_bob = device("bob-retry-fresh", "222:1@s.whatsapp.net").await;
    let alice_identity_fresh = common::vouch(&fresh_bob, &alice, "111:1@s.whatsapp.net").await;
    // A reset device is re-paired by the same primary, under the same account.
    let fresh_identity = common::pair_under(&fresh_bob, &bob_paired.account_keys).await;
    let fresh_bundle = remote_bundle_from(&bundle_for(&fresh_bob).await, Some(fresh_identity));
    let retried = alice
        .resend(
            &client,
            RetryRequest {
                chat: &to,
                message_id: "R1",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 2,
                bundle: Some(&fresh_bundle),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        deliver(&fresh_bob, &alice_jid, &alice_identity_fresh, retried)
            .await
            .as_deref(),
        Some("lost once")
    );

    let unknown = alice
        .resend(
            &client,
            RetryRequest {
                chat: &to,
                message_id: "NOPE",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 1,
                bundle: None,
            },
        )
        .await;
    assert!(unknown.is_err());
}

/// Holds every prekey fetch until released, like a slow peer lookup.
struct SlowClient {
    inner: FakeClient,
    gate: Arc<tokio::sync::Notify>,
    /// Signalled once a fetch is held at the gate.
    entered: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl SendContextResolver for SlowClient {
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        self.inner.resolve_devices(jids).await
    }

    async fn fetch_prekeys(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        self.gate.notified().await;
        self.inner.fetch_prekeys(jids).await
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<PreKeyFetchOutcome, anyhow::Error> {
        Ok(PreKeyFetchOutcome {
            bundles: self.fetch_prekeys(jids).await?,
            rejected: Vec::new(),
        })
    }

    async fn resolve_group_routing_info(
        &self,
        jid: &Jid,
    ) -> Result<Arc<GroupRoutingInfo>, anyhow::Error> {
        self.inner.resolve_group_routing_info(jid).await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_send_does_not_block_receiving_from_another_peer() {
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let carol_jid: Jid = "333:1@s.whatsapp.net".parse().unwrap();
    let dave_jid: Jid = "444:1@s.whatsapp.net".parse().unwrap();
    let alice = device("alice-slow", "111:1@s.whatsapp.net").await;
    let carol = device("carol-slow", "333:1@s.whatsapp.net").await;
    let dave = Arc::new(device("dave-slow", "444:1@s.whatsapp.net").await);
    let alice_identity = common::vouch(&dave, &alice, "111:1@s.whatsapp.net").await;

    let gate = Arc::new(tokio::sync::Notify::new());
    let entered = Arc::new(tokio::sync::Notify::new());
    let slow = Arc::new(SlowClient {
        inner: FakeClient {
            devices: vec![carol_jid.clone()],
            bundles: HashMap::from([(carol_jid.clone(), bundle_for(&carol).await)]),
            device_identities: HashMap::from([vouch(&dave, &carol, &carol_jid).await]),
            group: None,
        },
        gate: gate.clone(),
        entered: entered.clone(),
    });
    let send = {
        let dave = dave.clone();
        let slow = slow.clone();
        let to = carol_jid.to_non_ad();
        tokio::spawn(async move {
            dave.send_direct(&TokioRuntime, slow.as_ref(), &to, &text("to carol"), "S1")
                .await
        })
    };

    // Dave's send is now held inside the fetch, so the receive below only
    // passes if it really overlaps a blocked send.
    tokio::time::timeout(Duration::from_secs(5), entered.notified())
        .await
        .expect("the send never reached its prekey fetch");
    assert!(!send.is_finished(), "the send must still be blocked");

    // Alice writes to Dave while Dave's send is stuck fetching Carol's keys.
    let to_dave = FakeClient {
        devices: vec![dave_jid.clone()],
        bundles: HashMap::from([(dave_jid.clone(), bundle_for(&dave).await)]),
        device_identities: HashMap::from([vouch(&alice, &dave, &dave_jid).await]),
        group: None,
    };
    let sent = alice
        .send_direct(
            &TokioRuntime,
            &to_dave,
            &dave_jid.to_non_ad(),
            &text("to dave"),
            "A1",
        )
        .await
        .unwrap();
    let received = tokio::time::timeout(
        Duration::from_secs(5),
        deliver(&dave, &alice_jid, &alice_identity, sent.stanza),
    )
    .await
    .expect("receive was blocked by an unrelated send");
    assert_eq!(received.as_deref(), Some("to dave"));

    gate.notify_one();
    send.await.unwrap().unwrap();
}

#[tokio::test]
async fn a_corrupt_record_is_pruned_instead_of_stopping_the_prune() {
    let records = Arc::new(MemoryRecordStore::new());
    let lease = records
        .acquire("prune-corrupt", "node-a", 0, 60_000)
        .await
        .unwrap()
        .unwrap();
    let store = Arc::new(RecordSignalStore::new(
        records,
        Arc::new(AesGcmSealer::new(&[5; 32])),
        lease.fence,
    ));
    let ops = SignalOps::create(store.clone()).await.unwrap();
    use wacore_recordstore::Namespace;
    store
        .put_aux(Namespace::SentMessage, "corrupt", b"x")
        .await
        .unwrap();
    store
        .put_aux(Namespace::DecryptBuffer, "corrupt", b"x")
        .await
        .unwrap();
    // Cutoff 0 keeps every decodable record, so only the corrupt ones go.
    assert_eq!(ops.prune_sent_messages(0).await.unwrap(), 1);
    assert_eq!(ops.prune_decrypt_buffer(0).await.unwrap(), 1);
    assert!(
        store
            .load_aux(Namespace::SentMessage, "corrupt")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .load_aux(Namespace::DecryptBuffer, "corrupt")
            .await
            .unwrap()
            .is_none()
    );
}

#[async_trait]
impl SignalResolver for SlowClient {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        self.entered.notify_one();
        self.gate.notified().await;
        self.inner.fetch_verifiable_prekeys(jids).await
    }
}

/// Maps phone user `222` to LID `900222` and holds no prekey bundles, so a
/// send can only reach that user through a session that already exists.
struct MappedClient {
    inner: FakeClient,
}

#[async_trait]
impl SendContextResolver for MappedClient {
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, jids: &[Jid]) -> anyhow::Result<Vec<Jid>> {
        self.inner.resolve_devices(jids).await
    }

    async fn fetch_prekeys(&self, jids: &[Jid]) -> anyhow::Result<HashMap<Jid, PreKeyBundle>> {
        self.inner.fetch_prekeys(jids).await
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> anyhow::Result<PreKeyFetchOutcome> {
        self.inner.fetch_prekeys_for_identity_check(jids).await
    }

    async fn resolve_group_routing_info(&self, jid: &Jid) -> anyhow::Result<Arc<GroupRoutingInfo>> {
        self.inner.resolve_group_routing_info(jid).await
    }

    async fn get_lid_for_phone(&self, user: &str) -> Option<wacore_binary::CompactString> {
        (user == "222").then(|| "900222".into())
    }
}

#[async_trait]
impl SignalResolver for MappedClient {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        self.inner.fetch_verifiable_prekeys(jids).await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_send_reuses_an_older_pn_session_when_the_mapped_lid_has_none() {
    use wacore::types::message::AddressingMode;
    let group: Jid = "120363000000002@g.us".parse().unwrap();
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-pn-fallback",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    let bob = device("bob-pn-fallback", "222@s.whatsapp.net").await;
    let alice_identity = common::vouch(&bob, &alice, "111:1@s.whatsapp.net").await;

    // A PN session from before the LID mapping was known.
    let dm_client = FakeClient {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::new(),
        group: None,
    };
    let dm = alice
        .send_direct(
            &TokioRuntime,
            &dm_client,
            &bob_jid.to_non_ad(),
            &text("before"),
            "DM1",
        )
        .await
        .unwrap();
    let (kind, version, ciphertext) = enc_for(&dm.stanza, &bob_jid);
    let dm_chat = alice_jid.to_non_ad().to_string();
    receive_group_component(
        &bob,
        &dm_chat.parse().unwrap(),
        &alice_jid,
        &alice_identity,
        (kind, version, ciphertext),
    )
    .await
    .unwrap();

    // The frozen map now points bob at a LID with no session and no bundle.
    // The send must still deliver the sender key over the PN session, which
    // `SignalOps` locks together with the mapped LID address.
    let client = MappedClient {
        inner: FakeClient {
            devices: vec![alice_jid.clone(), bob_jid.clone()],
            bundles: HashMap::new(),
            device_identities: HashMap::new(),
            group: Some(Arc::new(GroupRoutingInfo::new(
                vec![alice_jid.to_non_ad(), bob_jid.clone()],
                AddressingMode::Pn,
            ))),
        },
    };
    let sent = alice
        .send_group(&TokioRuntime, &client, &group, &text("after mapping"), "G1")
        .await
        .unwrap();
    assert_eq!(sent.distribution_targets, vec![bob_jid.clone()]);
    let carrier = pairwise_enc_for(&sent.stanza, &bob_jid).expect("the PN session carries the key");
    receive_group_component(&bob, &group, &alice_jid, &alice_identity, carrier)
        .await
        .unwrap();
    let Received::Message(received) = receive_group_component(
        &bob,
        &group,
        &alice_jid,
        &alice_identity,
        skmsg_of(&sent.stanza),
    )
    .await
    .unwrap() else {
        panic!("expected the group payload")
    };
    assert_eq!(
        received.content.message.conversation.as_deref(),
        Some("after mapping")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_acknowledgment_from_before_a_rotation_does_not_mark_the_new_key() {
    use wacore::types::message::AddressingMode;

    let group: Jid = "120363000000003@g.us".parse().unwrap();
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-stale-ack",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    let bob = device("bob-stale-ack", "222@s.whatsapp.net").await;
    let client = FakeClient {
        devices: vec![alice_jid.clone(), bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::new(),
        group: Some(Arc::new(GroupRoutingInfo::new(
            vec![alice_jid.to_non_ad(), bob_jid.clone()],
            AddressingMode::Pn,
        ))),
    };
    let send = |text_body: &'static str, id: &'static str| {
        let (alice, client, group) = (&alice, &client, &group);
        async move {
            alice
                .send_group(&TokioRuntime, client, group, &text(text_body), id)
                .await
                .unwrap()
        }
    };

    let first = send("before rotation", "S1").await;
    assert_eq!(first.distribution_targets, vec![bob_jid.clone()]);
    alice.forget_sender_key_devices(&group).await.unwrap();
    // The first send with the replacement key is never acknowledged.
    let lost = send("lost", "S2").await;
    assert_eq!(lost.distribution_targets, vec![bob_jid.clone()]);
    assert_ne!(lost.distribution_epoch, first.distribution_epoch);

    // The old send's acknowledgment arrives late and must record nothing.
    assert!(
        !alice
            .mark_sender_key_distributed(
                &group,
                &first.distribution_targets,
                first.distribution_epoch
            )
            .await
            .unwrap()
    );
    let next = send("after the late ack", "S3").await;
    assert_eq!(
        next.distribution_targets,
        vec![bob_jid.clone()],
        "bob never acknowledged the replacement key"
    );
    assert!(
        alice
            .mark_sender_key_distributed(
                &group,
                &next.distribution_targets,
                next.distribution_epoch
            )
            .await
            .unwrap()
    );
    assert!(send("warm", "S4").await.distribution_targets.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_for_a_device_outside_the_conversation_is_refused() {
    use wacore::types::message::AddressingMode;
    use wacore_signal_ops::{OpsError, RetryRequest, RetryRoute};

    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let eve_jid: Jid = "333:1@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-retry-check",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    common::pair(&alice).await;
    let bob = device("bob-retry-check", "222:1@s.whatsapp.net").await;
    let eve = device("eve-retry-check", "333:1@s.whatsapp.net").await;
    let eve_bundle = remote_bundle_from(&bundle_for(&eve).await, None);
    let group: Jid = "120363000000004@g.us".parse().unwrap();
    let client = FakeClient {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::from([vouch(&alice, &bob, &bob_jid).await]),
        group: Some(Arc::new(GroupRoutingInfo::new(
            vec!["111@s.whatsapp.net".parse().unwrap(), bob_jid.to_non_ad()],
            AddressingMode::Pn,
        ))),
    };
    let to = bob_jid.to_non_ad();
    alice
        .send_direct(&TokioRuntime, &client, &to, &text("for bob only"), "D1")
        .await
        .unwrap();
    alice
        .send_group(&TokioRuntime, &client, &group, &text("members only"), "G1")
        .await
        .unwrap();

    let refused = |result: Result<Vec<u8>, OpsError>| {
        assert!(
            matches!(result, Err(OpsError::InvalidInput(_))),
            "expected a refused retry, got {result:?}"
        );
    };
    // A stranger asks for the direct message with its own keys.
    refused(
        alice
            .resend(
                &client,
                RetryRequest {
                    chat: &to,
                    message_id: "D1",
                    requester: &eve_jid,
                    encryption_jid: &eve_jid,
                    route: RetryRoute::Direct { recipient: None },
                    retry_count: 1,
                    bundle: Some(&eve_bundle),
                },
            )
            .await,
    );
    // The real recipient is named, but the encryption address is someone else's.
    refused(
        alice
            .resend(
                &client,
                RetryRequest {
                    chat: &to,
                    message_id: "D1",
                    requester: &bob_jid,
                    encryption_jid: &eve_jid,
                    route: RetryRoute::Direct { recipient: None },
                    retry_count: 1,
                    bundle: Some(&eve_bundle),
                },
            )
            .await,
    );
    // A non-member asks for the group message.
    refused(
        alice
            .resend(
                &client,
                RetryRequest {
                    chat: &group,
                    message_id: "G1",
                    requester: &eve_jid,
                    encryption_jid: &eve_jid,
                    route: RetryRoute::Group {
                        addressing_mode: AddressingMode::Pn,
                    },
                    retry_count: 1,
                    bundle: Some(&eve_bundle),
                },
            )
            .await,
    );
    // Nothing was written for the stranger.
    assert!(
        !alice
            .has_session(&eve_jid.to_protocol_address())
            .await
            .unwrap()
    );

    // Bob asking from his LID is the same account as the PN chat.
    let bob_lid: Jid = "900222:1@lid".parse().unwrap();
    // The LID form of the same account carries Bob's own device identity.
    let bob_paired = common::pair(&bob).await;
    alice
        .trust_account_identity(&bob_lid, bob_paired.account)
        .await
        .unwrap();
    let bob_lid_bundle =
        remote_bundle_from(&bundle_for(&bob).await, Some(bob_paired.device_identity));
    let mapped = MappedClient { inner: client };
    let retried = alice
        .resend(
            &mapped,
            RetryRequest {
                chat: &to,
                message_id: "D1",
                requester: &bob_lid,
                encryption_jid: &bob_lid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 1,
                bundle: Some(&bob_lid_bundle),
            },
        )
        .await;
    assert!(retried.is_ok(), "the recipient's LID form is accepted");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_group_retry_is_answered_only_for_members_the_message_went_to() {
    use wacore::types::message::AddressingMode;
    use wacore_signal_ops::{OpsError, RetryRequest, RetryRoute};

    let alice_member: Jid = "111@s.whatsapp.net".parse().unwrap();
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let carol_jid: Jid = "444:1@s.whatsapp.net".parse().unwrap();
    let eve_jid: Jid = "333:1@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-audience",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    common::pair(&alice).await;
    let bob = device("bob-audience", "222:1@s.whatsapp.net").await;
    let carol = device("carol-audience", "444:1@s.whatsapp.net").await;
    let eve = device("eve-audience", "333:1@s.whatsapp.net").await;
    let group: Jid = "120363000000005@g.us".parse().unwrap();
    let identities = HashMap::from([
        vouch(&alice, &bob, &bob_jid).await,
        vouch(&alice, &carol, &carol_jid).await,
    ]);
    let client_for = |members: Vec<Jid>, bundles: HashMap<Jid, PreKeyBundle>| FakeClient {
        devices: vec![bob_jid.clone(), carol_jid.clone()],
        bundles,
        device_identities: identities.clone(),
        group: Some(Arc::new(GroupRoutingInfo::new(members, AddressingMode::Pn))),
    };
    let bundles = HashMap::from([
        (bob_jid.clone(), bundle_for(&bob).await),
        (carol_jid.clone(), bundle_for(&carol).await),
    ]);
    let before = client_for(
        vec![
            alice_member.clone(),
            bob_jid.to_non_ad(),
            carol_jid.to_non_ad(),
        ],
        bundles,
    );
    alice
        .send_group(&TokioRuntime, &before, &group, &text("before eve"), "G1")
        .await
        .unwrap();

    // Eve joined and Bob left after the send.
    let after = client_for(
        vec![alice_member, carol_jid.to_non_ad(), eve_jid.to_non_ad()],
        HashMap::new(),
    );
    fn retry<'a>(
        group: &'a Jid,
        requester: &'a Jid,
        bundle: Option<&'a wacore_signal_ops::RemoteBundle>,
    ) -> RetryRequest<'a> {
        RetryRequest {
            chat: group,
            message_id: "G1",
            requester,
            encryption_jid: requester,
            route: RetryRoute::Group {
                addressing_mode: AddressingMode::Pn,
            },
            retry_count: 1,
            bundle,
        }
    }
    let eve_bundle = remote_bundle_from(&bundle_for(&eve).await, None);
    let bob_bundle = remote_bundle_from(&bundle_for(&bob).await, None);
    assert!(
        matches!(
            alice
                .resend(&after, retry(&group, &eve_jid, Some(&eve_bundle)))
                .await,
            Err(OpsError::InvalidInput(_))
        ),
        "a member who joined later gets no earlier message"
    );
    assert!(
        matches!(
            alice
                .resend(&after, retry(&group, &bob_jid, Some(&bob_bundle)))
                .await,
            Err(OpsError::InvalidInput(_))
        ),
        "a removed member gets nothing more"
    );
    let retried = alice.resend(&after, retry(&group, &carol_jid, None)).await;
    assert!(
        retried.is_ok(),
        "a current recipient is answered: {retried:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_whose_route_does_not_match_its_chat_is_refused() {
    use wacore::types::message::AddressingMode;
    use wacore_signal_ops::{OpsError, RetryRequest, RetryRoute};

    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let alice = device_with_lid(
        "alice-route-check",
        "111:1@s.whatsapp.net",
        Some("900111:1@lid"),
    )
    .await;
    common::pair(&alice).await;
    let bob = device("bob-route-check", "222:1@s.whatsapp.net").await;
    let group: Jid = "120363000000006@g.us".parse().unwrap();
    let client = FakeClient {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(bob_jid.clone(), bundle_for(&bob).await)]),
        device_identities: HashMap::from([vouch(&alice, &bob, &bob_jid).await]),
        group: Some(Arc::new(GroupRoutingInfo::new(
            vec!["111@s.whatsapp.net".parse().unwrap(), bob_jid.to_non_ad()],
            AddressingMode::Pn,
        ))),
    };
    let to = bob_jid.to_non_ad();
    alice
        .send_direct(&TokioRuntime, &client, &to, &text("direct"), "D1")
        .await
        .unwrap();
    alice
        .send_group(&TokioRuntime, &client, &group, &text("group"), "G1")
        .await
        .unwrap();

    let retry = |chat: &Jid, message_id: &'static str, route: RetryRoute| {
        let (alice, client, bob_jid) = (&alice, &client, &bob_jid);
        let chat = chat.clone();
        async move {
            alice
                .resend(
                    client,
                    RetryRequest {
                        chat: &chat,
                        message_id,
                        requester: bob_jid,
                        encryption_jid: bob_jid,
                        route,
                        retry_count: 1,
                        bundle: None,
                    },
                )
                .await
        }
    };
    let direct = || RetryRoute::Direct { recipient: None };
    let in_group = || RetryRoute::Group {
        addressing_mode: AddressingMode::Pn,
    };

    // A saved group message cannot be re-sent through the direct route.
    let result = retry(&group, "G1", direct()).await;
    assert!(
        matches!(result, Err(OpsError::InvalidInput(_))),
        "expected a refused retry, got {result:?}"
    );
    // A saved direct message cannot be re-sent through the group route.
    let result = retry(&to, "D1", in_group()).await;
    assert!(
        matches!(result, Err(OpsError::InvalidInput(_))),
        "expected a refused retry, got {result:?}"
    );
    // A broadcast-list route does not fit either chat.
    let result = retry(&to, "D1", RetryRoute::BroadcastList).await;
    assert!(
        matches!(result, Err(OpsError::InvalidInput(_))),
        "expected a refused retry, got {result:?}"
    );

    // The matching routes still work.
    assert!(retry(&to, "D1", direct()).await.is_ok());
    assert!(retry(&group, "G1", in_group()).await.is_ok());
}
