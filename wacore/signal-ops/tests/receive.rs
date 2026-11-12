// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Buffered receive: idempotent redelivery, atomic commits, sender keys and
//! the app-state key share gate, with real padded WhatsApp messages.

use futures::executor::block_on;
use std::sync::Arc;
use wacore::libsignal::protocol::{DeviceId, ProtocolAddress};
use wacore::messages::MessageUtils;
use wacore_recordstore::{AesGcmSealer, LeaseStore, MemoryRecordStore, RecordSignalStore};
use wacore_signal_ops::{
    Encrypted, ReceiveKind, ReceiveRequest, Received, ReceivedMessage, RemoteBundle, SignalOps,
};
use waproto::whatsapp as wa;

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

fn connect(from: &Device, to: &Device, to_addr: &ProtocolAddress) {
    let identity = block_on(to.ops.public_identity());
    let prekey = block_on(to.ops.generate_prekeys(1)).unwrap().remove(0);
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
    connect(&alice, &bob, &bob_addr);

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
    connect(&alice, &bob, &bob_addr);
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
    connect(&alice, &bob, &bob_addr);

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
    connect(&alice, &bob, &bob_addr);
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

    let from_self = block_on(alice.ops.encrypt(&bob_addr, &padded(&share))).unwrap();
    let own = message(
        block_on(bob.ops.receive(request(
            &chat,
            &alice_addr,
            kind(&from_self),
            &from_self.ciphertext,
            true,
        )))
        .unwrap(),
    );
    assert!(own.app_state_key_share.is_some());
}

#[test]
fn pruning_drops_old_buffer_entries() {
    let alice = device("alice-prune");
    let bob = device("bob-prune");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &bob_addr);
    let chat = alice_addr.name().to_owned();
    let sent = block_on(alice.ops.encrypt(&bob_addr, &padded(&text("old")))).unwrap();
    block_on(bob.ops.receive(request(
        &chat,
        &alice_addr,
        kind(&sent),
        &sent.ciphertext,
        false,
    )))
    .unwrap();

    assert_eq!(block_on(bob.ops.prune_decrypt_buffer(0)).unwrap(), 0);
    assert_eq!(block_on(bob.ops.prune_decrypt_buffer(u64::MAX)).unwrap(), 1);
}

#[test]
fn a_bad_sender_key_does_not_block_the_message_carrying_it() {
    let alice = device("alice-bad-skdm");
    let bob = device("bob-bad-skdm");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    connect(&alice, &bob, &bob_addr);
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
