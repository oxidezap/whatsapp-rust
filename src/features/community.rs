//! Community feature.
//!
//! Communities are parent groups that contain linked subgroups.
//! Uses the `w:g2` IQ namespace for mutations and MEX (GraphQL) for metadata queries.

use crate::client::Client;
use crate::features::groups::GroupError;
use crate::features::groups::GroupMetadata;
use crate::features::groups::GroupParticipant;
use crate::features::groups::GroupParticipantOptions;
use crate::features::groups::ParticipantChangeResponse;
use crate::features::groups::PreviousDescription;
use crate::features::mex::{MexError, mex_request};
use crate::request::IqError;
use log::warn;
use thiserror::Error;
use wacore::iq::groups::{
    CommunityParticipatingIq, DeleteCommunityIq, GetLinkedGroupsParticipantsIq, GroupCreateOptions,
    JoinLinkedGroupIq, LinkSubgroupsIq, QueryLinkedGroupIq, UnlinkSubgroupsIq,
};
use wacore::iq::mex_operations::{fetch_all_subgroups, query_subgroup_participant_count};
use wacore_binary::Jid;

/// Error returned by community operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CommunityError {
    /// A `w:g2` IQ to the server failed.
    #[error(transparent)]
    Iq(#[from] IqError),
    /// A MEX (GraphQL) metadata query/mutation failed or returned bad data.
    #[error(transparent)]
    Mex(#[from] MexError),
    /// A delegated group operation failed (e.g. setting the community description).
    #[error(transparent)]
    Group(#[from] GroupError),
    /// The request was malformed or the server response was missing required data.
    #[error("invalid community request: {0}")]
    InvalidRequest(String),
}

// Types

/// Classification of a group within the community hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupType {
    /// Regular standalone group (not part of a community).
    Default,
    /// Community parent group.
    Community,
    /// A subgroup linked to a community.
    LinkedSubgroup,
    /// The default announcement subgroup of a community.
    LinkedAnnouncementGroup,
    /// The general chat subgroup of a community.
    LinkedGeneralGroup,
}

/// Options for creating a new community.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateCommunityOptions {
    pub name: String,
    pub description: Option<String>,
    /// Whether the community is closed (requires approval to join).
    pub closed: bool,
    /// Allow non-admin members to create subgroups.
    pub allow_non_admin_sub_group_creation: bool,
    /// Create a general chat subgroup alongside the community.
    pub create_general_chat: bool,
}

impl CreateCommunityOptions {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            closed: false,
            allow_non_admin_sub_group_creation: false,
            create_general_chat: true,
        }
    }
}

/// Result of creating a community.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CreateCommunityResult {
    pub metadata: GroupMetadata,
}

/// A subgroup within a community.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CommunitySubgroup {
    pub id: Jid,
    pub subject: String,
    pub participant_count: Option<u32>,
    /// Server-reported subgroup creation timestamp, when available.
    pub creation: Option<u64>,
    /// Server-reported subgroup creator, when available.
    pub owner: Option<Jid>,
    pub is_default_sub_group: bool,
    pub is_general_chat: bool,
}

/// Result of linking subgroups to a community.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LinkSubgroupsResult {
    pub linked_jids: Vec<Jid>,
    pub failed_groups: Vec<(Jid, u32)>,
}

/// Result of unlinking subgroups from a community.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnlinkSubgroupsResult {
    pub unlinked_jids: Vec<Jid>,
    pub failed_groups: Vec<(Jid, u32)>,
}

/// Determine the group type from metadata fields.
pub fn group_type(metadata: &GroupMetadata) -> GroupType {
    if metadata.is_default_sub_group {
        GroupType::LinkedAnnouncementGroup
    } else if metadata.is_general_chat {
        GroupType::LinkedGeneralGroup
    } else if metadata.parent_group_jid.is_some() {
        GroupType::LinkedSubgroup
    } else if metadata.is_parent_group {
        GroupType::Community
    } else {
        GroupType::Default
    }
}

// Feature handle

pub struct Community<'a> {
    client: &'a Client,
}

