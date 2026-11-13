// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! A prekey message starts a session, so its sender is verified like a
//! fetched bundle; pins follow a PN-to-LID migration; own JIDs are checked.

use crate::records::{AesGcmSealer, LeaseStore, MemoryRecordStore, RecordSignalStore};
use futures::executor::block_on;
use std::sync::Arc;
use wacore::signal::{
    BundleRejection, EncKind, OpsError, RemoteBundle, SignalOperationStore, SignalOps,
};
use wacore::types::jid::JidExt as _;
use wacore_binary::Jid;

use crate::common;
use crate::runtime;
use runtime::InlineRuntime;

type Store = RecordSignalStore<MemoryRecordStore, AesGcmSealer>;

fn store(scope: &str) -> Arc<Store> {
    let records = Arc::new(MemoryRecordStore::new());
    let lease = block_on(records.acquire(scope, "node-a", 0, 60_000))
        .unwrap()
        .expect("lease available");
    Arc::new(RecordSignalStore::new(
        records,
        Arc::new(AesGcmSealer::new(&[9; 32])),
        lease.fence,
    ))
}

fn device(scope: &str) -> SignalOps<Store> {
    block_on(SignalOps::create(store(scope))).unwrap()
}

fn jid(text: &str) -> Jid {
    text.parse().unwrap()
}

/// What the client would fetch for `ops`: one one-time prekey, optionally.
fn bundle_of(ops: &SignalOps<Store>, device_identity: Option<Vec<u8>>) -> RemoteBundle {
    let identity = block_on(ops.public_identity());
    let prekey = block_on(ops.generate_prekeys(&InlineRuntime, 1))
        .unwrap()
        .remove(0);
    RemoteBundle {
        registration_id: identity.registration_id,
        device_id: 1,
        identity_key: identity.identity_key,
        signed_prekey_id: identity.signed_prekey.id,
        signed_prekey: identity.signed_prekey.public_key,
        signed_prekey_signature: identity.signed_prekey.signature,
        prekey: Some((prekey.id, prekey.public_key)),
        device_identity,
    }
}

fn refusal<T: std::fmt::Debug>(result: Result<T, OpsError>) -> BundleRejection {
    match result {
        Err(OpsError::BundleRefused(reason)) => reason,
        other => panic!("expected a refused bundle, got {other:?}"),
    }
}

#[test]
fn a_prekey_message_is_verified_against_the_pinned_account_before_it_creates_a_session() {
    let alice_jid = jid("111:1@s.whatsapp.net");
    let bob_jid = jid("222@s.whatsapp.net");
    let (alice, eve, bob) = (device("alice-pk"), device("eve-pk"), device("bob-pk"));
    let alice_addr = alice_jid.to_protocol_address();
    let bob_addr = bob_jid.to_protocol_address();

    // Bob knows Alice's account. Eve has her own account and valid identity.
    let alice_paired = block_on(common::pair(&alice));
    block_on(bob.trust_account_identity(&alice_jid, alice_paired.account)).unwrap();
    let eve_paired = block_on(common::pair(&eve));

    // Both start a session from the same bundle, so a refusal that consumed
    // the one-time prekey would break the honest message later.
    let bundle = bundle_of(&bob, None);
    block_on(alice.establish_session(&bob_addr, &bundle)).unwrap();
    block_on(eve.establish_session(&bob_addr, &bundle)).unwrap();

    // Eve's message under Alice's name, with Eve's own valid device identity.
    let forged = block_on(eve.encrypt(&bob_addr, b"forged")).unwrap();
    assert_eq!(forged.kind, EncKind::PreKey);
    let reason = refusal(block_on(bob.decrypt(
        &alice_addr,
        forged.kind,
        &forged.ciphertext,
        Some(&eve_paired.device_identity),
    )));
    assert_eq!(reason, BundleRejection::InvalidDeviceIdentity);
    assert!(!block_on(bob.has_session(&alice_addr)).unwrap());

    // A companion's prekey message without its device identity.
    let honest = block_on(alice.encrypt(&bob_addr, b"hello bob")).unwrap();
    let reason = refusal(block_on(bob.decrypt(
        &alice_addr,
        honest.kind,
        &honest.ciphertext,
        None,
    )));
    assert_eq!(reason, BundleRejection::MissingDeviceIdentity);
    assert!(!block_on(bob.has_session(&alice_addr)).unwrap());

    // The legitimate message still works afterwards.
    let opened = block_on(bob.decrypt(
        &alice_addr,
        honest.kind,
        &honest.ciphertext,
        Some(&alice_paired.device_identity),
    ))
    .unwrap();
    assert_eq!(opened.plaintext, b"hello bob");
    assert!(block_on(bob.has_session(&alice_addr)).unwrap());
}

