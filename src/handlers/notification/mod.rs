use super::traits::StanzaHandler;
use crate::client::Client;
use crate::types::events::Event;
use async_trait::async_trait;
use log::debug;
use std::sync::Arc;
use wacore_binary::OwnedNodeRef;

/// Handler for `<notification>` stanzas.
///
/// Processes various notification types including:
/// - Encrypt notifications (key upload requests)
/// - Server sync notifications
/// - Account sync notifications (push name updates)
/// - Device notifications (device add/remove/update)
#[derive(Default)]
pub struct NotificationHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for NotificationHandler {
    fn tag(&self) -> &'static str {
        "notification"
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        handle_notification_impl(&client, node).await;
        true
    }
}

/// Dispatch notification by type. Each arm calls a separate async fn so the
/// compiler doesn't size this future for all arms simultaneously.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.dispatch", level = "debug", skip_all)
)]
async fn handle_notification_impl(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    let nr = node.get();
    let notification_type = nr.attrs().optional_string("type");

    match notification_type.as_deref().unwrap_or_default() {
        "encrypt" => handle_encrypt_notification(client, nr).await,
        "server_sync" => handle_server_sync_notification(client, nr),
        "account_sync" => handle_account_sync_notification(client, nr).await,
        "devices" => handle_devices_notification(client, nr).await,
        "link_code_companion_reg" => {
            crate::pair_code::handle_pair_code_notification(client, nr).await;
        }
        "business" => handle_business_notification(client, nr).await,
        "picture" => handle_picture_notification(client, nr),
        "privacy_token" => handle_privacy_token_notification(client, nr).await,
        "status" => handle_status_notification(client, nr),
        "contacts" => handle_contacts_notification(client, nr).await,
        "w:gp2" => handle_group_notification(client, Arc::clone(&node)).await,
        "disappearing_mode" => handle_disappearing_mode_notification(client, nr),
        "newsletter" => handle_newsletter_notification(client, Arc::clone(&node)),
        "mex" => handle_mex_notification(client, nr),
        crate::passkey::flow::NOTIF_PASSKEY_REQUEST => {
            crate::passkey::flow::handle_passkey_notification(client, Arc::clone(&node)).await;
        }
        crate::passkey::flow::NOTIF_PASSKEY_CONTINUATION => {
            crate::passkey::flow::handle_passkey_continuation(client, Arc::clone(&node)).await;
        }
        "mediaretry" => {
            debug!(
                "Received mediaretry notification for msg {}",
                nr.attrs().optional_string("id").unwrap_or_default()
            );
        }
        other => {
            debug!("Unhandled notification type '{other}', dispatching raw event");
            client
                .core
                .event_bus
                .dispatch(Event::Notification(Arc::clone(&node)));
        }
    }
}

mod device;
mod groups;
mod privacy_business;
mod profile;

