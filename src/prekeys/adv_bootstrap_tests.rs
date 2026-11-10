use super::*;
use crate::test_utils::{
    create_iq_test_client, decode_sent_iq, node_to_owned_ref, poll_until, wait_for_lock_waiter,
};
use std::time::Duration;
use wacore::iq::prekeys::PreKeyBundleUserNode;
use wacore::libsignal::protocol::{IdentityKey, PreKeyBundle};
use wacore::protocol::ProtocolNode;
use wacore::types::jid::JidExt;
use wacore_binary::builder::NodeBuilder;

#[test]
fn adv_bootstrap_memory_reports_retained_capacity_after_cancelled_flights() {
    let budgets = crate::inbound_queue::QueueBudgets::default();
    let baseline = budgets.memory_stats().0;
    assert!(budgets.adv_bootstrap.get().is_none());
    let registry = budgets.adv_bootstrap.get_or_init(Arc::default);
    let initialized = budgets.memory_stats().0;
    assert!(initialized.bytes > baseline.bytes);
    let mut first_peak = None;
    for _ in 0..16 {
        let leases: Vec<_> = (0..8)
            .map(|index| {
                registry
                    .claim(0, &Jid::pn(format!("1202555{:04}", 100 + index)))
                    .ok()
                    .expect("independent account flight")
            })
            .collect();
        let active = budgets.memory_stats().0;
        assert!(active.bytes > initialized.bytes);
        assert_eq!(active.entries, baseline.entries);
        let first_peak = *first_peak.get_or_insert(active.bytes);
        drop(leases);
        assert!(registry.flights.lock().unwrap().is_empty());
        let retained = budgets.memory_stats().0;
        assert_eq!(retained.entries, baseline.entries);
        assert!(
            retained.bytes > initialized.bytes,
            "empty map retains its buffer"
        );
        assert!(
            retained.bytes < active.bytes,
            "cancelled flights release their allocations"
        );
        assert!(
            retained.bytes <= first_peak,
            "repeated bursts must not accumulate retained bytes"
        );
    }
}

fn request(
    client: &Arc<Client>,
    companion: Jid,
) -> tokio::task::JoinHandle<anyhow::Result<[u8; 32]>> {
    let client = client.clone();
    tokio::spawn(async move { client.ensure_adv_account_identity(&companion).await })
}

