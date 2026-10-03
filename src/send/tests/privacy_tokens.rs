//! Offline sends use real Signal sessions and Noise-framed transport output;
//! they do not authenticate to WhatsApp or establish server acceptance.
use super::*;
use std::time::Duration;
use wacore::iq::{abprops::web, tctoken::compute_cs_token};
use wacore::store::traits::TcTokenEntry;

const RECEIVED_TOKEN: &[u8] = &[0x81, 0x02, 0x00, 0xfe];
const SALT: &[u8] = &[0xff, 0x00, 0x31, 0x80];

async fn configure_props(client: &Arc<Client>, legacy: Option<bool>, nct: bool) {
    let mut props = vec![(
        web::WA_NCT_TOKEN_SEND_ENABLED.code,
        if nct { "1" } else { "0" }.into(),
    )];
    if let Some(enabled) = legacy {
        props.push((
            web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES.code,
            if enabled { "1" } else { "0" }.into(),
        ));
    }
    client.ab_props.apply_props(false, props.into_iter()).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetNctSalt(Some(SALT.to_vec())))
        .await;
}

async fn configure_tokens(client: &Arc<Client>, jid: &Jid, legacy: Option<bool>, nct: bool) {
    configure_props(client, legacy, nct).await;
    let now = wacore::time::now_secs();
    client
        .persistence_manager
        .backend()
        .put_tc_token(
            &jid.user,
            &TcTokenEntry {
                token: RECEIVED_TOKEN.to_vec(),
                token_timestamp: now,
                sender_timestamp: Some(now),
            },
        )
        .await
        .unwrap();
}

fn assert_tokens(stanza: &wacore_binary::NodeRef<'_>, tc: Option<&[u8]>, cs: Option<&[u8]>) {
    assert_eq!(
        stanza
            .get_optional_child("tctoken")
            .and_then(|n| n.content_bytes()),
        tc
    );
    assert_eq!(
        stanza
            .get_optional_child("cstoken")
            .and_then(|n| n.content_bytes()),
        cs
    );
    assert_eq!(
        stanza.get_children_by_tag("tctoken").count(),
        usize::from(tc.is_some())
    );
    assert_eq!(
        stanza.get_children_by_tag("cstoken").count(),
        usize::from(cs.is_some())
    );
}

fn assert_encrypted_dm(stanza: &wacore_binary::NodeRef<'_>, destination: &Jid) {
    assert_eq!(stanza.tag, "message");
    assert_eq!(stanza.attrs().optional_jid("to"), Some(destination.clone()));
    let participants = stanza
        .get_optional_child("participants")
        .expect("encrypted fanout");
    let recipients: Vec<_> = participants.get_children_by_tag("to").collect();
    assert!(!recipients.is_empty());
    for recipient in recipients {
        let enc = recipient
            .get_optional_child("enc")
            .expect("Signal ciphertext");
        assert!(enc.content_bytes().is_some_and(|b| !b.is_empty()));
        assert!(matches!(
            enc.attrs().optional_string("type").as_deref(),
            Some("pkmsg" | "msg")
        ));
    }
}

