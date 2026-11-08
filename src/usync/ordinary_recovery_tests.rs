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

/// Produce real signed Signal material, retaining the validated certificate anchor.
fn session_bundle(
    identity: &KeyPair,
    device: u16,
    account: Option<&KeyPair>,
) -> wacore::libsignal::protocol::PreKeyBundle {
    use wacore::libsignal::protocol::{IdentityKey, PreKeyBundle};
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signed = KeyPair::generate(&mut rng);
    let signature = identity
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap();
    let bundle = PreKeyBundle::new(
        1234,
        u32::from(device).into(),
        None,
        1.into(),
        signed.public_key,
        signature.to_vec(),
        IdentityKey::new(identity.public_key),
    )
    .unwrap();
    match account {
        Some(account) => bundle.with_account_identity(IdentityKey::new(account.public_key)),
        None => bundle,
    }
}

/// Encode a fictitious primary IQ response with a valid or corrupted prekey.
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

/// Sign the shared device-list fixture with a chosen account key.
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

/// Report the real processing-permit slow path, so the test can order a
/// receiver's primary lock attempt after recovery has started waiting.
struct PermitWaitRuntime(async_channel::Sender<()>);

impl wacore::runtime::Runtime for PermitWaitRuntime {
    fn spawn(
        &self,
        future: wacore::runtime::BoxFuture<'static, ()>,
    ) -> wacore::runtime::AbortHandle {
        wacore::runtime::Runtime::spawn(&crate::runtime_impl::TokioRuntime, future)
    }
    fn sleep(&self, duration: Duration) -> wacore::runtime::BoxFuture<'static, ()> {
        if duration == Duration::from_secs(10) {
            let _ = self.0.try_send(());
        }
        wacore::runtime::Runtime::sleep(&crate::runtime_impl::TokioRuntime, duration)
    }
    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> wacore::runtime::BoxFuture<'static, ()> {
        wacore::runtime::Runtime::spawn_blocking(&crate::runtime_impl::TokioRuntime, f)
    }
    fn yield_now(&self) -> Option<wacore::runtime::BoxFuture<'static, ()>> {
        wacore::runtime::Runtime::yield_now(&crate::runtime_impl::TokioRuntime)
    }
}

/// Before the fix this parks the recovery on the sole permit while the
/// permit's receiver waits for the primary mutex held by that recovery.
#[tokio::test]
async fn offline_recovery_admits_processing_before_primary_locks() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (waiting, recovery_waiting) = async_channel::unbounded();
    let (client, transport) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550199");
    crate::test_utils::seed_adv_primary(&client, &user).await;
    let old = client.cached_adv_account_identity(&user).await.unwrap();
    seed_fresh_session(&client, &user).await;
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let response = DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: user.clone(),
            devices: vec![UsyncDevice::new(0, None)],
            phash: None,
            key_index_bytes: Some(signed_list(&replacement, false)),
        }],
        lid_mappings: vec![],
    };
    let error = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap_err();
    client.enter_drain_mode_for_tests();
    assert!(!client.inbound_commit_batch.has_entries());
    let mutex = client
        .session_lock_for(user.to_protocol_address().as_str())
        .await;
    let (entered, owns_permit) = async_channel::unbounded();
    let (decrypt, start_decrypt) = async_channel::unbounded();
    let inbound = {
        let client = client.clone();
        tokio::spawn(async move {
            let _permit = client.acquire_message_processing_permit().await;
            entered.send(()).await.unwrap();
            start_decrypt.recv().await.unwrap();
            let _session = mutex.lock_arc().await;
        })
    };
    owns_permit.recv().await.unwrap();
    let recovery = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&user, &replacement, true)).await;
    tokio::time::timeout(Duration::from_secs(5), recovery_waiting.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        old
    );
    decrypt.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), inbound)
        .await
        .expect("recovery must not own the primary while awaiting the drain permit")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), recovery)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        Some(*replacement.public_key.public_key_bytes())
    );
    assert!(!has_session(&client, &user.with_device(7).to_protocol_address()).await);
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn stale_maintenance_permit_cannot_flush_a_new_drain() {
    let (client, _) = create_iq_test_client().await;
    let scope = client.begin_signal_maintenance().await.unwrap();
    client.enter_drain_mode_for_tests();
    assert!(scope.flush().await.is_err());
    drop(scope);
    client.signal_shutdown_sync();
}

