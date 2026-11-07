//! Missed primary rotations must recover ordinary lists without trusting their envelope key.

use super::tests::{adv_wire_response, has_session, seed_fresh_session, signed_key_index_bytes};
use super::*;
use crate::cache::Freshness;
use crate::test_utils::{TestEventCollector, answer_iq, create_iq_test_client, decode_sent_iq};
use std::sync::Arc;
use std::time::Duration;
use wacore::libsignal::protocol::KeyPair;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::events::{Event, EventInterest, EventKind};
use wacore::types::jid::JidExt as _;
use wacore_binary::builder::NodeBuilder;

fn primary_response(user: &Jid, account: &KeyPair, valid: bool) -> wacore_binary::Node {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signed = KeyPair::generate(&mut rng);
    let mut signature = account
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap()
        .to_vec();
    if !valid {
        signature[0] ^= 1;
    }
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("list")
            .children([NodeBuilder::new("user")
                .attr("jid", user.to_string())
                .children([
                    NodeBuilder::new("registration")
                        .bytes(1234u32.to_be_bytes().to_vec())
                        .build(),
                    NodeBuilder::new("type").bytes(vec![5]).build(),
                    NodeBuilder::new("identity")
                        .bytes(account.public_key.public_key_bytes().to_vec())
                        .build(),
                    NodeBuilder::new("skey")
                        .children([
                            NodeBuilder::new("id").bytes(vec![0, 0, 1]).build(),
                            NodeBuilder::new("value")
                                .bytes(signed.public_key.public_key_bytes().to_vec())
                                .build(),
                            NodeBuilder::new("signature").bytes(signature).build(),
                        ])
                        .build(),
                ])
                .build()])
            .build()])
        .build()
}

