#![allow(clippy::disallowed_methods)]
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
