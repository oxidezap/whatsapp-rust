// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Buffered receive: idempotent redelivery, atomic commits, sender keys and
//! the app-state key share gate, with real padded WhatsApp messages.

use futures::executor::block_on;
use std::sync::Arc;
use wacore::libsignal::protocol::{DeviceId, KeyPair, ProtocolAddress};
use wacore::messages::MessageUtils;
use wacore_recordstore::{
    AesGcmSealer, Fence, LeaseStore, MemoryRecordStore, Namespace, RecordSignalStore, RecordStore,
    WriteOp,
};
use wacore_signal_ops::{
    Encrypted, ReceiveKind, ReceiveRequest, Received, ReceivedMessage, RemoteBundle, SignalOps,
};
use waproto::whatsapp as wa;

mod common;
mod runtime;
use runtime::InlineRuntime;

type Store = RecordSignalStore<MemoryRecordStore, AesGcmSealer>;

const TTL: u64 = 60_000;
const GROUP: &str = "120363000000001@g.us";

struct Device {
    records: Arc<MemoryRecordStore>,
    scope: &'static str,
    ops: SignalOps<Store>,
}

fn store_for(records: &Arc<MemoryRecordStore>, scope: &str, holder: &str, now: u64) -> Arc<Store> {
    let lease = block_on(records.acquire(scope, holder, now, TTL))
        .unwrap()
        .expect("lease available");
    Arc::new(RecordSignalStore::new(
        records.clone(),
        Arc::new(AesGcmSealer::new(&[9; 32])),
        lease.fence,
    ))
}

fn device(scope: &'static str) -> Device {
    let records = Arc::new(MemoryRecordStore::new());
    let ops = block_on(SignalOps::create(store_for(&records, scope, "node-a", 0))).unwrap();
    Device {
        records,
        scope,
        ops,
    }
}

fn address(user: &str) -> ProtocolAddress {
    ProtocolAddress::new(&format!("{user}@s.whatsapp.net"), DeviceId::from(1))
}

thread_local! {
    /// The `<device-identity>` each test sender's prekey messages carry, by
    /// sender address. Leaked so a request can borrow it for any lifetime;
    /// each test runs on its own thread.
    static SENDER_IDENTITIES: std::cell::RefCell<std::collections::HashMap<String, &'static [u8]>> =
        std::cell::RefCell::default();
}

/// Have `receiver` pin `sender`'s account and remember the identity its
/// prekey messages must carry (see [`request`]).
fn accept_sender<R: wacore_signal_ops::ServiceStore>(
    receiver: &SignalOps<R>,
    sender: &SignalOps<Store>,
    sender_addr: &ProtocolAddress,
) {
    let identity = block_on(common::vouch(receiver, sender, sender_addr.name()));
    SENDER_IDENTITIES.with(|map| {
        map.borrow_mut().insert(
            sender_addr.name().to_owned(),
            Box::leak(identity.into_boxed_slice()),
        );
    });
}

fn connect(from: &Device, to: &Device, from_addr: &ProtocolAddress, to_addr: &ProtocolAddress) {
    accept_sender(&to.ops, &from.ops, from_addr);
    let device_identity = block_on(common::vouch(&from.ops, &to.ops, to_addr.name()));
    let identity = block_on(to.ops.public_identity());
    let prekey = block_on(to.ops.generate_prekeys(&InlineRuntime, 1))
        .unwrap()
        .remove(0);
    block_on(from.ops.establish_session(
        to_addr,
        &RemoteBundle {
            registration_id: identity.registration_id,
            device_id: 1,
            identity_key: identity.identity_key,
            signed_prekey_id: identity.signed_prekey.id,
            signed_prekey: identity.signed_prekey.public_key,
            signed_prekey_signature: identity.signed_prekey.signature,
            prekey: Some((prekey.id, prekey.public_key)),
            device_identity: Some(device_identity),
        },
    ))
    .unwrap();
}

