//! Compatibility projection for the established event payloads. The normalized
//! union remains the complete parsed data. Raw nodes are retained only where
//! the public legacy variant explicitly requires them.
use super::actions_normalized::{NormalizedGroupAction, RequestedUser};
use super::create_normalized::CreateParticipant;
use super::normalized::GroupParseError;
use super::{GroupNotificationAction, GroupNotificationActionTag, GroupParticipantInfo};
use wacore_binary::NodeRef;

fn unsigned64(value: i64, field: &'static str) -> Result<u64, GroupParseError> {
    value
        .try_into()
        .map_err(|_| GroupParseError::NumericRange(field))
}
fn unsigned32(value: i64, field: &'static str) -> Result<u32, GroupParseError> {
    value
        .try_into()
        .map_err(|_| GroupParseError::NumericRange(field))
}
fn participants(
    values: &[CreateParticipant],
) -> Result<Vec<GroupParticipantInfo>, GroupParseError> {
    values
        .iter()
        .map(|p| {
            Ok(GroupParticipantInfo {
                jid: p.id.clone(),
                phone_number: p.phone_number.clone(),
                display_name: p.display_name.clone(),
                r#type: Some(p.role),
                lid: p.lid.clone(),
                username: p.username.clone(),
                join_time: p
                    .join_time
                    .map(|time| unsigned64(time, "join_time"))
                    .transpose()?,
                group_history_sent_state: p.group_history_sent_state,
            })
        })
        .collect()
}
fn requested_users(values: &[RequestedUser]) -> Vec<GroupParticipantInfo> {
    values
        .iter()
        .map(|p| GroupParticipantInfo {
            jid: p.wid.clone(),
            phone_number: p.phone_number.clone(),
            display_name: None,
            r#type: None,
            lid: None,
            username: p.username.clone(),
            join_time: None,
            group_history_sent_state: None,
        })
        .collect()
}
fn wire_text(wire: &NodeRef<'_>, field: &'static str) -> Result<String, GroupParseError> {
    wire.get_attr(field)
        .map(|value| value.as_str().into_owned())
        .ok_or(GroupParseError::Field(field))
}
fn require_raw_tag(
    wire: &NodeRef<'_>,
    expected: GroupNotificationActionTag,
) -> Result<(), GroupParseError> {
    if GroupNotificationActionTag::from(wire.tag.as_ref()) == expected {
        Ok(())
    } else {
        Err(GroupParseError::UnknownAction(wire.tag.to_string()))
    }
}