#[tokio::test]
async fn hosted_and_ordinary_companions_share_one_canonical_cold_bootstrap() {
    for primary in [Jid::pn("12025550185"), Jid::lid("100000000000185")] {
        let (client, transport) = create_iq_test_client().await;
        let hosted = primary.with_device_hosting(99, true);
        let leader = request(&client, hosted.clone());
        let iq = decode_sent_iq(&transport, 0).await;
        let node = iq.get();
        let requested = node
            .get_optional_child("key")
            .unwrap()
            .get_optional_child("user")
            .unwrap()
            .attrs()
            .optional_jid("jid")
            .unwrap();
        assert_eq!(requested, primary);
        let id = node.attrs().optional_string("id").unwrap().into_owned();
        let joined = request(&client, primary.with_device(7));
        wait_for_joiner(&client).await;
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        answer(&client, &id, &primary, &account, true).await;
        assert_eq!(
            leader.await.unwrap().unwrap(),
            *account.public_key.public_key_bytes()
        );
        assert_eq!(
            joined.await.unwrap().unwrap(),
            *account.public_key.public_key_bytes()
        );
        assert_eq!(transport.sent_count(), 1);
        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend
                .load_identity(primary.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(*account.public_key.public_key_bytes())
        );
        assert!(
            backend
                .load_identity(hosted.with_device(0).to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn cold_hosted_prekey_fetch_authenticates_and_persists_only_the_ordinary_anchor() {
    for primary in [Jid::pn("12025550189"), Jid::lid("100000000000189")] {
        let (client, transport) = create_iq_test_client().await;
        let hosted = primary.with_device_hosting(99, true);
        let ordinary = primary.with_device(7);
        let requested = vec![hosted.clone(), ordinary.clone()];
        let fetch = {
            let client = client.clone();
            let requested = requested.clone();
            tokio::spawn(async move { client.fetch_pre_keys(&requested, None).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let node = iq.get();
        let users: Vec<_> = node
            .get_optional_child("key")
            .unwrap()
            .children()
            .unwrap()
            .iter()
            .map(|user| user.attrs().optional_jid("jid").unwrap())
            .collect();
        assert_eq!(
            users,
            vec![hosted.clone(), ordinary.clone(), primary.clone()]
        );
        let id = node.attrs().optional_string("id").unwrap().into_owned();
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let make_bundle = |key: &KeyPair| {
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            let signed = KeyPair::generate(&mut rng);
            PreKeyBundle::new(
                1,
                0u32.into(),
                None,
                1u32.into(),
                signed.public_key,
                key.private_key
                    .calculate_signature(&signed.public_key.serialize(), &mut rng)
                    .unwrap()
                    .to_vec(),
                IdentityKey::new(key.public_key),
            )
            .unwrap()
        };
        let mut replies = Vec::new();
        for jid in &requested {
            let device = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            let certificate = waproto::codec::adv_signed_device_identity_to_vec(
                &wacore::adv::test_util::signed_identity(
                    &account,
                    &device,
                    b"\x18\x07",
                    &[6, 0],
                    Some(if jid == &hosted { &[6, 6] } else { &[6, 1] }),
                    false,
                ),
            );
            replies.push(
                PreKeyBundleUserNode::from_bundle(
                    jid.clone(),
                    &make_bundle(&device),
                    Some(certificate),
                )
                .unwrap()
                .into_node(),
            );
        }
        replies.push(
            PreKeyBundleUserNode::from_bundle(primary.clone(), &make_bundle(&account), None)
                .unwrap()
                .into_node(),
        );
        client
            .process_node(node_to_owned_ref(
                &NodeBuilder::new("iq")
                    .attr("id", id)
                    .attr("type", "result")
                    .children([NodeBuilder::new("list").children(replies).build()])
                    .build(),
            ))
            .await;
        let result = fetch.await.unwrap().unwrap();
        assert_eq!(
            result.bundles.len(),
            2,
            "bootstrap primary is not an unsolicited result"
        );
        for jid in requested {
            assert_eq!(
                result
                    .bundles
                    .get(&jid)
                    .unwrap()
                    .account_identity()
                    .unwrap()
                    .public_key()
                    .public_key_bytes(),
                account.public_key.public_key_bytes()
            );
        }
        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend
                .load_identity(primary.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(*account.public_key.public_key_bytes())
        );
        assert!(
            backend
                .load_identity(hosted.with_device(0).to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(transport.sent_count(), 1);
        client.signal_shutdown_sync();
    }
}

async fn wait_for_joiner(client: &Client) {
    poll_until("another caller to join the ADV flight", || {
        client
            .adv_bootstrap()
            .flights
            .lock()
            .unwrap()
            .values()
            .any(|flight| Arc::strong_count(flight) > 2)
    })
    .await;
}

#[test]
fn shared_adv_bootstrap_preserves_typed_conflicts_and_other_error_sources() {
    use crate::usync::DeviceListIdentityConflict as Conflict;
    for conflict in [Conflict::Alias, Conflict::Primary, Conflict::Bootstrap] {
        let message = conflict.to_string();
        let shared = Arc::new(anyhow::Error::new(conflict).context("bootstrap transaction"));
        for _ in 0..2 {
            let error = shared_adv_bootstrap_error(shared.clone());
            assert!(error.is::<Conflict>());
            assert_eq!(
                error.downcast_ref::<Conflict>().unwrap().to_string(),
                message
            );
        }
    }
    let shared = Arc::new(anyhow::Error::new(std::io::Error::other(
        "storage unavailable",
    )));
    let error = shared_adv_bootstrap_error(shared);
    assert!(!error.is::<Conflict>());
    assert!(error.chain().any(|source| source.is::<std::io::Error>()));
}

#[tokio::test]
async fn adv_bootstrap_alias_conflict_remains_typed_for_leader_and_joiner() {
    let (client, transport) = create_iq_test_client().await;
    let primary = Jid::pn("12025550191");
    let lid = Jid::lid("100000000000191");
    let leader = request(&client, primary.with_device(7));
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    let joiner = request(&client, primary.with_device(9));
    wait_for_joiner(&client).await;
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            primary.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    client.lid_pn_cache.evict_reverse_for_tests(&lid.user).await;
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    answer(&client, &id, &primary, &account, true).await;
    for caller in [leader, joiner] {
        let error = caller.await.unwrap().unwrap_err();
        assert!(matches!(
            error.downcast_ref::<crate::usync::DeviceListIdentityConflict>(),
            Some(crate::usync::DeviceListIdentityConflict::Alias)
        ));
    }
    assert_eq!(transport.sent_count(), 1);
    assert!(
        client
            .cached_adv_account_identity(&primary)
            .await
            .unwrap()
            .is_none()
    );
    client.signal_shutdown_sync();
}

async fn answer(client: &Arc<Client>, id: &str, primary: &Jid, account: &KeyPair, valid: bool) {
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
    let bundle = PreKeyBundle::new(
        1,
        0u32.into(),
        None,
        1u32.into(),
        signed.public_key,
        signature,
        IdentityKey::new(account.public_key),
    )
    .unwrap();
    let user = PreKeyBundleUserNode::from_bundle(primary.clone(), &bundle, None)
        .unwrap()
        .into_node();
    // Exercise the actual IQ correlator, not direct waiter completion.
    client
        .process_node(node_to_owned_ref(
            &NodeBuilder::new("iq")
                .attr("id", id)
                .attr("type", "result")
                .children([NodeBuilder::new("list").children([user]).build()])
                .build(),
        ))
        .await;
}

#[tokio::test]
async fn adv_bootstrap_shares_success_and_invalid_signature_without_refetching() {
    for valid in [true, false] {
        let (client, transport) = create_iq_test_client().await;
        let primary = Jid::pn("12025550170");
        let leader = request(&client, primary.with_device(7));
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        let joined = request(&client, primary.with_device(9));
        wait_for_joiner(&client).await;
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        answer(&client, &id, &primary, &account, valid).await;
        let first = leader.await.unwrap();
        let second = joined.await.unwrap();
        assert_eq!(first.is_ok(), valid);
        assert_eq!(second.is_ok(), valid);
        if valid {
            assert_eq!(first.unwrap(), second.unwrap());
        }
        assert_eq!(transport.sent_count(), 1);
        let persisted = client
            .persistence_manager
            .backend()
            .load_identity(primary.to_protocol_address().as_str())
            .await
            .unwrap();
        assert_eq!(persisted.is_some(), valid);
        assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
        assert!(client.response_waiters_guard().is_empty());
        client.signal_shutdown_sync();
    }
}

#[tokio::test(start_paused = true)]
async fn adv_bootstrap_shares_timeout_and_releases_all_admission() {
    let (client, transport) = create_iq_test_client().await;
    let primary = Jid::pn("12025550171");
    let leader = request(&client, primary.with_device(7));
    let _iq = decode_sent_iq(&transport, 0).await;
    let joined = request(&client, primary.with_device(9));
    wait_for_joiner(&client).await;
    tokio::time::advance(Duration::from_secs(76)).await;
    assert!(leader.await.unwrap().is_err());
    assert!(joined.await.unwrap().is_err());
    assert_eq!(transport.sent_count(), 1);
    assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
    assert!(client.response_waiters_guard().is_empty());
    assert!(
        client
            .cached_adv_account_identity(&primary)
            .await
            .unwrap()
            .is_none()
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_bootstrap_caps_queries_and_cancellation_releases_slots() {
    let (client, transport) = create_iq_test_client().await;
    let mut tasks = Vec::new();
    for index in 0..10 {
        tasks.push(request(
            &client,
            Jid::pn_device(format!("1202555{:04}", 110 + index), 7),
        ));
    }
    poll_until("all cold accounts to enter admission", || {
        client.adv_bootstrap().flights.lock().unwrap().len() == 10
    })
    .await;
    poll_until("eight ADV queries to reach the transport", || {
        transport.sent_count() == 8
    })
    .await;
    assert_eq!(client.response_waiters_guard().len(), 8);
    for task in &tasks {
        task.abort();
    }
    for task in tasks {
        assert!(task.await.unwrap_err().is_cancelled());
    }
    assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
    assert!(client.response_waiters_guard().is_empty());
    let permits: Vec<_> = (0..8)
        .map(|_| client.adv_bootstrap().queries.try_acquire().unwrap())
        .collect();
    assert!(client.adv_bootstrap().queries.try_acquire().is_none());
    drop(permits);
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_bootstrap_cancelled_leader_allows_valid_joiner_to_take_over() {
    let (client, transport) = create_iq_test_client().await;
    let primary = Jid::pn("12025550172");
    let leader = request(&client, primary.with_device(7));
    let _first = decode_sent_iq(&transport, 0).await;
    let joined = request(&client, primary.with_device(9));
    wait_for_joiner(&client).await;
    leader.abort();
    assert!(leader.await.unwrap_err().is_cancelled());

    let iq = decode_sent_iq(&transport, 1).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    answer(&client, &id, &primary, &account, true).await;
    assert_eq!(
        joined.await.unwrap().unwrap(),
        *account.public_key.public_key_bytes()
    );
    assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_bootstrap_rechecks_current_trust_and_connection_before_publication() {
    for (retire_connection, fire_shutdown) in [(false, false), (true, false), (false, true)] {
        let (client, transport) = create_iq_test_client().await;
        let primary = Jid::pn("12025550173");
        let address = primary.to_protocol_address();
        let lock = client.session_lock_for(address.as_str()).await;
        let bootstrap = request(&client, primary.with_device(7));
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        let guard = lock.lock().await;
        let baseline = Arc::strong_count(&lock);
        let stale = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        answer(&client, &id, &primary, &stale, true).await;
        wait_for_lock_waiter(&lock, baseline).await;
        let current = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        if fire_shutdown {
            client.notify_connection_shutdown();
        } else if retire_connection {
            client.connection_generation.fetch_add(1, Ordering::AcqRel);
        } else {
            client
                .signal_cache
                .persist_identity_if_absent_or_equal(
                    &address,
                    current.public_key.public_key_bytes(),
                    &*client.persistence_manager.backend(),
                )
                .await
                .unwrap();
        }
        drop(guard);
        let result = bootstrap.await.unwrap();
        let persisted = client
            .persistence_manager
            .backend()
            .load_identity(address.as_str())
            .await
            .unwrap();
        if retire_connection || fire_shutdown {
            assert!(result.is_err());
            assert!(persisted.is_none());
        } else {
            assert_eq!(result.unwrap(), *current.public_key.public_key_bytes());
            assert_eq!(persisted, Some(*current.public_key.public_key_bytes()));
        }
        assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_bootstrap_owned_writer_keeps_primary_fence_after_caller_cancellation() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;

    for gate_method in ["put_identity", "after:put_identity"] {
        let (mut probe, _) = ProbeBackend::new();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        probe.gate_method = gate_method;
        let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let primary = Jid::pn("12025550174");
        let address = primary.to_protocol_address();
        let lock = client.session_lock_for(address.as_str()).await;
        let bootstrap = request(&client, primary.with_device(7));
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        answer(&client, &id, &primary, &account, true).await;
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        bootstrap.abort();
        assert!(bootstrap.await.unwrap_err().is_cancelled());
        assert!(client.adv_bootstrap().flights.lock().unwrap().is_empty());
        assert!(
            lock.try_lock().is_none(),
            "the actual writer still owns the account fence"
        );
        assert_eq!(client.inbound_queues().work.pending(), 1);
        release.send(()).await.unwrap();
        poll_until("the owned ADV writer to finish", || {
            client.inbound_queues().work.pending() == 0
        })
        .await;
        assert!(lock.try_lock().is_some());
        assert_eq!(
            client
                .persistence_manager
                .backend()
                .load_identity(address.as_str())
                .await
                .unwrap(),
            Some(*account.public_key.public_key_bytes())
        );
        client.signal_shutdown_sync();
    }
}

/// Dropping all callers cannot release storage while the admitted write owns it.
#[tokio::test]
async fn adv_bootstrap_writer_delays_store_release_after_client_drop() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let (mut probe, dropped) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "put_identity";
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let released = client.store_release();
    let primary = Jid::pn("12025550179");
    let bootstrap = request(&client, primary.with_device(7));
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    answer(&client, &id, &primary, &account, true).await;
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    bootstrap.abort();
    assert!(bootstrap.await.unwrap_err().is_cancelled());
    client.signal_shutdown_sync();
    drop(client);
    let mut wait = Box::pin(released.wait());
    assert!(futures::poll!(&mut wait).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    release.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
}
