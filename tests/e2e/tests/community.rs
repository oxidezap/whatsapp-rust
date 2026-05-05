use e2e_tests::TestClient;
use log::info;
use whatsapp_rust::GroupType;
use whatsapp_rust::features::{CreateCommunityOptions, GroupCreateOptions, group_type};

#[tokio::test]
async fn test_community_create() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_create").await?;

    let result = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Test Community"))
        .await?;

    assert!(
        result.metadata.id.server == "g.us",
        "community JID should be a group: {}",
        result.metadata.id
    );

    info!("Created community: {}", result.metadata.id);

    // The create response itself should classify as a community without
    // a follow-up metadata query — overlay in `GroupCreateIq::parse_response`
    // restores `is_parent_group` even if the server omits `<parent>`.
    assert!(
        result.metadata.is_parent_group,
        "create result should classify as a parent group"
    );
    assert_eq!(group_type(&result.metadata), GroupType::Community);

    // Cross-check against a fresh `get_metadata` query.
    let metadata = client
        .client
        .groups()
        .get_metadata(&result.metadata.id)
        .await?;
    assert!(metadata.is_parent_group, "should be a parent group");
    assert_eq!(group_type(&metadata), GroupType::Community);

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_create_with_general_chat() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_general").await?;

    let result = client
        .client
        .community()
        .create(CreateCommunityOptions {
            name: "Community With General".to_string(),
            description: None,
            closed: false,
            allow_non_admin_sub_group_creation: false,
            create_general_chat: true,
        })
        .await?;

    info!("Created community: {}", result.metadata.id);

    // Fetch subgroups — should have at least a default announcement subgroup
    let subgroups = client
        .client
        .community()
        .get_subgroups(&result.metadata.id)
        .await?;

    assert!(
        !subgroups.is_empty(),
        "community should have at least one subgroup"
    );

    info!(
        "Subgroups: {:?}",
        subgroups
            .iter()
            .map(|s| format!(
                "{} (default={}, general={})",
                s.id, s.is_default_sub_group, s.is_general_chat
            ))
            .collect::<Vec<_>>()
    );

    // Verify default subgroup exists
    assert!(
        subgroups.iter().any(|s| s.is_default_sub_group),
        "should have a default announcement subgroup"
    );

    // Verify general chat subgroup was created (create_general_chat: true)
    assert!(
        subgroups.iter().any(|s| s.is_general_chat),
        "should have a general chat subgroup when create_general_chat is true, got: {:?}",
        subgroups
            .iter()
            .map(|s| format!(
                "{} (default={}, general={})",
                s.id, s.is_default_sub_group, s.is_general_chat
            ))
            .collect::<Vec<_>>()
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_get_subgroups() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_subgroups").await?;

    let result = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Subgroups Test"))
        .await?;

    let subgroups = client
        .client
        .community()
        .get_subgroups(&result.metadata.id)
        .await?;

    // A newly created community should have an auto-created default subgroup
    assert!(
        subgroups.iter().any(|s| s.is_default_sub_group),
        "should contain the default announcement subgroup"
    );

    info!(
        "Community {} has {} subgroup(s)",
        result.metadata.id,
        subgroups.len()
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_link_subgroup() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_link").await?;

    // Create a community
    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Link Test Community"))
        .await?;

    info!("Created community: {}", community.metadata.id);

    // Create a regular group to link as a subgroup
    let group = client
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Sub Group"))
        .await?;

    info!("Created group: {}", group.metadata.id);

    // Link the group to the community
    let link_result = client
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    assert!(
        link_result.failed_groups.is_empty(),
        "no groups should fail linking: {:?}",
        link_result.failed_groups
    );
    assert!(
        link_result.linked_jids.contains(&group.metadata.id),
        "linked JIDs should contain the subgroup"
    );

    info!("Linked subgroup {} to community", group.metadata.id);

    // Verify the subgroup appears in the community's subgroup list
    let subgroups = client
        .client
        .community()
        .get_subgroups(&community.metadata.id)
        .await?;

    assert!(
        subgroups.iter().any(|s| s.id == group.metadata.id),
        "linked group should appear in subgroup list"
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_unlink_subgroup() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_unlink").await?;

    // Create community + link a subgroup
    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Unlink Test"))
        .await?;

    let group = client
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Unlink Sub"))
        .await?;

    client
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    info!(
        "Linked subgroup {} to community {}",
        group.metadata.id, community.metadata.id
    );

    // Unlink the subgroup
    let unlink_result = client
        .client
        .community()
        .unlink_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
            false,
        )
        .await?;

    assert!(
        unlink_result.failed_groups.is_empty(),
        "no groups should fail unlinking"
    );
    assert!(
        unlink_result.unlinked_jids.contains(&group.metadata.id),
        "unlinked JIDs should contain the subgroup"
    );

    info!("Unlinked subgroup {}", group.metadata.id);

    // Verify it's gone from the subgroup list
    let subgroups = client
        .client
        .community()
        .get_subgroups(&community.metadata.id)
        .await?;

    assert!(
        !subgroups.iter().any(|s| s.id == group.metadata.id),
        "unlinked group should not appear in subgroup list"
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_deactivate() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_deactivate").await?;

    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Deactivate Test"))
        .await?;

    info!("Created community: {}", community.metadata.id);

    // Deactivate the community
    client
        .client
        .community()
        .deactivate(&community.metadata.id)
        .await?;

    info!("Deactivated community: {}", community.metadata.id);

    // Querying the deactivated community should fail or return non-parent
    let metadata_result = client
        .client
        .groups()
        .get_metadata(&community.metadata.id)
        .await;
    match metadata_result {
        Ok(metadata) => {
            // If the server still returns the group, it should no longer be a parent
            assert!(
                !metadata.is_parent_group,
                "deactivated community should not be a parent group"
            );
        }
        Err(_) => {
            // Expected: the community was deleted
            info!("Community no longer queryable (deleted)");
        }
    }

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_query_linked_group() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_query_linked").await?;

    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Query Linked Test"))
        .await?;

    let group = client
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Queryable Sub"))
        .await?;

    client
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    // Query the linked group's metadata from the community
    let metadata = client
        .client
        .community()
        .query_linked_group(&community.metadata.id, &group.metadata.id)
        .await?;

    assert_eq!(metadata.id, group.metadata.id, "metadata JID should match");
    assert_eq!(metadata.subject, "Queryable Sub");

    info!(
        "Queried linked group: {} (subject={})",
        metadata.id, metadata.subject
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_join_subgroup() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_community_join_a").await?;
    let client_b = TestClient::connect("e2e_community_join_b").await?;

    let jid_b_pn = client_b
        .client
        .get_pn()
        .await
        .expect("Client B should have a PN JID")
        .to_non_ad();
    let jid_b_lid = client_b
        .client
        .get_lid()
        .await
        .expect("Client B should have a LID JID")
        .to_non_ad();

    // Client A creates a community and links a subgroup
    let community = client_a
        .client
        .community()
        .create(CreateCommunityOptions::new("Join Test Community"))
        .await?;

    let group = client_a
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Joinable Sub"))
        .await?;

    client_a
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    // Add Client B to the community parent group first
    client_a
        .client
        .groups()
        .add_participants(&community.metadata.id, std::slice::from_ref(&jid_b_lid))
        .await?;

    info!("Added client B to community, now joining subgroup");

    // Client B joins the subgroup via the community
    let metadata = client_b
        .client
        .community()
        .join_subgroup(&community.metadata.id, &group.metadata.id)
        .await?;

    assert_eq!(metadata.id, group.metadata.id);

    // Verify client B is in the subgroup's participant list (may be LID or PN)
    let b_in_subgroup = metadata.participants.iter().any(|p| {
        p.jid == jid_b_pn || p.jid == jid_b_lid || p.phone_number.as_ref() == Some(&jid_b_pn)
    });
    assert!(
        b_in_subgroup,
        "client B ({} / {}) should be in the subgroup after joining, got: {:?}",
        jid_b_pn,
        jid_b_lid,
        metadata
            .participants
            .iter()
            .map(|p| &p.jid)
            .collect::<Vec<_>>()
    );

    info!("Client B joined subgroup {}", group.metadata.id);

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_get_linked_groups_participants() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_participants").await?;

    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Participants Test"))
        .await?;

    // Link a subgroup so linked_groups_participants has something to return
    let group = client
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Participants Sub"))
        .await?;

    client
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    // Get all participants across linked groups
    let participants = client
        .client
        .community()
        .get_linked_groups_participants(&community.metadata.id)
        .await?;

    let own_pn = client
        .client
        .get_pn()
        .await
        .expect("should have PN JID")
        .to_non_ad();
    let own_lid = client
        .client
        .get_lid()
        .await
        .expect("should have LID JID")
        .to_non_ad();

    info!(
        "Got {} participant(s) across linked groups",
        participants.len()
    );

    assert!(
        !participants.is_empty(),
        "should return at least the creator as a participant across linked groups"
    );

    let creator_found = participants
        .iter()
        .any(|p| p.jid == own_pn || p.jid == own_lid || p.phone_number.as_ref() == Some(&own_pn));
    assert!(
        creator_found,
        "creator ({} / {}) should be in linked groups participants, got: {:?}",
        own_pn,
        own_lid,
        participants.iter().map(|p| &p.jid).collect::<Vec<_>>()
    );

    client.disconnect().await;
    Ok(())
}

#[tokio::test]
async fn test_community_subgroup_participant_counts() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_community_counts").await?;

    let community = client
        .client
        .community()
        .create(CreateCommunityOptions::new("Counts Test"))
        .await?;

    // Link a subgroup
    let group = client
        .client
        .groups()
        .create_group(GroupCreateOptions::new("Count Sub"))
        .await?;

    client
        .client
        .community()
        .link_subgroups(
            &community.metadata.id,
            std::slice::from_ref(&group.metadata.id),
        )
        .await?;

    // Fetch participant counts
    let counts = client
        .client
        .community()
        .get_subgroup_participant_counts(&community.metadata.id)
        .await?;

    info!("Subgroup participant counts: {:?}", counts);

    // The linked subgroup should appear with a count >= 1 (at least the creator)
    let subgroup_count = counts
        .iter()
        .find(|(jid, _)| *jid == group.metadata.id)
        .map(|(_, count)| *count);

    assert!(
        subgroup_count.is_some(),
        "linked subgroup should appear in participant counts, got: {:?}",
        counts
    );
    assert!(
        subgroup_count.unwrap() >= 1,
        "subgroup participant count should be >= 1, got: {}",
        subgroup_count.unwrap()
    );

    client.disconnect().await;
    Ok(())
}