async fn assert_dm_operations(legacy: Option<bool>, nct: bool, migrated: bool) {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (pn, lid) = seed_dm_wire_namespace_state(&client).await;
    configure_tokens(&client, &lid, legacy, nct).await;
    if migrated {
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLidMigrated(true))
            .await;
    }
    for operation in 0..3 {
        let result = match operation {
            0 => client
                .send_message(pn.clone(), wa::Message::text("offline text"))
                .await
                .unwrap(),
            1 => client
                .send_reaction(
                    pn.clone(),
                    wa::MessageKey {
                        remote_jid: Some(pn.to_string()),
                        from_me: Some(false),
                        id: Some("ORIGINAL".into()),
                        participant: None,
                    },
                    "👍",
                )
                .await
                .unwrap(),
            2 => client
                .edit_message(pn.clone(), "ORIGINAL", wa::Message::text("offline edit"))
                .await
                .unwrap(),
            _ => unreachable!(),
        };
        let owned = crate::test_utils::decode_sent_iq(&transport, operation).await;
        let stanza = owned.get();
        assert_encrypted_dm(stanza, if migrated { &lid } else { &pn });
        assert_tokens(stanza, Some(RECEIVED_TOKEN), None);
        assert_eq!(
            stanza.attrs().optional_string("id").as_deref(),
            Some(result.message_id.as_str())
        );
        if operation == 0 {
            assert!(
                stanza.get_optional_child("device-identity").is_some(),
                "authenticated pre-key envelope"
            );
        }
        if operation == 2 {
            assert_eq!(stanza.attrs().optional_string("edit").as_deref(), Some("1"));
        }
    }
    assert_eq!(transport.sent().len(), 3, "no per-message token issuance");
    let entry = client
        .persistence_manager
        .backend()
        .get_tc_token(&lid.user)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(entry.token, RECEIVED_TOKEN);
    assert!(
        client
            .persistence_manager
            .backend()
            .get_tc_token(&pn.user)
            .await
            .unwrap()
            .is_none(),
        "recipient LID owns the stored token"
    );
}

#[tokio::test]
async fn dm_valid_tc_token_with_legacy_flag_false_and_nct_off() {
    assert_dm_operations(Some(false), false, false).await;
}

#[tokio::test]
async fn dm_valid_tc_token_with_legacy_flag_false_and_nct_available() {
    assert_dm_operations(Some(false), true, false).await;
}

#[tokio::test]
async fn dm_valid_tc_token_with_legacy_flag_absent_and_lid_addressing() {
    assert_dm_operations(None, true, true).await;
}

#[tokio::test]
async fn dm_valid_tc_token_with_legacy_flag_true() {
    assert_dm_operations(Some(true), true, false).await;
}

/// Exercise actual backend lookup, expiry props, raw salt and bare LID binding,
/// rather than replacing them with selector booleans. Sender cadence is separate.
#[tokio::test]
async fn attachment_validity_props_and_identity_matrix() {
    let client = crate::test_utils::create_test_client().await;
    let now = 10 * 3600 + 100;
    let cutoff = 9 * 3600; // receiver duration=3600, buckets=2
    for has_lid in [false, true] {
        let to = if has_lid {
            Jid::lid("770000001").with_device(17)
        } else {
            Jid::pn("5511000000042").with_device(17)
        };
        for legacy in [None, Some(false), Some(true)] {
            for nct in [false, true] {
                for salt in [None, Some(SALT)] {
                    for state in 0..4 {
                        // absent, empty, expired, valid exactly at cutoff
                        configure_props(&client, legacy, nct).await;
                        client
                            .ab_props
                            .apply_props(
                                true,
                                [
                                    (web::TCTOKEN_DURATION.code, "3600".into()),
                                    (web::TCTOKEN_NUM_BUCKETS.code, "2".into()),
                                ]
                                .into_iter(),
                            )
                            .await;
                        client
                            .persistence_manager
                            .process_command(DeviceCommand::SetNctSalt(salt.map(<[u8]>::to_vec)))
                            .await;
                        let backend = client.persistence_manager.backend();
                        let entry = TcTokenEntry {
                            token: if state == 1 {
                                Vec::new()
                            } else {
                                RECEIVED_TOKEN.to_vec()
                            },
                            token_timestamp: if state == 2 { cutoff - 1 } else { cutoff },
                            sender_timestamp: Some(now),
                        };
                        if state == 0 {
                            backend.delete_tc_token(&to.user).await.unwrap();
                        } else {
                            backend.put_tc_token(&to.user, &entry).await.unwrap();
                        }
                        let mut nodes = Vec::new();
                        let should_issue = client
                            .maybe_include_tc_token(&to, &mut nodes, SendInstant(now))
                            .await;
                        assert_eq!(
                            should_issue,
                            state == 0,
                            "attachment does not alter sender cadence"
                        );
                        let expected_cs = (state != 3 && nct && salt.is_some() && has_lid)
                            .then(|| compute_cs_token(SALT, "770000001@lid"));
                        let expected_tc = (state == 3).then_some(RECEIVED_TOKEN);
                        assert_eq!(
                            nodes.len(),
                            usize::from(expected_tc.is_some() || expected_cs.is_some()),
                            "LID={has_lid}, legacy={legacy:?}, NCT={nct}, salt={}, state={state}",
                            salt.is_some()
                        );
                        let wrapper = NodeBuilder::new("message").children(nodes).build();
                        assert_tokens(&wrapper.as_node_ref(), expected_tc, expected_cs.as_deref());
                        if state != 0 {
                            let stored = backend.get_tc_token(&to.user).await.unwrap().unwrap();
                            assert_eq!(stored.token, entry.token);
                            assert_eq!(stored.token_timestamp, entry.token_timestamp);
                            assert_eq!(stored.sender_timestamp, entry.sender_timestamp);
                        }
                    }
                }
            }
        }
    }
    assert_ne!(
        compute_cs_token(SALT, "770000001@lid"),
        compute_cs_token(SALT, "770000001:17@lid")
    );
}

