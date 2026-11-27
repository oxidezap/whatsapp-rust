//! Fixed wire fixtures from the previous schema, independent of regenerated output.
#![allow(clippy::disallowed_methods)] // Test the generated codec directly.

use waproto::protobuf::Message as _;
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
fn lid_mapping_payload_preserves_shared_limits_and_late_error_order() {
    use waproto::protobuf::{
        DecodeError, DecodeOptions,
        encoding::{put_bytes_field, put_varint},
    };

    let mut wire = Vec::new();
    put_bytes_field(1, &[0x08, 42, 0x10, 123], &mut wire);
    put_varint(100 << 3, &mut wire);
    put_varint(1, &mut wire);
    wire.extend_from_slice(&[0x0a, 0x80]);
    assert_eq!(
        wa::LIDMigrationMappingSyncPayload::decode_from_slice(&wire).unwrap_err(),
        DecodeError::UnexpectedEof
    );
    assert_eq!(
        DecodeOptions::new()
            .with_unknown_field_limit(0)
            .decode_from_slice::<wa::LIDMigrationMappingSyncPayload>(&wire)
            .unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
    assert_eq!(
        DecodeOptions::new()
            .with_element_memory_limit(size_of::<wa::LIDMigrationMapping>())
            .decode_from_slice::<wa::LIDMigrationMappingSyncPayload>(&wire)
            .unwrap_err(),
        DecodeError::ElementMemoryLimitExceeded
    );

    let mut nested = vec![0x08, 42, 0x10, 123];
    put_varint((100 << 3) | 3, &mut nested);
    put_varint(1 << 3, &mut nested);
    put_varint(7, &mut nested);
    put_varint((100 << 3) | 4, &mut nested);
    let mut grouped = Vec::new();
    put_bytes_field(1, &nested, &mut grouped);
    for limit in [0, 1, 2] {
        let decoded = DecodeOptions::new()
            .with_unknown_field_limit(limit)
            .decode_from_slice::<wa::LIDMigrationMappingSyncPayload>(&grouped);
        assert_eq!(decoded.is_ok(), limit == 2);
    }
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
fn reclaimed_labyrinth_inputs_keep_their_distinct_wire_contracts() {
    use waproto::protobuf::{MessageView as _, ViewEncode as _};
    let historical = [0x0a, 1, 1, 0x12, 1, 2, 0x1a, 1, b'x', 0x20, 1, 0x2a, 1, 3];
    let current = [0x0a, 1, 1, 0x10, 2, 0x1a, 1, b'x', 0x20, 1, 0x2a, 1, 3];
    let old = wa::DecryptMessageInput::decode_from_slice(&historical).unwrap();
    let new = wa::WebDecryptMessageInput::decode_from_slice(&current).unwrap();
    assert_eq!(old.epoch_anon_id, [2]);
    assert_eq!(new.epoch_anon_id, 2);
    assert_eq!(old.encode_to_vec(), historical);
    assert_eq!(new.encode_to_vec(), current);
    assert!(wa::DecryptMessageInput::decode_from_slice(&current).is_err());
    assert!(wa::WebDecryptMessageInput::decode_from_slice(&historical).is_err());
    let old_view = wa::DecryptMessageInputView::decode_view(&historical).unwrap();
    let new_view = wa::WebDecryptMessageInputView::decode_view(&current).unwrap();
    assert_eq!(old_view.epoch_anon_id, [2]);
    assert_eq!(new_view.epoch_anon_id, 2);
    assert_eq!(old_view.encode_to_vec(), historical);
    assert_eq!(new_view.encode_to_vec(), current);
    assert_eq!(old_view.to_owned_message().unwrap(), old);
    assert_eq!(new_view.to_owned_message().unwrap(), new);
}

#[test]
fn reclaimed_labyrinth_outputs_preserve_flat_and_wrapped_payloads() {
    let historical = [0x0a, 1, 42];
    let current = [0x0a, 3, 0x0a, 1, 42];
    let old = wa::DecryptMessageOutput::decode_from_slice(&historical).unwrap();
    let new = wa::WebDecryptMessageOutput::decode_from_slice(&current).unwrap();
    assert_eq!(old.plaintext_payload.as_deref(), Some([42].as_slice()));
    let Some(wa::web_decrypt_message_output::Result::Success(success)) = &new.result else {
        panic!("expected wrapped success");
    };
    assert_eq!(success.plaintext_payload, [42]);
    assert_eq!(old.encode_to_vec(), historical);
    assert_eq!(new.encode_to_vec(), current);
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
fn bot_metadata_clone_preserves_nested_fields_without_aliasing() {
    let mut session = wa::BotSessionMetadata::default();
    session.session_id = Some("fixture-session".into());
    let mut original = wa::BotMetadata::default();
    original.persona_id = Some("fixture-persona".into());
    original.session_metadata = Some(session).into();
    original.ai_conversation_context = Some(vec![1, 2, 3]);

    let encoded = original.encode_to_vec();
    let mut cloned = original.clone();
    assert_eq!(cloned, original);
    assert_eq!(cloned.encode_to_vec(), encoded);
    cloned.session_metadata.as_option_mut().unwrap().session_id = Some("changed".into());
    assert_eq!(
        original.session_metadata.session_id.as_deref(),
        Some("fixture-session")
    );
    cloned.clone_from(&original);
    assert_eq!(cloned, original);
}

#[test]
fn media_message_clone_preserves_nested_data_without_aliasing() {
    let mut context = wa::ContextInfo::default();
    context.stanza_id = Some("fixture-stanza".into());
    let mut image = wa::message::ImageMessage::default();
    image.caption = Some("fixture-image".into());
    image.jpeg_thumbnail = Some(vec![1, 2, 3]);
    image.context_info = Some(context).into();
    let mut message = wa::Message::default();
    message.image_message = Some(image).into();

    let encoded = message.encode_to_vec();
    let mut cloned = message.clone();
    assert_eq!(cloned, message);
    assert_eq!(cloned.encode_to_vec(), encoded);
    cloned
        .image_message
        .as_option_mut()
        .unwrap()
        .context_info
        .as_option_mut()
        .unwrap()
        .stanza_id = Some("changed".into());
    assert_eq!(
        message.image_message.context_info.stanza_id.as_deref(),
        Some("fixture-stanza")
    );
}

#[test]
fn wallpaper_string_and_argb_encodings_keep_their_distinct_wire_types() {
    use wa::message::{
        ChatSolidColorWallpaper, ChatSolidColorWallpaperOwnedView, ChatSolidColorWallpaperView,
    };
    use waproto::protobuf::{MessageView as _, ViewEncode as _};
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
