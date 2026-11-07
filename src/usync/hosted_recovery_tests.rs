//! Hosted rotation recovery across storage errors and caller cancellation.

use super::tests::{has_session, seed_fresh_session, signed_key_index_bytes};
use super::*;
use crate::store::release::tests::ProbeBackend;
use crate::store::traits::Backend;
use crate::test_utils::{TestEventCollector, create_iq_test_client_with_backend};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use wacore::libsignal::protocol::{KeyPair, ProtocolAddress};
use wacore::store::traits::{DeviceInfo, DeviceListRecord, TcTokenEntry};
use wacore::types::events::Event;
use wacore::types::jid::JidExt as _;
use wacore::usync::{UserDeviceList, UsyncDevice};

const GROUP: &str = "120363000000000170@g.us";

/// Recovery includes a cached orphan even when a concurrent flush evicts it.
#[tokio::test]
async fn hosted_orphan_recovery_captures_state_flushed_during_enumeration() {
    let (mut probe, _) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "after:get_signal_addresses_for_user";
    let fixture = Rotation::new(probe, Jid::pn("12025550188")).await;
    let orphan = seed_fresh_session(&fixture.client, &fixture.user.with_device(7)).await;
    fixture
        .client
        .invalidate_device_cache(&fixture.user.user)
        .await;
    assert!(
        fixture
            .raw
            .get_session(orphan.as_str())
            .await
            .unwrap()
            .is_none()
    );
    let operation = {
        let client = fixture.client.clone();
        let response = fixture.response.clone();
        tokio::spawn(async move {
            client
                .process_device_list_response(&response, crate::cache::Freshness::Refresh)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    fixture
        .client
        .flush_signal_cache_batch_safe()
        .await
        .unwrap();
    fixture.client.signal_cache.clear_after_flush().await;
    assert!(
        fixture
            .raw
            .get_session(orphan.as_str())
            .await
            .unwrap()
            .is_some()
    );
    release.send(()).await.unwrap();
    operation.await.unwrap().unwrap();
    fixture.assert_completed().await;
    assert!(!has_session(&fixture.client, &orphan).await);
    assert!(
        fixture
            .raw
            .get_session(orphan.as_str())
            .await
            .unwrap()
            .is_none()
    );
    fixture.client.signal_shutdown_sync();
}

/// Failed orphan recovery cannot discard valid members or replace partial trust.
#[tokio::test]
async fn group_refresh_preserves_members_when_orphan_recovery_fails() {
    for corrupt_row in [false, true] {
        let (mut probe, _) = ProbeBackend::new();
        let fail = Arc::new(AtomicBool::new(false));
        probe.fail_before_method = Some(("get_signal_addresses_for_user", fail.clone()));
        let fixture = Rotation::new(probe, Jid::pn("12025550182")).await;
        fixture
            .client
            .invalidate_device_cache(&fixture.user.user)
            .await;
        if corrupt_row {
            fixture
                .raw
                .put_identity("12025550182:65536@c.us.0", [4; 32])
                .await
                .unwrap();
        } else {
            fail.store(true, Ordering::Relaxed);
        }
        let member = Jid::pn("12025550183");
        crate::test_utils::seed_adv_primary(&fixture.client, &member).await;
        let mut response = fixture.response.clone();
        response.device_lists.push(UserDeviceList {
            user: member.clone(),
            devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(2, Some(1))],
            phash: None,
            key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
        });
        let result = fixture
            .client
            .try_process_group_device_list_response(
                &response,
                &[fixture.user.clone(), member.clone()],
                fixture.client.device_topology.current(),
            )
            .await
            .unwrap()
            .unwrap();
        assert!(result.contains(&member.with_device(2)));
        assert!(!result.iter().any(|device| device.user == fixture.user.user));
        assert!(
            fixture
                .raw
                .get_devices(&fixture.user.user)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            fixture
                .client
                .cached_adv_account_identity(&fixture.user)
                .await
                .unwrap(),
            Some(fixture.old_key)
        );
        assert!(has_session(&fixture.client, &fixture.session).await);
        assert!(fixture.collector.events().is_empty());
        assert!(
            fixture.apply().await.is_err(),
            "strict recovery must fail closed"
        );
        assert_eq!(
            fixture
                .client
                .cached_adv_account_identity(&fixture.user)
                .await
                .unwrap(),
            Some(fixture.old_key)
        );
        fixture.client.signal_shutdown_sync();
    }
}

/// Orphan discovery includes every state source and respects exact user bounds.
#[tokio::test]
async fn orphan_discovery_covers_all_sources_without_crossing_users() {
    for sqlite in [false, true] {
        let (probe, _) = ProbeBackend::new();
        let backend = if sqlite {
            crate::test_utils::create_test_backend().await
        } else {
            probe.backend
        };
        for user in ["12025550180", "100000000000180", "user_%\\", "USER_%\\"] {
            let session_only = format!("{user}:6@hosted.0");
            let session = format!("{user}:7@c.us.0");
            let identity = format!("{user}:8@hosted.lid.0");
            let tracked = format!("{user}:9@s.whatsapp.net");
            backend.put_session(&session_only, &[1]).await.unwrap();
            backend.put_session(&session, &[1]).await.unwrap();
            backend.put_identity(&session, [2; 32]).await.unwrap();
            backend.put_identity(&identity, [3; 32]).await.unwrap();
            backend
                .set_sender_key_status(GROUP, &[(tracked.as_str(), true)])
                .await
                .unwrap();
            backend
                .put_identity(&format!("{user}0:8@lid.0"), [4; 32])
                .await
                .unwrap();
            let mut actual = backend.get_signal_addresses_for_user(user).await.unwrap();
            actual.sort_unstable();
            let mut expected: Vec<Arc<str>> = [session_only, session, identity, tracked]
                .into_iter()
                .map(Arc::from)
                .collect();
            expected.sort_unstable();
            assert_eq!(actual, expected);
        }
    }
}

/// A connected client with an independently signed hosted replacement.
struct Rotation {
    client: Arc<Client>,
    raw: Arc<dyn Backend>,
    transport: Arc<crate::transport::mock::CapturingMockTransport>,
    collector: Arc<TestEventCollector>,
    user: Jid,
    old_key: [u8; 32],
    primary_session: ProtocolAddress,
    session: ProtocolAddress,
    response: DeviceListResponse,
}

impl Rotation {
    /// Seed durable old trust, sessions and warm marks with a verified new list.
    async fn new(probe: ProbeBackend, user: Jid) -> Self {
        let raw = probe.backend.clone();
        let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let old = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let old_key = *old.public_key.public_key_bytes();
        client
            .signal_cache
            .put_identity(&user.to_protocol_address(), &old_key)
            .await;
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(99, Some(1))].into(),
                timestamp: 1,
                phash: None,
                raw_id: Some(1),
            })
            .await
            .unwrap();
        let session = seed_fresh_session(&client, &user.with_device(99)).await;
        let primary_session = seed_fresh_session(&client, &user).await;
        client.flush_signal_cache_batch_safe().await.unwrap();
        client
            .persistence_manager
            .set_sender_key_status(
                GROUP,
                &[
                    (user.to_string().as_str(), true),
                    (user.with_device(99).to_string().as_str(), true),
                ],
            )
            .await
            .unwrap();
        assert_eq!(
            client
                .skdm_device_map(GROUP)
                .await
                .device_has_key(&user.user, 99),
            Some(true)
        );
        let now = client.runtime.wall_time_secs();
        raw.put_tc_token(
            user.user.as_str(),
            &TcTokenEntry {
                token: vec![1],
                token_timestamp: now,
                sender_timestamp: Some(now),
            },
        )
        .await
        .unwrap();
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(Self::new_key().to_vec());
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![
                    UsyncDevice::new(0, None),
                    UsyncDevice::new(99, Some(1)).with_hosting(true),
                ],
                phash: None,
                key_index_bytes: Some(waproto::codec::adv_signed_key_index_list_to_vec(&signed)),
            }],
            lid_mappings: vec![],
        };
        Self {
            client,
            raw,
            transport,
            collector,
            user,
            old_key,
            primary_session,
            session,
            response,
        }
    }

    /// The fixture's independently generated hosted account signing key.
    fn new_key() -> [u8; 32] {
        *crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes()
    }

    /// Exercise production response validation and guarded publication.
    async fn apply(&self) -> anyhow::Result<Vec<Jid>> {
        self.client
            .process_device_list_response(&self.response, crate::cache::Freshness::Refresh)
            .await
    }

    /// Verify durable cleanup, primary preservation, notification and token IQ.
    async fn assert_completed(&self) {
        assert_eq!(
            self.raw
                .load_identity(self.user.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(Self::new_key())
        );
        assert!(!has_session(&self.client, &self.session).await);
        assert!(has_session(&self.client, &self.primary_session).await);
        assert!(
            self.raw
                .get_session(self.session.as_str())
                .await
                .unwrap()
                .is_none()
        );
        let rows = self.raw.get_sender_key_devices(GROUP).await.unwrap();
        assert!(!rows.iter().any(|(jid, _)| jid == &self.user.to_string()
            || jid == &self.user.with_device(99).to_string()));
        let map = self.client.skdm_device_map(GROUP).await;
        assert_eq!(map.device_has_key(&self.user.user, 0), None);
        assert_eq!(map.device_has_key(&self.user.user, 99), None);
        let events = self.collector.events();
        let changes: Vec<_> = events
            .iter()
            .filter_map(|event| match &**event {
                Event::IdentityChange(change) => Some(change),
                _ => None,
            })
            .collect();
        assert_eq!(
            changes.len(),
            1,
            "the durable rotation must report its original key change exactly once"
        );
        assert_eq!(changes[0].previous_identity_key, Some(self.old_key));
        assert_eq!(changes[0].current_identity_key, Some(Self::new_key()));
        let token = crate::test_utils::decode_sent_iq(&self.transport, 0).await;
        assert_eq!(
            token.get().attrs().optional_string("xmlns").as_deref(),
            Some("privacy")
        );
        let id = token
            .get()
            .attrs()
            .optional_string("id")
            .unwrap()
            .into_owned();
        let response = wacore_binary::builder::NodeBuilder::new("iq")
            .attr("type", "result")
            .children([wacore_binary::builder::NodeBuilder::new("tokens").build()])
            .build();
        crate::test_utils::answer_iq(&self.client, &id, &response).await;
        assert_eq!(self.transport.sent_count(), 1);
    }
}

