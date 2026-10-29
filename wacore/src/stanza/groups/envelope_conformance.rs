//! Auto-generated isolated official group envelope conformance cases (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Original callback SHA-256 c795544e7e6f20cf484cbac4c40ccfe731b4460f31b7ae4863e6ab3bab1bb4d9, module-relative bytes 10592..22915.
//! Original XML/Wap/JID/time methods run in the fixture harness. Account facts, country/Wid/binary bridges and telemetry are mocked.
//! Only empty/marker/unknown/suspended actions are exercised here; complex actions have separate conformance cases.

use super::{GroupParseContext, GroupParseError, parse_normalized_group_notification};
use crate::stanza::groups::actions_normalized::NormalizedGroupAction;
use wacore_binary::{Jid, builder::NodeBuilder};

#[test]
fn original_group_envelope_callback_matches_rust_projection() {
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = None;
        let me_pn: Option<Jid> = None;
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "absent-to-does-not-require-local-lid.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "absent-to-does-not-require-local-lid"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "absent-to-does-not-require-local-lid.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "absent-to-does-not-require-local-lid.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "absent-to-does-not-require-local-lid.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "absent-to-does-not-require-local-lid.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "absent-to-does-not-require-local-lid.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "absent-to-does-not-require-local-lid.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "absent-to-does-not-require-local-lid.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "absent-to-does-not-require-local-lid.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "absent-to-does-not-require-local-lid"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "absent-to-does-not-require-local-lid.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "absent-to-does-not-require-local-lid.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("to", "100001:2@lid")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "exact-local-lid-device.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "exact-local-lid-device"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "exact-local-lid-device.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "exact-local-lid-device.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "exact-local-lid-device.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "exact-local-lid-device.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "exact-local-lid-device.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "exact-local-lid-device.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "exact-local-lid-device.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "exact-local-lid-device.hasIncompleteParticipantInformation"
        );
        assert_eq!(parsed.timestamp, 123_i64, "exact-local-lid-device");
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "exact-local-lid-device.caught PN diagnostics"
        );
        assert_eq!(parsed.actions.len(), 0, "exact-local-lid-device.actions");
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("to", "200002:2@s.whatsapp.net")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "exact-local-pn-device.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "exact-local-pn-device"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "exact-local-pn-device.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "exact-local-pn-device.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "exact-local-pn-device.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "exact-local-pn-device.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "exact-local-pn-device.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "exact-local-pn-device.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "exact-local-pn-device.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "exact-local-pn-device.hasIncompleteParticipantInformation"
        );
        assert_eq!(parsed.timestamp, 123_i64, "exact-local-pn-device");
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "exact-local-pn-device.caught PN diagnostics"
        );
        assert_eq!(parsed.actions.len(), 0, "exact-local-pn-device.actions");
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("to", "100001:3@lid")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Destination,
            "same-account-other-device-is-not-destination"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("id", "synthetic")
            .attr("to", "100001:3@lid")
            .attr("type", "w:gp2")
            .children(vec![NodeBuilder::new("future_action").build()])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Destination,
            "destination-error-precedes-missing-from-time-and-unknown-action"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("from"),
            "missing-from-is-required"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "participant-is-optional.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "participant-is-optional"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "participant-is-optional.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "participant-is-optional.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "participant-is-optional.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "participant-is-optional.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "participant-is-optional.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "participant-is-optional.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "participant-is-optional.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "participant-is-optional.hasIncompleteParticipantInformation"
        );
        assert_eq!(parsed.timestamp, 123_i64, "participant-is-optional");
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "participant-is-optional.caught PN diagnostics"
        );
        assert_eq!(parsed.actions.len(), 0, "participant-is-optional.actions");
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "123456@g.us")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("participant"),
            "invalid-participant-is-not-a-user"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("t"),
            "missing-time-is-required"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![
                NodeBuilder::new("missing_participant_identification").build(),
            ])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("id"),
            "missing-id-is-required-after-action-processing"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "100001@lid")
            .attr("participant_pn", "100002@lid")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "invalid-participant-pn-lid-is-caught-locally.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "invalid-participant-pn-lid-is-caught-locally"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            Some("100001@lid"),
            "invalid-participant-pn-lid-is-caught-locally.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "invalid-participant-pn-lid-is-caught-locally.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "invalid-participant-pn-lid-is-caught-locally.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "invalid-participant-pn-lid-is-caught-locally.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "invalid-participant-pn-lid-is-caught-locally.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "invalid-participant-pn-lid-is-caught-locally.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "invalid-participant-pn-lid-is-caught-locally.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "invalid-participant-pn-lid-is-caught-locally.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "invalid-participant-pn-lid-is-caught-locally"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "invalid-participant-pn-lid-is-caught-locally.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "invalid-participant-pn-lid-is-caught-locally.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_pn", "200002:2@s.whatsapp.net")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "invalid-participant-pn-device-is-caught-locally.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "invalid-participant-pn-device-is-caught-locally"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "invalid-participant-pn-device-is-caught-locally.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "invalid-participant-pn-device-is-caught-locally.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "invalid-participant-pn-device-is-caught-locally.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "invalid-participant-pn-device-is-caught-locally"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "invalid-participant-pn-device-is-caught-locally.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "invalid-participant-pn-device-is-caught-locally.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_pn", "200003@s.whatsapp.net")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "valid-participant-pn-is-retained.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "valid-participant-pn-is-retained"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "valid-participant-pn-is-retained.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some("200003@s.whatsapp.net"),
            "valid-participant-pn-is-retained.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "valid-participant-pn-is-retained.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "valid-participant-pn-is-retained.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "valid-participant-pn-is-retained.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "valid-participant-pn-is-retained.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "valid-participant-pn-is-retained.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "valid-participant-pn-is-retained.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "valid-participant-pn-is-retained"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "valid-participant-pn-is-retained.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "valid-participant-pn-is-retained.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("addressing_mode", "lid")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_country_code", "US")
            .attr("participant_username", "synthetic_username")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![
                NodeBuilder::new("missing_participant_identification").build(),
            ])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "disabled-username-gate-drops-marker-action-only.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "disabled-username-gate-drops-marker-action-only"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "disabled-username-gate-drops-marker-action-only.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, true,
            "disabled-username-gate-drops-marker-action-only.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "disabled-username-gate-drops-marker-action-only.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "disabled-username-gate-drops-marker-action-only"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "disabled-username-gate-drops-marker-action-only.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "disabled-username-gate-drops-marker-action-only.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("addressing_mode", "lid")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_country_code", "US")
            .attr("participant_username", "synthetic_username")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![
                NodeBuilder::new("missing_participant_identification").build(),
            ])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = true;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: true,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "enabled-username-gate-stamps-incomplete-marker.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "enabled-username-gate-stamps-incomplete-marker"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "enabled-username-gate-stamps-incomplete-marker.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "enabled-username-gate-stamps-incomplete-marker.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            Some("synthetic_username"),
            "enabled-username-gate-stamps-incomplete-marker.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            Some("US"),
            "enabled-username-gate-stamps-incomplete-marker.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "enabled-username-gate-stamps-incomplete-marker.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "enabled-username-gate-stamps-incomplete-marker.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, true,
            "enabled-username-gate-stamps-incomplete-marker.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, true,
            "enabled-username-gate-stamps-incomplete-marker.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "enabled-username-gate-stamps-incomplete-marker"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "enabled-username-gate-stamps-incomplete-marker.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "enabled-username-gate-stamps-incomplete-marker.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![NodeBuilder::new("future_action").build()])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::UnknownAction("future_action".into()),
            "unknown-action-is-an-error"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![NodeBuilder::new("future_action").build()])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::UnknownAction("future_action".into()),
            "unknown-action-precedes-final-id"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "4294967297")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "timestamp-wraps-to-signed32.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "timestamp-wraps-to-signed32"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "timestamp-wraps-to-signed32.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "timestamp-wraps-to-signed32.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "timestamp-wraps-to-signed32.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "timestamp-wraps-to-signed32.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "timestamp-wraps-to-signed32.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "timestamp-wraps-to-signed32.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "timestamp-wraps-to-signed32.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "timestamp-wraps-to-signed32.hasIncompleteParticipantInformation"
        );
        assert_eq!(parsed.timestamp, 1_i64, "timestamp-wraps-to-signed32");
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "timestamp-wraps-to-signed32.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "timestamp-wraps-to-signed32.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "2147483648")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "timestamp-reserved-minimum-is-clamped.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "timestamp-reserved-minimum-is-clamped"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "timestamp-reserved-minimum-is-clamped.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "timestamp-reserved-minimum-is-clamped.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "timestamp-reserved-minimum-is-clamped.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, -2147483647_i64,
            "timestamp-reserved-minimum-is-clamped"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "timestamp-reserved-minimum-is-clamped.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "timestamp-reserved-minimum-is-clamped.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![NodeBuilder::new("suspended").build()])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "simple-unit-action-is-normalized.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "simple-unit-action-is-normalized"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "simple-unit-action-is-normalized.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "simple-unit-action-is-normalized.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "simple-unit-action-is-normalized.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "simple-unit-action-is-normalized.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "simple-unit-action-is-normalized.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "simple-unit-action-is-normalized.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "simple-unit-action-is-normalized.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "simple-unit-action-is-normalized.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "simple-unit-action-is-normalized"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "simple-unit-action-is-normalized.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            1,
            "simple-unit-action-is-normalized.actions"
        );
        assert!(
            matches!(&parsed.actions[0],NormalizedGroupAction::Suspend{value} if *value),
            "simple-unit-action-is-normalized"
        );
    }
    {
        let node = NodeBuilder::new("iq")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("to", "200002:2@s.whatsapp.net")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = None;
        let me_pn: Option<Jid> = None;
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Tag,
            "tag-error-precedes-local-device-access"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "absent-children-content-is-empty-actions.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "absent-children-content-is-empty-actions"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "absent-children-content-is-empty-actions.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "absent-children-content-is-empty-actions.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "absent-children-content-is-empty-actions.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "absent-children-content-is-empty-actions.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "absent-children-content-is-empty-actions.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "absent-children-content-is-empty-actions.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "absent-children-content-is-empty-actions.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "absent-children-content-is-empty-actions.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "absent-children-content-is-empty-actions"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "absent-children-content-is-empty-actions.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "absent-children-content-is-empty-actions.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .bytes(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("children"),
            "empty-byte-content-is-not-children-before-final-id"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .string_content("")
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("children"),
            "empty-string-content-is-not-children"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .attr("to", "200002:2@s.whatsapp.net")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = None;
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::MissingContext("me_device_lid"),
            "to-requires-lid-fact-before-matching-pn"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "future_time")
            .attr("type", "w:gp2")
            .children(vec![NodeBuilder::new("future_action").build()])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap_err(),
            GroupParseError::Field("t"),
            "invalid-time-precedes-unknown-action"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@bot")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "bot-participant-is-a-supported-user-class.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "bot-participant-is-a-supported-user-class"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            Some("200002@bot"),
            "bot-participant-is-a-supported-user-class.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "bot-participant-is-a-supported-user-class.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "bot-participant-is-a-supported-user-class.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "bot-participant-is-a-supported-user-class.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "bot-participant-is-a-supported-user-class.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "bot-participant-is-a-supported-user-class.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "bot-participant-is-a-supported-user-class.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "bot-participant-is-a-supported-user-class.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "bot-participant-is-a-supported-user-class"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "bot-participant-is-a-supported-user-class.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "bot-participant-is-a-supported-user-class.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_pn", "100002@s.whatsapp.net")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = false;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "pn-prefix-outside-official-user-class-is-caught.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "pn-prefix-outside-official-user-class-is-caught"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "pn-prefix-outside-official-user-class-is-caught.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "pn-prefix-outside-official-user-class-is-caught.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "pn-prefix-outside-official-user-class-is-caught.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "pn-prefix-outside-official-user-class-is-caught"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            1,
            "pn-prefix-outside-official-user-class-is-caught.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "pn-prefix-outside-official-user-class-is-caught.actions"
        );
    }
    {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant_country_code", "future_country")
            .attr("t", "123")
            .attr("type", "w:gp2")
            .children(vec![])
            .build();
        let me_lid: Option<Jid> = Some("100001:2@lid".parse::<Jid>().unwrap());
        let me_pn: Option<Jid> = Some("200002:2@s.whatsapp.net".parse::<Jid>().unwrap());
        let is_me = |jid: &Jid| {
            let _ = jid;
            false
        };
        let country_enabled = true;
        let country = |value: &str| {
            assert!(country_enabled, "disabled country lookup");
            match value {
                "US" | "BR" => Some(value.to_owned()),
                _ => None,
            }
        };
        let context = GroupParseContext {
            me_device_lid: me_lid.as_ref(),
            me_device_pn: me_pn.as_ref(),
            username_displayed: true,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_normalized_group_notification(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.external_id.as_str(),
            "synthetic",
            "unknown-country-is-undefined-with-username-gate-enabled.externalId"
        );
        assert_eq!(
            parsed.chat_id.to_string(),
            "123456@g.us",
            "unknown-country-is-undefined-with-username-gate-enabled"
        );
        assert_eq!(
            parsed.author.as_ref().map(ToString::to_string).as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.author"
        );
        assert_eq!(
            parsed
                .author_phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.authorPhoneNumber"
        );
        assert_eq!(
            parsed.author_username.as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.authorUsername"
        );
        assert_eq!(
            parsed.author_username_country_code.as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.authorUsernameCountryCode"
        );
        assert_eq!(
            parsed.pushname.as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.pushname"
        );
        assert_eq!(
            parsed.offline.as_deref(),
            None,
            "unknown-country-is-undefined-with-username-gate-enabled.offline"
        );
        assert_eq!(
            parsed.is_lid_addressing_mode, false,
            "unknown-country-is-undefined-with-username-gate-enabled.isLidAddressingMode"
        );
        assert_eq!(
            parsed.has_incomplete_participant_information, false,
            "unknown-country-is-undefined-with-username-gate-enabled.hasIncompleteParticipantInformation"
        );
        assert_eq!(
            parsed.timestamp, 123_i64,
            "unknown-country-is-undefined-with-username-gate-enabled"
        );
        assert_eq!(
            parsed.diagnostics.len(),
            0,
            "unknown-country-is-undefined-with-username-gate-enabled.caught PN diagnostics"
        );
        assert_eq!(
            parsed.actions.len(),
            0,
            "unknown-country-is-undefined-with-username-gate-enabled.actions"
        );
    }
}
