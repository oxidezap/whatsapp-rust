//! Owned subgroup-suggestion projections from WAWebHandleGroupNotification.b(),
//! CREATED_SUBGROUP_SUGGESTION, REVOKED_SUB_GROUP_SUGGESTIONS and CHANGE_NUMBER.
//! Snapshot 2.3000.1047483476, handler bundle
//! 09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8,
//! module bytes1693402..1716842, SHA-256
//! a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1.
use super::GroupNotificationActionTag;
use super::identity::{self, IntegerReadError};
use crate::WireEnum;
use wacore_binary::{Jid, NodeContentRef, NodeRef};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSubgroupSuggestion {
    pub parent_group_id: Jid,
    pub id: Jid,
    pub owner: Jid,
    pub subject: String,
    pub description: Option<String>,
    pub creation: i64,
    pub is_existing_group: bool,
    pub participant_count: Option<i64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum RevokedSuggestionReason {
    #[wire = "approved"]
    Approved,
    #[wire = "rejected"]
    Rejected,
    #[wire = "cancelled"]
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedSubgroupSuggestion {
    pub id: Jid,
    pub owner: Jid,
    pub reason: Option<RevokedSuggestionReason>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokedSubgroupSuggestions {
    pub parent_group_id: Jid,
    pub suggestions: Vec<RevokedSubgroupSuggestion>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubgroupSuggestionsOwnerChanged {
    pub parent_group_id: Jid,
    pub suggestions: Vec<Jid>,
    pub old_owner: Jid,
    pub new_owner: Jid,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SuggestionParseError {
    #[error("missing or invalid {0}")]
    Field(&'static str),
    #[error("unsupported non-exact JavaScript integer in {0}")]
    NumericRange(&'static str),
    /// The upstream plain-object lookup returns a function/object for inherited
    /// keys. It is not an unknown enum string and cannot be silently coerced.
    #[error("unsupported inherited suggestion reason {0}")]
    InheritedReason(String),
}
type ParseResult<T> = Result<T, SuggestionParseError>;
fn text(node: &NodeRef<'_>, field: &'static str) -> ParseResult<String> {
    node.get_attr(field)
        .map(|v| v.as_str().into_owned())
        .ok_or(SuggestionParseError::Field(field))
}
fn jid(
    node: &NodeRef<'_>,
    field: &'static str,
    parse: fn(&str) -> Option<Jid>,
) -> ParseResult<Jid> {
    parse(&text(node, field)?).ok_or(SuggestionParseError::Field(field))
}
fn parent(parent: &Jid) -> ParseResult<Jid> {
    identity::parse_group(&parent.to_string()).ok_or(SuggestionParseError::Field("from"))
}
fn integer_result(value: Result<i64, IntegerReadError>, field: &'static str) -> ParseResult<i64> {
    value.map_err(|error| match error {
        IntegerReadError::NotInteger => SuggestionParseError::Field(field),
        IntegerReadError::OutsideExactRange => SuggestionParseError::NumericRange(field),
    })
}
fn has_content(node: &NodeRef<'_>) -> bool {
    match node.content.as_ref() {
        Some(NodeContentRef::Bytes(_)) => true,
        Some(NodeContentRef::String(s)) => !s.is_empty(),
        Some(NodeContentRef::Nodes(_)) | None => false,
    }
}
fn content(node: &NodeRef<'_>, field: &'static str) -> ParseResult<String> {
    if !has_content(node) {
        return Err(SuggestionParseError::Field(field));
    }
    node.content_as_string()
        .map(|v| v.to_string())
        .ok_or(SuggestionParseError::Field(field))
}
/// The source's helper b() reads subgroup id, creator, creation and subject before
/// any optional payload. Missing body content is ignored, not an empty sentinel.
pub fn parse_created_suggestion(
    node: &NodeRef<'_>,
    parent_group_id: &Jid,
) -> ParseResult<CreatedSubgroupSuggestion> {
    if !matches!(
        GroupNotificationActionTag::from(node.tag.as_ref()),
        GroupNotificationActionTag::CreatedSubGroupSuggestion
    ) {
        return Err(SuggestionParseError::Field("created_sub_group_suggestion"));
    }
    let parent_group_id = parent(parent_group_id)?;
    let suggestion = node
        .get_optional_child("sub_group_suggestion")
        .ok_or(SuggestionParseError::Field("sub_group_suggestion"))?;
    let id = jid(suggestion, "jid", identity::parse_group)?;
    let owner = jid(suggestion, "creator", identity::parse_user)?;
    let creation = integer_result(
        identity::parse_js_time(&text(suggestion, "creation")?),
        "creation",
    )?;
    let subject = suggestion
        .get_optional_child("subject")
        .ok_or(SuggestionParseError::Field("subject"))?;
    let subject = content(subject, "subject")?;
    let description = if let Some(body) = suggestion
        .get_optional_child("description")
        .and_then(|d| d.get_optional_child("body"))
        && has_content(body)
    {
        Some(content(body, "body")?)
    } else {
        None
    };
    let is_existing_group = if let Some(child) = suggestion.get_optional_child("is_existing_group")
        && has_content(child)
    {
        content(child, "is_existing_group")? == "true"
    } else {
        false
    };
    let participant_count = if let Some(child) = suggestion.get_optional_child("participant_count")
        && has_content(child)
    {
        Some(integer_result(
            identity::parse_js_integer(&content(child, "participant_count")?),
            "participant_count",
        )?)
    } else {
        None
    };
    Ok(CreatedSubgroupSuggestion {
        parent_group_id,
        id,
        owner,
        creation,
        subject,
        description,
        is_existing_group,
        participant_count,
    })
}
fn revoked_reason(value: &str) -> ParseResult<Option<RevokedSuggestionReason>> {
    if matches!(
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
    ) {
        return Err(SuggestionParseError::InheritedReason(value.into()));
    }
    Ok(RevokedSuggestionReason::try_from(value).ok())
}
/// Preserves source child order and distinguishes absent/unknown reasons from
/// inherited object-prototype values which the typed projection cannot model.
pub fn parse_revoked_suggestions(
    node: &NodeRef<'_>,
    parent_group_id: &Jid,
) -> ParseResult<RevokedSubgroupSuggestions> {
    if !matches!(
        GroupNotificationActionTag::from(node.tag.as_ref()),
        GroupNotificationActionTag::RevokedSubGroupSuggestions
    ) {
        return Err(SuggestionParseError::Field("revoked_sub_group_suggestions"));
    }
    let parent_group_id = parent(parent_group_id)?;
    if matches!(
        node.content.as_ref(),
        Some(NodeContentRef::Bytes(_) | NodeContentRef::String(_))
    ) {
        return Err(SuggestionParseError::Field("children"));
    }
    let suggestions = node
        .get_children_by_tag("sub_group_suggestion")
        .map(|s| {
            let id = jid(s, "jid", identity::parse_group)?;
            let owner = jid(s, "creator", identity::parse_user)?;
            let reason = s
                .get_attr("reason")
                .map(|v| revoked_reason(&v.as_str()))
                .transpose()?
                .flatten();
            Ok(RevokedSubgroupSuggestion { id, owner, reason })
        })
        .collect::<ParseResult<Vec<_>>>()?;
    Ok(RevokedSubgroupSuggestions {
        parent_group_id,
        suggestions,
    })
}
/// CHANGE_NUMBER's old owner is required on the envelope, and its new owner is
/// required on the action. The source reads both after subgroup ids.
pub fn parse_suggestions_change_number(
    node: &NodeRef<'_>,
    parent_group_id: &Jid,
    envelope_author: Option<&Jid>,
) -> ParseResult<SubgroupSuggestionsOwnerChanged> {
    if !matches!(
        GroupNotificationActionTag::from(node.tag.as_ref()),
        GroupNotificationActionTag::ChangeNumber
    ) {
        return Err(SuggestionParseError::Field("change_number"));
    }
    let parent_group_id = parent(parent_group_id)?;
    if matches!(
        node.content.as_ref(),
        Some(NodeContentRef::Bytes(_) | NodeContentRef::String(_))
    ) {
        return Err(SuggestionParseError::Field("children"));
    }
    let suggestions = node
        .get_children_by_tag("sub_group_suggestion")
        .map(|s| jid(s, "jid", identity::parse_group))
        .collect::<ParseResult<Vec<_>>>()?;
    let old_owner = envelope_author
        .and_then(|j| identity::parse_user(&j.to_string()))
        .ok_or(SuggestionParseError::Field("participant"))?;
    let new_owner = jid(node, "jid", identity::parse_user)?;
    Ok(SubgroupSuggestionsOwnerChanged {
        parent_group_id,
        suggestions,
        old_owner,
        new_owner,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    fn parent_jid() -> Jid {
        "123456@g.us".parse().unwrap()
    }
    fn suggestion() -> NodeBuilder {
        NodeBuilder::new("sub_group_suggestion")
            .attr("jid", "234567@g.us")
            .attr("creator", "200002@s.whatsapp.net")
            .attr("creation", "123")
    }
    #[test]
    fn created_suggestion_has_owned_data_and_optional_defaults() {
        let node = NodeBuilder::new("created_sub_group_suggestion")
            .children([suggestion()
                .children([
                    NodeBuilder::new("subject")
                        .string_content("synthetic subgroup")
                        .build(),
                    NodeBuilder::new("description")
                        .children([NodeBuilder::new("body").bytes(vec![]).build()])
                        .build(),
                    NodeBuilder::new("participant_count")
                        .string_content("12people")
                        .build(),
                ])
                .build()])
            .build();
        let parsed = parse_created_suggestion(&node.as_node_ref(), &parent_jid()).unwrap();
        assert_eq!(parsed.subject, "synthetic subgroup");
        assert_eq!(parsed.description.as_deref(), Some(""));
        assert!(!parsed.is_existing_group);
        assert_eq!(parsed.participant_count, Some(12));
    }
    #[test]
    fn malformed_identity_and_required_content_fail_in_source_order() {
        let node = NodeBuilder::new("created_sub_group_suggestion")
            .children([NodeBuilder::new("sub_group_suggestion")
                .attr("jid", "100001@lid")
                .build()])
            .build();
        assert_eq!(
            parse_created_suggestion(&node.as_node_ref(), &parent_jid()).unwrap_err(),
            SuggestionParseError::Field("jid")
        );
        let empty = NodeBuilder::new("created_sub_group_suggestion")
            .children([suggestion()
                .children([NodeBuilder::new("subject").string_content("").build()])
                .build()])
            .build();
        assert_eq!(
            parse_created_suggestion(&empty.as_node_ref(), &parent_jid()).unwrap_err(),
            SuggestionParseError::Field("subject")
        );
    }
    #[test]
    fn revoked_reasons_preserve_order_and_unknown_policy() {
        let node = NodeBuilder::new("revoked_sub_group_suggestions")
            .children([
                suggestion().attr("reason", "approved").build(),
                suggestion().attr("reason", "future_reason").build(),
                suggestion().build(),
            ])
            .build();
        let parsed = parse_revoked_suggestions(&node.as_node_ref(), &parent_jid()).unwrap();
        assert_eq!(
            parsed.suggestions[0].reason,
            Some(RevokedSuggestionReason::Approved)
        );
        assert!(parsed.suggestions[1].reason.is_none());
        assert!(parsed.suggestions[2].reason.is_none());
        for key in ["constructor", "__proto__", "toString"] {
            let node = NodeBuilder::new("revoked_sub_group_suggestions")
                .children([suggestion().attr("reason", key).build()])
                .build();
            assert_eq!(
                parse_revoked_suggestions(&node.as_node_ref(), &parent_jid()).unwrap_err(),
                SuggestionParseError::InheritedReason(key.into())
            );
        }
    }
    #[test]
    fn change_number_requires_both_owners_after_subgroup_ids() {
        let node = NodeBuilder::new("change_number")
            .attr("jid", "100002@lid")
            .children([NodeBuilder::new("sub_group_suggestion")
                .attr("jid", "234567@g.us")
                .build()])
            .build();
        assert_eq!(
            parse_suggestions_change_number(&node.as_node_ref(), &parent_jid(), None).unwrap_err(),
            SuggestionParseError::Field("participant")
        );
        let old_owner: Jid = "200002@bot".parse().unwrap();
        let parsed =
            parse_suggestions_change_number(&node.as_node_ref(), &parent_jid(), Some(&old_owner))
                .unwrap();
        assert_eq!(parsed.old_owner, old_owner);
        assert_eq!(parsed.new_owner.to_string(), "100002@lid");
        assert_eq!(parsed.suggestions.len(), 1);
    }
}