/// Recovered targets and future additions cannot inherit orphaned old sessions.
#[tokio::test]
async fn hosted_rotation_without_registry_retires_each_device_before_reuse() {
    for sqlite in [false, true] {
        let (mut probe, _) = ProbeBackend::new();
        if sqlite {
            probe.backend = crate::test_utils::create_test_backend().await;
        }
        let fixture = Rotation::new(probe, Jid::pn("12025550173")).await;
        let latent = seed_fresh_session(&fixture.client, &fixture.user.with_device(7)).await;
        fixture
            .client
            .flush_signal_cache_batch_safe()
            .await
            .unwrap();
        fixture
            .raw
            .set_sender_key_status(
                GROUP,
                &[(fixture.user.with_device(7).to_string().as_str(), true)],
            )
            .await
            .unwrap();
        fixture
            .client
            .invalidate_device_cache(&fixture.user.user)
            .await;
        assert!(
            fixture
                .raw
                .get_devices(&fixture.user.user)
                .await
                .unwrap()
                .is_none()
        );
        fixture.apply().await.unwrap();
        fixture.assert_completed().await;
        assert!(!has_session(&fixture.client, &latent).await);
        let mut later = fixture.response.clone();
        later.device_lists[0].devices =
            vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))];
        fixture
            .client
            .process_device_list_response(&later, crate::cache::Freshness::Refresh)
            .await
            .unwrap();
        assert!(!has_session(&fixture.client, &latent).await);
        assert_eq!(
            fixture
                .client
                .skdm_device_map(GROUP)
                .await
                .device_has_key(&fixture.user.user, 7),
            None
        );
        assert!(
            fixture
                .raw
                .get_sender_key_devices(GROUP)
                .await
                .unwrap()
                .is_empty()
        );
        fixture.client.signal_shutdown_sync();
    }
}