#[tokio::test]
async fn dm_invalid_received_token_uses_only_guarded_fallback() {
    for (token, timestamp, nct, salt, expect_cs) in [
        (RECEIVED_TOKEN.to_vec(), 0, true, Some(SALT.to_vec()), true),
        (
            Vec::new(),
            wacore::time::now_secs(),
            true,
            Some(SALT.to_vec()),
            true,
        ),
        (
            RECEIVED_TOKEN.to_vec(),
            0,
            false,
            Some(SALT.to_vec()),
            false,
        ),
        (RECEIVED_TOKEN.to_vec(), 0, true, None, false),
    ] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let (pn, lid) = seed_dm_wire_namespace_state(&client).await;
        configure_props(&client, Some(true), nct).await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetNctSalt(salt))
            .await;
        client
            .persistence_manager
            .backend()
            .put_tc_token(
                &lid.user,
                &TcTokenEntry {
                    token,
                    token_timestamp: timestamp,
                    sender_timestamp: Some(wacore::time::now_secs()),
                },
            )
            .await
            .unwrap();
        client
            .send_message(pn.clone(), wa::Message::text("fallback control"))
            .await
            .unwrap();
        let owned = crate::test_utils::decode_sent_iq(&transport, 0).await;
        assert_encrypted_dm(owned.get(), &pn);
        let cs = expect_cs.then(|| compute_cs_token(SALT, &lid.to_string()));
        assert_tokens(owned.get(), None, cs.as_deref());
        assert_eq!(transport.sent().len(), 1);
    }
}

#[tokio::test]
async fn self_pn_and_lid_sends_never_attach_tokens() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    seed_dm_wire_namespace_state(&client).await;
    let own = client.persistence_manager.get_device_snapshot();
    let pn = own.pn.as_ref().unwrap().to_non_ad();
    let lid = own.lid.as_ref().unwrap().to_non_ad();
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(pn.with_device(1))))
        .await;
    client
        .add_lid_pn_mapping(
            &lid.user,
            &pn.user,
            crate::lid_pn_cache::LearningSource::Usync,
        )
        .await
        .unwrap();
    crate::test_utils::seed_peer_session(&client, &lid).await;
    configure_tokens(&client, &lid, Some(true), true).await;
    for (index, target) in [pn.clone(), lid].into_iter().enumerate() {
        client
            .send_message(target, wa::Message::text("self control"))
            .await
            .unwrap();
        let owned = crate::test_utils::decode_sent_iq(&transport, index).await;
        assert_encrypted_dm(owned.get(), &pn);
        assert_tokens(owned.get(), None, None);
    }
    assert_eq!(transport.sent().len(), 2);
}

