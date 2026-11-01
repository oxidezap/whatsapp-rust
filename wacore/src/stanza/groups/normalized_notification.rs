//! Source-qualified envelope program plus owned action normalization. Runtime
//! cache, session, ACK and persistence effects are deliberately outside parsing.
use super::actions_normalized::{
    self, ActionContext, GroupActionParseError, NormalizedGroupAction,
};
use super::envelope_runtime::{self, Value};
use super::normalization_warning::{GroupWarningSink, IgnoreWarnings};
use super::normalized::{GroupParseContext, GroupParseError};
use wacore_binary::{Jid, NodeRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupNotificationDiagnostic {
    pub message_parts: Vec<&'static str>,
    pub error: GroupParseError,
}

/// Complete owned result for the qualified Web group-notification parser.
/// Optional Rust values collapse null/undefined only at this public data boundary;
/// the generated envelope program retains their distinct guard semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedGroupNotification {
    pub external_id: String,
    pub chat_id: Jid,
    pub is_lid_addressing_mode: bool,
    pub author: Option<Jid>,
    pub author_phone_number: Option<Jid>,
    pub author_username: Option<String>,
    pub author_username_country_code: Option<String>,
    pub timestamp: i64,
    pub pushname: Option<String>,
    pub offline: Option<String>,
    pub actions: Vec<NormalizedGroupAction>,
    pub has_incomplete_participant_information: bool,
    pub diagnostics: Vec<GroupNotificationDiagnostic>,
}
fn field<'a>(value: &'a Value, name: &'static str) -> Result<&'a Value, GroupParseError> {
    value.field(name).ok_or(GroupParseError::Field(name))
}
fn jid(value: &Value, name: &'static str) -> Result<Jid, GroupParseError> {
    field(value, name)?
        .as_jid()
        .cloned()
        .ok_or(GroupParseError::Field(name))
}
fn optional_jid(value: &Value, name: &'static str) -> Result<Option<Jid>, GroupParseError> {
    let v = field(value, name)?;
    if v.is_nullish() {
        Ok(None)
    } else {
        v.as_jid()
            .cloned()
            .map(Some)
            .ok_or(GroupParseError::Field(name))
    }
}
fn string(value: &Value, name: &'static str) -> Result<String, GroupParseError> {
    field(value, name)?
        .as_string()
        .map(ToOwned::to_owned)
        .ok_or(GroupParseError::Field(name))
}
fn optional_string(value: &Value, name: &'static str) -> Result<Option<String>, GroupParseError> {
    let v = field(value, name)?;
    if v.is_nullish() {
        Ok(None)
    } else {
        v.as_string()
            .map(|v| Some(v.to_owned()))
            .ok_or(GroupParseError::Field(name))
    }
}
fn integer(value: &Value, name: &'static str) -> Result<i64, GroupParseError> {
    field(value, name)?
        .as_integer()
        .ok_or(GroupParseError::Field(name))
}
fn boolean(value: &Value, name: &'static str) -> Result<bool, GroupParseError> {
    field(value, name)?
        .as_bool()
        .ok_or(GroupParseError::Field(name))
}
fn action_error(error: GroupActionParseError) -> GroupParseError {
    use super::create_normalized::CreateParseError;
    use super::suggestions_normalized::SuggestionParseError;
    match error {
        GroupActionParseError::Parse(error) => error,
        GroupActionParseError::Create(CreateParseError::Field(field))
        | GroupActionParseError::Suggestion(SuggestionParseError::Field(field)) => {
            GroupParseError::Field(field)
        }
        GroupActionParseError::Create(CreateParseError::NumericRange(field))
        | GroupActionParseError::Suggestion(SuggestionParseError::NumericRange(field)) => {
            GroupParseError::NumericRange(field)
        }
        other => GroupParseError::Action(Box::new(other)),
    }
}

pub fn parse_normalized_group_notification(
    node: &NodeRef<'_>,
    context: &GroupParseContext<'_>,
) -> Result<NormalizedGroupNotification, GroupParseError> {
    parse_normalized_group_notification_with_warnings(node, context, &mut IgnoreWarnings)
}

/// Like the pure parser, with an explicit ordered warning boundary. The sink
/// can fail; such failures propagate before/after payload reads as in Web.
pub fn parse_normalized_group_notification_with_warnings(
    node: &NodeRef<'_>,
    context: &GroupParseContext<'_>,
    warnings: &mut dyn GroupWarningSink,
) -> Result<NormalizedGroupNotification, GroupParseError> {
    let mut state = envelope_runtime::parse_prefix(node, context).map_err(|error| match error {
        GroupParseError::Rejected(_) => GroupParseError::Destination,
        other => other,
    })?;
    let facts = envelope_runtime::action_facts(&state)?;
    let group_jid = jid(&facts, "chatId")?;
    let author = optional_jid(&facts, "author")?;
    let action_context = ActionContext {
        group_jid: &group_jid,
        author: author.as_ref(),
        timestamp: integer(&facts, "timestamp")?,
        is_lid_addressing_mode: boolean(&facts, "isLidAddressingMode")?,
        username_displayed: context.username_displayed,
        history_after_join: context.history_after_join,
        is_me_account: context.is_me_account,
        opus_enabled: context.opus_enabled,
        normalize_country_code: context.normalize_country_code,
    };
    if node.content.is_some() && node.children().is_none() {
        return Err(GroupParseError::Field("children"));
    }
    let mut actions = vec![];
    for child in node.children().into_iter().flatten() {
        if child.tag.as_ref() == envelope_runtime::MISSING_IDENTIFICATION_TAG {
            state.mark_missing()?;
        } else if let Some(action) =
            actions_normalized::parse_action_result_with_warnings(child, &action_context, warnings)
                .map_err(action_error)?
        {
            actions.push(action);
        }
    }
    state.mark_actions()?;
    let result = envelope_runtime::finish_projection(node, context, &mut state)?;
    Ok(NormalizedGroupNotification {
        external_id: string(&result, "externalId")?,
        chat_id: jid(&result, "chatId")?,
        is_lid_addressing_mode: boolean(&result, "isLidAddressingMode")?,
        author: optional_jid(&result, "author")?,
        author_phone_number: optional_jid(&result, "authorPhoneNumber")?,
        author_username: optional_string(&result, "authorUsername")?,
        author_username_country_code: optional_string(&result, "authorUsernameCountryCode")?,
        timestamp: integer(&result, "ts")?,
        pushname: optional_string(&result, "pushname")?,
        offline: optional_string(&result, "offline")?,
        actions,
        has_incomplete_participant_information: boolean(
            &result,
            "hasIncompleteParticipantInformation",
        )?,
        diagnostics: state
            .diagnostics
            .into_iter()
            .map(|d| GroupNotificationDiagnostic {
                message_parts: d.message_parts.to_vec(),
                error: d.error,
            })
            .collect(),
    })
}
