//! Complete owned normalization of the qualified group-notification action switch.
//! Source: WAWebHandleGroupNotification, WA 2.3000.1052559453.
//! <https://github.com/oxidezap/whatspec/blob/ae3880925bacd23863cc30ec480c9f82bfd869f1/tests/conformance/groups/versions/2.3000.1052559453/semantic-review.json>.
//! Runtime cache/session changes, storage and ACK handling stay outside.
use super::create_normalized::{
    self, CreateParseError, CreateParticipant, GroupCreateContext, NormalizedGroupCreate,
};
use super::identity::{self, IntegerReadError};
use super::link_normalized::{self, GroupLinkKind, NormalizedLink};
use super::normalization_warning::{GroupWarningSink, IgnoreWarnings};
use super::normalized::{self, GroupParseError, GroupUnlinkKind, NormalizedUnlink};
use super::suggestions_normalized::{
    self, CreatedSubgroupSuggestion, RevokedSubgroupSuggestions, SubgroupSuggestionsOwnerChanged,
    SuggestionParseError,
};
use super::{GroupNotificationActionTag, MembershipRequestMethod};
use crate::WireEnum;
use crate::types::wire_enums::MemberAddMode;
use wacore_binary::{Jid, NodeContentRef, NodeRef};

/// Pure facts from the already validated envelope and effective account state.
/// The parser never reads mutable runtime state or guesses feature defaults.
pub struct ActionContext<'a> {
    pub group_jid: &'a Jid,
    pub author: Option<&'a Jid>,
    pub timestamp: i64,
    pub is_lid_addressing_mode: bool,
    pub username_displayed: bool,
    /// Retained for caller compatibility; this snapshot reads participant history unconditionally.
    pub history_after_join: bool,
    pub is_me_account: &'a dyn Fn(&Jid) -> bool,
    pub opus_enabled: bool,
    pub normalize_country_code: &'a dyn Fn(&str) -> Option<String>,
}
impl ActionContext<'_> {
    fn create_context(&self) -> GroupCreateContext<'_> {
        GroupCreateContext {
            username_displayed: self.username_displayed,
            history_after_join: self.history_after_join,
            opus_enabled: self.opus_enabled,
            envelope_author: self.author,
            is_me_account: self.is_me_account,
            normalize_country_code: self.normalize_country_code,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum GroupActionParseError {
    #[error("{0}")]
    Parse(#[from] GroupParseError),
    #[error("{0}")]
    Create(#[from] CreateParseError),
    #[error("{0}")]
    Suggestion(#[from] SuggestionParseError),
    /// Source r("nullthrows")(author) throws an ordinary Error, not a parsing
    /// failure, when the no-content request fallback has no envelope author.
    #[error("request fallback requires an envelope author")]
    MissingRequestFallbackAuthor,
}
type ParseResult<T> = Result<T, GroupActionParseError>;

/// Normalized discriminants are separate from the owned payload union. Each
/// string has one WireEnum definition; physical wire dispatch uses the existing
/// GroupNotificationActionTag enum, including its deliberately collapsed aliases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum NormalizedGroupActionType {
    #[wire = "create"]
    Create,
    #[wire = "add"]
    Add,
    #[wire = "delete"]
    Delete,
    #[wire = "remove"]
    Remove,
    #[wire = "promote"]
    Promote,
    #[wire = "demote"]
    Demote,
    #[wire = "linked_group_promote"]
    LinkedGroupPromote,
    #[wire = "linked_group_demote"]
    LinkedGroupDemote,
    #[wire = "modify"]
    Modify,
    #[wire = "subject"]
    Subject,
    #[wire = "desc_add"]
    DescriptionAdded,
    #[wire = "desc_remove"]
    DescriptionRemoved,
    #[wire = "suspend"]
    Suspend,
    #[wire = "restrict"]
    Restrict,
    #[wire = "announce"]
    Announce,
    #[wire = "no_frequently_forwarded"]
    NoFrequentlyForwarded,
    #[wire = "invite"]
    Invite,
    #[wire = "ephemeral"]
    Ephemeral,
    #[wire = "revoke_invite"]
    RevokeInvite,
    #[wire = "growth_locked"]
    GrowthLocked,
    #[wire = "growth_unlocked"]
    GrowthUnlocked,
    #[wire = "sub_group_link"]
    SubGroupLink,
    #[wire = "parent_group_link"]
    ParentGroupLink,
    #[wire = "sibling_group_link"]
    SiblingGroupLink,
    #[wire = "sub_group_unlink"]
    SubGroupUnlink,
    #[wire = "parent_group_unlink"]
    ParentGroupUnlink,
    #[wire = "sibling_group_unlink"]
    SiblingGroupUnlink,
    #[wire = "delete_parent_group_unlink"]
    DeleteParentGroupUnlink,
    #[wire = "integrity_parent_group_unlink"]
    IntegrityParentGroupUnlink,
    #[wire = "integrity_sub_group_unlink"]
    IntegritySubGroupUnlink,
    #[wire = "membership_approval_mode"]
    MembershipApprovalMode,
    #[wire = "membership_approval_request"]
    MembershipApprovalRequest,
    #[wire = "allow_admin_reports"]
    AllowAdminReports,
    #[wire = "admin_report_received"]
    AdminReportReceived,
    #[wire = "created_membership_requests"]
    CreatedMembershipRequests,
    #[wire = "revoked_membership_requests"]
    RevokedMembershipRequests,
    #[wire = "allow_non_admin_sub_group_creation"]
    AllowNonAdminSubgroupCreation,
    #[wire = "created_subgroup_suggestion"]
    CreatedSubgroupSuggestion,
    #[wire = "revoked_sub_group_suggestions"]
    RevokedSubgroupSuggestions,
    #[wire = "subgroup_suggestions_change_number"]
    SubgroupSuggestionsOwnerChanged,
    #[wire = "member_add_mode"]
    MemberAddMode,
    #[wire = "general_chat_auto_add_disabled"]
    GeneralChatAutoAddDisabled,
    #[wire = "group_safety_check"]
    GroupSafetyCheck,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupAddReason {
    #[wire = "invite"]
    Invite,
    #[wire = "accept"]
    Accept,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupRemoveReason {
    #[wire = "default_sub_group_demote"]
    DefaultSubgroupDemote,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupDeleteReason {
    #[wire = "integrity_delete_parent"]
    IntegrityDeleteParent,
    #[wire = "delete_parent"]
    DeleteParent,
}
/// Internal normalized values from WAWebRequestMethodType, distinct from wire
/// request_method strings used by the established public adapter enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum NormalizedMembershipRequestMethod {
    #[wire = "NonAdminAdd"]
    NonAdminAdd,
    #[wire = "InviteLink"]
    InviteLink,
    #[wire = "LinkedGroupJoin"]
    LinkedGroupJoin,
}
impl From<MembershipRequestMethod> for NormalizedMembershipRequestMethod {
    fn from(value: MembershipRequestMethod) -> Self {
        match value {
            MembershipRequestMethod::NonAdminAdd => Self::NonAdminAdd,
            MembershipRequestMethod::InviteLink => Self::InviteLink,
            MembershipRequestMethod::LinkedGroupJoin => Self::LinkedGroupJoin,
        }
    }
}
impl From<NormalizedMembershipRequestMethod> for MembershipRequestMethod {
    fn from(value: NormalizedMembershipRequestMethod) -> Self {
        match value {
            NormalizedMembershipRequestMethod::NonAdminAdd => Self::NonAdminAdd,
            NormalizedMembershipRequestMethod::InviteLink => Self::InviteLink,
            NormalizedMembershipRequestMethod::LinkedGroupJoin => Self::LinkedGroupJoin,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletedGroup {
    pub id: Jid,
    pub subject: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedInviteParticipant {
    pub id: Jid,
    pub expiration: i64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedUser {
    pub wid: Jid,
    pub username: Option<String>,
    /// Officially attrUserJid, unlike participant.phone_number's PhoneUserJid.
    pub phone_number: Option<Jid>,
}

/// All values are parsed and owned. This union carries no raw Node substitute.
/// Legacy event adapters can retain their established shapes separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedGroupAction {
    Create {
        data: Box<NormalizedGroupCreate>,
    },
    Add {
        participants: Vec<CreateParticipant>,
        reason: Option<GroupAddReason>,
        is_lid_addressing_mode: bool,
    },
    Delete {
        reason: Option<GroupDeleteReason>,
        group_datas: Vec<DeletedGroup>,
    },
    Remove {
        participants: Vec<CreateParticipant>,
        reason: Option<GroupRemoveReason>,
        is_lid_addressing_mode: bool,
    },
    Promote {
        participants: Vec<CreateParticipant>,
    },
    Demote {
        participants: Vec<CreateParticipant>,
    },
    LinkedGroupPromote {
        participants: Vec<CreateParticipant>,
        jid: Option<Jid>,
    },
    LinkedGroupDemote {
        participants: Vec<CreateParticipant>,
        jid: Option<Jid>,
    },
    Modify {
        participants: Vec<CreateParticipant>,
    },
    Subject {
        subject: String,
        owner: Option<Jid>,
        owner_pn: Option<Jid>,
        owner_username: Option<String>,
        subject_time: Option<i64>,
    },
    DescriptionAdded {
        id: String,
        description: Option<String>,
        timestamp: i64,
    },
    DescriptionRemoved {
        id: String,
    },
    Suspend {
        value: bool,
    },
    Restrict {
        value: bool,
        threshold: Option<String>,
    },
    Announce {
        value: bool,
    },
    NoFrequentlyForwarded {
        value: bool,
    },
    Invite {
        code: String,
    },
    /// NOT_EPHEMERAL omits trigger/initiatedByMe and fixes duration to zero.
    Ephemeral {
        duration: i64,
        trigger: Option<i64>,
        initiated_by_me: Option<bool>,
    },
    RevokeInvite {
        participants: Vec<RevokedInviteParticipant>,
    },
    GrowthLocked {
        expiration: i64,
        lock_type: String,
    },
    GrowthUnlocked,
    Link {
        data: NormalizedLink,
    },
    Unlink {
        data: NormalizedUnlink,
    },
    MembershipApprovalMode {
        value: bool,
        triggered: Option<String>,
    },
    MembershipApprovalRequest {
        request_method: NormalizedMembershipRequestMethod,
        parent_group_id: Option<Jid>,
    },
    AllowAdminReports {
        value: bool,
        should_skip_generation: bool,
        author: Option<Jid>,
        triggered: Option<String>,
    },
    AdminReportReceived {
        should_skip_generation: bool,
        timestamp: i64,
    },
    CreatedMembershipRequests {
        request_method: NormalizedMembershipRequestMethod,
        parent_group_id: Option<Jid>,
        requests: Vec<RequestedUser>,
        skip_generation: bool,
    },
    RevokedMembershipRequests {
        requests: Vec<Jid>,
    },
    AllowNonAdminSubgroupCreation {
        value: bool,
    },
    CreatedSubgroupSuggestion {
        data: CreatedSubgroupSuggestion,
    },
    RevokedSubgroupSuggestions {
        data: RevokedSubgroupSuggestions,
    },
    SubgroupSuggestionsOwnerChanged {
        data: SubgroupSuggestionsOwnerChanged,
    },
    MemberAddMode {
        mode: Option<MemberAddMode>,
    },
    GeneralChatAutoAddDisabled,
    GroupSafetyCheck {
        value: bool,
    },
}
impl NormalizedGroupAction {
    /// Ordinary unknown link/unlink types retain an absent action identity.
    /// Prototype-derived non-string identities are rejected before construction.
    pub fn action_type(&self) -> Option<NormalizedGroupActionType> {
        use NormalizedGroupActionType as K;
        Some(match self {
            Self::Create { .. } => K::Create,
            Self::Add { .. } => K::Add,
            Self::Delete { .. } => K::Delete,
            Self::Remove { .. } => K::Remove,
            Self::Promote { .. } => K::Promote,
            Self::Demote { .. } => K::Demote,
            Self::LinkedGroupPromote { .. } => K::LinkedGroupPromote,
            Self::LinkedGroupDemote { .. } => K::LinkedGroupDemote,
            Self::Modify { .. } => K::Modify,
            Self::Subject { .. } => K::Subject,
            Self::DescriptionAdded { .. } => K::DescriptionAdded,
            Self::DescriptionRemoved { .. } => K::DescriptionRemoved,
            Self::Suspend { .. } => K::Suspend,
            Self::Restrict { .. } => K::Restrict,
            Self::Announce { .. } => K::Announce,
            Self::NoFrequentlyForwarded { .. } => K::NoFrequentlyForwarded,
            Self::Invite { .. } => K::Invite,
            Self::Ephemeral { .. } => K::Ephemeral,
            Self::RevokeInvite { .. } => K::RevokeInvite,
            Self::GrowthLocked { .. } => K::GrowthLocked,
            Self::GrowthUnlocked => K::GrowthUnlocked,
            Self::Link { data } => match data.kind? {
                GroupLinkKind::SubGroup => K::SubGroupLink,
                GroupLinkKind::ParentGroup => K::ParentGroupLink,
                GroupLinkKind::SiblingGroup => K::SiblingGroupLink,
            },
            Self::Unlink { data } => match data.kind? {
                GroupUnlinkKind::SubGroup => K::SubGroupUnlink,
                GroupUnlinkKind::ParentGroup => K::ParentGroupUnlink,
                GroupUnlinkKind::SiblingGroup => K::SiblingGroupUnlink,
                GroupUnlinkKind::DeleteParentGroup => K::DeleteParentGroupUnlink,
                GroupUnlinkKind::IntegrityParentGroup => K::IntegrityParentGroupUnlink,
                GroupUnlinkKind::IntegritySubGroup => K::IntegritySubGroupUnlink,
            },
            Self::MembershipApprovalMode { .. } => K::MembershipApprovalMode,
            Self::MembershipApprovalRequest { .. } => K::MembershipApprovalRequest,
            Self::AllowAdminReports { .. } => K::AllowAdminReports,
            Self::AdminReportReceived { .. } => K::AdminReportReceived,
            Self::CreatedMembershipRequests { .. } => K::CreatedMembershipRequests,
            Self::RevokedMembershipRequests { .. } => K::RevokedMembershipRequests,
            Self::AllowNonAdminSubgroupCreation { .. } => K::AllowNonAdminSubgroupCreation,
            Self::CreatedSubgroupSuggestion { .. } => K::CreatedSubgroupSuggestion,
            Self::RevokedSubgroupSuggestions { .. } => K::RevokedSubgroupSuggestions,
            Self::SubgroupSuggestionsOwnerChanged { .. } => K::SubgroupSuggestionsOwnerChanged,
            Self::MemberAddMode { .. } => K::MemberAddMode,
            Self::GeneralChatAutoAddDisabled => K::GeneralChatAutoAddDisabled,
            Self::GroupSafetyCheck { .. } => K::GroupSafetyCheck,
        })
    }
}

fn text(node: &NodeRef<'_>, field: &'static str) -> ParseResult<String> {
    node.get_attr(field)
        .map(|v| v.as_str().into_owned())
        .ok_or_else(|| GroupParseError::Field(field).into())
}
fn maybe_text(node: &NodeRef<'_>, field: &str) -> Option<String> {
    node.get_attr(field).map(|v| v.as_str().into_owned())
}
fn integer_result(value: Result<i64, IntegerReadError>, field: &'static str) -> ParseResult<i64> {
    value.map_err(|error| match error {
        IntegerReadError::NotInteger => GroupParseError::Field(field).into(),
        IntegerReadError::OutsideExactRange => GroupParseError::NumericRange(field).into(),
    })
}
fn integer(node: &NodeRef<'_>, field: &'static str) -> ParseResult<i64> {
    integer_result(identity::parse_js_integer(&text(node, field)?), field)
}
fn maybe_integer(node: &NodeRef<'_>, field: &'static str) -> ParseResult<Option<i64>> {
    node.get_attr(field)
        .map(|_| integer(node, field))
        .transpose()
}
fn maybe_time(node: &NodeRef<'_>, field: &'static str) -> ParseResult<Option<i64>> {
    node.get_attr(field)
        .map(|v| integer_result(identity::parse_js_time(&v.as_str()), field))
        .transpose()
}
fn jid(
    node: &NodeRef<'_>,
    field: &'static str,
    parse: fn(&str) -> Option<Jid>,
) -> ParseResult<Jid> {
    parse(&text(node, field)?).ok_or_else(|| GroupParseError::Field(field).into())
}
fn maybe_jid(
    node: &NodeRef<'_>,
    field: &'static str,
    parse: fn(&str) -> Option<Jid>,
) -> ParseResult<Option<Jid>> {
    node.get_attr(field)
        .map(|v| parse(&v.as_str()).ok_or_else(|| GroupParseError::Field(field).into()))
        .transpose()
}
fn child<'node, 'source>(
    node: &'node NodeRef<'source>,
    tag: &'static str,
) -> ParseResult<&'node NodeRef<'source>> {
    node.get_optional_child(tag)
        .ok_or_else(|| GroupParseError::Field(tag).into())
}
fn content(node: &NodeRef<'_>, field: &'static str) -> ParseResult<String> {
    let valid = match node.content.as_ref() {
        Some(NodeContentRef::Bytes(_)) => true,
        Some(NodeContentRef::String(s)) => !s.is_empty(),
        Some(NodeContentRef::Nodes(_)) | None => false,
    };
    if !valid {
        return Err(GroupParseError::Field(field).into());
    }
    node.content_as_string()
        .map(|v| v.to_string())
        .ok_or_else(|| GroupParseError::Field(field).into())
}
fn mapped_children(node: &NodeRef<'_>) -> ParseResult<()> {
    if matches!(
        node.content.as_ref(),
        Some(NodeContentRef::Bytes(_) | NodeContentRef::String(_))
    ) {
        return Err(GroupParseError::Field("children").into());
    }
    Ok(())
}
fn add_reason(value: &str) -> Option<GroupAddReason> {
    if value == "default_sub_group_admin_add" {
        return Some(GroupAddReason::DefaultSubgroupAdminAdd);
    }
    GroupAddReason::try_from(value)
        .ok()
        .filter(|v| *v != GroupAddReason::DefaultSubgroupAdminAdd)
}
fn prototype_key(value: &str) -> bool {
    matches!(
        value,
        "constructor"
            | "__defineGetter__"
            | "__defineSetter__"
            | "hasOwnProperty"
            | "__lookupGetter__"
            | "__lookupSetter__"
            | "isPrototypeOf"
            | "propertyIsEnumerable"
            | "toLocaleString"
            | "toString"
            | "valueOf"
            | "__proto__"
    )
}
fn request_method(
    node: &NodeRef<'_>,
) -> ParseResult<Result<NormalizedMembershipRequestMethod, GroupParseError>> {
    let value = text(node, "request_method")?;
    Ok(if prototype_key(&value) {
        Err(GroupParseError::UnsupportedActionType("request_method"))
    } else {
        Ok(MembershipRequestMethod::try_from(value.as_str())
            .unwrap_or(MembershipRequestMethod::InviteLink)
            .into())
    })
}

/// Parse one action in source order. The marker returns no action; the enclosing
/// parser separately records its presence and applies the username-display gate.
pub fn parse_action_result(
    node: &NodeRef<'_>,
    context: &ActionContext<'_>,
) -> ParseResult<Option<NormalizedGroupAction>> {
    parse_action_result_with_warnings(node, context, &mut IgnoreWarnings)
}

pub fn parse_action_result_with_warnings(
    node: &NodeRef<'_>,
    context: &ActionContext<'_>,
    warnings: &mut dyn GroupWarningSink,
) -> ParseResult<Option<NormalizedGroupAction>> {
    use GroupNotificationActionTag as T;
    if node.tag.as_ref() == super::MISSING_PARTICIPANT_IDENTIFICATION_TAG {
        return Ok(None);
    }
    let tag = T::from(node.tag.as_ref());
    let participants = || {
        create_normalized::parse_participants(node, &context.create_context())
            .map_err(GroupActionParseError::from)
    };
    let action = match tag {
        T::Create => NormalizedGroupAction::Create {
            data: Box::new(create_normalized::parse_group_create(
                node,
                &context.create_context(),
            )?),
        },
        T::Add => {
            let participants = participants()?;
            let reason = maybe_text(node, "reason").and_then(|v| add_reason(&v));
            NormalizedGroupAction::Add {
                participants,
                reason,
                is_lid_addressing_mode: context.is_lid_addressing_mode,
            }
        }
        T::Delete => {
            let reason = maybe_text(node, "reason")
                .and_then(|v| GroupDeleteReason::try_from(v.as_str()).ok());
            NormalizedGroupAction::Delete {
                reason,
                group_datas: vec![DeletedGroup {
                    id: context.group_jid.clone(),
                    subject: String::new(),
                }],
            }
        }
        T::Remove => {
            let participants = participants()?;
            let reason = maybe_text(node, "reason")
                .and_then(|v| GroupRemoveReason::try_from(v.as_str()).ok());
            NormalizedGroupAction::Remove {
                participants,
                reason,
                is_lid_addressing_mode: context.is_lid_addressing_mode,
            }
        }
        T::Promote => NormalizedGroupAction::Promote {
            participants: participants()?,
        },
        T::Demote => NormalizedGroupAction::Demote {
            participants: participants()?,
        },
        T::LinkedGroupPromote => {
            let participants = participants()?;
            let jid = maybe_jid(node, "jid", identity::parse_group)?;
            NormalizedGroupAction::LinkedGroupPromote { participants, jid }
        }
        T::LinkedGroupDemote => {
            let participants = participants()?;
            let jid = maybe_jid(node, "jid", identity::parse_group)?;
            NormalizedGroupAction::LinkedGroupDemote { participants, jid }
        }
        T::Modify => NormalizedGroupAction::Modify {
            participants: participants()?,
        },
        T::Subject => {
            let subject = text(node, "subject")?;
            let owner = maybe_jid(node, "s_o", identity::parse_user)?;
            let owner_pn = maybe_jid(node, "s_o_pn", identity::parse_user)?;
            let owner_username = maybe_text(node, "s_o_username");
            let subject_time = maybe_time(node, "s_t")?;
            NormalizedGroupAction::Subject {
                subject,
                owner,
                owner_pn,
                owner_username,
                subject_time,
            }
        }
        T::Description => {
            let id = text(node, "id")?;
            if node.get_optional_child("delete").is_some() {
                NormalizedGroupAction::DescriptionRemoved { id }
            } else {
                let description = node
                    .get_optional_child("body")
                    .map(|body| content(body, "body"))
                    .transpose()?;
                NormalizedGroupAction::DescriptionAdded {
                    id,
                    description,
                    timestamp: context.timestamp,
                }
            }
        }
        T::Suspended => NormalizedGroupAction::Suspend { value: true },
        T::Unsuspended => NormalizedGroupAction::Suspend { value: false },
        T::Locked => NormalizedGroupAction::Restrict {
            value: true,
            threshold: maybe_text(node, "threshold"),
        },
        T::Unlocked => NormalizedGroupAction::Restrict {
            value: false,
            threshold: None,
        },
        T::Announce => NormalizedGroupAction::Announce { value: true },
        T::NotAnnounce => NormalizedGroupAction::Announce { value: false },
        T::NoFrequentlyForwarded => NormalizedGroupAction::NoFrequentlyForwarded { value: true },
        T::FrequentlyForwardedOk => NormalizedGroupAction::NoFrequentlyForwarded { value: false },
        T::Invite => NormalizedGroupAction::Invite {
            code: text(node, "code")?,
        },
        T::Ephemeral => {
            // The public tag enum folds the NOT_EPHEMERAL alias. Compare with
            // its generated canonical spelling to preserve the two source arms.
            if node.tag.as_ref() == T::Ephemeral.as_str() {
                let duration = integer(node, "expiration")?;
                let trigger = maybe_integer(node, "trigger")?;
                let initiated_by_me = Some(context.author.is_some_and(context.is_me_account));
                NormalizedGroupAction::Ephemeral {
                    duration,
                    trigger,
                    initiated_by_me,
                }
            } else {
                NormalizedGroupAction::Ephemeral {
                    duration: 0,
                    trigger: None,
                    initiated_by_me: None,
                }
            }
        }
        T::RevokeInvite => {
            mapped_children(node)?;
            let mut participants = vec![];
            for participant in node.get_children_by_tag("participant") {
                let id = jid(participant, "jid", identity::parse_user)?;
                let expiration = integer(participant, "expiration")?;
                participants.push(RevokedInviteParticipant { id, expiration });
            }
            NormalizedGroupAction::RevokeInvite { participants }
        }
        T::GrowthUnlocked => NormalizedGroupAction::GrowthUnlocked,
        T::GrowthLocked => {
            let expiration = integer(node, "expiration")?;
            let lock_type = text(node, "type")?;
            NormalizedGroupAction::GrowthLocked {
                expiration,
                lock_type,
            }
        }
        T::Link => {
            let data = link_normalized::parse_link_with_warnings(node, warnings)?;
            if data.kind.is_none() {
                return Ok(None);
            }
            NormalizedGroupAction::Link { data }
        }
        T::Unlink => {
            let data = normalized::parse_unlink_with_warnings(node, warnings)?;
            if data.kind.is_none() {
                return Ok(None);
            }
            NormalizedGroupAction::Unlink { data }
        }
        T::MembershipApprovalMode => {
            let group_join = child(node, "group_join")?;
            let value = text(group_join, "state")? == "on";
            let triggered = maybe_text(node, "triggered");
            NormalizedGroupAction::MembershipApprovalMode { value, triggered }
        }
        T::MembershipApprovalRequest => {
            let request_method = request_method(node)?;
            let parent_group_id = maybe_jid(node, "parent_group_jid", identity::parse_group)?;
            NormalizedGroupAction::MembershipApprovalRequest {
                request_method: request_method?,
                parent_group_id,
            }
        }
        T::AllowAdminReports | T::NotAllowAdminReports => {
            NormalizedGroupAction::AllowAdminReports {
                value: matches!(tag, T::AllowAdminReports),
                should_skip_generation: false,
                author: context.author.cloned(),
                triggered: maybe_text(node, "triggered"),
            }
        }
        T::Reports => NormalizedGroupAction::AdminReportReceived {
            should_skip_generation: true,
            timestamp: context.timestamp,
        },
        T::CreatedMembershipRequests => {
            let request_method = request_method(node)?;
            let parent_group_id = maybe_jid(node, "parent_group_jid", identity::parse_group)?;
            let requests = if matches!(node.content.as_ref(), Some(NodeContentRef::Nodes(_))) {
                let mut requests = vec![];
                for requested in node.get_children_by_tag("requested_user") {
                    let wid = jid(requested, "jid", identity::parse_user)?;
                    let username = if context.username_displayed {
                        maybe_text(requested, "username")
                    } else {
                        None
                    };
                    let phone_number = maybe_jid(requested, "phone_number", identity::parse_user)?;
                    requests.push(RequestedUser {
                        wid,
                        username,
                        phone_number,
                    });
                }
                requests
            } else {
                vec![RequestedUser {
                    wid: context
                        .author
                        .cloned()
                        .ok_or(GroupActionParseError::MissingRequestFallbackAuthor)?,
                    username: None,
                    phone_number: None,
                }]
            };
            let skip_generation = maybe_text(node, "suppress_sys_msg").as_deref() == Some("true");
            NormalizedGroupAction::CreatedMembershipRequests {
                request_method: request_method?,
                parent_group_id,
                requests,
                skip_generation,
            }
        }
        T::RevokedMembershipRequests => {
            mapped_children(node)?;
            let mut requests = vec![];
            for participant in node.get_children_by_tag("participant") {
                requests.push(jid(participant, "jid", identity::parse_user)?);
            }
            NormalizedGroupAction::RevokedMembershipRequests { requests }
        }
        T::AllowNonAdminSubGroupCreation | T::NotAllowNonAdminSubGroupCreation => {
            NormalizedGroupAction::AllowNonAdminSubgroupCreation {
                value: matches!(tag, T::AllowNonAdminSubGroupCreation),
            }
        }
        T::CreatedSubGroupSuggestion => NormalizedGroupAction::CreatedSubgroupSuggestion {
            data: suggestions_normalized::parse_created_suggestion(node, context.group_jid)?,
        },
        T::RevokedSubGroupSuggestions => NormalizedGroupAction::RevokedSubgroupSuggestions {
            data: suggestions_normalized::parse_revoked_suggestions(node, context.group_jid)?,
        },
        T::ChangeNumber => NormalizedGroupAction::SubgroupSuggestionsOwnerChanged {
            data: suggestions_normalized::parse_suggestions_change_number(
                node,
                context.group_jid,
                context.author,
            )?,
        },
        T::MemberAddMode => NormalizedGroupAction::MemberAddMode {
            mode: MemberAddMode::try_from(content(node, "member_add_mode")?.as_str()).ok(),
        },
        T::AutoAddDisabled => NormalizedGroupAction::GeneralChatAutoAddDisabled,
        T::GroupSafetyCheck => NormalizedGroupAction::GroupSafetyCheck { value: true },
        // These legacy wire types have no arm in this snapshot's official
        // handler. The normalized parser must not invent successful actions.
        T::IsCapiHostedGroup | T::LimitSharingEnabled | T::Unknown(_) => {
            return Err(GroupParseError::UnknownAction(node.tag.to_string()).into());
        }
    };
    Ok(Some(action))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    fn me(jid: &Jid) -> bool {
        jid.user == "200002"
    }
    fn country(_: &str) -> Option<String> {
        None
    }
    fn context<'a>(group: &'a Jid, author: Option<&'a Jid>) -> ActionContext<'a> {
        ActionContext {
            group_jid: group,
            author,
            timestamp: 123,
            is_lid_addressing_mode: true,
            username_displayed: false,
            history_after_join: false,
            is_me_account: &me,
            opus_enabled: false,
            normalize_country_code: &country,
        }
    }
    #[test]
    fn paired_tags_collapse_and_ephemeral_alias_ignores_wire_attributes() {
        let group = "123456@g.us".parse().unwrap();
        let author = "200002@s.whatsapp.net".parse().unwrap();
        let ctx = context(&group, Some(&author));
        for (tag, expected) in [("locked", true), ("unlocked", false)] {
            let node = NodeBuilder::new(tag).build();
            let action = parse_action_result(&node.as_node_ref(), &ctx)
                .unwrap()
                .unwrap();
            assert_eq!(
                action.action_type(),
                Some(NormalizedGroupActionType::Restrict)
            );
            assert!(matches!(action,NormalizedGroupAction::Restrict{value,..} if value==expected));
        }
        let node = NodeBuilder::new("not_ephemeral")
            .attr("expiration", "999")
            .attr("trigger", "7")
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx).unwrap(),
            Some(NormalizedGroupAction::Ephemeral {
                duration: 0,
                trigger: None,
                initiated_by_me: None
            })
        ));
        let node = NodeBuilder::new("ephemeral")
            .attr("expiration", " -2seconds")
            .attr("trigger", "3trailing")
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx).unwrap(),
            Some(NormalizedGroupAction::Ephemeral {
                duration: -2,
                trigger: Some(3),
                initiated_by_me: Some(true)
            })
        ));
    }
    #[test]
    fn description_and_admin_reports_keep_envelope_derived_values() {
        let group = "123456@g.us".parse().unwrap();
        let author = "200002@s.whatsapp.net".parse().unwrap();
        let ctx = context(&group, Some(&author));
        let node = NodeBuilder::new("description")
            .attr("id", "synthetic")
            .children([NodeBuilder::new("body")
                .string_content("description")
                .build()])
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx).unwrap(),
            Some(NormalizedGroupAction::DescriptionAdded {
                timestamp: 123,
                description: Some(_),
                ..
            })
        ));
        let node = NodeBuilder::new("reports").build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx).unwrap(),
            Some(NormalizedGroupAction::AdminReportReceived {
                timestamp: 123,
                should_skip_generation: true
            })
        ));
        let node = NodeBuilder::new("not_allow_admin_reports")
            .attr("triggered", "synthetic")
            .build();
        let Some(NormalizedGroupAction::AllowAdminReports {
            value,
            author: actual,
            triggered,
            should_skip_generation,
        }) = parse_action_result(&node.as_node_ref(), &ctx).unwrap()
        else {
            panic!("reports")
        };
        assert!(!value && !should_skip_generation);
        assert_eq!(actual, Some(author));
        assert_eq!(triggered.as_deref(), Some("synthetic"));
    }
    #[test]
    fn created_requests_distinguish_no_children_empty_array_and_scalars() {
        let group = "123456@g.us".parse().unwrap();
        let author = "200002@s.whatsapp.net".parse().unwrap();
        let ctx = context(&group, Some(&author));
        for node in [
            NodeBuilder::new("created_membership_requests")
                .attr("request_method", "future")
                .build(),
            NodeBuilder::new("created_membership_requests")
                .attr("request_method", "future")
                .string_content("scalar")
                .build(),
        ] {
            let Some(NormalizedGroupAction::CreatedMembershipRequests {
                requests,
                request_method,
                ..
            }) = parse_action_result(&node.as_node_ref(), &ctx).unwrap()
            else {
                panic!("requests")
            };
            assert_eq!(requests.len(), 1);
            assert_eq!(requests[0].wid, author);
            assert_eq!(
                request_method,
                NormalizedMembershipRequestMethod::InviteLink
            );
        }
        let node = NodeBuilder::new("created_membership_requests")
            .attr("request_method", "invite_link")
            .children([])
            .build();
        assert!(
            matches!(parse_action_result(&node.as_node_ref(),&ctx).unwrap(),Some(NormalizedGroupAction::CreatedMembershipRequests{requests,..}) if requests.is_empty())
        );
        let node = NodeBuilder::new("created_membership_requests")
            .attr("request_method", "invite_link")
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &context(&group, None)),
            Err(GroupActionParseError::MissingRequestFallbackAuthor)
        ));
    }
    #[test]
    fn requested_user_fields_are_gated_and_phone_alias_is_generic_user() {
        let group = "123456@g.us".parse().unwrap();
        let ctx = context(&group, None);
        let node = NodeBuilder::new("created_membership_requests")
            .attr("request_method", "invite_link")
            .attr("suppress_sys_msg", "true")
            .children([NodeBuilder::new("requested_user")
                .attr("jid", "100001@lid")
                .attr("phone_number", "100002@lid")
                .attr("username", "synthetic")
                .build()])
            .build();
        let Some(NormalizedGroupAction::CreatedMembershipRequests {
            requests,
            skip_generation,
            ..
        }) = parse_action_result(&node.as_node_ref(), &ctx).unwrap()
        else {
            panic!("requests")
        };
        assert!(skip_generation);
        assert!(requests[0].username.is_none());
        assert!(requests[0].phone_number.as_ref().unwrap().is_lid());
    }
    #[test]
    fn request_method_prototype_rejection_preserves_later_payload_error_order() {
        let group = "123456@g.us".parse().unwrap();
        let ctx = context(&group, None);
        let node = NodeBuilder::new("membership_approval_request")
            .attr("request_method", "toString")
            .attr("parent_group_jid", "200002@lid")
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx),
            Err(GroupActionParseError::Parse(GroupParseError::Field(
                "parent_group_jid"
            )))
        ));
        let node = NodeBuilder::new("membership_approval_request")
            .attr("request_method", "toString")
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx),
            Err(GroupActionParseError::Parse(
                GroupParseError::UnsupportedActionType("request_method")
            ))
        ));
    }
    #[test]
    fn strict_required_payloads_and_scalar_mapping_are_not_silently_defaulted() {
        let group = "123456@g.us".parse().unwrap();
        let ctx = context(&group, None);
        for (node, field) in [
            (NodeBuilder::new("subject").build(), "subject"),
            (
                NodeBuilder::new("membership_approval_mode").build(),
                "group_join",
            ),
            (
                NodeBuilder::new("membership_approval_request").build(),
                "request_method",
            ),
            (
                NodeBuilder::new("revoke").string_content("").build(),
                "children",
            ),
            (
                NodeBuilder::new("growth_locked")
                    .attr("expiration", "not_integer")
                    .build(),
                "expiration",
            ),
        ] {
            assert!(
                matches!(parse_action_result(&node.as_node_ref(),&ctx),Err(GroupActionParseError::Parse(GroupParseError::Field(actual))) if actual==field)
            );
        }
        for tag in ["is_capi_hosted_group", "limit_sharing_enabled", "future"] {
            let node = NodeBuilder::new(tag).build();
            assert!(matches!(
                parse_action_result(&node.as_node_ref(), &ctx),
                Err(GroupActionParseError::Parse(
                    GroupParseError::UnknownAction(_)
                ))
            ));
        }
    }
    #[test]
    fn revoke_payload_and_linked_group_identity_are_preserved() {
        let group = "123456@g.us".parse().unwrap();
        let ctx = context(&group, None);
        let node = NodeBuilder::new("revoke")
            .children([NodeBuilder::new("participant")
                .attr("jid", "100001@lid")
                .attr("expiration", " +5days")
                .build()])
            .build();
        assert!(
            matches!(parse_action_result(&node.as_node_ref(),&ctx).unwrap(),Some(NormalizedGroupAction::RevokeInvite{participants}) if participants.len()==1&&participants[0].expiration==5)
        );
        let node = NodeBuilder::new("linked_group_promote")
            .attr("jid", "987654@g.us")
            .children([])
            .build();
        assert!(matches!(
            parse_action_result(&node.as_node_ref(), &ctx).unwrap(),
            Some(NormalizedGroupAction::LinkedGroupPromote { jid: Some(_), .. })
        ));
    }
}
