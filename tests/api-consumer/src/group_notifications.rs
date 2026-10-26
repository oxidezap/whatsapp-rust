//! Received group types evolve independently of their supported constructors.
use wacore::stanza::groups::{GroupNotification, GroupNotificationAction, GroupParticipantInfo};

pub fn fixture() -> GroupNotification {
    let participant = GroupParticipantInfo::builder()
        .jid("100000001@s.whatsapp.net".parse().unwrap())
        .build();
    GroupNotification::builder()
        .group_jid("100000002@g.us".parse().unwrap())
        .timestamp(42)
        .is_lid_addressing_mode(false)
        .has_incomplete_participant_information(false)
        .actions(vec![
            GroupNotificationAction::add()
                .participants(vec![participant])
                .build(),
            GroupNotificationAction::subject()
                .subject("Synthetic group".into())
                .build(),
        ])
        .build()
}

pub fn recognized_items(notification: &GroupNotification) -> usize {
    let GroupNotification { actions, .. } = notification;
    actions
        .iter()
        .map(|action| match action {
            GroupNotificationAction::Add { participants, .. } => participants.len(),
            GroupNotificationAction::Subject { subject, .. } => usize::from(!subject.is_empty()),
            _ => 0,
        })
        .sum()
}

#[cfg(test)]
#[test]
fn public_builders_and_forward_compatible_patterns() {
    let notification = fixture();
    assert_eq!(recognized_items(&notification), 2);
    let action = &notification.actions[0];
    assert_eq!(action.wire_tag(), "add");
    let json = serde_json::to_value(action).unwrap();
    assert_eq!(json["type"], "add");
    assert_eq!(json["participants"][0]["jid"]["user"], "100000001");
    assert!(json.get("reason").is_none());
    assert!(json["participants"][0].get("phone_number").is_none());
}

#[cfg(feature = "group-notification-literal")]
fn forbidden_notification_literal(value: GroupNotification) -> GroupNotification {
    GroupNotification { ..value }
}

#[cfg(feature = "group-participant-literal")]
fn forbidden_participant_literal(value: GroupParticipantInfo) -> GroupParticipantInfo {
    GroupParticipantInfo { ..value }
}

#[cfg(feature = "group-action-literal")]
fn forbidden_action_literal() -> GroupNotificationAction {
    GroupNotificationAction::Add {
        participants: vec![],
        reason: None,
    }
}

#[cfg(feature = "group-action-exact-pattern")]
fn forbidden_exact_pattern(action: GroupNotificationAction) {
    if let GroupNotificationAction::Add {
        participants,
        reason,
    } = action
    {
        let _ = (participants, reason);
    }
}

#[cfg(feature = "group-action-exhaustive")]
fn forbidden_exhaustive(action: GroupNotificationAction) {
    // Even a wire fallback variant cannot cover future typed Rust variants.
    match action {
        GroupNotificationAction::Add { .. } => (),
        GroupNotificationAction::Remove { .. } => (),
        GroupNotificationAction::Promote { .. } => (),
        GroupNotificationAction::Demote { .. } => (),
        GroupNotificationAction::Modify { .. } => (),
        GroupNotificationAction::Subject { .. } => (),
        GroupNotificationAction::Description { .. } => (),
        GroupNotificationAction::Locked { .. } => (),
        GroupNotificationAction::Unlocked => (),
        GroupNotificationAction::Announce => (),
        GroupNotificationAction::NotAnnounce => (),
        GroupNotificationAction::Ephemeral { .. } => (),
        GroupNotificationAction::MembershipApprovalMode { .. } => (),
        GroupNotificationAction::MembershipApprovalRequest { .. } => (),
        GroupNotificationAction::CreatedMembershipRequests { .. } => (),
        GroupNotificationAction::RevokedMembershipRequests { .. } => (),
        GroupNotificationAction::MemberAddMode { .. } => (),
        GroupNotificationAction::NoFrequentlyForwarded => (),
        GroupNotificationAction::FrequentlyForwardedOk => (),
        GroupNotificationAction::Invite { .. } => (),
        GroupNotificationAction::RevokeInvite => (),
        GroupNotificationAction::GrowthLocked { .. } => (),
        GroupNotificationAction::GrowthUnlocked => (),
        GroupNotificationAction::Create { .. } => (),
        GroupNotificationAction::Delete { .. } => (),
        GroupNotificationAction::Link { .. } => (),
        GroupNotificationAction::Unlink { .. } => (),
        GroupNotificationAction::LinkedGroupPromote { .. } => (),
        GroupNotificationAction::LinkedGroupDemote { .. } => (),
        GroupNotificationAction::Suspended => (),
        GroupNotificationAction::Unsuspended => (),
        GroupNotificationAction::AutoAddDisabled => (),
        GroupNotificationAction::IsCapiHostedGroup => (),
        GroupNotificationAction::GroupSafetyCheck => (),
        GroupNotificationAction::LimitSharingEnabled { .. } => (),
        GroupNotificationAction::AllowAdminReports => (),
        GroupNotificationAction::NotAllowAdminReports => (),
        GroupNotificationAction::Reports => (),
        GroupNotificationAction::AllowNonAdminSubGroupCreation => (),
        GroupNotificationAction::NotAllowNonAdminSubGroupCreation => (),
        GroupNotificationAction::CreatedSubGroupSuggestion { .. } => (),
        GroupNotificationAction::RevokedSubGroupSuggestions { .. } => (),
        GroupNotificationAction::ChangeNumber { .. } => (),
        GroupNotificationAction::Unknown { .. } => (),
    }
}
