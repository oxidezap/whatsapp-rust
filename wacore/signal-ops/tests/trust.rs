// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Peer prekey bundles must be bound to the peer account before a session is
//! built from them, whether they come from a send's fetch, a retry receipt
//! or `establish_session`.

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
use wacore::types::jid::JidExt as _;
use wacore_binary::{Jid, NodeContent};
use wacore_recordstore::{AesGcmSealer, LeaseStore, MemoryRecordStore, RecordSignalStore};
use wacore_signal_ops::{
    BundleRejection, FetchedBundle, OpsError, ReceiveKind, ReceiveRequest, Received, RemoteBundle,
    RetryRequest, RetryRoute, SignalOps, SignalResolver, VerifiablePrekeys,
};
use waproto::whatsapp as wa;

mod common;
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
    };
    let sent = alice
        .send_group(&TokioRuntime, &client, &group, &text("members"), "G1")
        .await
        .unwrap();
    assert_eq!(sent.distribution_targets, vec![bob_jid.clone()]);
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
    // The refused retry started no new epoch, so the original send's
    // acknowledgment still counts.
    assert!(
        alice
            .mark_sender_key_distributed(
                &group,
                &sent.distribution_targets,
                sent.distribution_epoch
            )
            .await
            .unwrap(),
        "a refused retry must not change the distribution epoch"
    );
}