#[test]
fn a_primary_that_changes_identity_is_refused_until_the_application_accepts_it() {
    let carol_jid = jid("444@s.whatsapp.net");
    let bob_jid = jid("222@s.whatsapp.net");
    let (carol, reset_carol, bob) = (
        device("carol-pk"),
        device("carol-reset-pk"),
        device("bob-pk2"),
    );
    let carol_addr = carol_jid.to_protocol_address();
    let bob_addr = bob_jid.to_protocol_address();

    block_on(carol.establish_session(&bob_addr, &bundle_of(&bob, None))).unwrap();
    block_on(reset_carol.establish_session(&bob_addr, &bundle_of(&bob, None))).unwrap();

    // A primary pins its identity key on first use.
    let first = block_on(carol.encrypt(&bob_addr, b"first")).unwrap();
    block_on(bob.decrypt(&carol_addr, first.kind, &first.ciphertext, None)).unwrap();
    let pinned = block_on(bob.account_identity(&carol_jid)).unwrap();
    assert_eq!(pinned, Some(block_on(carol.public_identity()).identity_key));

    // The same address now arrives with different keys.
    let reset = block_on(reset_carol.encrypt(&bob_addr, b"after reset")).unwrap();
    assert_eq!(reset.kind, EncKind::PreKey);
    let reason = refusal(block_on(bob.decrypt(
        &carol_addr,
        reset.kind,
        &reset.ciphertext,
        None,
    )));
    assert_eq!(reason, BundleRejection::IdentityChanged);

    let accepted = block_on(reset_carol.public_identity()).identity_key;
    block_on(bob.trust_account_identity(&carol_jid, accepted)).unwrap();
    let opened = block_on(bob.decrypt(&carol_addr, reset.kind, &reset.ciphertext, None)).unwrap();
    assert_eq!(opened.plaintext, b"after reset");
}

#[test]
fn a_prekey_message_from_another_device_of_the_same_account_needs_no_pin() {
    let me_jid = jid("111:1@s.whatsapp.net");
    let other_jid = jid("111:2@s.whatsapp.net");
    let stranger_jid = jid("111:3@s.whatsapp.net");
    let (me, other, stranger) = (
        device("me-own"),
        device("other-own"),
        device("stranger-own"),
    );
    block_on(me.set_own_jids(&me_jid.to_string(), None)).unwrap();
    block_on(other.set_own_jids(&other_jid.to_string(), None)).unwrap();
    let me_paired = block_on(common::pair(&me));
    let other_identity = block_on(common::pair_under(&other, &me_paired.account_keys));
    let (me_addr, other_addr) = (
        me_jid.to_protocol_address(),
        other_jid.to_protocol_address(),
    );

    // The other device verifies `me` against the account it was paired under.
    block_on(other.establish_session(
        &me_addr,
        &bundle_of(&me, Some(me_paired.device_identity.clone())),
    ))
    .unwrap();
    let sent = block_on(other.encrypt(&me_addr, b"from my other phone")).unwrap();
    assert_eq!(sent.kind, EncKind::PreKey);
    assert_eq!(block_on(me.account_identity(&other_jid)).unwrap(), None);
    let opened = block_on(me.decrypt(
        &other_addr,
        sent.kind,
        &sent.ciphertext,
        Some(&other_identity),
    ))
    .unwrap();
    assert_eq!(opened.plaintext, b"from my other phone");

    // A device claiming the same user but paired under another account fails.
    block_on(stranger.trust_account_identity(&me_jid, me_paired.account)).unwrap();
    block_on(stranger.establish_session(
        &me_addr,
        &bundle_of(&me, Some(me_paired.device_identity.clone())),
    ))
    .unwrap();
    let stranger_paired = block_on(common::pair(&stranger));
    let forged = block_on(stranger.encrypt(&me_addr, b"not yours")).unwrap();
    let reason = refusal(block_on(me.decrypt(
        &stranger_jid.to_protocol_address(),
        forged.kind,
        &forged.ciphertext,
        Some(&stranger_paired.device_identity),
    )));
    assert_eq!(reason, BundleRejection::InvalidDeviceIdentity);
}