// Test-only proto encoding; the repo allows it in tests.
#[allow(clippy::disallowed_methods)]
fn padded(message: &wa::Message) -> Vec<u8> {
    MessageUtils::pad_message_v2(waproto::codec::message_to_vec(message))
}

fn text(body: &str) -> wa::Message {
    let mut message = wa::Message::default();
    message.conversation = Some(body.to_owned());
    message
}

fn kind(encrypted: &Encrypted) -> ReceiveKind {
    match encrypted.kind {
        wacore_signal_ops::EncKind::PreKey => ReceiveKind::PreKey,
        wacore_signal_ops::EncKind::Message => ReceiveKind::Message,
    }
}

fn request<'a>(
    chat: &'a str,
    sender: &'a ProtocolAddress,
    kind: ReceiveKind,
    ciphertext: &'a [u8],
    is_from_me: bool,
) -> ReceiveRequest<'a> {
    ReceiveRequest {
        chat,
        sender,
        kind,
        ciphertext,
        padding_version: 2,
        is_from_me,
        // Only a prekey message carries the sender's device identity.
        device_identity: (kind == ReceiveKind::PreKey)
            .then(|| SENDER_IDENTITIES.with(|map| map.borrow().get(sender.name()).copied()))
            .flatten(),
    }
}

fn message(received: Received) -> ReceivedMessage {
    match received {
        Received::Message(message) => *message,
        Received::AlreadyDelivered { .. } => panic!("expected a message"),
    }
}

#[test]
fn redelivery_returns_the_buffered_result_until_delivered() {
    let alice = device("alice-redeliver");
    let bob = device("bob-redeliver");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);

    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("hello")))).unwrap();
    let chat = alice_addr.name().to_owned();
    let req = request(&chat, &alice_addr, kind(&sent), &sent.ciphertext, false);

    let first = message(block_on(bob.ops.receive(req)).unwrap());
    assert_eq!(first.content.message.conversation.as_deref(), Some("hello"));
    assert!(!first.redelivered);

    let again = message(block_on(bob.ops.receive(req)).unwrap());
    assert!(again.redelivered);
    assert_eq!(again.receipt_key, first.receipt_key);
    assert_eq!(again.content.message.conversation.as_deref(), Some("hello"));

    block_on(bob.ops.mark_delivered(&first.receipt_key)).unwrap();
    assert!(matches!(
        block_on(bob.ops.receive(req)).unwrap(),
        Received::AlreadyDelivered { .. }
    ));
}

#[test]
fn a_fenced_out_receive_leaves_the_ciphertext_decryptable_by_the_successor() {
    let alice = device("alice-atomic");
    let bob = device("bob-atomic");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);
    let chat = alice_addr.name().to_owned();

    let first = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("one")))).unwrap();
    block_on(bob.ops.receive(request(
        &chat,
        &alice_addr,
        kind(&first),
        &first.ciphertext,
        false,
    )))
    .unwrap();

    let second = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("two")))).unwrap();
    let successor = store_for(&bob.records, bob.scope, "node-b", TTL + 1);
    let stale = block_on(bob.ops.receive(request(
        &chat,
        &alice_addr,
        kind(&second),
        &second.ciphertext,
        false,
    )));
    assert!(stale.expect_err("fenced out").is_fence_lost());

    // The stale node's ratchet advance never committed, so nothing was lost.
    let bob_b = block_on(SignalOps::open(successor)).unwrap();
    let got = message(
        block_on(bob_b.receive(request(
            &chat,
            &alice_addr,
            kind(&second),
            &second.ciphertext,
            false,
        )))
        .unwrap(),
    );
    assert_eq!(got.content.message.conversation.as_deref(), Some("two"));
}

