// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Two linked devices talking through real Signal sessions, each keeping its
//! state in a sealed, fenced record store.

use futures::executor::block_on;
use std::sync::Arc;
use wacore::libsignal::protocol::{DeviceId, ProtocolAddress};
use wacore::store::traits::SignalStore;
use wacore_recordstore::{
    AesGcmSealer, LeaseStore, MemoryRecordStore, Namespace, RecordSignalStore, RecordStore,
};
mod common;
use common::pair_success_container;
use wacore_signal_ops::{EncKind, OpsError, RemoteBundle, SignalOps};

mod runtime;
use runtime::InlineRuntime;

type Store = RecordSignalStore<MemoryRecordStore, AesGcmSealer>;

const TTL: u64 = 60_000;
const DATA_KEY: [u8; 32] = [42; 32];

struct Node {
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
        Arc::new(AesGcmSealer::new(&DATA_KEY)),
        lease.fence,
    ))
}

fn new_device(scope: &'static str) -> Node {
    let records = Arc::new(MemoryRecordStore::new());
    let store = store_for(&records, scope, "node-a", 0);
    let ops = block_on(SignalOps::create(store)).unwrap();
    Node {
        records,
        scope,
        ops,
    }
}

fn address(user: &str) -> ProtocolAddress {
    ProtocolAddress::new(&format!("{user}@s.whatsapp.net"), DeviceId::from(1))
}

/// What the WhatsApp client would fetch from the server for `node`.
/// `from` pins the account that signed `node`'s device, as it would after
/// meeting the peer's primary.
fn bundle_for(
    from: &Node,
    node: &Node,
    to: &ProtocolAddress,
    with_prekey: bool,
) -> (RemoteBundle, Option<u32>) {
    let device_identity = block_on(common::vouch(&from.ops, &node.ops, to.name()));
    let identity = block_on(node.ops.public_identity());
    let prekey = with_prekey.then(|| {
        let keys = block_on(node.ops.generate_prekeys(&InlineRuntime, 1)).unwrap();
        (keys[0].id, keys[0].public_key)
    });
    (
        RemoteBundle {
            registration_id: identity.registration_id,
            device_id: 1,
            identity_key: identity.identity_key,
            signed_prekey_id: identity.signed_prekey.id,
            signed_prekey: identity.signed_prekey.public_key,
            signed_prekey_signature: identity.signed_prekey.signature,
            prekey,
            device_identity: Some(device_identity),
        },
        prekey.map(|(id, _)| id),
    )
}

fn raw_records_contain(node: &Node, needle: &[u8]) -> bool {
    Namespace::ALL.iter().any(|ns| {
        let keys = block_on(node.records.scan_keys(node.scope, *ns, "", None)).unwrap();
        keys.iter().any(|key| {
            let raw = block_on(node.records.get(node.scope, *ns, key))
                .unwrap()
                .unwrap();
            raw.windows(needle.len()).any(|w| w == needle)
        })
    })
}

#[test]
fn a_session_runs_both_ways_and_stays_sealed() {
    let alice = new_device("alice-device");
    let bob = new_device("bob-device");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    let alice_identity = block_on(common::vouch(&bob.ops, &alice.ops, alice_addr.name()));

    let (bundle, prekey_id) = bundle_for(&alice, &bob, &bob_addr, true);
    block_on(alice.ops.establish_session(&bob_addr, &bundle)).unwrap();

    let first = block_on(alice.ops.encrypt(&bob_addr, b"hello bob")).unwrap();
    assert_eq!(first.kind, EncKind::PreKey);
    let opened = block_on(bob.ops.decrypt(
        &alice_addr,
        first.kind,
        &first.ciphertext,
        Some(&alice_identity),
    ))
    .unwrap();
    assert_eq!(opened.plaintext, b"hello bob");

    // The consumed one-time prekey is gone, after the session became durable.
    let bob_store = store_for(&bob.records, bob.scope, "node-a", 1);
    assert_eq!(
        block_on(bob_store.load_prekey(prekey_id.unwrap())).unwrap(),
        None
    );

    let reply = block_on(bob.ops.encrypt(&alice_addr, b"hi alice")).unwrap();
    assert_eq!(reply.kind, EncKind::Message);
    let opened = block_on(
        alice
            .ops
            .decrypt(&bob_addr, reply.kind, &reply.ciphertext, None),
    )
    .unwrap();
    assert_eq!(opened.plaintext, b"hi alice");

    for round in 0..5u8 {
        let to_bob = block_on(alice.ops.encrypt(&bob_addr, &[b'a', round])).unwrap();
        let got = block_on(
            bob.ops
                .decrypt(&alice_addr, to_bob.kind, &to_bob.ciphertext, None),
        )
        .unwrap();
        assert_eq!(got.plaintext, [b'a', round]);
        let to_alice = block_on(bob.ops.encrypt(&alice_addr, &[b'b', round])).unwrap();
        let got = block_on(
            alice
                .ops
                .decrypt(&bob_addr, to_alice.kind, &to_alice.ciphertext, None),
        )
        .unwrap();
        assert_eq!(got.plaintext, [b'b', round]);
    }

    assert!(!raw_records_contain(&bob, b"hello bob"));
    let bob_identity = block_on(bob.ops.public_identity());
    assert!(
        !raw_records_contain(&bob, &bob_identity.adv_secret),
        "device secrets reached the store unsealed"
    );
}