/// Response replay may restore a mapping evicted before recovery captured its
/// session locks. That newly visible alias must not be reset without its lock.
#[tokio::test]
async fn ordinary_recovery_rejects_an_alias_restored_after_its_lock_snapshot() {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    let (client, transport) = create_iq_test_client().await;
    let phone = Jid::pn("12025550193");
    let lid = Jid::lid("100000000000193");
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
        seed_fresh_session(&client, user).await;
    }
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            phone.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    client.lid_pn_cache.clear().await;
    assert!(client.device_list_identity_alias(&phone).await.is_none());
    let alias_lock = client
        .session_lock_for(lid.to_protocol_address().as_str())
        .await;
    let alias_guard = alias_lock.lock_arc().await;
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let error = DeviceListAdvError {
        user: phone.clone(),
        hosted: false,
        response: DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: phone.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: Some(signed_list(&replacement, false)),
            }],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.user.clone(),
                lid: lid.user.clone(),
            }],
        },
        topology_generation: client.device_topology.current(),
    };
    let recovery = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error.into()).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&phone, &replacement, true)).await;
    let error = tokio::time::timeout(Duration::from_secs(5), recovery)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(
        error.to_string().contains("account alias changed"),
        "{error:#}"
    );
    assert!(has_session(&client, &lid.to_protocol_address()).await);
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        Some(
            *crate::test_utils::adv_test_account()
                .public_key
                .public_key_bytes()
        )
    );
    drop(alias_guard);
    client.signal_shutdown_sync();
}

/// A different contact's refresh must wait outside the global topology locks.
/// Otherwise a recovery owning the drain permit can wait on that refresh
/// forever while the refresh waits for the permit to persist its cleanup.
#[tokio::test]
async fn offline_refresh_waits_for_processing_before_global_topology_locks() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550194");
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let refresh = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .process_device_list_response(
                    &DeviceListResponse {
                        device_lists: vec![UserDeviceList {
                            user,
                            devices: vec![UsyncDevice::new(0, None)],
                            phash: None,
                            key_index_bytes: None,
                        }],
                        lid_mappings: vec![],
                    },
                    Freshness::Refresh,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    let mapping = tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
        .await
        .expect("refresh must not hold mappings while waiting for processing");
    let registry = tokio::time::timeout(
        Duration::from_secs(5),
        client.device_topology.lock_registry(),
    )
    .await
    .expect("refresh must not hold registry while waiting for processing");
    drop(registry);
    drop(mapping);
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), refresh)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn offline_device_removal_releases_registry_before_waiting_for_processing() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550199");
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let removal = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move { client.patch_device_remove(&user.user, 7).await })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    let registry = tokio::time::timeout(
        Duration::from_secs(5),
        client.device_topology.lock_registry(),
    )
    .await
    .expect("notification must release registry before waiting for processing");
    drop(registry);
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), removal)
        .await
        .unwrap()
        .unwrap();
    assert!(!has_session(&client, &user.with_device(7).to_protocol_address()).await);
    client.signal_shutdown_sync();
}

/// A crash while waiting for the drain permit must still leave the previous
/// raw_id in durable storage, so replay knows companion cleanup is required.
#[tokio::test]
async fn offline_device_add_does_not_publish_raw_id_before_durable_session_retirement() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::stanza::devices::{DeviceElement, KeyIndexInfo};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550197");
    crate::test_utils::seed_adv_primary(&client, &user).await;
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(2),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    let backend = client.persistence_manager.backend();
    let address = user.with_device(7).to_protocol_address();
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_some()
    );
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let add = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move {
            client
                .patch_device_add(
                    &user.user,
                    &DeviceElement {
                        jid: user.with_device(7),
                        key_index: Some(1),
                        lid: None,
                    },
                    Some(&KeyIndexInfo {
                        timestamp: 100,
                        signed_bytes: Some(signed_key_index_bytes(vec![1], 1)),
                    }),
                )
                .await;
        })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        backend
            .get_devices(&user.user)
            .await
            .unwrap()
            .unwrap()
            .raw_id,
        Some(2)
    );
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_some()
    );
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), add)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        backend
            .get_devices(&user.user)
            .await
            .unwrap()
            .unwrap()
            .raw_id,
        Some(1)
    );
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_none()
    );
    client.signal_shutdown_sync();
}