/// A hosted change updates both current primary aliases before later PN/LID sync.
#[tokio::test]
async fn hosted_rotation_reconciles_both_primary_aliases() {
    for hosted_lid in [true, false] {
        let pn = Jid::pn("12025550174");
        let lid = Jid::lid("100000000000174");
        let user = if hosted_lid { lid.clone() } else { pn.clone() };
        let alias = if hosted_lid { pn.clone() } else { lid.clone() };
        let (probe, _) = ProbeBackend::new();
        let fixture = Rotation::new(probe, user).await;
        let entry = crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            pn.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        );
        fixture.client.lid_pn_cache.add(&entry).await;
        fixture
            .client
            .signal_cache
            .put_identity(&alias.to_protocol_address(), &fixture.old_key)
            .await;
        fixture
            .client
            .flush_signal_cache_batch_safe()
            .await
            .unwrap();
        let token = fixture
            .raw
            .get_tc_token(&fixture.user.user)
            .await
            .unwrap()
            .unwrap();
        fixture.raw.put_tc_token(&lid.user, &token).await.unwrap();
        fixture.apply().await.unwrap();
        fixture.assert_completed().await;
        assert_eq!(
            fixture
                .raw
                .load_identity(alias.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(Rotation::new_key())
        );
        let mut later = fixture.response.clone();
        later.device_lists[0].user = alias.clone();
        later.device_lists[0].devices = vec![UsyncDevice::new(0, None)];
        fixture
            .client
            .process_device_list_response(&later, crate::cache::Freshness::Refresh)
            .await
            .unwrap();
        assert_eq!(
            fixture
                .client
                .cached_adv_account_identity(&alias)
                .await
                .unwrap(),
            Some(Rotation::new_key())
        );
        fixture.client.signal_shutdown_sync();
    }
}