#[test]
fn a_sender_key_carried_in_a_pairwise_message_unlocks_group_traffic() {
    let alice = device("alice-skdm");
    let bob = device("bob-skdm");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);

    let skdm = block_on(alice.ops.sender_key_distribution(GROUP, &alice_addr)).unwrap();
    let mut distribution = wa::message::SenderKeyDistributionMessage::default();
    distribution.group_id = Some(GROUP.to_owned());
    distribution.axolotl_sender_key_distribution_message = Some(skdm);
    let mut carrier = wa::Message::default();
    carrier.sender_key_distribution_message = Some(distribution).into();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&carrier))).unwrap();
    let received = message(
        block_on(bob.ops.receive(request(
            GROUP,
            &alice_addr,
            kind(&sent),
            &sent.ciphertext,
            false,
        )))
        .unwrap(),
    );
    assert!(received.content.is_skdm_only);

    let skmsg = block_on(
        alice
            .ops
            .group_encrypt(GROUP, &alice_addr, &padded(&text("hi group"))),
    )
    .unwrap();
    let group = message(
        block_on(bob.ops.receive(request(
            GROUP,
            &alice_addr,
            ReceiveKind::SenderKey,
            &skmsg,
            false,
        )))
        .unwrap(),
    );
    assert_eq!(
        group.content.message.conversation.as_deref(),
        Some("hi group")
    );
}

#[test]
fn only_own_devices_can_hand_over_app_state_keys() {
    let alice = device("alice-keys");
    let bob = device("bob-keys");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.app_state_sync_key_share = Some(wa::message::AppStateSyncKeyShare::default()).into();
    let mut share = wa::Message::default();
    share.protocol_message = Some(protocol).into();
    let chat = alice_addr.name().to_owned();

    let from_peer = block_on(alice.ops.encrypt(&bob_addr, &padded(&share))).unwrap();
    let peer = message(
        block_on(bob.ops.receive(request(
            &chat,
            &alice_addr,
            kind(&from_peer),
            &from_peer.ciphertext,
            false,
        )))
        .unwrap(),
    );
    assert!(peer.app_state_key_share.is_none());
    assert!(
        peer.content
            .message
            .protocol_message
            .as_option()
            .is_none_or(|p| p.app_state_sync_key_share.as_option().is_none()),
        "a peer's key share must not reach the app inside the message"
    );

    // The client claims Alice is one of Bob's own devices: refused, so her
    // share cannot pass as Bob's own.
    let claimed = block_on(alice.ops.encrypt(&bob_addr, &padded(&share))).unwrap();
    assert!(matches!(
        block_on(bob.ops.receive(request(
            &chat,
            &alice_addr,
            kind(&claimed),
            &claimed.ciphertext,
            true,
        ))),
        Err(wacore_signal_ops::OpsError::InvalidInput(_))
    ));

    block_on(bob.ops.set_own_jids("222:1@s.whatsapp.net", None)).unwrap();

    // Bob's own second device, paired under Bob's account, may hand it over.
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let bob_identity = block_on(common::pair_under(&bob.ops, &account));
    let second = device("bob-keys-second");
    block_on(second.ops.set_own_jids("222:2@s.whatsapp.net", None)).unwrap();
    let second_identity = block_on(common::pair_under(&second.ops, &account));
    let identity = block_on(bob.ops.public_identity());
    let prekey = block_on(bob.ops.generate_prekeys(&InlineRuntime, 1))
        .unwrap()
        .remove(0);
    block_on(second.ops.establish_session(
        &bob_addr,
        &RemoteBundle {
            registration_id: identity.registration_id,
            device_id: 1,
            identity_key: identity.identity_key,
            signed_prekey_id: identity.signed_prekey.id,
            signed_prekey: identity.signed_prekey.public_key,
            signed_prekey_signature: identity.signed_prekey.signature,
            prekey: Some((prekey.id, prekey.public_key)),
            device_identity: Some(bob_identity),
        },
    ))
    .unwrap();
    let second_addr = ProtocolAddress::new("222@s.whatsapp.net", DeviceId::from(2));
    let from_self = block_on(second.ops.encrypt(&bob_addr, &padded(&share))).unwrap();
    let own = message(
        block_on(bob.ops.receive(ReceiveRequest {
            chat: &chat,
            sender: &second_addr,
            kind: kind(&from_self),
            ciphertext: &from_self.ciphertext,
            padding_version: 2,
            is_from_me: true,
            device_identity: Some(&second_identity),
        }))
        .unwrap(),
    );
    assert!(own.app_state_key_share.is_some());
}

