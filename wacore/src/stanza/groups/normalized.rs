//! Contextual validation and source-qualified owned group projections. Existing
//! public events and the legacy raw-node adapter keep their shape.
use super::normalization_warning::{Decision, GroupWarningSink, IgnoreWarnings};
pub use super::normalized_notification::{
    GroupNotificationDiagnostic, NormalizedGroupNotification, parse_normalized_group_notification,
    parse_normalized_group_notification_with_warnings,
};
use super::{GroupNotification, GroupParticipantInfo};
use crate::WireEnum;
use wacore_binary::{Jid, NodeRef};
#[path = "unlink_generated.rs"]
mod generated;

/// Facts supplied by the caller from the same account/device snapshot.
pub struct GroupParseContext<'a> {
    pub me_device_lid: Option<&'a Jid>,
    pub me_device_pn: Option<&'a Jid>,
    pub username_displayed: bool,
    /// Retained for caller compatibility; this snapshot reads participant history unconditionally.
    pub history_after_join: bool,
    pub opus_enabled: bool,
    pub is_me_account: &'a dyn Fn(&Jid) -> bool,
    pub normalize_country_code: &'a dyn Fn(&str) -> Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GroupParseError {
    #[error("expected notification tag")]
    Tag,
    #[error("notification destination does not match this device")]
    Destination,
    #[error("missing or invalid {0}")]
    Field(&'static str),
    #[error("{prefix}{0}", prefix=super::envelope_runtime::UNKNOWN_ACTION_ERROR_PREFIX)]
    UnknownAction(String),
    #[error("integer outside the qualified API range for {0}")]
    NumericRange(&'static str),
    #[error("non-string action value for {0}")]
    UnsupportedActionType(&'static str),
    #[error("missing context fact {0}")]
    MissingContext(&'static str),
    #[error("envelope rejected: {0}")]
    Rejected(String),
    #[error("{0}")]
    Action(#[source] Box<super::actions_normalized::GroupActionParseError>),
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
    /// Unknown types warn and return null after payload validation.
    pub kind: Option<GroupUnlinkKind>,
    pub groups: Vec<UnlinkedGroup>,
}

/// Decode payload before the decision and its warnings, preserving source error order.
pub fn parse_unlink(node: &NodeRef<'_>) -> Result<NormalizedUnlink, GroupParseError> {
    parse_unlink_with_warnings(node, &mut IgnoreWarnings)
}

pub fn parse_unlink_with_warnings(
    node: &NodeRef<'_>,
    warnings: &mut dyn GroupWarningSink,
) -> Result<NormalizedUnlink, GroupParseError> {
    let kind = node
        .get_attr("unlink_type")
        .ok_or(GroupParseError::Field("unlink_type"))?;
    let reason = node.get_attr("unlink_reason");
    let mut groups = vec![];
    if node.content.is_some() && node.children().is_none() {
        return Err(GroupParseError::Field("children"));
    }
    for child in node.get_children_by_tag("group") {
        let jid = required_jid(child, "jid", JidKind::Group)?;
        let subject = child
            .get_attr("subject")
            .ok_or(GroupParseError::Field("subject"))?
            .to_string();
        let subject_time = read_integer(child, "s_t")?;
        groups.push(UnlinkedGroup {
            jid,
            subject,
            subject_time,
        });
    }
    let action =
        match generated::decide_unlink(&kind.as_str(), reason.map(|v| v.as_str()).as_deref())? {
            Decision::Action(value) => Some(value),
            Decision::Null(warning) => {
                warning.emit(&kind.as_str(), warnings)?;
                return Ok(NormalizedUnlink {
                    kind: None,
                    groups: Vec::new(),
                });
            }
        };
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
}
fn required_jid(
    node: &NodeRef<'_>,
    field: &'static str,
    kind: JidKind,
) -> Result<Jid, GroupParseError> {
    let raw = node
        .get_attr(field)
        .ok_or(GroupParseError::Field(field))?
        .as_str();
    let jid = match kind {
        JidKind::Group => super::identity::parse_group(&raw),
    };
    jid.ok_or(GroupParseError::Field(field))
}
fn read_integer(node: &NodeRef<'_>, field: &'static str) -> Result<i64, GroupParseError> {
    let raw = node
        .get_attr(field)
        .ok_or(GroupParseError::Field(field))?
        .as_str();
    super::identity::parse_js_integer(&raw).map_err(|error| match error {
        super::identity::IntegerReadError::NotInteger => GroupParseError::Field(field),
        super::identity::IntegerReadError::OutsideExactRange => {
            GroupParseError::NumericRange(field)
        }
    })
}

impl GroupNotification {
    /// Validate the envelope using actual local identities and account gates.
    /// This adapter still carries raw create/link/unlink payloads. Use
    /// `parse_unlink` for the separately qualified normalized unlink result.
    pub fn try_from_node_ref_with_context(
        node: &NodeRef<'_>,
        context: &GroupParseContext<'_>,
    ) -> Result<Self, GroupParseError> {
        let parsed = parse_normalized_group_notification(node, context)?;
        let wire_actions = node.children().into_iter().flatten().filter(|child| {
            child.tag.as_ref() != super::envelope_runtime::MISSING_IDENTIFICATION_TAG
        });
        let mut actions = vec![];
        for (action, wire) in parsed.actions.iter().zip(wire_actions) {
            actions.push(super::legacy_adapter::action_to_legacy(action, wire)?);
        }
        if actions.len() != parsed.actions.len() {
            return Err(GroupParseError::Field("actions"));
        }
        Ok(Self {
            group_jid: parsed.chat_id,
            notification_id: Some(parsed.external_id),
            notify: parsed.pushname,
            offline: parsed.offline,
            participant: parsed.author,
            participant_pn: parsed.author_phone_number,
            participant_username: parsed.author_username,
            participant_country_code: parsed.author_username_country_code,
            timestamp: u64::try_from(parsed.timestamp)
                .map_err(|_| GroupParseError::NumericRange("t"))?,
            is_lid_addressing_mode: parsed.is_lid_addressing_mode,
            has_incomplete_participant_information: parsed.has_incomplete_participant_information,
            actions,
        })
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
    use super::super::GroupNotificationAction;
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
        let pn = Jid::pn_device("200002", 2);
        let context = GroupParseContext {
            me_device_lid: Some(&lid),
            me_device_pn: Some(&pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
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
                me_device_lid: Some(&lid),
                me_device_pn: None,
                username_displayed: enabled,
                history_after_join: false,
                opus_enabled: false,
                is_me_account: &|_| false,
                normalize_country_code: &|_| None,
            };
            let node = base()
                .attr("participant_pn", "100004@lid")
                .attr("participant_username", "synthetic")
                .children([
                    NodeBuilder::new(super::super::MISSING_PARTICIPANT_IDENTIFICATION_TAG).build(),
                ])
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
            me_device_lid: Some(&lid),
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
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
    fn participant_defaults_and_history_are_independent_of_old_gate() {
        let lid = Jid::lid_device("100001", 2);
        for enabled in [false, true] {
            let context = GroupParseContext {
                me_device_lid: Some(&lid),
                me_device_pn: None,
                username_displayed: false,
                history_after_join: enabled,
                opus_enabled: false,
                is_me_account: &|_| false,
                normalize_country_code: &|_| None,
            };
            let node = base()
                .children([NodeBuilder::new("add")
                    .children([
                        NodeBuilder::new("participant")
                            .attr("jid", "200002@s.whatsapp.net")
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
            assert_eq!(participants[0].join_time, Some(123));
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
    #[test]
    fn username_gates_follow_the_specific_action() {
        let lid = Jid::lid_device("100001", 2);
        let context = GroupParseContext {
            me_device_lid: Some(&lid),
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
        };
        let node = base()
            .children([
                NodeBuilder::new("subject")
                    .attr("subject", "synthetic subject")
                    .attr("s_o_username", "owner_username")
                    .build(),
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .children([NodeBuilder::new("requested_user")
                        .attr("jid", "100002@lid")
                        .attr("username", "request_username")
                        .build()])
                    .build(),
                NodeBuilder::new("add")
                    .children([NodeBuilder::new("participant")
                        .attr("jid", "100003@lid")
                        .attr("participant_username", "unsupported_alias")
                        .build()])
                    .build(),
            ])
            .build();
        let parsed =
            GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                .unwrap();
        let GroupNotificationAction::Subject {
            subject_owner_username,
            ..
        } = &parsed.actions[0]
        else {
            panic!("subject")
        };
        assert_eq!(subject_owner_username.as_deref(), Some("owner_username"));
        let GroupNotificationAction::CreatedMembershipRequests { requests, .. } =
            &parsed.actions[1]
        else {
            panic!("requests")
        };
        assert!(requests[0].username.is_none());
        let GroupNotificationAction::Add { participants, .. } = &parsed.actions[2] else {
            panic!("add")
        };
        assert!(participants[0].username.is_none());
    }
    #[test]
    fn participant_username_is_not_gated_by_the_display_preference() {
        let lid = Jid::lid_device("100001", 2);
        let context = GroupParseContext {
            me_device_lid: Some(&lid),
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
        };
        let node = base()
            .children([NodeBuilder::new("add")
                .children([NodeBuilder::new("participant")
                    .attr("jid", "100002@lid")
                    .attr("username", "participant_username")
                    .build()])
                .build()])
            .build();
        let parsed =
            GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                .unwrap();
        let GroupNotificationAction::Add { participants, .. } = &parsed.actions[0] else {
            panic!("add")
        };
        assert_eq!(
            participants[0].username.as_deref(),
            Some("participant_username")
        );
    }
    #[test]
    fn present_unknown_role_rejects_before_missing_participant_jid() {
        let lid = Jid::lid_device("100001", 2);
        let context = GroupParseContext {
            me_device_lid: Some(&lid),
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
        };
        let node = base()
            .children([NodeBuilder::new("add")
                .children([NodeBuilder::new("participant")
                    .attr("type", "future_role")
                    .build()])
                .build()])
            .build();
        assert_eq!(
            GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                .unwrap_err(),
            GroupParseError::Field("type")
        );
    }
    #[test]
    fn official_integer_prefix_and_identity_classes_are_preserved() {
        let lid = Jid::lid_device("100001", 2);
        let context = GroupParseContext {
            me_device_lid: Some(&lid),
            me_device_pn: None,
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &|_| false,
            normalize_country_code: &|_| None,
        };
        for author in [
            "200002@bot",
            "200002@msgr",
            "1-200002@interop",
            "200002@s.whatsapp.net",
        ] {
            let node = base()
                .attr("participant", author)
                .attr("t", " +123seconds")
                .build();
            assert_eq!(
                GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                    .unwrap()
                    .timestamp,
                123
            );
        }
        for author in [
            "200002:2@s.whatsapp.net",
            "100002@s.whatsapp.net",
            "100001:2@lid",
        ] {
            let node = base().attr("participant", author).build();
            assert_eq!(
                GroupNotification::try_from_node_ref_with_context(&node.as_node_ref(), &context)
                    .unwrap_err(),
                GroupParseError::Field("participant")
            );
        }
    }
}

#[cfg(test)]
#[path = "envelope_conformance.rs"]
mod conformance;

#[cfg(test)]
#[path = "actions_conformance.rs"]
mod actions_conformance;