impl<'a> Community<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create a new community.
    ///
    /// If a description is provided, it is set via a follow-up IQ after creation
    /// (the group create stanza does not support inline descriptions for communities).
    pub async fn create(
        &self,
        options: CreateCommunityOptions,
    ) -> Result<CreateCommunityResult, CommunityError> {
        let description = options.description.clone();

        let create_options = GroupCreateOptions {
            subject: options.name,
            is_parent: true,
            closed: options.closed,
            allow_non_admin_sub_group_creation: options.allow_non_admin_sub_group_creation,
            create_general_chat: options.create_general_chat,
            ..Default::default()
        };

        let mut metadata = self
            .client
            .groups()
            .create_group(create_options)
            .await?
            .metadata;

        if let Some(desc_text) = description
            && let Ok(desc) = wacore::iq::groups::GroupDescription::new(&desc_text)
        {
            self.client
                .groups()
                // The group was just created, so nothing can have set a
                // description ahead of us.
                .set_description(&metadata.id, Some(desc), PreviousDescription::Absent)
                .await?;
            metadata.description = Some(desc_text);
        }

        Ok(CreateCommunityResult { metadata })
    }

    /// Create a subgroup already linked to a parent group.
    pub async fn create_subgroup(
        &self,
        name: impl Into<String>,
        participants: &[Jid],
        parent_jid: impl Into<Jid>,
    ) -> Result<CreateCommunityResult, CommunityError> {
        let options = GroupCreateOptions {
            subject: name.into(),
            participants: participants
                .iter()
                .cloned()
                .map(GroupParticipantOptions::new)
                .collect(),
            linked_parent: Some(parent_jid.into()),
            ..Default::default()
        };
        let metadata = self.client.groups().create_group(options).await?.metadata;
        Ok(CreateCommunityResult { metadata })
    }

    /// Deactivate (delete) a community. Subgroups are unlinked but not deleted.
    pub async fn deactivate(&self, community_jid: impl Into<Jid>) -> Result<(), CommunityError> {
        let community_jid = &community_jid.into();
        self.client
            .execute(DeleteCommunityIq::new(community_jid))
            .await?;
        Ok(())
    }

    /// Remove participants from the parent and all linked groups.
    pub async fn remove_participants(
        &self,
        community_jid: impl Into<Jid>,
        participants: &[Jid],
    ) -> Result<Vec<ParticipantChangeResponse>, CommunityError> {
        Ok(self
            .client
            .groups()
            .remove_participants_including_linked_groups(community_jid, participants)
            .await?)
    }

    /// Link existing groups as subgroups of a community.
    pub async fn link_subgroups(
        &self,
        community_jid: impl Into<Jid>,
        subgroup_jids: &[Jid],
    ) -> Result<LinkSubgroupsResult, CommunityError> {
        let community_jid = &community_jid.into();
        let response = self
            .client
            .execute(LinkSubgroupsIq::new(community_jid, subgroup_jids))
            .await?;

        let mut linked_jids = Vec::with_capacity(response.groups.len());
        let mut failed_groups = Vec::with_capacity(response.groups.len());

        for group in response.groups {
            if let Some(error) = group.error {
                failed_groups.push((group.jid, error));
            } else {
                linked_jids.push(group.jid);
            }
        }

        Ok(LinkSubgroupsResult {
            linked_jids,
            failed_groups,
        })
    }

    /// Unlink subgroups from a community.
    pub async fn unlink_subgroups(
        &self,
        community_jid: impl Into<Jid>,
        subgroup_jids: &[Jid],
        remove_orphan_members: bool,
    ) -> Result<UnlinkSubgroupsResult, CommunityError> {
        let community_jid = &community_jid.into();
        let response = self
            .client
            .execute(UnlinkSubgroupsIq::new(
                community_jid,
                subgroup_jids,
                remove_orphan_members,
            ))
            .await?;

        let mut unlinked_jids = Vec::with_capacity(response.groups.len());
        let mut failed_groups = Vec::with_capacity(response.groups.len());

        for group in response.groups {
            if let Some(error) = group.error {
                failed_groups.push((group.jid, error));
            } else {
                unlinked_jids.push(group.jid);
            }
        }

        Ok(UnlinkSubgroupsResult {
            unlinked_jids,
            failed_groups,
        })
    }

    /// Fetch all subgroups of a community via MEX (GraphQL).
    pub async fn get_subgroups(
        &self,
        community_jid: &Jid,
    ) -> Result<Vec<CommunitySubgroup>, CommunityError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(fetch_all_subgroups {
                group_id: Some(community_jid.to_string()),
                ..Default::default()
            }))
            .await?;

        let data = response.data.ok_or_else(|| {
            CommunityError::InvalidRequest("MEX response missing data field".into())
        })?;

        let group_query = &data["xwa2_group_query_by_id"];
        let mut subgroups = Vec::new();

        // Parse default subgroup
        if let Some(default_sub) = group_query.get("default_sub_group")
            && !default_sub.is_null()
            && let Some(sg) = parse_subgroup_node(default_sub, true)
        {
            subgroups.push(sg);
        }

        // Parse regular subgroups
        if let Some(sub_groups) = group_query.get("sub_groups")
            && let Some(edges) = sub_groups.get("edges").and_then(|e| e.as_array())
        {
            for edge in edges {
                if let Some(node) = edge.get("node")
                    && let Some(sg) = parse_subgroup_node(node, false)
                {
                    subgroups.push(sg);
                }
            }
        }

        Ok(subgroups)
    }

    /// Fetch all parent groups the account currently participates in.
    pub async fn get_participating(
        &self,
    ) -> Result<std::collections::HashMap<Jid, GroupMetadata>, CommunityError> {
        let response = self.client.execute(CommunityParticipatingIq::new()).await?;
        let mut result: std::collections::HashMap<Jid, GroupMetadata> = response
            .groups
            .into_iter()
            .map(|community| {
                let id = community.id.clone();
                (id, GroupMetadata::from(community))
            })
            .collect();

        for metadata in result.values_mut() {
            self.client.groups().fill_participant_pns(metadata).await;
        }

        Ok(result)
    }

    /// Fetch participant counts per subgroup via MEX (GraphQL).
    pub async fn get_subgroup_participant_counts(
        &self,
        community_jid: &Jid,
    ) -> Result<Vec<(Jid, u32)>, CommunityError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(query_subgroup_participant_count {
                input: Some(query_subgroup_participant_count::Input {
                    group_jid: Some(community_jid.to_string()),
                    ..Default::default()
                }),
            }))
            .await?;

        let data = response.data.ok_or_else(|| {
            CommunityError::InvalidRequest("MEX response missing data field".into())
        })?;

        let group_query = &data["xwa2_group_query_by_id"];
        let edges_ref = group_query
            .get("sub_groups")
            .and_then(|s| s.get("edges"))
            .and_then(|e| e.as_array());
        let mut counts = Vec::with_capacity(edges_ref.map_or(0, |e| e.len()));

        if let Some(edges) = edges_ref {
            for edge in edges {
                if let Some(node) = edge.get("node") {
                    let id_str = node["id"].as_str().unwrap_or_default();
                    let count = node
                        .get("total_participants_count")
                        .or_else(|| node.get("participants_count"))
                        .and_then(|c| c.as_u64())
                        .unwrap_or(0) as u32;
                    match id_str.parse::<Jid>() {
                        Ok(jid) => counts.push((jid, count)),
                        Err(_) => warn!(
                            "community: skipping subgroup with unparseable id: {:?}",
                            id_str
                        ),
                    }
                }
            }
        }

        Ok(counts)
    }

    /// Query a linked subgroup's metadata from the parent community.
    pub async fn query_linked_group(
        &self,
        community_jid: impl Into<Jid>,
        subgroup_jid: impl Into<Jid>,
    ) -> Result<GroupMetadata, CommunityError> {
        let community_jid = &community_jid.into();
        let subgroup_jid = &subgroup_jid.into();
        let response = self
            .client
            .execute(QueryLinkedGroupIq::new(community_jid, subgroup_jid))
            .await?;
        Ok(GroupMetadata::from(response))
    }

    /// Join a linked subgroup via the parent community.
    pub async fn join_subgroup(
        &self,
        community_jid: impl Into<Jid>,
        subgroup_jid: impl Into<Jid>,
    ) -> Result<GroupMetadata, CommunityError> {
        let community_jid = &community_jid.into();
        let subgroup_jid = &subgroup_jid.into();
        let response = self
            .client
            .execute(JoinLinkedGroupIq::new(community_jid, subgroup_jid))
            .await?;
        Ok(GroupMetadata::from(response))
    }

    /// Get all participants across all linked groups of a community.
    pub async fn get_linked_groups_participants(
        &self,
        community_jid: impl Into<Jid>,
    ) -> Result<Vec<GroupParticipant>, CommunityError> {
        let community_jid = &community_jid.into();
        let response = self
            .client
            .execute(GetLinkedGroupsParticipantsIq::new(community_jid))
            .await?;
        Ok(response.into_iter().map(Into::into).collect())
    }
}

