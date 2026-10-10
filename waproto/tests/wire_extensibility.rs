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
fn failed_nested_oneof_merge_keeps_valid_partial_edits() {
    use wa::message::interactive_message::{Header, HeaderView, header::Media};
    use waproto::buffa::{DecodeContext, ViewEncode, encoding::Tag};

    fn caption(header: &Header) -> Option<&str> {
        match header.media.as_ref() {
            Some(Media::ImageMessage(image)) => image.caption.as_deref(),
            _ => None,
        }
    }

    // Future field, then image caption "a". A second image accepts caption
    // "c" before failing on an unknown varint field with no payload.
    let first = [0xc0, 0x3e, 7, 0x22, 3, 0x1a, 1, b'a'];
    let failed = [0x22, 5, 0x1a, 1, b'c', 0xc0, 0x3e];
    let completed = [0x22, 3, 0x1a, 1, b'b'];
    let batch = [completed.as_slice(), failed.as_slice()].concat();
    let expected = [0xc0, 0x3e, 7, 0x22, 3, 0x1a, 1, b'c'];
    let unknown = core::cell::Cell::new(usize::MAX);
    let ctx = DecodeContext::new(100, &unknown);

    let mut owned = Header::decode_from_slice(&first).unwrap();
    let mut payload = &failed[..];
    let tag = Tag::decode(&mut payload).unwrap();
    assert!(owned.merge_field(tag, &mut payload, ctx).is_err());
    assert_eq!(caption(&owned), Some("c"));
    assert_eq!(owned.encode_to_vec(), expected, "direct owned merge");

    let mut view = HeaderView::decode_view(&first).unwrap();
    assert!(
        view.merge_view_field(tag, &failed[1..], &failed, ctx)
            .is_err()
    );
    assert_eq!(caption(&view.to_owned_message().unwrap()), Some("c"));
    assert_eq!(view.encode_to_vec(), expected, "direct view merge");

    let mut owned = Header::decode_from_slice(&first).unwrap();
    assert!(owned.merge_to_limit(&mut &batch[..], ctx, 0).is_err());
    assert_eq!(caption(&owned), Some("c"));
    assert_eq!(owned.encode_to_vec(), expected, "batched owned merge");

    let mut view = HeaderView::decode_view(&first).unwrap();
    assert!(view.merge_into_view(&batch, ctx).is_err());
    assert_eq!(caption(&view.to_owned_message().unwrap()), Some("c"));
    assert_eq!(view.encode_to_vec(), expected, "batched view merge");
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}

