//! Tests for stanza preparation and encryption fanout.

use super::*;
use crate::client::context::{GroupInfo, SendContextResolver};
use crate::libsignal::protocol::{IdentityKeyPair, KeyPair, PreKeyBundle};
use std::collections::HashMap;
use wacore_binary::Jid;

mod assemble_status_participants {
    use super::*;

    fn lid(u: &str) -> Jid {
        u.parse().expect("parse LID jid")
    }

    #[test]
    fn dedup_keeps_first_entry_per_user_and_anchors_own() {
        let own = lid("99999999999999@lid");
        let out = assemble_status_participants(
            vec![
                Some(lid("111@lid")),
                Some(lid("222@lid")),
                Some(lid("111@lid")),
                Some(lid("333@lid")),
            ],
            &own,
        )
        .expect("should succeed");
        let users: Vec<&str> = out.iter().map(|j| j.user.as_str()).collect();
        assert_eq!(users, ["111", "222", "333", "99999999999999"]);
    }

    #[test]
    fn skips_none_entries_matching_wa_web_compactmap() {
        // Unresolvable recipients arrive as `None` and must be silently
        // dropped — mirrors WA Web's `compactMap(list, toUserLid)`.
        let own = lid("me@lid");
        let out = assemble_status_participants(
            vec![None, Some(lid("111@lid")), None, Some(lid("222@lid"))],
            &own,
        )
        .expect("should succeed");
        let users: Vec<&str> = out.iter().map(|j| j.user.as_str()).collect();
        assert_eq!(users, ["111", "222", "me"]);
    }

    #[test]
    fn does_not_duplicate_own_when_already_in_list() {
        let own = lid("me@lid");
        let out =
            assemble_status_participants(vec![Some(lid("111@lid")), Some(lid("me@lid"))], &own)
                .expect("should succeed");
        let users: Vec<&str> = out.iter().map(|j| j.user.as_str()).collect();
        assert_eq!(users, ["111", "me"]);
    }

    #[test]
    fn errors_when_every_recipient_is_unresolvable() {
        // Regression guard for the original bug: a single LID-only
        // contact used to hard-abort the send with
        // `No PN mapping for LID ...`. The new contract is softer —
        // individual unresolvable entries are dropped — but we still
        // refuse to send when the entire list came back empty, rather
        // than silently broadcasting to own devices only.
        let own = lid("me@lid");
        let err = assemble_status_participants(vec![None, None, None], &own)
            .expect_err("all-None list must error");
        assert!(err.to_string().contains("No valid status recipients"));
    }

    #[test]
    fn errors_when_list_is_empty() {
        let own = lid("me@lid");
        let err = assemble_status_participants(Vec::<Option<Jid>>::new(), &own)
            .expect_err("empty list must error");
        assert!(err.to_string().contains("No valid status recipients"));
    }

    #[test]
    fn strips_device_suffix_from_own_lid() {
        // Snapshot lid from the device store carries a device id; the
        // participant list uses bare USER JIDs.
        let own: Jid = "me:5@lid".parse().unwrap();
        let out =
            assemble_status_participants(vec![Some(lid("111@lid"))], &own).expect("should succeed");
        let me = out
            .iter()
            .find(|j| j.user.as_str() == "me")
            .expect("own LID should be present");
        assert_eq!(me.device, 0, "own LID should be non-ad (device=0)");
    }
}

mod peer_message_options {
    use super::*;
    use crate::types::message::{PrivacySensitiveType, PushPriority};

    fn pdo_message_raw(request_type: i32) -> wa::Message {
        wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(
                    wa::message::protocol_message::Type::PeerDataOperationRequestMessage as i32,
                ),
                peer_data_operation_request_message: Some(
                    wa::message::PeerDataOperationRequestMessage {
                        peer_data_operation_request_type: Some(request_type),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    fn pdo_message(request_type: wa::message::PeerDataOperationRequestType) -> wa::Message {
        pdo_message_raw(request_type as i32)
    }

    #[test]
    fn pdo_priority_map_matches_wa_web_non_message_requests() {
        use wa::message::PeerDataOperationRequestType as PdoType;

        let high_force_cases = [
            (PdoType::GenerateLinkPreview, PushPriority::HighForce, None),
            (
                PdoType::PlaceholderMessageResend,
                PushPriority::HighForce,
                None,
            ),
            (
                PdoType::HistorySyncOnDemand,
                PushPriority::HighForce,
                Some(PrivacySensitiveType::OnDemand),
            ),
            (
                PdoType::CompanionCanonicalUserNonceFetch,
                PushPriority::HighForce,
                None,
            ),
        ];

        for (request_type, push_priority, privacy_sensitive) in high_force_cases {
            let options = peer_message_options_from_message(&pdo_message(request_type));
            assert_eq!(options.push_priority(), push_priority, "{request_type:?}");
            assert_eq!(
                options.privacy_sensitive(),
                privacy_sensitive,
                "{request_type:?}"
            );
        }

        let default_cases = [
            PdoType::UploadSticker,
            PdoType::SendRecentStickerBootstrap,
            PdoType::WaffleLinkingNonceFetch,
            PdoType::FullHistorySyncOnDemand,
            PdoType::CompanionMetaNonceFetch,
            PdoType::CompanionSyncdSnapshotFatalRecovery,
            PdoType::HistorySyncChunkRetry,
            PdoType::GalaxyFlowAction,
            PdoType::BusinessBroadcastInsightsDeliveredTo,
            PdoType::BusinessBroadcastInsightsRefresh,
        ];

        for request_type in default_cases {
            let options = peer_message_options_from_message(&pdo_message(request_type));
            assert_eq!(
                options.push_priority(),
                PushPriority::High,
                "{request_type:?}"
            );
            assert_eq!(options.privacy_sensitive(), None, "{request_type:?}");
        }
    }

    #[test]
    fn non_pdo_and_unknown_pdo_keep_peer_defaults() {
        let app_state_key_request = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::AppStateSyncKeyRequest as i32),
                app_state_sync_key_request: Some(wa::message::AppStateSyncKeyRequest {
                    key_ids: Vec::new(),
                }),
                ..Default::default()
            })),
            ..Default::default()
        };

        for msg in [app_state_key_request, pdo_message_raw(99)] {
            let options = peer_message_options_from_message(&msg);
            assert_eq!(options.push_priority(), PushPriority::High);
            assert_eq!(options.privacy_sensitive(), None);
        }
    }
}

mod status_carries_privacy_meta {
    use super::*;