/// Storage round trips stay bounded for cold groups while either direction's
/// durable history still forces admission before publishing the mapping.
#[tokio::test]
async fn mapping_history_uses_batches_for_both_cold_and_persisted_aliases() {
    use crate::store::release::tests::ProbeBackend;
    use wacore::usync::UsyncLidMapping;
    let mappings: Vec<_> = (0..256)
        .map(|index| UsyncLidMapping {
            phone_number: format!("1555000{index:05}").into(),
            lid: format!("100000000{index:06}").into(),
        })
        .collect();
    for method in ["get_devices", "get_devices_batch"] {
        let (mut probe, _) = ProbeBackend::new();
        let storage = probe.backend.clone();
        let (entered, calls) = async_channel::unbounded();
        probe.entered = Some(entered);
        probe.gate_method = method;
        let (client, _) =
            crate::test_utils::create_iq_test_client_with_backend(Arc::new(probe)).await;
        while calls.try_recv().is_ok() {}
        assert!(!client.device_mapping_has_registry_history(&mappings).await);
        storage
            .update_device_list(DeviceListRecord {
                user: mappings[255].lid.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        assert!(client.device_mapping_has_registry_history(&mappings).await);
        assert_eq!(
            calls.len(),
            if method == "get_devices_batch" { 2 } else { 0 },
            "history checks must not issue individual record reads"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn group_recovery_budget_counts_requests_across_repeated_batches() {
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (client, transport) = create_iq_test_client().await;
    let users = [
        Jid::pn("12025550201"),
        Jid::pn("12025550202"),
        Jid::pn("12025550203"),
    ];
    for user in &users {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    let signer = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let bytes = signed_list(&signer, false);
    let response_for = |users: &[Jid]| DeviceListResponse {
        device_lists: users
            .iter()
            .map(|user| UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: Some(bytes.clone()),
            })
            .collect(),
        lid_mappings: vec![],
    };
    let first = response_for(&users[..1]);
    let later = response_for(&users[1..]);
    let operation = {
        let client = client.clone();
        tokio::spawn(async move {
            let mut budget = GroupAdvRecoveryBudget {
                remaining: 2,
                attempted: std::collections::HashSet::new(),
            };
            client
                .recover_group_adv_identities(&first, &mut budget)
                .await;
            // A topology retry must not repeat a known failed member.
            client
                .recover_group_adv_identities(&first, &mut budget)
                .await;
            client
                .recover_group_adv_identities(&later, &mut budget)
                .await;
            budget
        })
    };
    let wrong_primary = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    for (round, user) in users[..2].iter().enumerate() {
        for step in 0..3 {
            let iq = decode_sent_iq(&transport, round * 3 + step).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            if step == 2 {
                let requested = iq
                    .get()
                    .get_optional_child("key")
                    .unwrap()
                    .get_optional_child("user")
                    .unwrap()
                    .attrs()
                    .jid("jid");
                assert_eq!(&requested, user);
                answer_iq(&client, &id, &primary_response(user, &wrong_primary, true)).await;
            } else {
                let requested = iq
                    .get()
                    .get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap();
                assert_eq!(requested[0].attrs().jid("jid"), *user);
                answer_iq(&client, &id, &adv_wire_response(user, bytes.clone())).await;
            }
        }
    }
    let budget = tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(budget.remaining, 0);
    assert_eq!(budget.attempted.len(), 2);
    assert_eq!(
        transport.sent().len(),
        6,
        "only two eligible members may issue recovery IQs"
    );
    for user in &users {
        assert_eq!(
            client.cached_adv_account_identity(user).await.unwrap(),
            Some(
                *crate::test_utils::adv_test_account()
                    .public_key
                    .public_key_bytes()
            )
        );
    }
    client.signal_shutdown_sync();
}

/// Real installers cannot restore old sessions while a cancelled caller's
/// recovery worker saves trust; orphan companions are retired under both aliases.
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
        let bundle = session_bundle(crate::test_utils::adv_test_account(), 0, None);
        client
            .install_prekey_bundle_cached(
                user,
                &bundle,
                &mut client.signal_adapter(),
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .await
            .unwrap();
    }
    let orphan = lid.with_device(7);
    let device = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let old_companion = session_bundle(&device, 7, Some(crate::test_utils::adv_test_account()));
    client
        .install_prekey_bundle_cached(
            &orphan,
            &old_companion,
            &mut client.signal_adapter(),
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .unwrap();
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
    let mut caller = Some({
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error).await })
    });
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&phone, &account, true)).await;
    let mut installers = Vec::new();
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let (started, entered) = async_channel::unbounded();
        for user in [&phone, &lid] {
            let mut bundle = session_bundle(crate::test_utils::adv_test_account(), 0, None);
            // The SDK records this observation before a primary key IQ.
            bundle.set_identity_snapshot(Some(
                *crate::test_utils::adv_test_account()
                    .public_key
                    .public_key_bytes(),
            ));
            let client = client.clone();
            let user = user.clone();
            let started = started.clone();
            installers.push(tokio::spawn(async move {
                started.send(()).await.unwrap();
                client
                    .install_prekey_bundle_cached(
                        &user,
                        &bundle,
                        &mut client.signal_adapter(),
                        &mut rand::make_rng::<rand::rngs::StdRng>(),
                    )
                    .await
            }));
        }
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
        assert!(!has_session(&client, &orphan.to_protocol_address()).await);
        let running = client.clone();
        let waiting_orphan = orphan.clone();
        let waiting_bundle = old_companion.clone();
        installers.push(tokio::spawn(async move {
            started.send(()).await.unwrap();
            running
                .install_prekey_bundle_cached(
                    &waiting_orphan,
                    &waiting_bundle,
                    &mut running.signal_adapter(),
                    &mut rand::make_rng::<rand::rngs::StdRng>(),
                )
                .await
        }));
        for _ in 0..3 {
            tokio::time::timeout(Duration::from_secs(5), entered.recv())
                .await
                .unwrap()
                .unwrap();
        }
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert!(
            installers.iter().all(|task| !task.is_finished()),
            "real installers must remain blocked until both aliases publish"
        );
        if let Some(caller) = caller.take() {
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
        }
        release.send(()).await.unwrap();
    }
    for installer in installers {
        assert!(
            tokio::time::timeout(Duration::from_secs(5), installer)
                .await
                .unwrap()
                .unwrap()
                .is_err(),
            "a retained primary or companion bundle must not restore old trust"
        );
    }
    // Acquiring the guards is a completion barrier for the detached worker,
    // independent of the cancelled requester's join handle.
    for user in [&phone, &lid] {
        let mutex = client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
        drop(
            tokio::time::timeout(Duration::from_secs(5), mutex.lock_arc())
                .await
                .unwrap(),
        );
    }
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
    let record = client.load_device_record_for_jid(&phone).await.unwrap();
    assert!(
        record.timestamp > 1,
        "the detached worker must publish topology"
    );
    assert!(
        client
            .install_prekey_bundle_cached(
                &orphan,
                &old_companion,
                &mut client.signal_adapter(),
                &mut rand::make_rng::<rand::rngs::StdRng>()
            )
            .await
            .is_err(),
        "a retained old certificate must fail after rotation"
    );
    assert!(!has_session(&client, &orphan.to_protocol_address()).await);
    let fresh = session_bundle(&device, 7, Some(&account));
    client
        .install_prekey_bundle_cached(
            &orphan,
            &fresh,
            &mut client.signal_adapter(),
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .unwrap();
    assert!(has_session(&client, &orphan.to_protocol_address()).await);
    client.signal_shutdown_sync();
}

