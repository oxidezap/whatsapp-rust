//! Typed CREATE projection of WAWebHandleGroupNotification.T(), helpers y/g/h,
//! and WAWebGroupsQueryApi.extractLinkedParent. Runtime telemetry, storage,
//! cache mutation and ACK handling remain outside this parser.
//!
//! Qualified snapshot 2.3000.1047483476, bundle set
//! 05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.
//! Handler bundle 09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8,
//! module bytes 1693402..1716842, module SHA-256
//! a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1.
//! Linked-parent helper module bytes 524523..532908, SHA-256
//! 0db1b65d79a6a1ad9648528c31359696af86feb8ea070edf476f1a9c4a02cf39.
//! Trigger helper bundle 430cb8979511aada95178605faa35a8483446f677ce6d1fe0fd66333c7879eee,
//! module bytes 792136..794820, SHA-256
//! 3f6f783ec1f2044d44cb52693893c84bbc14b5708ea63eb64458aeec3f025986.
use super::identity::{self, IntegerReadError};
use super::{GroupHistorySentState, GroupNotificationActionTag, GroupParticipantType};
use crate::WireEnum;
use crate::iq::groups::MemberLinkMode;
use crate::types::wire_enums::MemberAddMode;
use wacore_binary::{Jid, NodeContentRef, NodeRef};

/// Effective account facts and pure lookups supplied by the caller. The parser
/// never guesses feature defaults or reads mutable account/runtime state.
pub struct GroupCreateContext<'a> {
    pub username_displayed: bool,
    pub history_after_join: bool,
    pub opus_enabled: bool,
    pub envelope_author: Option<&'a Jid>,
    pub is_me_account: &'a dyn Fn(&Jid) -> bool,
    /// WAWebAsISOCountryCode's registry lookup. Unknown values normalize to None.
    pub normalize_country_code: &'a dyn Fn(&str) -> Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum CreateDisappearingTrigger {
    #[wire = "chat_settings"]
    ChatSettings,
    #[wire = "account_settings"]
    AccountSettings,
    #[wire = "bulk_change"]
    BulkChange,
    #[wire = "unknown_groups"]
    UnknownGroups,
}