/// Pair with the original child node after parsing and excluding marker nodes.
/// Numeric values that cannot fit the public unsigned fields fail explicitly;
/// extra normalized metadata remains available through the normalized API.
pub(super) fn action_to_legacy(
    action: &NormalizedGroupAction,
    wire: &NodeRef<'_>,
) -> Result<GroupNotificationAction, GroupParseError> {
    use GroupNotificationAction as L;
    use NormalizedGroupAction as N;
    Ok(match action {
        N::Create { .. } => {
            require_raw_tag(wire, GroupNotificationActionTag::Create)?;
            L::Create {
                raw: wire.to_owned(),
            }
        }
        N::Add {
            participants: values,
            reason,
            ..
        } => L::Add {
            participants: participants(values)?,
            reason: reason.as_ref().map(|value| value.as_str().to_string()),
        },
        N::Delete { reason, .. } => L::Delete {
            reason: reason.as_ref().map(|value| value.as_str().to_string()),
        },
        N::Remove {
            participants: values,
            reason,
            ..
        } => L::Remove {
            participants: participants(values)?,
            reason: reason.as_ref().map(|value| value.as_str().to_string()),
        },
        N::Promote {
            participants: values,
        } => L::Promote {
            participants: participants(values)?,
        },
        N::Demote {
            participants: values,
        } => L::Demote {
            participants: participants(values)?,
        },
        N::LinkedGroupPromote {
            participants: values,
            ..
        } => L::LinkedGroupPromote {
            participants: participants(values)?,
        },
        N::LinkedGroupDemote {
            participants: values,
            ..
        } => L::LinkedGroupDemote {
            participants: participants(values)?,
        },
        N::Modify {
            participants: values,
        } => L::Modify {
            participants: participants(values)?,
        },
        N::Subject {
            subject,
            owner,
            owner_pn,
            owner_username,
            subject_time,
        } => L::Subject {
            subject: subject.clone(),
            subject_owner: owner.clone(),
            subject_owner_pn: owner_pn.clone(),
            subject_owner_username: owner_username.clone(),
            subject_time: subject_time
                .map(|time| unsigned64(time, "s_t"))
                .transpose()?,
        },
        N::DescriptionAdded {
            id, description, ..
        } => L::Description {
            id: id.clone(),
            description: description.clone(),
        },
        N::DescriptionRemoved { id } => L::Description {
            id: id.clone(),
            description: None,
        },
        N::Suspend { value: true } => L::Suspended,
        N::Suspend { value: false } => L::Unsuspended,
        N::Restrict {
            value: true,
            threshold,
        } => L::Locked {
            threshold: threshold.clone(),
        },
        N::Restrict { value: false, .. } => L::Unlocked,
        N::Announce { value: true } => L::Announce,
        N::Announce { value: false } => L::NotAnnounce,
        N::NoFrequentlyForwarded { value: true } => L::NoFrequentlyForwarded,
        N::NoFrequentlyForwarded { value: false } => L::FrequentlyForwardedOk,
        N::Invite { code } => L::Invite { code: code.clone() },
        N::Ephemeral {
            duration, trigger, ..
        } => L::Ephemeral {
            expiration: unsigned32(*duration, "expiration")?,
            trigger: trigger
                .map(|value| unsigned32(value, "trigger"))
                .transpose()?,
        },
        N::RevokeInvite { .. } => L::RevokeInvite,
        N::GrowthLocked {
            expiration,
            lock_type,
        } => L::GrowthLocked {
            expiration: unsigned32(*expiration, "expiration")?,
            lock_type: lock_type.clone(),
        },
        N::GrowthUnlocked => L::GrowthUnlocked,
        N::Link { .. } => {
            require_raw_tag(wire, GroupNotificationActionTag::Link)?;
            L::Link {
                link_type: wire_text(wire, "link_type")?,
                raw: wire.to_owned(),
            }
        }
        N::Unlink { .. } => {
            require_raw_tag(wire, GroupNotificationActionTag::Unlink)?;
            L::Unlink {
                unlink_type: wire_text(wire, "unlink_type")?,
                unlink_reason: wire
                    .get_attr("unlink_reason")
                    .map(|value| value.as_str().into_owned()),
                raw: wire.to_owned(),
            }
        }
        N::MembershipApprovalMode { value, .. } => L::MembershipApprovalMode { enabled: *value },
        N::MembershipApprovalRequest {
            request_method,
            parent_group_id,
        } => L::MembershipApprovalRequest {
            request_method: (*request_method).into(),
            parent_group_jid: parent_group_id.clone(),
        },
        N::AllowAdminReports { value: true, .. } => L::AllowAdminReports,
        N::AllowAdminReports { value: false, .. } => L::NotAllowAdminReports,
        N::AdminReportReceived { .. } => L::Reports,
        N::CreatedMembershipRequests {
            request_method,
            parent_group_id,
            requests,
            ..
        } => L::CreatedMembershipRequests {
            request_method: (*request_method).into(),
            parent_group_jid: parent_group_id.clone(),
            requests: requested_users(requests),
        },
        N::RevokedMembershipRequests { requests } => L::RevokedMembershipRequests {
            participants: requests.clone(),
        },
        N::AllowNonAdminSubgroupCreation { value: true } => L::AllowNonAdminSubGroupCreation,
        N::AllowNonAdminSubgroupCreation { value: false } => L::NotAllowNonAdminSubGroupCreation,
        N::CreatedSubgroupSuggestion { .. } => {
            require_raw_tag(wire, GroupNotificationActionTag::CreatedSubGroupSuggestion)?;
            L::CreatedSubGroupSuggestion {
                raw: wire.to_owned(),
            }
        }
        N::RevokedSubgroupSuggestions { .. } => {
            require_raw_tag(wire, GroupNotificationActionTag::RevokedSubGroupSuggestions)?;
            L::RevokedSubGroupSuggestions {
                raw: wire.to_owned(),
            }
        }
        N::SubgroupSuggestionsOwnerChanged { data } => L::ChangeNumber {
            new_owner: Some(data.new_owner.clone()),
            sub_group_suggestions: data.suggestions.clone(),
        },
        N::MemberAddMode { mode } => L::MemberAddMode {
            mode: mode
                .as_ref()
                .ok_or(GroupParseError::UnsupportedActionType("member_add_mode"))?
                .as_str()
                .to_string(),
        },
        N::GeneralChatAutoAddDisabled => L::AutoAddDisabled,
        N::GroupSafetyCheck { value: true } => L::GroupSafetyCheck,
        N::GroupSafetyCheck { value: false } => {
            return Err(GroupParseError::UnsupportedActionType(
                "group_safety_check=false",
            ));
        }
    })
}

