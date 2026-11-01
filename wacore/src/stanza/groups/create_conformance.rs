//! Auto-generated isolated official CREATE conformance cases (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
//! Original handler module SHA-256 d1641fad966fd3b32d6b9859e074f804526e67181584811364774047208c273d.
//! The complete original groupNotificationParser callback and official XML/Wap/JID/time/enum dependencies run in the versioned fixture harness.
//! Effective account flags/country registry, Wid conversion, binary adapter, native-Error bridge and telemetry are mocked.

use super::{CreateParseError, GroupCreateContext, parse_group_create};
use crate::stanza::groups::{GroupHistorySentState, GroupParticipantType};
use wacore_binary::{Jid, builder::NodeBuilder};

#[test]
fn isolated_official_create_helpers_match_rust_projection() {
    {
        let node = NodeBuilder::new("create")
            .attr("reason", "accept")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "200002@s.whatsapp.net")
                            .attr("username", "ungated")
                            .build(),
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .attr("type", "superadmin")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(
            !(parsed.is_new_group),
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "default-and-unconditional-participant-username.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.creation, 123_i64,
            "default-and-unconditional-participant-username"
        );
        assert!(
            !(group.restrict),
            "default-and-unconditional-participant-username.restrict"
        );
        assert!(
            !(group.announce),
            "default-and-unconditional-participant-username.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "default-and-unconditional-participant-username.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "default-and-unconditional-participant-username.support"
        );
        assert!(
            !(group.is_parent_group),
            "default-and-unconditional-participant-username.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "default-and-unconditional-participant-username.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "default-and-unconditional-participant-username.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "default-and-unconditional-participant-username.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "default-and-unconditional-participant-username.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "default-and-unconditional-participant-username.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "default-and-unconditional-participant-username.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "default-and-unconditional-participant-username.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "default-and-unconditional-participant-username.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "default-and-unconditional-participant-username.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "default-and-unconditional-participant-username.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "default-and-unconditional-participant-username.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "default-and-unconditional-participant-username.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "default-and-unconditional-participant-username.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "default-and-unconditional-participant-username.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "default-and-unconditional-participant-username.subjectTime"
        );
        assert_eq!(
            group.size, None,
            "default-and-unconditional-participant-username.size"
        );
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "default-and-unconditional-participant-username.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "default-and-unconditional-participant-username.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "default-and-unconditional-participant-username.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            group.participants.len(),
            2,
            "default-and-unconditional-participant-username"
        );
        let participant = &group.participants[0];
        assert_eq!(
            participant.id.to_string(),
            "200002@s.whatsapp.net",
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.username.as_deref(),
            Some("ungated"),
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert!(
            !(participant.is_admin),
            "default-and-unconditional-participant-username"
        );
        assert!(
            !(participant.is_super_admin),
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::Participant,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.join_time, None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            None,
            "default-and-unconditional-participant-username"
        );
        let participant = &group.participants[1];
        assert_eq!(
            participant.id.to_string(),
            "100001@lid",
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.username.as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "default-and-unconditional-participant-username"
        );
        assert!(
            participant.is_admin,
            "default-and-unconditional-participant-username"
        );
        assert!(
            participant.is_super_admin,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::SuperAdmin,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.join_time, None,
            "default-and-unconditional-participant-username"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            None,
            "default-and-unconditional-participant-username"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("creator", "bad")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("type", "future_role")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &context).unwrap_err(),
            CreateParseError::Field("type"),
            "unknown-role-precedes-creator"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("creator_country_code", "future")
                    .attr("creator_username", "gated")
                    .attr("s_o_username", "gated")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .attr("join_time", "invalid")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &context).unwrap_err(),
            CreateParseError::Field("join_time"),
            "invalid-participant-join-time-error"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .attr("type", "new")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("ephemeral")
                            .attr("expiration", "12seconds")
                            .attr("trigger", "unknown")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(parsed.is_new_group, "new-group-unknown-trigger");
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "new-group-unknown-trigger.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "new-group-unknown-trigger"
        );
        assert_eq!(group.creation, 123_i64, "new-group-unknown-trigger");
        assert!(!(group.restrict), "new-group-unknown-trigger.restrict");
        assert!(!(group.announce), "new-group-unknown-trigger.announce");
        assert!(
            !(group.no_frequently_forwarded),
            "new-group-unknown-trigger.noFrequentlyForwarded"
        );
        assert!(!(group.support), "new-group-unknown-trigger.support");
        assert!(
            !(group.is_parent_group),
            "new-group-unknown-trigger.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "new-group-unknown-trigger.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "new-group-unknown-trigger.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "new-group-unknown-trigger.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "new-group-unknown-trigger.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "new-group-unknown-trigger.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "new-group-unknown-trigger.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "new-group-unknown-trigger.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "new-group-unknown-trigger.groupSafetyCheck"
        );
        assert!(!(group.has_capi), "new-group-unknown-trigger.hasCapi");
        assert!(
            !(group.limit_sharing_enabled),
            "new-group-unknown-trigger.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "new-group-unknown-trigger.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "new-group-unknown-trigger.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "new-group-unknown-trigger.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "new-group-unknown-trigger.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "new-group-unknown-trigger.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "new-group-unknown-trigger.subjectTime"
        );
        assert_eq!(group.size, None, "new-group-unknown-trigger.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "new-group-unknown-trigger.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "new-group-unknown-trigger.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "new-group-unknown-trigger.memberShareGroupHistoryMode"
        );
        assert!(group.ephemerality.is_some(), "new-group-unknown-trigger");
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            Some(12_i64),
            "new-group-unknown-trigger"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "new-group-unknown-trigger"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            Some(false),
            "new-group-unknown-trigger"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "new-group-unknown-trigger"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "new-group-unknown-trigger"
        );
        assert_eq!(group.participants.len(), 0, "new-group-unknown-trigger");
    }
    {
        let node = NodeBuilder::new("create")
            .attr("type", "existing")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("ephemeral")
                            .attr("expiration", "12seconds")
                            .attr("trigger", "unknown")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(!(parsed.is_new_group), "existing-group-unknown-trigger");
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "existing-group-unknown-trigger.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "existing-group-unknown-trigger"
        );
        assert_eq!(group.creation, 123_i64, "existing-group-unknown-trigger");
        assert!(!(group.restrict), "existing-group-unknown-trigger.restrict");
        assert!(!(group.announce), "existing-group-unknown-trigger.announce");
        assert!(
            !(group.no_frequently_forwarded),
            "existing-group-unknown-trigger.noFrequentlyForwarded"
        );
        assert!(!(group.support), "existing-group-unknown-trigger.support");
        assert!(
            !(group.is_parent_group),
            "existing-group-unknown-trigger.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "existing-group-unknown-trigger.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "existing-group-unknown-trigger.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "existing-group-unknown-trigger.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "existing-group-unknown-trigger.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "existing-group-unknown-trigger.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "existing-group-unknown-trigger.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "existing-group-unknown-trigger.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "existing-group-unknown-trigger.groupSafetyCheck"
        );
        assert!(!(group.has_capi), "existing-group-unknown-trigger.hasCapi");
        assert!(
            !(group.limit_sharing_enabled),
            "existing-group-unknown-trigger.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "existing-group-unknown-trigger.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "existing-group-unknown-trigger.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "existing-group-unknown-trigger.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "existing-group-unknown-trigger.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "existing-group-unknown-trigger.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "existing-group-unknown-trigger.subjectTime"
        );
        assert_eq!(group.size, None, "existing-group-unknown-trigger.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "existing-group-unknown-trigger.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "existing-group-unknown-trigger.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "existing-group-unknown-trigger.memberShareGroupHistoryMode"
        );
        assert!(
            group.ephemerality.is_some(),
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            Some(12_i64),
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            Some("unknown_groups"),
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            Some(false),
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "existing-group-unknown-trigger"
        );
        assert_eq!(
            group.participants.len(),
            0,
            "existing-group-unknown-trigger"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .attr("reason", "default_sub_group_admin_add")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("linked_parent")
                            .attr("jid", "123456@g.us")
                            .build(),
                        NodeBuilder::new("parent")
                            .attr("default_membership_approval_mode", "request_required")
                            .build(),
                        NodeBuilder::new("limit_sharing_enabled").build(),
                        NodeBuilder::new("member_link_mode")
                            .bytes(vec![102, 117, 116, 117, 114, 101, 95, 109, 111, 100, 101])
                            .build(),
                        NodeBuilder::new("member_add_mode")
                            .bytes(vec![
                                97, 108, 108, 95, 109, 101, 109, 98, 101, 114, 95, 97, 100, 100,
                            ])
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new("auto_add_disabled").build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: true,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(!(parsed.is_new_group), "parent-mode-flags-and-reason");
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            Some("default_subgroup_admin_add"),
            "parent-mode-flags-and-reason.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "parent-mode-flags-and-reason.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "parent-mode-flags-and-reason"
        );
        assert_eq!(group.creation, 123_i64, "parent-mode-flags-and-reason");
        assert!(!(group.restrict), "parent-mode-flags-and-reason.restrict");
        assert!(!(group.announce), "parent-mode-flags-and-reason.announce");
        assert!(
            !(group.no_frequently_forwarded),
            "parent-mode-flags-and-reason.noFrequentlyForwarded"
        );
        assert!(!(group.support), "parent-mode-flags-and-reason.support");
        assert!(
            group.is_parent_group,
            "parent-mode-flags-and-reason.isParentGroup"
        );
        assert!(
            group.is_parent_group_closed,
            "parent-mode-flags-and-reason.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "parent-mode-flags-and-reason.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "parent-mode-flags-and-reason.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "parent-mode-flags-and-reason.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "parent-mode-flags-and-reason.allowNonAdminSubGroupCreation"
        );
        assert!(
            group.general_chat_auto_add_disabled,
            "parent-mode-flags-and-reason.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "parent-mode-flags-and-reason.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "parent-mode-flags-and-reason.groupSafetyCheck"
        );
        assert!(!(group.has_capi), "parent-mode-flags-and-reason.hasCapi");
        assert!(
            !(group.limit_sharing_enabled),
            "parent-mode-flags-and-reason.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "parent-mode-flags-and-reason.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "parent-mode-flags-and-reason.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "parent-mode-flags-and-reason.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "parent-mode-flags-and-reason.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some("123456@g.us"),
            "parent-mode-flags-and-reason.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "parent-mode-flags-and-reason.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "parent-mode-flags-and-reason.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "parent-mode-flags-and-reason.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "parent-mode-flags-and-reason.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "parent-mode-flags-and-reason.subjectTime"
        );
        assert_eq!(group.size, None, "parent-mode-flags-and-reason.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "parent-mode-flags-and-reason.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            Some("all_member_add"),
            "parent-mode-flags-and-reason.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "parent-mode-flags-and-reason.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "parent-mode-flags-and-reason"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "parent-mode-flags-and-reason"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "parent-mode-flags-and-reason"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "parent-mode-flags-and-reason"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "parent-mode-flags-and-reason"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "parent-mode-flags-and-reason"
        );
        assert_eq!(group.participants.len(), 0, "parent-mode-flags-and-reason");
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "4294967297")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .attr("join_time", "2147483648")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: true,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(
            !(parsed.is_new_group),
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "time-cast-wraps-before-normalization.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.creation, 1_i64,
            "time-cast-wraps-before-normalization"
        );
        assert!(
            !(group.restrict),
            "time-cast-wraps-before-normalization.restrict"
        );
        assert!(
            !(group.announce),
            "time-cast-wraps-before-normalization.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "time-cast-wraps-before-normalization.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "time-cast-wraps-before-normalization.support"
        );
        assert!(
            !(group.is_parent_group),
            "time-cast-wraps-before-normalization.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "time-cast-wraps-before-normalization.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "time-cast-wraps-before-normalization.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "time-cast-wraps-before-normalization.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "time-cast-wraps-before-normalization.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "time-cast-wraps-before-normalization.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "time-cast-wraps-before-normalization.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "time-cast-wraps-before-normalization.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "time-cast-wraps-before-normalization.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "time-cast-wraps-before-normalization.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "time-cast-wraps-before-normalization.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "time-cast-wraps-before-normalization.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "time-cast-wraps-before-normalization.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "time-cast-wraps-before-normalization.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "time-cast-wraps-before-normalization.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "time-cast-wraps-before-normalization.subjectTime"
        );
        assert_eq!(
            group.size, None,
            "time-cast-wraps-before-normalization.size"
        );
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "time-cast-wraps-before-normalization.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "time-cast-wraps-before-normalization.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "time-cast-wraps-before-normalization.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            group.participants.len(),
            1,
            "time-cast-wraps-before-normalization"
        );
        let participant = &group.participants[0];
        assert_eq!(
            participant.id.to_string(),
            "100001@lid",
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.username.as_deref(),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "time-cast-wraps-before-normalization"
        );
        assert!(
            !(participant.is_admin),
            "time-cast-wraps-before-normalization"
        );
        assert!(
            !(participant.is_super_admin),
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::Participant,
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.join_time,
            Some(-2147483647_i64),
            "time-cast-wraps-before-normalization"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            None,
            "time-cast-wraps-before-normalization"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("creator", "invalid")
                    .attr("subject", "synthetic group")
                    .children(vec![
                        NodeBuilder::new("description")
                            .children(vec![NodeBuilder::new("body").bytes(vec![]).build()])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &context).unwrap_err(),
            CreateParseError::Field("id"),
            "empty-byte-description-still-requires-id-before-creator"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic")
                    .children(vec![
                        NodeBuilder::new("member_share_group_history_mode")
                            .bytes(vec![97, 100, 109, 105, 110, 95, 115, 104, 97, 114, 101])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(!(parsed.is_new_group), "history-share-mode-admin_share");
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "history-share-mode-admin_share.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic",
            "history-share-mode-admin_share"
        );
        assert_eq!(group.creation, 123_i64, "history-share-mode-admin_share");
        assert!(!(group.restrict), "history-share-mode-admin_share.restrict");
        assert!(!(group.announce), "history-share-mode-admin_share.announce");
        assert!(
            !(group.no_frequently_forwarded),
            "history-share-mode-admin_share.noFrequentlyForwarded"
        );
        assert!(!(group.support), "history-share-mode-admin_share.support");
        assert!(
            !(group.is_parent_group),
            "history-share-mode-admin_share.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "history-share-mode-admin_share.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "history-share-mode-admin_share.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "history-share-mode-admin_share.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "history-share-mode-admin_share.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "history-share-mode-admin_share.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "history-share-mode-admin_share.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "history-share-mode-admin_share.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "history-share-mode-admin_share.groupSafetyCheck"
        );
        assert!(!(group.has_capi), "history-share-mode-admin_share.hasCapi");
        assert!(
            !(group.limit_sharing_enabled),
            "history-share-mode-admin_share.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "history-share-mode-admin_share.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-admin_share.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "history-share-mode-admin_share.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "history-share-mode-admin_share.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "history-share-mode-admin_share.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "history-share-mode-admin_share.subjectTime"
        );
        assert_eq!(group.size, None, "history-share-mode-admin_share.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-admin_share.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-admin_share.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            Some("admin_share"),
            "history-share-mode-admin_share.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "history-share-mode-admin_share"
        );
        assert_eq!(
            group.participants.len(),
            0,
            "history-share-mode-admin_share"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic")
                    .children(vec![
                        NodeBuilder::new("member_share_group_history_mode")
                            .bytes(vec![
                                97, 108, 108, 95, 109, 101, 109, 98, 101, 114, 95, 115, 104, 97,
                                114, 101,
                            ])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(
            !(parsed.is_new_group),
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "history-share-mode-all_member_share.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic",
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.creation, 123_i64,
            "history-share-mode-all_member_share"
        );
        assert!(
            !(group.restrict),
            "history-share-mode-all_member_share.restrict"
        );
        assert!(
            !(group.announce),
            "history-share-mode-all_member_share.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "history-share-mode-all_member_share.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "history-share-mode-all_member_share.support"
        );
        assert!(
            !(group.is_parent_group),
            "history-share-mode-all_member_share.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "history-share-mode-all_member_share.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "history-share-mode-all_member_share.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "history-share-mode-all_member_share.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "history-share-mode-all_member_share.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "history-share-mode-all_member_share.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "history-share-mode-all_member_share.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "history-share-mode-all_member_share.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "history-share-mode-all_member_share.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "history-share-mode-all_member_share.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "history-share-mode-all_member_share.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "history-share-mode-all_member_share.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-all_member_share.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "history-share-mode-all_member_share.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "history-share-mode-all_member_share.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "history-share-mode-all_member_share.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "history-share-mode-all_member_share.subjectTime"
        );
        assert_eq!(group.size, None, "history-share-mode-all_member_share.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-all_member_share.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-all_member_share.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            Some("all_member_share"),
            "history-share-mode-all_member_share.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "history-share-mode-all_member_share"
        );
        assert_eq!(
            group.participants.len(),
            0,
            "history-share-mode-all_member_share"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic")
                    .children(vec![
                        NodeBuilder::new("member_share_group_history_mode")
                            .bytes(vec![102, 117, 116, 117, 114, 101, 95, 109, 111, 100, 101])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(!(parsed.is_new_group), "history-share-mode-future_mode");
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "history-share-mode-future_mode.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic",
            "history-share-mode-future_mode"
        );
        assert_eq!(group.creation, 123_i64, "history-share-mode-future_mode");
        assert!(!(group.restrict), "history-share-mode-future_mode.restrict");
        assert!(!(group.announce), "history-share-mode-future_mode.announce");
        assert!(
            !(group.no_frequently_forwarded),
            "history-share-mode-future_mode.noFrequentlyForwarded"
        );
        assert!(!(group.support), "history-share-mode-future_mode.support");
        assert!(
            !(group.is_parent_group),
            "history-share-mode-future_mode.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "history-share-mode-future_mode.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "history-share-mode-future_mode.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "history-share-mode-future_mode.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "history-share-mode-future_mode.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "history-share-mode-future_mode.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "history-share-mode-future_mode.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "history-share-mode-future_mode.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "history-share-mode-future_mode.groupSafetyCheck"
        );
        assert!(!(group.has_capi), "history-share-mode-future_mode.hasCapi");
        assert!(
            !(group.limit_sharing_enabled),
            "history-share-mode-future_mode.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "history-share-mode-future_mode.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-share-mode-future_mode.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "history-share-mode-future_mode.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "history-share-mode-future_mode.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "history-share-mode-future_mode.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "history-share-mode-future_mode.subjectTime"
        );
        assert_eq!(group.size, None, "history-share-mode-future_mode.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-future_mode.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-future_mode.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-share-mode-future_mode.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "history-share-mode-future_mode"
        );
        assert_eq!(
            group.participants.len(),
            0,
            "history-share-mode-future_mode"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("subject", "synthetic")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("group_history_sent_state", "HISTORY_SENT")
                            .attr("jid", "100001@lid")
                            .attr("join_time", "123")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(
            !(parsed.is_new_group),
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "history-is-read-without-feature-gate.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic",
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.creation, 123_i64,
            "history-is-read-without-feature-gate"
        );
        assert!(
            !(group.restrict),
            "history-is-read-without-feature-gate.restrict"
        );
        assert!(
            !(group.announce),
            "history-is-read-without-feature-gate.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "history-is-read-without-feature-gate.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "history-is-read-without-feature-gate.support"
        );
        assert!(
            !(group.is_parent_group),
            "history-is-read-without-feature-gate.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "history-is-read-without-feature-gate.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "history-is-read-without-feature-gate.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "history-is-read-without-feature-gate.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "history-is-read-without-feature-gate.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "history-is-read-without-feature-gate.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "history-is-read-without-feature-gate.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "history-is-read-without-feature-gate.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "history-is-read-without-feature-gate.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "history-is-read-without-feature-gate.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "history-is-read-without-feature-gate.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "history-is-read-without-feature-gate.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "history-is-read-without-feature-gate.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "history-is-read-without-feature-gate.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "history-is-read-without-feature-gate.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "history-is-read-without-feature-gate.subjectTime"
        );
        assert_eq!(
            group.size, None,
            "history-is-read-without-feature-gate.size"
        );
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-is-read-without-feature-gate.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-is-read-without-feature-gate.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "history-is-read-without-feature-gate.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            group.participants.len(),
            1,
            "history-is-read-without-feature-gate"
        );
        let participant = &group.participants[0];
        assert_eq!(
            participant.id.to_string(),
            "100001@lid",
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.username.as_deref(),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "history-is-read-without-feature-gate"
        );
        assert!(
            !(participant.is_admin),
            "history-is-read-without-feature-gate"
        );
        assert!(
            !(participant.is_super_admin),
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::Participant,
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.join_time,
            Some(123_i64),
            "history-is-read-without-feature-gate"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            Some(2_i64),
            "history-is-read-without-feature-gate"
        );
    }
    {
        let node = NodeBuilder::new("create")
            .children(vec![
                NodeBuilder::new("group")
                    .attr("creation", "123")
                    .attr("creator_country_code", "future")
                    .attr("creator_username", "gated")
                    .attr("s_o_username", "gated")
                    .attr("subject", "synthetic")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .attr("join_time", "123")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
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
        let context = GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert!(
            !(parsed.is_new_group),
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "disabled-creator-country-and-username-success.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic",
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.creation, 123_i64,
            "disabled-creator-country-and-username-success"
        );
        assert!(
            !(group.restrict),
            "disabled-creator-country-and-username-success.restrict"
        );
        assert!(
            !(group.announce),
            "disabled-creator-country-and-username-success.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "disabled-creator-country-and-username-success.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "disabled-creator-country-and-username-success.support"
        );
        assert!(
            !(group.is_parent_group),
            "disabled-creator-country-and-username-success.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "disabled-creator-country-and-username-success.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "disabled-creator-country-and-username-success.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "disabled-creator-country-and-username-success.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "disabled-creator-country-and-username-success.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "disabled-creator-country-and-username-success.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "disabled-creator-country-and-username-success.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "disabled-creator-country-and-username-success.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "disabled-creator-country-and-username-success.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "disabled-creator-country-and-username-success.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "disabled-creator-country-and-username-success.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "disabled-creator-country-and-username-success.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "disabled-creator-country-and-username-success.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "disabled-creator-country-and-username-success.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "disabled-creator-country-and-username-success.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "disabled-creator-country-and-username-success.subjectTime"
        );
        assert_eq!(
            group.size, None,
            "disabled-creator-country-and-username-success.size"
        );
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "disabled-creator-country-and-username-success.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "disabled-creator-country-and-username-success.memberAddMode"
        );
        assert_eq!(
            group.member_share_history_mode.as_ref().map(|v| v.as_str()),
            None,
            "disabled-creator-country-and-username-success.memberShareGroupHistoryMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            group.participants.len(),
            1,
            "disabled-creator-country-and-username-success"
        );
        let participant = &group.participants[0];
        assert_eq!(
            participant.id.to_string(),
            "100001@lid",
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.username.as_deref(),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-creator-country-and-username-success"
        );
        assert!(
            !(participant.is_admin),
            "disabled-creator-country-and-username-success"
        );
        assert!(
            !(participant.is_super_admin),
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::Participant,
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.join_time,
            Some(123_i64),
            "disabled-creator-country-and-username-success"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            None,
            "disabled-creator-country-and-username-success"
        );
    }
}