#[test]
fn an_unpaired_account_takes_no_app_state_keys_from_its_own_number() {
    let bob = device("bob-unpaired-keys");
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.app_state_sync_key_share = Some(wa::message::AppStateSyncKeyShare::default()).into();
    let mut share = wa::Message::default();
    share.protocol_message = Some(protocol).into();
    let chat = "222@s.whatsapp.net".to_owned();
    // Before pairing, a sender that only names Bob's own number proves
    // nothing: another device poses as Bob's primary.
    block_on(bob.ops.set_own_jids("222:1@s.whatsapp.net", None)).unwrap();
    let poser = device("bob-keys-poser");
    let own_primary = ProtocolAddress::new("222@s.whatsapp.net", DeviceId::from(0));
    let bob_public = block_on(bob.ops.public_identity());
    let bob_prekey = block_on(bob.ops.generate_prekeys(&InlineRuntime, 1))
        .unwrap()
        .remove(0);
    // The poser addresses Bob as a primary, which needs no device identity.
    let bob_primary = ProtocolAddress::new("222@s.whatsapp.net", DeviceId::from(0));
    block_on(poser.ops.establish_session(
        &bob_primary,
        &RemoteBundle {
            registration_id: bob_public.registration_id,
            device_id: 0,
            identity_key: bob_public.identity_key,
            signed_prekey_id: bob_public.signed_prekey.id,
            signed_prekey: bob_public.signed_prekey.public_key,
            signed_prekey_signature: bob_public.signed_prekey.signature,
            prekey: Some((bob_prekey.id, bob_prekey.public_key)),
            device_identity: None,
        },
    ))
    .unwrap();
    let posed = block_on(poser.ops.encrypt(&bob_primary, &padded(&share))).unwrap();
    let posed_request = |is_from_me| ReceiveRequest {
        chat: &chat,
        sender: &own_primary,
        kind: kind(&posed),
        ciphertext: &posed.ciphertext,
        padding_version: 2,
        is_from_me,
        device_identity: None,
    };
    assert!(matches!(
        block_on(bob.ops.receive(posed_request(true))),
        Err(wacore_signal_ops::OpsError::InvalidInput(_))
    ));
    let posed = message(block_on(bob.ops.receive(posed_request(false))).unwrap());
    assert!(
        posed.app_state_key_share.is_none(),
        "an unpaired account has no own devices to take keys from"
    );
}

#[test]
fn pruning_drops_old_delivered_markers_and_keeps_pending_results() {
    let alice = device("alice-prune");
    let bob = device("bob-prune");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);
    let chat = alice_addr.name().to_owned();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("old")))).unwrap();
    let req = request(&chat, &alice_addr, kind(&sent), &sent.ciphertext, false);
    let first = message(block_on(bob.ops.receive(req)).unwrap());

    // Not yet delivered: the buffer holds the only copy of the plaintext.
    assert_eq!(block_on(bob.ops.prune_decrypt_buffer(u64::MAX)).unwrap(), 0);
    assert!(message(block_on(bob.ops.receive(req)).unwrap()).redelivered);

    block_on(bob.ops.mark_delivered(&first.receipt_key)).unwrap();
    assert_eq!(block_on(bob.ops.prune_decrypt_buffer(0)).unwrap(), 0);
    assert_eq!(block_on(bob.ops.prune_decrypt_buffer(u64::MAX)).unwrap(), 1);
}

