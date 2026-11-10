use super::*;
use std::time::Duration;
use wacore::libsignal::protocol::{IdentityKey, KeyPair};
use wacore::store::commands::DeviceCommand;
use wacore::types::jid::adv_account_jid;

fn fixture(
    account: &KeyPair,
    hosted: bool,
    correct_prefix: bool,
) -> (Vec<EncPayload>, Vec<u8>, [u8; 32]) {
    let device = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let identity = IdentityKey::new(device.public_key);
    let inner = SignalMessage::new(
        3,
        &[0; 32],
        device.public_key,
        0,
        0,
        &[0; 16],
        &identity,
        &identity,
    )
    .unwrap();
    let message =
        PreKeySignalMessage::new(3, 1234, None, 1.into(), device.public_key, identity, inner)
            .unwrap();
    let mut details = waproto::whatsapp::ADVDeviceIdentity::default();
    details.key_index = Some(7);
    details.device_type = Some(if hosted {
        waproto::whatsapp::ADVEncryptionType::HOSTED
    } else {
        waproto::whatsapp::ADVEncryptionType::E2EE
    });
    let certificate = waproto::codec::adv_signed_device_identity_to_vec(
        &wacore::adv::test_util::signed_identity(
            account,
            &device,
            &waproto::codec::adv_device_identity_to_vec(&details),
            if hosted { &[6, 5] } else { &[6, 0] },
            Some(if hosted == correct_prefix {
                &[6, 6]
            } else {
                &[6, 1]
            }),
            true,
        ),
    );
    (
        vec![EncPayload {
            ciphertext: message.serialize().into(),
            enc_type: EncType::PreKeyMessage,
            padding_version: 2,
            enc_index: 0,
            state: None,
            session_type: None,
        }],
        certificate,
        *device.public_key.public_key_bytes(),
    )
}

async fn pair(client: &Client, own: &Jid) {
    let command = if own.server == wacore_binary::Server::Lid {
        DeviceCommand::SetLid(Some(own.with_device(1)))
    } else {
        DeviceCommand::SetId(Some(own.with_device(1)))
    };
    client.persistence_manager.process_command(command).await;
    let mut account = waproto::whatsapp::ADVSignedDeviceIdentity::default();
    account.account_signature_key = Some(
        crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes()
            .to_vec(),
    );
    client
        .persistence_manager
        .process_command(DeviceCommand::SetAccount(Some(account)))
        .await;
}