#[tokio::test]
async fn namespace_fixture_same_second_seed_replay_counterfactual() {
    use crate::lid_pn_cache::{LearningSource, LidPnCache, LidPnEntry};

    let pn = "100000000000777";
    let original_lid = "555000000000777";
    let replacement_lid = "111111111111";
    let seed_second = 1_700_000_000;
    for (offset, expected_after_replay) in [(0, original_lid), (1, replacement_lid)] {
        let cache = LidPnCache::new();
        let original =
            LidPnEntry::with_timestamp(original_lid, pn, seed_second, LearningSource::Usync);
        cache.add(&original).await;
        // Model a startup read that finishes applying after the live remap.
        // The strict-newer twin distinguishes timestamp ordering from tokens.
        let startup_snapshot = [original];
        let replacement = LidPnEntry::with_timestamp(
            replacement_lid,
            pn,
            seed_second + offset,
            LearningSource::Usync,
        );
        cache.add(&replacement).await;
        assert_eq!(
            cache.get_current_lid(pn).await.as_deref(),
            Some(replacement_lid)
        );
        cache.warm_up(startup_snapshot).await;
        assert_eq!(
            cache.get_current_lid(pn).await.as_deref(),
            Some(expected_after_replay)
        );
    }
}

#[tokio::test]
async fn other_namespace_peer_still_gets_received_token() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    // This tests full JID domains, not remapping: seed the intended alias once
    // so a delayed same-second startup snapshot cannot replay another alias.
    let (pn, lid) =
        seed_dm_wire_namespace_state_for_peer_lid(&client, Jid::lid("111111111111")).await;
    let own_pn = client.pn().unwrap();
    assert_eq!(lid.user, own_pn.user);
    assert_ne!(lid.server, own_pn.server);
    // A companion, not device 0: the existing fanout filter must not mistake
    // the synthetic peer's primary device for our exact sending-device tuple.
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(own_pn.with_device(1))))
        .await;
    configure_tokens(&client, &lid, Some(false), true).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    let result = client
        .send_message(pn.clone(), wa::Message::text("namespace control"))
        .await
        .unwrap();
    let owned = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_eq!(
        owned.get().attrs().optional_string("id").as_deref(),
        Some(result.message_id.as_str())
    );
    assert_encrypted_dm(owned.get(), &lid);
    assert_tokens(owned.get(), Some(RECEIVED_TOKEN), None);
}