#[test]
fn a_failed_over_node_keeps_the_session_and_the_old_node_is_fenced_out() {
    let alice = new_device("alice-failover");
    let bob = new_device("bob-failover");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    let alice_identity = block_on(common::vouch(&bob.ops, &alice.ops, alice_addr.name()));
    let (bundle, _) = bundle_for(&alice, &bob, &bob_addr, true);
    block_on(alice.ops.establish_session(&bob_addr, &bundle)).unwrap();
    let first = block_on(alice.ops.encrypt(&bob_addr, b"before failover")).unwrap();
    block_on(bob.ops.decrypt(
        &alice_addr,
        first.kind,
        &first.ciphertext,
        Some(&alice_identity),
    ))
    .unwrap();

    // node-a's lease lapses; node-b takes the scope from the same records.
    let takeover = store_for(&bob.records, bob.scope, "node-b", TTL + 1);
    let bob_b = block_on(SignalOps::open(takeover)).unwrap();

    let next = block_on(alice.ops.encrypt(&bob_addr, b"after failover")).unwrap();
    let stale = block_on(bob.ops.decrypt(
        &alice_addr,
        next.kind,
        &next.ciphertext,
        Some(&alice_identity),
    ));
    let err = stale.expect_err("the superseded node must not advance the ratchet");
    assert!(err.is_fence_lost(), "unexpected error: {err:?}");

    let opened = block_on(bob_b.decrypt(
        &alice_addr,
        next.kind,
        &next.ciphertext,
        Some(&alice_identity),
    ))
    .unwrap();
    assert_eq!(opened.plaintext, b"after failover");
}

#[test]
fn group_messages_decrypt_with_a_distributed_sender_key() {
    let alice = new_device("alice-group");
    let bob = new_device("bob-group");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    let alice_identity = block_on(common::vouch(&bob.ops, &alice.ops, alice_addr.name()));
    let group = "120363000000001@g.us";
    let (bundle, _) = bundle_for(&alice, &bob, &bob_addr, true);
    block_on(alice.ops.establish_session(&bob_addr, &bundle)).unwrap();

    let skdm = block_on(alice.ops.sender_key_distribution(group, &alice_addr)).unwrap();
    let carried = block_on(alice.ops.encrypt(&bob_addr, &skdm)).unwrap();
    let received = block_on(bob.ops.decrypt(
        &alice_addr,
        carried.kind,
        &carried.ciphertext,
        Some(&alice_identity),
    ))
    .unwrap();
    block_on(
        bob.ops
            .process_sender_key_distribution(group, &alice_addr, &received.plaintext),
    )
    .unwrap();

    let skmsg = block_on(alice.ops.group_encrypt(group, &alice_addr, b"hello group")).unwrap();
    let plaintext = block_on(bob.ops.group_decrypt(group, &alice_addr, &skmsg)).unwrap();
    assert_eq!(plaintext, b"hello group");
}

