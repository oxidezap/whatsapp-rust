use waproto::protobuf::MessageView as _;
#[cfg(feature = "protobuf-reference")]
#[path = "support/reference.rs"]
mod reference;
#[cfg(feature = "protobuf-reference")]
#[path = "support/reference_enum_serde.rs"]
mod reference_enum_serde;
use waproto::protobuf::{DecodeError, DecodeOptions, Message as _};
use waproto::whatsapp::message::interactive_message::{Header, header::Media};
use waproto::whatsapp::{
    ADVKeyIndexList, ADVKeyIndexListView, BotCapabilityMetadata, BotCapabilityMetadataView,
};

#[test]
fn single_text_view_preserves_presence_and_decode_limits() {
    let options = DecodeOptions::new()
        .with_recursion_limit(0)
        .with_unknown_field_limit(0)
        .with_element_memory_limit(0);
    let decoded = options
        .decode_view::<waproto::whatsapp::MessageView<'_>>(&[0x0a, 0])
        .unwrap();
    assert_eq!(decoded.conversation, Some(""));
    assert!(matches!(
        DecodeOptions::new()
            .with_max_message_size(1)
            .decode_view::<waproto::whatsapp::MessageView<'_>>(&[0x0a, 0]),
        Err(DecodeError::MessageTooLarge)
    ));
    let mut with_unknown = vec![0x0a, 30];
    with_unknown.extend_from_slice(&[b'a'; 30]);
    with_unknown.extend_from_slice(&[0x82, 0xb5, 0x18, 1, b'x']);
    assert!(matches!(
        options.decode_view::<waproto::whatsapp::MessageView<'_>>(&with_unknown),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}

#[test]
fn single_numeric_view_preserves_presence_and_decode_limits() {
    let options = DecodeOptions::new()
        .with_recursion_limit(0)
        .with_unknown_field_limit(0)
        .with_element_memory_limit(0);
    let decoded = options
        .decode_view::<ADVKeyIndexListView<'_>>(&[0x08, 0x96, 0x01])
        .unwrap();
    assert_eq!(decoded.raw_id, Some(150));
    assert!(matches!(
        options.decode_view::<ADVKeyIndexListView<'_>>(&[0x0a, 1, 0]),
        Err(DecodeError::WireTypeMismatch { .. })
    ));
    assert!(matches!(
        options.decode_view::<ADVKeyIndexListView<'_>>(&[0x08, 1, 0x80, 0xb5, 0x18, 1]),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}

struct CustomIndexView<'a>(ADVKeyIndexListView<'a>);

impl<'a> waproto::protobuf::MessageView<'a> for CustomIndexView<'a> {
    type Owned = ADVKeyIndexList;

    fn decode_view_with_ctx(
        input: &'a [u8],
        context: waproto::protobuf::DecodeContext<'_>,
    ) -> Result<Self, DecodeError> {
        ADVKeyIndexListView::decode_view_with_ctx(input, context).map(Self)
    }

    fn merge_view_field(
        &mut self,
        tag: waproto::protobuf::encoding::Tag,
        input: &'a [u8],
        before_tag: &'a [u8],
        context: waproto::protobuf::DecodeContext<'_>,
    ) -> Result<&'a [u8], DecodeError> {
        self.0.merge_view_field(tag, input, before_tag, context)
    }

    fn to_owned_message(&self) -> Result<Self::Owned, DecodeError> {
        self.0.to_owned_message()
    }
}

#[test]
fn custom_view_respects_decode_options() {
    assert!(CustomIndexView::decode_view(&[0x22, 1, 0]).is_ok());
    let options = DecodeOptions::new().with_element_memory_limit(0);
    assert!(matches!(
        options.decode_view::<CustomIndexView<'_>>(&[0x22, 1, 0]),
        Err(DecodeError::ElementMemoryLimitExceeded)
    ));
    let options = DecodeOptions::new().with_unknown_field_limit(0);
    assert!(matches!(
        options.decode_view::<CustomIndexView<'_>>(&[0x80, 0xb5, 0x18, 1]),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[cfg_attr(miri, ignore)]
#[test]
fn direct_view_rejects_input_above_protocol_size_limit() {
    use std::ffi::c_void;

    const PROT_READ: i32 = 1;
    const MAP_PRIVATE: i32 = 0x02;
    const MAP_ANONYMOUS: i32 = 0x20;
    const MAP_NORESERVE: i32 = 0x4000;

    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            fd: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }

    struct Mapping(*mut c_void, usize);
    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: the mapping is owned here and has not been unmapped.
            unsafe { munmap(self.0, self.1) };
        }
    }

    let length = waproto::protobuf::MAX_MESSAGE_BYTES as usize + 1;
    // SAFETY: an anonymous, read-only mapping with MAP_NORESERVE consumes no
    // backing pages until read. The decoder must reject it before reading.
    let address = unsafe {
        mmap(
            std::ptr::null_mut(),
            length,
            PROT_READ,
            MAP_PRIVATE | MAP_ANONYMOUS | MAP_NORESERVE,
            -1,
            0,
        )
    };
    assert_ne!(address as isize, -1, "virtual mapping failed");
    assert!(!address.is_null());
    let mapping = Mapping(address, length);
    // SAFETY: mapping owns this readable address range until after the decode.
    let input = unsafe { std::slice::from_raw_parts(mapping.0.cast::<u8>(), mapping.1) };
    assert!(matches!(
        waproto::whatsapp::MessageView::decode_view(input),
        Err(DecodeError::MessageTooLarge)
    ));
}

#[test]
fn proto_macro_constructs_messages_without_extra_copies() {
    use waproto::prelude::*;

    let image = proto!(wa::message::ImageMessage {
        caption: Some("Hello".into()),
    });
    let image_pointer = image.caption.as_ref().unwrap().as_ptr();
    let message = proto!(wa::Message {
        conversation: Some("Test".to_owned()),
        image_message: image,
    });
    assert_eq!(message.conversation.as_deref(), Some("Test"));
    let image = message.image_message.as_option().unwrap();
    assert_eq!(image.caption.as_deref(), Some("Hello"));
    assert_eq!(image.caption.as_ref().unwrap().as_ptr(), image_pointer);
    assert_eq!(proto!(wa::Message {}), wa::Message::default());
    let changed = proto!(wa::message::ImageMessage {
        caption: Some("Changed".into()),
        ..image.clone()
    });
    assert_eq!(changed.caption.as_deref(), Some("Changed"));
    assert_eq!(image.caption.as_deref(), Some("Hello"));
    assert_eq!(
        proto!(wa::message::ImageMessage { ..changed.clone() }),
        changed
    );
}

#[test]
fn proto_macro_accepts_text_and_existing_field_conversions() {
    use waproto::prelude::*;

    let image = proto!(wa::message::ImageMessage { caption: "Hello" });
    let message = proto!(wa::Message {
        conversation: Some("Old".into()),
        image_message: image,
    });
    assert_eq!(message.conversation.as_deref(), Some("Old"));
    let changed = proto!(wa::Message {
        conversation: "New",
        ..message
    });
    assert_eq!(changed.conversation.as_deref(), Some("New"));
    assert_eq!(
        changed
            .image_message
            .as_option()
            .unwrap()
            .caption
            .as_deref(),
        Some("Hello")
    );
    let cleared = proto!(wa::Message {
        conversation: None,
        ..changed
    });
    assert_eq!(cleared.conversation, None);
    let owned = String::from("Owned");
    let pointer = owned.as_ptr();
    let moved = proto!(wa::Message {
        conversation: owned
    });
    assert_eq!(moved.conversation.as_ref().unwrap().as_ptr(), pointer);
    let text = String::from("Borrowed");
    let borrowed = proto!(wa::Message {
        conversation: &text
    });
    assert_eq!(borrowed.conversation.as_deref(), Some(text.as_str()));
    let key = proto!(wa::MessageKey { from_me: true });
    assert_eq!(key.from_me, Some(true));
}

#[test]
fn proto_macro_update_evaluates_fields_before_base() {
    use std::cell::RefCell;
    use waproto::prelude::*;

    let calls = RefCell::new(Vec::new());
    let _message = proto!(wa::message::ImageMessage {
        caption: {
            calls.borrow_mut().push("field");
            "Hello"
        },
        ..{
            calls.borrow_mut().push("base");
            wa::message::ImageMessage::default()
        }
    });
    assert_eq!(*calls.borrow(), ["field", "base"]);
}

#[test]
fn proto_macro_update_accepts_borrowed_temporaries() {
    use waproto::prelude::*;

    let name = "World";
    let image = wa::message::ImageMessage::default();
    let result = proto!(wa::message::ImageMessage {
        caption: format!("Hello {name}").as_str(),
        ..image
    });
    assert_eq!(result.caption.as_deref(), Some("Hello World"));
}

#[test]
fn proto_macro_update_accepts_a_borrow_from_the_base() {
    use waproto::prelude::*;

    let image = wa::message::ImageMessage::default().with_caption("Hello");
    let image = proto!(wa::message::ImageMessage {
        caption: image.caption.as_deref().unwrap_or_default(),
        ..image
    });
    assert_eq!(image.caption.as_deref(), Some("Hello"));
}

#[test]
fn proto_macro_update_does_not_shadow_the_base() {
    use waproto::prelude::*;

    let caption = wa::message::ImageMessage::default();
    let image = proto!(wa::message::ImageMessage {
        caption: "New",
        ..caption
    });
    assert_eq!(image.caption.as_deref(), Some("New"));
}

#[test]
#[allow(clippy::disallowed_methods)]
fn ordinary_singular_fields_reuse_buffers() {
    use bytes::Buf as _;
    use std::cell::Cell;
    use waproto::protobuf::{DecodeContext, RECURSION_LIMIT};

    let budget = Cell::new(10);
    let context = DecodeContext::new(RECURSION_LIMIT, &budget);
    let mut notification = waproto::whatsapp::message::HistorySyncNotification::default();
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"old");
    let pointer = bytes.as_ptr();
    notification.file_sha256 = Some(bytes);
    let mut input = (&[0x0a, 3, b'n'][..]).chain(&b"ew"[..]);
    notification.merge(&mut input, context).unwrap();
    assert_eq!(notification.file_sha256.as_deref(), Some(&b"new"[..]));
    assert_eq!(notification.file_sha256.as_ref().unwrap().as_ptr(), pointer);

    let mut contact = waproto::whatsapp::sync_action_value::ContactAction::default();
    let mut name = String::with_capacity(64);
    name.push_str("old");
    let pointer = name.as_ptr();
    contact.full_name = Some(name);
    let mut input = (&[0x0a, 3, b'a'][..]).chain(&[0xc3, 0xa9][..]);
    contact.merge(&mut input, context).unwrap();
    assert_eq!(contact.full_name.as_deref(), Some("aé"));
    assert_eq!(contact.full_name.as_ref().unwrap().as_ptr(), pointer);
    let mut input = (&[0x0a, 2, 0xc3][..]).chain(&[0xff][..]);
    assert_eq!(
        contact.merge(&mut input, context),
        Err(DecodeError::InvalidUtf8)
    );
    assert_eq!(contact.full_name.as_deref(), Some(""));
    assert_eq!(contact.full_name.as_ref().unwrap().as_ptr(), pointer);
}

#[test]
#[allow(clippy::disallowed_methods)]
fn required_fields_reuse_buffers() {
    let mut keys = waproto::whatsapp::AddDeviceKeysOutput::default();
    keys.device_public_key = Vec::with_capacity(64);
    keys.device_public_key.extend_from_slice(b"old");
    let pointer = keys.device_public_key.as_ptr();
    keys.merge_from_slice(&[0x0a, 3, b'n', b'e', b'w']).unwrap();
    assert_eq!(keys.device_public_key, b"new");
    assert_eq!(keys.device_public_key.as_ptr(), pointer);

    let mut conversation = waproto::whatsapp::Conversation::default();
    conversation.id = String::with_capacity(64);
    conversation.id.push_str("old");
    let pointer = conversation.id.as_ptr();
    conversation
        .merge_from_slice(&[0x0a, 3, b'n', b'e', b'w'])
        .unwrap();
    assert_eq!(conversation.id, "new");
    assert_eq!(conversation.id.as_ptr(), pointer);
}

#[test]
#[cfg(feature = "protobuf-reference")]
#[allow(clippy::disallowed_methods)]
fn invalid_singular_fields_match_reference_presence() {
    use buffa::Message as _;
    let mut actual = waproto::whatsapp::sync_action_value::ContactAction::default();
    let mut expected = reference::sync_action_value::ContactAction::default();
    assert!(waproto::protobuf::Message::merge_from_slice(&mut actual, &[0x0a, 1, 0xff]).is_err());
    assert!(expected.merge_from_slice(&[0x0a, 1, 0xff]).is_err());
    assert_eq!(actual.full_name, expected.full_name);

    let mut actual = waproto::whatsapp::message::HistorySyncNotification::default();
    let mut expected = reference::message::HistorySyncNotification::default();
    assert!(waproto::protobuf::Message::merge_from_slice(&mut actual, &[0x0a, 2, 0x01]).is_err());
    assert!(expected.merge_from_slice(&[0x0a, 2, 0x01]).is_err());
    assert_eq!(actual.file_sha256, expected.file_sha256);
}

#[test]
#[allow(clippy::disallowed_methods)]
fn sync_action_data_bytes_merge_reuses_buffer() {
    use bytes::Buf as _;
    use std::cell::Cell;
    use waproto::protobuf::{DecodeContext, RECURSION_LIMIT};

    let mut action = waproto::whatsapp::SyncActionData::default();
    let mut index = Vec::with_capacity(64);
    index.extend_from_slice(b"old");
    let pointer = index.as_ptr();
    action.index = Some(index);
    let budget = Cell::new(10);
    let mut input = (&[0x0a, 3, b'n'][..]).chain(&b"ew"[..]);
    action
        .merge(&mut input, DecodeContext::new(RECURSION_LIMIT, &budget))
        .unwrap();
    assert_eq!(action.index.as_deref(), Some(&b"new"[..]));
    assert_eq!(action.index.as_ref().unwrap().as_ptr(), pointer);
    assert_eq!(
        action.merge_from_slice(&[0x0a, 2, 1]),
        Err(DecodeError::UnexpectedEof)
    );
    assert_eq!(action.index.as_deref(), Some(&b"new"[..]));
    let mut empty = waproto::whatsapp::SyncActionData::default();
    assert_eq!(
        empty.merge_from_slice(&[0x0a, 2, 1]),
        Err(DecodeError::UnexpectedEof)
    );
    assert_eq!(empty.index.as_deref(), Some(&[][..]));
}

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
fn unknown_length_delimited_fields_validate_length_and_budget() {
    use waproto::whatsapp::Message;

    let input = [0x82, 0xb5, 0x18, 2, 0x01];
    let options = DecodeOptions::new().with_unknown_field_limit(1);
    assert_eq!(
        options.decode_from_slice::<Message>(&input).unwrap_err(),
        DecodeError::UnexpectedEof
    );
    let zero = DecodeOptions::new().with_unknown_field_limit(0);
    assert_eq!(
        zero.decode_from_slice::<Message>(&input).unwrap_err(),
        DecodeError::UnknownFieldLimitExceeded
    );
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