#[tokio::test]
async fn group_newsletter_and_status_addon_keep_token_exclusions() {
    let group = GroupSendFixture::new().await;
    configure_tokens(&group.client, &group.group, Some(true), true).await;
    group.send_text("group control").await;
    let owned = group.stanza(0).await;
    assert_eq!(
        owned.get().attrs().optional_jid("to"),
        Some(group.group.clone())
    );
    assert_tokens(owned.get(), None, None);

    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (pn, lid) = seed_dm_wire_namespace_state(&client).await;
    configure_tokens(&client, &lid, Some(true), true).await;
    let channel: Jid = "120363000000000042@newsletter".parse().unwrap();
    configure_tokens(&client, &channel, Some(true), true).await;
    client
        .newsletter()
        .edit_message(&channel, "ORIGINAL", wa::Message::text("channel control"))
        .await
        .unwrap();
    let owned = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_eq!(owned.get().attrs().optional_jid("to"), Some(channel));
    assert!(owned.get().get_optional_child("plaintext").is_some());
    assert_tokens(owned.get(), None, None);

    client
        .send_message(
            Jid::status_broadcast(),
            wa::Message {
                reaction_message: buffa::MessageField::some(wa::message::ReactionMessage {
                    key: buffa::MessageField::some(wa::MessageKey {
                        remote_jid: Some(Jid::status_broadcast().to_string()),
                        from_me: Some(false),
                        id: Some("STATUSPOST".into()),
                        participant: Some(pn.to_string()),
                    }),
                    text: Some("👍".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let owned = crate::test_utils::decode_sent_iq(&transport, 1).await;
    assert_encrypted_dm(owned.get(), &Jid::status_broadcast());
    assert_tokens(owned.get(), None, None);
    assert_eq!(transport.sent().len(), 2);

    let bot: Jid = "770000099@bot".parse().unwrap();
    configure_tokens(&client, &bot, Some(true), true).await;
    let mut nodes = Vec::new();
    assert!(
        !client
            .maybe_include_tc_token(&bot, &mut nodes, SendInstant::now())
            .await
    );
    assert!(nodes.is_empty());
}

async fn sqlite_fixture(
    uri: &str,
) -> (
    Arc<Client>,
    Arc<crate::transport::mock::CapturingMockTransport>,
) {
    let backend = Arc::new(crate::store::SqliteStore::open(uri).await.unwrap());
    crate::test_utils::create_iq_test_client_with_backend(backend).await
}

fn memory_uri() -> String {
    format!(
        "file:token_fixture_{}?mode=memory&cache=shared",
        uuid::Uuid::new_v4()
    )
}

/// Only synthetic storage is damaged; Signal/authentication tables stay intact.
async fn drop_fixture_table(uri: String, table: &'static str) {
    tokio::task::spawn_blocking(move || {
        use diesel::{Connection, connection::SimpleConnection};
        let mut conn = diesel::SqliteConnection::establish(&uri).unwrap();
        conn.batch_execute(&format!("DROP TABLE {table}")).unwrap();
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn received_token_survives_backend_reopen_and_dm_send() {
    let directory = std::env::temp_dir().join(format!("privacy-token-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&directory).unwrap();
    let cleanup = scopeguard::guard(directory, |directory| {
        // Best effort on assertion failure; successful cleanup is checked below.
        let _ = std::fs::remove_dir_all(directory);
    });
    let path = cleanup.join("tokens.db");
    let uri = path.to_str().unwrap();
    let (original, _) = sqlite_fixture(uri).await;
    let (_, lid) = seed_dm_wire_namespace_state(&original).await;
    configure_tokens(&original, &lid, Some(false), true).await;
    let stored = original
        .persistence_manager
        .backend()
        .get_tc_token(&lid.user)
        .await
        .unwrap()
        .unwrap();
    let backend = original.persistence_manager.backend();
    let weak_backend = Arc::downgrade(&backend);
    drop(backend);
    let released = original.store_release();
    original.shutdown().await.device.unwrap();
    drop(original);
    // No host-owned backend/Device handles or unawaited backend I/O exist in
    // this fixture. Join crate ownership, then prove its original store died
    // before opening a new pool; this is a clean reopen, not a power-loss test.
    tokio::time::timeout(Duration::from_secs(5), released.wait())
        .await
        .unwrap();
    assert!(
        weak_backend.upgrade().is_none(),
        "original store is still alive"
    );
    let (reloaded, transport) = sqlite_fixture(uri).await;
    let (pn, _) = seed_dm_wire_namespace_state(&reloaded).await;
    configure_props(&reloaded, None, true).await;
    reloaded
        .send_message(pn.clone(), wa::Message::text("reload control"))
        .await
        .unwrap();
    let owned = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_encrypted_dm(owned.get(), &pn);
    assert_tokens(owned.get(), Some(RECEIVED_TOKEN), None);
    let after = reloaded
        .persistence_manager
        .backend()
        .get_tc_token(&lid.user)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.token, stored.token);
    assert_eq!(after.token_timestamp, stored.token_timestamp);
    assert_eq!(after.sender_timestamp, stored.sender_timestamp);
    let released = reloaded.store_release();
    reloaded.shutdown().await.device.unwrap();
    drop(reloaded);
    tokio::time::timeout(Duration::from_secs(5), released.wait())
        .await
        .unwrap();
    std::fs::remove_dir_all(scopeguard::ScopeGuard::into_inner(cleanup)).unwrap();
}

#[tokio::test]
async fn token_read_failure_retains_nct_fallback() {
    let uri = memory_uri();
    let (client, transport) = sqlite_fixture(&uri).await;
    let (pn, lid) = seed_dm_wire_namespace_state(&client).await;
    configure_tokens(&client, &lid, Some(false), true).await;
    drop_fixture_table(uri, "tc_tokens").await;
    assert!(
        client
            .persistence_manager
            .backend()
            .get_tc_token(&lid.user)
            .await
            .is_err()
    );
    client
        .send_message(pn.clone(), wa::Message::text("storage failure control"))
        .await
        .unwrap();
    let owned = crate::test_utils::decode_sent_iq(&transport, 0).await;
    assert_encrypted_dm(owned.get(), &pn);
    assert_tokens(
        owned.get(),
        None,
        Some(&compute_cs_token(SALT, &lid.to_string())),
    );
}

#[tokio::test]
async fn nack_reachout_and_device_removal_remain_distinct() {
    use wacore::types::events::Event;
    let (client, _) = crate::test_utils::create_iq_test_client().await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let ack = NodeBuilder::new("ack")
        .attr("class", "message")
        .attr("id", "NACK")
        .attr("from", "s.whatsapp.net")
        .attr("error", "463")
        .build();
    client
        .process_node(crate::test_utils::node_to_owned_ref(&ack))
        .await;
    assert!(client.auto_reconnect_enabled());
    assert!(
        collector
            .events()
            .iter()
            .any(|event| matches!(&**event, Event::ServerAck(_)))
    );
    assert!(
        !collector
            .events()
            .iter()
            .any(|event| matches!(&**event, Event::LoggedOut(_)))
    );

    let payload = serde_json::json!({"data": {"xwa2_notify_account_reachout_timelock": {
        "enforcement_type": "RESTRICT_ALL_COMPANIONS", "is_active": true,
        "time_enforcement_ends": "1704153600", "future_field": {"keep": true}
    }}, "extensions": {"keep": [1, 2, 3]}});
    let notify = NodeBuilder::new("notification")
        .attr("type", "mex")
        .attr("id", "TIMELOCK")
        .attr("from", "s.whatsapp.net")
        .children([NodeBuilder::new("update")
            .attr("op_name", "NotificationUserReachoutTimelockUpdate")
            .bytes(payload.to_string().into_bytes())
            .build()])
        .build();
    client
        .process_node(crate::test_utils::node_to_owned_ref(&notify))
        .await;
    assert!(client.auto_reconnect_enabled());
    let conflict = NodeBuilder::new("stream:error")
        .children([NodeBuilder::new("conflict")
            .attr("type", "device_removed")
            .build()])
        .build();
    client.handle_stream_error(&conflict.as_node_ref()).await;
    assert!(!client.auto_reconnect_enabled());
    let events = collector.events();
    let typed = events
        .iter()
        .position(|event| matches!(&**event, Event::ReachoutTimelockUpdate(_)))
        .unwrap();
    let raw = events
        .iter()
        .position(|event| matches!(&**event, Event::MexNotification(_)))
        .unwrap();
    let terminal = events
        .iter()
        .position(|event| matches!(&**event, Event::LoggedOut(_)))
        .unwrap();
    assert!(typed < raw && raw < terminal);
    if let Event::ReachoutTimelockUpdate(update) = &*events[typed] {
        assert_eq!(
            update.state.enforcement_type.as_deref(),
            Some("RESTRICT_ALL_COMPANIONS")
        );
    }
    if let Event::MexNotification(update) = &*events[raw] {
        assert_eq!(update.payload, payload);
    }
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(&***event, Event::LoggedOut(_)))
            .count(),
        1
    );
}

#[tokio::test]
async fn signal_persistence_failure_still_blocks_token_bearing_ciphertext() {
    let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
    let (client, transport) =
        crate::test_utils::create_iq_test_client_with_backend(backend.clone()).await;
    let (pn, lid) = seed_dm_wire_namespace_state(&client).await;
    configure_tokens(&client, &lid, Some(false), true).await;
    backend.set_fail_session_writes(true);
    assert!(
        client
            .send_message(pn, wa::Message::text("pre-wire failure control"))
            .await
            .is_err()
    );
    assert!(
        backend.session_batch_write_count() > 0,
        "durable flush was attempted"
    );
    assert!(
        transport.sent().is_empty(),
        "no ciphertext or token escapes the durability gate"
    );
}