#[test]
fn a_prekey_message_for_a_rotated_signed_prekey_still_decrypts() {
    let alice = new_device("alice-rotate");
    let bob = new_device("bob-rotate");
    let (alice_addr, bob_addr) = (address("111"), address("222"));
    let alice_identity = block_on(common::vouch(&bob.ops, &alice.ops, alice_addr.name()));
    let (bundle, _) = bundle_for(&alice, &bob, &bob_addr, false);
    let before = block_on(bob.ops.public_identity()).signed_prekey;
    block_on(alice.ops.establish_session(&bob_addr, &bundle)).unwrap();

    let after = block_on(bob.ops.rotate_signed_prekey()).unwrap();
    assert_ne!(after.id, before.id);

    let first = block_on(alice.ops.encrypt(&bob_addr, b"old signed prekey")).unwrap();
    let opened = block_on(bob.ops.decrypt(
        &alice_addr,
        first.kind,
        &first.ciphertext,
        Some(&alice_identity),
    ))
    .unwrap();
    assert_eq!(opened.plaintext, b"old signed prekey");
}

#[test]
fn reopening_a_scope_restores_the_same_device() {
    let bob = new_device("bob-reopen");
    let identity = block_on(bob.ops.public_identity());
    let reopened = block_on(SignalOps::open(store_for(
        &bob.records,
        bob.scope,
        "node-a",
        1,
    )))
    .unwrap();
    assert_eq!(block_on(reopened.public_identity()), identity);
    let again = block_on(SignalOps::create(store_for(
        &bob.records,
        bob.scope,
        "node-a",
        2,
    )));
    assert!(
        matches!(again, Err(OpsError::InvalidInput(_))),
        "create overwrote existing keys"
    );
}

#[test]
fn pairing_signs_a_valid_container_with_the_device_prefix() {
    let bob = new_device("bob-pairing-ok");
    let identity = block_on(bob.ops.public_identity());
    let (container, account) =
        pair_success_container(identity.identity_key, &identity.adv_secret, 7);

    let signed = block_on(bob.ops.sign_pairing(&container)).unwrap();
    assert_eq!(signed.key_index, 7);

    let decoded =
        waproto::codec::adv_signed_device_identity_decode(&signed.signed_identity).unwrap();
    let details = decoded.details.as_deref().unwrap();
    let message = [
        &[6u8, 1][..],
        details,
        &identity.identity_key,
        account.public_key.public_key_bytes(),
    ]
    .concat();
    let device_key =
        wacore::libsignal::protocol::PublicKey::from_djb_public_key_bytes(&identity.identity_key)
            .unwrap();
    assert!(device_key.verify_signature(&message, decoded.device_signature.as_deref().unwrap()));
}

#[test]
fn pairing_refuses_a_container_for_another_secret() {
    let bob = new_device("bob-pairing-secret");
    let identity = block_on(bob.ops.public_identity());
    let (container, _) = pair_success_container(identity.identity_key, &[1; 32], 7);
    assert!(matches!(
        block_on(bob.ops.sign_pairing(&container)),
        Err(OpsError::Pairing { code: 401, .. })
    ));
}

#[test]
fn pairing_refuses_a_forged_container() {
    let bob = new_device("bob-pairing");
    let refused = block_on(bob.ops.sign_pairing(b"not a pairing container"));
    assert!(matches!(refused, Err(OpsError::Pairing { .. })));
}

#[test]
fn forget_user_drops_every_device_of_that_user_only() {
    let records = Arc::new(MemoryRecordStore::new());
    let store = store_for(&records, "forget", "node-a", 0);
    let ops = block_on(SignalOps::create(store.clone())).unwrap();
    let forgotten = [
        "123@c.us.0",
        "123:7@c.us.0",
        "123:12@c.us.0",
        "123:99@hosted.0",
    ];
    let kept = [
        "1234@c.us.0",
        "1234:7@c.us.0",
        "1234:99@hosted.0",
        "123@lid.0",
        "123:7@lid.0",
        "123:99@hosted.lid.0",
    ];
    // Stored addresses use WA Web's `c.us` server for phone-number users.
    for address in forgotten.iter().chain(&kept) {
        block_on(store.put_session(address, b"session")).unwrap();
        block_on(store.put_identity(address, [9; 32])).unwrap();
    }

    let user: wacore_binary::Jid = "123:7@s.whatsapp.net".parse().unwrap();
    block_on(ops.forget_user(&user)).unwrap();

    for address in forgotten {
        assert!(
            block_on(store.get_session(address)).unwrap().is_none(),
            "{address}"
        );
        assert!(
            block_on(store.load_identity(address)).unwrap().is_none(),
            "{address}"
        );
    }
    for address in kept {
        assert!(
            block_on(store.get_session(address)).unwrap().is_some(),
            "{address}"
        );
        assert!(
            block_on(store.load_identity(address)).unwrap().is_some(),
            "{address}"
        );
    }
    // Nothing left to forget is not an error.
    block_on(ops.forget_user(&user)).unwrap();
}

