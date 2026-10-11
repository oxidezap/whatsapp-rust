#![allow(clippy::disallowed_methods)]
#[path = "../build_support/semantic_flat.rs"]
mod flat;
#[path = "fixtures/semantic_projection.rs"]
pub mod semantic;
use buffa::Message as _;
use semantic::Visitor as _;
use waproto::whatsapp as wa;
include!(concat!(env!("OUT_DIR"), "/semantic_header_visitors.rs"));

#[test]
fn actual_header_projection_visits_nested_messages_without_encoding() {
    let mut header = wa::message::interactive_message::Header::default();
    header.title = Some("outside projection".into());
    let mut quote = wa::Message::default();
    quote.conversation = Some("fictitious quote".into());
    let mut context = wa::ContextInfo::default();
    context.quoted_message = quote.into();
    let mut image = wa::message::ImageMessage::default();
    image.caption = Some("fictitious caption".into());
    image.jpeg_thumbnail = Some(vec![42; 64]);
    image.context_info = context.into();
    header.media =
        Some(wa::message::interactive_message::header::Media::ImageMessage(Box::new(image)));
    let unknown = std::cell::Cell::new(usize::MAX);
    let projection =
        semantic::Arena::from_visitor(SCHEMA, &header, buffa::DecodeContext::new(100, &unknown))
            .unwrap();
    assert_eq!(projection.encode_calls(), 0);
    assert!(projection.matches(&header));
    header.title = None;
    assert_eq!(projection.encode(), header.encode_to_vec());
    let size = projection_size(0, &header).unwrap();
    assert_eq!(size.encoded, header.encode_to_vec().len());
    assert!(flat_activation_fits(&size, 40));
}
#[test]
fn actual_descendant_enum_unknown_and_recursion_select_fallback() {
    let mut image = wa::message::ImageMessage::default();
    assert!(image.supported());
    image.image_source_type = Some(wa::message::image_message::ImageSourceType::UserImage);
    assert!(!image.supported());
    image.image_source_type = None;
    image.__buffa_unknown_fields.push(buffa::UnknownField {
        number: 199,
        data: buffa::UnknownFieldData::Varint(1),
    });
    assert!(!image.supported());
    let mut message = wa::Message::default();
    assert!(message.supported());
    message.image_message = wa::message::ImageMessage::default().into();
    assert!(!message.supported());
}

#[test]
fn empty_present_nested_messages_and_bytes_remain_distinct_from_absence() {
    use wa::message::interactive_message::{Header, header::Media};
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = buffa::DecodeContext::new(100, &unknown);
    let mut thumbnail = Header::decode_from_slice(&[0x32, 0]).unwrap();
    let projection = semantic::Arena::from_visitor(SCHEMA, &thumbnail, ctx).unwrap();
    assert_eq!(projection.encode(), [0x32, 0]);
    thumbnail.media = None;
    assert!(!projection.matches(&thumbnail));

    let wire = [0x22, 5, 0x8a, 1, 2, 0x1a, 0];
    let mut header = Header::decode_from_slice(&wire).unwrap();
    let projection = semantic::Arena::from_visitor(SCHEMA, &header, ctx).unwrap();
    assert_eq!(projection.encode(), wire);
    let Some(Media::ImageMessage(image)) = header.media.as_mut() else {
        panic!("image");
    };
    image.context_info.as_option_mut().unwrap().quoted_message = Default::default();
    assert!(!projection.matches(&header));
}
#[test]
fn a_hidden_future_enum_selects_fallback_even_when_the_typed_field_is_absent() {
    let image = wa::message::ImageMessage::decode_from_slice(&[0xf8, 1, 99]).unwrap();
    assert!(image.image_source_type.is_none());
    assert!(image.__buffa_unknown_fields.active());
    assert!(!image.supported());
}
#[test]
fn failed_actual_image_occurrence_does_not_bless_its_partial_caption() {
    use wa::message::interactive_message::Header;
    let mut header = Header::decode_from_slice(&[0x22, 3, 0x1a, 1, b'x']).unwrap();
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = buffa::DecodeContext::new(100, &unknown);
    let mut projection = semantic::Arena::from_visitor(SCHEMA, &header, ctx).unwrap();
    let before = projection.encode();
    let failed = [0x22, 5, 0x1a, 1, b'y', 0xc0, 0x3e];
    assert!(projection.stage(&failed, ctx).is_err());
    assert!(header.merge_from_slice(&failed).is_err());
    assert_eq!(projection.encode(), before);
    assert!(!projection.matches(&header));
}

