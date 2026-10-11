#![allow(clippy::disallowed_methods)]

use std::mem::{align_of, size_of};
use waproto::buffa::{Message, UnknownField, UnknownFieldData, UnknownFields};
use waproto::whatsapp::__unknown_storage::Storage;

fn future() -> UnknownFields {
    let mut nested = UnknownFields::new();
    nested.push(UnknownField {
        number: 202,
        data: UnknownFieldData::LengthDelimited(vec![0x55; 1024]),
    });
    let mut fields = UnknownFields::new();
    fields.push(UnknownField {
        number: 201,
        data: UnknownFieldData::Group(nested),
    });
    fields
}

#[test]
fn storage_keeps_layout_and_owned_unknown_records() {
    assert_eq!(size_of::<Storage>(), size_of::<usize>());
    assert_eq!(align_of::<Storage>(), align_of::<usize>());
    let mut storage = Storage::from(future());
    let cloned = storage.clone();
    storage.clear();
    assert!(storage.is_empty());
    assert!(storage.clone().is_empty());
    let restored = UnknownFields::from(cloned);
    assert_eq!(restored, future());
    let storage = Storage::from(restored);
    let fields: Vec<_> = storage.into_iter().collect();
    assert_eq!(fields, future().into_iter().collect::<Vec<_>>());
}

#[test]
fn empty_retained_capacity_and_replaced_owners_drop_once() {
    for _ in 0..8 {
        let mut storage = Storage::from(future());
        storage.retain(|_| false);
        assert!(storage.is_empty());
        assert_eq!(storage, Storage::default());
        let empty = std::mem::replace(&mut storage, Storage::from(future()));
        drop(empty);
        let owned = UnknownFields::from(std::mem::take(&mut storage));
        assert_eq!(owned, future());
        drop(storage);
        drop(owned);
    }
}

#[test]
fn shared_message_clones_keep_unknown_wire_after_original_is_dropped() {
    fn check<T: Message + Clone>() {
        let mut unknown = UnknownFields::new();
        for (index, data) in [
            UnknownFieldData::Varint(u64::MAX),
            UnknownFieldData::Fixed64(0x1122_3344_5566_7788),
            UnknownFieldData::Fixed32(0xaabb_ccdd),
            UnknownFieldData::LengthDelimited(vec![11, 22, 33, 44]),
            UnknownFieldData::Group(future()),
        ]
        .into_iter()
        .enumerate()
        {
            unknown.push(UnknownField {
                number: 1000 + index as u32,
                data,
            });
        }
        let mut wire = Vec::new();
        unknown.write_to(&mut wire);
        let message = T::decode_from_slice(&wire).unwrap();
        let cloned = message.clone();
        drop(message);
        assert_eq!(cloned.encode_to_vec(), wire);
    }
    check::<waproto::whatsapp::Message>();
    check::<waproto::whatsapp::ContextInfo>();
    check::<waproto::whatsapp::BotMetadata>();
    check::<waproto::whatsapp::MessageContextInfo>();
    check::<waproto::whatsapp::AIRichResponseSubMessage>();
    check::<waproto::whatsapp::SyncActionValue>();
    check::<waproto::whatsapp::WebMessageInfo>();
    check::<waproto::whatsapp::MessageKey>();
    check::<waproto::whatsapp::message::ImageMessage>();
    check::<waproto::whatsapp::message::VideoMessage>();
    check::<waproto::whatsapp::message::InteractiveMessage>();
    check::<waproto::whatsapp::message::HighlyStructuredMessage>();
    check::<waproto::whatsapp::message::ProtocolMessage>();
    check::<waproto::whatsapp::message::ExtendedTextMessage>();
    check::<waproto::whatsapp::message::PeerDataOperationRequestMessage>();
    check::<waproto::whatsapp::message::PeerDataOperationRequestResponseMessage>();
    check::<waproto::whatsapp::message::AudioMessage>();
    check::<waproto::whatsapp::message::StickerMessage>();
    check::<waproto::whatsapp::message::DocumentMessage>();
    check::<waproto::whatsapp::message::ProductMessage>();

    let mut message = waproto::whatsapp::Message::default();
    message.conversation = Some("synthetic clone fixture".into());
    let cloned = message.clone();
    message.conversation.as_mut().unwrap().clear();
    assert_eq!(
        cloned.conversation.as_deref(),
        Some("synthetic clone fixture")
    );
}

#[test]
fn borrowed_journal_clones_drop_independently_and_keep_owned_baselines() {
    use waproto::buffa::{MessageView, ViewEncode};
    use waproto::whatsapp::__wire_order::ViewStorage;
    use waproto::whatsapp::message::interactive_message::{Header, HeaderView, header::Media};

    assert_eq!(size_of::<ViewStorage<'_>>(), size_of::<usize>());
    assert_eq!(align_of::<ViewStorage<'_>>(), align_of::<usize>());
    drop(ViewStorage::default().clone());
    let mut header = Header::default();
    header.media = Some(Media::ImageMessage(Box::new(
        waproto::whatsapp::message::ImageMessage::default()
            .with_caption("synthetic borrowed journal")
            .with_jpeg_thumbnail(vec![0x55; 32]),
    )));
    let known = header.encode_to_vec();
    let mut unknown = Vec::new();
    future().write_to(&mut unknown);
    for wire in [
        [unknown.as_slice(), known.as_slice()].concat(),
        [known.as_slice(), unknown.as_slice()].concat(),
    ] {
        let view = HeaderView::decode_view(&wire).unwrap();
        let expected = view.encode_to_vec();
        let first = view.clone();
        drop(view);
        let owned = first.to_owned_message().unwrap();
        let second = first.clone();
        drop(first);
        assert_eq!(second.encode_to_vec(), expected);
        assert_eq!(owned.encode_to_vec(), expected);
        drop(second);
        assert_eq!(owned.encode_to_vec(), expected);
    }
}
