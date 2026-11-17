use super::*;

#[tokio::test]
async fn lid_dm_propagates_known_phone_to_other_devices() {
    for caller_uses_lid in [false, true] {
        let client = crate::test_utils::create_test_client().await;
        let (peer_pn, peer_lid) = seed_dm_wire_namespace_state(&client).await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLidMigrated(true))
            .await;
        let target = if caller_uses_lid {
            peer_lid.clone()
        } else {
            peer_pn.clone()
        }
        .with_device(9);
        let request_id = "DM_PEER_PHONE";
        let waiter = client
            .wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", request_id));
        let mut message = wa::Message::default();
        message.conversation = Some("hello".into());
        let result = client
            .send_message_impl(
                target,
                &message,
                SendPipelineOptions {
                    request_id: Some(request_id),
                    ..Default::default()
                },
            )
            .await;
        assert!(
            result.is_err(),
            "offline transport fails after capturing the stanza"
        );
        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(node.attrs().optional_jid("to"), Some(peer_lid.clone()));
        assert_eq!(
            node.attrs().optional_jid("peer_recipient_pn"),
            Some(peer_pn.clone())
        );
        assert!(node.attrs().optional_jid("recipient_pn").is_none());

        let own = client.persistence_manager.get_device_snapshot();
        let echo = NodeBuilder::new("message")
            .attr("id", request_id)
            .attr("from", own.lid.as_ref().unwrap())
            .attr("recipient", &peer_lid)
            .attr(
                "peer_recipient_pn",
                node.attrs().optional_jid("peer_recipient_pn").unwrap(),
            )
            .attr("t", "1700000000")
            .build();
        let info = wacore::messages::parse_message_info(
            &echo.as_node_ref(),
            own.pn.as_ref().unwrap(),
            own.lid.as_ref(),
        )
        .unwrap();
        assert_eq!(info.source.chat, peer_lid);
        assert_eq!(info.source.recipient_alt, Some(peer_pn.clone()));
        let receiver = crate::test_utils::create_test_client().await;
        assert!(
            receiver
                .lid_pn_cache
                .get_phone_number(&peer_lid.user)
                .await
                .is_none()
        );
        receiver
            .cache_lid_pn_from_message(&info.source.chat, info.source.recipient_alt.as_ref(), false)
            .await;
        assert_eq!(
            receiver
                .lid_pn_cache
                .get_phone_number(&peer_lid.user)
                .await
                .as_deref(),
            Some(peer_pn.user.as_str())
        );
    }
}

async fn capture(client: &Arc<Client>, target: Jid, message: &wa::Message, id: &str) -> Arc<Node> {
    let waiter =
        client.wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", id));
    assert!(
        client
            .send_message_impl(
                target,
                message,
                SendPipelineOptions {
                    request_id: Some(id),
                    ..Default::default()
                }
            )
            .await
            .is_err()
    );
    tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
        .await
        .unwrap()
        .unwrap()
}

fn text() -> wa::Message {
    let mut message = wa::Message::default();
    message.conversation = Some("hello".into());
    message
}

#[tokio::test]
async fn lid_dm_uses_reverse_mapping_instead_of_retired_caller_phone() {
    let client = crate::test_utils::create_test_client().await;
    let (old_phone, lid) = seed_dm_wire_namespace_state(&client).await;
    let current_phone = Jid::pn("12025550187");
    client
        .add_lid_pn_mapping(
            &lid.user,
            &current_phone.user,
            crate::lid_pn_cache::LearningSource::Usync,
        )
        .await
        .unwrap();
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    assert_eq!(client.resolve_dm_wire_jid(&old_phone).await, lid);
    assert_eq!(
        client
            .get_lid_pn_entry(&lid)
            .await
            .unwrap()
            .unwrap()
            .phone_number
            .as_ref(),
        current_phone.user.as_str()
    );
    let node = capture(&client, old_phone, &text(), "CURRENT_PEER_PHONE").await;
    assert_eq!(node.attrs().optional_jid("to"), Some(lid));
    assert_eq!(
        node.attrs().optional_jid("peer_recipient_pn"),
        Some(current_phone)
    );
}

#[tokio::test]
async fn lid_dm_restores_phone_from_backend_after_reverse_cache_eviction() {
    let client = crate::test_utils::create_test_client().await;
    let (phone, lid) = seed_dm_wire_namespace_state(&client).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    client.lid_pn_cache.evict_reverse_for_tests(&lid.user).await;
    assert!(
        client
            .lid_pn_cache
            .get_phone_number(&lid.user)
            .await
            .is_none()
    );
    let node = capture(&client, lid, &text(), "COLD_PEER_PHONE").await;
    assert_eq!(node.attrs().optional_jid("peer_recipient_pn"), Some(phone));
}

#[tokio::test]
async fn lid_dm_without_known_phone_omits_phone_attributes() {
    let client = crate::test_utils::create_test_client().await;
    let (_, _) = seed_dm_wire_namespace_state(&client).await;
    let lid = Jid::lid("100000000000888");
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    client
        .update_device_list(wacore::store::traits::DeviceListRecord {
            user: lid.user.as_str().into(),
            devices: [wacore::store::traits::DeviceInfo::new(0, None)].into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        })
        .await
        .unwrap();
    crate::test_utils::seed_peer_session(&client, &lid).await;
    let node = capture(&client, lid.clone(), &text(), "UNKNOWN_PEER_PHONE").await;
    assert_eq!(node.attrs().optional_jid("to"), Some(lid));
    assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
    assert!(node.attrs().optional_jid("recipient_pn").is_none());
}

