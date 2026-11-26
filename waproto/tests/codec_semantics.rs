use waproto::protobuf::MessageView as _;
use waproto::protobuf::{DecodeError, DecodeOptions, Message as _};
use waproto::whatsapp::message::interactive_message::{Header, header::Media};
use waproto::whatsapp::{
    ADVKeyIndexList, ADVKeyIndexListView, BotCapabilityMetadata, BotCapabilityMetadataView,
};

#[test]
fn repeated_numeric_fields_respect_element_memory_limit() {
    let options = DecodeOptions::new().with_element_memory_limit(0);
    for input in [&[0x22, 1, 0][..], &[0x20, 0][..]] {
        assert_eq!(
            options
                .decode_from_slice::<ADVKeyIndexList>(input)
                .unwrap_err(),
            DecodeError::ElementMemoryLimitExceeded
        );
        assert_eq!(
            options
                .decode_view::<ADVKeyIndexListView<'_>>(input)
                .unwrap_err(),
            DecodeError::ElementMemoryLimitExceeded
        );
    }
}

#[test]
fn numeric_element_budget_is_shared_across_packed_and_unpacked_values() {
    let input = [0x22, 1, 1, 0x20, 2];
    let one_element = DecodeOptions::new().with_element_memory_limit(size_of::<u32>());
    assert_eq!(
        one_element
            .decode_from_slice::<ADVKeyIndexList>(&input)
            .unwrap_err(),
        DecodeError::ElementMemoryLimitExceeded
    );
    assert_eq!(
        one_element
            .decode_view::<ADVKeyIndexListView<'_>>(&input)
            .unwrap_err(),
        DecodeError::ElementMemoryLimitExceeded
    );
    let two_elements = DecodeOptions::new().with_element_memory_limit(2 * size_of::<u32>());
    assert_eq!(
        two_elements
            .decode_from_slice::<ADVKeyIndexList>(&input)
            .unwrap()
            .valid_indexes,
        [1, 2]
    );
    assert_eq!(
        &*two_elements
            .decode_view::<ADVKeyIndexListView<'_>>(&input)
            .unwrap()
            .valid_indexes,
        &[1, 2]
    );
}

#[test]
fn closed_enum_budget_counts_only_recognized_values() {
    let options = DecodeOptions::new().with_element_memory_limit(0);
    for input in [&[0x08, 1][..], &[0x0a, 1, 1][..]] {
        assert_eq!(
            options
                .decode_from_slice::<BotCapabilityMetadata>(input)
                .unwrap_err(),
            DecodeError::ElementMemoryLimitExceeded
        );
        assert_eq!(
            options
                .decode_view::<BotCapabilityMetadataView<'_>>(input)
                .unwrap_err(),
            DecodeError::ElementMemoryLimitExceeded
        );
    }
    for input in [&[0x08, 127][..], &[0x0a, 1, 127][..]] {
        assert!(
            options
                .decode_from_slice::<BotCapabilityMetadata>(input)
                .unwrap()
                .capabilities
                .is_empty()
        );
        assert!(
            options
                .decode_view::<BotCapabilityMetadataView<'_>>(input)
                .unwrap()
                .capabilities
                .is_empty()
        );
    }
}

#[test]
#[allow(clippy::disallowed_methods)]
fn malformed_new_message_variant_preserves_previous_oneof() {
    let mut header = Header::default();
    header.media = Some(Media::JpegThumbnail(vec![1, 2, 3]));
    let original = header.clone();

    assert!(header.merge_from_slice(&[0x1a, 1, 0x80]).is_err());
    assert_eq!(header, original);
}

#[test]
#[allow(clippy::disallowed_methods)]
fn repeated_message_variant_merges_into_existing_oneof() {
    let mut image = waproto::whatsapp::message::ImageMessage::default();
    image.caption = Some("caption".into());
    let mut header = Header::default();
    header.media = Some(Media::ImageMessage(Box::new(image)));

    header.merge_from_slice(&[0x22, 3, 0x0a, 1, b'x']).unwrap();
    let Some(Media::ImageMessage(image)) = header.media else {
        panic!("expected image");
    };
    assert_eq!(image.url.as_deref(), Some("x"));
    assert_eq!(image.caption.as_deref(), Some("caption"));
}