#[test]
fn incoming_unsupported_and_unknown_children_are_rejected_before_mutation_without_budget_debits() {
    let unknown = std::cell::Cell::new(0);
    let budget = std::cell::Cell::new(0);
    let ctx = buffa::DecodeContext::new(100, &unknown).with_element_memory(&budget);
    assert!(can_stage_known(&[0x22, 3, 0x1a, 1, b'x'], ctx).unwrap());
    // Declared enum31 and truly unknown199 both require the original decoder.
    assert!(!can_stage_known(&[0x22, 3, 0xf8, 1, 99], ctx).unwrap());
    assert!(!can_stage_known(&[0x22, 3, 0xb8, 12, 1], ctx).unwrap());
    assert!(!can_stage_known(&[0x1a, 0], ctx).unwrap());
    assert!(!can_stage_known(&[0x0a, 1, b'x'], ctx).unwrap());
    assert!(!can_stage_known(&[0x22, 5, 0x1a, 1, b'y', 0xc0, 0x3e], ctx).unwrap());
    assert!(matches!(
        can_stage_known(&[0x22, 3, 0x1a, 2, b'y'], ctx),
        Err(buffa::DecodeError::UnexpectedEof)
    ));
    assert_eq!(unknown.get(), 0);
    assert_eq!(budget.get(), 0);
}
#[test]
fn the_effective_locally_retained_message_field_is_not_omitted_by_the_visitor_guard() {
    let mut message = wa::Message::default();
    message.newsletter_admin_profile_message_v2 = wa::message::FutureProofMessage::default().into();
    assert!(!message.supported());
}

#[test]
fn a_supported_record_with_invalid_utf8_cannot_commit_its_staged_projection() {
    use wa::message::interactive_message::{Header, header::Media};
    let mut header = Header::decode_from_slice(&[0x22, 3, 0x1a, 1, b'x']).unwrap();
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = buffa::DecodeContext::new(100, &unknown);
    let mut projection = semantic::Arena::from_visitor(SCHEMA, &header, ctx).unwrap();
    let before = projection.encode();
    let failed = [0x22, 6, 0x1a, 1, b'y', 0x1a, 1, 0xff];
    assert!(can_stage_known(&failed, ctx).unwrap());
    let staged = projection.stage(&failed, ctx).unwrap();
    assert!(header.merge_from_slice(&failed).is_err());
    drop(staged);
    assert_eq!(projection.encode(), before);
    let Some(Media::ImageMessage(image)) = header.media else {
        panic!("image");
    };
    assert_eq!(image.caption.as_deref(), Some(""));
}

#[test]
fn activation_cost_probe_keeps_small_shapes_on_the_original_backend_without_debits() {
    use wa::message::interactive_message::Header;
    let unknown = std::cell::Cell::new(0);
    let budget = std::cell::Cell::new(0);
    let ctx = buffa::DecodeContext::new(100, &unknown).with_element_memory(&budget);
    for wire in [
        &[][..],
        &[0x22, 0],
        &[0x32, 0],
        &[0x22, 5, 0x8a, 1, 2, 0x1a, 0],
    ] {
        let header = Header::decode_from_slice(wire).unwrap();
        let size = projection_size(0, &header).unwrap();
        assert_eq!(size.encoded, header.encode_to_vec().len());
        assert!(!flat_activation_fits(&size, 40));
        assert!(flat::Prefix::freeze(&header, ctx).is_err());
    }
    assert!(!flat_activation_fits(
        &ProjectionSize {
            encoded: usize::MAX,
            payload: 0
        },
        40
    ));
    assert_eq!(unknown.get(), 0);
    assert_eq!(budget.get(), 0);
}

