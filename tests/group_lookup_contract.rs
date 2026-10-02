//! Downstream use of both group projections through one lookup contract.

use whatsapp_rust::{
    Client, GroupError, GroupHierarchy, GroupLookupResult, GroupMetadata, GroupMetadataResult,
    GroupOverview, GroupOverviewResult, Jid, SubgroupKind,
};

// A group-list UI can share its partial/refusal handling without knowing which
// projection the caller fetched. The application supplies its found renderer.
fn row<T>(result: &GroupLookupResult<T>, found: impl FnOnce(&T) -> String) -> String {
    match result {
        GroupLookupResult::Found(value) => found(value),
        GroupLookupResult::Truncated {
            id,
            participant_count,
        } => format!("{id}: {participant_count} members (details unavailable)"),
        GroupLookupResult::Forbidden(id) => format!("{id}: forbidden"),
        GroupLookupResult::NotFound(id) => format!("{id}: not found"),
        _ => "unrecognized group outcome".to_owned(),
    }
}

async fn fetch_rows(client: &Client, jids: &[Jid]) -> Result<Vec<String>, GroupError> {
    let overview_rows = client
        .groups()
        .fetch_overviews(jids)
        .await?
        .iter()
        .map(|result| row(result, |overview| format!("{:?}", overview.hierarchy)))
        .collect::<Vec<_>>();
    let metadata_rows = client
        .groups()
        .fetch_metadata_batch(jids)
        .await?
        .iter()
        .map(|result| row(result, |metadata| format!("{:?}", metadata.hierarchy())))
        .collect::<Vec<_>>();
    Ok(overview_rows.into_iter().chain(metadata_rows).collect())
}

#[test]
fn both_endpoints_accept_the_same_status_renderer() {
    // Type-check real endpoints without a connection or hidden test features.
    let _ = fetch_rows;
    let id: Jid = "120363000000000021@g.us".parse().unwrap();
    let metadata = GroupMetadata {
        id,
        participant_count: Some(42),
        ..Default::default()
    };
    let full: GroupMetadataResult = GroupLookupResult::Found(Box::new(metadata));
    let GroupLookupResult::Found(metadata) = &full else {
        panic!("expected found metadata")
    };
    let overview: GroupOverviewResult =
        GroupLookupResult::Found(GroupOverview::from(metadata.as_ref()));
    assert_eq!(
        row(&full, |metadata| format!("{:?}", metadata.hierarchy())),
        row(&overview, |overview| format!("{:?}", overview.hierarchy))
    );
    let GroupLookupResult::Found(overview) = overview else {
        panic!("expected found projection")
    };
    assert_eq!(overview.participant_count, Some(42));
    assert_eq!(overview.subject, None);
}

#[test]
fn projection_mapping_never_invents_a_found_payload() {
    let id: Jid = "120363000000000022@g.us".parse().unwrap();
    for full in [
        GroupMetadataResult::Truncated {
            id: id.clone(),
            participant_count: 900,
        },
        GroupMetadataResult::Forbidden(id.clone()),
        GroupMetadataResult::NotFound(id.clone()),
    ] {
        let before = row(&full, |_| panic!("no metadata available"));
        let overview: GroupOverviewResult = full.map(|_| panic!("no metadata available"));
        assert_eq!(before, row(&overview, |_| panic!("no overview available")));
    }
}

#[test]
fn missing_count_does_not_become_zero_or_participant_length() {
    let metadata = GroupMetadata::default();
    assert_eq!(metadata.participant_count, None);
    assert_eq!(GroupOverview::from(&metadata).participant_count, None);
}

#[test]
fn hierarchy_preserves_all_roles_and_independent_flag_precedence() {
    let parent: Jid = "120363000000000011@g.us".parse().unwrap();
    for linked in [false, true] {
        for community in [false, true] {
            for announcement in [false, true] {
                for general in [false, true] {
                    let metadata = GroupMetadata {
                        parent_group_jid: linked.then(|| parent.clone()),
                        is_parent_group: community,
                        is_default_sub_group: announcement,
                        is_general_chat: general,
                        ..Default::default()
                    };
                    let expected = if linked {
                        GroupHierarchy::Subgroup {
                            parent: parent.clone(),
                            kind: if announcement {
                                SubgroupKind::Announcement
                            } else if general {
                                SubgroupKind::General
                            } else {
                                SubgroupKind::Regular
                            },
                        }
                    } else if community {
                        GroupHierarchy::Community
                    } else {
                        GroupHierarchy::Standalone
                    };
                    assert_eq!(metadata.hierarchy(), expected);
                    let overview = GroupOverview::from(&metadata);
                    assert_eq!(overview.hierarchy, expected);
                    assert_eq!(overview.parent_group_jid(), linked.then_some(&parent));
                    assert_eq!(overview.subject, None);
                    assert_eq!(overview.participant_count, None);
                    // Classification must not erase the independent raw flags.
                    assert_eq!(metadata.is_parent_group, community);
                    assert_eq!(metadata.is_default_sub_group, announcement);
                    assert_eq!(metadata.is_general_chat, general);
                }
            }
        }
    }
}

#[test]
fn hierarchy_retains_the_parent_for_a_general_chat() {
    let parent: Jid = "120363000000000011@g.us".parse().unwrap();
    let metadata = GroupMetadata {
        parent_group_jid: Some(parent.clone()),
        is_general_chat: true,
        ..Default::default()
    };
    assert!(
        matches!(metadata.hierarchy(), GroupHierarchy::Subgroup { parent: actual, kind: SubgroupKind::General } if actual == parent)
    );
}