#[test]
fn failed_nested_oneof_without_mutation_keeps_future_order() {
    use wa::message::interactive_message::{Header, HeaderView};
    use waproto::buffa::{DecodeContext, ViewEncode};
    let first = [0xc0, 0x3e, 7, 0x22, 3, 0x1a, 1, b'a'];
    let completed = [0x22, 3, 0x1a, 1, b'b', 0xc8, 0x3e, 8];
    let failed = [0x22, 2, 0xc0, 0x3e];
    let batch = [completed.as_slice(), failed.as_slice()].concat();
    let expected = [first.as_slice(), completed.as_slice()].concat();
    let unknown = core::cell::Cell::new(usize::MAX);
    let ctx = DecodeContext::new(100, &unknown);
    let mut owned = Header::decode_from_slice(&first).unwrap();
    assert!(owned.merge_to_limit(&mut &batch[..], ctx, 0).is_err());
    assert_eq!(owned.encode_to_vec(), expected);
    let mut view = HeaderView::decode_view(&first).unwrap();
    assert!(view.merge_into_view(&batch, ctx).is_err());
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
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

#[test]
fn failed_enum_batch_keeps_unreceived_nonoptional_default() {
    use waproto::buffa::{DecodeContext, ViewEncode};
    // FilterClause exposes a nonoptional enum with default AND = 1.
    // Journal activation must seed it even when the field was not received.
    let first = [0xc0, 0x3e, 7];
    let later = [0x08, 2, 0x80];
    let expected = [0x08, 1, 0xc0, 0x3e, 7, 0x08, 2];
    let unknown = core::cell::Cell::new(usize::MAX);
    let ctx = DecodeContext::new(100, &unknown);
    let mut owned = wa::qp::FilterClause::decode_from_slice(&first).unwrap();
    assert!(owned.merge_to_limit(&mut &later[..], ctx, 0).is_err());
    assert_eq!(owned.encode_to_vec(), expected);
    let mut view = wa::qp::FilterClauseView::decode_view(&first).unwrap();
    assert!(view.merge_into_view(&later, ctx).is_err());
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}

#[test]
fn retry_after_failed_fragmented_batch_fits_linear_memory_budget() {
    use waproto::buffa::{DecodeContext, ViewEncode};
    let first = [0xc0, 0x3e, 7];
    let mut batch = Vec::new();
    for _ in 0..512 {
        batch.extend_from_slice(&[0x32, 2, 0x0a, 0]);
    }
    batch.push(0x80);
    let retry = [0x32, 0];
    let mut expected = first.to_vec();
    expected.extend_from_slice(&batch[..batch.len() - 1]);
    let unknown = core::cell::Cell::new(usize::MAX);
    let mut owned = wa::message::InteractiveMessage::decode_from_slice(&first).unwrap();
    let mut view = wa::message::InteractiveMessageView::decode_view(&first).unwrap();
    assert!(owned.merge_from_slice(&batch).is_err());
    assert!(
        view.merge_into_view(&batch, DecodeContext::new(100, &unknown))
            .is_err()
    );
    assert_eq!(owned.encode_to_vec(), expected);
    assert_eq!(view.encode_to_vec(), expected);
    for allowance in [0, 4096] {
        let budget = core::cell::Cell::new(allowance);
        let ctx = DecodeContext::new(100, &unknown).with_element_memory(&budget);
        assert!(owned.merge(&mut &retry[..], ctx).is_err());
        if allowance != 0 {
            assert!(
                budget.get() < 256,
                "failed replay debits decoded elements, not only input buffers"
            );
        }
        assert_eq!(owned.encode_to_vec(), expected);
        let budget = core::cell::Cell::new(allowance);
        let ctx = DecodeContext::new(100, &unknown).with_element_memory(&budget);
        assert!(view.merge_into_view(&retry, ctx).is_err());
        if allowance != 0 {
            assert!(
                budget.get() < 256,
                "failed view replay debits decoded elements, not only input buffers"
            );
        }
        assert_eq!(view.encode_to_vec(), expected);
        assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
    }
    let budget = core::cell::Cell::new(128 * 1024);
    let ctx = DecodeContext::new(100, &unknown).with_element_memory(&budget);
    let mut bytes = &retry[..];
    owned
        .merge(&mut bytes, ctx)
        .expect("owned retry fits the element budget");
    let budget = core::cell::Cell::new(128 * 1024);
    let ctx = DecodeContext::new(100, &unknown).with_element_memory(&budget);
    view.merge_into_view(&retry, ctx)
        .expect("view retry fits the element budget");
    expected.extend_from_slice(&retry);
    assert_eq!(owned.encode_to_vec(), expected);
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}

#[test]
fn recovery_replays_nested_unknowns_without_recharging_the_caller_allowance() {
    use wa::message::interactive_message::{Header, HeaderView};
    use waproto::buffa::{DecodeContext, ViewEncode};
    let first = [0xc0, 0x3e, 7, 0x22, 6, 0x1a, 1, b'a', 0xc0, 0x3e, 9];
    let completed = [0x22, 6, 0x1a, 1, b'b', 0xc0, 0x3e, 8];
    let failed = [0x22, 1, 0x80];
    let batch = [completed.as_slice(), failed.as_slice()].concat();
    let retry = [0x22, 0];
    let expected = [first.as_slice(), completed.as_slice(), retry.as_slice()].concat();
    let mut owned = Header::decode_from_slice(&first).unwrap();
    let mut view = HeaderView::decode_view(&first).unwrap();
    let unknown = core::cell::Cell::new(1);
    assert!(
        owned
            .merge(&mut &batch[..], DecodeContext::new(100, &unknown))
            .is_err()
    );
    assert_eq!(unknown.get(), 0);
    let unknown = core::cell::Cell::new(1);
    assert!(
        view.merge_into_view(&batch, DecodeContext::new(100, &unknown))
            .is_err()
    );
    assert_eq!(unknown.get(), 0);
    let budget = core::cell::Cell::new(128 * 1024);
    owned
        .merge(
            &mut &retry[..],
            DecodeContext::new(100, &unknown).with_element_memory(&budget),
        )
        .unwrap();
    assert_eq!(unknown.get(), 0);
    let budget = core::cell::Cell::new(128 * 1024);
    view.merge_into_view(
        &retry,
        DecodeContext::new(100, &unknown).with_element_memory(&budget),
    )
    .unwrap();
    assert_eq!(unknown.get(), 0);
    assert_eq!(owned.encode_to_vec(), expected);
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), expected);
}
