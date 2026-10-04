//! High-level output mocks preserve public field reading and extensible patterns.
//!
//! ```compile_fail,E0639
//! use wa::GroupMetadata;
//! let metadata = GroupMetadata {
//!     subject: Some("Fixture group".into()),
//!     ..GroupMetadata::new("120363000000000001@g.us".parse().unwrap())
//! };
//! ```
//!
//! ```compile_fail,E0639
//! use wa::{GroupParticipant, Jid};
//! use wa::wacore::iq::groups::ParticipantType;
//! let participant = GroupParticipant {
//!     jid: Jid::pn("15550000001"), phone_number: None, lid: None,
//!     username: None, participant_type: ParticipantType::Member, details: None,
//! };
//! ```
//!
//! ```compile_fail,E0638
//! use wa::{GroupParticipant, Jid};
//! let GroupParticipant { jid, phone_number, lid, username, participant_type, details } =
//!     GroupParticipant::new(Jid::pn("15550000001"));
//! ```
//!
//! ```compile_fail,E0638
//! use wa::GroupMetadata;
//! let GroupMetadata { id } =
//!     GroupMetadata::new("120363000000000001@g.us".parse().unwrap());
//! ```
//!
//! ```compile_fail,E0599
//! use wa::GroupMetadata;
//! let metadata = GroupMetadata::default();
//! ```

use wa::{GroupMetadata, GroupParticipant, Jid};

/// A host can synthesize full metadata without mirroring every protocol field.
pub fn metadata(id: Jid, member: Jid, parent: Jid) -> GroupMetadata {
    let mut participant = GroupParticipant::new(member);
    participant.participant_type = wa::wacore::iq::groups::ParticipantType::Admin;
    let mut metadata = GroupMetadata::new(id);
    metadata.subject = Some("Fixture subgroup".into());
    metadata.participants.push(participant);
    metadata.parent_group_jid = Some(parent);
    metadata.is_general_chat = true;
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;
    use wa::{GroupHierarchy, GroupOverview, SubgroupKind};

    #[test]
    fn minimal_mocks_keep_optional_observations_absent() {
        let metadata = GroupMetadata::new("120363000000000001@g.us".parse().unwrap());
        assert!(metadata.subject.is_none());
        assert!(metadata.participant_count.is_none());
        assert!(metadata.creation_time.is_none());
        assert!(metadata.ephemeral.is_none());
        assert!(metadata.member_add_mode.is_none());
        assert!(metadata.participants.is_empty());
        assert_eq!(metadata.hierarchy(), GroupHierarchy::Standalone);
        let participant = GroupParticipant::new(Jid::pn("15550000001"));
        assert!(!participant.is_admin());
        assert!(!participant.is_super_admin());
        let GroupParticipant {
            jid,
            phone_number,
            lid,
            username,
            details,
            ..
        } = participant;
        assert_eq!(jid, Jid::pn("15550000001"));
        assert!(phone_number.is_none());
        assert!(lid.is_none());
        assert!(username.is_none());
        assert!(details.is_none());
    }

    #[test]
    fn mock_customization_and_projection_keep_the_normalized_hierarchy() {
        let id: Jid = "120363000000000001@g.us".parse().unwrap();
        let parent: Jid = "120363000000000002@g.us".parse().unwrap();
        let mut metadata = metadata(id.clone(), Jid::pn("15550000001"), parent.clone());
        assert!(metadata.participants[0].is_admin());
        assert!(!metadata.participants[0].is_super_admin());
        // The linked parent still takes precedence over a bare community flag.
        metadata.is_parent_group = true;
        let expected = GroupHierarchy::Subgroup {
            parent,
            kind: SubgroupKind::General,
        };
        assert_eq!(metadata.hierarchy(), expected);
        let overview = GroupOverview::from(&metadata);
        assert_eq!(overview.hierarchy, expected);
        assert_eq!(overview.participant_count, None);
        let GroupMetadata {
            id: actual,
            subject,
            participants,
            ..
        } = metadata;
        assert_eq!(actual, id);
        assert_eq!(subject.as_deref(), Some("Fixture subgroup"));
        assert_eq!(participants.len(), 1);
    }
}
