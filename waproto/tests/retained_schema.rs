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

#[test]
fn wallpaper_string_and_argb_encodings_keep_their_distinct_wire_types() {
    use buffa::{MessageView as _, ViewEncode as _};
    use wa::message::{
        ChatSolidColorWallpaper, ChatSolidColorWallpaperOwnedView, ChatSolidColorWallpaperView,
    };
    // Historical strings use length-delimited fields 1 and 2. The new source
    // uses fixed32 at the same numbers, without converting old string values.
    let legacy = [0x0a, 3, b'r', b'e', b'd', 0x12, 4, b'b', b'l', b'u', b'e'];
    let argb = [0x0d, 0x33, 0x22, 0x11, 0xff, 0x15, 0x66, 0x55, 0x44, 0xff];
    for bytes in [&legacy[..], &argb[..]] {
        let owned = ChatSolidColorWallpaper::decode_from_slice(bytes).unwrap();
        let view = ChatSolidColorWallpaperView::decode_view(bytes).unwrap();
        let owned_view =
            ChatSolidColorWallpaperOwnedView::decode(bytes::Bytes::copy_from_slice(bytes)).unwrap();
        assert_eq!(owned.encode_to_vec(), bytes);
        assert_eq!(view.encode_to_vec(), bytes);
        assert_eq!(view.to_owned_message().unwrap(), owned);
        if bytes == legacy {
            assert_eq!(owned.color_light.as_deref(), Some("red"));
            assert_eq!(owned_view.color_dark(), Some("blue"));
            assert_eq!(owned.background_light_argb, None);
        } else {
            assert_eq!(owned.background_light_argb, Some(0xff112233));
            assert_eq!(owned_view.background_dark_argb(), Some(0xff445566));
            assert_eq!(owned.color_light, None);
        }
    }
    // Neither decoder may accept unrelated wire types for these known fields.
    assert!(ChatSolidColorWallpaper::decode_from_slice(&[0x08, 1]).is_err());
    assert!(ChatSolidColorWallpaperView::decode_view(&[0x08, 1]).is_err());
    assert!(ChatSolidColorWallpaper::decode_from_slice(&[0x0d, 1, 2]).is_err());
    assert!(ChatSolidColorWallpaperView::decode_view(&[0x0d, 1, 2]).is_err());
}
