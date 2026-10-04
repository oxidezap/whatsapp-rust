//! Independent input consumer. Default mode also tests high-level mock outputs.
//!
//! Old literals are rejected even when using a valid constructor as a base.
//! These controls use the same accessible imports as the positive tests.
//!
//! ```compile_fail,E0639
//! use wacore::iq::groups::GroupCreateOptions;
//! let options = GroupCreateOptions {
//!     subject: "Fixture group".into(),
//!     ..GroupCreateOptions::new("Fixture group")
//! };
//! ```
//!
//! ```compile_fail,E0639
//! use wacore::iq::groups::GroupParticipantOptions;
//! use wacore_binary::jid::Jid;
//! let participant = GroupParticipantOptions {
//!     jid: Jid::pn("15550000001"), phone_number: None, privacy: None,
//! };
//! ```
//!
//! ```compile_fail,E0638
//! use wacore::iq::groups::GroupParticipantOptions;
//! use wacore_binary::jid::Jid;
//! let GroupParticipantOptions { jid, phone_number, privacy } =
//!     GroupParticipantOptions::new(Jid::pn("15550000001"));
//! ```
//!
//! ```compile_fail,E0638
//! use wacore::iq::groups::GroupCreateOptions;
//! let GroupCreateOptions {
//!     subject, participants, member_link_mode, member_add_mode,
//!     membership_approval_mode, ephemeral_expiration, is_parent, closed,
//!     allow_non_admin_sub_group_creation, create_general_chat,
//!     linked_parent, hidden_group, description,
//! } = GroupCreateOptions::new("Fixture group");
//! ```
//!
//! ```compile_fail,E0599
//! use wacore::iq::groups::GroupCreateOptions;
//! let options = GroupCreateOptions::default();
//! ```
//!
//! ```compile_fail
//! use wacore::iq::groups::GroupCreateOptions;
//! let options: GroupCreateOptions = GroupCreateOptions::builder().build();
//! ```
//!
//! ```compile_fail
//! use wacore::iq::groups::GroupParticipantOptions;
//! let participant: GroupParticipantOptions = GroupParticipantOptions::builder().build();
//! ```

use wacore::iq::groups::{GroupCreateOptions, GroupParticipantOptions};
use wacore_binary::jid::Jid;

/// Constructors and builders are available without the Tokio runtime crate.
pub fn inputs(jid: Jid) -> (GroupCreateOptions, GroupParticipantOptions) {
    let participant: GroupParticipantOptions = GroupParticipantOptions::builder()
        .jid(jid)
        .privacy(vec![1, 2, 3])
        .build();
    let options: GroupCreateOptions = GroupCreateOptions::builder()
        .subject("Fixture group")
        .participants(vec![participant.clone()])
        .build();
    (options, participant)
}

#[cfg(feature = "runtime")]
pub mod mocks;

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::iq::groups::{
        MemberAddMode, MemberLinkMode, MembershipApprovalMode, build_create_group_node,
    };

    #[test]
    fn constructors_and_builders_keep_the_same_creation_defaults() {
        let jid = Jid::pn("15550000001");
        let built: GroupCreateOptions = GroupCreateOptions::builder()
            .subject("Fixture group")
            .build();
        let new = GroupCreateOptions::new("Fixture group");
        assert_eq!(
            build_create_group_node(&new),
            build_create_group_node(&built)
        );
        assert_eq!(new.member_link_mode, Some(MemberLinkMode::AdminLink));
        assert_eq!(new.member_add_mode, Some(MemberAddMode::AllMemberAdd));
        assert_eq!(
            new.membership_approval_mode,
            Some(MembershipApprovalMode::Off)
        );
        assert_eq!(new.ephemeral_expiration, Some(0));
        assert!(new.participants.is_empty());
        assert!(!new.is_parent);
        assert!(!new.closed);
        assert!(!new.allow_non_admin_sub_group_creation);
        assert!(!new.create_general_chat);
        assert!(!new.hidden_group);
        assert!(new.linked_parent.is_none());
        assert!(new.description.is_none());

        let (options, participant) = inputs(jid.clone());
        let GroupCreateOptions {
            subject,
            participants,
            ..
        } = options;
        assert_eq!(subject, "Fixture group");
        assert_eq!(participants.len(), 1);
        let GroupParticipantOptions {
            jid: actual,
            privacy,
            ..
        } = participant;
        assert_eq!(actual, jid);
        assert_eq!(privacy, Some(vec![1, 2, 3]));
    }

    #[test]
    fn fluent_construction_public_mutation_and_generic_builder_conversion() {
        struct HostParticipant(GroupParticipantOptions);
        impl From<GroupParticipantOptions> for HostParticipant {
            fn from(value: GroupParticipantOptions) -> Self {
                Self(value)
            }
        }
        struct HostCreate(GroupCreateOptions);
        impl From<GroupCreateOptions> for HostCreate {
            fn from(value: GroupCreateOptions) -> Self {
                Self(value)
            }
        }
        let host: HostParticipant = GroupParticipantOptions::builder()
            .jid(Jid::pn("15550000001"))
            .build();
        let host: HostCreate = GroupCreateOptions::builder()
            .subject("Fixture group")
            .participants(vec![host.0])
            .is_parent(true)
            .build();
        assert!(host.0.is_parent);

        let member = GroupParticipantOptions::new(Jid::pn("15550000002")).with_privacy(vec![4, 5]);
        let mut options = GroupCreateOptions::new("Fixture group").with_participant(member);
        // Explicit absence remains possible for advanced hosts.
        options.member_link_mode = None;
        options.member_add_mode = None;
        options.membership_approval_mode = None;
        options.ephemeral_expiration = None;
        assert_eq!(options.participants.len(), 1);
        assert!(options.member_link_mode.is_none());
    }
}