#[test]
fn a_bad_sender_key_does_not_block_the_message_carrying_it() {
    let alice = device("alice-bad-skdm");
    let bob = device("bob-bad-skdm");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);
    let mut distribution = wa::message::SenderKeyDistributionMessage::default();
    distribution.group_id = Some(GROUP.to_owned());
    distribution.axolotl_sender_key_distribution_message = Some(vec![0xde, 0xad]);
    let mut carrier = wa::Message::default();
    carrier.conversation = Some("still readable".to_owned());
    carrier.sender_key_distribution_message = Some(distribution).into();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&carrier))).unwrap();
    let req = request(GROUP, &alice_addr, kind(&sent), &sent.ciphertext, false);
    let first = message(block_on(bob.ops.receive(req)).unwrap());
    assert!(first.sender_key_rejected);
    assert_eq!(
        first.content.message.conversation.as_deref(),
        Some("still readable")
    );
    // Committed: a redelivery is served from the buffer, not decrypted again.
    let again = message(block_on(bob.ops.receive(req)).unwrap());
    assert!(again.redelivered);
    assert!(
        again.sender_key_rejected,
        "a redelivery must still report the rejected distribution"
    );
}

/// Fails the next sender-key read, as a transient backend error would.
struct FlakySenderKeys {
    inner: Arc<MemoryRecordStore>,
    fail: std::sync::atomic::AtomicBool,
}

impl FlakySenderKeys {
    fn check(&self, ns: Namespace) -> wacore::store::error::Result<()> {
        if ns == Namespace::SenderKey && self.fail.swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(wacore::store::error::StoreError::Validation(
                "backend unavailable".into(),
            ));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl RecordStore for FlakySenderKeys {
    async fn get(
        &self,
        scope: &str,
        ns: Namespace,
        key: &str,
    ) -> wacore::store::error::Result<Option<Vec<u8>>> {
        self.check(ns)?;
        self.inner.get(scope, ns, key).await
    }

    async fn get_many(
        &self,
        scope: &str,
        ns: Namespace,
        keys: &[&str],
    ) -> wacore::store::error::Result<Vec<(String, Vec<u8>)>> {
        self.check(ns)?;
        self.inner.get_many(scope, ns, keys).await
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> wacore::store::error::Result<Vec<String>> {
        self.inner.scan_keys(scope, ns, prefix, limit).await
    }

    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> wacore::store::error::Result<()> {
        self.inner.write(fence, ops).await
    }
}

#[test]
fn a_storage_failure_installing_a_sender_key_is_retried_on_redelivery() {
    let alice = device("alice-flaky");
    let records = Arc::new(FlakySenderKeys {
        inner: Arc::new(MemoryRecordStore::new()),
        fail: std::sync::atomic::AtomicBool::new(false),
    });
    let lease = block_on(records.inner.acquire("bob-flaky", "node-a", 0, TTL))
        .unwrap()
        .expect("lease available");
    let bob = block_on(SignalOps::create(Arc::new(RecordSignalStore::new(
        records.clone(),
        Arc::new(AesGcmSealer::new(&[9; 32])),
        lease.fence,
    ))))
    .unwrap();
    let (alice_addr, bob_addr) = (address("111"), address("222"));

    let device_identity = block_on(common::vouch(&alice.ops, &bob, bob_addr.name()));
    accept_sender(&bob, &alice.ops, &alice_addr);
    let identity = block_on(bob.public_identity());
    let prekey = block_on(bob.generate_prekeys(&InlineRuntime, 1))
        .unwrap()
        .remove(0);
    block_on(alice.ops.establish_session(
        &bob_addr,
        &RemoteBundle {
            registration_id: identity.registration_id,
            device_id: 1,
            identity_key: identity.identity_key,
            signed_prekey_id: identity.signed_prekey.id,
            signed_prekey: identity.signed_prekey.public_key,
            signed_prekey_signature: identity.signed_prekey.signature,
            prekey: Some((prekey.id, prekey.public_key)),
            device_identity: Some(device_identity),
        },
    ))
    .unwrap();

    let skdm = block_on(alice.ops.sender_key_distribution(GROUP, &alice_addr)).unwrap();
    let mut distribution = wa::message::SenderKeyDistributionMessage::default();
    distribution.group_id = Some(GROUP.to_owned());
    distribution.axolotl_sender_key_distribution_message = Some(skdm);
    let mut carrier = wa::Message::default();
    carrier.sender_key_distribution_message = Some(distribution).into();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&carrier))).unwrap();
    let req = request(GROUP, &alice_addr, kind(&sent), &sent.ciphertext, false);