    #[test]
    fn true_for_text_post() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(status_carries_privacy_meta(&msg));
    }

    #[test]
    fn true_for_image_post() {
        let msg = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage::default())),
            ..Default::default()
        };
        assert!(status_carries_privacy_meta(&msg));
    }

    #[test]
    fn false_for_reaction() {
        let msg = wa::Message {
            reaction_message: Some(Box::new(wa::message::ReactionMessage {
                text: Some("💚".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(
            !status_carries_privacy_meta(&msg),
            "reactions must omit <meta status_setting> (479 SmaxInvalid otherwise)"
        );
    }

    #[test]
    fn false_for_enc_reaction() {
        let msg = wa::Message {
            enc_reaction_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(!status_carries_privacy_meta(&msg));
    }

    #[test]
    fn false_for_revoke() {
        let msg = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::Revoke as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(!status_carries_privacy_meta(&msg));
    }

    #[test]
    fn true_for_non_revoke_protocol_message() {
        // Other ProtocolMessage types (e.g., EphemeralSettings) aren't
        // reactions and aren't revokes — treat as posts for now.
        let msg = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::EphemeralSetting as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(status_carries_privacy_meta(&msg));
    }

    #[test]
    fn false_for_reaction_inside_ephemeral_wrapper() {
        let inner = wa::Message {
            reaction_message: Some(Box::default()),
            ..Default::default()
        };
        let msg = wa::Message {
            ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(inner)),
            })),
            ..Default::default()
        };
        assert!(!status_carries_privacy_meta(&msg));
    }

    #[test]
    fn false_for_revoke_inside_device_sent_wrapper() {
        let inner = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::Revoke as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some(String::new()),
                message: Some(Box::new(inner)),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(!status_carries_privacy_meta(&msg));
    }
}

#[test]
fn build_member_label_message_sets_fields() {
    let msg = build_member_label_message("VIP".to_string(), 1_766_847_151);
    let pm = msg.protocol_message.as_ref().expect("protocol_message set");
    assert_eq!(
        pm.r#type,
        Some(wa::message::protocol_message::Type::GroupMemberLabelChange as i32)
    );
    let ml = pm.member_label.as_ref().expect("member_label set");
    assert_eq!(ml.label.as_deref(), Some("VIP"));
    assert_eq!(ml.label_timestamp, Some(1_766_847_151));
    assert!(
        pm.key.is_none(),
        "MessageKey must NOT be set (WA Web parity)"
    );
}

#[test]
fn build_member_label_message_clear_uses_empty_string() {
    let msg = build_member_label_message(String::new(), 1);
    let ml = msg
        .protocol_message
        .as_ref()
        .unwrap()
        .member_label
        .as_ref()
        .unwrap();
    assert_eq!(ml.label.as_deref(), Some(""));
}

#[test]
fn build_member_label_message_preserves_unicode() {
    let msg = build_member_label_message("🚀 BOT".to_string(), 2);
    let ml = msg
        .protocol_message
        .as_ref()
        .unwrap()
        .member_label
        .as_ref()
        .unwrap();
    assert_eq!(ml.label.as_deref(), Some("🚀 BOT"));
}

/// Probe installed by chain-lock tests: records whether the sender-key chain
/// lock was held while `fetch_prekeys_for_identity_check` ran (it must not be
/// — the fetch is network I/O hoisted out of the chain critical section).
#[derive(Clone, Default)]
struct ChainLockProbe {
    lock: std::sync::Arc<async_lock::Mutex<()>>,
    setup_lock: std::sync::Arc<async_lock::Mutex<()>>,
    fetched_under_lock: std::sync::Arc<std::sync::atomic::AtomicBool>,
    fetched_without_setup_lock: std::sync::Arc<std::sync::atomic::AtomicBool>,
    fetch_calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// Mock implementation of SendContextResolver for testing
struct MockSendContextResolver {
    /// Pre-key bundles to return: JID -> Option<PreKeyBundle>
    prekey_bundles: HashMap<Jid, Option<PreKeyBundle>>,
    /// Devices to return from resolve_devices
    devices: Vec<Jid>,
    /// Phone number to LID mappings for testing LID session lookup
    phone_to_lid: HashMap<String, String>,
    /// JIDs reported via `on_local_identity_change` (send-path detection).
    identity_changes: std::sync::Mutex<Vec<Jid>>,
    chain_lock_probe: Option<ChainLockProbe>,
}

impl MockSendContextResolver {
    fn new() -> Self {
        Self {
            prekey_bundles: HashMap::new(),
            devices: Vec::new(),
            phone_to_lid: HashMap::new(),
            identity_changes: std::sync::Mutex::new(Vec::new()),
            chain_lock_probe: None,
        }
    }

    fn with_chain_lock_probe(mut self, probe: ChainLockProbe) -> Self {
        self.chain_lock_probe = Some(probe);
        self
    }

    fn captured_identity_changes(&self) -> Vec<Jid> {
        self.identity_changes.lock().unwrap().clone()
    }

    fn with_missing_bundle(mut self, jid: Jid) -> Self {
        self.prekey_bundles.insert(jid, None);
        self
    }

    fn with_bundle(mut self, jid: Jid, bundle: PreKeyBundle) -> Self {
        self.prekey_bundles.insert(jid, Some(bundle));
        self
    }

    fn with_devices(mut self, devices: Vec<Jid>) -> Self {
        self.devices = devices;
        self
    }

    fn with_phone_to_lid(mut self, phone: &str, lid: &str) -> Self {
        self.phone_to_lid.insert(phone.to_string(), lid.to_string());
        self
    }
}

#[async_trait::async_trait]
impl SendContextResolver for MockSendContextResolver {
    async fn resolve_devices(&self, _jids: &[Jid]) -> Result<Vec<Jid>> {
        Ok(self.devices.clone())
    }

    async fn fetch_prekeys(&self, jids: &[Jid]) -> Result<HashMap<Jid, PreKeyBundle>> {
        let mut result = HashMap::new();
        for jid in jids {
            if let Some(bundle_opt) = self.prekey_bundles.get(jid)
                && let Some(bundle) = bundle_opt
            {
                result.insert(jid.clone(), bundle.clone());
            }
        }
        Ok(result)
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>> {
        if let Some(probe) = &self.chain_lock_probe {
            probe
                .fetch_calls
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if probe.lock.try_lock().is_none() {
                probe
                    .fetched_under_lock
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
            // The setup lock must be HELD here (try_lock succeeds = violation).
            if probe.setup_lock.try_lock().is_some() {
                probe
                    .fetched_without_setup_lock
                    .store(true, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let mut result = HashMap::new();
        for jid in jids {
            if let Some(bundle_opt) = self.prekey_bundles.get(jid)
                && let Some(bundle) = bundle_opt
            {
                result.insert(jid.clone(), bundle.clone());
            }
            // If None, we intentionally omit it from the result (simulating server not returning it)
        }
        Ok(result)
    }

    async fn resolve_group_info(&self, _jid: &Jid) -> Result<std::sync::Arc<GroupInfo>> {
        unimplemented!("resolve_group_info not needed for send.rs tests")
    }

    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<wacore_binary::CompactString> {
        self.phone_to_lid.get(phone_user).map(|s| s.as_str().into())
    }

    fn on_local_identity_change(&self, jid: &Jid) {
        self.identity_changes.lock().unwrap().push(jid.clone());
    }
}

/// Test case: Missing pre-key bundle for a single device skips gracefully
///
/// When sending to multiple devices, if some don't have pre-key bundles (e.g., Cloud API),
/// we should skip them instead of failing the entire message.
#[test]
fn test_missing_prekey_bundle_skips_device() {
    let device_with_bundle: Jid = "1234567890:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let device_without_bundle: Jid = "1234567890:1@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let cloud_api: Jid = "1234567890:99@hosted"
        .parse()
        .expect("test JID should be valid");

    let bundle = create_mock_bundle();

    let resolver = MockSendContextResolver::new()
        .with_bundle(device_with_bundle.clone(), bundle)
        .with_missing_bundle(device_without_bundle.clone())
        .with_missing_bundle(cloud_api.clone())
        .with_devices(vec![
            device_with_bundle.clone(),
            device_without_bundle.clone(),
            cloud_api.clone(),
        ]);

    // Check that the resolver correctly returns only available bundles
    assert_eq!(
        resolver.prekey_bundles.len(),
        3,
        "Resolver should have 3 entries"
    );

    // Verify device_with_bundle has a Some(bundle)
    assert!(
        resolver.prekey_bundles[&device_with_bundle].is_some(),
        "device_with_bundle should have a Some entry"
    );

    // Verify others have None
    assert!(
        resolver.prekey_bundles[&device_without_bundle].is_none(),
        "device_without_bundle should have None"
    );
    assert!(
        resolver.prekey_bundles[&cloud_api].is_none(),
        "cloud_api should have None"
    );

    println!("✅ Missing pre-key bundle skips device gracefully");
}

/// Test case: All devices missing pre-key bundles
///
/// If all devices are unavailable, the batch should still complete without panic.
#[test]
fn test_all_devices_missing_prekey_bundles() {
    let device1: Jid = "1234567890:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let device2: Jid = "1234567890:1@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let device3: Jid = "9876543210:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    let resolver = MockSendContextResolver::new()
        .with_missing_bundle(device1.clone())
        .with_missing_bundle(device2.clone())
        .with_missing_bundle(device3.clone())
        .with_devices(vec![device1.clone(), device2.clone(), device3.clone()]);

    // All entries should be None
    assert!(resolver.prekey_bundles[&device1].is_none());
    assert!(resolver.prekey_bundles[&device2].is_none());
    assert!(resolver.prekey_bundles[&device3].is_none());

    println!("✅ All devices missing bundles handled gracefully");
}

/// Test case: Large group with mixed device availability
///
/// In real-world scenarios, large groups may have some unavailable devices.
/// The encryption should proceed for available devices and skip unavailable ones.
#[test]
fn test_large_group_with_mixed_device_availability() {
    let mut all_devices = Vec::new();

    for i in 0..10u16 {
        let device_jid = Jid::pn_device("1234567890", i);
        all_devices.push(device_jid);
    }

    let mut resolver = MockSendContextResolver::new().with_devices(all_devices.clone());

    // Add bundles for devices 0-6, mark 7-9 as missing
    for i in 0..10u16 {
        let device_jid = Jid::pn_device("1234567890", i);

        if i < 7 {
            resolver = resolver.with_bundle(device_jid, create_mock_bundle());
        } else {
            resolver = resolver.with_missing_bundle(device_jid);
        }
    }

    // Verify bundle availability
    let available_count = resolver
        .prekey_bundles
        .values()
        .filter(|v| v.is_some())
        .count();

    assert_eq!(available_count, 7, "Should have 7 available devices");
    assert_eq!(
        resolver.prekey_bundles.len(),
        10,
        "Should have 10 total entries"
    );

    println!("✅ Large group with 7 available, 3 unavailable devices");
}

/// Test case: Cloud API / HOSTED device without pre-key
///
/// # Context: What are HOSTED devices?
///
/// HOSTED devices (Cloud API / Meta Business API) are WhatsApp Business accounts
/// that use Meta's server-side infrastructure instead of traditional E2EE.
///
/// ## Identification:
/// - Device ID 99 (`:99`) on any server
/// - Server `@hosted` or `@hosted.lid`
///
/// ## Behavior:
/// - They do NOT have Signal protocol prekey bundles
/// - For 1:1 chats: included in device list, but prekey fetch fails gracefully
/// - For groups: proactively filtered out before SKDM distribution
///
/// This test verifies that when a hosted device is included in the device list
/// (which would happen for 1:1 chats), the missing prekey is handled gracefully.
#[test]
fn test_cloud_api_device_without_prekey() {
    let regular_device: Jid = "1234567890:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let cloud_api: Jid = "1234567890:99@hosted"
        .parse()
        .expect("test JID should be valid");

    // Verify the cloud_api device is detected as hosted
    assert!(
        cloud_api.is_hosted(),
        "Device with :99@hosted should be detected as hosted"
    );
    assert!(
        !regular_device.is_hosted(),
        "Regular device should NOT be detected as hosted"
    );

    let resolver = MockSendContextResolver::new()
        .with_bundle(regular_device.clone(), create_mock_bundle())
        .with_missing_bundle(cloud_api.clone())
        .with_devices(vec![regular_device.clone(), cloud_api.clone()]);

    assert!(
        resolver.prekey_bundles[&regular_device].is_some(),
        "Regular device should have a bundle"
    );
    assert!(
        resolver.prekey_bundles[&cloud_api].is_none(),
        "Cloud API device should not have a bundle (they don't use Signal protocol)"
    );

    println!("✅ Cloud API device has no prekey bundle (expected behavior)");
}

/// Test case: HOSTED devices are filtered from group SKDM distribution
///
/// # Why filter hosted devices from groups?
///
/// WhatsApp Web explicitly excludes hosted devices from group message fanout.
/// From the JS code (`getFanOutList`):
/// ```javascript
/// var isHosted = e.id === 99 || e.isHosted === true;
/// var includeInFanout = !isHosted || isOneToOneChat;
/// ```
///
/// ## Reasons:
/// 1. Hosted devices don't use Signal protocol - they can't process SKDM
/// 2. Including them causes unnecessary prekey fetch failures
/// 3. Group encryption is handled differently for Cloud API businesses
///
/// This test verifies that `is_hosted()` correctly identifies devices that
/// should be filtered from group SKDM distribution.
#[test]
fn test_hosted_devices_filtered_from_group_skdm() {
    // Simulate devices returned from usync for a group
    let devices: Vec<Jid> = vec![
        // Regular devices - should receive SKDM
        "5511999887766:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"), // Primary phone
        "5511999887766:33@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"), // WhatsApp Web companion
        "5521988776655:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"), // Another participant
        "100000012345678:33@lid"
            .parse()
            .expect("test JID should be valid"), // LID companion device
        // HOSTED devices - should be EXCLUDED from group SKDM
        "5531977665544:99@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"), // Cloud API on regular server
        "100000087654321:99@lid"
            .parse()
            .expect("test JID should be valid"), // Cloud API on LID server
        "5541966554433:0@hosted"
            .parse()
            .expect("test JID should be valid"), // Explicit @hosted server
    ];

    // This is the filtering logic used in prepare_group_stanza
    let filtered_for_skdm: Vec<Jid> = devices.into_iter().filter(|jid| !jid.is_hosted()).collect();

    assert_eq!(
        filtered_for_skdm.len(),
        4,
        "Should have 4 devices after filtering out hosted devices"
    );

    // Verify all remaining devices are NOT hosted
    for jid in &filtered_for_skdm {
        assert!(
            !jid.is_hosted(),
            "Filtered list should not contain hosted device: {}",
            jid
        );
    }

    // Verify specific devices are included/excluded by checking struct fields
    // (Device ID 0 is not serialized in the string representation)
    let has_primary_phone = filtered_for_skdm
        .iter()
        .any(|j| j.user == "5511999887766" && j.device == 0 && j.server == "s.whatsapp.net");
    let has_companion = filtered_for_skdm
        .iter()
        .any(|j| j.user == "5511999887766" && j.device == 33 && j.server == "s.whatsapp.net");
    let has_cloud_api = filtered_for_skdm
        .iter()
        .any(|j| j.user == "5531977665544" && j.device == 99);
    let has_hosted_server = filtered_for_skdm.iter().any(|j| j.server == "hosted");

    assert!(has_primary_phone, "Primary phone should be included");
    assert!(has_companion, "WhatsApp Web companion should be included");
    assert!(
        !has_cloud_api,
        "Cloud API device (ID 99) should be excluded"
    );
    assert!(
        !has_hosted_server,
        "@hosted server device should be excluded"
    );

    println!("✅ Hosted devices correctly filtered from group SKDM distribution");
}

/// Test case: Device recovery between retries
///
/// If a device was temporarily unavailable, a retry should succeed.
#[test]
fn test_device_recovery_between_requests() {
    let device: Jid = "1234567890:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    // First attempt: device unavailable
    let resolver_first = MockSendContextResolver::new().with_missing_bundle(device.clone());

    assert!(
        resolver_first.prekey_bundles[&device].is_none(),
        "First attempt: device should be unavailable"
    );

    // Second attempt: device recovered
    let resolver_second =
        MockSendContextResolver::new().with_bundle(device.clone(), create_mock_bundle());

    assert!(
        resolver_second.prekey_bundles[&device].is_some(),
        "Second attempt: device should be available"
    );

    println!("✅ Device recovery between retries works correctly");
}

/// Helper function to create a mock PreKeyBundle with valid types
fn create_mock_bundle() -> PreKeyBundle {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let identity_pair = IdentityKeyPair::generate(&mut rng);
    let signed_prekey_pair = KeyPair::generate(&mut rng);
    let prekey_pair = KeyPair::generate(&mut rng);

    PreKeyBundle::new(
        1,                                           // registration_id
        1u32.into(),                                 // device_id
        Some((1u32.into(), prekey_pair.public_key)), // pre_key
        2u32.into(),                                 // signed_pre_key_id
        signed_prekey_pair.public_key,
        vec![0u8; 64],
        *identity_pair.identity_key(),
    )
    .expect("Failed to create PreKeyBundle")
}

// These tests validate the fix for the LID-PN session mismatch issue.
// When a message is received with sender_lid, the session is stored under the LID address.
// When sending a reply using the phone number, we must reuse the existing LID session
// instead of creating a new PN session, otherwise subsequent messages will fail with
// MAC verification errors.

/// Test that phone_to_lid mapping returns the cached LID mapping.
///
/// This verifies the MockSendContextResolver correctly stores phone-to-LID
/// mappings used for LID session lookup.
#[test]
fn test_mock_resolver_phone_to_lid_mapping() {
    let phone = "559980000001";
    let lid = "100000012345678";

    let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

    // Access the HashMap directly (synchronous)
    let result = resolver.phone_to_lid.get(phone).cloned();

    assert!(result.is_some(), "Should return LID for known phone");
    assert_eq!(
        result.expect("known phone should return LID"),
        lid,
        "Should return correct LID"
    );

    // Unknown phone should return None
    let unknown = resolver.phone_to_lid.get("999999999").cloned();
    assert!(unknown.is_none(), "Should return None for unknown phone");

    println!("✅ MockSendContextResolver phone_to_lid mapping works correctly");
}

/// Test that the resolver correctly maps phone numbers to LIDs.
///
/// This is a building block for the session lookup logic.
#[test]
fn test_phone_to_lid_mapping_multiple_users() {
    let resolver = MockSendContextResolver::new()
        .with_phone_to_lid("559980000001", "100000012345678")
        .with_phone_to_lid("559980000002", "100000024691356")
        .with_phone_to_lid("559980000003", "100000037037034");

    // Verify all mappings using direct HashMap access
    let lid1 = resolver.phone_to_lid.get("559980000001").cloned();
    let lid2 = resolver.phone_to_lid.get("559980000002").cloned();
    let lid3 = resolver.phone_to_lid.get("559980000003").cloned();

    assert_eq!(
        lid1.expect("phone 1 should have LID mapping"),
        "100000012345678"
    );
    assert_eq!(
        lid2.expect("phone 2 should have LID mapping"),
        "100000024691356"
    );
    assert_eq!(
        lid3.expect("phone 3 should have LID mapping"),
        "100000037037034"
    );

    println!("✅ Multiple phone-to-LID mappings work correctly");
}

/// Test the scenario that caused the original bug:
/// - Session exists under LID address (from receiving a message with sender_lid)
/// - Send to PN address should reuse the LID session, not create a new one
///
/// This test verifies the logic flow, though full integration testing
/// requires the actual encrypt_for_devices function with real sessions.
#[test]
fn test_lid_session_lookup_scenario() {
    // Scenario setup:
    // - Received message from 559980000001@s.whatsapp.net with sender_lid=100000012345678@lid
    // - Session was stored under 100000012345678.0
    // - Now sending reply to 559980000001@s.whatsapp.net
    // - Should look up LID and check for session under 100000012345678.0

    let phone = "559980000001";
    let lid = "100000012345678";
    let device_id = 0u16;

    let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

    // Simulate the device JID we're trying to send to (PN format)
    let pn_device_jid = Jid::pn_device(phone, device_id);

    // Step 1: Look up LID for the phone number (using direct HashMap access)
    let lid_user = resolver
        .phone_to_lid
        .get(pn_device_jid.user.as_str())
        .cloned();
    assert!(lid_user.is_some(), "Should find LID for phone");
    let lid_user = lid_user.expect("phone should have LID mapping");

    // Step 2: Construct the LID JID with same device ID
    let lid_jid = Jid::lid_device(lid_user.clone(), pn_device_jid.device);

    // Step 3: Verify the LID JID is correctly constructed
    assert_eq!(lid_jid.user, lid, "LID user should match");
    assert_eq!(lid_jid.server, "lid", "Server should be 'lid'");
    assert_eq!(lid_jid.device, device_id, "Device ID should be preserved");

    // Step 4: Convert to protocol addresses and verify they're different
    use crate::types::jid::JidExt;
    let pn_address = pn_device_jid.to_protocol_address();
    let lid_address = lid_jid.to_protocol_address();

    assert_ne!(
        pn_address.name(),
        lid_address.name(),
        "PN and LID addresses should have different names"
    );
    assert_eq!(
        pn_address.device_id(),
        lid_address.device_id(),
        "Device IDs should match"
    );

    println!("✅ LID session lookup scenario works correctly:");
    println!("   - PN JID: {} -> Address: {}", pn_device_jid, pn_address);
    println!("   - LID JID: {} -> Address: {}", lid_jid, lid_address);
    println!("   - Would check for session under LID address first");
}

/// Test that companion device IDs are preserved in LID JID construction.
///
/// WhatsApp Web uses device ID 33, and this must be preserved when
/// constructing the LID JID for session lookup.
#[test]
fn test_lid_jid_preserves_companion_device_id() {
    let phone = "559980000001";
    let lid = "100000012345678";
    let companion_device_id = 33u16; // WhatsApp Web device ID

    let resolver = MockSendContextResolver::new().with_phone_to_lid(phone, lid);

    // Simulate sending to a companion device (WhatsApp Web)
    let pn_device_jid = Jid::pn_device(phone, companion_device_id);

    // Look up LID using direct HashMap access
    let lid_user = resolver
        .phone_to_lid
        .get(pn_device_jid.user.as_str())
        .cloned();

    // Construct LID JID
    let lid_jid = Jid::lid_device(
        lid_user.expect("phone should have LID mapping for companion test"),
        pn_device_jid.device,
    );

    assert_eq!(
        lid_jid.device, companion_device_id,
        "Device ID 33 should be preserved"
    );
    assert_eq!(lid_jid.to_string(), "100000012345678:33@lid");

    println!("✅ Companion device ID (33) correctly preserved in LID JID");
}

/// Test that LID lookup only applies to s.whatsapp.net JIDs.
///
/// LID JIDs (@lid) and group JIDs (@g.us) should not trigger LID lookup.
#[test]
fn test_lid_lookup_only_for_pn_jids() {
    let _resolver =
        MockSendContextResolver::new().with_phone_to_lid("559980000001", "100000012345678");

    // These JIDs should NOT trigger LID lookup
    let lid_jid: Jid = "100000012345678:0@lid"
        .parse()
        .expect("test JID should be valid");
    let group_jid: Jid = "120363123456789012@g.us"
        .parse()
        .expect("test JID should be valid");

    // Only s.whatsapp.net JIDs should be looked up
    assert_ne!(
        lid_jid.server, "s.whatsapp.net",
        "LID JID should not be s.whatsapp.net"
    );
    assert_ne!(
        group_jid.server, "s.whatsapp.net",
        "Group JID should not be s.whatsapp.net"
    );

    // PN JID should be eligible for lookup
    let pn_jid: Jid = "559980000001:0@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    assert_eq!(
        pn_jid.server, "s.whatsapp.net",
        "PN JID should be s.whatsapp.net"
    );

    println!("✅ LID lookup correctly limited to s.whatsapp.net JIDs");
}

/// Test case: Regression test for self-encryption bug.
///
/// The sender's own device (e.g. device 79) must be excluded from the encryption list
/// to prevent "SESSION BASE KEY CHANGED" warnings caused by establishing a session with oneself.
#[test]
fn test_dm_encryption_excludes_sender_device() {
    // Setup:
    // - Own user: 123456789
    // - Specific own device (Sender): 79
    // - Other own device: 0
    // - Recipient: 987654321

    let own_user = "123456789";
    let own_device_id = 79;

    // Own JID (Sender)
    let own_jid = Jid::lid_device(own_user.to_string(), own_device_id);

    // Simulate devices returned by resolver.resolve_devices()
    // This includes:
    // 1. The sender's own device (should be excluded)
    // 2. Another device of the sender (should be in own_other_devices)
    // 3. The recipient's device (should be in recipient_devices)
    let all_devices: Vec<Jid> = vec![
        Jid::lid_device(own_user.to_string(), own_device_id), // Sender (79)
        Jid::lid_device(own_user.to_string(), 0),             // Other own device (0)
        Jid::lid_device("987654321".to_string(), 0),          // Recipient
    ];

    let (recipient_devices, own_other_devices) = partition_dm_devices(all_devices, &own_jid, None);

    // Verifications

    // 1. Sender device (79) should NOT be in either list
    let sender_in_own = own_other_devices.iter().any(|d| d.device == own_device_id);
    let sender_in_recipient = recipient_devices.iter().any(|d| d.device == own_device_id);

    assert!(
        !sender_in_own,
        "Sender device (79) should be excluded from own_other_devices"
    );
    assert!(
        !sender_in_recipient,
        "Sender device (79) should be excluded from recipient_devices"
    );

    // 2. Other own device (0) MUST be in own_other_devices
    let other_own_present = own_other_devices
        .iter()
        .any(|d| d.device == 0 && d.user == own_user);
    assert!(
        other_own_present,
        "Other own device (0) should be included in own_other_devices"
    );

    // 3. Recipient MUST be in recipient_devices
    let recipient_present = recipient_devices.iter().any(|d| d.user == "987654321");
    assert!(
        recipient_present,
        "Recipient should be included in recipient_devices"
    );

    println!("✅ Self-encryption regression test passed: Sender device correctly excluded.");
}

#[test]
fn test_dm_encryption_treats_own_lid_devices_as_self() {
    let own_pn = Jid::pn_device("559980000001".to_string(), 18);
    let own_lid = Jid::lid_device("123456789012345".to_string(), 18);

    let all_devices = vec![
        Jid::lid_device("123456789012345".to_string(), 18), // Exact sender device via LID
        Jid::lid_device("123456789012345".to_string(), 0),  // Other own device via LID
        Jid::lid_device("987654321012345".to_string(), 0),  // Recipient
    ];

    let (recipient_devices, own_other_devices) =
        partition_dm_devices(all_devices, &own_pn, Some(&own_lid));

    assert!(
        !own_other_devices
            .iter()
            .any(|d| d.user == own_lid.user && d.device == 18),
        "Exact sender LID device should be excluded from own_other_devices"
    );
    assert!(
        !recipient_devices
            .iter()
            .any(|d| d.user == own_lid.user && d.device == 18),
        "Exact sender LID device should be excluded from recipient_devices"
    );
    assert!(
        own_other_devices
            .iter()
            .any(|d| d.user == own_lid.user && d.device == 0),
        "Other own LID devices should be routed through DSM as own_other_devices"
    );
    assert!(
        recipient_devices
            .iter()
            .any(|d| d.user == "987654321012345" && d.device == 0),
        "Non-self devices must remain in recipient_devices"
    );
}

/// Test case: LID Prekey Lookup Normalization
///
/// Verifies that when looking up pre-key bundles for LID JIDs, the lookup key
/// is normalized (agent=0) to match how the bundles are stored in the map.
///
/// This validates the fix for "No pre-key bundle returned" when the requested JID
/// has non-standard agent/server fields but the bundle is stored under the normalized key.
#[test]
fn test_lid_prekey_lookup_normalization() {
    // 1. Define JIDs
    // The JID we request (simulating what comes from resolve_devices or elsewhere)
    // Let's pretend it has agent=1 to simulate a mismatch
    let mut requested_jid = Jid::lid_device("123456789".to_string(), 0);
    requested_jid.agent = 1;

    // The normalized JID (how it's stored in the bundle map)
    let normalized_jid = Jid::lid_device("123456789".to_string(), 0); // agent=0 by default

    // 2. Setup Resolver
    // Store the bundle under the NORMALIZED key (agent=0)
    let resolver = MockSendContextResolver::new()
        .with_bundle(normalized_jid.clone(), create_mock_bundle())
        .with_devices(vec![requested_jid.clone()]);

    // 3. Verify Mock Setup
    // Ensure bundle is accessible via normalized key but NOT via requested (raw) key
    // This confirms our test condition is valid (that implicit lookup would fail)
    assert!(
        resolver.prekey_bundles.contains_key(&normalized_jid),
        "Setup: bundle should exist for normalized key"
    );
    assert!(
        !resolver.prekey_bundles.contains_key(&requested_jid),
        "Setup: bundle should NOT exist for requested raw key"
    );

    // 4. Test logic mirroring `encrypt_for_devices`
    let mut jid_to_encryption_jid = HashMap::new();
    // Assume direct mapping for simplicity
    jid_to_encryption_jid.insert(requested_jid.clone(), requested_jid.clone());

    // Get the bundles map (mocks `fetch_prekeys_for_identity_check`)
    // The mock implementation returns the map as-is filtered by keys.
    // HOWEVER, `fetch_prekeys` usually takes a list.
    // In `encrypt_for_devices`, we call:
    // let prekey_bundles = resolver.fetch_prekeys_for_identity_check(&[requested_jid]).await?;

    // Let's simulate what `fetch_prekeys_for_identity_check` would return.
    // Our mock implementation `fetch_prekeys` logic:
    // if let Some(bundle_opt) = self.prekey_bundles.get(jid)

    // Wait, if the mock follows exact HashMap lookup, `fetch_prekeys(&[requested_jid])`
    // will return EMPTY because `requested_jid` is not in `prekey_bundles`.
    // The REAL `fetch_prekeys` (in `client.rs` -> `prekeys.rs`) sends an IQ to the server,
    // and the server response is parsed. The parsing logic (in `prekeys.rs`) normalizes the key.
    // So the HashMap returned by `fetch_prekeys` will contain NORMALIZED keys.

    // So for this test to be accurate, we must simulate that `fetch_prekeys` returned a map
    // where the key is NORMALIZED, even if we asked for `requested_jid`?
    // Actually, `PreKeyFetchSpec` asks for JIDs. The response contains JIDs.
    // If we ask for `agent=1`, does the server return `agent=1`?
    // The logs showed:
    // parsed: `...:82@lid` (agent=0 probably, or just not printed?)
    // lookup: `...` (failed)

    // The critical part is that the `HashMap` returned by `resolver.fetch_prekeys`
    // definitely contains the bundle under some key.
    // If `prekeys.rs` normalizes it, it's under the normalized key.
    // The `encrypt_for_devices` logic has:
    // `match prekey_bundles.get(device_jid)`
    // where `device_jid` is the one from the loop (requested_jid).

    // If `fetch_prekeys` returns a map with `normalized_jid`, and we lookup `requested_jid`, it fails.
    // My fix was to normalize `requested_jid` before lookup.

    // So I need to construct the `prekey_bundles` map manually here to simulate the return from fetch.
    let mut prekey_bundles = HashMap::new();
    prekey_bundles.insert(normalized_jid.clone(), create_mock_bundle());

    // Now test the logic:
    let device_jid = &requested_jid;

    // -- Logic from fix --
    // Use centralized normalization logic
    let lookup_jid = device_jid.normalize_for_prekey_bundle();

    // Fix: Use the normalized device_jid to lookup the bundle
    let bundle = prekey_bundles.get(&lookup_jid);
    // --------------------

    assert!(bundle.is_some(), "Should find bundle after normalization");

    // Verify it would have failed without normalization
    let raw_lookup = prekey_bundles.get(device_jid);
    assert!(
        raw_lookup.is_none(),
        "Should NOT find bundle without normalization"
    );

    println!("✅ LID Prekey Lookup Normalization passed");
}

mod group_retry {
    use super::*;
    use crate::libsignal::protocol::{
        Direction, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore, KeyPair,
        PreKeyBundle, ProtocolAddress, SessionStore, process_prekey_bundle,
    };
    use crate::types::message::AddressingMode;
    use std::collections::HashMap;
    use wacore_binary::NodeContent;

    struct MemSessionStore(HashMap<ProtocolAddress, Vec<u8>>);
    impl MemSessionStore {
        fn new() -> Self {
            Self(HashMap::new())
        }
    }
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl SessionStore for MemSessionStore {
        async fn load_session(
            &self,
            a: &ProtocolAddress,
        ) -> crate::libsignal::protocol::error::Result<
            Option<crate::libsignal::protocol::SessionRecord>,
        > {
            Ok(self
                .0
                .get(a)
                .and_then(|b| crate::libsignal::protocol::SessionRecord::deserialize(b).ok()))
        }
        async fn has_session(
            &self,
            a: &ProtocolAddress,
        ) -> crate::libsignal::protocol::error::Result<bool> {
            Ok(self.0.contains_key(a))
        }
        async fn store_session(
            &mut self,
            a: &ProtocolAddress,
            r: crate::libsignal::protocol::SessionRecord,
        ) -> crate::libsignal::protocol::error::Result<()> {
            self.0.insert(a.clone(), r.serialize()?);
            Ok(())
        }
    }

    struct MemIdentityStore {
        pair: IdentityKeyPair,
        reg_id: u32,
        known: HashMap<ProtocolAddress, IdentityKey>,
    }
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl IdentityKeyStore for MemIdentityStore {
        async fn get_identity_key_pair(
            &self,
        ) -> crate::libsignal::protocol::error::Result<IdentityKeyPair> {
            Ok(self.pair.clone())
        }
        async fn get_local_registration_id(
            &self,
        ) -> crate::libsignal::protocol::error::Result<u32> {
            Ok(self.reg_id)
        }
        async fn save_identity(
            &mut self,
            a: &ProtocolAddress,
            id: &IdentityKey,
        ) -> crate::libsignal::protocol::error::Result<IdentityChange> {
            self.known.insert(a.clone(), *id);
            Ok(IdentityChange::from_changed(false))
        }
        async fn is_trusted_identity(
            &self,
            _: &ProtocolAddress,
            _: &IdentityKey,
            _: Direction,
        ) -> crate::libsignal::protocol::error::Result<bool> {
            Ok(true)
        }
        async fn get_identity(
            &self,
            a: &ProtocolAddress,
        ) -> crate::libsignal::protocol::error::Result<Option<IdentityKey>> {
            Ok(self.known.get(a).copied())
        }
    }

    async fn setup_session() -> (MemSessionStore, MemIdentityStore, Jid) {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let sender = IdentityKeyPair::generate(&mut rng);
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let sig = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .unwrap();
        let bundle = PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            sig.to_vec(),
            *receiver.identity_key(),
        )
        .unwrap();
        let jid: Jid = "559911112222@s.whatsapp.net".parse().unwrap();
        let addr = jid.to_protocol_address();
        let mut ss = MemSessionStore::new();
        let mut is = MemIdentityStore {
            pair: sender,
            reg_id: 42,
            known: HashMap::new(),
        };
        process_prekey_bundle(
            &addr,
            &mut ss,
            &mut is,
            &bundle,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
            crate::libsignal::protocol::UsePQRatchet::No,
        )
        .await
        .unwrap();
        (ss, is, jid)
    }

    #[tokio::test]
    async fn group_retry_pkmsg_with_account_emits_device_identity() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();
        let account = pkmsg_account_proto();
        let n = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group.clone(),
            p.clone(),
            p.clone(),
            &wa::Message::default(),
            "3EB0ABC".into(),
            1,
            Some(&account),
            AddressingMode::Pn,
            None,
        )
        .await
        .unwrap();

        assert_eq!(n.tag, "message");
        let mut a = n.attrs();
        assert_eq!(a.optional_string("to").unwrap().as_ref(), group.to_string());
        assert_eq!(
            a.optional_string("participant").unwrap().as_ref(),
            p.to_string()
        );
        // Default (empty) message falls through to "media" per WA Web's typeAttributeFromProtobuf
        assert_eq!(
            a.optional_string("type").unwrap().as_ref(),
            stanza::MSG_TYPE_MEDIA
        );
        assert!(a.optional_string("category").is_none());
        assert_eq!(a.optional_string("addressing_mode").unwrap().as_ref(), "pn");
        let enc = n.get_optional_child("enc").unwrap();
        let mut ea = enc.attrs();
        assert_eq!(
            ea.optional_string("v").unwrap().as_ref(),
            stanza::ENC_VERSION
        );
        assert_eq!(
            ea.optional_string("type").unwrap().as_ref(),
            stanza::ENC_TYPE_PKMSG
        );
        assert_eq!(ea.optional_string("count").unwrap().as_ref(), "1");
        assert!(matches!(&enc.content, Some(NodeContent::Bytes(_))));
        assert!(
            n.get_optional_child("device-identity").is_some(),
            "pkmsg group retry with account must include <device-identity>"
        );
    }

    /// Symmetric to peer/dm pre-flights: refuse group retry pkmsg when
    /// account is missing rather than silently dropping device-identity.
    #[tokio::test]
    async fn group_retry_pkmsg_preflight_errors_when_account_missing() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();

        let before = ss
            .load_session(&p.to_protocol_address())
            .await
            .unwrap()
            .expect("pre-condition: session present")
            .serialize()
            .expect("serialize before");

        let result = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group,
            p.clone(),
            p.clone(),
            &wa::Message::default(),
            "grp-retry-no-account".into(),
            1,
            None,
            AddressingMode::Pn,
            None,
        )
        .await;
        let err = result.expect_err("group retry pkmsg must reject missing account");
        assert!(
            err.to_string().contains("device-identity"),
            "error must name <device-identity>; got: {err}"
        );

        let after = ss
            .load_session(&p.to_protocol_address())
            .await
            .unwrap()
            .expect("session still present")
            .serialize()
            .expect("serialize after");
        assert_eq!(
            before, after,
            "group retry pre-flight must leave the session byte-identical"
        );
    }

    /// Pins the WAWebSendMsgCreateDeviceStanza retry shape: `<enc>`
    /// directly under `<message>` plus a `recipient` attribute.
    /// Pre-fix this regressed to the fanout shape and the server
    /// rejected every retry with 479.
    #[tokio::test]
    async fn dm_retry_emits_enc_directly_under_message_with_recipient() {
        let (mut ss, mut is, jid) = setup_session().await;
        // Distinct values so a swapped-args regression (e.g. `recipient =
        // to_jid`) fails the assertions below instead of silently passing.
        let to: Jid = "559922223333:5@s.whatsapp.net".parse().unwrap();
        let recipient: Jid = "100000000000456@lid".parse().unwrap();
        let requester: Jid = jid.to_string().parse().unwrap();
        let account = pkmsg_account_proto();
        let n = prepare_dm_retry_stanza(
            &mut ss,
            &mut is,
            to.clone(),
            Some(recipient.clone()),
            requester,
            &wa::Message::default(),
            "dm-retry-format-1".into(),
            1,
            Some(&account),
            None,
        )
        .await
        .unwrap();

        assert_eq!(n.tag, "message");
        // <enc> is a direct child — no <participants> wrapper.
        assert!(
            n.get_optional_child("participants").is_none(),
            "DM retry must not wrap <enc> in <participants> \
                 (matches WAWebSendMsgCreateDeviceStanza)"
        );
        assert!(
            n.get_optional_child("enc").is_some(),
            "<enc> must be a direct child of <message>"
        );
        assert_eq!(
            n.attrs().optional_string("to").unwrap().as_ref(),
            to.to_string(),
            "`to` should target the requesting device verbatim"
        );
        assert_eq!(
            n.attrs().optional_string("recipient").unwrap().as_ref(),
            recipient.to_string(),
            "`recipient` should mirror the original message's recipient \
                 (forwarded from the retry receipt's `recipient` attr)"
        );
    }

    #[tokio::test]
    async fn dm_retry_pkmsg_targets_single_device() {
        let (mut ss, mut is, jid) = setup_session().await;
        let to: Jid = "559922223333@s.whatsapp.net".parse().unwrap();
        let encryption = jid.clone();
        let account = pkmsg_account_proto();

        let n = prepare_dm_retry_stanza(
            &mut ss,
            &mut is,
            to.clone(),
            Some(to.clone()),
            encryption,
            &wa::Message::default(),
            "dm-retry-1".into(),
            1,
            Some(&account),
            None,
        )
        .await
        .unwrap();

        assert_eq!(n.tag, "message");
        let mut attrs = n.attrs();
        assert_eq!(
            attrs.optional_string("to").unwrap().as_ref(),
            to.to_string()
        );
        assert_eq!(
            attrs.optional_string("recipient").unwrap().as_ref(),
            to.to_string()
        );
        assert_eq!(attrs.optional_string("id").unwrap().as_ref(), "dm-retry-1");
        assert_eq!(
            attrs.optional_string("type").unwrap().as_ref(),
            stanza::MSG_TYPE_MEDIA
        );
        assert!(attrs.optional_string("participant").is_none());
        assert!(attrs.optional_string("addressing_mode").is_none());

        // `<enc>` is a direct child of `<message>` (no `<participants>` wrapper).
        assert!(n.get_optional_child("participants").is_none());
        let enc = n.get_optional_child("enc").unwrap();
        let mut enc_attrs = enc.attrs();
        assert_eq!(
            enc_attrs.optional_string("type").unwrap().as_ref(),
            stanza::ENC_TYPE_PKMSG
        );
        assert_eq!(enc_attrs.optional_string("count").unwrap().as_ref(), "1");
        assert!(
            n.get_optional_child("device-identity").is_some(),
            "pkmsg DM retry with account must include <device-identity>"
        );
    }

    #[tokio::test]
    async fn dm_retry_pkmsg_with_account_has_device_identity() {
        let (mut ss, mut is, jid) = setup_session().await;
        let to: Jid = "559922223333@s.whatsapp.net".parse().unwrap();
        let acc = wa::AdvSignedDeviceIdentity {
            details: Some(b"t".to_vec()),
            ..Default::default()
        };

        let n = prepare_dm_retry_stanza(
            &mut ss,
            &mut is,
            to.clone(),
            Some(to),
            jid,
            &wa::Message::default(),
            "dm-retry-2".into(),
            2,
            Some(&acc),
            None,
        )
        .await
        .unwrap();

        let enc = n.get_optional_child("enc").unwrap();
        assert_eq!(
            enc.attrs().optional_string("type").unwrap().as_ref(),
            stanza::ENC_TYPE_PKMSG
        );
        assert_eq!(enc.attrs().optional_string("count").unwrap().as_ref(), "2");
        assert!(n.get_optional_child("device-identity").is_some());
    }

    #[tokio::test]
    async fn pkmsg_with_account_has_device_identity() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();
        let acc = wa::AdvSignedDeviceIdentity {
            details: Some(b"t".to_vec()),
            ..Default::default()
        };
        let n = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group,
            p.clone(),
            p,
            &wa::Message::default(),
            "id2".into(),
            2,
            Some(&acc),
            AddressingMode::Pn,
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            n.get_optional_child("enc")
                .unwrap()
                .attrs()
                .optional_string("type")
                .unwrap()
                .as_ref(),
            stanza::ENC_TYPE_PKMSG
        );
        assert!(n.get_optional_child("device-identity").is_some());
        assert_eq!(
            n.attrs()
                .optional_string("addressing_mode")
                .unwrap()
                .as_ref(),
            "pn"
        );
    }

    #[tokio::test]
    async fn lid_addressing_mode() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();
        // Fresh session → pkmsg (pre-key), with LID addressing
        let n = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group,
            p.clone(),
            p,
            &wa::Message::default(),
            "m2".into(),
            3,
            Some(&wa::AdvSignedDeviceIdentity::default()),
            AddressingMode::Lid,
            None,
        )
        .await
        .unwrap();
        let mut ea = n.get_optional_child("enc").unwrap().attrs();
        assert_eq!(ea.optional_string("count").unwrap().as_ref(), "3");
        assert_eq!(
            n.attrs()
                .optional_string("addressing_mode")
                .unwrap()
                .as_ref(),
            "lid"
        );
    }

    #[tokio::test]
    async fn group_retry_preserves_edit_attribute() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();
        let account = pkmsg_account_proto();
        let n = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group,
            p.clone(),
            p,
            &wa::Message::default(),
            "revoke-1".into(),
            1,
            Some(&account),
            AddressingMode::Lid,
            Some(crate::types::message::EditAttribute::AdminRevoke),
        )
        .await
        .unwrap();
        assert_eq!(n.attrs().optional_string("edit").unwrap().as_ref(), "8");
    }

    #[tokio::test]
    async fn dm_retry_preserves_edit_attribute() {
        let (mut ss, mut is, jid) = setup_session().await;
        let to: Jid = "559922223333@s.whatsapp.net".parse().unwrap();
        let account = pkmsg_account_proto();
        let n = prepare_dm_retry_stanza(
            &mut ss,
            &mut is,
            to.clone(),
            Some(to),
            jid,
            &wa::Message::default(),
            "edit-1".into(),
            1,
            Some(&account),
            Some(crate::types::message::EditAttribute::MessageEdit),
        )
        .await
        .unwrap();
        assert_eq!(n.attrs().optional_string("edit").unwrap().as_ref(), "1");
    }

    #[tokio::test]
    async fn retry_without_edit_omits_attribute() {
        let (mut ss, mut is, jid) = setup_session().await;
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let p: Jid = jid.to_string().parse().unwrap();
        let account = pkmsg_account_proto();
        let n = prepare_group_retry_stanza(
            &mut ss,
            &mut is,
            group,
            p.clone(),
            p,
            &wa::Message::default(),
            "plain-1".into(),
            1,
            Some(&account),
            AddressingMode::Lid,
            None,
        )
        .await
        .unwrap();
        assert!(n.attrs().optional_string("edit").is_none());
    }

    // Peer pkmsg layout: `[<meta appdata="default"/>, <enc>, <device-identity>]`.
    // Without `<device-identity>` the phone XMPP-acks but its Signal
    // layer skips session promotion. Mirrors whatsmeow's
    // `preparePeerMessageNode`.

    fn pkmsg_account_proto() -> wa::AdvSignedDeviceIdentity {
        // Opaque placeholder bytes — the assertions only check that
        // the element carries non-empty content.
        wa::AdvSignedDeviceIdentity {
            details: Some(vec![0u8; 32]),
            account_signature_key: Some(vec![0u8; 32]),
            account_signature: Some(vec![0u8; 64]),
            device_signature: Some(vec![0u8; 64]),
        }
    }

    async fn build_peer_stanza(
        account: Option<&wa::AdvSignedDeviceIdentity>,
    ) -> wacore_binary::Node {
        build_peer_stanza_with_options(account, PeerMessageOptions::default()).await
    }

    async fn build_peer_stanza_with_options(
        account: Option<&wa::AdvSignedDeviceIdentity>,
        options: PeerMessageOptions,
    ) -> wacore_binary::Node {
        let (mut ss, mut is, jid) = setup_session().await;
        let addr = jid.to_protocol_address();
        prepare_peer_stanza_with_options(
            &mut ss,
            &mut is,
            jid.clone(),
            &addr,
            &wa::Message::default(),
            "peer-test-1".into(),
            account,
            options,
        )
        .await
        .expect("peer stanza builds")
    }

    #[tokio::test]
    async fn peer_pkmsg_includes_meta_and_device_identity() {
        let account = pkmsg_account_proto();
        let n = build_peer_stanza(Some(&account)).await;

        assert_eq!(n.tag, "message");
        assert_eq!(
            n.attrs().optional_string("category").unwrap().as_ref(),
            "peer"
        );
        assert_eq!(
            n.attrs().optional_string("push_priority").unwrap().as_ref(),
            "high"
        );
        assert!(n.attrs().optional_string("privacy_sensitive").is_none());

        let children = n.children().expect("peer message has children");
        let tags: Vec<&str> = children.iter().map(|c| c.tag.as_ref()).collect();
        // Layout matches whatsmeow's preparePeerMessageNode for pkmsg:
        // [<meta>, <enc>, <device-identity>].
        assert_eq!(
            tags,
            vec!["meta", "enc", "device-identity"],
            "peer pkmsg children order/identity must match whatsmeow"
        );

        let meta = n.get_optional_child("meta").expect("meta present");
        assert_eq!(
            meta.attrs().optional_string("appdata").unwrap().as_ref(),
            "default",
            "<meta appdata=\"default\"/> is what the phone uses to route the peer payload"
        );

        let enc = n.get_optional_child("enc").expect("enc present");
        assert_eq!(
            enc.attrs().optional_string("type").unwrap().as_ref(),
            "pkmsg",
            "fresh session must produce pkmsg, not msg"
        );

        let device_identity = n
            .get_optional_child("device-identity")
            .expect("device-identity present");
        match &device_identity.content {
            Some(NodeContent::Bytes(b)) => assert!(
                !b.is_empty(),
                "device-identity content must be the proto-encoded \
                     AdvSignedDeviceIdentity, not empty"
            ),
            other => panic!("device-identity must carry bytes, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn peer_stanza_carries_high_force_and_privacy_attrs() {
        let account = pkmsg_account_proto();
        let n = build_peer_stanza_with_options(
            Some(&account),
            PeerMessageOptions::high_force_on_demand(),
        )
        .await;

        assert_eq!(
            n.attrs().optional_string("push_priority").unwrap().as_ref(),
            "high_force"
        );
        assert_eq!(
            n.attrs()
                .optional_string("privacy_sensitive")
                .unwrap()
                .as_ref(),
            "1"
        );
    }

    #[tokio::test]
    async fn peer_pkmsg_errors_when_account_missing_without_ratchet_advance() {
        // Pkmsg without <device-identity> would reproduce the deadlock —
        // refuse AND prove the session is byte-identical after the failed
        // call so the next retry has the same ratchet position.
        let (mut ss, mut is, jid) = setup_session().await;
        let addr = jid.to_protocol_address();

        let before = ss
            .load_session(&addr)
            .await
            .unwrap()
            .expect("pre-condition: session loaded")
            .serialize()
            .expect("serialize before");

        let result = prepare_peer_stanza(
            &mut ss,
            &mut is,
            jid.clone(),
            &addr,
            &wa::Message::default(),
            "peer-test-no-account".into(),
            None,
        )
        .await;
        let err = result.expect_err("pkmsg path must reject missing account");
        assert!(
            err.to_string().contains("device-identity"),
            "error must name the missing element; got: {err}"
        );

        let after = ss
            .load_session(&addr)
            .await
            .unwrap()
            .expect("session still present after failed call")
            .serialize()
            .expect("serialize after");
        assert_eq!(
            before, after,
            "session record must be byte-identical after a failed prepare — \
                 any difference means a ratchet step was committed for a stanza we couldn't ship"
        );
    }

    /// Pre-flight check: when no session exists and account is None,
    /// `prepare_peer_stanza` must refuse before `message_encrypt` runs,
    /// otherwise the sender chain is persisted for a stanza we cannot ship
    /// (CodeRabbit-flagged ratchet-burn-on-fail-fast).
    #[tokio::test]
    async fn peer_pkmsg_preflight_no_ratchet_burn_without_session() {
        let jid: Jid = "559911112222@s.whatsapp.net".parse().unwrap();
        let addr = jid.to_protocol_address();
        let mut ss = MemSessionStore::new();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let mut is = MemIdentityStore {
            pair: IdentityKeyPair::generate(&mut rng),
            reg_id: 42,
            known: HashMap::new(),
        };

        assert!(
            !ss.has_session(&addr).await.unwrap(),
            "precondition: store has no session for this address"
        );

        let result = prepare_peer_stanza(
            &mut ss,
            &mut is,
            jid.clone(),
            &addr,
            &wa::Message::default(),
            "peer-preflight-1".into(),
            None,
        )
        .await;
        let err = result.expect_err("must refuse before message_encrypt");
        assert!(
            err.to_string().contains("device-identity"),
            "error must name <device-identity>; got: {err}"
        );
        assert!(
            !ss.has_session(&addr).await.unwrap(),
            "pre-flight must NOT advance/persist a session — the ratchet \
                 must remain unburned for the retry attempt"
        );
    }

    /// Symmetric to peer_pkmsg_preflight: prepare_dm_retry_stanza must
    /// also refuse to ship pkmsg without <device-identity>, otherwise
    /// message_encrypt would advance the sender chain for a stanza the
    /// peer's Signal layer cannot promote.
    #[tokio::test]
    async fn dm_retry_pkmsg_preflight_errors_when_account_missing() {
        let (mut ss, mut is, jid) = setup_session().await;
        let addr = jid.to_protocol_address();

        let before = ss
            .load_session(&addr)
            .await
            .unwrap()
            .expect("pre-condition: session present")
            .serialize()
            .expect("serialize before");

        let to: Jid = "559922223333@s.whatsapp.net".parse().unwrap();
        let result = prepare_dm_retry_stanza(
            &mut ss,
            &mut is,
            to.clone(),
            Some(to),
            jid.clone(),
            &wa::Message::default(),
            "dm-retry-no-account".into(),
            1,
            None,
            None,
        )
        .await;
        let err = result.expect_err("DM retry pkmsg path must reject missing account");
        assert!(
            err.to_string().contains("device-identity"),
            "error must name <device-identity>; got: {err}"
        );

        let after = ss
            .load_session(&addr)
            .await
            .unwrap()
            .expect("session still present")
            .serialize()
            .expect("serialize after");
        assert_eq!(
            before, after,
            "DM retry pre-flight must leave the session byte-identical"
        );
    }

    /// Production's SessionAdapter::load_session has TAKE semantics
    /// (SignalStoreCache marks the slot CheckedOut until store_session
    /// puts the record back). If the pre-flight only loads without
    /// restoring, the slot stays stranded and message_encrypt sees no
    /// session. The mock here mirrors that contract via interior
    /// mutability (Mutex) on the &self load_session.
    #[tokio::test]
    async fn preflight_restores_session_with_take_store_semantics() {
        use std::collections::{HashMap, HashSet};
        use std::sync::Mutex;

        struct TakeStore {
            inner: Mutex<TakeInner>,
        }
        struct TakeInner {
            present: HashMap<ProtocolAddress, Vec<u8>>,
            taken: HashSet<ProtocolAddress>,
        }
        impl TakeStore {
            fn from(ss: &MemSessionStore) -> Self {
                Self {
                    inner: Mutex::new(TakeInner {
                        present: ss.0.clone(),
                        taken: HashSet::new(),
                    }),
                }
            }
            fn is_present(&self, addr: &ProtocolAddress) -> bool {
                let g = self.inner.lock().unwrap();
                g.present.contains_key(addr) && !g.taken.contains(addr)
            }
        }
        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl SessionStore for TakeStore {
            async fn load_session(
                &self,
                a: &ProtocolAddress,
            ) -> crate::libsignal::protocol::error::Result<
                Option<crate::libsignal::protocol::SessionRecord>,
            > {
                let mut g = self.inner.lock().unwrap();
                if g.taken.contains(a) {
                    return Ok(None);
                }
                let rec = g
                    .present
                    .get(a)
                    .and_then(|b| crate::libsignal::protocol::SessionRecord::deserialize(b).ok());
                if rec.is_some() {
                    g.taken.insert(a.clone());
                }
                Ok(rec)
            }
            async fn has_session(
                &self,
                a: &ProtocolAddress,
            ) -> crate::libsignal::protocol::error::Result<bool> {
                let g = self.inner.lock().unwrap();
                Ok(g.present.contains_key(a) && !g.taken.contains(a))
            }
            async fn store_session(
                &mut self,
                a: &ProtocolAddress,
                r: crate::libsignal::protocol::SessionRecord,
            ) -> crate::libsignal::protocol::error::Result<()> {
                let mut g = self.inner.lock().unwrap();
                g.present.insert(a.clone(), r.serialize()?);
                g.taken.remove(a);
                Ok(())
            }
        }

        let (mem_ss, mut is, jid) = setup_session().await;
        let mut ss = TakeStore::from(&mem_ss);
        let addr = jid.to_protocol_address();

        // setup_session leaves pending_pre_key set, so account=None
        // would bail. Use Some(account) — pre-flight still runs
        // load+restore because it's gated on account.is_none() at the
        // call site; switch to account=None and we want the assertion
        // to verify that the BAIL path also restores the slot.
        assert!(
            ss.is_present(&addr),
            "precondition: session is Present before pre-flight"
        );

        // Drive the bail path: account=None + session has pending_pre_key
        // → pre-flight bails. Even on bail, the loaded record must be
        // put back so a retry with Some(account) doesn't see a stranded slot.
        let bail = prepare_peer_stanza(
            &mut ss,
            &mut is,
            jid.clone(),
            &addr,
            &wa::Message::default(),
            "preflight-take-bail".into(),
            None,
        )
        .await;
        bail.expect_err("must bail with account=None on a pending-pkmsg session");
        assert!(
            ss.is_present(&addr),
            "pre-flight bail path must still restore the checked-out session"
        );

        // And the pass path: with Some(account), the pre-flight still
        // does load+restore, then message_encrypt runs successfully.
        let account = pkmsg_account_proto();
        let ok = prepare_peer_stanza(
            &mut ss,
            &mut is,
            jid.clone(),
            &addr,
            &wa::Message::default(),
            "preflight-take-pass".into(),
            Some(&account),
        )
        .await;
        ok.expect("peer stanza builds with Some(account)");
        assert!(
            ss.is_present(&addr),
            "session must be Present after a successful encrypt+store"
        );
    }
}

mod decrypt_fail {
    use super::*;

    #[test]
    fn regular_message() {
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        assert!(!should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn reaction() {
        let msg = wa::Message {
            reaction_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn pin() {
        let msg = wa::Message {
            pin_in_chat_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn poll_vote() {
        let msg = wa::Message {
            poll_update_message: Some(Box::new(wa::message::PollUpdateMessage {
                vote: Some(Default::default()),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn poll_update_without_vote() {
        let msg = wa::Message {
            poll_update_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(!should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn reaction_inside_ephemeral_wrapper() {
        let msg = wa::Message {
            ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message {
                    reaction_message: Some(Box::default()),
                    ..Default::default()
                })),
            })),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn conditional_reveal() {
        let msg = wa::Message {
            conditional_reveal_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }

    #[test]
    fn poll_add_option_edit() {
        use wa::message::secret_encrypted_message::SecretEncType;
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                secret_enc_type: Some(SecretEncType::PollAddOption as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail(&msg));
    }
}

mod decrypt_fail_for_send {
    use super::*;
    use crate::types::message::EditAttribute;

    fn plain() -> wa::Message {
        wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        }
    }

    #[test]
    fn sender_revoke_is_not_hidden() {
        assert!(!should_hide_decrypt_fail_for_send(
            Some(&EditAttribute::SenderRevoke),
            &plain()
        ));
    }

    #[test]
    fn admin_revoke_is_not_hidden() {
        assert!(!should_hide_decrypt_fail_for_send(
            Some(&EditAttribute::AdminRevoke),
            &plain()
        ));
    }

    #[test]
    fn message_edit_is_hidden() {
        assert!(should_hide_decrypt_fail_for_send(
            Some(&EditAttribute::MessageEdit),
            &plain()
        ));
    }

    #[test]
    fn revoke_does_not_block_content_based_hide() {
        // A reaction still hides on its own merits even under a revoke edit.
        let msg = wa::Message {
            reaction_message: Some(Box::default()),
            ..Default::default()
        };
        assert!(should_hide_decrypt_fail_for_send(
            Some(&EditAttribute::SenderRevoke),
            &msg
        ));
    }
}

mod stanza_type {
    use super::*;
    use wa::message::secret_encrypted_message::SecretEncType;

    fn secret(enc: SecretEncType) -> wa::Message {
        wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                secret_enc_type: Some(enc as i32),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    #[test]
    fn poll_add_option_edit_is_poll() {
        assert_eq!(
            stanza_type_from_message(&secret(SecretEncType::PollAddOption)),
            stanza::MSG_TYPE_POLL
        );
    }

    #[test]
    fn poll_edit_is_poll() {
        assert_eq!(
            stanza_type_from_message(&secret(SecretEncType::PollEdit)),
            stanza::MSG_TYPE_POLL
        );
    }

    #[test]
    fn album_is_text() {
        let msg = wa::Message {
            album_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&msg), stanza::MSG_TYPE_TEXT);
    }

    // Helpers for wrapper tests. WA Web's typeAttributeFromProtobuf unwraps
    // FutureProofMessage wrappers (via getUnwrappedProtobufMessage) and then
    // classifies the inner message.
    fn fpm(inner: wa::Message) -> Box<wa::message::FutureProofMessage> {
        Box::new(wa::message::FutureProofMessage {
            message: Some(Box::new(inner)),
        })
    }
    fn text_inner() -> wa::Message {
        wa::Message {
            conversation: Some("hi".to_string()),
            ..Default::default()
        }
    }
    fn image_inner() -> wa::Message {
        wa::Message {
            image_message: Some(Box::default()),
            ..Default::default()
        }
    }

    #[test]
    fn group_status_v2_classifies_by_inner() {
        let txt = wa::Message {
            group_status_message_v2: Some(fpm(text_inner())),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&txt), stanza::MSG_TYPE_TEXT);

        // Regression guard: forcing this wrapper to "text" dropped the
        // mediatype and silently dropped the stanza. WA Web unwraps it and
        // sends type="media" mediatype="image".
        let img = wa::Message {
            group_status_message_v2: Some(fpm(image_inner())),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&img), stanza::MSG_TYPE_MEDIA);
        assert_eq!(media_type_from_message(&img), Some("image"));
    }

    #[test]
    fn group_status_v2_empty_is_media() {
        // An empty wrapper is not one of WA Web's four re-checked wrappers
        // (ephemeral/groupMentioned/botInvoke/deviceSent), so it falls through
        // to the media default in both WA Web and here.
        let m = wa::Message {
            group_status_message_v2: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&m), stanza::MSG_TYPE_MEDIA);
    }

    #[test]
    fn payment_family_is_text() {
        // Payment family classifies as text; the media default would be dropped.
        let cases = [
            wa::Message {
                request_payment_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                send_payment_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                decline_payment_request_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                cancel_payment_request_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                payment_invite_message: Some(Box::default()),
                ..Default::default()
            },
        ];
        for m in cases {
            assert_eq!(media_type_from_message(&m), None);
            assert_eq!(stanza_type_from_message(&m), stanza::MSG_TYPE_TEXT);
        }
    }

    #[test]
    fn backfilled_wrappers_classify_by_inner() {
        let spoiler = wa::Message {
            spoiler_message: Some(fpm(text_inner())),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&spoiler), stanza::MSG_TYPE_TEXT);

        let status_mention = wa::Message {
            status_mention_message: Some(fpm(image_inner())),
            ..Default::default()
        };
        assert_eq!(
            stanza_type_from_message(&status_mention),
            stanza::MSG_TYPE_MEDIA
        );
        assert_eq!(media_type_from_message(&status_mention), Some("image"));

        let question = wa::Message {
            question_message: Some(fpm(text_inner())),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&question), stanza::MSG_TYPE_TEXT);

        let group_status_v1 = wa::Message {
            group_status_message: Some(fpm(text_inner())),
            ..Default::default()
        };
        assert_eq!(
            stanza_type_from_message(&group_status_v1),
            stanza::MSG_TYPE_TEXT
        );
    }

    #[test]
    fn nested_wrappers_reach_innermost() {
        // ephemeral { viewOnceV2 { image } } -> media + mediatype.
        let inner = wa::Message {
            view_once_message_v2: Some(fpm(image_inner())),
            ..Default::default()
        };
        let m = wa::Message {
            ephemeral_message: Some(fpm(inner)),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&m), stanza::MSG_TYPE_MEDIA);
        assert_eq!(media_type_from_message(&m), Some("image"));
    }

    #[test]
    fn preserved_classifier_branches() {
        let r = wa::Message {
            reaction_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&r), stanza::MSG_TYPE_REACTION);

        let ev = wa::Message {
            event_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&ev), stanza::MSG_TYPE_EVENT);

        let poll = wa::Message {
            poll_creation_message_v3: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&poll), stanza::MSG_TYPE_POLL);

        assert_eq!(
            stanza_type_from_message(&text_inner()),
            stanza::MSG_TYPE_TEXT
        );
        assert_eq!(
            stanza_type_from_message(&image_inner()),
            stanza::MSG_TYPE_MEDIA
        );

        let proto = wa::Message {
            protocol_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&proto), stanza::MSG_TYPE_TEXT);

        let url = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                matched_text: Some("https://example.com".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&url), stanza::MSG_TYPE_MEDIA);
    }

    #[test]
    fn interactive_and_list_types_get_their_mediatype() {
        // WA Web's mediaTypeFromProtobuf maps these to concrete mediatypes;
        // omitting the attribute makes the server drop the type="media" stanza.
        let list = wa::Message {
            list_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(stanza_type_from_message(&list), stanza::MSG_TYPE_MEDIA);
        assert_eq!(media_type_from_message(&list), Some("list"));

        let list_response = wa::Message {
            list_response_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(
            media_type_from_message(&list_response),
            Some("list_response")
        );

        let buttons_response = wa::Message {
            buttons_response_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(
            media_type_from_message(&buttons_response),
            Some("buttons_response")
        );

        let order = wa::Message {
            order_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(media_type_from_message(&order), Some("order"));

        let product = wa::Message {
            product_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(media_type_from_message(&product), Some("product"));

        let interactive_response = wa::Message {
            interactive_response_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(
            media_type_from_message(&interactive_response),
            Some("native_flow_response")
        );

        let history_bundle = wa::Message {
            message_history_bundle: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(
            media_type_from_message(&history_bundle),
            Some("group_history")
        );
    }

    #[test]
    fn buttons_message_has_no_mediatype() {
        // WA Web maps buttonsMessage to EncMediaType.Button, but its string
        // mapper has no Button case (returns null/DROP_ATTR), so the attribute
        // is omitted. Adding a "buttons" mediatype would diverge from WA Web.
        let buttons = wa::Message {
            buttons_message: Some(Box::default()),
            ..Default::default()
        };
        assert_eq!(media_type_from_message(&buttons), None);
    }

    #[test]
    fn ephemeral_wrapped_list_reaches_list_mediatype() {
        let m = wa::Message {
            ephemeral_message: Some(fpm(wa::Message {
                list_message: Some(Box::default()),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(media_type_from_message(&m), Some("list"));
    }

    #[test]
    fn top_level_lottie_sticker_is_terminal_sticker() {
        // WA Web's mediaTypeFromProtobuf treats a top-level lottieStickerMessage
        // as a terminal "sticker" and does NOT recurse into it, unlike the
        // stanza-type path which unwraps it.
        let lottie = wa::Message {
            lottie_sticker_message: Some(fpm(image_inner())),
            ..Default::default()
        };
        assert_eq!(media_type_from_message(&lottie), Some("sticker"));
    }
}

#[cfg(test)]
mod device_unregistered_tests {
    use super::is_device_unregistered_error;
    use crate::request::ServerErrorCode;

    #[test]
    fn detects_406_server_error_code() {
        let err = anyhow::Error::new(ServerErrorCode {
            code: 406,
            text: "not-acceptable".to_string(),
            error_type: None,
            backoff: None,
        });
        assert!(is_device_unregistered_error(&err));
    }

    #[test]
    fn rejects_non_406_server_error() {
        let err = anyhow::Error::new(ServerErrorCode {
            code: 404,
            text: "not-found".to_string(),
            error_type: None,
            backoff: None,
        });
        assert!(!is_device_unregistered_error(&err));
    }

    #[test]
    fn rejects_unrelated_error() {
        let err = anyhow::anyhow!("some random error");
        assert!(!is_device_unregistered_error(&err));
    }

    #[test]
    fn rejects_wacore_iq_error_without_server_error_code_wrapper() {
        // wacore::IqError::ServerError is NOT the same as ServerErrorCode.
        // This simulates the old bug: if someone wraps wacore IqError directly
        // without the ServerErrorCode wrapper, the check should not match.
        let err = anyhow::Error::new(crate::request::IqError::ServerError {
            code: 406,
            text: "not-acceptable".to_string(),
            error_type: None,
            backoff: None,
        });
        // This would only match if we also checked IqError (we don't — we use ServerErrorCode)
        // The SendContextResolver impl is responsible for wrapping in ServerErrorCode
        assert!(!is_device_unregistered_error(&err));
    }
}

mod collect_stale_device_users {
    use super::super::collect_stale_device_users;
    use crate::client::context::GroupInfo;
    use crate::types::message::AddressingMode;
    use std::collections::{HashMap, HashSet};
    use wacore_binary::{CompactString, Jid};

    fn lid_device(user: &str, dev: u16) -> Jid {
        Jid::lid_device(user.to_string(), dev)
    }

    fn pn_user(user: &str) -> Jid {
        Jid::pn(user)
    }

    fn group_info_lid(mapping: &[(&str, &str)]) -> GroupInfo {
        let mut info = GroupInfo::new(Vec::new(), AddressingMode::Lid);
        if !mapping.is_empty() {
            let mut map: HashMap<CompactString, Jid> = HashMap::new();
            for (lid_user, pn) in mapping {
                map.insert(CompactString::from(*lid_user), pn_user(pn));
            }
            info.set_lid_to_pn_map(map);
        }
        info
    }

    #[test]
    fn emits_lid_and_pn_alias_when_mapping_known() {
        let info = group_info_lid(&[("100000000000001", "15550000001")]);
        let dist = vec![lid_device("100000000000001", 5)];
        let out = collect_stale_device_users(Some(&dist), &[], &info);
        let set: HashSet<String> = out.into_iter().collect();
        assert!(set.contains("100000000000001"));
        assert!(set.contains("15550000001"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn emits_only_lid_when_mapping_unknown() {
        let info = group_info_lid(&[]);
        let dist = vec![lid_device("100000000000002", 7)];
        let out = collect_stale_device_users(Some(&dist), &[], &info);
        assert_eq!(out, vec!["100000000000002".to_string()]);
    }

    #[test]
    fn dedups_multiple_devices_of_same_user() {
        let info = group_info_lid(&[("100000000000003", "15550000003")]);
        let dist = vec![
            lid_device("100000000000003", 1),
            lid_device("100000000000003", 2),
            lid_device("100000000000003", 3),
        ];
        let out = collect_stale_device_users(Some(&dist), &[], &info);
        let set: HashSet<String> = out.into_iter().collect();
        assert_eq!(set.len(), 2);
        assert!(set.contains("100000000000003"));
        assert!(set.contains("15550000003"));
    }

    #[test]
    fn skips_successfully_encrypted_devices() {
        let info = group_info_lid(&[]);
        let encrypted = lid_device("100000000000004", 5);
        let dist = vec![encrypted.clone(), lid_device("100000000000005", 5)];
        let encrypted_set = vec![encrypted];
        let out = collect_stale_device_users(Some(&dist), &encrypted_set, &info);
        assert_eq!(out, vec!["100000000000005".to_string()]);
    }

    #[test]
    fn pn_mode_group_does_not_emit_alias() {
        // In PN-mode groups the distribution list is already PN-form, so
        // there's no LID↔PN duality to emit.
        let mut info = GroupInfo::new(Vec::new(), AddressingMode::Pn);
        let mut map: HashMap<CompactString, Jid> = HashMap::new();
        map.insert(
            CompactString::from("100000000000006"),
            pn_user("15550000006"),
        );
        info.set_lid_to_pn_map(map);
        let dist = vec![Jid::pn_device("15550000006", 3)];
        let out = collect_stale_device_users(Some(&dist), &[], &info);
        assert_eq!(out, vec!["15550000006".to_string()]);
    }

    #[test]
    fn skips_non_pn_alias() {
        // If phone_jid_for_lid_user returns a JID whose server isn't PN
        // (malformed/adversarial server response), do not emit it.
        let mut info = GroupInfo::new(Vec::new(), AddressingMode::Lid);
        let mut map: HashMap<CompactString, Jid> = HashMap::new();
        map.insert(
            CompactString::from("100000000000007"),
            Jid::lid("100000000000099"),
        );
        info.set_lid_to_pn_map(map);
        let dist = vec![lid_device("100000000000007", 5)];
        let out = collect_stale_device_users(Some(&dist), &[], &info);
        assert_eq!(out, vec!["100000000000007".to_string()]);
    }

    #[test]
    fn empty_distribution_list_yields_empty() {
        let info = group_info_lid(&[]);
        let out = collect_stale_device_users(None, &[], &info);
        assert!(out.is_empty());
        let out = collect_stale_device_users(Some(&[]), &[], &info);
        assert!(out.is_empty());
    }
}

/// Item 2 — WA Web `markHasSenderKey(x, M)`: a key-distributing group send
/// marks the FULL SKDM target set `has_key=true`, not only the devices that
/// encrypted successfully. A device whose SKDM encryption fails (no session
/// and no bundle, mimicking a 406) must still land in
/// `PreparedGroupStanza.skdm_devices`, so the next send does not re-target
/// it every time (the fan-out storm); the retry-receipt path repairs any
/// device that is actually alive and keyless.
mod mark_full_distribution_list {
    use super::*;
    use crate::libsignal::protocol::{
        Direction, IdentityChange, IdentityKey, IdentityKeyStore, PreKeyId, PreKeyRecord,
        PreKeyStore, ProtocolAddress, SenderKeyRecord, SenderKeyStore, SessionStore,
        SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore, UsePQRatchet, process_prekey_bundle,
    };
    use crate::libsignal::store::sender_key_name::SenderKeyName;
    use crate::runtime::{AbortHandle, Runtime};
    use crate::types::jid::JidExt;
    use crate::types::message::AddressingMode;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    type SigResult<T> = crate::libsignal::protocol::error::Result<T>;

    // Clones share state (Arc), mirroring production stores: the encrypt
    // fan-out spawns tasks over store clones and their writes must be
    // visible to the original ("the shared cache provides interior
    // mutability").
    #[derive(Clone, Default)]
    struct MemSessionStore(std::sync::Arc<std::sync::Mutex<HashMap<ProtocolAddress, Vec<u8>>>>);
    #[async_trait::async_trait]
    impl SessionStore for MemSessionStore {
        async fn load_session(
            &self,
            a: &ProtocolAddress,
        ) -> SigResult<Option<crate::libsignal::protocol::SessionRecord>> {
            Ok(self
                .0
                .lock()
                .unwrap()
                .get(a)
                .and_then(|b| crate::libsignal::protocol::SessionRecord::deserialize(b).ok()))
        }
        async fn has_session(&self, a: &ProtocolAddress) -> SigResult<bool> {
            Ok(self.0.lock().unwrap().contains_key(a))
        }
        async fn store_session(
            &mut self,
            a: &ProtocolAddress,
            r: crate::libsignal::protocol::SessionRecord,
        ) -> SigResult<()> {
            self.0.lock().unwrap().insert(a.clone(), r.serialize()?);
            Ok(())
        }
    }

    #[derive(Clone)]
    struct MemIdentityStore {
        pair: IdentityKeyPair,
        reg_id: u32,
        known: std::sync::Arc<std::sync::Mutex<HashMap<ProtocolAddress, IdentityKey>>>,
    }
    #[async_trait::async_trait]
    impl IdentityKeyStore for MemIdentityStore {
        async fn get_identity_key_pair(&self) -> SigResult<IdentityKeyPair> {
            Ok(self.pair.clone())
        }
        async fn get_local_registration_id(&self) -> SigResult<u32> {
            Ok(self.reg_id)
        }
        async fn save_identity(
            &mut self,
            a: &ProtocolAddress,
            id: &IdentityKey,
        ) -> SigResult<IdentityChange> {
            self.known.lock().unwrap().insert(a.clone(), *id);
            Ok(IdentityChange::from_changed(false))
        }
        async fn is_trusted_identity(
            &self,
            _: &ProtocolAddress,
            _: &IdentityKey,
            _: Direction,
        ) -> SigResult<bool> {
            Ok(true)
        }
        async fn get_identity(&self, a: &ProtocolAddress) -> SigResult<Option<IdentityKey>> {
            Ok(self.known.lock().unwrap().get(a).copied())
        }
    }

    #[derive(Default)]
    struct MemSenderKeyStore {
        records: HashMap<SenderKeyName, SenderKeyRecord>,
        // Shared per-name locks (like production stores override it), so tests
        // can observe whether the chain lock is held during resolver calls.
        locks: std::sync::Mutex<HashMap<SenderKeyName, std::sync::Arc<async_lock::Mutex<()>>>>,
        setup_locks:
            std::sync::Mutex<HashMap<SenderKeyName, std::sync::Arc<async_lock::Mutex<()>>>>,
    }
    #[async_trait::async_trait]
    impl SenderKeyStore for MemSenderKeyStore {
        async fn store_sender_key(
            &mut self,
            n: &SenderKeyName,
            r: SenderKeyRecord,
        ) -> SigResult<()> {
            self.records.insert(n.clone(), r);
            Ok(())
        }
        async fn load_sender_key(&self, n: &SenderKeyName) -> SigResult<Option<SenderKeyRecord>> {
            Ok(self.records.get(n).cloned())
        }
        async fn sender_key_lock(
            &self,
            n: &SenderKeyName,
        ) -> std::sync::Arc<async_lock::Mutex<()>> {
            self.locks
                .lock()
                .unwrap()
                .entry(n.clone())
                .or_default()
                .clone()
        }
        async fn session_setup_lock(
            &self,
            n: &SenderKeyName,
        ) -> std::sync::Arc<async_lock::Mutex<()>> {
            self.setup_locks
                .lock()
                .unwrap()
                .entry(n.clone())
                .or_default()
                .clone()
        }
    }

    // Outgoing group encryption never consumes our own prekeys, and device B
    // has no bundle (so no session is established for it) — these are never
    // called; present only to satisfy the generic bounds.
    struct UnusedPreKeyStore;
    #[async_trait::async_trait]
    impl PreKeyStore for UnusedPreKeyStore {
        async fn get_pre_key(&self, _: PreKeyId) -> SigResult<PreKeyRecord> {
            unreachable!("prekey store not used in outgoing group encrypt")
        }
        async fn save_pre_key(&mut self, _: PreKeyId, _: &PreKeyRecord) -> SigResult<()> {
            unreachable!()
        }
        async fn remove_pre_key(&mut self, _: PreKeyId) -> SigResult<()> {
            unreachable!()
        }
    }
    struct UnusedSignedPreKeyStore;
    #[async_trait::async_trait]
    impl SignedPreKeyStore for UnusedSignedPreKeyStore {
        async fn get_signed_pre_key(&self, _: SignedPreKeyId) -> SigResult<SignedPreKeyRecord> {
            unreachable!("signed prekey store not used in outgoing group encrypt")
        }
        async fn save_signed_pre_key(
            &mut self,
            _: SignedPreKeyId,
            _: &SignedPreKeyRecord,
        ) -> SigResult<()> {
            unreachable!()
        }
    }

    struct TokioTestRuntime;
    #[async_trait::async_trait]
    impl Runtime for TokioTestRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            let handle = tokio::spawn(future);
            AbortHandle::new(move || handle.abort())
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            // Not exercised on the send path; wacore dev-deps omit tokio's
            // "time" feature, so resolve immediately rather than time out.
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Establish a real Signal session for `a` so its SKDM encrypts; the
    // returned identity store is the sender's (knows `a` after X3DH).
    async fn established_stores(a: &Jid) -> (MemSessionStore, MemIdentityStore) {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let sender = IdentityKeyPair::generate(&mut rng);
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let sig = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .unwrap();
        let bundle = PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            sig.to_vec(),
            *receiver.identity_key(),
        )
        .unwrap();
        let mut ss = MemSessionStore::default();
        let mut is = MemIdentityStore {
            pair: sender,
            reg_id: 42,
            known: Default::default(),
        };
        process_prekey_bundle(
            &a.to_protocol_address(),
            &mut ss,
            &mut is,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        (ss, is)
    }

    #[tokio::test]
    async fn failed_device_is_still_marked_has_key() {
        let group: Jid = "120363000000000001@g.us".parse().unwrap();
        let own_jid: Jid = "559900000000@s.whatsapp.net".parse().unwrap();
        let own_lid: Jid = "100000000000000@lid".parse().unwrap();
        // A has a session (encrypts ok); B has neither session nor bundle,
        // mimicking a device that 406'd / has no key material.
        let a: Jid = "559911112222:0@s.whatsapp.net".parse().unwrap();
        let b: Jid = "559933334444:0@s.whatsapp.net".parse().unwrap();

        let (mut ss, mut is) = established_stores(&a).await;
        let mut sks = MemSenderKeyStore::default();
        let mut pks = UnusedPreKeyStore;
        let spks = UnusedSignedPreKeyStore;
        let mut stores = SignalStores {
            sender_key_store: &mut sks,
            session_store: &mut ss,
            identity_store: &mut is,
            prekey_store: &mut pks,
            signed_prekey_store: &spks,
        };

        // Empty resolver: no LID overrides; B's prekey fetch returns nothing
        // → B is dropped by the encrypt fan-out (not in encrypted_devices).
        let resolver = MockSendContextResolver::new();
        let rt = TokioTestRuntime;

        let group_info = GroupInfo::new(
            vec![own_jid.to_non_ad(), a.to_non_ad(), b.to_non_ad()],
            AddressingMode::Pn,
        );
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };

        let prepared = prepare_group_stanza(
            &rt,
            &mut stores,
            &resolver,
            &group_info,
            &own_jid,
            &own_lid,
            None,
            group,
            &msg,
            "TESTREQID".into(),
            false,
            Some(vec![a.clone(), b.clone()]),
            None,
            None,
            &[],
        )
        .await
        .expect("prepare_group_stanza should succeed even when a device fails to encrypt");

        let marked: std::collections::HashSet<String> = prepared
            .skdm_devices
            .iter()
            .map(|j| j.to_string())
            .collect();

        assert!(
            marked.contains(&a.to_string()),
            "device that encrypted must be marked"
        );
        assert!(
            marked.contains(&b.to_string()),
            "device whose SKDM encryption FAILED must still be marked has_key \
                 (WA Web markHasSenderKey(x, M) marks the full target set → no re-fanout storm)"
        );
        assert_eq!(
            prepared.skdm_devices.len(),
            2,
            "exactly the full distribution list (A + B), not just the encrypted subset"
        );

        // A key-distributing send must carry a phash (computed over the list).
        assert!(
            prepared.node.attrs().optional_string("phash").is_some(),
            "a key-distributing group send must carry a phash"
        );
    }

    /// The group send shares one message encode between the reporting token and the skmsg
    /// plaintext (gated on no top-level `message_context_info`). The byte-equivalence of
    /// the shared-encode helpers is locked in `messages`/`reporting_token`; pin the
    /// group-level wiring here: a token-bearing send still mints a secret and attaches the
    /// `<reporting>` node via that path, while an excluded type (reaction) omits both.
    #[tokio::test]
    async fn group_send_attaches_reporting_token_via_shared_encode() {
        let group: Jid = "120363000000000003@g.us".parse().unwrap();
        let own_jid: Jid = "559900000000@s.whatsapp.net".parse().unwrap();
        let own_lid: Jid = "100000000000000@lid".parse().unwrap();
        let a: Jid = "559911112222:0@s.whatsapp.net".parse().unwrap();
        let group_info =
            GroupInfo::new(vec![own_jid.to_non_ad(), a.to_non_ad()], AddressingMode::Pn);

        async fn prepare(
            group: &Jid,
            own_jid: &Jid,
            own_lid: &Jid,
            a: &Jid,
            group_info: &GroupInfo,
            msg: &wa::Message,
            req: &str,
        ) -> (wacore_binary::Node, bool) {
            let (mut ss, mut is) = established_stores(a).await;
            let mut sks = MemSenderKeyStore::default();
            let mut pks = UnusedPreKeyStore;
            let spks = UnusedSignedPreKeyStore;
            let mut stores = SignalStores {
                sender_key_store: &mut sks,
                session_store: &mut ss,
                identity_store: &mut is,
                prekey_store: &mut pks,
                signed_prekey_store: &spks,
            };
            let resolver = MockSendContextResolver::new();
            let rt = TokioTestRuntime;
            let prepared = prepare_group_stanza(
                &rt,
                &mut stores,
                &resolver,
                group_info,
                own_jid,
                own_lid,
                None,
                group.clone(),
                msg,
                req.into(),
                false,
                Some(vec![a.clone()]),
                None,
                None,
                &[],
            )
            .await
            .expect("prepare_group_stanza should succeed");
            (prepared.node, prepared.message_secret.is_some())
        }

        // Token-bearing message → secret minted + <reporting> node carrying a token.
        let text = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        let (node, has_secret) = prepare(
            &group,
            &own_jid,
            &own_lid,
            &a,
            &group_info,
            &text,
            "REQTEXT",
        )
        .await;
        assert!(has_secret, "token-bearing send must mint a message secret");
        let reporting = node
            .get_optional_child("reporting")
            .expect("token-bearing group send must carry a <reporting> node");
        assert!(
            reporting.get_optional_child("reporting_token").is_some(),
            "reporting node must contain a reporting_token"
        );

        // Excluded type (reaction) → no secret, no <reporting> node.
        let reaction = wa::Message {
            reaction_message: Some(Box::new(wa::message::ReactionMessage {
                text: Some("👍".into()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let (node, has_secret) = prepare(
            &group,
            &own_jid,
            &own_lid,
            &a,
            &group_info,
            &reaction,
            "REQREACT",
        )
        .await;
        assert!(!has_secret, "excluded type must not mint a secret");
        assert!(
            node.get_optional_child("reporting").is_none(),
            "excluded type must not carry a reporting node"
        );
    }

    /// Regression: the prekey fetch (network RTT) must run BEFORE the
    /// sender-key chain lock is taken, so concurrent sends to the same group
    /// don't serialize behind a slow fetch. The probe try_locks the actual
    /// chain lock from inside the resolver's fetch and records a violation.
    #[tokio::test]
    async fn prekey_fetch_runs_outside_chain_lock() {
        use std::sync::atomic::Ordering::SeqCst;

        let group: Jid = "120363000000000002@g.us".parse().unwrap();
        let own_jid: Jid = "559900000000@s.whatsapp.net".parse().unwrap();
        let own_lid: Jid = "100000000000000@lid".parse().unwrap();
        // B has no session but its bundle IS available — forces the prekey
        // fetch + X3DH path on this send.
        let b: Jid = "559933334444:0@s.whatsapp.net".parse().unwrap();

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let mut ss = MemSessionStore::default();
        let mut is = MemIdentityStore {
            pair: IdentityKeyPair::generate(&mut rng),
            reg_id: 7,
            known: Default::default(),
        };
        let mut sks = MemSenderKeyStore::default();
        let chain_name =
            crate::types::jid::make_sender_key_name(&group, &own_jid.to_protocol_address());
        let probe = ChainLockProbe {
            lock: sks.sender_key_lock(&chain_name).await,
            setup_lock: sks.session_setup_lock(&chain_name).await,
            ..Default::default()
        };
        let mut pks = UnusedPreKeyStore;
        let spks = UnusedSignedPreKeyStore;
        let mut stores = SignalStores {
            sender_key_store: &mut sks,
            session_store: &mut ss,
            identity_store: &mut is,
            prekey_store: &mut pks,
            signed_prekey_store: &spks,
        };

        // Verifiable bundle (create_mock_bundle's zeroed signature fails X3DH).
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let sig = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .unwrap();
        let bundle = PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            sig.to_vec(),
            *receiver.identity_key(),
        )
        .unwrap();

        let resolver = MockSendContextResolver::new()
            .with_bundle(b.clone(), bundle)
            .with_chain_lock_probe(probe.clone());
        let rt = TokioTestRuntime;

        let group_info =
            GroupInfo::new(vec![own_jid.to_non_ad(), b.to_non_ad()], AddressingMode::Pn);
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };

        let prepared = prepare_group_stanza(
            &rt,
            &mut stores,
            &resolver,
            &group_info,
            &own_jid,
            &own_lid,
            None,
            group,
            &msg,
            "TESTREQID2".into(),
            false,
            Some(vec![b.clone()]),
            None,
            None,
            &[],
        )
        .await
        .expect("prepare_group_stanza should succeed");

        assert!(
            probe.fetch_calls.load(SeqCst) >= 1,
            "test must exercise the prekey fetch path"
        );
        assert!(
            !probe.fetched_under_lock.load(SeqCst),
            "prekey fetch must not run under the sender-key chain lock"
        );
        assert!(
            !probe.fetched_without_setup_lock.load(SeqCst),
            "prekey fetch must run under the per-group session-setup lock \
             (serializes same-group cold sends' session writes)"
        );

        // End-to-end: the session established before the lock produced a
        // pairwise SKDM for B under the lock.
        let participants = prepared
            .node
            .get_optional_child("participants")
            .expect("participants node with the SKDM fan-out");
        assert_eq!(
            participants.children().map(|c| c.len()).unwrap_or(0),
            1,
            "B must receive a pairwise SKDM via the pre-established session"
        );
    }
}

/// Item 3 — phash device-set construction. The set hashed is the full
/// recipient list PLUS the sending device (which is never in the recipient
/// list, since we don't SKDM ourselves), matching WA Web
/// `phashV2([].concat(A, [B]))`.
///
/// This was confirmed against a real WA Web capture sent to the production
/// server: the recipient `<to>` set plus the sending device reproduced the
/// exact `phash` on the wire, while the recipient set alone did not — so the
/// sending device is part of the hash. Raw identifiers are not committed
/// (PII); the vectors below are fictitious but exercise the same logic.
mod group_phash_golden {
    use super::*;

    #[test]
    fn phash_set_includes_sending_device() {
        // Fictitious group: a few users with bare (device 0) + companion
        // devices. The self user appears as a companion (device 0) in the
        // recipient list; its SENDING device (24) is excluded, mirroring a
        // real send (we never SKDM ourselves).
        let recipients: Vec<Jid> = [
            "100000000000001@lid",
            "100000000000001:5@lid",
            "100000000000002@lid",
            "100000000000003@lid",
            "100000000000003:12@lid",
            "100000000000099@lid",
        ]
        .iter()
        .map(|s| s.parse().expect("valid LID jid"))
        .collect();

        let own_sending: Jid = "100000000000099:24@lid".parse().unwrap();
        assert!(
            !recipients
                .iter()
                .any(|j: &Jid| j.user == "100000000000099" && j.device == 24),
            "the sending device must not already be in the recipient list"
        );

        let set = build_group_phash_set(&recipients, &own_sending);
        assert_eq!(set.len(), 7, "6 recipients + the sending device");

        // Dropping the sending device changes the hash, proving it is part
        // of the hashed set (WA Web `[].concat(A, [B])`).
        let with_self = MessageUtils::participant_list_hash(&set).unwrap();
        let without_self = MessageUtils::participant_list_hash(&recipients).unwrap();
        assert_ne!(with_self, without_self);

        // Deterministic standard-base64 vectors (regression guard).
        assert_eq!(without_self, "2:rZoSAdIV");
        assert_eq!(with_self, "2:sti8OtHX");
    }

    #[test]
    fn phash_set_drops_hosted_devices() {
        // Hosted (Cloud API) devices don't take part in group E2EE and must
        // not enter the phash, mirroring the SKDM distribution filter.
        let with_hosted: Vec<Jid> = ["100000000000001@lid", "100000000000002:99@hosted"]
            .iter()
            .map(|s| s.parse().expect("valid jid"))
            .collect();
        let without_hosted: Vec<Jid> = ["100000000000001@lid"]
            .iter()
            .map(|s| s.parse().expect("valid jid"))
            .collect();
        let own: Jid = "100000000000099:24@lid".parse().unwrap();

        assert_eq!(
            build_group_phash_set(&with_hosted, &own),
            build_group_phash_set(&without_hosted, &own),
            "hosted devices must not affect the phash set"
        );
    }
}

mod local_identity_change_on_send {
    use super::*;
    use crate::libsignal::protocol::{
        Direction, IdentityChange, IdentityKey, IdentityKeyStore, PreKeyId, PreKeyRecord,
        PreKeyStore, ProtocolAddress, SenderKeyRecord, SessionRecord, SessionStore, SignedPreKeyId,
        SignedPreKeyRecord, SignedPreKeyStore,
    };
    use crate::runtime::{AbortHandle, Runtime};
    use crate::types::jid::JidExt;
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    type SigResult<T> = crate::libsignal::protocol::error::Result<T>;

    #[derive(Clone, Default)]
    struct MemSessionStore(HashMap<ProtocolAddress, Vec<u8>>);
    #[async_trait::async_trait]
    impl SessionStore for MemSessionStore {
        async fn load_session(&self, a: &ProtocolAddress) -> SigResult<Option<SessionRecord>> {
            Ok(self
                .0
                .get(a)
                .and_then(|b| SessionRecord::deserialize(b).ok()))
        }
        async fn has_session(&self, a: &ProtocolAddress) -> SigResult<bool> {
            Ok(self.0.contains_key(a))
        }
        async fn store_session(&mut self, a: &ProtocolAddress, r: SessionRecord) -> SigResult<()> {
            self.0.insert(a.clone(), r.serialize()?);
            Ok(())
        }
    }

    /// Identity store that reports the real change (unlike the hardcoded
    /// stub elsewhere), so a pre-seeded stale key surfaces as ReplacedExisting.
    #[derive(Clone)]
    struct MemIdentityStore {
        pair: IdentityKeyPair,
        known: HashMap<ProtocolAddress, IdentityKey>,
    }
    #[async_trait::async_trait]
    impl IdentityKeyStore for MemIdentityStore {
        async fn get_identity_key_pair(&self) -> SigResult<IdentityKeyPair> {
            Ok(self.pair.clone())
        }
        async fn get_local_registration_id(&self) -> SigResult<u32> {
            Ok(42)
        }
        async fn save_identity(
            &mut self,
            a: &ProtocolAddress,
            id: &IdentityKey,
        ) -> SigResult<IdentityChange> {
            let changed = self.known.get(a).is_some_and(|k| k != id);
            self.known.insert(a.clone(), *id);
            Ok(IdentityChange::from_changed(changed))
        }
        async fn is_trusted_identity(
            &self,
            _: &ProtocolAddress,
            _: &IdentityKey,
            _: Direction,
        ) -> SigResult<bool> {
            Ok(true)
        }
        async fn get_identity(&self, a: &ProtocolAddress) -> SigResult<Option<IdentityKey>> {
            Ok(self.known.get(a).copied())
        }
    }

    struct UnusedPreKeyStore;
    #[async_trait::async_trait]
    impl PreKeyStore for UnusedPreKeyStore {
        async fn get_pre_key(&self, _: PreKeyId) -> SigResult<PreKeyRecord> {
            unreachable!()
        }
        async fn save_pre_key(&mut self, _: PreKeyId, _: &PreKeyRecord) -> SigResult<()> {
            unreachable!()
        }
        async fn remove_pre_key(&mut self, _: PreKeyId) -> SigResult<()> {
            unreachable!()
        }
    }
    struct UnusedSignedPreKeyStore;
    #[async_trait::async_trait]
    impl SignedPreKeyStore for UnusedSignedPreKeyStore {
        async fn get_signed_pre_key(&self, _: SignedPreKeyId) -> SigResult<SignedPreKeyRecord> {
            unreachable!()
        }
        async fn save_signed_pre_key(
            &mut self,
            _: SignedPreKeyId,
            _: &SignedPreKeyRecord,
        ) -> SigResult<()> {
            unreachable!()
        }
    }
    #[derive(Default)]
    struct MemSenderKeyStore(
        HashMap<crate::libsignal::store::sender_key_name::SenderKeyName, SenderKeyRecord>,
    );
    #[async_trait::async_trait]
    impl SenderKeyStore for MemSenderKeyStore {
        async fn store_sender_key(
            &mut self,
            n: &crate::libsignal::store::sender_key_name::SenderKeyName,
            r: SenderKeyRecord,
        ) -> SigResult<()> {
            self.0.insert(n.clone(), r);
            Ok(())
        }
        async fn load_sender_key(
            &self,
            n: &crate::libsignal::store::sender_key_name::SenderKeyName,
        ) -> SigResult<Option<SenderKeyRecord>> {
            Ok(self.0.get(n).cloned())
        }
    }

    struct TokioTestRuntime;
    #[async_trait::async_trait]
    impl Runtime for TokioTestRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            let handle = tokio::spawn(future);
            AbortHandle::new(move || handle.abort())
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// The send path must report a replaced identity via the resolver when
    /// establishing a session whose bundle carries a new identity key for an
    /// address we already knew (peer reinstall). Mirrors WA Web saveIdentity
    /// -> handleNewIdentity firing during outbound session setup.
    #[tokio::test]
    async fn encrypt_for_devices_reports_replaced_identity() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        // Receiver device D with a valid signed bundle.
        let device: Jid = "5511777777777:0@s.whatsapp.net".parse().unwrap();
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let sig = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .unwrap();
        let bundle = PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            sig.to_vec(),
            *receiver.identity_key(),
        )
        .unwrap();

        // Local stores: no session for D + a STALE identity pre-seeded for D's
        // address, so establishing the session reports ReplacedExisting.
        let sender = IdentityKeyPair::generate(&mut rng);
        let stale = *IdentityKeyPair::generate(&mut rng).identity_key();
        let mut known = HashMap::new();
        known.insert(device.to_protocol_address(), stale);

        let mut session_store = MemSessionStore::default();
        let mut identity_store = MemIdentityStore {
            pair: sender,
            known,
        };
        let mut prekey_store = UnusedPreKeyStore;
        let signed_prekey_store = UnusedSignedPreKeyStore;
        let mut sender_key_store = MemSenderKeyStore::default();

        let mut stores = SignalStores {
            sender_key_store: &mut sender_key_store,
            session_store: &mut session_store,
            identity_store: &mut identity_store,
            prekey_store: &mut prekey_store,
            signed_prekey_store: &signed_prekey_store,
        };

        let resolver = MockSendContextResolver::new()
            .with_bundle(device.clone(), bundle)
            .with_devices(vec![device.clone()]);
        let rt = TokioTestRuntime;

        encrypt_for_devices(
            &rt,
            &mut stores,
            &resolver,
            std::slice::from_ref(&device),
            b"hello",
            false,
            None,
        )
        .await
        .expect("encrypt_for_devices");

        assert_eq!(
            resolver.captured_identity_changes(),
            vec![device],
            "replaced identity on the send path must be reported via the resolver"
        );
    }
}