fn json_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse::<u64>().ok())
}

fn json_jid(value: &serde_json::Value) -> Option<Jid> {
    if let Some(value) = value.as_str() {
        return value.parse().ok();
    }

    let object = value.as_object()?;
    ["id", "lid", "pn"]
        .into_iter()
        .filter_map(|field| object.get(field)?.as_str())
        .find_map(|value| value.parse().ok())
}

fn json_bool(value: &serde_json::Value) -> Option<bool> {
    value.as_bool().or_else(|| match value.as_str()? {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    })
}

fn parse_subgroup_node(node: &serde_json::Value, is_default: bool) -> Option<CommunitySubgroup> {
    let id_str = node.get("id")?.as_str()?;
    let jid: Jid = id_str.parse().ok()?;

    // Subject can be a plain string or an object {"value": "..."}
    let subject = node
        .get("subject")
        .and_then(|s| {
            s.as_str().map(|v| v.to_string()).or_else(|| {
                s.get("value")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
            })
        })
        .unwrap_or_default();

    let participant_count = node
        .get("participants_count")
        .or_else(|| node.get("total_participants_count"))
        .and_then(json_u64)
        .and_then(|count| u32::try_from(count).ok());

    let creation = node
        .get("creation")
        .or_else(|| node.get("creation_time"))
        .and_then(json_u64)
        .or_else(|| node.get("subject")?.get("creation_time").and_then(json_u64));
    let owner = node
        .get("creator")
        .or_else(|| node.get("owner"))
        .and_then(json_jid)
        .or_else(|| node.get("subject")?.get("creator").and_then(json_jid));

    // Check if properties indicate general chat
    let is_general_from_props = node
        .get("properties")
        .and_then(|p| p.get("general_chat"))
        .and_then(json_bool)
        .unwrap_or(false);

    Some(CommunitySubgroup {
        id: jid,
        subject,
        participant_count,
        creation,
        owner,
        is_default_sub_group: is_default,
        is_general_chat: is_general_from_props,
    })
}