    records
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(
        block_on(bob.receive(req)).is_err(),
        "a storage failure must not commit the message without its key"
    );
    let retried = message(block_on(bob.receive(req)).unwrap());
    assert!(
        !retried.redelivered,
        "nothing was buffered by the failed attempt"
    );
    assert!(!retried.sender_key_rejected);

    let skmsg = block_on(alice.ops.group_encrypt(
        GROUP,
        &alice_addr,
        &padded(&text("after retry")),
    ))
    .unwrap();
    let group = message(
        block_on(bob.receive(request(
            GROUP,
            &alice_addr,
            ReceiveKind::SenderKey,
            &skmsg,
            false,
        )))
        .unwrap(),
    );
    assert_eq!(
        group.content.message.conversation.as_deref(),
        Some("after retry")
    );
}

#[test]
fn a_sender_key_for_another_group_is_not_installed_under_this_chat() {
    const OTHER: &str = "120363000000009@g.us";
    let alice = device("alice-other-group");
    let bob = device("bob-other-group");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);

    let skdm = block_on(alice.ops.sender_key_distribution(OTHER, &alice_addr)).unwrap();
    let mut distribution = wa::message::SenderKeyDistributionMessage::default();
    distribution.group_id = Some(OTHER.to_owned());
    distribution.axolotl_sender_key_distribution_message = Some(skdm);
    let mut carrier = wa::Message::default();
    carrier.sender_key_distribution_message = Some(distribution).into();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&carrier))).unwrap();
    let received = message(
        block_on(bob.ops.receive(request(
            GROUP,
            &alice_addr,
            kind(&sent),
            &sent.ciphertext,
            false,
        )))
        .unwrap(),
    );
    assert!(received.sender_key_rejected);

    // The other group's chain was not stored under this chat.
    let skmsg = block_on(alice.ops.group_encrypt(
        OTHER,
        &alice_addr,
        &padded(&text("other group")),
    ))
    .unwrap();
    assert!(
        block_on(bob.ops.receive(request(
            GROUP,
            &alice_addr,
            ReceiveKind::SenderKey,
            &skmsg,
            false,
        )))
        .is_err(),
        "a key for another group must not decrypt this chat's messages"
    );
}

#[test]
fn a_message_that_replaces_the_stored_identity_reports_it() {
    let alice = device("alice-identity-msg");
    let bob = device("bob-identity-msg");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &alice_addr, &bob_addr);
    let chat = alice_addr.name().to_owned();
    let bob_chat = bob_addr.name().to_owned();

    // Establish both directions so Alice's next message is a plain `msg`.
    let first = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("one")))).unwrap();
    block_on(bob.ops.receive(request(
        &chat,
        &alice_addr,
        kind(&first),
        &first.ciphertext,
        false,
    )))
    .unwrap();
    let reply = block_on(bob.ops.encrypt(&alice_addr, &padded(&text("two")))).unwrap();
    block_on(alice.ops.receive(request(
        &bob_chat,
        &bob_addr,
        kind(&reply),
        &reply.ciphertext,
        false,
    )))
    .unwrap();

    // Bob's stored identity for Alice no longer matches the session's.
    let other = block_on(alice.ops.public_identity())
        .signed_prekey
        .public_key;
    let store = store_for(&bob.records, bob.scope, "node-a", 0);
    block_on(wacore::store::traits::SignalStore::put_identity(
        &*store,
        alice_addr.as_str(),
        other,
    ))
    .unwrap();

    let next = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("three")))).unwrap();
    assert!(matches!(kind(&next), ReceiveKind::Message));
    let received = message(
        block_on(bob.ops.receive(request(
            &chat,
            &alice_addr,
            kind(&next),
            &next.ciphertext,
            false,
        )))
        .unwrap(),
    );
    assert!(
        received.identity_changed,
        "a msg that replaces the stored identity must say so"
    );
}
