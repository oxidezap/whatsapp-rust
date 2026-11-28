#![allow(clippy::disallowed_methods)]

use divan::black_box;
use waproto::protobuf::{Message as _, MessageView as _, ViewEncode as _};
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

fn message_wire(shape: &str) -> Vec<u8> {
    let message = match shape {
        "text" => wa::Message::default().with_conversation("synthetic text"),
        "text_context" => {
            let mut message = wa::Message::default().with_conversation("synthetic text");
            let mut context = wa::MessageContextInfo::default();
            context.message_secret = Some(vec![0x5a; 32]);
            message.message_context_info = Some(context).into();
            message
        }
        "oneof" => {
            let image = wa::message::ImageMessage::default().with_caption("synthetic image");
            let mut header = wa::message::interactive_message::Header::default();
            header.media = Some(
                wa::message::interactive_message::header::Media::ImageMessage(Box::new(image)),
            );
            let mut interactive = wa::message::InteractiveMessage::default();
            interactive.header = Some(header).into();
            let mut message = wa::Message::default();
            message.interactive_message = Some(interactive).into();
            message
        }
        "media" => {
            let mut message = wa::Message::default();
            message.image_message = Some(
                wa::message::ImageMessage::default()
                    .with_caption("synthetic caption")
                    .with_jpeg_thumbnail(vec![0x5a; 1024]),
            )
            .into();
            message
        }
        _ => unreachable!("declared benchmark shape"),
    };
    message.encode_to_vec()
}

fn sync_wire() -> Vec<u8> {
    let index = b"[\"star\",\"fictitious\",\"message-id\",\"1\",\"0\"]";
    let mut wire = vec![0x0a, index.len() as u8];
    wire.extend_from_slice(index);
    wire.extend_from_slice(&[
        0x12, 0x0a, 0x08, 0x80, 0xe2, 0xcf, 0xaa, 0x06, 0x12, 0x02, 0x08, 0x01, 0x1a, 0x00, 0x20,
        0x01,
    ]);
    let current = wa::SyncActionData::decode_from_slice(&wire).expect("valid sync fixture");
    assert_eq!(current.encode_to_vec(), wire);
    wire
}

fn packed_wire() -> Vec<u8> {
    let mut value = wa::ADVKeyIndexList::default();
    value.raw_id = Some(7);
    value.valid_indexes = (0..256).collect();
    value.encode_to_vec()
}

fn map_wire() -> Vec<u8> {
    use waproto::protobuf::encoding::{put_bytes_field, put_varint};
    let mut wire = Vec::new();
    for number in 0..32_u64 {
        let mut value = Vec::new();
        put_varint(8, &mut value);
        put_varint(number + 1, &mut value);
        let mut entry = Vec::new();
        put_varint(8, &mut entry);
        put_varint(number, &mut entry);
        put_bytes_field(2, &value, &mut entry);
        put_bytes_field(1, &entry, &mut wire);
    }
    let value = wa::Config::decode_from_slice(&wire).expect("valid map fixture");
    assert_eq!(value.field.len(), 32);
    for number in 0..32 {
        assert_eq!(value.field[&number].min_version, Some(number + 1));
    }
    let view = wa::ConfigView::decode_view(&wire).expect("valid map view fixture");
    assert_eq!(view.encode_to_vec(), wire);
    wire
}

fn history_wire() -> Vec<u8> {
    let mut value = wa::HistorySync::default();
    for number in 0..64 {
        let mut conversation = wa::Conversation::default();
        conversation.id = format!("fictitious-{number}");
        conversation.name = Some(format!("conversation-{number}"));
        conversation.unread_count = Some(number);
        value.conversations.push(conversation);
    }
    value.encode_to_vec()
}

fn oneof_wire() -> Vec<u8> {
    let image = wa::message::ImageMessage::default()
        .with_caption("synthetic image")
        .with_jpeg_thumbnail(vec![0x5a; 128]);
    let mut header = wa::message::interactive_message::Header::default();
    header.media =
        Some(wa::message::interactive_message::header::Media::ImageMessage(Box::new(image)));
    header.encode_to_vec()
}

fn unknown_wire() -> Vec<u8> {
    let mut wire = message_wire("text");
    wire.extend_from_slice(&[0xc2, 0x3e, 4, 11, 22, 33, 44]);
    wire
}