/// Mix a rotated contact, a valid contact and an omitted cached group member.
async fn check_group_rotation_after_migration(migrate: bool) {
    for recover in [true, false] {
        let (client, transport) = create_iq_test_client().await;
        let rotated = Jid::pn("12025550193");
        let valid = Jid::pn("12025550194");
        let omitted = Jid::pn("12025550195");
        for user in [&rotated, &valid, &omitted] {
            crate::test_utils::seed_adv_primary(&client, user).await;
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                    raw_id: Some(1),
                    timestamp: 1,
                    phash: None,
                })
                .await
                .unwrap();
        }
        let lid = Jid::lid("100000000000193");
        let old = client.cached_adv_account_identity(&rotated).await.unwrap();
        if migrate {
            client
                .add_lid_pn_mapping(
                    &lid.user,
                    &rotated.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
                .unwrap();
            assert_eq!(
                client.cached_adv_account_identity(&rotated).await.unwrap(),
                None
            );
            assert_eq!(client.cached_adv_account_identity(&lid).await.unwrap(), old);
        }
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let rotated_wire = adv_wire_response(&rotated, signed_list(&account, true));
        let valid_wire = adv_wire_response(&valid, signed_key_index_bytes(vec![1], 1));
        let users: Vec<_> = [&rotated_wire, &valid_wire]
            .iter()
            .map(|wire| {
                wire.get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap()[0]
                    .clone()
            })
            .collect();
        let group_wire = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list").children(users).build()])
                .build()])
            .build();
        let caller = {
            let client = client.clone();
            let users = vec![rotated.clone(), valid.clone(), omitted.clone()];
            tokio::spawn(async move { client.refresh_group_user_devices(users).await })
        };
        for index in 0..3 {
            let iq = decode_sent_iq(&transport, index).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(
                &client,
                &id,
                if index == 0 {
                    &group_wire
                } else {
                    &rotated_wire
                },
            )
            .await;
        }
        let iq = decode_sent_iq(&transport, 3).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&rotated, &account, recover)).await;
        if recover {
            let iq = decode_sent_iq(&transport, 4).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(&client, &id, &group_wire).await;
        }
        let devices = tokio::time::timeout(Duration::from_secs(5), caller)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(devices.contains(&valid.with_device(2)));
        assert!(devices.contains(&omitted.with_device(7)));
        assert!(devices.contains(&rotated.with_device(if recover { 2 } else { 7 })));
        assert_eq!(
            client.cached_adv_account_identity(&rotated).await.unwrap(),
            if recover {
                Some(*account.public_key.public_key_bytes())
            } else if migrate {
                None
            } else {
                old
            }
        );
        if migrate {
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                if recover {
                    Some(*account.public_key.public_key_bytes())
                } else {
                    old
                }
            );
        }
        assert_eq!(transport.sent().len(), if recover { 5 } else { 4 });
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn group_refresh_recovers_rotations_and_preserves_members_on_failure() {
    check_group_rotation_after_migration(false).await;
}

