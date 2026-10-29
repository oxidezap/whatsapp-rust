//! Contextual validation and a qualified unlink projection. This is deliberately
//! opt-in until create/link and effective account gates are qualified. Existing
//! public events and the legacy raw-node adapter keep their shape.
use super::{GroupNotification, GroupNotificationAction, GroupParticipantInfo};
use crate::WireEnum;
use wacore_binary::{Jid, NodeRef};
#[path = "unlink_generated.rs"]
mod generated;

/// Facts supplied by the caller from the same account/device snapshot.
pub struct GroupParseContext<'a> {
    pub me_device_lid: &'a Jid,
    pub me_device_pn: Option<&'a Jid>,
    pub username_displayed: bool,
    pub history_after_join: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroupParseError {
    #[error("expected notification tag")]
    Tag,
    #[error("notification destination does not match this device")]
    Destination,
    #[error("missing or invalid {0}")]
    Field(&'static str),
    #[error("unrecognized group action {0}")]
    UnknownAction(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupUnlinkKind {
    #[wire = "sub_group_unlink"]
    SubGroup,
    #[wire = "parent_group_unlink"]
    ParentGroup,
    #[wire = "sibling_group_unlink"]
    SiblingGroup,
    #[wire = "delete_parent_group_unlink"]
    DeleteParentGroup,
    #[wire = "integrity_parent_group_unlink"]
    IntegrityParentGroup,
    #[wire = "integrity_sub_group_unlink"]
    IntegritySubGroup,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlinkedGroup {
    pub jid: Jid,
    pub subject: String,
    pub subject_time: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUnlink {
    /// Upstream returns an undefined action for an unknown unlink type.
    pub kind: Option<GroupUnlinkKind>,
    pub groups: Vec<UnlinkedGroup>,
}

/// Decode data first, then apply the generated decision table, matching I().
pub fn parse_unlink(node: &NodeRef<'_>) -> Result<NormalizedUnlink, GroupParseError> {
    let kind = node
        .get_attr("unlink_type")
        .ok_or(GroupParseError::Field("unlink_type"))?;
    let reason = node.get_attr("unlink_reason");
    let mut groups = vec![];
    for child in node.get_children_by_tag("group") {
        let jid = required_jid(child, "jid", JidKind::Group)?;
        let subject = child
            .get_attr("subject")
            .ok_or(GroupParseError::Field("subject"))?
            .to_string();
        let subject_time = child
            .get_attr("s_t")
            .and_then(|v| v.as_str().parse::<i64>().ok())
            .ok_or(GroupParseError::Field("s_t"))?;
        groups.push(UnlinkedGroup {
            jid,
            subject,
            subject_time,
        });
    }
    let action = generated::normalize_unlink(&kind.as_str(), reason.map(|v| v.as_str()).as_deref());
    let kind = action
        .map(|a| {
            GroupUnlinkKind::try_from(a)
                .map_err(|_| GroupParseError::Field("generated unlink action"))
        })
        .transpose()?;
    Ok(NormalizedUnlink { kind, groups })
}

enum JidKind {
    Group,
    User,
    Phone,
    Lid,
}
fn required_jid(
    node: &NodeRef<'_>,
    field: &'static str,
    kind: JidKind,
) -> Result<Jid, GroupParseError> {
    let jid = node
        .attrs()
        .optional_jid(field)
        .ok_or(GroupParseError::Field(field))?;
    let valid = !jid.user.is_empty()
        && match kind {
            JidKind::Group => jid.server == wacore_binary::jid::Server::Group,
            JidKind::User => jid.is_pn() || jid.is_lid(),
            JidKind::Phone => jid.is_pn(),
            JidKind::Lid => jid.is_lid(),
        };
    valid.then_some(jid).ok_or(GroupParseError::Field(field))
}
fn optional_jid(
    node: &NodeRef<'_>,
    field: &'static str,
    kind: JidKind,
) -> Result<(), GroupParseError> {
    if node.get_attr(field).is_some() {
        required_jid(node, field, kind)?;
    }
    Ok(())
}

impl GroupNotification {
    /// Validate the envelope using actual local identities and account gates.
    /// This adapter still carries raw create/link/unlink payloads. Use
    /// `parse_unlink` for the separately qualified normalized unlink result.
    pub fn try_from_node_ref_with_context(
        node: &NodeRef<'_>,
        context: &GroupParseContext<'_>,
    ) -> Result<Self, GroupParseError> {
        if node.tag.as_ref() != "notification" {
            return Err(GroupParseError::Tag);
        }
        if let Some(to) = node.get_attr("to") {
            let lid = context.me_device_lid.to_string();
            let pn = context.me_device_pn.map(ToString::to_string);
            if to.as_str() != lid && pn.as_deref() != Some(to.as_str().as_ref()) {
                return Err(GroupParseError::Destination);
            }
        }
        required_jid(node, "from", JidKind::Group)?;
        optional_jid(node, "participant", JidKind::User)?;
        // The official parser catches just this PN accessor's failure.
        let participant_pn = required_jid(node, "participant_pn", JidKind::Phone).ok();
        if node
            .get_attr("t")
            .and_then(|v| v.as_str().parse::<u64>().ok())
            .is_none()
        {
            return Err(GroupParseError::Field("t"));
        }
        // Action and participant validation precedes the final id accessor.
        if let Some(children) = node.children() {
            for child in children {
                if child.tag.as_ref() == "missing_participant_identification" {
                    continue;
                }
                if matches!(
                    super::parse_action(child),
                    Some(GroupNotificationAction::Unknown { .. })
                ) {
                    return Err(GroupParseError::UnknownAction(child.tag.to_string()));
                }
                if matches!(
                    child.tag.as_ref(),
                    "add"
                        | "remove"
                        | "promote"
                        | "demote"
                        | "modify"
                        | "linked_group_promote"
                        | "linked_group_demote"
                ) {
                    for p in child.get_children_by_tag("participant") {
                        if let Some(role) = p.attrs().optional_string("type")
                            && super::GroupParticipantType::try_from(role.as_ref()).is_err()
                        {
                            return Err(GroupParseError::Field("type"));
                        }
                        optional_jid(p, "lid", JidKind::Lid)?;
                        optional_jid(p, "phone_number", JidKind::Phone)?;
                        required_jid(p, "jid", JidKind::User)?;
                    }
                }
            }
        }
        if node.get_attr("id").is_none() {
            return Err(GroupParseError::Field("id"));
        }
        let mut parsed = Self::try_from_node_ref(node).ok_or(GroupParseError::Field("from"))?;
        parsed.participant_pn = participant_pn;
        parsed.has_incomplete_participant_information &= context.username_displayed;
        if !context.username_displayed {
            parsed.participant_username = None;
            parsed.participant_country_code = None;
        }
        for action in &mut parsed.actions {
            let participants = match action {
                GroupNotificationAction::Add { participants, .. }
                | GroupNotificationAction::Remove { participants, .. }
                | GroupNotificationAction::Promote { participants }
                | GroupNotificationAction::Demote { participants }
                | GroupNotificationAction::Modify { participants }
                | GroupNotificationAction::LinkedGroupPromote { participants }
                | GroupNotificationAction::LinkedGroupDemote { participants } => Some(participants),
                GroupNotificationAction::Subject {
                    subject_owner_username,
                    ..
                } => {
                    if !context.username_displayed {
                        *subject_owner_username = None;
                    }
                    None
                }
                _ => None,
            };
            if let Some(participants) = participants
                && !context.history_after_join
            {
                for p in participants {
                    p.join_time = None;
                    p.group_history_sent_state = None;
                }
            }
        }
        Ok(parsed)
    }
}
impl GroupParticipantInfo {
    pub fn is_super_admin(&self) -> bool {
        self.r#type == Some(super::GroupParticipantType::SuperAdmin)
    }
    pub fn is_admin(&self) -> bool {
        self.is_super_admin() || self.r#type == Some(super::GroupParticipantType::Admin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    fn base() -> NodeBuilder {
        NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
    }
    #[test]
    fn identities_and_first_failure_use_real_context() {
        let lid = Jid::lid_device("100001", 2);
        let pn = Jid::pn_device("100002", 2);
        let context = GroupParseContext {
            me_device_lid: &lid,
            me_device_pn: Some(&pn),
            username_displayed: false,
            history_after_join: false,
        };
        for to in [lid.to_string(), pn.to_string()] {
            let node = base().attr("to", to).build();
            assert!(
                GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                    .is_ok()
            );
        }
        let wrong = NodeBuilder::new("notification")
            .attr("to", "100003:2@lid")
            .build();
        assert_eq!(
            GroupNotification::try_from_node_ref_with_context(&wrong.as_node_ref(), &context)
                .unwrap_err(),
            GroupParseError::Destination
        );
        let account = base().attr("to", "100001@lid").build();
        assert_eq!(
            GroupNotification::try_from_node_ref_with_context(&account.as_node_ref(), &context)
                .unwrap_err(),
            GroupParseError::Destination
        );
    }
    #[test]
    fn pn_failure_is_local_and_markers_are_gated_without_actions() {
        let lid = Jid::lid_device("100001", 2);
        for enabled in [false, true] {
            let context = GroupParseContext {
                me_device_lid: &lid,
                me_device_pn: None,
                username_displayed: enabled,
                history_after_join: false,
            };
            let node = base()
                .attr("participant_pn", "100004@lid")
                .attr("participant_username", "synthetic")
                .children([NodeBuilder::new("missing_participant_identification").build()])
                .build();
            let parsed =
                GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                    .unwrap();
            assert!(parsed.participant_pn.is_none());
            assert_eq!(parsed.has_incomplete_participant_information, enabled);
            assert_eq!(parsed.participant_username.is_some(), enabled);
            assert!(parsed.actions.is_empty());
        }
    }
    #[test]
    fn unknown_action_precedes_missing_final_id() {
        let lid = Jid::lid_device("100001", 2);
        let context = GroupParseContext {
            me_device_lid: &lid,
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("t", "123")
            .children([NodeBuilder::new("future_action").build()])
            .build();
        assert_eq!(
            GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                .unwrap_err(),
            GroupParseError::UnknownAction("future_action".into())
        );
    }
    #[test]
    fn participant_defaults_and_history_gate() {
        let lid = Jid::lid_device("100001", 2);
        for enabled in [false, true] {
            let context = GroupParseContext {
                me_device_lid: &lid,
                me_device_pn: None,
                username_displayed: false,
                history_after_join: enabled,
            };
            let node = base()
                .children([NodeBuilder::new("add")
                    .children([
                        NodeBuilder::new("participant")
                            .attr("jid", "100002@s.whatsapp.net")
                            .attr("join_time", "123")
                            .build(),
                        NodeBuilder::new("participant")
                            .attr("jid", "100003@lid")
                            .attr("type", "superadmin")
                            .build(),
                    ])
                    .build()])
                .build();
            let parsed =
                GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                    .unwrap();
            let GroupNotificationAction::Add { participants, .. } = &parsed.actions[0] else {
                panic!("expected add")
            };
            assert_eq!(
                participants[0].r#type,
                Some(super::super::GroupParticipantType::Participant)
            );
            assert_eq!(participants[0].join_time.is_some(), enabled);
            assert!(participants[1].is_admin());
            assert!(participants[1].is_super_admin());
        }
    }
    #[test]
    fn unlink_preserves_integrity_distinctions_and_group_data_errors() {
        for (kind, reason, expected) in [
            (
                "parent_group",
                "integrity_delete_parent",
                GroupUnlinkKind::IntegrityParentGroup,
            ),
            (
                "sub_group",
                "integrity_delete_parent",
                GroupUnlinkKind::IntegritySubGroup,
            ),
            (
                "sub_group",
                "delete_parent",
                GroupUnlinkKind::DeleteParentGroup,
            ),
            (
                "sibling_group",
                "delete_parent",
                GroupUnlinkKind::SiblingGroup,
            ),
        ] {
            let node = NodeBuilder::new("unlink")
                .attr("unlink_type", kind)
                .attr("unlink_reason", reason)
                .build();
            assert_eq!(
                parse_unlink(&node.as_node_ref()).unwrap().kind,
                Some(expected)
            );
        }
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .children([NodeBuilder::new("group").attr("jid", "100001@lid").build()])
            .build();
        assert_eq!(
            parse_unlink(&node.as_node_ref()).unwrap_err(),
            GroupParseError::Field("jid")
        );
    }
}