#[tokio::test]
async fn hosted_own_adv_uses_paired_anchor_without_iq_or_signal_namespace_changes() {
    for own in [Jid::pn("12025550181"), Jid::lid("100000000000181")] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        pair(&client, &own).await;
        for hosted in [false, true] {
            let sender = own.with_device_hosting(if hosted { 99 } else { 7 }, hosted);
            let original = sender.to_protocol_address();
            let (payloads, certificate, identity) =
                fixture(crate::test_utils::adv_test_account(), hosted, true);
            assert_eq!(
                client
                    .preflight_incoming_companion_identity(&sender, Some(&certificate), &payloads)
                    .await,
                None
            );
            let fence = client
                .lock_incoming_companion_account(&sender, Some(&certificate), &payloads)
                .await
                .unwrap();
            assert_eq!(sender.to_protocol_address(), original);
            let anchor_lock = client
                .session_lock_for(own.to_protocol_address().as_str())
                .await;
            assert!(anchor_lock.try_lock().is_none());
            drop(fence);
            assert_eq!(
                transport.sent_count(),
                0,
                "paired trust requires neither IQ nor ACK"
            );
            let backend = client.persistence_manager.backend();
            assert!(
                backend
                    .load_identity(sender.with_device(0).to_protocol_address().as_str())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                client
                    .signal_cache
                    .get_identity(&original, &*backend)
                    .await
                    .unwrap()
                    .is_none()
            );
            // A known device remains keyed by its original Signal namespace.
            client.signal_cache.put_identity(&original, &identity).await;
            assert!(
                client
                    .validate_incoming_companion_identity(&sender, None, &identity)
                    .await
                    .is_ok()
            );
            if hosted {
                assert!(
                    client
                        .signal_cache
                        .get_identity(&own.with_device(99).to_protocol_address(), &*backend)
                        .await
                        .unwrap()
                        .is_none()
                );
            }
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn hosted_and_ordinary_adv_reject_wrong_keys_signatures_and_prefixes() {
    for own in [Jid::pn("12025550182"), Jid::lid("100000000000182")] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        pair(&client, &own).await;
        for hosted in [false, true] {
            let sender = own.with_device_hosting(if hosted { 99 } else { 7 }, hosted);
            let foreign = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            let (_, certificate, identity) = fixture(&foreign, hosted, true);
            assert!(
                client
                    .validate_incoming_companion_identity(&sender, Some(&certificate), &identity)
                    .await
                    .is_err()
            );
            let (_, certificate, identity) =
                fixture(crate::test_utils::adv_test_account(), hosted, false);
            assert!(
                client
                    .validate_incoming_companion_identity(&sender, Some(&certificate), &identity)
                    .await
                    .is_err()
            );
            let (_, certificate, identity) =
                fixture(crate::test_utils::adv_test_account(), hosted, true);
            for account_signature in [false, true] {
                let mut corrupted =
                    waproto::codec::adv_signed_device_identity_decode(&certificate).unwrap();
                let signature = if account_signature {
                    &mut corrupted.account_signature
                } else {
                    &mut corrupted.device_signature
                };
                signature.as_mut().unwrap()[0] ^= 1;
                assert!(
                    client
                        .validate_incoming_companion_identity(
                            &sender,
                            Some(&waproto::codec::adv_signed_device_identity_to_vec(
                                &corrupted
                            )),
                            &identity
                        )
                        .await
                        .is_err()
                );
            }
            assert!(
                client
                    .signal_cache
                    .get_identity(
                        &sender.to_protocol_address(),
                        &*client.persistence_manager.backend()
                    )
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(
            transport.sent_count(),
            0,
            "rejected certificates cannot send ACKs or fetch replacement trust"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn hosted_adv_does_not_borrow_paired_trust_from_the_other_addressing_family() {
    for own in [Jid::pn("12025550183"), Jid::lid("12025550183")] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        pair(&client, &own).await;
        let other = if own.server == wacore_binary::Server::Pn {
            Jid::lid(own.user.clone())
        } else {
            Jid::pn(own.user.clone())
        };
        let sender = other.with_device_hosting(99, true);
        assert!(
            client
                .cached_adv_account_identity(&sender)
                .await
                .unwrap()
                .is_none()
        );
        let (_, certificate, identity) = fixture(crate::test_utils::adv_test_account(), true, true);
        let operation = {
            let client = client.clone();
            tokio::spawn(async move {
                client
                    .validate_incoming_companion_identity(&sender, Some(&certificate), &identity)
                    .await
            })
        };
        let iq = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let node = iq.get();
        let requested = node
            .get_optional_child("key")
            .unwrap()
            .get_optional_child("user")
            .unwrap()
            .attrs()
            .optional_jid("jid")
            .unwrap();
        assert_eq!(
            requested, other,
            "the untrusted family must establish its own ordinary anchor"
        );
        let id = node.attrs().optional_string("id").unwrap();
        let response = wacore_binary::builder::NodeBuilder::new("iq")
            .attr("type", "result")
            .children([wacore_binary::builder::NodeBuilder::new("list").build()])
            .build();
        crate::test_utils::answer_iq(&client, &id, &response).await;
        assert!(operation.await.unwrap().is_err());
        assert_eq!(
            transport.sent_count(),
            1,
            "no ACK for an unverified message"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn hosted_receive_rechecks_the_ordinary_anchor_after_rotation() {
    for own in [Jid::pn("12025550184"), Jid::lid("100000000000184")] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        pair(&client, &own).await;
        let sender = own.with_device_hosting(99, true);
        let (payloads, certificate, identity) =
            fixture(crate::test_utils::adv_test_account(), true, true);
        client
            .validate_incoming_companion_identity(&sender, Some(&certificate), &identity)
            .await
            .unwrap();
        let lock = client
            .session_lock_for(adv_account_jid(&sender).to_protocol_address().as_str())
            .await;
        let recovery = lock.lock_arc().await;
        let baseline = Arc::strong_count(&lock);
        let receive = {
            let client = client.clone();
            let sender = sender.clone();
            tokio::spawn(async move {
                client
                    .lock_incoming_companion_account(&sender, Some(&certificate), &payloads)
                    .await
                    .map(|_| ())
            })
        };
        crate::test_utils::wait_for_lock_waiter(&lock, baseline).await;
        let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        client
            .signal_cache
            .put_identity(
                &own.to_protocol_address(),
                replacement.public_key.public_key_bytes(),
            )
            .await;
        drop(recovery);
        assert!(
            tokio::time::timeout(Duration::from_secs(5), receive)
                .await
                .unwrap()
                .unwrap()
                .is_err()
        );
        assert_eq!(
            client.cached_adv_account_identity(&sender).await.unwrap(),
            Some(*replacement.public_key.public_key_bytes())
        );
        assert!(
            client
                .signal_cache
                .get_identity(
                    &sender.to_protocol_address(),
                    &*client.persistence_manager.backend()
                )
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(transport.sent_count(), 0);
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn hosted_lid_participant_send_and_receive_use_one_account_before_device_lock_order() {
    use crate::store::release::tests::ProbeBackend;
    use wacore::proto_helpers::MessageBuilderExt;
    use wacore::store::traits::{DeviceInfo, DeviceListRecord};
    let peer = Jid::lid("100000000000187");
    let hosted = peer.with_device_hosting(99, true);
    let own = Jid::pn("12025550187");
    let (mut probe, _) = ProbeBackend::new();
    // Persist a real Signal session before constructing the observed client,
    // leaving its cache cold so the send crosses the backend admission gate.
    let (scratch, _) =
        crate::test_utils::create_iq_test_client_with_backend(probe.backend.clone()).await;
    crate::test_utils::seed_peer_session(&scratch, &hosted).await;
    crate::test_utils::seed_adv_primary(&scratch, &hosted).await;
    scratch
        .signal_cache
        .flush(probe.backend.as_ref())
        .await
        .unwrap();
    probe
        .backend
        .save(&scratch.persistence_manager.get_device_snapshot())
        .await
        .unwrap();
    scratch.signal_shutdown_sync();

    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "after:get_session";
    let (client, transport) =
        crate::test_utils::create_iq_test_client_with_backend(Arc::new(probe)).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(own.clone())))
        .await;
    for (user, devices) in [
        (own.user.as_str(), vec![DeviceInfo::new(0, None)]),
        (
            peer.user.as_str(),
            vec![DeviceInfo::new(99, Some(7)).with_hosting(true)],
        ),
    ] {
        client
            .update_device_list(DeviceListRecord {
                user: user.into(),
                devices: devices.into_boxed_slice(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
    }
    let (payloads, certificate, identity) =
        fixture(crate::test_utils::adv_test_account(), true, true);
    client
        .validate_incoming_companion_identity(&hosted, Some(&certificate), &identity)
        .await
        .unwrap();
    let fence = client
        .lock_incoming_companion_account(&hosted, Some(&certificate), &payloads)
        .await
        .unwrap();
    let anchor = client
        .session_lock_for(peer.to_protocol_address().as_str())
        .await;
    let device = client
        .session_lock_for(hosted.to_protocol_address().as_str())
        .await;
    let baseline = Arc::strong_count(&anchor);
    let mut send = {
        let client = client.clone();
        let peer = peer.clone();
        let hosted = hosted.clone();
        tokio::spawn(async move {
            // Normal DM fanout deliberately excludes hosted devices. This
            // public pairwise builder retains them and uses the production
            // send lock ordering, real Signal encryption and pre-wire commit.
            let message = waproto::whatsapp::Message::text("synthetic hosted lock ordering");
            let (nodes, _) = client
                .signal()
                .create_participant_nodes(std::slice::from_ref(&peer), &message)
                .await?;
            assert_eq!(nodes.len(), 1);
            assert_eq!(
                nodes[0].as_node_ref().attrs().optional_jid("jid"),
                Some(hosted)
            );
            client
                .send_node(
                    wacore_binary::builder::NodeBuilder::new("message")
                        .attr("to", peer)
                        .attr("id", "SYNTHETIC_HOSTED_PARTICIPANT_SEND")
                        .attr("type", "text")
                        .children([wacore_binary::builder::NodeBuilder::new("participants")
                            .children(nodes)
                            .build()])
                        .build(),
                )
                .await?;
            Ok::<(), anyhow::Error>(())
        })
    };
    tokio::select! {
        result = &mut send => panic!("participant send finished before its backend gate: {result:?}"),
        arrived = tokio::time::timeout(Duration::from_secs(5), arrived.recv()) => {
            arrived.expect("participant send reaches the cold session read").unwrap();
        },
    }
    release.send(()).await.unwrap();
    crate::test_utils::wait_for_lock_waiter(&anchor, baseline).await;
    assert!(
        device.try_lock().is_some(),
        "send must wait on the ordinary LID account before taking the hosted device"
    );
    let info = Arc::new(MessageInfo {
        id: "SYNTHETIC_HOSTED_LOCK_ORDER".into(),
        source: crate::types::message::MessageSource {
            chat: peer.clone(),
            sender: hosted.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    // Real receive decryption owns the acquired account fence. The fixture's
    // absent signed prekey fails safely; it must still release both locks.
    tokio::time::timeout(
        Duration::from_secs(5),
        client.clone().decrypt_session_enc_batch_with_origin(
            payloads,
            &info,
            &hosted,
            crate::types::events::DecryptFailMode::Show,
            None,
            (raw_pending::StanzaOrigin::new(), fence),
        ),
    )
    .await
    .unwrap();
    tokio::time::timeout(Duration::from_secs(5), send)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        transport.sent_count() > 0,
        "production send reaches the Noise transport"
    );
    assert!(anchor.try_lock().is_some());
    assert!(device.try_lock().is_some());
    assert!(
        client
            .persistence_manager
            .backend()
            .load_identity(hosted.with_device(0).to_protocol_address().as_str())
            .await
            .unwrap()
            .is_none()
    );
    client.signal_shutdown_sync();
}