impl Client {
    pub fn community(&self) -> Community<'_> {
        Community::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subgroup_parser_preserves_typed_metadata() {
        let node = serde_json::json!({
            "id": "120363000000000002@g.us",
            "subject": {
                "value": "Fictitious subgroup",
                "creation_time": "1700000012"
            },
            "creator": {
                "id": "100000000000002@lid",
                "pn": "15550000002@s.whatsapp.net"
            },
            "total_participants_count": 42,
            "properties": { "general_chat": "1" }
        });

        let subgroup = parse_subgroup_node(&node, false).expect("valid subgroup");
        assert_eq!(subgroup.subject, "Fictitious subgroup");
        assert_eq!(subgroup.creation, Some(1_700_000_012));
        assert_eq!(subgroup.participant_count, Some(42));
        assert_eq!(subgroup.owner, Some("100000000000002@lid".parse().unwrap()));
        assert!(subgroup.is_general_chat);
        assert!(!subgroup.is_default_sub_group);
    }

    #[test]
    fn subgroup_parser_accepts_legacy_scalar_metadata() {
        let node = serde_json::json!({
            "id": "120363000000000003@g.us",
            "subject": "Legacy subgroup",
            "creation": 1700000024,
            "owner": "15550000003@s.whatsapp.net",
            "properties": { "general_chat": false }
        });

        let subgroup = parse_subgroup_node(&node, true).expect("valid subgroup");
        assert_eq!(subgroup.creation, Some(1_700_000_024));
        assert_eq!(
            subgroup.owner,
            Some("15550000003@s.whatsapp.net".parse().unwrap())
        );
        assert!(!subgroup.is_general_chat);
        assert!(subgroup.is_default_sub_group);
    }
}
