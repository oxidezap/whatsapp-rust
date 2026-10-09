//! Synthetic bytes exercise the persisted/wire contract independently of the
//! construction and matching restrictions checked by generated_api.
#![allow(clippy::disallowed_methods)]

use waproto::buffa::{Message, MessageView};
use waproto::whatsapp as wa;

#[test]
fn unknown_message_fields_survive_owned_and_view_edits() {
    // conversation = "old", then unknown field 1000 = 7.
    let wire = [0x0a, 3, b'o', b'l', b'd', 0xc0, 0x3e, 7];
    let mut message = wa::Message::decode_from_slice(&wire).unwrap();
    message.conversation = Some("new".into());
    let expected = [0x0a, 3, b'n', b'e', b'w', 0xc0, 0x3e, 7];
    assert_eq!(message.encode_to_vec(), expected);
    let handle = wa::MessageOwnedView::decode(wire.to_vec().into()).unwrap();
    let mut restored = handle.to_owned_message();
    restored.conversation = Some("new".into());
    assert_eq!(restored.encode_to_vec(), expected);
    let view = wa::MessageView::decode_view(&wire).unwrap();
    let mut message = view.to_owned_message().unwrap();
    message.conversation = Some("new".into());
    assert_eq!(message.encode_to_vec(), expected);
    let json = serde_json::to_value(message).unwrap();
    assert_eq!(json["conversation"], "new");
    assert!(
        json.as_object()
            .unwrap()
            .keys()
            .all(|key| !key.starts_with("__"))
    );
}