/// A hosting flag change alone cannot retire the primary's session or identity.
#[tokio::test]
async fn primary_hosting_change_preserves_signal_state_and_clears_only_its_warm_mark() {
    let (probe, _) = ProbeBackend::new();
    let fixture = Rotation::new(probe, Jid::pn("12025550175")).await;
    fixture
        .client
        .signal_cache
        .put_identity(&fixture.user.to_protocol_address(), &Rotation::new_key())
        .await;
    fixture
        .client
        .flush_signal_cache_batch_safe()
        .await
        .unwrap();
    let mut response = fixture.response.clone();
    response.device_lists[0].devices = vec![
        UsyncDevice::new(0, None).with_hosting(true),
        UsyncDevice::new(99, Some(1)),
    ];
    fixture
        .client
        .process_device_list_response(&response, crate::cache::Freshness::Refresh)
        .await
        .unwrap();
    assert!(has_session(&fixture.client, &fixture.primary_session).await);
    assert!(has_session(&fixture.client, &fixture.session).await);
    assert_eq!(
        fixture
            .client
            .cached_adv_account_identity(&fixture.user)
            .await
            .unwrap(),
        Some(Rotation::new_key())
    );
    let map = fixture.client.skdm_device_map(GROUP).await;
    assert_eq!(map.device_has_key(&fixture.user.user, 0), None);
    assert_eq!(map.device_has_key(&fixture.user.user, 99), Some(true));
    assert!(fixture.collector.events().is_empty());
    fixture.client.signal_shutdown_sync();
}

/// A partially committed alias write retains both old cache anchors for retry.
#[tokio::test]
async fn hosted_alias_write_failure_retries_the_complete_rotation() {
    let (mut probe, _) = ProbeBackend::new();
    let fail = Arc::new(AtomicBool::new(false));
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "after:put_identity";
    probe.fail_before_method = Some(("put_identity", fail.clone()));
    let fixture = Rotation::new(probe, Jid::lid("100000000000178")).await;
    let alias = Jid::pn("12025550178");
    fixture
        .client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            fixture.user.user.to_string(),
            alias.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    fixture
        .client
        .signal_cache
        .put_identity(&alias.to_protocol_address(), &fixture.old_key)
        .await;
    fixture
        .client
        .flush_signal_cache_batch_safe()
        .await
        .unwrap();
    let operation = {
        let client = fixture.client.clone();
        let response = fixture.response.clone();
        tokio::spawn(async move {
            client
                .process_device_list_response(&response, crate::cache::Freshness::Refresh)
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        fixture
            .raw
            .load_identity(fixture.user.to_protocol_address().as_str())
            .await
            .unwrap(),
        Some(Rotation::new_key())
    );
    fail.store(true, Ordering::Relaxed);
    release.send(()).await.unwrap();
    assert!(operation.await.unwrap().is_err());
    for user in [&fixture.user, &alias] {
        assert_eq!(
            fixture
                .client
                .cached_adv_account_identity(user)
                .await
                .unwrap(),
            Some(fixture.old_key)
        );
    }
    assert!(fixture.collector.events().is_empty());
    fail.store(false, Ordering::Relaxed);
    for _ in 0..16 {
        release.send(()).await.unwrap();
    }
    fixture.apply().await.unwrap();
    fixture.assert_completed().await;
    assert_eq!(
        fixture
            .raw
            .load_identity(alias.to_protocol_address().as_str())
            .await
            .unwrap(),
        Some(Rotation::new_key())
    );
    fixture.client.signal_shutdown_sync();
}

/// A conflicting self result does not discard valid group member updates.
#[tokio::test]
async fn group_refresh_keeps_conflicting_self_and_applies_valid_members() {
    use wacore::store::commands::DeviceCommand;
    let (probe, _) = ProbeBackend::new();
    let fixture = Rotation::new(probe, Jid::pn("12025550176")).await;
    fixture
        .client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(fixture.user.with_device(1))))
        .await;
    let member = Jid::pn("12025550177");
    crate::test_utils::seed_adv_primary(&fixture.client, &member).await;
    let mut response = fixture.response.clone();
    response.device_lists.push(UserDeviceList {
        user: member.clone(),
        devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(2, Some(1))],
        phash: None,
        key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
    });
    let result = fixture
        .client
        .try_process_group_device_list_response(
            &response,
            &[fixture.user.clone(), member.clone()],
            fixture.client.device_topology.current(),
        )
        .await
        .unwrap()
        .unwrap();
    assert!(result.contains(&member.with_device(2)));
    assert!(result.contains(&fixture.user.with_device(99)));
    assert!(has_session(&fixture.client, &fixture.session).await);
    assert_eq!(
        fixture
            .client
            .cached_adv_account_identity(&fixture.user)
            .await
            .unwrap(),
        Some(fixture.old_key)
    );
    assert!(
        fixture
            .client
            .process_device_list_response(&response, crate::cache::Freshness::Refresh)
            .await
            .is_err()
    );
    assert!(fixture.collector.events().is_empty());
    fixture.client.signal_shutdown_sync();
}

