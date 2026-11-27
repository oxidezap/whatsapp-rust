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
#[allow(clippy::disallowed_methods)]
fn message_writer_observes_current_fields_after_sizing() {
    use waproto::protobuf::{SizeCache, ViewEncode};
    use waproto::whatsapp::{Message, MessageView};

    let mut message = Message::default().with_conversation("hello");
    message.view_once_message = Some(Default::default()).into();
    message.bot_invoke_message = Some(Default::default()).into();
    message.newsletter_scheduled_message = Some(Default::default()).into();
    let original = message.encode_to_vec();
    let mut cache = SizeCache::new();
    let size = message.compute_size(&mut cache);
    message.conversation = Some("world".into());
    let mut output = Vec::new();
    message.write_to(&mut cache, &mut output);
    assert_eq!(size as usize, output.len());
    assert_eq!(output, message.encode_to_vec());

    let mut view = MessageView::decode_view(&original).unwrap();
    let mut cache = SizeCache::new();
    let size = ViewEncode::compute_size(&view, &mut cache);
    view.conversation = Some("world");
    let mut output = Vec::new();
    ViewEncode::write_to(&view, &mut cache, &mut output);
    assert_eq!(size as usize, output.len());
    assert_eq!(output, message.encode_to_vec());
}

#[test]
#[allow(clippy::disallowed_methods)]
fn nested_message_maps_encode_with_consistent_size_cache() {
    use waproto::protobuf::{SizeCache, ViewEncode as _};
    use waproto::whatsapp::{Config, ConfigView, Field};

    let mut config = Config::default();
    for key in [0, 1, 127, 128, u32::MAX] {
        let mut value = Field::default();
        value.min_version = Some(key);
        for child_key in 0..20 {
            let mut child = Field::default();
            child.max_version = Some(child_key);
            value.subfield.insert(child_key, child);
        }
        config.field.insert(key, value);
    }

    let expected = config.encode_to_vec();
    let mut cache = SizeCache::new();
    assert_eq!(config.compute_size(&mut cache) as usize, expected.len());
    let mut encoded = Vec::new();
    config.write_to(&mut cache, &mut encoded);
    assert_eq!(encoded, expected);
    cache.clear();
    assert_eq!(config.compute_size(&mut cache) as usize, expected.len());
    let mut repeated = Vec::new();
    config.write_to(&mut cache, &mut repeated);
    assert_eq!(repeated, expected);
    let view = ConfigView::decode_view(&expected).unwrap();
    assert_eq!(view.encode_to_vec(), expected);
    assert_eq!(view.to_owned_message().unwrap(), config);
    #[cfg(feature = "protobuf-reference")]
    {
        let reference =
            <reference::Config as buffa::Message>::decode_from_slice(&expected).unwrap();
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::to_value(reference).unwrap()
        );
    }
}

#[cfg(feature = "protobuf-reference")]
#[test]
fn map_key_changes_between_passes_match_reference() {
    use waproto::protobuf::{SizeCache, ViewEncode};
    use waproto::whatsapp::{Config, ConfigView, Field};

    for (before, after) in [(1, 128), (128, 1)] {
        let mut initial = Config::default();
        let mut child = Field::default();
        child.min_version = Some(7);
        child.subfield.insert(2, Field::default());
        initial.field.insert(before, child);
        let initial_wire = initial.encode_to_vec();
        let child = initial.field.remove(&before).unwrap();
        initial.field.insert(after, child);
        let changed_wire = initial.encode_to_vec();

        let mut owned = Config::decode_from_slice(&initial_wire).unwrap();
        let mut reference =
            <reference::Config as buffa::Message>::decode_from_slice(&initial_wire).unwrap();
        let mut cache = SizeCache::new();
        let mut reference_cache = buffa::SizeCache::new();
        owned.compute_size(&mut cache);
        buffa::Message::compute_size(&reference, &mut reference_cache);
        owned.field = Config::decode_from_slice(&changed_wire).unwrap().field;
        reference.field = <reference::Config as buffa::Message>::decode_from_slice(&changed_wire)
            .unwrap()
            .field;
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        owned.write_to(&mut cache, &mut actual);
        buffa::Message::write_to(&reference, &mut reference_cache, &mut expected);
        assert_eq!(actual, expected);

        let mut view = ConfigView::decode_view(&initial_wire).unwrap();
        let mut reference_view =
            <reference::ConfigView<'_> as buffa::MessageView>::decode_view(&initial_wire).unwrap();
        let mut cache = SizeCache::new();
        let mut reference_cache = buffa::SizeCache::new();
        ViewEncode::compute_size(&view, &mut cache);
        buffa::ViewEncode::compute_size(&reference_view, &mut reference_cache);
        view.field = ConfigView::decode_view(&changed_wire).unwrap().field;
        reference_view.field =
            <reference::ConfigView<'_> as buffa::MessageView>::decode_view(&changed_wire)
                .unwrap()
                .field;
        let mut actual = Vec::new();
        let mut expected = Vec::new();
        ViewEncode::write_to(&view, &mut cache, &mut actual);
        buffa::ViewEncode::write_to(&reference_view, &mut reference_cache, &mut expected);
        assert_eq!(actual, expected);
    }
}

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