macro_rules! codec_case {
    ($module:ident, $model:ty, $view:ty, $fixture:ident, $owned_encode:ident, $view_encode:ident) => {
        mod $module {
            use super::*;

            fn wire() -> Vec<u8> {
                let wire = $fixture();
                <$model as waproto::protobuf::Message>::decode_from_slice(&wire)
                    .expect("valid owned fixture");
                <$view as waproto::protobuf::MessageView>::decode_view(&wire)
                    .expect("valid view fixture");
                wire
            }

            #[divan::bench]
            fn owned_sdk(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$model as waproto::protobuf::Message>::decode_from_slice(black_box(&wire))
                            .expect("valid owned fixture"),
                    )
                });
            }

            #[divan::bench]
            fn view_sdk(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$view as waproto::protobuf::MessageView>::decode_view(black_box(&wire))
                            .expect("valid view fixture"),
                    )
                });
            }

            codec_case!(@view_encode $view_encode, $view);

            codec_case!(@owned_encode $owned_encode, $model);
        }
    };
    (@view_encode yes, $view:ty) => {
        #[divan::bench]
        fn encode_view_sdk(bencher: divan::Bencher) {
            let wire = wire();
            let value = <$view as waproto::protobuf::MessageView>::decode_view(&wire)
                .expect("valid view fixture");
            bencher.bench(|| {
                black_box(waproto::protobuf::ViewEncode::encode_to_vec(black_box(&value)))
            });
        }
    };
    (@view_encode no, $view:ty) => {};
    (@owned_encode yes, $model:ty) => {
        #[divan::bench]
        fn encode_owned_sdk(bencher: divan::Bencher) {
            let wire = wire();
            let value = <$model as waproto::protobuf::Message>::decode_from_slice(&wire)
                .expect("valid owned fixture");
            bencher.bench(|| {
                black_box(waproto::protobuf::Message::encode_to_vec(black_box(&value)))
            });
        }
    };
    (@owned_encode no, $model:ty) => {};
}

codec_case!(
    packed,
    wa::ADVKeyIndexList,
    wa::ADVKeyIndexListView<'_>,
    packed_wire,
    yes,
    yes
);
codec_case!(map, wa::Config, wa::ConfigView<'_>, map_wire, no, no);
codec_case!(
    history,
    wa::HistorySync,
    wa::HistorySyncView<'_>,
    history_wire,
    yes,
    yes
);
codec_case!(
    oneof,
    wa::message::interactive_message::Header,
    wa::message::interactive_message::HeaderView<'_>,
    oneof_wire,
    yes,
    yes
);
codec_case!(
    unknown,
    wa::Message,
    wa::MessageView<'_>,
    unknown_wire,
    yes,
    yes
);

mod fragmented {
    use super::*;
    use bytes::Buf as _;

    #[divan::bench]
    fn owned_sdk(bencher: divan::Bencher) {
        let mut wire = vec![0x0a, 0x8e, 0];
        wire.extend_from_slice(b"synthetic text");
        wa::Message::decode_from_slice(&wire).expect("valid fragmented fixture");
        bencher.bench(|| {
            let mut input = wire[..2].chain(&wire[2..]);
            black_box(wa::Message::decode(black_box(&mut input)).expect("valid fragmented fixture"))
        });
    }
}

#[divan::bench(args = ["text", "text_context", "media", "oneof"])]
fn message_owned_sdk(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(wa::Message::decode_from_slice(black_box(&wire)).expect("valid message fixture"))
    });
}

#[divan::bench(args = ["text", "text_context", "media"])]
fn message_view_sdk(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(wa::MessageView::decode_view(black_box(&wire)).expect("valid message fixture"))
    });
}

#[divan::bench(args = ["text", "text_context", "media", "oneof"])]
fn message_encode_sdk(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    let message = wa::Message::decode_from_slice(&wire).expect("valid message fixture");
    bencher.bench(|| black_box(black_box(&message).encode_to_vec()));
}

#[divan::bench]
fn sync_owned_sdk(bencher: divan::Bencher) {
    let wire = sync_wire();
    bencher.bench(|| {
        black_box(
            wa::SyncActionData::decode_from_slice(black_box(&wire)).expect("valid sync fixture"),
        )
    });
}

#[divan::bench]
fn sync_view_sdk(bencher: divan::Bencher) {
    let wire = sync_wire();
    let view = wa::SyncActionDataView::decode_view(&wire).expect("valid sync fixture");
    assert_eq!(view.encode_to_vec(), wire);
    bencher.bench(|| {
        black_box(
            wa::SyncActionDataView::decode_view(black_box(&wire)).expect("valid sync fixture"),
        )
    });
}