/// Failed cleanup must leave its durable device history available to the retry.
#[tokio::test]
async fn hosted_cleanup_failure_retains_history_for_the_retry() {
    for user in [Jid::pn("12025550170"), Jid::lid("100000000000170")] {
        let (mut probe, _) = ProbeBackend::new();
        let fail = Arc::new(AtomicBool::new(false));
        probe.fail_before_method = Some(("delete_sender_key_device_rows", fail.clone()));
        let fixture = Rotation::new(probe, user).await;
        fail.store(true, Ordering::Relaxed);
        assert!(fixture.apply().await.is_err());
        let previous = fixture.raw.get_devices(&fixture.user.user).await.unwrap();
        assert!(
            previous.is_some(),
            "failed cleanup must retain its durable retry record"
        );
        assert!(has_session(&fixture.client, &fixture.session).await);
        fail.store(false, Ordering::Relaxed);
        fixture.apply().await.unwrap();
        fixture.assert_completed().await;
        fixture.client.signal_shutdown_sync();
    }
}

/// A failed identity write cannot be mistaken for an already completed rotation.
#[tokio::test]
async fn hosted_identity_write_failure_retries_persistence_and_followups() {
    let (mut probe, _) = ProbeBackend::new();
    let fail = Arc::new(AtomicBool::new(false));
    probe.fail_before_method = Some(("put_identity", fail.clone()));
    let fixture = Rotation::new(probe, Jid::pn("12025550171")).await;
    fail.store(true, Ordering::Relaxed);
    assert!(fixture.apply().await.is_err());
    assert_eq!(
        fixture
            .raw
            .load_identity(fixture.user.to_protocol_address().as_str())
            .await
            .unwrap(),
        Some(fixture.old_key)
    );
    fail.store(false, Ordering::Relaxed);
    fixture.apply().await.unwrap();
    fixture.assert_completed().await;
    fixture.client.signal_shutdown_sync();
}

/// Dropping the requester after a storage commit must not lose its follow-ups.
#[tokio::test]
async fn hosted_rotation_finishes_after_caller_cancellation() {
    for gate_method in ["delete_sender_key_device_rows", "after:put_identity"] {
        let (mut probe, _) = ProbeBackend::new();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        probe.gate_method = gate_method;
        let fixture = Rotation::new(probe, Jid::pn("12025550172")).await;
        let caller = {
            let client = fixture.client.clone();
            let response = fixture.response.clone();
            tokio::spawn(async move {
                client
                    .process_device_list_response(&response, crate::cache::Freshness::Refresh)
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        caller.abort();
        assert!(caller.await.unwrap_err().is_cancelled());
        // Subsequent calls to the same probed method should not park again.
        for _ in 0..16 {
            let _ = release.send(()).await;
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if fixture
                    .collector
                    .events()
                    .iter()
                    .any(|event| matches!(&**event, Event::IdentityChange(_)))
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the rotation must finish even after its caller is cancelled");
        // Acquiring the publication guard also waits for the device-list write.
        let guard = fixture.client.device_topology.lock_registry().await;
        drop(guard);
        fixture.assert_completed().await;
        fixture.client.signal_shutdown_sync();
    }
}
