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
