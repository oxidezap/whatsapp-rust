//! Fixed wire fixtures from the previous schema, independent of regenerated output.
#![allow(clippy::disallowed_methods)] // Test the generated codec directly.

use buffa::Message as _;
use waproto::whatsapp as wa;

#[test]
fn retained_newsletter_field_decodes_as_a_known_field() {
    // Message.newsletterAdminProfileMessageV2 = 117, empty FutureProofMessage.
    let bytes = [0xaa, 0x07, 0x00];
    let message = wa::Message::decode_from_slice(&bytes).unwrap();
    assert!(message.newsletter_admin_profile_message_v2.is_set());
    assert_eq!(message.encode_to_vec(), bytes);
}

#[test]
fn locally_persisted_seed_keeps_field_number_100() {
    let bytes = [0xa2, 0x06, 0x03, 1, 2, 3];
    let key = wa::session_structure::chain::MessageKey::decode_from_slice(&bytes).unwrap();
    assert_eq!(key.seed.as_deref(), Some([1, 2, 3].as_slice()));
    assert_eq!(key.encode_to_vec(), bytes);
}

#[test]
fn lid_mapping_retention_survives_regeneration() {
    let bytes = [0x08, 42, 0x10, 123];
    let mapping = wa::LIDMigrationMapping::decode_from_slice(&bytes).unwrap();
    assert_eq!((mapping.pn, mapping.assigned_lid), (42, 123));
    assert_eq!(mapping.encode_to_vec(), bytes);
    let _ = wa::LIDMigrationMappingSyncPayload::default();
}

#[test]
fn absent_bundle_families_keep_their_public_types() {
    let _ = wa::LabyrinthWaCommand::default();
    let _ = wa::MinosCommand::default();
    let _ = wa::MandrakeMekBundle::default();
    let _ = wa::ConsumerApplication::default();
    let _ = wa::ExtendedContentMessage::default();
    let _ = wa::SubProtocol::default();
}

#[test]
fn message_clone_preserves_retained_and_new_fields_without_aliasing() {
    // Retained field 117 contains a nested conversation, and new field 132
    // contains an empty FutureProofMessage. Both must survive Clone unchanged.
    let bytes = [0xaa, 0x07, 5, 0x0a, 3, 0x0a, 1, b'x', 0xa2, 0x08, 0];
    let original = wa::Message::decode_from_slice(&bytes).unwrap();
    let mut cloned = original.clone();
    assert_eq!(cloned, original);
    assert_eq!(cloned.encode_to_vec(), bytes);
    assert!(cloned.newsletter_scheduled_message.is_set());
    cloned
        .newsletter_admin_profile_message_v2
        .as_option_mut()
        .unwrap()
        .message
        .as_option_mut()
        .unwrap()
        .conversation = Some("changed".to_owned());
    assert_eq!(
        original
            .newsletter_admin_profile_message_v2
            .message
            .conversation
            .as_deref(),
        Some("x")
    );
    cloned.clone_from(&original);
    assert_eq!(cloned, original);
}