fn signed_list(account: &KeyPair, embedded_key: bool) -> Vec<u8> {
    let mut signed =
        waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
            .unwrap();
    signed.account_signature = Some(
        account
            .private_key
            .calculate_signature(
                &[&[6, 2], signed.details.as_ref().unwrap().as_slice()].concat(),
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .unwrap()
            .to_vec(),
    );
    signed.account_signature_key =
        embedded_key.then(|| account.public_key.public_key_bytes().to_vec());
    waproto::codec::adv_signed_key_index_list_to_vec(&signed)
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Rotated,
    InvalidPrekey,
    WrongPrimary,
    TopologyChanged,
    IdentityChanged,
    AliasIdentityChanged,
}

/// Primary installers cannot restore an old session while trust is being saved.
#[tokio::test]
async fn ordinary_recovery_holds_both_primary_locks_through_identity_publication() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use wacore::usync::{UserDeviceList, UsyncDevice};

    let (mut probe, _) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "after:put_identity";
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let phone = Jid::pn("12025550192");
    let lid = Jid::lid("100000000000192");
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            phone.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
        seed_fresh_session(&client, user).await;
    }
    client
        .update_device_list(DeviceListRecord {
            user: phone.user.as_str().into(),
            devices: [DeviceInfo::new(0, None)].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let response = DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: phone.clone(),
            devices: vec![UsyncDevice::new(0, None)],
            phash: None,
            key_index_bytes: Some(signed_list(&account, false)),
        }],
        lid_mappings: vec![],
    };
    let error = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap_err();
    let caller = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&phone, &account, true)).await;
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        for user in [&phone, &lid] {
            let mutex = client
                .session_lock_for(user.to_protocol_address().as_str())
                .await;
            assert!(
                mutex.try_lock().is_none(),
                "primary lock must cover the durable trust write"
            );
            assert!(!has_session(&client, &user.to_protocol_address()).await);
        }
        release.send(()).await.unwrap();
    }
    tokio::time::timeout(Duration::from_secs(5), caller)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    for user in [&phone, &lid] {
        assert_eq!(
            client.cached_adv_account_identity(user).await.unwrap(),
            Some(*account.public_key.public_key_bytes())
        );
        let mutex = client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
        assert!(mutex.try_lock().is_some());
        assert!(!has_session(&client, &user.to_protocol_address()).await);
    }
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn ordinary_list_ignores_envelope_key_without_rotating_trust() {
    for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
        let (client, transport) = create_iq_test_client().await;
        let user = Jid::pn("12025550191");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        let trusted = client.cached_adv_account_identity(&user).await.unwrap();
        let collector = Arc::new(TestEventCollector::default());
        let subscription = client.subscribe_handler(collector.clone());
        assert!(subscription.update_interest(EventInterest::of(&[EventKind::IdentityChange])));
        subscription.detach();
        let primary = seed_fresh_session(&client, &user).await;
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(
            KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                .public_key
                .public_key_bytes()
                .to_vec(),
        );
        let response = adv_wire_response(
            &user,
            waproto::codec::adv_signed_key_index_list_to_vec(&signed),
        );
        let task = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move {
                client
                    .fetch_user_devices_with_freshness(vec![user], freshness)
                    .await
            })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &response).await;
        let devices = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(devices, vec![user.clone(), user.with_device(2)]);
        assert_eq!(transport.sent().len(), 1);
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            trusted
        );
        assert!(has_session(&client, &primary).await);
        assert!(collector.events().is_empty());
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn ordinary_rotation_recovers_only_with_a_fresh_authenticated_primary() {
    for user in [Jid::pn("12025550190"), Jid::lid("100000000000190")] {
        for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
            for embedded_key in [false, true] {
                for outcome in [
                    Outcome::Rotated,
                    Outcome::InvalidPrekey,
                    Outcome::WrongPrimary,
                    Outcome::TopologyChanged,
                    Outcome::IdentityChanged,
                    Outcome::AliasIdentityChanged,
                ] {
                    let (client, transport) = create_iq_test_client().await;
                    let alias = if user.is_pn() {
                        Jid::lid("100000000000190")
                    } else {
                        Jid::pn("12025550190")
                    };
                    client
                        .lid_pn_cache
                        .add(&crate::lid_pn_cache::LidPnEntry::new(
                            "100000000000190".to_string(),
                            "12025550190".to_string(),
                            crate::lid_pn_cache::LearningSource::Usync,
                        ))
                        .await;
                    crate::test_utils::seed_adv_primary(&client, &user).await;
                    crate::test_utils::seed_adv_primary(&client, &alias).await;
                    let old_key = client
                        .cached_adv_account_identity(&user)
                        .await
                        .unwrap()
                        .unwrap();
                    let collector = Arc::new(TestEventCollector::default());
                    let subscription = client.subscribe_handler(collector.clone());
                    assert!(
                        subscription
                            .update_interest(EventInterest::of(&[EventKind::IdentityChange]))
                    );
                    subscription.detach();
                    client
                        .update_device_list(DeviceListRecord {
                            user: user.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(2, Some(1))].into(),
                            raw_id: Some(1),
                            timestamp: 1,
                            phash: None,
                        })
                        .await
                        .unwrap();
                    let primary_session = seed_fresh_session(&client, &user).await;
                    let companion_session = seed_fresh_session(&client, &user.with_device(2)).await;
                    let alias_primary_session = seed_fresh_session(&client, &alias).await;
                    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let response = adv_wire_response(&user, signed_list(&account, embedded_key));
                    let task = {
                        let client = client.clone();
                        let user = user.clone();
                        tokio::spawn(async move {
                            client
                                .fetch_user_devices_with_freshness(vec![user], freshness)
                                .await
                        })
                    };
                    for index in 0..2 {
                        let iq = decode_sent_iq(&transport, index).await;
                        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
                        answer_iq(&client, &id, &response).await;
                    }
                    let iq = decode_sent_iq(&transport, 2).await;
                    let node = iq.get();
                    assert_eq!(
                        node.attrs().optional_string("xmlns").as_deref(),
                        Some("encrypt"),
                        "user={user} freshness={freshness:?} embedded={embedded_key} outcome={outcome:?}"
                    );
                    assert_eq!(
                        node.get_optional_child("key")
                            .unwrap()
                            .get_optional_child("user")
                            .unwrap()
                            .attrs()
                            .jid("jid"),
                        user
                    );
                    assert_eq!(
                        client.cached_adv_account_identity(&user).await.unwrap(),
                        Some(old_key)
                    );
                    assert!(has_session(&client, &primary_session).await);
                    assert!(has_session(&client, &companion_session).await);
                    let mut expected_key = old_key;
                    let mut expected_alias_key = old_key;
                    match outcome {
                        Outcome::TopologyChanged => {
                            client.invalidate_device_cache(&user.user).await
                        }
                        Outcome::IdentityChanged => {
                            expected_key =
                                *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                                    .public_key
                                    .public_key_bytes();
                            client
                                .signal_cache
                                .put_identity(&user.to_protocol_address(), &expected_key)
                                .await;
                        }
                        Outcome::AliasIdentityChanged => {
                            expected_alias_key =
                                *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                                    .public_key
                                    .public_key_bytes();
                            client
                                .signal_cache
                                .put_identity(&alias.to_protocol_address(), &expected_alias_key)
                                .await;
                        }
                        _ => {}
                    }
                    let different = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let key = if matches!(outcome, Outcome::WrongPrimary) {
                        &different
                    } else {
                        &account
                    };
                    let prekey =
                        primary_response(&user, key, !matches!(outcome, Outcome::InvalidPrekey));
                    let id = node.attrs().optional_string("id").unwrap().into_owned();
                    answer_iq(&client, &id, &prekey).await;
                    let result = tokio::time::timeout(Duration::from_secs(5), task)
                        .await
                        .unwrap()
                        .unwrap();
                    if matches!(outcome, Outcome::Rotated) {
                        assert_eq!(result.unwrap(), vec![user.clone(), user.with_device(2)]);
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(!has_session(&client, &primary_session).await);
                        assert!(!has_session(&client, &alias_primary_session).await);
                        assert!(!has_session(&client, &companion_session).await);
                        assert_eq!(
                            client.cached_adv_account_identity(&alias).await.unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(collector.events().iter().any(|event| matches!(&**event,
                        Event::IdentityChange(change) if change.implicit)));
                    } else {
                        assert!(result.is_err(), "{outcome:?} must fail closed");
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            Some(expected_key)
                        );
                        assert!(has_session(&client, &primary_session).await);
                        assert!(has_session(&client, &alias_primary_session).await);
                        assert!(has_session(&client, &companion_session).await);
                        assert_eq!(
                            client.cached_adv_account_identity(&alias).await.unwrap(),
                            Some(expected_alias_key)
                        );
                        assert!(collector.events().is_empty());
                    }
                    assert_eq!(transport.sent().len(), 3, "recovery must be bounded");
                    client.signal_shutdown_sync();
                }
            }
        }
    }
}