#[cfg(test)]
mod tests {
    use super::super::actions_normalized::{
        ActionContext, NormalizedMembershipRequestMethod, parse_action_result,
    };
    use super::super::{GroupParticipantType, MembershipRequestMethod};
    use super::*;
    use wacore_binary::{Jid, builder::NodeBuilder};
    fn is_me(_: &Jid) -> bool {
        false
    }
    fn country(_: &str) -> Option<String> {
        None
    }
    fn context(group: &Jid) -> ActionContext<'_> {
        ActionContext {
            group_jid: group,
            author: None,
            timestamp: 123,
            is_lid_addressing_mode: false,
            username_displayed: false,
            history_after_join: true,
            is_me_account: &is_me,
            opus_enabled: false,
            normalize_country_code: &country,
        }
    }
    #[test]
    fn non_ephemeral_alias_never_reparses_contradictory_wire_duration() {
        let group: Jid = "123456@g.us".parse().unwrap();
        let node = NodeBuilder::new("not_ephemeral")
            .attr("expiration", "999")
            .attr("trigger", "99")
            .build();
        let normalized = parse_action_result(&node.as_node_ref(), &context(&group))
            .unwrap()
            .unwrap();
        let legacy = action_to_legacy(&normalized, &node.as_node_ref()).unwrap();
        assert!(matches!(
            legacy,
            GroupNotificationAction::Ephemeral {
                expiration: 0,
                trigger: None
            }
        ));
    }
    #[test]
    fn numeric_narrowing_never_substitutes_zero_or_wraps() {
        let wire = NodeBuilder::new("ephemeral").build();
        for duration in [-1, i64::from(u32::MAX) + 1] {
            let action = NormalizedGroupAction::Ephemeral {
                duration,
                trigger: None,
                initiated_by_me: None,
            };
            assert_eq!(
                action_to_legacy(&action, &wire.as_node_ref()).unwrap_err(),
                GroupParseError::NumericRange("expiration")
            );
        }
    }
    #[test]
    fn request_method_fallback_uses_the_established_public_enum() {
        let group: Jid = "123456@g.us".parse().unwrap();
        let node = NodeBuilder::new("membership_approval_request")
            .attr("request_method", "future_method")
            .build();
        let action = parse_action_result(&node.as_node_ref(), &context(&group))
            .unwrap()
            .unwrap();
        assert!(matches!(
            action,
            NormalizedGroupAction::MembershipApprovalRequest {
                request_method: NormalizedMembershipRequestMethod::InviteLink,
                ..
            }
        ));
        let legacy = action_to_legacy(&action, &node.as_node_ref()).unwrap();
        assert!(matches!(
            legacy,
            GroupNotificationAction::MembershipApprovalRequest {
                request_method: MembershipRequestMethod::InviteLink,
                ..
            }
        ));
    }
    #[test]
    fn participants_keep_role_history_and_username_projection() {
        let group: Jid = "123456@g.us".parse().unwrap();
        let node = NodeBuilder::new("add")
            .children([NodeBuilder::new("participant")
                .attr("jid", "200002@s.whatsapp.net")
                .attr("type", "superadmin")
                .attr("join_time", "123")
                .attr("username", "synthetic_user")
                .build()])
            .build();
        let action = parse_action_result(&node.as_node_ref(), &context(&group))
            .unwrap()
            .unwrap();
        let legacy = action_to_legacy(&action, &node.as_node_ref()).unwrap();
        let GroupNotificationAction::Add { participants, .. } = legacy else {
            panic!("add")
        };
        assert_eq!(
            participants[0].r#type,
            Some(GroupParticipantType::SuperAdmin)
        );
        assert_eq!(participants[0].join_time, Some(123));
        assert_eq!(participants[0].username.as_deref(), Some("synthetic_user"));
    }
    #[test]
    fn absent_member_add_mode_is_explicitly_unrepresentable() {
        let wire = NodeBuilder::new("member_add_mode").build();
        assert_eq!(
            action_to_legacy(
                &NormalizedGroupAction::MemberAddMode { mode: None },
                &wire.as_node_ref()
            )
            .unwrap_err(),
            GroupParseError::UnsupportedActionType("member_add_mode")
        );
    }
}