#[test]
fn unknown_closed_enum_value_survives_roundtrip() {
    // ADVDeviceIdentity.accountType = 99. A closed enum exposes no fabricated
    // known value, but its original number must survive a subsequent encode.
    let wire = [0x20, 99];
    let message = wa::ADVDeviceIdentity::decode_from_slice(&wire).unwrap();
    assert_eq!(message.account_type, None);
    assert_eq!(message.encode_to_vec(), wire);
    let view = wa::ADVDeviceIdentityView::decode_view(&wire).unwrap();
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn repeated_bot_selection_retains_future_value_positions() {
    use waproto::buffa::ViewEncode;
    let expected = [0x08, 0, 0x08, 2, 0x08, 1];
    for wire in [expected.as_slice(), &[0x0a, 3, 0, 2, 1]] {
        let owned = wa::BotModeSelectionMetadata::decode_from_slice(wire).unwrap();
        let view = wa::BotModeSelectionMetadataView::decode_view(wire).unwrap();
        assert_eq!(owned.mode.len(), 2);
        assert_eq!(owned.encode_to_vec(), expected);
        assert_eq!(view.encode_to_vec(), expected);
        assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
    }
}

#[test]
fn repeated_bot_selection_long_list_fits_linear_memory_budget() {
    use waproto::buffa::{DecodeOptions, ViewEncode};
    let mut wire = vec![0x08, 2, 0x0a, 0x80, 4];
    wire.extend(std::iter::repeat_n(0, 512));
    let mut expected = vec![0x08, 2];
    for _ in 0..512 {
        expected.extend_from_slice(&[0x08, 0]);
    }
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options
        .decode_from_slice::<wa::BotModeSelectionMetadata>(&wire)
        .unwrap();
    let view = options
        .decode_view::<wa::BotModeSelectionMetadataView<'_>>(&wire)
        .unwrap();
    assert_eq!(owned.mode.len(), 512);
    assert_eq!(owned.encode_to_vec(), expected);
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}

#[test]
fn fragmented_message_oneof_fits_linear_memory_budget() {
    use waproto::buffa::{DecodeOptions, ViewEncode};
    let mut wire = vec![0xc0, 0x3e, 7];
    for _ in 0..512 {
        wire.extend_from_slice(&[0x32, 2, 0x0a, 0]);
    }
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options
        .decode_from_slice::<wa::message::InteractiveMessage>(&wire)
        .expect("small fragmented owned message");
    let view = options
        .decode_view::<wa::message::InteractiveMessageView<'_>>(&wire)
        .expect("small fragmented borrowed message");
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn empty_oneof_fragments_do_not_recopy_large_existing_payload() {
    use waproto::buffa::{DecodeOptions, ViewEncode};
    // Future field, then nativeFlowMessage containing a 16 KiB JSON string.
    let mut wire = vec![0xc0, 0x3e, 7, 0x32, 0x84, 0x80, 1, 0x12, 0x80, 0x80, 1];
    wire.extend(std::iter::repeat_n(b'x', 16 * 1024));
    for _ in 0..512 {
        wire.extend_from_slice(&[0x32, 0]);
    }
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options
        .decode_from_slice::<wa::message::InteractiveMessage>(&wire)
        .unwrap();
    let view = options
        .decode_view::<wa::message::InteractiveMessageView<'_>>(&wire)
        .unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn protocol_type_setter_keeps_same_value_replacement_after_future_type() {
    let wire = [0x10, 0, 0x10, 99];
    let message = wa::message::ProtocolMessage::decode_from_slice(&wire)
        .unwrap()
        .with_type(wa::message::protocol_message::Type::REVOKE);
    assert_eq!(message.encode_to_vec(), [0x10, 99, 0x10, 0]);
}

#[test]
fn image_header_accepts_packed_scan_lengths_within_decode_budget() {
    use waproto::buffa::{DecodeOptions, ViewEncode};
    // Future field, imageMessage, then 512 packed values of unpacked scanLengths.
    let mut wire = vec![0xc0, 0x3e, 7, 0x22, 0x84, 4, 0xb2, 1, 0x80, 4];
    wire.extend(std::iter::repeat_n(0, 512));
    let options = DecodeOptions::new().with_element_memory_limit(128 * 1024);
    let owned = options
        .decode_from_slice::<wa::message::interactive_message::Header>(&wire)
        .unwrap();
    let view = options
        .decode_view::<wa::message::interactive_message::HeaderView<'_>>(&wire)
        .unwrap();
    assert_eq!(owned.encode_to_vec(), wire);
    assert_eq!(view.encode_to_vec(), wire);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn failed_protocol_event_reservation_keeps_completed_later_type() {
    use waproto::buffa::{DecodeContext, ViewEncode};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 99, 0x10, 3];
    let expected = [&first[..], &later[..]].concat();
    for allowance in 0..600 {
        let unknown = core::cell::Cell::new(usize::MAX);
        let budget = core::cell::Cell::new(allowance);
        let mut owned = wa::message::ProtocolMessage::decode_from_slice(&first).unwrap();
        let _ = owned.merge_to_limit(
            &mut &later[..],
            DecodeContext::new(100, &unknown).with_element_memory(&budget),
            0,
        );
        if owned.r#type == Some(wa::message::protocol_message::Type::EPHEMERAL_SETTING) {
            assert_eq!(owned.encode_to_vec(), expected, "owned budget {allowance}");
        }
        let budget = core::cell::Cell::new(allowance);
        let mut view = wa::message::ProtocolMessageView::decode_view(&first).unwrap();
        let _ = view.merge_into_view(
            &later,
            DecodeContext::new(100, &unknown).with_element_memory(&budget),
        );
        if view.r#type == Some(wa::message::protocol_message::Type::EPHEMERAL_SETTING) {
            assert_eq!(view.encode_to_vec(), expected, "view budget {allowance}");
            assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
        }
    }
}

#[test]
fn failed_protocol_batch_keeps_future_type_after_revoke() {
    use waproto::buffa::{DecodeContext, ViewEncode};
    let first = [0xc0, 0x3e, 7];
    let later = [0x10, 0, 0x10, 99];
    let expected = [&first[..], &later[..]].concat();
    for allowance in 0..400 {
        let unknown = core::cell::Cell::new(usize::MAX);
        let budget = core::cell::Cell::new(allowance);
        let mut owned = wa::message::ProtocolMessage::decode_from_slice(&first).unwrap();
        let _ = owned.merge_to_limit(
            &mut &later[..],
            DecodeContext::new(100, &unknown).with_element_memory(&budget),
            0,
        );
        if owned.__buffa_unknown_fields.len() == 2 {
            assert_eq!(owned.encode_to_vec(), expected, "owned budget {allowance}");
        }
        let budget = core::cell::Cell::new(allowance);
        let mut view = wa::message::ProtocolMessageView::decode_view(&first).unwrap();
        let _ = view.merge_into_view(
            &later,
            DecodeContext::new(100, &unknown).with_element_memory(&budget),
        );
        if view.__buffa_unknown_fields.len() == 2 {
            assert_eq!(view.encode_to_vec(), expected, "view budget {allowance}");
            assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
        }
    }
}

#[test]
fn replacing_a_future_closed_enum_value_preserves_the_explicit_edit() {
    let mut message = wa::ADVDeviceIdentity::decode_from_slice(&[0x20, 99]).unwrap();
    message.account_type = Some(wa::ADVEncryptionType::HOSTED);
    assert_eq!(message.encode_to_vec(), [0x20, 99, 0x20, 1]);
    assert_eq!(
        wa::ADVDeviceIdentity::decode_from_slice(&message.encode_to_vec())
            .unwrap()
            .account_type,
        Some(wa::ADVEncryptionType::HOSTED),
    );
}

#[test]
fn local_seed_and_future_persisted_fields_survive() {
    use wa::session_structure::chain::MessageKey;
    assert_eq!(
        waproto::tags::session_structure::chain::message_key::SEED,
        100
    );
    // Locally retained seed = [1, 2, 3], then a future persisted field 101 = 7.
    let wire = [0xa2, 0x06, 3, 1, 2, 3, 0xa8, 0x06, 7];
    let key = MessageKey::decode_from_slice(&wire).unwrap();
    assert_eq!(key.seed.as_deref(), Some([1, 2, 3].as_slice()));
    assert_eq!(key.encode_to_vec(), wire);
    let json = serde_json::to_value(key).unwrap();
    assert!(json.get("seed").is_none());
    assert!(json.get("__buffa_unknown_fields").is_none());
}

#[test]
fn retained_lid_mapping_keeps_wire_identity() {
    let mut mapping = wa::LIDMigrationMapping::default();
    mapping.pn = 15_550_000_001;
    mapping.assigned_lid = 90_000_000_001;
    mapping.latest_lid = Some(90_000_000_002);
    let mut payload = wa::LIDMigrationMappingSyncPayload::default();
    payload.pn_to_lid_mappings.push(mapping);
    payload.chat_db_migration_timestamp = Some(123);
    let wire = payload.encode_to_vec();
    assert_eq!(
        wa::LIDMigrationMappingSyncPayload::decode_from_slice(&wire).unwrap(),
        payload
    );
}