#[test]
fn frozen_prefix_canonicalizes_in_place_across_presence_and_length_boundaries() {
    use wa::message::interactive_message::{Header, header::Media};
    let unknown = std::cell::Cell::new(usize::MAX);
    let ctx = buffa::DecodeContext::new(100, &unknown);
    for len in [0, 1, 127, 128, 255, 16_383, 16_384] {
        let mut quote = wa::Message::default();
        quote.conversation = Some("q".repeat(len));
        let mut context = wa::ContextInfo::default();
        context.quoted_message = quote.into();
        let mut image = wa::message::ImageMessage::default();
        image.caption = Some("c".repeat(len));
        image.jpeg_thumbnail = Some(vec![42; len]);
        image.context_info = context.into();
        let mut header = Header::default();
        header.media = Some(Media::ImageMessage(Box::new(image)));
        let prefix = flat::Prefix::freeze(&header, ctx).unwrap();
        let address = prefix.allocation_address();
        let capacity = prefix.capacity();
        let wire = prefix.into_wire().unwrap();
        assert_eq!(wire.as_ptr() as usize, address);
        assert_eq!(wire.capacity(), capacity);
        assert_eq!(wire, header.encode_to_vec());
    }
    for wire in [
        &[][..],
        &[0x22, 0],
        &[0x32, 0],
        &[0x22, 5, 0x8a, 1, 2, 0x1a, 0],
    ] {
        let header = Header::decode_from_slice(wire).unwrap();
        assert_eq!(
            flat::Prefix::freeze(&header, ctx)
                .unwrap()
                .into_wire()
                .unwrap(),
            wire
        );
    }
}

#[test]
fn snapshotting_existing_fields_does_not_spend_the_incoming_recursion_allowance() {
    use wa::message::interactive_message::Header;
    let wire = [0x22, 8, 0x8a, 1, 5, 0x1a, 3, 0x0a, 1, b'q'];
    let mut header = Header::decode_from_slice(&wire).unwrap();
    let unknown = std::cell::Cell::new(100);
    let ctx = buffa::DecodeContext::new(0, &unknown);
    let future = [0xc2, 0x3e, 1, b'x'];
    header.merge(&mut future.as_slice(), ctx).unwrap();
    let prefix = flat::Prefix::freeze(&header, ctx).unwrap();
    assert_eq!(prefix.into_wire().unwrap(), wire);
    assert_eq!(projection_size(0, &header).unwrap().encoded, wire.len());
}

#[test]
fn prefix_preparation_reserves_before_copying_and_conversion_needs_no_further_budget() {
    use wa::message::interactive_message::Header;
    let header = Header::decode_from_slice(&[0x32, 1, b'x']).unwrap();
    let unknown = std::cell::Cell::new(0);
    let short = std::cell::Cell::new(flat::METADATA);
    let ctx = buffa::DecodeContext::new(0, &unknown).with_element_memory(&short);
    assert!(matches!(
        flat::Prefix::freeze(&header, ctx),
        Err(buffa::DecodeError::ElementMemoryLimitExceeded)
    ));
    assert_eq!(short.get(), flat::METADATA);
    let exact = std::cell::Cell::new(flat::METADATA + 1);
    let ctx = ctx.with_element_memory(&exact);
    let prefix = flat::Prefix::freeze(&header, ctx).unwrap();
    assert_eq!(exact.get(), 0);
    assert_eq!(prefix.into_wire().unwrap(), header.encode_to_vec());
    assert_eq!(exact.get(), 0);
    assert_eq!(unknown.get(), 0);
}