#[test]
fn migrate_pn_to_lid_moves_regular_and_hosted_devices() {
    let records = Arc::new(MemoryRecordStore::new());
    let store = store_for(&records, "migrate", "node-a", 0);
    let ops = block_on(SignalOps::create(store.clone())).unwrap();
    // Device 0 and the hosted device exist only under PN.
    block_on(store.put_session("123@c.us.0", b"pn-0")).unwrap();
    block_on(store.put_identity("123@c.us.0", [1; 32])).unwrap();
    block_on(store.put_session("123:99@hosted.0", b"pn-hosted")).unwrap();
    block_on(store.put_identity("123:99@hosted.0", [2; 32])).unwrap();
    // Device 7 exists on both sides: PN session wins, LID identity wins.
    block_on(store.put_session("123:7@c.us.0", b"pn-7")).unwrap();
    block_on(store.put_identity("123:7@c.us.0", [3; 32])).unwrap();
    block_on(store.put_session("456:7@lid.0", b"lid-7")).unwrap();
    block_on(store.put_identity("456:7@lid.0", [4; 32])).unwrap();
    // Another user sharing the prefix is untouched.
    block_on(store.put_session("1234@c.us.0", b"other")).unwrap();

    let pn: wacore_binary::Jid = "123@s.whatsapp.net".parse().unwrap();
    let lid: wacore_binary::Jid = "456@lid".parse().unwrap();
    assert_eq!(block_on(ops.migrate_pn_to_lid(&pn, &lid)).unwrap(), 3);

    let session = |a: &str| block_on(store.get_session(a)).unwrap().map(|b| b.to_vec());
    let identity = |a: &str| block_on(store.load_identity(a)).unwrap();
    assert_eq!(session("456@lid.0").as_deref(), Some(&b"pn-0"[..]));
    assert_eq!(identity("456@lid.0"), Some([1; 32]));
    assert_eq!(
        session("456:99@hosted.lid.0").as_deref(),
        Some(&b"pn-hosted"[..])
    );
    assert_eq!(identity("456:99@hosted.lid.0"), Some([2; 32]));
    assert_eq!(session("456:7@lid.0").as_deref(), Some(&b"pn-7"[..]));
    assert_eq!(identity("456:7@lid.0"), Some([4; 32]));
    for gone in ["123@c.us.0", "123:7@c.us.0", "123:99@hosted.0"] {
        assert!(
            session(gone).is_none() && identity(gone).is_none(),
            "{gone}"
        );
    }
    assert!(session("1234@c.us.0").is_some());

    // Already migrated: nothing left to move.
    assert_eq!(block_on(ops.migrate_pn_to_lid(&pn, &lid)).unwrap(), 0);
    // Arguments the wrong way round are refused.
    assert!(matches!(
        block_on(ops.migrate_pn_to_lid(&lid, &pn)),
        Err(OpsError::InvalidInput(_))
    ));
}

#[test]
fn a_second_open_under_the_same_fence_shares_one_authority() {
    let records = Arc::new(MemoryRecordStore::new());
    let scope = "same-holder";
    let first = block_on(SignalOps::create(store_for(&records, scope, "node-a", 0))).unwrap();
    // Renewal by the same holder keeps the fence, so both instances write
    // under one generation and the store cannot tell their writes apart.
    let second = block_on(SignalOps::open(store_for(&records, scope, "node-a", 1))).unwrap();

    let (a, b) = block_on(futures::future::join(
        first.rotate_signed_prekey(),
        second.rotate_signed_prekey(),
    ));
    let (a, b) = (a.unwrap(), b.unwrap());
    assert_ne!(a.id, b.id, "rotations under one fence must not reuse an id");

    let latest = block_on(first.public_identity()).signed_prekey;
    assert_eq!(block_on(second.public_identity()).signed_prekey, latest);
    assert!(latest.id == a.id || latest.id == b.id);

    drop((first, second));
    let reopened = block_on(SignalOps::open(store_for(&records, scope, "node-a", 2))).unwrap();
    assert_eq!(block_on(reopened.public_identity()).signed_prekey, latest);
}