/// The CREATE reason vocabulary differs from ADD: `accept` is not in its table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupCreateReason {
    #[wire = "invite"]
    Invite,
    #[wire = "linked_group_join"]
    LinkedGroupJoin,
    #[wire = "auto_add"]
    AutoAdd,
    #[wire = "default_subgroup_admin_add"]
    DefaultSubgroupAdminAdd,
    #[wire = "default_sub_group_promote"]
    DefaultSubgroupPromote,
    #[wire = "invite_auto_add"]
    InviteAutoAdd,
    #[wire = "general_chat_auto_add"]
    GeneralChatAutoAdd,
}
impl GroupCreateReason {
    pub fn normalized_value(&self) -> &str {
        self.as_str()
    }
}
fn parse_create_reason(value: &str) -> Option<GroupCreateReason> {
    // The wire table key includes an extra underscore while ADD_REASON's
    // normalized output does not. The normalized spelling is not a wire alias.
    if value == "default_sub_group_admin_add" {
        return Some(GroupCreateReason::DefaultSubgroupAdminAdd);
    }
    GroupCreateReason::try_from(value)
        .ok()
        .filter(|value| *value != GroupCreateReason::DefaultSubgroupAdminAdd)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateParticipant {
    pub id: Jid,
    pub display_name: Option<String>,
    pub role: GroupParticipantType,
    pub is_super_admin: bool,
    pub is_admin: bool,
    pub lid: Option<Jid>,
    pub phone_number: Option<Jid>,
    /// y() reads this unconditionally; only the missing-mapping diagnostic is gated.
    pub username: Option<String>,
    pub join_time: Option<i64>,
    pub group_history_sent_state: Option<GroupHistorySentState>,
}
impl CreateParticipant {
    /// The official post-join parser returns the internal numeric enum code,
    /// while the participant wire attribute carries the string enum spelling.
    pub fn history_sent_state_code(&self) -> Option<u8> {
        self.group_history_sent_state.map(|state| match state {
            GroupHistorySentState::HistoryNotSent => 1,
            GroupHistorySentState::HistorySent => 2,
            GroupHistorySentState::NoticeSent => 3,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateDescription {
    pub content: String,
    pub id: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEphemerality {
    pub duration: i64,
    pub trigger: Option<CreateDisappearingTrigger>,
    pub initiated_by_me: bool,
}

/// Owned normalized values. No raw nodes are retained as a substitute for parsed data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedGroupInfo {
    pub subject: String,
    pub restrict: bool,
    pub announce: bool,
    pub no_frequently_forwarded: bool,
    pub ephemerality: Option<CreateEphemerality>,
    pub owner: Option<Jid>,
    /// Officially read with attrUserJid, despite the `_pn` wire suffix.
    pub creator_pn: Option<Jid>,
    pub creator_country_code: Option<String>,
    pub creator_username: Option<String>,
    pub creation: i64,
    pub participants: Vec<CreateParticipant>,
    pub description: Option<CreateDescription>,
    pub subject_owner: Option<Jid>,
    /// Officially read with attrUserJid, not attrPhoneUserJid.
    pub subject_owner_pn: Option<Jid>,
    pub subject_owner_username: Option<String>,
    pub subject_time: Option<i64>,
    pub support: bool,
    pub is_parent_group: bool,
    pub is_parent_group_closed: bool,
    pub parent_group: Option<Jid>,
    pub default_subgroup: bool,
    pub general_subgroup: bool,
    pub size: Option<i64>,
    pub membership_approval_mode: bool,
    pub allow_non_admin_subgroup_creation: bool,
    /// This child belongs to the CREATE action, not its nested group node.
    pub general_chat_auto_add_disabled: bool,
    pub hidden_subgroup: bool,
    pub group_safety_check: bool,
    pub group_adder: Option<Jid>,
    pub has_capi: bool,
    pub limit_sharing_enabled: bool,
    pub member_link_mode: Option<MemberLinkMode>,
    pub member_add_mode: Option<MemberAddMode>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedGroupCreate {
    pub reason: Option<GroupCreateReason>,
    pub context_group_id: Option<Jid>,
    pub group_info: CreatedGroupInfo,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CreateParseError {
    #[error("missing or invalid {0}")]
    Field(&'static str),
    #[error("unsupported non-exact JavaScript integer in {0}")]
    NumericRange(&'static str),
}

type ParseResult<T> = Result<T, CreateParseError>;
fn text(node: &NodeRef<'_>, key: &'static str) -> ParseResult<String> {
    node.get_attr(key)
        .map(|v| v.as_str().into_owned())
        .ok_or(CreateParseError::Field(key))
}
fn maybe_text(node: &NodeRef<'_>, key: &str) -> Option<String> {
    node.get_attr(key).map(|v| v.as_str().into_owned())
}
fn integer(node: &NodeRef<'_>, key: &'static str) -> ParseResult<i64> {
    identity::parse_js_integer(&text(node, key)?).map_err(|error| match error {
        IntegerReadError::NotInteger => CreateParseError::Field(key),
        IntegerReadError::OutsideExactRange => CreateParseError::NumericRange(key),
    })
}
fn time(node: &NodeRef<'_>, key: &'static str) -> ParseResult<i64> {
    identity::parse_js_time(&text(node, key)?).map_err(|error| match error {
        IntegerReadError::NotInteger => CreateParseError::Field(key),
        IntegerReadError::OutsideExactRange => CreateParseError::NumericRange(key),
    })
}
fn maybe_time(node: &NodeRef<'_>, key: &'static str) -> ParseResult<Option<i64>> {
    node.get_attr(key).map(|_| time(node, key)).transpose()
}
fn maybe_integer(node: &NodeRef<'_>, key: &'static str) -> ParseResult<Option<i64>> {
    node.get_attr(key).map(|_| integer(node, key)).transpose()
}
fn maybe_jid(
    node: &NodeRef<'_>,
    key: &'static str,
    parse: fn(&str) -> Option<Jid>,
) -> ParseResult<Option<Jid>> {
    node.get_attr(key)
        .map(|v| parse(&v.as_str()).ok_or(CreateParseError::Field(key)))
        .transpose()
}
fn user(node: &NodeRef<'_>, key: &'static str) -> ParseResult<Jid> {
    identity::parse_user(&text(node, key)?).ok_or(CreateParseError::Field(key))
}
fn child<'node, 'source>(
    node: &'node NodeRef<'source>,
    tag: &'static str,
) -> ParseResult<&'node NodeRef<'source>> {
    node.get_optional_child(tag)
        .ok_or(CreateParseError::Field(tag))
}
fn has(node: &NodeRef<'_>, tag: &str) -> bool {
    node.get_optional_child(tag).is_some()
}
fn has_content(node: &NodeRef<'_>) -> bool {
    match node.content.as_ref() {
        Some(NodeContentRef::Bytes(_)) => true,
        Some(NodeContentRef::String(s)) => !s.is_empty(),
        Some(NodeContentRef::Nodes(_)) | None => false,
    }
}
fn content(node: &NodeRef<'_>, tag: &'static str) -> ParseResult<String> {
    if !has_content(node) {
        return Err(CreateParseError::Field(tag));
    }
    node.content_as_string()
        .map(|s| s.to_string())
        .ok_or(CreateParseError::Field(tag))
}
pub(super) fn parse_participants(
    group: &NodeRef<'_>,
    context: &GroupCreateContext<'_>,
) -> ParseResult<Vec<CreateParticipant>> {
    if matches!(
        group.content.as_ref(),
        Some(NodeContentRef::Bytes(_) | NodeContentRef::String(_))
    ) {
        return Err(CreateParseError::Field("children"));
    }
    group
        .get_children_by_tag("participant")
        .map(|p| {
            let role = maybe_text(p, "type")
                .map(|v| {
                    GroupParticipantType::try_from(v.as_str())
                        .map_err(|_| CreateParseError::Field("type"))
                })
                .transpose()?
                .unwrap_or(GroupParticipantType::Participant);
            let lid = maybe_jid(p, "lid", identity::parse_lid)?;
            let phone_number = maybe_jid(p, "phone_number", identity::parse_phone)?;
            let display_name = maybe_text(p, "display_name");
            let id = user(p, "jid")?;
            let username = maybe_text(p, "username");
            let join_time = if context.history_after_join {
                maybe_time(p, "join_time")?
            } else {
                None
            };
            let group_history_sent_state = if context.history_after_join {
                maybe_text(p, "group_history_sent_state")
                    .and_then(|v| GroupHistorySentState::try_from(v.as_str()).ok())
            } else {
                None
            };
            Ok(CreateParticipant {
                id,
                display_name,
                is_super_admin: role == GroupParticipantType::SuperAdmin,
                is_admin: matches!(
                    role,
                    GroupParticipantType::Admin | GroupParticipantType::SuperAdmin
                ),
                role,
                lid,
                phone_number,
                username,
                join_time,
                group_history_sent_state,
            })
        })
        .collect()
}
fn description(group: &NodeRef<'_>) -> ParseResult<Option<CreateDescription>> {
    if let Some(desc) = group.get_optional_child("description")
        && let Some(body) = desc.get_optional_child("body")
        && has_content(body)
    {
        return Ok(Some(CreateDescription {
            content: content(body, "body")?,
            id: text(desc, "id")?,
        }));
    }
    Ok(None)
}
fn ephemerality(
    group: &NodeRef<'_>,
    is_new: bool,
    context: &GroupCreateContext<'_>,
) -> ParseResult<Option<CreateEphemerality>> {
    // h() validates creator even when there is no ephemeral child.
    let creator = maybe_jid(group, "creator", identity::parse_user)?;
    let Some(ephemeral) = group.get_optional_child("ephemeral") else {
        return Ok(None);
    };
    let duration = integer(ephemeral, "expiration")?;
    let trigger = match maybe_text(ephemeral, "trigger").as_deref() {
        Some("1") => Some(CreateDisappearingTrigger::ChatSettings),
        Some("2") => Some(CreateDisappearingTrigger::AccountSettings),
        Some("3") => Some(CreateDisappearingTrigger::BulkChange),
        _ if !is_new => Some(CreateDisappearingTrigger::UnknownGroups),
        _ => None,
    };
    let initiated_by_me = creator.as_ref().is_some_and(context.is_me_account);
    Ok(Some(CreateEphemerality {
        duration,
        trigger,
        initiated_by_me,
    }))
}
fn linked_parent(group: &NodeRef<'_>) -> ParseResult<Option<Jid>> {
    group
        .get_optional_child("linked_parent")
        .map(|p| maybe_jid(p, "jid", identity::parse_group))
        .transpose()
        .map(Option::flatten)
}

/// Parse one CREATE child after envelope identity validation. Error order follows
/// T(): participant helper, description, ephemerality/creator, linked parent,
/// group fields, then the action's reason and context-group identifier.
pub fn parse_group_create(
    node: &NodeRef<'_>,
    context: &GroupCreateContext<'_>,
) -> ParseResult<NormalizedGroupCreate> {
    if !matches!(
        GroupNotificationActionTag::from(node.tag.as_ref()),
        GroupNotificationActionTag::Create
    ) {
        return Err(CreateParseError::Field("create"));
    }
    let group = child(node, "group")?;
    let is_new = maybe_text(node, "type").as_deref() == Some("new");
    let participants = parse_participants(group, context)?;
    let description = description(group)?;
    let ephemerality = ephemerality(group, is_new, context)?;
    let parent_group = linked_parent(group)?;
    let subject = text(group, "subject")?;
    let restrict = has(group, "locked");
    let announce = has(group, "announcement");
    let no_frequently_forwarded = has(group, "no_frequently_forwarded");
    let owner = maybe_jid(group, "creator", identity::parse_user)?;
    let creator_pn = maybe_jid(group, "creator_pn", identity::parse_user)?;
    let creator_country_code = if context.username_displayed {
        maybe_text(group, "creator_country_code").and_then(|v| (context.normalize_country_code)(&v))
    } else {
        None
    };
    let creator_username = if context.username_displayed {
        maybe_text(group, "creator_username")
    } else {
        None
    };
    let creation = time(group, "creation")?;
    let subject_owner = maybe_jid(group, "s_o", identity::parse_user)?;
    let subject_owner_pn = maybe_jid(group, "s_o_pn", identity::parse_user)?;
    let subject_owner_username = if context.username_displayed {
        maybe_text(group, "s_o_username")
    } else {
        None
    };
    let subject_time = maybe_time(group, "s_t")?;
    let support = has(group, "support");
    let is_parent_group = has(group, "parent");
    let is_parent_group_closed = group
        .get_optional_child("parent")
        .and_then(|p| maybe_text(p, "default_membership_approval_mode"))
        .as_deref()
        == Some("request_required");
    let default_subgroup = has(group, "default_sub_group");
    let general_subgroup = has(group, "general_chat");
    let size = maybe_integer(group, "size")?;
    let membership_approval_mode = group
        .get_optional_child("membership_approval_mode")
        .and_then(|p| p.get_optional_child("group_join"))
        .and_then(|p| maybe_text(p, "state"))
        .as_deref()
        == Some("on");
    let allow_non_admin_subgroup_creation = has(group, "allow_non_admin_sub_group_creation");
    let general_chat_auto_add_disabled = has(node, "auto_add_disabled");
    let hidden_subgroup = has(group, "hidden_group");
    let group_safety_check = has(group, "group_safety_check");
    let group_adder = context.envelope_author.cloned();
    let has_capi = has(group, "capi");
    let limit_sharing_enabled = !context.opus_enabled && has(group, "limit_sharing_enabled");
    let member_link_mode = group
        .get_optional_child("member_link_mode")
        .map(|c| content(c, "member_link_mode"))
        .transpose()?
        .and_then(|v| MemberLinkMode::try_from(v.as_str()).ok());
    let member_add_mode = group
        .get_optional_child("member_add_mode")
        .map(|c| content(c, "member_add_mode"))
        .transpose()?
        .and_then(|v| MemberAddMode::try_from(v.as_str()).ok());
    let reason = maybe_text(node, "reason").and_then(|v| parse_create_reason(v.as_str()));
    let context_group_id = maybe_jid(node, "context_group_jid", identity::parse_group)?;
    Ok(NormalizedGroupCreate {
        reason,
        context_group_id,
        group_info: CreatedGroupInfo {
            subject,
            restrict,
            announce,
            no_frequently_forwarded,
            ephemerality,
            owner,
            creator_pn,
            creator_country_code,
            creator_username,
            creation,
            participants,
            description,
            subject_owner,
            subject_owner_pn,
            subject_owner_username,
            subject_time,
            support,
            is_parent_group,
            is_parent_group_closed,
            parent_group,
            default_subgroup,
            general_subgroup,
            size,
            membership_approval_mode,
            allow_non_admin_subgroup_creation,
            general_chat_auto_add_disabled,
            hidden_subgroup,
            group_safety_check,
            group_adder,
            has_capi,
            limit_sharing_enabled,
            member_link_mode,
            member_add_mode,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    fn context() -> GroupCreateContext<'static> {
        GroupCreateContext {
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            envelope_author: None,
            is_me_account: &|_| false,
            normalize_country_code: &|_| panic!("disabled lookup"),
        }
    }
    fn group() -> NodeBuilder {
        NodeBuilder::new("group")
            .attr("subject", "synthetic group")
            .attr("creation", "123")
    }
    #[test]
    fn create_has_owned_normalized_data_and_correct_defaults() {
        let node = NodeBuilder::new("create")
            .attr("reason", "accept")
            .children([group()
                .children([
                    NodeBuilder::new("participant")
                        .attr("jid", "200002@s.whatsapp.net")
                        .attr("username", "ungated")
                        .build(),
                    NodeBuilder::new("participant")
                        .attr("jid", "100001@lid")
                        .attr("type", "superadmin")
                        .build(),
                ])
                .build()])
            .build();
        let parsed = parse_group_create(&node.as_node_ref(), &context()).unwrap();
        assert!(parsed.reason.is_none());
        assert_eq!(parsed.group_info.subject, "synthetic group");
        assert_eq!(
            parsed.group_info.participants[0].role,
            GroupParticipantType::Participant
        );
        assert_eq!(
            parsed.group_info.participants[0].username.as_deref(),
            Some("ungated")
        );
        assert!(parsed.group_info.participants[1].is_admin);
        assert!(parsed.group_info.participants[1].is_super_admin);
        assert!(!parsed.group_info.membership_approval_mode);
    }
    #[test]
    fn participant_type_failure_precedes_creator_subject_and_timestamp() {
        let node = NodeBuilder::new("create")
            .children([NodeBuilder::new("group")
                .attr("creator", "bad")
                .children([NodeBuilder::new("participant")
                    .attr("type", "future_role")
                    .build()])
                .build()])
            .build();
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &context()).unwrap_err(),
            CreateParseError::Field("type")
        );
    }
    #[test]
    fn disabled_flags_skip_country_registry_and_history_accessors() {
        let node = NodeBuilder::new("create")
            .children([group()
                .attr("creator_country_code", "invalid")
                .attr("creator_username", "gated")
                .attr("s_o_username", "gated")
                .children([NodeBuilder::new("participant")
                    .attr("jid", "100001@lid")
                    .attr("join_time", "invalid")
                    .build()])
                .build()])
            .build();
        let parsed = parse_group_create(&node.as_node_ref(), &context()).unwrap();
        assert!(parsed.group_info.creator_country_code.is_none());
        assert!(parsed.group_info.creator_username.is_none());
        assert!(parsed.group_info.subject_owner_username.is_none());
        let mut enabled = context();
        enabled.history_after_join = true;
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &enabled).unwrap_err(),
            CreateParseError::Field("join_time")
        );
    }
    #[test]
    fn ephemerality_uses_action_type_and_trigger_conversion() {
        for (kind, expected) in [
            ("new", None),
            ("existing", Some(CreateDisappearingTrigger::UnknownGroups)),
        ] {
            let node = NodeBuilder::new("create")
                .attr("type", kind)
                .children([group()
                    .children([NodeBuilder::new("ephemeral")
                        .attr("expiration", "12seconds")
                        .attr("trigger", "unknown")
                        .build()])
                    .build()])
                .build();
            let parsed = parse_group_create(&node.as_node_ref(), &context()).unwrap();
            let ephemeral = parsed.group_info.ephemerality.unwrap();
            assert_eq!(ephemeral.duration, 12);
            assert_eq!(ephemeral.trigger, expected);
        }
    }
    #[test]
    fn linked_parent_and_outer_auto_add_flags_have_distinct_paths() {
        let node = NodeBuilder::new("create")
            .attr("reason", "default_sub_group_admin_add")
            .children([
                group()
                    .children([
                        NodeBuilder::new("linked_parent")
                            .attr("jid", "123456@g.us")
                            .build(),
                        NodeBuilder::new("parent")
                            .attr("default_membership_approval_mode", "request_required")
                            .build(),
                        NodeBuilder::new("limit_sharing_enabled").build(),
                        NodeBuilder::new("member_link_mode")
                            .string_content("future_mode")
                            .build(),
                        NodeBuilder::new("member_add_mode")
                            .string_content("all_member_add")
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new("auto_add_disabled").build(),
            ])
            .build();
        let mut context = context();
        context.opus_enabled = true;
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert_eq!(
            parsed.reason.unwrap().normalized_value(),
            "default_subgroup_admin_add"
        );
        assert!(parsed.group_info.parent_group.is_some());
        assert!(parsed.group_info.is_parent_group_closed);
        assert!(parsed.group_info.general_chat_auto_add_disabled);
        assert!(!parsed.group_info.limit_sharing_enabled);
        assert!(parsed.group_info.member_link_mode.is_none());
        assert_eq!(
            parsed.group_info.member_add_mode,
            Some(MemberAddMode::AllMemberAdd)
        );
    }
    #[test]
    fn reason_rebinding_does_not_create_a_wire_alias() {
        assert_eq!(
            parse_create_reason("default_sub_group_admin_add"),
            Some(GroupCreateReason::DefaultSubgroupAdminAdd)
        );
        assert_eq!(parse_create_reason("default_subgroup_admin_add"), None);
        assert_eq!(
            GroupCreateReason::DefaultSubgroupAdminAdd.as_str(),
            "default_subgroup_admin_add"
        );
    }
    #[test]
    fn create_time_fields_use_official_signed32_wrapping() {
        let node = NodeBuilder::new("create")
            .children([group()
                .attr("creation", "4294967297")
                .children([NodeBuilder::new("participant")
                    .attr("jid", "100001@lid")
                    .attr("join_time", "2147483648")
                    .build()])
                .build()])
            .build();
        let mut context = context();
        context.history_after_join = true;
        let parsed = parse_group_create(&node.as_node_ref(), &context).unwrap();
        assert_eq!(parsed.group_info.creation, 1);
        assert_eq!(
            parsed.group_info.participants[0].join_time,
            Some(-2_147_483_647)
        );
    }
    #[test]
    fn empty_byte_description_reads_id_before_the_invalid_creator() {
        let node = NodeBuilder::new("create")
            .children([group()
                .attr("creator", "invalid")
                .children([NodeBuilder::new("description")
                    .children([NodeBuilder::new("body").bytes(vec![]).build()])
                    .build()])
                .build()])
            .build();
        assert_eq!(
            parse_group_create(&node.as_node_ref(), &context()).unwrap_err(),
            CreateParseError::Field("id")
        );
    }
}

#[cfg(test)]
#[path = "create_conformance.rs"]
mod conformance;