#[tokio::test]
async fn pn_wire_dm_omits_peer_phone_for_pn_and_lid_callers() {
    for use_lid in [false, true] {
        let client = crate::test_utils::create_test_client().await;
        let (phone, lid) = seed_dm_wire_namespace_state(&client).await;
        let node = capture(
            &client,
            if use_lid { lid } else { phone.clone() },
            &text(),
            "PN_PEER_PHONE",
        )
        .await;
        assert_eq!(node.attrs().optional_jid("to"), Some(phone));
        assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
    }
}

#[tokio::test]
async fn status_reaction_omits_peer_phone_even_with_known_mapping() {
    let client = crate::test_utils::create_test_client().await;
    let (_, lid) = seed_dm_wire_namespace_state(&client).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    let mut message = wa::Message::default();
    let reaction = message.reaction_message.get_or_insert_default();
    reaction.text = Some("👍".into());
    let key = reaction.key.get_or_insert_default();
    key.remote_jid = Some(Jid::status_broadcast().to_string());
    key.participant = Some(lid.to_string());
    key.id = Some("STATUS_ORIGINAL".into());
    let node = capture(
        &client,
        Jid::status_broadcast(),
        &message,
        "STATUS_PEER_PHONE",
    )
    .await;
    assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
}

#[tokio::test]
async fn own_device_retry_propagates_known_peer_phone() {
    for use_lid in [false, true] {
        let client = crate::test_utils::create_test_client().await;
        let (phone, lid) = seed_dm_wire_namespace_state(&client).await;
        let own = client.persistence_manager.get_device_snapshot();
        let requester = if use_lid {
            own.lid.as_ref()
        } else {
            own.pn.as_ref()
        }
        .unwrap()
        .with_device(7);
        let node = capture_retry(&client, &lid, &requester, Some(&lid)).await;
        assert_eq!(node.attrs().optional_jid("to"), Some(requester));
        assert_eq!(node.attrs().optional_jid("recipient"), Some(lid));
        assert_eq!(node.attrs().optional_jid("peer_recipient_pn"), Some(phone));
    }
}

async fn capture_retry(
    client: &Arc<Client>,
    chat: &Jid,
    requester: &Jid,
    recipient: Option<&Jid>,
) -> Arc<Node> {
    crate::test_utils::seed_peer_session(client, requester).await;
    let id = "RETRY_PEER_PHONE";
    let message = if let Some(recipient) = recipient {
        wacore::messages::wrap_device_sent(text(), recipient.to_string())
    } else {
        text()
    };
    let mut request = crate::features::MessageRetransmission::new(
        chat.clone(),
        requester.clone(),
        message,
        crate::MessageId::new(id).unwrap(),
        1,
    );
    if let Some(recipient) = recipient {
        request = request.with_recipient(recipient.clone());
    }
    let waiter =
        client.wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", id));
    assert!(client.retransmit_message(request).await.is_err());
    tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn own_device_retry_omits_phone_for_pn_or_unknown_lid_chat() {
    for unknown_lid in [false, true] {
        let client = crate::test_utils::create_test_client().await;
        let (phone, _) = seed_dm_wire_namespace_state(&client).await;
        let chat = if unknown_lid {
            Jid::lid("100000000000888")
        } else {
            phone
        };
        let requester = client
            .persistence_manager
            .get_device_snapshot()
            .pn
            .as_ref()
            .unwrap()
            .with_device(7);
        let node = capture_retry(&client, &chat, &requester, Some(&chat)).await;
        assert_eq!(node.attrs().optional_jid("recipient"), Some(chat));
        assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
        assert!(node.attrs().optional_jid("recipient_pn").is_none());
    }
}

#[tokio::test]
async fn remote_device_retry_does_not_carry_own_device_phone_metadata() {
    let client = crate::test_utils::create_test_client().await;
    let (_, lid) = seed_dm_wire_namespace_state(&client).await;
    let requester = lid.with_device(7);
    let node = capture_retry(&client, &lid, &requester, None).await;
    assert_eq!(node.attrs().optional_jid("to"), Some(requester));
    assert!(node.attrs().optional_jid("recipient").is_none());
    assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
}

#[tokio::test]
async fn retry_phone_metadata_requires_matching_account_namespace() {
    let client = crate::test_utils::create_test_client().await;
    let (_, chat) = seed_dm_wire_namespace_state(&client).await;
    let own = client.persistence_manager.get_device_snapshot();
    let requester = Jid::lid(own.pn.as_ref().unwrap().user.as_str()).with_device(7);
    let node = capture_retry(&client, &chat, &requester, Some(&chat)).await;
    assert!(node.attrs().optional_jid("peer_recipient_pn").is_none());
}

#[tokio::test]
async fn self_chat_uses_own_phone_without_a_cached_mapping() {
    let client = crate::test_utils::create_test_client().await;
    seed_dm_wire_namespace_state(&client).await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(true))
        .await;
    let own = client.persistence_manager.get_device_snapshot();
    let lid = own.lid.as_ref().unwrap().to_non_ad();
    let phone = own.pn.as_ref().unwrap().to_non_ad();
    assert!(client.get_lid_pn_entry(&lid).await.unwrap().is_none());
    for user in [&lid, &phone] {
        client
            .update_device_list(wacore::store::traits::DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [
                    wacore::store::traits::DeviceInfo::new(0, None),
                    wacore::store::traits::DeviceInfo::new(7, None),
                ]
                .into(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        crate::test_utils::seed_peer_session(&client, &user.with_device(7)).await;
    }
    let node = capture(&client, lid.clone(), &text(), "SELF_PEER_PHONE").await;
    assert_eq!(node.attrs().optional_jid("to"), Some(lid));
    assert_eq!(node.attrs().optional_jid("peer_recipient_pn"), Some(phone));
}