// `pub(crate)` re-export keeps `crate::handlers::notification::handle_local_identity_change`
// resolving for device_registry.rs.
pub(crate) use device::*;
use groups::*;
use privacy_business::*;
use profile::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::{TestEventCollector, create_test_client};
    use std::sync::Arc;
    use wacore::stanza::devices::{DeviceNotification, DeviceNotificationType};
    use wacore::stanza::groups::GroupNotificationAction;
    use wacore::types::events::{
        ContactNumberChanged, ContactSyncRequested, ContactUpdated, DeviceListUpdateType,
    };
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{Jid, Node};

    fn node_to_arc(node: Node) -> Arc<OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(&node)
    }

    #[test]
    fn test_parse_device_add_notification() {
        // Per WhatsApp Web: add operation has single device + key-index-list
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("add")
                .children([
                    NodeBuilder::new("device")
                        .attr("jid", "1234567890:1@s.whatsapp.net")
                        .build(),
                    NodeBuilder::new("key-index-list")
                        .attr("ts", "1000")
                        .bytes(vec![0x01, 0x02, 0x03])
                        .build(),
                ])
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(parsed.operation.operation_type, DeviceNotificationType::Add);
        assert_eq!(parsed.operation.device_ids(), vec![1]);
        // Verify key index info
        assert!(parsed.operation.key_index.is_some());
        assert_eq!(parsed.operation.key_index.as_ref().unwrap().timestamp, 1000);
    }

    #[test]
    fn test_parse_device_remove_notification() {
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("remove")
                .children([
                    NodeBuilder::new("device")
                        .attr("jid", "1234567890:3@s.whatsapp.net")
                        .build(),
                    NodeBuilder::new("key-index-list")
                        .attr("ts", "2000")
                        .build(),
                ])
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Remove
        );
        assert_eq!(parsed.operation.device_ids(), vec![3]);
    }

    #[test]
    fn test_parse_device_update_notification_with_hash() {
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("update")
                .attr("hash", "2:abcdef123456")
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Update
        );
        assert_eq!(
            parsed.operation.contact_hash,
            Some("2:abcdef123456".to_string())
        );
        // Update operations don't have devices (just hash for lookup)
        assert!(parsed.operation.devices.is_empty());
    }

    #[test]
    fn test_parse_empty_device_notification_fails() {
        // Per WhatsApp Web: at least one operation (add/remove/update) is required
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .build();

        let result = DeviceNotification::try_parse(&node.as_node_ref());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing required operation")
        );
    }

    #[test]
    fn test_parse_multiple_operations_uses_priority() {
        // Per WhatsApp Web: only ONE operation is processed with priority remove > add > update
        // If both remove and add are present, remove should be processed
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([
                NodeBuilder::new("add")
                    .children([
                        NodeBuilder::new("device")
                            .attr("jid", "1234567890:5@s.whatsapp.net")
                            .build(),
                        NodeBuilder::new("key-index-list")
                            .attr("ts", "3000")
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new("remove")
                    .children([
                        NodeBuilder::new("device")
                            .attr("jid", "1234567890:2@s.whatsapp.net")
                            .build(),
                        NodeBuilder::new("key-index-list")
                            .attr("ts", "3001")
                            .build(),
                    ])
                    .build(),
            ])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        // Should process remove, not add (priority: remove > add > update)
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Remove
        );
        assert_eq!(parsed.operation.device_ids(), vec![2]);
    }

    #[test]
    fn test_device_list_update_type_from_notification_type() {
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Add),
            DeviceListUpdateType::Add
        );
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Remove),
            DeviceListUpdateType::Remove
        );
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Update),
            DeviceListUpdateType::Update
        );
    }

    // Tests for account_sync device parsing

    #[test]
    fn test_parse_account_sync_device_list_basic() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "15551234567@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "15551234567:64@s.whatsapp.net")
                    .attr("key-index", "2")
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert_eq!(devices.len(), 2);

        // Primary device (device 0)
        assert_eq!(devices[0].jid.user, "15551234567");
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[0].key_index, None);

        // Companion device (device 64)
        assert_eq!(devices[1].jid.user, "15551234567");
        assert_eq!(devices[1].jid.device, 64);
        assert_eq!(devices[1].key_index, Some(2));
    }

    #[test]
    fn test_parse_account_sync_device_list_with_key_index_list() {
        // Real-world structure includes <key-index-list> which should be ignored
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "15551234567@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "15551234567:77@s.whatsapp.net")
                    .attr("key-index", "15")
                    .build(),
                NodeBuilder::new("key-index-list")
                    .attr("ts", "1766612162")
                    .bytes(vec![0x01, 0x02, 0x03]) // Simulated signed bytes
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        // Should only parse <device> tags, not <key-index-list>
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[1].jid.device, 77);
        assert_eq!(devices[1].key_index, Some(15));
    }

    #[test]
    fn test_parse_account_sync_device_list_empty() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert!(devices.is_empty());
    }

    #[test]
    fn test_parse_account_sync_device_list_multiple_devices() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:XYZ123")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "1234567890@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:1@s.whatsapp.net")
                    .attr("key-index", "1")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:2@s.whatsapp.net")
                    .attr("key-index", "5")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:3@s.whatsapp.net")
                    .attr("key-index", "10")
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert_eq!(devices.len(), 4);

        // Verify device IDs are correctly parsed
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[1].jid.device, 1);
        assert_eq!(devices[2].jid.device, 2);
        assert_eq!(devices[3].jid.device, 3);

        // Verify key indexes
        assert_eq!(devices[0].key_index, None);
        assert_eq!(devices[1].key_index, Some(1));
        assert_eq!(devices[2].key_index, Some(5));
        assert_eq!(devices[3].key_index, Some(10));
    }

    // ── disappearing_mode notification parsing tests ─────────────────────

    /// Helper: parse a disappearing_mode notification node the same way
    /// the handler does, returning `(duration, setting_timestamp)` or `None`
    /// on validation failure.
    fn parse_disappearing_mode(node: &Node) -> Option<(u32, i64)> {
        let dm_node = node.get_optional_child("disappearing_mode")?;
        let mut dm_attrs = dm_node.attrs();
        let duration = dm_attrs
            .optional_string("duration")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let setting_timestamp = dm_attrs
            .optional_string("t")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|&t| wacore::time::from_secs(t).is_some())?;
        Some((duration, setting_timestamp))
    }

    #[test]
    fn test_parse_disappearing_mode_valid() {
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("type", "disappearing_mode")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "86400")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, ts) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 86400);
        assert_eq!(ts, 1773519041);
    }

    #[test]
    fn test_parse_disappearing_mode_disabled() {
        // duration=0 means disappearing messages disabled
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "0")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, ts) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 0, "duration=0 means disabled");
        assert_eq!(ts, 1773519041);
    }

    #[test]
    fn test_parse_disappearing_mode_missing_child() {
        // No <disappearing_mode> child → returns None
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("type", "disappearing_mode")
            .build();

        assert!(
            parse_disappearing_mode(&node).is_none(),
            "should return None when child element is missing"
        );
    }

    #[test]
    fn test_parse_disappearing_mode_missing_timestamp() {
        // Missing 't' attribute → returns None (required field)
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "86400")
                .build()])
            .build();

        assert!(
            parse_disappearing_mode(&node).is_none(),
            "should return None when 't' attribute is missing"
        );
    }

    #[test]
    fn test_parse_disappearing_mode_missing_duration_defaults_to_zero() {
        // Missing duration defaults to 0 (WA Web: attrInt("duration", 0))
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, _) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 0, "missing duration should default to 0");
    }

    #[tokio::test]
    async fn test_contacts_update_dispatches_contact_updated_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-update-1")
            .attr("t", "1773519041")
            .children([NodeBuilder::new("update")
                .attr("jid", "5511999999999@s.whatsapp.net")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactUpdated(ContactUpdated { jid, .. })
                    if jid == &Jid::pn("5511999999999")
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_modify_with_lid_creates_mappings() {
        // WA Web: old/new are PN JIDs, old_lid/new_lid are LID JIDs.
        // Creates two mappings: old_lid→old_pn AND new_lid→new_pn.
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-1")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .attr("old_lid", "100000011111111@lid")
                .attr("new_lid", "100000022222222@lid")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        // Both LID-PN mappings should be created
        assert_eq!(
            client
                .lid_pn_cache
                .get_phone_number("100000011111111")
                .await,
            Some("5511999999999".to_string()),
            "old_lid should map to old PN"
        );
        assert_eq!(
            client
                .lid_pn_cache
                .get_phone_number("100000022222222")
                .await,
            Some("5511888888888".to_string()),
            "new_lid should map to new PN"
        );

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactNumberChanged(ContactNumberChanged {
                        old_jid, new_jid, old_lid, new_lid, ..
                    })
                    if old_jid == &Jid::pn("5511999999999")
                        && new_jid == &Jid::pn("5511888888888")
                        && old_lid.is_some()
                        && new_lid.is_some()
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_modify_without_lid_skips_mapping() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-2")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        // Event should still be dispatched, just without LID info
        assert_eq!(collector.events().len(), 1);
    }

    #[tokio::test]
    async fn test_contacts_sync_dispatches_contact_sync_requested_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-sync-1")
            .children([NodeBuilder::new("sync").attr("after", "1773519041").build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactSyncRequested(ContactSyncRequested { after, .. })
                    if after.is_some()
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_add_remove_do_not_dispatch_events() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        for tag in ["add", "remove"] {
            let node = NodeBuilder::new("notification")
                .attr("type", "contacts")
                .attr("from", "s.whatsapp.net")
                .attr("id", format!("contacts-{tag}-1"))
                .children([NodeBuilder::new(tag).build()])
                .build();
            handle_notification_impl(&client, node_to_arc(node)).await;
        }

        assert!(
            collector.events().is_empty(),
            "add/remove should not dispatch events"
        );
    }

    #[tokio::test]
    async fn test_contacts_empty_notification_ignored() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // No child element
        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-empty-1")
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "empty contacts notification should not dispatch events"
        );
    }

    /// Same PN on both sides is still dispatched as a ContactNumberChanged
    /// event (with `old_jid == new_jid`). WA Web JS has no special guard for
    /// this case either; the LID mapping update is a no-op when LIDs are
    /// also equal. Consumers can filter if they care.
    #[tokio::test]
    async fn test_contacts_modify_same_jid_still_dispatches() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-same")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511999999999@s.whatsapp.net")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &*events[0],
            Event::ContactNumberChanged(ContactNumberChanged { old_jid, new_jid, .. })
                if old_jid == new_jid
        ));
    }

    /// Partial LID (only `new_lid`, missing `old_lid`) must NOT create any
    /// LID-PN mapping, since WA Web requires BOTH for createLidPnMappings.
    #[tokio::test]
    async fn test_contacts_modify_partial_lid_skips_mappings() {
        let client = create_test_client().await;

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-partial")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .attr("new_lid", "100000022222222@lid")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            client
                .lid_pn_cache
                .get_phone_number("100000022222222")
                .await
                .is_none(),
            "no mapping should be created when old_lid is missing"
        );
    }

    /// Missing `new` attribute: the parser should warn and not dispatch
    /// anything, mirroring WA Web's parser error path.
    #[tokio::test]
    async fn test_contacts_modify_missing_new_attr_drops_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-bad")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(collector.events().is_empty());
    }

    /// Group `w:gp2` change_number: the parsed action must carry the new
    /// owner from the child's `jid` attr and the sub_group_suggestions from
    /// `<sub_group_suggestion jid=.../>` children. The old owner is the
    /// notification's top-level `participant` attribute, surfaced on
    /// `GroupUpdate.participant`.
    #[tokio::test]
    async fn test_group_change_number_dispatches_with_new_owner_and_suggestions() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", "120363000000000000@g.us")
            .attr("participant", "5511999999999@s.whatsapp.net")
            .attr("id", "gp2-change-1")
            .attr("t", "1773519041")
            .children([NodeBuilder::new("change_number")
                .attr("jid", "5511888888888@s.whatsapp.net")
                .children([
                    NodeBuilder::new("sub_group_suggestion")
                        .attr("jid", "120363111111111111@g.us")
                        .build(),
                    NodeBuilder::new("sub_group_suggestion")
                        .attr("jid", "120363222222222222@g.us")
                        .build(),
                ])
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        let group_update = events
            .iter()
            .find_map(|e| match &**e {
                Event::GroupUpdate(u) => Some(u),
                _ => None,
            })
            .expect("expected GroupUpdate");

        assert_eq!(
            group_update.participant.as_ref().map(|j| j.user.as_str()),
            Some("5511999999999"),
            "old owner comes from notification.participant"
        );
        match &group_update.action {
            GroupNotificationAction::ChangeNumber {
                new_owner,
                sub_group_suggestions,
            } => {
                assert_eq!(
                    new_owner.as_ref().map(|j| j.user.as_str()),
                    Some("5511888888888")
                );
                assert_eq!(sub_group_suggestions.len(), 2);
            }
            other => panic!("expected ChangeNumber, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_contacts_update_hash_only_ignored() {
        // WA Web sends <update hash="Quvc"/> without jid when using hash-based lookup.
        // We don't maintain a userhash index, so this should be a no-op.
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "551199887766@s.whatsapp.net")
            .attr("id", "3251801952")
            .attr("t", "1773668072")
            .children([NodeBuilder::new("update").attr("hash", "Quvc").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "hash-only update without jid should not dispatch events"
        );
    }

    #[tokio::test]
    async fn test_identity_change_dispatches_event_and_invalidates_cache() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Pre-populate device registry so clear_device_record has something to clear
        let record = wacore::store::traits::DeviceListRecord {
            user: "5511999999999".into(),
            devices: vec![wacore::store::traits::DeviceInfo {
                device_id: 1,
                key_index: None,
            }],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: Some(42),
        };
        client
            .device_registry_cache
            .raw_insert_for_tests("5511999999999".into(), Arc::new(record))
            .await;

        // Seed a stored identity so the had-prior-identity gate runs the full reset
        // (delete + notify), matching WA Web's `if (!isEmpty(loadIdentityKey(addr)))`.
        {
            use wacore::types::jid::JidExt;
            let target: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
            client
                .signal_cache
                .put_identity(&target.to_protocol_address(), &[7u8; 32])
                .await;
        }

        // Simulate identity change notification: type="encrypt" with <identity/> child
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("id", "identity-change-1")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        // Should have dispatched IdentityChange event
        let events = collector.events();
        assert!(
            events
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "should dispatch IdentityChange event, got: {:?}",
            events
        );

        // Device registry cache should be invalidated
        assert!(
            client
                .device_registry_cache
                .get("5511999999999")
                .await
                .is_none(),
            "device registry cache should be invalidated after identity change"
        );
    }

    #[tokio::test]
    async fn test_identity_change_ignores_self_primary() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Set our own JID so the self-check works
        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
            })
            .await;

        // Identity change FROM our own JID — should be ignored per WA Web's isMePrimary
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("id", "identity-change-self")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "self identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_identity_change_ignores_companion_device() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999:5@s.whatsapp.net")
            .attr("id", "identity-change-2")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "companion device identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_dispatches_implicit_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let sender: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        let events = collector.events();
        // The event is dispatched last (after clear_device_record +
        // invalidate_device_cache), so observing it proves the handler ran to
        // completion. invalidate_device_cache itself is covered by
        // test_invalidate_device_cache_uses_correct_jid_types.
        let ic = events
            .iter()
            .find_map(|e| match &**e {
                Event::IdentityChange(ic) => Some(ic.clone()),
                _ => None,
            })
            .expect("local detection should dispatch IdentityChange");
        assert!(
            ic.implicit,
            "locally-detected identity change must set implicit=true"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_skips_self() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
            })
            .await;

        let sender: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        assert!(
            collector.events().is_empty(),
            "self identity change must never clear our own record"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_skips_companion_device() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let sender: Jid = "5511777777777:5@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        assert!(
            collector.events().is_empty(),
            "companion device identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_identity_change_deletes_primary_session() {
        use wacore::libsignal::protocol::SessionRecord;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;

        let target_jid: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        let addr = target_jid.to_protocol_address();

        // Pre-populate a session for the primary device
        client
            .signal_cache
            .put_session(&addr, SessionRecord::new_fresh())
            .await;
        client.signal_cache.put_identity(&addr, &[0u8; 32]).await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-3")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let backend = client.persistence_manager.backend();
        let has_session = client
            .signal_cache
            .has_session(&addr, &*backend)
            .await
            .unwrap();
        assert!(!has_session, "primary session should be deleted");

        let has_identity = client
            .signal_cache
            .get_identity(&addr, &*backend)
            .await
            .unwrap();
        assert!(has_identity.is_none(), "identity key should be deleted");
    }

    #[tokio::test]
    async fn test_identity_change_rotates_status_sender_key() {
        use wacore::libsignal::store::sender_key_name::SenderKeyName;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;

        // Set our own JID so sender key deletion knows which namespaces to check
        let own_jid: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some(own_jid.clone());
            })
            .await;

        // Pre-populate a sender key for status@broadcast
        let sk_name =
            SenderKeyName::from_parts("status@broadcast", own_jid.to_protocol_address().as_str());
        let sk_record = wacore::libsignal::protocol::SenderKeyRecord::new_empty();
        client
            .signal_cache
            .put_sender_key(&sk_name, sk_record)
            .await;

        // Seed a stored identity for the changed user so the had-prior-identity gate
        // runs the reset (which rotates the status sender key).
        let changed: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        client
            .signal_cache
            .put_identity(&changed.to_protocol_address(), &[7u8; 32])
            .await;

        // Fire identity change for a different user
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-4")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let backend = client.persistence_manager.backend();
        let sk = client
            .signal_cache
            .get_sender_key(&sk_name, &*backend)
            .await
            .unwrap();
        assert!(
            sk.is_none(),
            "status@broadcast sender key should be deleted for forward secrecy"
        );
    }

    #[tokio::test]
    async fn test_identity_change_with_offline_attribute() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Prior identity present so the gate runs (the offline attr only defers the
        // eager session re-establishment, not the change notification).
        let changed: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        client
            .signal_cache
            .put_identity(&changed.to_protocol_address(), &[7u8; 32])
            .await;

        // Notification with offline attribute should still be processed
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-5")
            .attr("offline", "1")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "offline identity change should still dispatch event"
        );
    }

    /// With no prior identity for the peer (e.g. a group-only member we never had a
    /// session with), the had-prior-identity gate skips the heavy reset: no change
    /// notification and no session/identity deletion. Only the device-list cleanup
    /// runs. Mirrors WA Web `if (!isEmpty(loadIdentityKey(addr)))`.
    #[tokio::test]
    async fn test_identity_change_no_prior_identity_skips_reset() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let target: Jid = "5511666666666@s.whatsapp.net".parse().unwrap();
        let addr = target.to_protocol_address();
        // Seed a device-registry entry (with a companion device) so the always-on
        // cleanup has something to do, but deliberately do NOT seed an identity.
        client
            .device_registry_cache
            .raw_insert_for_tests(
                "5511666666666".into(),
                Arc::new(wacore::store::traits::DeviceListRecord {
                    user: "5511666666666".into(),
                    devices: vec![wacore::store::traits::DeviceInfo {
                        device_id: 1,
                        key_index: None,
                    }],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: Some(1),
                }),
            )
            .await;

        // Seed a companion-device (device 1) Signal session: clear_device_record
        // runs even on the no-prior path, so this must be deleted afterward. Keyed
        // the same way delete_sessions_for_devices builds the address.
        let mut companion = wacore_binary::Jid::new("5511666666666", wacore_binary::Server::Pn);
        companion.device = 1;
        let companion_addr = companion.to_protocol_address();
        client
            .signal_cache
            .put_session(
                &companion_addr,
                wacore::libsignal::protocol::SessionRecord::new_fresh(),
            )
            .await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511666666666@s.whatsapp.net")
            .attr("id", "identity-change-noprior")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        // No change notification for a peer we had no prior identity with.
        assert!(
            collector.events().is_empty(),
            "no-prior-identity push must not dispatch IdentityChange, got: {:?}",
            collector.events()
        );
        // But the always-on device-list cleanup still ran.
        assert!(
            client
                .device_registry_cache
                .get("5511666666666")
                .await
                .is_none(),
            "device registry cache should still be invalidated on the no-prior path"
        );
        // And no identity was created by an (skipped) eager re-establishment.
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "no-prior path must not establish an identity"
        );
        // The always-on clear_device_record must still delete companion sessions.
        assert!(
            !client
                .signal_cache
                .has_session(&companion_addr, backend.as_ref())
                .await
                .unwrap(),
            "companion-device session must be cleared even on the no-prior path"
        );
    }

    /// Regression: when a PN->LID mapping was learned offline (migration deferred),
    /// the identity is still under the PN address while resolve_encryption_jid points
    /// at the LID. The gate must check the original PN address too and still run the
    /// reset (delete the stale PN identity + dispatch the event), not false-negative.
    #[tokio::test]
    async fn test_identity_change_resets_unmigrated_pn_identity_under_lid_resolve() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let pn = "5511555555555";
        let lid = "100000000000055";
        // Offline learn: records the PN->LID mapping in cache but skips the Signal
        // migration, so resolve points at the LID while state stays under the PN.
        client
            .learn_lid_pn_mapping_fast(lid, pn, LearningSource::Other, true)
            .await;

        let pn_jid: Jid = "5511555555555@s.whatsapp.net".parse().unwrap();
        // Confirm the setup actually diverges (resolve -> LID), else the test is moot.
        let resolved = client.resolve_encryption_jid(&pn_jid).await;
        assert!(
            resolved.is_lid(),
            "test setup: resolve_encryption_jid should return the LID, got {resolved}"
        );

        // Seed the identity under the PN address (not the LID).
        let pn_addr = pn_jid.to_protocol_address();
        client.signal_cache.put_identity(&pn_addr, &[7u8; 32]).await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511555555555@s.whatsapp.net")
            .attr("id", "identity-change-pnlid")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "must dispatch IdentityChange when the identity is under the unmigrated PN address"
        );
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&pn_addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "the stale PN identity must be deleted by the reset"
        );
    }

    /// Regression: a stanza can carry a `lid` attr while the local PN->LID cache is
    /// cold, so resolve_encryption_jid falls back to PN. If the identity lives under
    /// the stanza LID, the gate must still find it (via the stanza-LID candidate) and
    /// run the reset rather than skip it.
    #[tokio::test]
    async fn test_identity_change_resets_identity_under_stanza_lid_with_cold_cache() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Cold cache: no PN->LID mapping, so resolve_encryption_jid(PN) returns PN.
        let pn_jid: Jid = "5511444444444@s.whatsapp.net".parse().unwrap();
        let resolved = client.resolve_encryption_jid(&pn_jid).await;
        assert!(
            !resolved.is_lid(),
            "test setup: cache must be cold (resolve -> PN), got {resolved}"
        );

        // The identity lives under the LID carried by the stanza, not the PN.
        let lid_jid: Jid = "100000000000066@lid".parse().unwrap();
        let lid_addr = lid_jid.to_protocol_address();
        client
            .signal_cache
            .put_identity(&lid_addr, &[7u8; 32])
            .await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511444444444@s.whatsapp.net")
            .attr("lid", "100000000000066@lid")
            .attr("id", "identity-change-stanzalid")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "must dispatch IdentityChange when the identity is under the stanza LID"
        );
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&lid_addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "the stale stanza-LID identity must be deleted by the reset"
        );
    }
}