#[test]
fn required_view_presence_distinguishes_absent_and_default_values() {
    use waproto::whatsapp::{
        CitationView, ConversationView, CreateBackupSuccessView, HistorySyncView,
    };

    let absent = ConversationView::decode_view(&[]).unwrap();
    let empty = ConversationView::decode_view(&[0x0a, 0]).unwrap();
    assert!(!absent.has_id());
    assert!(empty.has_id());
    assert_eq!(absent.id, empty.id);

    let citation = CitationView::decode_view(&[0x0a, 0, 0x1a, 0]).unwrap();
    assert!(citation.has_title());
    assert!(!citation.has_subtitle());
    assert!(citation.has_cms_id());
    assert!(!citation.has_image_url());

    let absent_message = CreateBackupSuccessView::decode_view(&[]).unwrap();
    let empty_message = CreateBackupSuccessView::decode_view(&[0x0a, 0]).unwrap();
    assert!(!absent_message.has_device());
    assert!(empty_message.has_device());

    let absent_enum = HistorySyncView::decode_view(&[]).unwrap();
    let default_enum = HistorySyncView::decode_view(&[0x08, 0]).unwrap();
    let unknown_enum = HistorySyncView::decode_view(&[0x08, 127]).unwrap();
    assert!(!absent_enum.has_sync_type());
    assert!(default_enum.has_sync_type());
    assert!(!unknown_enum.has_sync_type());
}

#[test]
fn unknown_field_budget_is_shared_by_owned_and_view_decoders() {
    use waproto::whatsapp::{Message, MessageView};

    let input = [0x80, 0xb5, 0x18, 1];
    let options = DecodeOptions::new().with_unknown_field_limit(0);
    assert_eq!(
        options.decode_from_slice::<Message>(&input).unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
    assert_eq!(
        options.decode_view::<MessageView<'_>>(&input).unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
}

#[test]
fn unknown_field_budget_covers_nested_messages_maps_groups_and_closed_enums() {
    use waproto::whatsapp::{
        Config, ConfigView, HistorySync, HistorySyncView, Message, MessageView,
    };

    let zero = DecodeOptions::new().with_unknown_field_limit(0);
    let map_entry = [0x0a, 4, 0x08, 1, 0x18, 1];
    assert_eq!(
        zero.decode_from_slice::<Config>(&map_entry).unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
    assert_eq!(
        zero.decode_view::<ConfigView<'_>>(&map_entry).unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
    let unknown_enum = [0x08, 127];
    assert_eq!(
        zero.decode_from_slice::<HistorySync>(&unknown_enum)
            .unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
    assert_eq!(
        zero.decode_view::<HistorySyncView<'_>>(&unknown_enum)
            .unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );

    let one = DecodeOptions::new().with_unknown_field_limit(1);
    let nested = [0x80, 0xb5, 0x18, 1, 0x32, 4, 0x80, 0xb5, 0x18, 1];
    let group = [0x83, 0xb5, 0x18, 0x08, 1, 0x84, 0xb5, 0x18];
    for input in [&nested[..], &group[..]] {
        assert_eq!(
            one.decode_from_slice::<Message>(input).unwrap_err(),
            DecodeError::UnknownFieldLimitExceeded
        );
        assert_eq!(
            one.decode_view::<MessageView<'_>>(input).unwrap_err(),
            DecodeError::UnknownFieldLimitExceeded
        );
        let two = DecodeOptions::new().with_unknown_field_limit(2);
        assert!(two.decode_from_slice::<Message>(input).is_ok());
        assert!(two.decode_view::<MessageView<'_>>(input).is_ok());
    }
}
