//! Auto-generated isolated official CREATE conformance cases (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Original handler module SHA-256 a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1.
//! T/g/h/y and actual official XML/Wap/JID/time/enum dependency modules are executed by the fixture harness.
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
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.reason.as_ref().map(|v| v.normalized_value()),
            None,
            "disabled-country-and-history-gates.reason"
        );
        assert_eq!(
            parsed
                .context_group_id
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.contextGroupId"
        );
        let group = &parsed.group_info;
        assert_eq!(
            group.subject.as_str(),
            "synthetic group",
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.creation, 123_i64,
            "disabled-country-and-history-gates"
        );
        assert!(
            !(group.restrict),
            "disabled-country-and-history-gates.restrict"
        );
        assert!(
            !(group.announce),
            "disabled-country-and-history-gates.announce"
        );
        assert!(
            !(group.no_frequently_forwarded),
            "disabled-country-and-history-gates.noFrequentlyForwarded"
        );
        assert!(
            !(group.support),
            "disabled-country-and-history-gates.support"
        );
        assert!(
            !(group.is_parent_group),
            "disabled-country-and-history-gates.isParentGroup"
        );
        assert!(
            !(group.is_parent_group_closed),
            "disabled-country-and-history-gates.isParentGroupClosed"
        );
        assert!(
            !(group.default_subgroup),
            "disabled-country-and-history-gates.defaultSubgroup"
        );
        assert!(
            !(group.general_subgroup),
            "disabled-country-and-history-gates.generalSubgroup"
        );
        assert!(
            !(group.membership_approval_mode),
            "disabled-country-and-history-gates.membershipApprovalMode"
        );
        assert!(
            !(group.allow_non_admin_subgroup_creation),
            "disabled-country-and-history-gates.allowNonAdminSubGroupCreation"
        );
        assert!(
            !(group.general_chat_auto_add_disabled),
            "disabled-country-and-history-gates.generalChatAutoAddDisabled"
        );
        assert!(
            !(group.hidden_subgroup),
            "disabled-country-and-history-gates.hiddenSubgroup"
        );
        assert!(
            !(group.group_safety_check),
            "disabled-country-and-history-gates.groupSafetyCheck"
        );
        assert!(
            !(group.has_capi),
            "disabled-country-and-history-gates.hasCapi"
        );
        assert!(
            !(group.limit_sharing_enabled),
            "disabled-country-and-history-gates.limitSharingEnabled"
        );
        assert_eq!(
            group.owner.as_ref().map(ToString::to_string).as_deref(),
            None,
            "disabled-country-and-history-gates.owner"
        );
        assert_eq!(
            group
                .creator_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.creatorPn"
        );
        assert_eq!(
            group
                .subject_owner
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.subjectOwner"
        );
        assert_eq!(
            group
                .subject_owner_pn
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.subjectOwnerPn"
        );
        assert_eq!(
            group
                .parent_group
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.parentGroup"
        );
        assert_eq!(
            group
                .group_adder
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates.groupAdder"
        );
        assert_eq!(
            group.creator_country_code.as_deref(),
            None,
            "disabled-country-and-history-gates.creatorCountryCode"
        );
        assert_eq!(
            group.creator_username.as_deref(),
            None,
            "disabled-country-and-history-gates.creatorUsername"
        );
        assert_eq!(
            group.subject_owner_username.as_deref(),
            None,
            "disabled-country-and-history-gates.subjectOwnerUsername"
        );
        assert_eq!(
            group.subject_time, None,
            "disabled-country-and-history-gates.subjectTime"
        );
        assert_eq!(group.size, None, "disabled-country-and-history-gates.size");
        assert_eq!(
            group.member_link_mode.as_ref().map(|v| v.as_str()),
            None,
            "disabled-country-and-history-gates.memberLinkMode"
        );
        assert_eq!(
            group.member_add_mode.as_ref().map(|v| v.as_str()),
            None,
            "disabled-country-and-history-gates.memberAddMode"
        );
        assert!(
            !(group.ephemerality.is_some()),
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.duration),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group
                .ephemerality
                .as_ref()
                .and_then(|v| v.trigger)
                .as_ref()
                .map(|v| v.as_str()),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.ephemerality.as_ref().map(|v| v.initiated_by_me),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.content.as_str()),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.description.as_ref().map(|v| v.id.as_str()),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            group.participants.len(),
            1,
            "disabled-country-and-history-gates"
        );
        let participant = &group.participants[0];
        assert_eq!(
            participant.id.to_string(),
            "100001@lid",
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.display_name.as_deref(),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.username.as_deref(),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.lid.as_ref().map(ToString::to_string).as_deref(),
            None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant
                .phone_number
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            None,
            "disabled-country-and-history-gates"
        );
        assert!(
            !(participant.is_admin),
            "disabled-country-and-history-gates"
        );
        assert!(
            !(participant.is_super_admin),
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.role,
            GroupParticipantType::Participant,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.join_time, None,
            "disabled-country-and-history-gates"
        );
        assert_eq!(
            participant.group_history_sent_state.map(|v| match v {
                GroupHistorySentState::HistoryNotSent => 1_i64,
                GroupHistorySentState::HistorySent => 2_i64,
                GroupHistorySentState::NoticeSent => 3_i64,
            }),
            None,
            "disabled-country-and-history-gates"
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
}