#[tokio::test]
async fn group_refresh_recovers_rotated_primary_after_namespace_migration() {
    check_group_rotation_after_migration(true).await;
}

/// A valid trusted signature must not let envelope metadata rotate trust.
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

/// Rotations preserve the paired identity even with colliding numeric PN/LID IDs.
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
                    // A peer's numeric ID may equal our ID in the other
                    // namespace; it must still be recoverable as a peer.
                    use wacore::store::commands::DeviceCommand;
                    let own = if user.is_pn() {
                        Jid::lid(user.user.clone())
                    } else {
                        Jid::pn(user.user.clone())
                    };
                    client
                        .persistence_manager
                        .process_command(if user.is_pn() {
                            DeviceCommand::SetLid(Some(own.clone()))
                        } else {
                            DeviceCommand::SetId(Some(own.clone()))
                        })
                        .await;
                    let own_key = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                        .public_key
                        .public_key_bytes();
                    client
                        .signal_cache
                        .put_identity(&own.to_protocol_address(), &own_key)
                        .await;
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
                    assert_eq!(
                        client.cached_adv_account_identity(&own).await.unwrap(),
                        Some(own_key)
                    );
                    client.signal_shutdown_sync();
                }
            }
        }
    }
}

/// A learned PN/LID mapping must preserve the local primary trust anchor.
#[tokio::test]
async fn prepared_primary_survives_known_namespace_migration() {
    use crate::lid_pn_cache::LearningSource;
    use wacore::usync::{UserDeviceList, UsyncDevice};
    for migrate in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let pn = Jid::pn("12025550123");
        let lid = Jid::lid("100000000000123");
        crate::test_utils::seed_adv_primary(&client, &pn).await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: pn.clone(),
                devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))],
                phash: None,
                key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
            }],
            lid_mappings: vec![],
        };
        let anchors = client
            .prepare_device_list_identities(&response)
            .await
            .unwrap();
        let prepared = client.cached_adv_account_identity(&pn).await.unwrap();
        if migrate {
            client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
                .await
                .unwrap();
            assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), None);
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                prepared
            );
        }
        let result = client
            .apply_device_list_response_inner(
                &response,
                DeviceListApplyMode::StrictRefresh,
                None,
                None,
                DeviceListIdentityContext {
                    prepared: &anchors,
                    recovered: None,
                },
                None,
            )
            .await;
        client.signal_shutdown_sync();
        assert!(result.is_ok(), "migrate={migrate}: {result:?}");
    }
}