/// Alice holds a session with Bob's PN companion, so a migration has
/// something to move, and pins his phone-number account.
fn alice_with_a_pn_session(
    scope: &str,
    bob_a: &SignalOps<Store>,
    paired: &common::Paired,
) -> SignalOps<Store> {
    let alice = device(scope);
    let pn_device = jid("222:1@s.whatsapp.net");
    block_on(alice.trust_account_identity(&pn_device, paired.account)).unwrap();
    block_on(alice.establish_session(
        &pn_device.to_protocol_address(),
        &bundle_of(bob_a, Some(paired.device_identity.clone())),
    ))
    .unwrap();
    alice
}

#[test]
fn migrating_to_a_lid_carries_the_account_pin() {
    let bob_a = device("bob-a-migrate");
    let bob_b = device("bob-b-migrate");
    let paired = block_on(common::pair(&bob_a));
    let b_identity = block_on(common::pair_under(&bob_b, &paired.account_keys));
    let alice = alice_with_a_pn_session("alice-migrate", &bob_a, &paired);

    block_on(alice.migrate_pn_to_lid(&jid("222@s.whatsapp.net"), &jid("900222@lid"))).unwrap();
    assert_eq!(
        block_on(alice.account_identity(&jid("900222@lid"))).unwrap(),
        Some(paired.account)
    );

    // A second companion of the same account, first met at its LID address.
    let lid_device = jid("900222:2@lid");
    block_on(alice.establish_session(
        &lid_device.to_protocol_address(),
        &bundle_of(&bob_b, Some(b_identity)),
    ))
    .expect("the migrated pin anchors the LID companion");
}

#[test]
fn migrating_keeps_a_pin_the_lid_already_has() {
    let bob_a = device("bob-a-keep");
    let bob_b = device("bob-b-keep");
    let paired = block_on(common::pair(&bob_a));
    let b_identity = block_on(common::pair_under(&bob_b, &paired.account_keys));
    let alice = alice_with_a_pn_session("alice-keep", &bob_a, &paired);
    let other_key = [9u8; 32];
    block_on(alice.trust_account_identity(&jid("900222@lid"), other_key)).unwrap();

    block_on(alice.migrate_pn_to_lid(&jid("222@s.whatsapp.net"), &jid("900222@lid"))).unwrap();
    assert_eq!(
        block_on(alice.account_identity(&jid("900222@lid"))).unwrap(),
        Some(other_key)
    );
    let reason = refusal(block_on(alice.establish_session(
        &jid("900222:2@lid").to_protocol_address(),
        &bundle_of(&bob_b, Some(b_identity)),
    )));
    assert_eq!(reason, BundleRejection::InvalidDeviceIdentity);
}

#[test]
fn own_jids_are_checked_and_a_refusal_leaves_the_device_keys_alone() {
    let store = store("own-jids");
    let ops = block_on(SignalOps::create(store.clone())).unwrap();
    block_on(ops.set_own_jids("111:1@s.whatsapp.net", Some("900111:1@lid"))).unwrap();
    let blob = block_on(store.load_device_keys()).unwrap();
    let identity = block_on(ops.public_identity());

    let long = format!("{}@s.whatsapp.net", "1".repeat(70_000));
    let long_lid = format!("{}@lid", "9".repeat(70_000));
    let attempts: [(&str, Option<&str>); 6] = [
        // Swapped.
        ("900111:1@lid", Some("111:1@s.whatsapp.net")),
        // A LID where the phone number belongs.
        ("900111:1@lid", None),
        // A phone number where the LID belongs.
        ("111:1@s.whatsapp.net", Some("222:1@s.whatsapp.net")),
        // Too long for the device-key record.
        (&long, None),
        ("111:1@s.whatsapp.net", Some(&long_lid)),
        ("not a jid", None),
    ];
    for (pn, lid) in attempts {
        let result = block_on(ops.set_own_jids(pn, lid));
        assert!(
            matches!(result, Err(OpsError::InvalidInput(_))),
            "expected {:?} to be refused, got {result:?}",
            (pn.len(), lid.map(str::len))
        );
        assert_eq!(block_on(store.load_device_keys()).unwrap(), blob);
        assert_eq!(block_on(ops.public_identity()), identity);
    }
    let reopened = block_on(SignalOps::open(store)).unwrap();
    assert_eq!(block_on(reopened.public_identity()), identity);
}
