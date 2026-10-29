//! Qualified LINK payload normalization. The public legacy action can still
//! retain its raw Node; callers opt into this parsed, source-backed projection.
use super::GroupNotificationActionTag;
use super::identity::{IntegerReadError, parse_group, parse_js_integer};
use super::normalized::GroupParseError;
use crate::WireEnum;
use wacore_binary::{Jid, NodeRef};

#[path = "link_generated.rs"]
mod generated;

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum GroupLinkKind {
    #[wire = "sub_group_link"]
    SubGroup,
    #[wire = "parent_group_link"]
    ParentGroup,
    #[wire = "sibling_group_link"]
    SiblingGroup,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedGroup {
    pub jid: Jid,
    pub subject: String,
    pub subject_time: i64,
    /// None for parent links, even if the wire contains a hidden-group child.
    /// Some(false) records a presence read with no matching child.
    pub hidden_subgroup: Option<bool>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedLink {
    /// Ordinary unknown type strings produce an undefined action upstream.
    /// Inherited prototype properties are rejected as unqualified non-strings.
    pub kind: Option<GroupLinkKind>,
    pub groups: Vec<LinkedGroup>,
}

pub fn parse_link(node: &NodeRef<'_>) -> Result<NormalizedLink, GroupParseError> {
    if !matches!(
        GroupNotificationActionTag::from(node.tag.as_ref()),
        GroupNotificationActionTag::Link
    ) {
        return Err(GroupParseError::UnknownAction(node.tag.to_string()));
    }
    let kind = node
        .get_attr(generated::LINK_TYPE_ATTRIBUTE)
        .ok_or(GroupParseError::Field(generated::LINK_TYPE_ATTRIBUTE))?
        .as_str();
    // Keep payload read/error order even when a plain-object lookup produced an
    // unsupported action identity. Official JS constructs groupDatas afterward.
    let action = generated::normalize_link(&kind);
    let hidden_child = generated::hidden_subgroup_child(&kind);
    if node.content.is_some() && node.children().is_none() {
        return Err(GroupParseError::Field("children"));
    }
    let mut groups = vec![];
    for child in node.get_children_by_tag(generated::GROUP_CHILD_TAG) {
        let raw = child
            .get_attr(generated::GROUP_JID_ATTRIBUTE)
            .ok_or(GroupParseError::Field(generated::GROUP_JID_ATTRIBUTE))?
            .as_str();
        let jid =
            parse_group(&raw).ok_or(GroupParseError::Field(generated::GROUP_JID_ATTRIBUTE))?;
        let subject = child
            .get_attr(generated::GROUP_SUBJECT_ATTRIBUTE)
            .ok_or(GroupParseError::Field(generated::GROUP_SUBJECT_ATTRIBUTE))?
            .as_str()
            .into_owned();
        let time = child
            .get_attr(generated::GROUP_SUBJECT_TIME_ATTRIBUTE)
            .ok_or(GroupParseError::Field(
                generated::GROUP_SUBJECT_TIME_ATTRIBUTE,
            ))?
            .as_str();
        let subject_time = parse_js_integer(&time).map_err(|error| match error {
            IntegerReadError::NotInteger => {
                GroupParseError::Field(generated::GROUP_SUBJECT_TIME_ATTRIBUTE)
            }
            IntegerReadError::OutsideExactRange => {
                GroupParseError::NumericRange(generated::GROUP_SUBJECT_TIME_ATTRIBUTE)
            }
        })?;
        let hidden_subgroup = hidden_child.map(|tag| child.get_optional_child(tag).is_some());
        groups.push(LinkedGroup {
            jid,
            subject,
            subject_time,
            hidden_subgroup,
        });
    }
    let kind = action?
        .map(|value| {
            GroupLinkKind::try_from(value)
                .map_err(|_| GroupParseError::Field("generated link action"))
        })
        .transpose()?;
    Ok(NormalizedLink { kind, groups })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    #[test]
    fn group_data_errors_precede_unsupported_prototype_identity() {
        let node = NodeBuilder::new("link")
            .attr("link_type", "toString")
            .children([NodeBuilder::new("group").attr("jid", "200002@lid").build()])
            .build();
        assert_eq!(
            parse_link(&node.as_node_ref()).unwrap_err(),
            GroupParseError::Field("jid")
        );
    }
    #[test]
    fn subject_time_uses_official_js_decimal_prefix() {
        let node = NodeBuilder::new("link")
            .attr("link_type", "sub_group")
            .children([NodeBuilder::new("group")
                .attr("jid", "123456@g.us")
                .attr("subject", "synthetic")
                .attr("s_t", " +123seconds")
                .build()])
            .build();
        let actual = parse_link(&node.as_node_ref()).unwrap();
        assert_eq!(actual.groups[0].subject_time, 123);
        assert_eq!(actual.groups[0].hidden_subgroup, Some(false));
    }
}