fn migration_response(user: &Jid) -> DeviceListResponse {
    use wacore::usync::{UserDeviceList, UsyncDevice};
    DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: user.clone(),
            devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))],
            phash: None,
            key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
        }],
        lid_mappings: vec![],
    }
}

#[tokio::test]
async fn prepared_primary_rejects_missing_changed_and_conflicting_identity() {
    use crate::lid_pn_cache::LearningSource;
    for case in ["unknown-alias", "missing", "changed", "conflict"] {
        let (client, _) = create_iq_test_client().await;
        let pn = Jid::pn("12025550123");
        let lid = Jid::lid("100000000000123");
        let response = migration_response(&pn);
        crate::test_utils::seed_adv_primary(&client, &pn).await;
        let prepared = client
            .prepare_device_list_identities(&response)
            .await
            .unwrap();
        if case == "unknown-alias" {
            client.migrate_signal_sessions(&pn, &lid).await;
        } else if case == "missing" {
            client
                .signal_cache
                .delete_identity(&pn.to_protocol_address())
                .await;
        } else {
            client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
                .await
                .unwrap();
            client
                .signal_cache
                .put_identity(&lid.to_protocol_address(), &[42; 32])
                .await;
            if case == "conflict" {
                crate::test_utils::seed_adv_primary(&client, &pn).await;
            }
        }
        assert!(
            client
                .apply_device_list_response_inner(
                    &response,
                    DeviceListApplyMode::StrictRefresh,
                    None,
                    None,
                    DeviceListIdentityContext {
                        prepared: &prepared,
                        recovered: None
                    },
                    None,
                )
                .await
                .is_err(),
            "{case}"
        );
        assert!(
            client.load_device_record_for_jid(&pn).await.is_none(),
            "{case}"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn migrated_primary_survives_cache_reload_and_client_reopen() {
    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::{create_iq_test_client_with_backend, create_test_backend};
    let backend = create_test_backend().await;
    let (client, _) = create_iq_test_client_with_backend(backend.clone()).await;
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    client
        .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
        .await
        .unwrap();
    client.signal_cache.flush(backend.as_ref()).await.unwrap();
    client.signal_cache.clear().await;
    let response = migration_response(&pn);
    assert!(
        client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .is_ok()
    );
    client.signal_shutdown_sync();
    let (reopened, _) = create_iq_test_client_with_backend(backend.clone()).await;
    reopened.warm_up_lid_pn_cache().await.unwrap();
    assert!(
        reopened
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .is_ok()
    );
    assert_eq!(
        reopened.cached_adv_account_identity(&pn).await.unwrap(),
        None
    );
    reopened.signal_shutdown_sync();
}

/// The registry wait is a deterministic pause after primary/mapping locks.
#[tokio::test]
async fn primary_migration_waits_until_adv_publication_finishes() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    let (client, _) = create_iq_test_client().await;
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    client
        .lid_pn_cache
        .add(&LidPnEntry::new(
            lid.user.as_str(),
            pn.user.as_str(),
            LearningSource::Usync,
        ))
        .await;
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    let response = migration_response(&pn);
    let registry = client.device_topology.lock_registry().await;
    let publication = client.process_device_list_response(&response, Freshness::Refresh);
    futures::pin_mut!(publication);
    assert!(futures::poll!(publication.as_mut()).is_pending());
    let migration = client.migrate_signal_sessions(&pn, &lid);
    futures::pin_mut!(migration);
    assert!(futures::poll!(migration.as_mut()).is_pending());
    drop(registry);
    assert!(publication.await.is_ok());
    migration.await;
    assert!(
        client
            .cached_adv_account_identity(&pn)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        client
            .cached_adv_account_identity(&lid)
            .await
            .unwrap()
            .is_some()
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn stale_reverse_alias_cannot_supply_an_adv_primary() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    let (client, _) = create_iq_test_client().await;
    let pn = Jid::pn("12025550123");
    let old_lid = Jid::lid("100000000000123");
    let new_lid = Jid::lid("100000000000124");
    for lid in [&old_lid, &new_lid] {
        client
            .lid_pn_cache
            .add(&LidPnEntry::new(
                lid.user.as_str(),
                pn.user.as_str(),
                LearningSource::Usync,
            ))
            .await;
    }
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    let error = client
        .prepare_device_list_identities(&migration_response(&old_lid))
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("conflicting account alias"),
        "{error:#}"
    );
    client.signal_shutdown_sync();
}
