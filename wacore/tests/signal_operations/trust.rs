// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Peer prekey bundles must be bound to the peer account before a session is
//! built from them, whether they come from a send's fetch, a retry receipt
//! or `establish_session`.

use crate::records::{AesGcmSealer, LeaseStore, MemoryRecordStore, RecordSignalStore};
use async_trait::async_trait;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use wacore::client::context::{GroupRoutingInfo, SendContextResolver, SessionLockGuard};
use wacore::libsignal::protocol::{DeviceId, IdentityKey, KeyPair, PreKeyBundle, PublicKey};
use wacore::prekeys::PreKeyFetchOutcome;
use wacore::runtime::{AbortHandle, Runtime};
use wacore::signal::{
    BundleRejection, FetchedBundle, OpsError, ReceiveKind, ReceiveRequest, Received, RemoteBundle,
    RetryRequest, RetryRoute, SignalOps, SignalResolver, VerifiablePrekeys,
};
use wacore::types::jid::JidExt as _;
use wacore_binary::{Jid, NodeContent};
use waproto::whatsapp as wa;

use crate::common;
use crate::runtime;
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
    ops.set_own_jids(own_pn, None).await.unwrap();
    ops
}

/// `ops`'s bundle as the server hands it out, for device `device_id`.
async fn bundle_for(ops: &SignalOps<Store>, device_id: u32) -> PreKeyBundle {
    let identity = ops.public_identity().await;
    let prekey = ops
        .generate_prekeys(&InlineRuntime, 1)
        .await
        .unwrap()
        .remove(0);
    PreKeyBundle::new(
        identity.registration_id,
        DeviceId::from(device_id),
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

fn remote(bundle: &PreKeyBundle, device_identity: Option<Vec<u8>>) -> RemoteBundle {
    let key = |k: &PublicKey| -> [u8; 32] { *k.public_key_bytes() };
    RemoteBundle {
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

/// A primary device's bundle: its identity is the account key itself.
fn primary_bundle(account: &KeyPair) -> PreKeyBundle {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signed = KeyPair::generate(&mut rng);
    let signature = account
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap();
    PreKeyBundle::new(
        7,
        DeviceId::from(0),
        None,
        1.into(),
        signed.public_key,
        signature.to_vec(),
        IdentityKey::new(account.public_key),
    )
    .unwrap()
}

/// A client that answers with fixed devices and bundles, keeping each
/// companion's device identity.
struct Client {
    devices: Vec<Jid>,
    bundles: HashMap<Jid, FetchedBundle>,
    group: Option<Arc<GroupRoutingInfo>>,
    /// When set, a fetch signals `entered` and waits for `release`.
    gate: Option<(Arc<tokio::sync::Notify>, Arc<tokio::sync::Notify>)>,
}

#[async_trait]
impl SendContextResolver for Client {
    async fn lock_device_sessions(&self, _device_jids: &[Jid]) -> SessionLockGuard {
        SessionLockGuard::none()
    }

    async fn resolve_devices(&self, _jids: &[Jid]) -> anyhow::Result<Vec<Jid>> {
        Ok(self.devices.clone())
    }

    async fn fetch_prekeys(&self, _jids: &[Jid]) -> anyhow::Result<HashMap<Jid, PreKeyBundle>> {
        anyhow::bail!("session setup must use the verifiable fetch")
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        _jids: &[Jid],
    ) -> anyhow::Result<PreKeyFetchOutcome> {
        anyhow::bail!("session setup must use the verifiable fetch")
    }

    async fn resolve_group_routing_info(
        &self,
        _jid: &Jid,
    ) -> anyhow::Result<Arc<GroupRoutingInfo>> {
        self.group
            .clone()
            .ok_or_else(|| anyhow::anyhow!("no group here"))
    }
}

#[async_trait]
impl SignalResolver for Client {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        if let Some((entered, release)) = &self.gate {
            entered.notify_one();
            release.notified().await;
        }
        Ok(VerifiablePrekeys {
            bundles: jids
                .iter()
                .filter_map(|jid| self.bundles.get(jid).map(|b| (jid.clone(), b.clone())))
                .collect(),
            rejected: Vec::new(),
        })
    }
}

fn text(body: &str) -> wa::Message {
    let mut message = wa::Message::default();
    message.conversation = Some(body.to_owned());
    message
}

fn enc_for(stanza: &[u8], device: &Jid) -> Option<(String, Vec<u8>)> {
    let node = wacore_binary::marshal::unmarshal_packed_ref(stanza)
        .unwrap()
        .to_owned();
    let participants = node.get_optional_child("participants")?;
    let to = participants.get_children_by_tag("to").find(|to| {
        to.attrs().optional_string("jid").as_deref() == Some(device.to_string().as_str())
    })?;
    let enc = to.get_optional_child("enc")?;
    let kind = enc.attrs().optional_string("type")?.to_string();
    let Some(NodeContent::Bytes(bytes)) = &enc.content else {
        return None;
    };
    Some((kind, bytes.clone()))
}

/// Pair `ops` under a fresh account, returning the account key pair and the
/// device identity its bundles carry.
async fn pair_with_account(ops: &SignalOps<Store>) -> (KeyPair, Vec<u8>) {
    let identity = ops.public_identity().await;
    let (container, account) =
        common::pair_success_container(identity.identity_key, &identity.adv_secret, 1);
    let signed = ops.sign_pairing(&container).await.unwrap();
    (account, signed.signed_identity)
}

fn refused(result: Result<(), OpsError>, reason: BundleRejection) {
    match result {
        Err(OpsError::BundleRefused(actual)) => assert_eq!(actual, reason),
        other => panic!("expected the bundle to be refused with {reason:?}, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_bundle_is_installed_only_with_a_device_identity_from_its_account() {
    let alice = device("alice-adv", "111:1@s.whatsapp.net").await;
    let bob = device("bob-adv", "222:1@s.whatsapp.net").await;
    let eve = device("eve-adv", "333:1@s.whatsapp.net").await;
    let bob_paired = common::pair(&bob).await;
    let eve_paired = common::pair(&eve).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    alice
        .trust_account_identity(&bob_jid, bob_paired.account)
        .await
        .unwrap();
    let address = bob_jid.to_protocol_address();

    // No device identity at all.
    refused(
        alice
            .establish_session(&address, &remote(&bundle_for(&bob, 1).await, None))
            .await,
        BundleRejection::MissingDeviceIdentity,
    );
    // Eve's keys under Bob's name, with Eve's own valid device identity.
    refused(
        alice
            .establish_session(
                &address,
                &remote(
                    &bundle_for(&eve, 1).await,
                    Some(eve_paired.device_identity.clone()),
                ),
            )
            .await,
        BundleRejection::InvalidDeviceIdentity,
    );
    // Eve's keys carrying Bob's device identity: it binds Bob's key, not Eve's.
    refused(
        alice
            .establish_session(
                &address,
                &remote(
                    &bundle_for(&eve, 1).await,
                    Some(bob_paired.device_identity.clone()),
                ),
            )
            .await,
        BundleRejection::InvalidDeviceIdentity,
    );
    assert!(!alice.has_session(&address).await.unwrap());

    alice
        .establish_session(
            &address,
            &remote(&bundle_for(&bob, 1).await, Some(bob_paired.device_identity)),
        )
        .await
        .unwrap();
    assert!(alice.has_session(&address).await.unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_with_no_known_account_is_refused() {
    let alice = device("alice-no-anchor", "111:1@s.whatsapp.net").await;
    let bob = device("bob-no-anchor", "222:1@s.whatsapp.net").await;
    let paired = common::pair(&bob).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    refused(
        alice
            .establish_session(
                &bob_jid.to_protocol_address(),
                &remote(&bundle_for(&bob, 1).await, Some(paired.device_identity)),
            )
            .await,
        BundleRejection::NoAccountIdentity,
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_primary_identity_is_refused_until_the_application_accepts_it() {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = device("alice-primary", "111:1@s.whatsapp.net").await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let address = primary.to_protocol_address();
    let first = KeyPair::generate(&mut rng);
    let reinstalled = KeyPair::generate(&mut rng);

    // First contact pins the account.
    alice
        .establish_session(&address, &remote(&primary_bundle(&first), None))
        .await
        .unwrap();
    assert_eq!(
        alice.account_identity(&primary).await.unwrap(),
        Some(*first.public_key.public_key_bytes())
    );

    // A different primary key, as a client could supply at any time.
    refused(
        alice
            .establish_session(&address, &remote(&primary_bundle(&reinstalled), None))
            .await,
        BundleRejection::IdentityChanged,
    );

    // Only the application can accept the change.
    alice
        .trust_account_identity(&primary, *reinstalled.public_key.public_key_bytes())
        .await
        .unwrap();
    alice
        .establish_session(&address, &remote(&primary_bundle(&reinstalled), None))
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_send_skips_a_companion_whose_bundle_does_not_verify() {
    let alice = device("alice-send-adv", "111:1@s.whatsapp.net").await;
    let alice_paired = common::pair(&alice).await;
    let bob = device("bob-send-adv", "222:1@s.whatsapp.net").await;
    let eve = device("eve-send-adv", "333:1@s.whatsapp.net").await;
    let bob_paired = common::pair(&bob).await;
    let eve_paired = common::pair(&eve).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    alice
        .trust_account_identity(&bob_jid, bob_paired.account)
        .await
        .unwrap();
    let to = bob_jid.to_non_ad();

    let forged = Client {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(
            bob_jid.clone(),
            FetchedBundle {
                bundle: bundle_for(&eve, 1).await,
                device_identity: Some(eve_paired.device_identity),
            },
        )]),
        group: None,
        gate: None,
    };
    // Bob's only device fails verification, so there is nothing to send.
    let sent = alice
        .send_direct(&TokioRuntime, &forged, &to, &text("for bob"), "F1")
        .await;
    assert!(
        matches!(sent, Err(OpsError::Send(_))),
        "no stanza may be built from a refused bundle: {sent:?}"
    );
    assert!(
        !alice
            .has_session(&bob_jid.to_protocol_address())
            .await
            .unwrap()
    );

    let honest = Client {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(
            bob_jid.clone(),
            FetchedBundle {
                bundle: bundle_for(&bob, 1).await,
                device_identity: Some(bob_paired.device_identity),
            },
        )]),
        group: None,
        gate: None,
    };
    let sent = alice
        .send_direct(&TokioRuntime, &honest, &to, &text("for bob"), "F2")
        .await
        .unwrap();
    let (kind, ciphertext) = enc_for(&sent.stanza, &bob_jid).expect("bob is reached");
    let alice_jid: Jid = "111:1@s.whatsapp.net".parse().unwrap();
    // Bob verifies Alice's prekey message against her pinned account.
    bob.trust_account_identity(&alice_jid, alice_paired.account)
        .await
        .unwrap();
    let received = bob
        .receive(ReceiveRequest {
            chat: &alice_jid.to_non_ad().to_string(),
            sender: &alice_jid.to_protocol_address(),
            kind: ReceiveKind::try_from(kind.as_str()).unwrap(),
            ciphertext: &ciphertext,
            padding_version: 2,
            is_from_me: false,
            device_identity: Some(&alice_paired.device_identity),
        })
        .await
        .unwrap();
    let Received::Message(message) = received else {
        panic!("expected a message")
    };
    assert_eq!(
        message.content.message.conversation.as_deref(),
        Some("for bob")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_fresh_account_is_anchored_by_its_primary_in_the_same_fetch() {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = device("alice-fresh", "111:1@s.whatsapp.net").await;
    let bob = device("bob-fresh", "222:1@s.whatsapp.net").await;
    let paired = common::pair(&bob).await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let companion: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let impostor = KeyPair::generate(&mut rng);

    // A primary that is not the account which signed the companion.
    let mismatched = Client {
        devices: vec![primary.clone(), companion.clone()],
        bundles: HashMap::from([
            (
                primary.clone(),
                FetchedBundle {
                    bundle: primary_bundle(&impostor),
                    device_identity: None,
                },
            ),
            (
                companion.clone(),
                FetchedBundle {
                    bundle: bundle_for(&bob, 1).await,
                    device_identity: Some(paired.device_identity.clone()),
                },
            ),
        ]),
        group: None,
        gate: None,
    };
    let sent = alice
        .send_direct(&TokioRuntime, &mismatched, &primary, &text("hi"), "A1")
        .await
        .unwrap();
    assert!(enc_for(&sent.stanza, &companion).is_none());
    assert!(sent.unreached_devices.contains(&companion));
    // The impostor's primary was pinned on first use, so nothing later can
    // swap it either; that is what trust on first use means.
    assert_eq!(
        alice.account_identity(&primary).await.unwrap(),
        Some(*impostor.public_key.public_key_bytes())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_companion_is_accepted_when_its_account_primary_is_in_the_same_fetch() {
    let alice = device("alice-same-fetch", "111:1@s.whatsapp.net").await;
    let bob = device("bob-same-fetch", "222:1@s.whatsapp.net").await;
    let (account, device_identity) = pair_with_account(&bob).await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let companion: Jid = "222:1@s.whatsapp.net".parse().unwrap();

    let client = Client {
        devices: vec![primary.clone(), companion.clone()],
        bundles: HashMap::from([
            (
                primary.clone(),
                FetchedBundle {
                    bundle: primary_bundle(&account),
                    device_identity: None,
                },
            ),
            (
                companion.clone(),
                FetchedBundle {
                    bundle: bundle_for(&bob, 1).await,
                    device_identity: Some(device_identity),
                },
            ),
        ]),
        group: None,
        gate: None,
    };
    let sent = alice
        .send_direct(&TokioRuntime, &client, &primary, &text("hi"), "A2")
        .await
        .unwrap();
    assert!(enc_for(&sent.stanza, &companion).is_some());
    assert_eq!(
        alice.account_identity(&companion).await.unwrap(),
        Some(*account.public_key.public_key_bytes())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_naming_a_recipient_with_another_accounts_keys_is_refused() {
    let alice = device("alice-retry-adv", "111:1@s.whatsapp.net").await;
    common::pair(&alice).await;
    let bob = device("bob-retry-adv", "222:1@s.whatsapp.net").await;
    let eve = device("eve-retry-adv", "333:1@s.whatsapp.net").await;
    let bob_paired = common::pair(&bob).await;
    let eve_paired = common::pair(&eve).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    alice
        .trust_account_identity(&bob_jid, bob_paired.account)
        .await
        .unwrap();
    let to = bob_jid.to_non_ad();
    let client = Client {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(
            bob_jid.clone(),
            FetchedBundle {
                bundle: bundle_for(&bob, 1).await,
                device_identity: Some(bob_paired.device_identity.clone()),
            },
        )]),
        group: None,
        gate: None,
    };
    alice
        .send_direct(&TokioRuntime, &client, &to, &text("for bob only"), "R1")
        .await
        .unwrap();
    // Alice lost the session, so the retry receipt's keys would be used.
    alice.forget_user(&bob_jid.to_non_ad()).await.unwrap();

    // Bob named as requester and encryption address, with Eve's keys.
    let eve_bundle = remote(&bundle_for(&eve, 1).await, Some(eve_paired.device_identity));
    let result = alice
        .resend(
            &client,
            RetryRequest {
                chat: &to,
                message_id: "R1",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 1,
                bundle: Some(&eve_bundle),
            },
        )
        .await;
    assert!(
        matches!(
            result,
            Err(OpsError::BundleRefused(
                BundleRejection::InvalidDeviceIdentity
            ))
        ),
        "got {result:?}"
    );
    assert!(
        !alice
            .has_session(&bob_jid.to_protocol_address())
            .await
            .unwrap()
    );

    // Bob's own keys are still accepted.
    let bob_bundle = remote(&bundle_for(&bob, 1).await, Some(bob_paired.device_identity));
    alice
        .resend(
            &client,
            RetryRequest {
                chat: &to,
                message_id: "R1",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 2,
                bundle: Some(&bob_bundle),
            },
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_group_retry_bundle_leaves_distribution_state_untouched() {
    use wacore::types::message::AddressingMode;

    let group: Jid = "120363000000010@g.us".parse().unwrap();
    let alice = device("alice-group-retry-adv", "111:1@s.whatsapp.net").await;
    alice
        .set_own_jids("111:1@s.whatsapp.net", Some("900111:1@lid"))
        .await
        .unwrap();
    common::pair(&alice).await;
    let bob = device("bob-group-retry-adv", "222:1@s.whatsapp.net").await;
    let eve = device("eve-group-retry-adv", "333:1@s.whatsapp.net").await;
    let bob_paired = common::pair(&bob).await;
    let eve_paired = common::pair(&eve).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    alice
        .trust_account_identity(&bob_jid, bob_paired.account)
        .await
        .unwrap();
    let client = Client {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(
            bob_jid.clone(),
            FetchedBundle {
                bundle: bundle_for(&bob, 1).await,
                device_identity: Some(bob_paired.device_identity),
            },
        )]),
        group: Some(Arc::new(GroupRoutingInfo::new(
            vec!["111@s.whatsapp.net".parse().unwrap(), bob_jid.to_non_ad()],
            AddressingMode::Pn,
        ))),
        gate: None,
    };
    let sent = alice
        .send_group(&TokioRuntime, &client, &group, &text("members"), "G1")
        .await
        .unwrap();
    assert_eq!(sent.distribution_targets, vec![bob_jid.clone()]);
    // The server acknowledged the send, so Bob holds the sender key. A
    // companion counts as warm only with its primary, so mark both.
    let acknowledged = [bob_jid.with_device(0), bob_jid.clone()];
    assert!(
        alice
            .mark_sender_key_distributed(&group, &acknowledged, sent.distribution_epoch)
            .await
            .unwrap()
    );
    // Alice lost Bob's session, so the retry receipt's keys would be used.
    alice.forget_user(&bob_jid.to_non_ad()).await.unwrap();

    let eve_bundle = remote(&bundle_for(&eve, 1).await, Some(eve_paired.device_identity));
    let result = alice
        .resend(
            &client,
            RetryRequest {
                chat: &group,
                message_id: "G1",
                requester: &bob_jid,
                encryption_jid: &bob_jid,
                route: RetryRoute::Group {
                    addressing_mode: AddressingMode::Pn,
                },
                retry_count: 1,
                bundle: Some(&eve_bundle),
            },
        )
        .await;
    assert!(
        matches!(
            result,
            Err(OpsError::BundleRefused(
                BundleRejection::InvalidDeviceIdentity
            ))
        ),
        "got {result:?}"
    );
    assert!(
        !alice
            .has_session(&bob_jid.to_protocol_address())
            .await
            .unwrap(),
        "a refused retry must not create a session"
    );
    // The refused retry started no new epoch, so the original send's
    // acknowledgment still counts. No devices are named, so this check
    // writes no mark of its own.
    assert!(
        alice
            .mark_sender_key_distributed(&group, &[], sent.distribution_epoch)
            .await
            .unwrap(),
        "a refused retry must not change the distribution epoch"
    );
    // Bob's mark survived: the next send does not distribute the key to him
    // again.
    let next = alice
        .send_group(&TokioRuntime, &client, &group, &text("again"), "G2")
        .await
        .unwrap();
    assert!(
        next.distribution_targets.is_empty(),
        "a refused retry must keep Bob's distribution mark, got {:?}",
        next.distribution_targets
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn forgetting_a_user_waits_for_a_session_setup_already_in_flight() {
    let alice = Arc::new(device("alice-forget-race", "111:1@s.whatsapp.net").await);
    let bob = device("bob-forget-race", "222:1@s.whatsapp.net").await;
    let paired = common::pair(&bob).await;
    let bob_jid: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    alice
        .trust_account_identity(&bob_jid, paired.account)
        .await
        .unwrap();
    let (entered, release) = (
        Arc::new(tokio::sync::Notify::new()),
        Arc::new(tokio::sync::Notify::new()),
    );
    let client = Arc::new(Client {
        devices: vec![bob_jid.clone()],
        bundles: HashMap::from([(
            bob_jid.clone(),
            FetchedBundle {
                bundle: bundle_for(&bob, 1).await,
                device_identity: Some(paired.device_identity),
            },
        )]),
        group: None,
        gate: Some((entered.clone(), release.clone())),
    });

    // A first send to Bob is setting up his session.
    let send = {
        let (alice, client, to) = (alice.clone(), client.clone(), bob_jid.to_non_ad());
        tokio::spawn(async move {
            alice
                .send_direct(&TokioRuntime, &*client, &to, &text("hi"), "S1")
                .await
        })
    };
    entered.notified().await;

    // An identity reset for Bob must not finish while that setup can still
    // commit a session it would have deleted.
    let forget = {
        let (alice, user) = (alice.clone(), bob_jid.to_non_ad());
        tokio::spawn(async move { alice.forget_user(&user).await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(!forget.is_finished(), "the reset must wait for the send");

    release.notify_one();
    send.await.unwrap().unwrap();
    forget.await.unwrap().unwrap();
    assert!(
        !alice
            .has_session(&bob_jid.to_protocol_address())
            .await
            .unwrap(),
        "no session may survive the reset"
    );
}

/// `Client` with a known phone-number to LID mapping, so sends set up
/// sessions at the LID address.
struct Mapped {
    inner: Client,
    phones: &'static [&'static str],
    lid: &'static str,
}

#[async_trait]
impl SendContextResolver for Mapped {
    async fn lock_device_sessions(&self, device_jids: &[Jid]) -> SessionLockGuard {
        self.inner.lock_device_sessions(device_jids).await
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

    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<wacore_binary::CompactString> {
        self.phones.contains(&phone_user).then(|| self.lid.into())
    }
}

#[async_trait]
impl SignalResolver for Mapped {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys> {
        self.inner.fetch_verifiable_prekeys(jids).await
    }
}

async fn mapped_client(
    bob: &SignalOps<Store>,
    account: &KeyPair,
    device_identity: Vec<u8>,
) -> Mapped {
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let companion: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    Mapped {
        inner: Client {
            devices: vec![primary.clone(), companion.clone()],
            bundles: HashMap::from([
                (
                    primary,
                    FetchedBundle {
                        bundle: primary_bundle(account),
                        device_identity: None,
                    },
                ),
                (
                    companion,
                    FetchedBundle {
                        bundle: bundle_for(bob, 1).await,
                        device_identity: Some(device_identity),
                    },
                ),
            ]),
            group: None,
            gate: None,
        },
        phones: &["222"],
        lid: "900222",
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_first_send_at_a_mapped_lid_pins_the_account_there_too() {
    let alice = device("alice-mapped", "111:1@s.whatsapp.net").await;
    let bob = device("bob-mapped", "222:1@s.whatsapp.net").await;
    let (account, device_identity) = pair_with_account(&bob).await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let lid_companion: Jid = "900222:1@lid".parse().unwrap();

    let client = mapped_client(&bob, &account, device_identity.clone()).await;
    alice
        .send_direct(&TokioRuntime, &client, &primary, &text("hi"), "M1")
        .await
        .unwrap();
    assert_eq!(
        alice.account_identity(&lid_companion).await.unwrap(),
        Some(*account.public_key.public_key_bytes())
    );

    // A companion that later arrives at the LID verifies against that pin.
    let reset = remote(&bundle_for(&bob, 1).await, Some(device_identity));
    alice
        .establish_session(&lid_companion.to_protocol_address(), &reset)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_first_send_at_a_mapped_lid_refuses_keys_that_conflict_with_its_pin() {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = device("alice-mapped-kept", "111:1@s.whatsapp.net").await;
    let bob = device("bob-mapped-kept", "222:1@s.whatsapp.net").await;
    let (account, device_identity) = pair_with_account(&bob).await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let lid: Jid = "900222@lid".parse().unwrap();
    let accepted = KeyPair::generate(&mut rng);
    let accepted = *accepted.public_key.public_key_bytes();
    alice.trust_account_identity(&lid, accepted).await.unwrap();

    // Bob's PN bundles verify only against the PN pin, but the session would
    // be built at the LID, where another key was accepted: refused.
    let client = mapped_client(&bob, &account, device_identity).await;
    let sent = alice
        .send_direct(&TokioRuntime, &client, &primary, &text("hi"), "M2")
        .await;
    for device in [
        "900222@lid",
        "900222:1@lid",
        "222@s.whatsapp.net",
        "222:1@s.whatsapp.net",
    ] {
        let device: Jid = device.parse().unwrap();
        assert!(
            !alice
                .has_session(&device.to_protocol_address())
                .await
                .unwrap(),
            "no session for {device}"
        );
    }
    if let Ok(sent) = sent {
        let companion: Jid = "222:1@s.whatsapp.net".parse().unwrap();
        assert!(enc_for(&sent.stanza, &primary).is_none());
        assert!(enc_for(&sent.stanza, &companion).is_none());
    }
    assert_eq!(alice.account_identity(&lid).await.unwrap(), Some(accepted));
}

/// Records that hold the first armed write touching account pins until
/// released, as a slow database would.
struct PinWritePause {
    inner: MemoryRecordStore,
    armed: std::sync::atomic::AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[async_trait]
impl crate::records::RecordStore for PinWritePause {
    async fn get(
        &self,
        scope: &str,
        ns: crate::records::Namespace,
        key: &str,
    ) -> wacore::store::error::Result<Option<Vec<u8>>> {
        self.inner.get(scope, ns, key).await
    }

    async fn get_many(
        &self,
        scope: &str,
        ns: crate::records::Namespace,
        keys: &[&str],
    ) -> wacore::store::error::Result<Vec<(String, Vec<u8>)>> {
        self.inner.get_many(scope, ns, keys).await
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: crate::records::Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> wacore::store::error::Result<Vec<String>> {
        self.inner.scan_keys(scope, ns, prefix, limit).await
    }

    async fn write(
        &self,
        fence: &crate::records::Fence,
        ops: &[crate::records::WriteOp],
    ) -> wacore::store::error::Result<()> {
        use crate::records::{Namespace, WriteOp};
        let pins = ops.iter().any(|op| {
            matches!(
                op,
                WriteOp::Put {
                    ns: Namespace::AccountIdentity,
                    ..
                } | WriteOp::Insert {
                    ns: Namespace::AccountIdentity,
                    ..
                }
            )
        });
        if pins && self.armed.swap(false, std::sync::atomic::Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        self.inner.write(fence, ops).await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_explicit_trust_waits_for_a_migration_and_is_not_overwritten() {
    let records = Arc::new(PinWritePause {
        inner: MemoryRecordStore::new(),
        armed: std::sync::atomic::AtomicBool::new(false),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let lease = records
        .inner
        .acquire("alice-migrate-trust", "node-a", 0, 60_000)
        .await
        .unwrap()
        .unwrap();
    let alice = Arc::new(
        SignalOps::create(Arc::new(RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&[5; 32])),
            lease.fence,
        )))
        .await
        .unwrap(),
    );
    let pn: Jid = "222@s.whatsapp.net".parse().unwrap();
    let lid: Jid = "900222@lid".parse().unwrap();
    alice.trust_account_identity(&pn, [1; 32]).await.unwrap();

    records
        .armed
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let migration = {
        let (alice, pn, lid) = (alice.clone(), pn.clone(), lid.clone());
        tokio::spawn(async move { alice.migrate_pn_to_lid(&pn, &lid).await })
    };
    tokio::time::timeout(Duration::from_secs(5), records.entered.notified())
        .await
        .expect("the migration reaches its commit");

    // The application accepts a new key for the LID while the migration's
    // batch, which copies the old PN key there, is still being written.
    // Polled here, so the attempt has certainly reached the account lock.
    let trust = alice.trust_account_identity(&lid, [2; 32]);
    futures::pin_mut!(trust);
    assert!(
        futures::poll!(&mut trust).is_pending(),
        "explicit trust waits for the migration"
    );

    records.release.notify_one();
    migration.await.unwrap().unwrap();
    trust.await.unwrap();
    assert_eq!(alice.account_identity(&lid).await.unwrap(), Some([2; 32]));
}

#[tokio::test(flavor = "multi_thread")]
async fn each_phone_number_mapped_to_one_lid_must_agree_with_its_pin() {
    use wacore::types::message::AddressingMode;
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = device("alice-two-phones", "111:1@s.whatsapp.net").await;
    alice
        .set_own_jids("111:1@s.whatsapp.net", Some("900111:1@lid"))
        .await
        .unwrap();
    common::pair(&alice).await;
    let eve = device("eve-two-phones", "333:1@s.whatsapp.net").await;
    let (eve_account, eve_identity) = pair_with_account(&eve).await;
    let bob_account = KeyPair::generate(&mut rng);
    let lid: Jid = "900222@lid".parse().unwrap();
    alice
        .trust_account_identity(&lid, *bob_account.public_key.public_key_bytes())
        .await
        .unwrap();
    let bob_primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let eve_primary: Jid = "333@s.whatsapp.net".parse().unwrap();
    let eve_companion: Jid = "333:1@s.whatsapp.net".parse().unwrap();
    // Both phone numbers map to the LID pinned for Bob. Bob's key agrees
    // with it and is checked first; Eve's does not.
    let client = Mapped {
        inner: Client {
            devices: vec![
                bob_primary.clone(),
                eve_primary.clone(),
                eve_companion.clone(),
            ],
            bundles: HashMap::from([
                (
                    bob_primary.clone(),
                    FetchedBundle {
                        bundle: primary_bundle(&bob_account),
                        device_identity: None,
                    },
                ),
                (
                    eve_primary.clone(),
                    FetchedBundle {
                        bundle: primary_bundle(&eve_account),
                        device_identity: None,
                    },
                ),
                (
                    eve_companion.clone(),
                    FetchedBundle {
                        bundle: bundle_for(&eve, 1).await,
                        device_identity: Some(eve_identity),
                    },
                ),
            ]),
            group: Some(Arc::new(GroupRoutingInfo::new(
                vec![
                    "111@s.whatsapp.net".parse().unwrap(),
                    bob_primary.clone(),
                    eve_primary.clone(),
                ],
                AddressingMode::Pn,
            ))),
            gate: None,
        },
        phones: &["222", "333"],
        lid: "900222",
    };
    let group: Jid = "120363000000020@g.us".parse().unwrap();
    alice
        .send_group(&TokioRuntime, &client, &group, &text("hi"), "T1")
        .await
        .unwrap();
    // Only Eve's keys could set up a session for device 1 at the LID.
    let lid_companion: Jid = "900222:1@lid".parse().unwrap();
    assert!(
        !alice
            .has_session(&lid_companion.to_protocol_address())
            .await
            .unwrap(),
        "keys the LID pin does not accept built a session there"
    );
    assert_eq!(
        alice.account_identity(&lid).await.unwrap(),
        Some(*bob_account.public_key.public_key_bytes())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_at_a_lid_uses_the_pin_saved_for_its_phone_number() {
    let alice = device("alice-retry-lid", "111:1@s.whatsapp.net").await;
    common::pair(&alice).await;
    let bob = device("bob-retry-lid", "222:1@s.whatsapp.net").await;
    let (account, device_identity) = pair_with_account(&bob).await;
    let primary: Jid = "222@s.whatsapp.net".parse().unwrap();
    let companion: Jid = "222:1@s.whatsapp.net".parse().unwrap();
    let lid_companion: Jid = "900222:1@lid".parse().unwrap();

    // First send, before the LID is known: the pin is saved under the PN.
    let unmapped = Mapped {
        phones: &[],
        ..mapped_client(&bob, &account, device_identity.clone()).await
    };
    alice
        .send_direct(&TokioRuntime, &unmapped, &primary, &text("hi"), "R1")
        .await
        .unwrap();
    assert!(
        alice
            .account_identity(&"900222@lid".parse().unwrap())
            .await
            .unwrap()
            .is_none()
    );

    // Now the companion asks for a retry at its LID, where no session exists.
    let mut mapped = mapped_client(&bob, &account, device_identity.clone()).await;
    mapped.inner.bundles.insert(
        lid_companion.clone(),
        FetchedBundle {
            bundle: bundle_for(&bob, 1).await,
            device_identity: Some(device_identity),
        },
    );
    alice
        .resend(
            &mapped,
            RetryRequest {
                chat: &primary,
                message_id: "R1",
                requester: &companion,
                encryption_jid: &lid_companion,
                route: RetryRoute::Direct { recipient: None },
                retry_count: 1,
                bundle: None,
            },
        )
        .await
        .expect("the PN pin vouches for the LID companion");
    assert_eq!(
        alice
            .account_identity(&"900222@lid".parse().unwrap())
            .await
            .unwrap(),
        Some(*account.public_key.public_key_bytes())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hosted_companion_verifies_against_its_ordinary_accounts_pin() {
    use wacore::adv::test_util::{account_prefix, signed_identity};
    use waproto::whatsapp::ADVEncryptionType;
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = device("alice-hosted", "111:1@s.whatsapp.net").await;
    let account = KeyPair::generate(&mut rng);
    // The account is pinned under its phone number, as its primary would be.
    alice
        .trust_account_identity(
            &"222@s.whatsapp.net".parse().unwrap(),
            *account.public_key.public_key_bytes(),
        )
        .await
        .unwrap();

    // A hosted (Cloud API) companion of that account, with a hosted ADV chain.
    let hosted_device = KeyPair::generate(&mut rng);
    let mut details = waproto::whatsapp::ADVDeviceIdentity::default();
    details.raw_id = Some(1);
    details.key_index = Some(1);
    details.device_type = Some(ADVEncryptionType::HOSTED);
    let details = waproto::codec::adv_device_identity_to_vec(&details);
    let identity = signed_identity(
        &account,
        &hosted_device,
        &details,
        account_prefix(Some(ADVEncryptionType::HOSTED)),
        Some(&[6, 6]),
        false,
    );
    let identity = waproto::codec::adv_signed_device_identity_to_vec(&identity);
    let hosted: Jid = "222:99@hosted".parse().unwrap();
    let bundle = remote(&primary_bundle(&hosted_device), Some(identity.clone()));
    alice
        .establish_session(&hosted.to_protocol_address(), &bundle)
        .await
        .expect("the hosted companion verifies against the phone-number pin");

    // A hosted companion signed by another account is still refused.
    let other = KeyPair::generate(&mut rng);
    let forged = waproto::codec::adv_signed_device_identity_to_vec(&signed_identity(
        &other,
        &hosted_device,
        &details,
        account_prefix(Some(ADVEncryptionType::HOSTED)),
        Some(&[6, 6]),
        false,
    ));
    let forged_jid: Jid = "222:98@hosted".parse().unwrap();
    refused(
        alice
            .establish_session(
                &forged_jid.to_protocol_address(),
                &remote(&primary_bundle(&hosted_device), Some(forged)),
            )
            .await,
        BundleRejection::InvalidDeviceIdentity,
    );
}