#[test]
fn single_enum_views_preserve_open_and_closed_unknown_values() {
    use waproto::protobuf::EnumValue;
    use waproto::whatsapp::ai_rich_response_dynamic_metadata::AIRichResponseDynamicMetadataType;
    use waproto::whatsapp::{AIRichResponseDynamicMetadataView, SyncdMutationView};
    let options = DecodeOptions::new()
        .with_unknown_field_limit(0)
        .with_element_memory_limit(0);
    let open = options
        .decode_view::<SyncdMutationView<'_>>(&[0x08, 99])
        .unwrap();
    assert_eq!(open.operation, Some(EnumValue::Unknown(99)));
    let mut negative = vec![0x08];
    waproto::protobuf::encoding::encode_varint(u64::MAX, &mut negative);
    let open = options
        .decode_view::<SyncdMutationView<'_>>(&negative)
        .unwrap();
    assert_eq!(open.operation, Some(EnumValue::Unknown(-1)));
    let closed = options
        .decode_view::<AIRichResponseDynamicMetadataView<'_>>(&[0x08, 1])
        .unwrap();
    assert_eq!(
        closed.r#type,
        Some(AIRichResponseDynamicMetadataType::AI_RICH_RESPONSE_DYNAMIC_METADATA_TYPE_IMAGE)
    );
    assert!(matches!(
        options.decode_view::<AIRichResponseDynamicMetadataView<'_>>(&[0x08, 99]),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}

#[test]
fn single_nested_view_preserves_shared_limits_and_error_order() {
    use waproto::whatsapp::AIQueryFanoutView;
    let input = [0x0a, 5, 0x0a, 3, b'a', b'b', b'c'];
    let options = DecodeOptions::new()
        .with_recursion_limit(1)
        .with_unknown_field_limit(0)
        .with_element_memory_limit(0);
    let view = options
        .decode_view::<AIQueryFanoutView<'_>>(&input)
        .unwrap();
    assert_eq!(
        view.message_key.as_option().unwrap().remote_jid,
        Some("abc")
    );
    assert!(matches!(
        DecodeOptions::new()
            .with_recursion_limit(0)
            .decode_view::<AIQueryFanoutView<'_>>(&input),
        Err(DecodeError::RecursionLimitExceeded)
    ));
    assert!(matches!(
        DecodeOptions::new()
            .with_recursion_limit(0)
            .decode_view::<AIQueryFanoutView<'_>>(&[0x0a, 2, 0x0a]),
        Err(DecodeError::RecursionLimitExceeded)
    ));
    assert!(matches!(
        options.decode_view::<AIQueryFanoutView<'_>>(&[0x0a, 4, 0x80, 0xb5, 0x18, 1]),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}

#[test]
fn generated_views_preserve_overlong_tag_boundaries() {
    let input = [0x8a, 0, 1, b'a', 0x8a, 0, 1, b'b'];
    let view = waproto::whatsapp::MessageView::decode_view(&input).unwrap();
    assert_eq!(view.conversation, Some("b"));
    let owned = waproto::codec::message_decode(&input).unwrap();
    assert_eq!(owned.conversation.as_deref(), Some("b"));
}

#[test]
fn generated_and_default_view_decoders_preserve_results_and_budgets() {
    use std::cell::Cell;
    use std::fmt::Debug;
    use waproto::protobuf::DecodeContext;

    fn compare<'a, V: waproto::protobuf::MessageView<'a> + Default + Debug + PartialEq>(
        input: &'a [u8],
        depth: u32,
        unknown_limit: usize,
        element_limit: usize,
    ) {
        let raw_unknown = Cell::new(unknown_limit);
        let raw_elements = Cell::new(element_limit);
        let indexed_unknown = Cell::new(unknown_limit);
        let indexed_elements = Cell::new(element_limit);
        let raw_context =
            DecodeContext::new(depth, &raw_unknown).with_element_memory(&raw_elements);
        let indexed_context =
            DecodeContext::new(depth, &indexed_unknown).with_element_memory(&indexed_elements);
        let raw = V::decode_view_ctx(input, raw_context);
        let indexed = V::decode_view_with_ctx(input, indexed_context);
        assert_eq!(
            indexed, raw,
            "input={input:02x?}, depth={depth}, unknown={unknown_limit}, elements={element_limit}"
        );
        assert_eq!(indexed_unknown.get(), raw_unknown.get());
        assert_eq!(indexed_elements.get(), raw_elements.get());
    }

    for input in [
        &[][..],
        &[0x08, 0x96, 1, 0x22, 3, 1, 0xac, 2][..],
        &[0x08, 0x80][..],
        &[0x0a, 2, 0xff][..],
        &[0x28, 99][..],
        &[0x82, 0xb5, 0x18, 2, 1][..],
        &[0x88, 0, 1][..],
        &[0xab, 0x06, 0x08, 1, 0xac, 0x06][..],
    ] {
        for unknown in [0, 1, 10] {
            for elements in [0, 4, 128] {
                compare::<ADVKeyIndexListView<'_>>(input, 2, unknown, elements);
            }
        }
    }
    for input in [
        &[0x0a, 5, 0x0a, 3, b'a', b'b', b'c'][..],
        &[0x0a, 2, 0x0a][..],
    ] {
        for depth in [0, 1, 2] {
            compare::<waproto::whatsapp::AIQueryFanoutView<'_>>(input, depth, 1, 128);
        }
    }
    for input in [
        &[0x0a, 4, 0x08, 1, 0x12, 0][..],
        &[0x0a, 4, 0x08, 1, 0x12][..],
        &[0x0a, 2, 0x18, 0][..],
    ] {
        for depth in [0, 1, 2] {
            for unknown in [0, 1] {
                for elements in [0, 128] {
                    compare::<waproto::whatsapp::ConfigView<'_>>(input, depth, unknown, elements);
                }
            }
        }
    }
    let mut state = 0x96a3_5f27_31dc_b40eu64;
    for length in 0..=24 {
        for _ in 0..256 {
            let input: Vec<u8> = (0..length)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            compare::<ADVKeyIndexListView<'_>>(&input, 2, 1, 64);
            compare::<waproto::whatsapp::MessageView<'_>>(&input, 2, 1, 64);
            compare::<waproto::whatsapp::ConfigView<'_>>(&input, 2, 1, 64);
        }
    }
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
fn packed_view_preserves_partial_values_and_budget() {
    use std::cell::Cell;
    use waproto::protobuf::{
        DecodeContext,
        encoding::{Tag, WireType},
    };

    let payloads: &[&[u8]] = &[
        &[1, 0xac, 2, 3],
        &[1, 0x80],
        &[
            1, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 2, 3,
        ],
        &[1, 0x80, 0x80, 1, 2],
        &[0x80, 0x80, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        &[0x80, 0x80, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0x80],
        &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 2],
    ];
    for payload in payloads {
        let mut wire = vec![payload.len() as u8];
        wire.extend_from_slice(payload);
        for budget in [0, 4, 8, 16, 64] {
            let unknown_a = Cell::new(10);
            let unknown_b = Cell::new(10);
            let elements_a = Cell::new(budget);
            let elements_b = Cell::new(budget);
            let ctx_a = DecodeContext::new(10, &unknown_a).with_element_memory(&elements_a);
            let ctx_b = DecodeContext::new(10, &unknown_b).with_element_memory(&elements_b);
            let tag = Tag::new(4, WireType::LengthDelimited);
            let mut owned = ADVKeyIndexList::default();
            let mut view = ADVKeyIndexListView::default();
            let mut input = &wire[..];
            let a = owned.merge_field(tag, &mut input, ctx_a);
            let b = view.merge_view_field(tag, &wire, &wire, ctx_b).map(|_| ());
            assert_eq!(a, b, "payload={payload:?}, budget={budget}");
            assert_eq!(&owned.valid_indexes, &*view.valid_indexes);
            assert_eq!(elements_a.get(), elements_b.get());
            assert_eq!(unknown_a.get(), unknown_b.get());
        }
    }
}

#[test]
fn packed_view_preserves_values_across_block_boundaries_and_errors() {
    use std::cell::Cell;
    use waproto::protobuf::{
        DecodeContext,
        encoding::{Tag, WireType, encode_varint},
    };

    for count in [0usize, 1, 15, 16, 17, 31, 32, 33, 63, 64, 65, 127, 128, 129] {
        for suffix in [
            &[][..],
            &[0x80][..],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 2][..],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 1][..],
            &[
                0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0,
            ][..],
        ] {
            let mut payload = vec![1; count];
            payload.extend_from_slice(suffix);
            let mut wire = Vec::new();
            encode_varint(payload.len() as u64, &mut wire);
            wire.extend_from_slice(&payload);
            for budget in [
                0usize, 4, 60, 64, 68, 128, 252, 255, 256, 257, 260, 512, 1024,
            ] {
                let unknown_a = Cell::new(10);
                let unknown_b = Cell::new(10);
                let elements_a = Cell::new(budget);
                let elements_b = Cell::new(budget);
                let ctx_a = DecodeContext::new(10, &unknown_a).with_element_memory(&elements_a);
                let ctx_b = DecodeContext::new(10, &unknown_b).with_element_memory(&elements_b);
                let tag = Tag::new(4, WireType::LengthDelimited);
                let mut owned = ADVKeyIndexList::default();
                let mut view = ADVKeyIndexListView::default();
                owned.valid_indexes.push(9);
                view.valid_indexes.push(9);
                for _ in 0..2 {
                    let mut input = &wire[..];
                    let a = owned.merge_field(tag, &mut input, ctx_a);
                    let b = view.merge_view_field(tag, &wire, &wire, ctx_b).map(|_| ());
                    assert_eq!(a, b, "count={count}, suffix={suffix:?}, budget={budget}");
                    assert_eq!(&owned.valid_indexes, &*view.valid_indexes);
                    assert_eq!(elements_a.get(), elements_b.get());
                    assert_eq!(unknown_a.get(), unknown_b.get());
                    if a.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

#[test]
fn packed_owned_buffer_observes_budget_after_each_advance() {
    use std::cell::Cell;
    use waproto::protobuf::{
        DecodeContext,
        bytes::Buf,
        encoding::{Tag, WireType, decode_varint, encode_varint},
    };

    struct ObservingBuf<'a> {
        input: &'a [u8],
        chunk_size: usize,
        budget: &'a Cell<usize>,
        advances: Vec<(usize, usize)>,
    }
    impl Buf for ObservingBuf<'_> {
        fn remaining(&self) -> usize {
            self.input.len()
        }
        fn chunk(&self) -> &[u8] {
            &self.input[..self.input.len().min(self.chunk_size)]
        }
        fn advance(&mut self, count: usize) {
            self.advances.push((count, self.budget.get()));
            self.input = &self.input[count..];
        }
    }
    fn scalar(
        input: &mut impl Buf,
        output: &mut Vec<u32>,
        context: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        let length =
            usize::try_from(decode_varint(input)?).map_err(|_| DecodeError::MessageTooLarge)?;
        if length > input.remaining() {
            return Err(DecodeError::UnexpectedEof);
        }
        let mut packed = input.take(length);
        while packed.has_remaining() {
            let value = decode_varint(&mut packed)? as u32;
            context.register_element_memory(size_of::<u32>())?;
            output.push(value);
        }
        Ok(())
    }

    for count in [0usize, 1, 63, 64, 65, 128] {
        for suffix in [
            &[][..],
            &[0x80][..],
            &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 2][..],
            &[0xac, 2][..],
        ] {
            let mut payload = vec![1; count];
            payload.extend_from_slice(suffix);
            let mut wire = Vec::new();
            encode_varint(payload.len() as u64, &mut wire);
            wire.extend_from_slice(&payload);
            wire.push(0x55);
            for budget in [0, 4, 252, 255, 256, 260, 512, 1024] {
                for chunk_size in [1, 2, 7, 64, 1024] {
                    let actual_budget = Cell::new(budget);
                    let reference_budget = Cell::new(budget);
                    let actual_unknown = Cell::new(10);
                    let reference_unknown = Cell::new(10);
                    let mut actual_input = ObservingBuf {
                        input: &wire,
                        chunk_size,
                        budget: &actual_budget,
                        advances: Vec::new(),
                    };
                    let mut reference_input = ObservingBuf {
                        input: &wire,
                        chunk_size,
                        budget: &reference_budget,
                        advances: Vec::new(),
                    };
                    let actual_context =
                        DecodeContext::new(10, &actual_unknown).with_element_memory(&actual_budget);
                    let reference_context = DecodeContext::new(10, &reference_unknown)
                        .with_element_memory(&reference_budget);
                    let mut actual = ADVKeyIndexList::default();
                    actual.valid_indexes.push(9);
                    let mut expected = vec![9];
                    let result = actual.merge_field(
                        Tag::new(4, WireType::LengthDelimited),
                        &mut actual_input,
                        actual_context,
                    );
                    let reference = scalar(&mut reference_input, &mut expected, reference_context);
                    assert_eq!(result, reference);
                    assert_eq!(actual.valid_indexes, expected);
                    assert_eq!(actual_budget.get(), reference_budget.get());
                    assert_eq!(actual_input.input, reference_input.input);
                    assert_eq!(actual_input.advances, reference_input.advances);
                }
            }
        }
    }
}

#[test]
fn packed_u32_blocks_match_owned_partial_state() {
    use std::cell::Cell;
    use waproto::protobuf::{
        DecodeContext,
        encoding::{Tag, WireType},
    };

    let mut payloads = Vec::new();
    for len in [0, 1, 7, 8, 9, 15, 16, 17, 31, 32, 33, 64, 256] {
        let small = vec![1; len];
        payloads.push(small.clone());
        for pos in [0, len / 2, len.saturating_sub(1)] {
            if pos < len {
                let mut mixed = small.clone();
                mixed.splice(pos..=pos, [0xac, 2]);
                payloads.push(mixed);
                let mut truncated = small.clone();
                truncated[pos] = 0x80;
                truncated.truncate(pos + 1);
                payloads.push(truncated);
                let mut overflow = small.clone();
                overflow.splice(pos..=pos, [0x80; 10].into_iter().chain([2]));
                payloads.push(overflow);
            }
        }
    }

    for payload in payloads {
        let mut wire = Vec::new();
        waproto::protobuf::encoding::encode_varint(payload.len() as u64, &mut wire);
        wire.extend_from_slice(&payload);
        wire.extend_from_slice(&[0x55, 0x44]);
        for budget in [
            0, 1, 4, 7, 16, 28, 31, 32, 33, 60, 64, 128, 252, 255, 256, 257, 260, 1024, 4096,
        ] {
            for initial in [0, 3, 9, 63, 64, 65, 127, 128, 129] {
                let unknown_a = Cell::new(10);
                let unknown_b = Cell::new(10);
                let elements_a = Cell::new(budget);
                let elements_b = Cell::new(budget);
                let ctx_a = DecodeContext::new(10, &unknown_a).with_element_memory(&elements_a);
                let ctx_b = DecodeContext::new(10, &unknown_b).with_element_memory(&elements_b);
                let tag = Tag::new(4, WireType::LengthDelimited);
                let mut owned = ADVKeyIndexList::default();
                owned.valid_indexes = vec![99; initial];
                let mut view = ADVKeyIndexListView::default();
                view.valid_indexes = vec![99; initial].into();
                let mut input = &wire[..];
                let a = owned.merge_field(tag, &mut input, ctx_a);
                let b = view.merge_view_field(tag, &wire, &wire, ctx_b);
                assert_eq!(a, b.as_ref().map(|_| ()).map_err(Clone::clone));
                if let Ok(rest) = b {
                    assert_eq!(input, rest);
                }
                assert_eq!(&owned.valid_indexes, &*view.valid_indexes);
                assert_eq!(elements_a.get(), elements_b.get());
                assert_eq!(unknown_a.get(), unknown_b.get());
            }
        }
    }
}

#[test]
fn packed_long_payload_preserves_error_order_and_limits() {
    let mut input = vec![0x22, 33];
    input.extend_from_slice(&[1; 32]);
    input.push(0x80);
    for budget in [0, 4, 64, 128, 132] {
        let options = DecodeOptions::new().with_element_memory_limit(budget);
        let expected = if budget < 128 {
            DecodeError::ElementMemoryLimitExceeded
        } else {
            DecodeError::UnexpectedEof
        };
        assert_eq!(
            options
                .decode_from_slice::<ADVKeyIndexList>(&input)
                .unwrap_err(),
            expected
        );
        assert_eq!(
            options
                .decode_view::<ADVKeyIndexListView<'_>>(&input)
                .unwrap_err(),
            expected
        );
    }
    *input.last_mut().unwrap() = 1;
    let options = DecodeOptions::new().with_element_memory_limit(132);
    assert_eq!(
        options
            .decode_from_slice::<ADVKeyIndexList>(&input)
            .unwrap()
            .valid_indexes
            .len(),
        33
    );
    assert_eq!(
        options
            .decode_view::<ADVKeyIndexListView<'_>>(&input)
            .unwrap()
            .valid_indexes
            .len(),
        33
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
