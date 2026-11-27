#![allow(clippy::disallowed_methods)]

#[path = "../tests/support/reference.rs"]
mod reference;
#[path = "../tests/support/reference_enum_serde.rs"]
mod reference_enum_serde;

use divan::black_box;
use waproto::protobuf::{Message as _, MessageView as _, ViewEncode as _};
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

fn message_wire(shape: &str) -> Vec<u8> {
    let message = match shape {
        "text" => wa::Message::default().with_conversation("synthetic text"),
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
        _ => unreachable!(),
    };
    let wire = message.encode_to_vec();
    let old = <reference::Message as buffa::Message>::decode_from_slice(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    assert_eq!(buffa::Message::encode_to_vec(&old), wire);
    wire
}

fn sync_wire() -> Vec<u8> {
    let index = b"[\"star\",\"fictitious\",\"message-id\",\"1\",\"0\"]";
    let mut wire = vec![0x0a, index.len() as u8];
    wire.extend_from_slice(index);
    wire.extend_from_slice(&[
        0x12, 0x0a, 0x08, 0x80, 0xe2, 0xcf, 0xaa, 0x06, 0x12, 0x02, 0x08, 0x01, 0x1a, 0x00, 0x20,
        0x01,
    ]);
    let current = wa::SyncActionData::decode_from_slice(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    let old = <reference::SyncActionData as buffa::Message>::decode_from_slice(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    assert_eq!(current.encode_to_vec(), buffa::Message::encode_to_vec(&old));
    wire
}

fn packed_wire() -> Vec<u8> {
    let mut value = wa::ADVKeyIndexList::default();
    value.raw_id = Some(7);
    value.valid_indexes = (0..256).collect();
    value.encode_to_vec()
}

mod packed_sizes {
    use super::*;

    fn wire(case: &str) -> Vec<u8> {
        let (count, pattern) = case
            .split_once('-')
            .expect("benchmark case must contain a count and pattern");
        let count: u32 = count.parse().expect("benchmark count must be numeric");
        let mut value = wa::ADVKeyIndexList::default();
        value.valid_indexes = (0..count)
            .map(|index| match pattern {
                "small" => index % 128,
                "mixed" => index,
                "large" => u32::MAX - index,
                "three" => 16_384 + index,
                "first3" => {
                    if index == 0 {
                        16_384
                    } else {
                        1
                    }
                }
                "middle3" => {
                    if index == count / 2 {
                        16_384
                    } else {
                        1
                    }
                }
                "last3" => {
                    if index + 1 == count {
                        16_384
                    } else {
                        1
                    }
                }
                "alternate3" => {
                    if index % 2 == 0 {
                        16_384
                    } else {
                        1
                    }
                }
                "first5" => {
                    if index == 0 {
                        u32::MAX
                    } else {
                        1
                    }
                }
                "firstshort" => {
                    if index == 0 {
                        1
                    } else {
                        u32::MAX
                    }
                }
                "ten" | "firstten" => {
                    if pattern == "ten" || index == 0 {
                        u32::MAX
                    } else {
                        1
                    }
                }
                "first" => {
                    if index == 0 {
                        128
                    } else {
                        1
                    }
                }
                "middle" => {
                    if index == count / 2 {
                        128
                    } else {
                        1
                    }
                }
                "last" => {
                    if index + 1 == count {
                        128
                    } else {
                        1
                    }
                }
                _ => unreachable!(),
            })
            .collect();
        let wire = if matches!(pattern, "ten" | "firstten") {
            let mut payload = Vec::new();
            for index in 0..count {
                let n = if pattern == "ten" || index == 0 {
                    u64::MAX
                } else {
                    1
                };
                waproto::protobuf::encoding::encode_varint(n, &mut payload);
            }
            let mut wire = vec![0x22];
            waproto::protobuf::encoding::encode_varint(payload.len() as u64, &mut wire);
            wire.extend_from_slice(&payload);
            wire
        } else {
            value.encode_to_vec()
        };
        let reference = <reference::ADVKeyIndexList as buffa::Message>::decode_from_slice(&wire)
            .expect("synthetic protobuf benchmark fixture must be valid");
        assert_eq!(value.valid_indexes, reference.valid_indexes);
        wire
    }

    macro_rules! bench {
        ($name:ident, $ty:ty, $trait:path, $decode:ident) => {
            #[divan::bench(args = ["0-small", "1-small", "4-small", "8-small", "16-small", "32-small", "256-small", "256-mixed", "256-large", "256-first", "256-middle", "256-last", "256-three", "256-first3", "256-middle3", "256-last3", "256-alternate3", "256-first5", "256-firstshort", "256-ten", "256-firstten", "4096-mixed", "4096-first", "4096-middle", "4096-last"])]
            fn $name(bencher: divan::Bencher, case: &str) {
                let wire = wire(case);
                bencher.bench(|| black_box(<$ty as $trait>::$decode(black_box(&wire)).expect("synthetic protobuf benchmark fixture must be valid")));
            }
        };
    }

    bench!(
        owned_sdk,
        wa::ADVKeyIndexList,
        waproto::protobuf::Message,
        decode_from_slice
    );
    bench!(
        owned_buffa,
        reference::ADVKeyIndexList,
        buffa::Message,
        decode_from_slice
    );
    bench!(
        view_sdk,
        wa::ADVKeyIndexListView<'_>,
        waproto::protobuf::MessageView,
        decode_view
    );
    bench!(
        view_buffa,
        reference::ADVKeyIndexListView<'_>,
        buffa::MessageView,
        decode_view
    );
}

fn map_wire() -> Vec<u8> {
    let mut value = wa::Config::default();
    for number in 0..32 {
        let mut field = wa::Field::default();
        field.min_version = Some(number + 1);
        value.field.insert(number, field);
    }
    value.encode_to_vec()
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

macro_rules! codec_pair {
    ($module:ident, $model:ty, $view:ty, $reference:ty, $reference_view:ty, $fixture:ident) => {
        mod $module {
            use super::*;
            fn wire() -> Vec<u8> {
                let wire = $fixture();
                let current = <$model as waproto::protobuf::Message>::decode_from_slice(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                let reference = <$reference as buffa::Message>::decode_from_slice(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                assert_eq!(
                    serde_json::to_value(&current)
                        .expect("synthetic protobuf benchmark fixture must be valid"),
                    serde_json::to_value(&reference)
                        .expect("synthetic protobuf benchmark fixture must be valid")
                );
                wire
            }
            #[divan::bench]
            fn owned_sdk(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$model as waproto::protobuf::Message>::decode_from_slice(black_box(&wire))
                            .expect("synthetic protobuf benchmark fixture must be valid"),
                    )
                });
            }
            #[divan::bench]
            fn owned_buffa(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$reference as buffa::Message>::decode_from_slice(black_box(&wire))
                            .expect("synthetic protobuf benchmark fixture must be valid"),
                    )
                });
            }
            #[divan::bench]
            fn view_sdk(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$view as waproto::protobuf::MessageView>::decode_view(black_box(&wire))
                            .expect("synthetic protobuf benchmark fixture must be valid"),
                    )
                });
            }
            #[divan::bench]
            fn view_buffa(bencher: divan::Bencher) {
                let wire = wire();
                bencher.bench(|| {
                    black_box(
                        <$reference_view as buffa::MessageView>::decode_view(black_box(&wire))
                            .expect("synthetic protobuf benchmark fixture must be valid"),
                    )
                });
            }
            #[divan::bench]
            fn encode_owned_sdk(bencher: divan::Bencher) {
                let wire = wire();
                let value = <$model as waproto::protobuf::Message>::decode_from_slice(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                bencher.bench(|| {
                    black_box(waproto::protobuf::Message::encode_to_vec(black_box(&value)))
                });
            }
            #[divan::bench]
            fn encode_owned_buffa(bencher: divan::Bencher) {
                let wire = wire();
                let value = <$reference as buffa::Message>::decode_from_slice(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                bencher.bench(|| black_box(buffa::Message::encode_to_vec(black_box(&value))));
            }
            #[divan::bench]
            fn encode_view_sdk(bencher: divan::Bencher) {
                let wire = wire();
                let value = <$view as waproto::protobuf::MessageView>::decode_view(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                bencher.bench(|| {
                    black_box(waproto::protobuf::ViewEncode::encode_to_vec(black_box(
                        &value,
                    )))
                });
            }
            #[divan::bench]
            fn encode_view_buffa(bencher: divan::Bencher) {
                let wire = wire();
                let value = <$reference_view as buffa::MessageView>::decode_view(&wire)
                    .expect("synthetic protobuf benchmark fixture must be valid");
                bencher.bench(|| black_box(buffa::ViewEncode::encode_to_vec(black_box(&value))));
            }
        }
    };
}

codec_pair!(
    packed,
    wa::ADVKeyIndexList,
    wa::ADVKeyIndexListView<'_>,
    reference::ADVKeyIndexList,
    reference::ADVKeyIndexListView<'_>,
    packed_wire
);
codec_pair!(
    map,
    wa::Config,
    wa::ConfigView<'_>,
    reference::Config,
    reference::ConfigView<'_>,
    map_wire
);
codec_pair!(
    history,
    wa::HistorySync,
    wa::HistorySyncView<'_>,
    reference::HistorySync,
    reference::HistorySyncView<'_>,
    history_wire
);
codec_pair!(
    oneof,
    wa::message::interactive_message::Header,
    wa::message::interactive_message::HeaderView<'_>,
    reference::message::interactive_message::Header,
    reference::message::interactive_message::HeaderView<'_>,
    oneof_wire
);
codec_pair!(
    unknown,
    wa::Message,
    wa::MessageView<'_>,
    reference::Message,
    reference::MessageView<'_>,
    unknown_wire
);

mod fragmented {
    use super::*;
    use bytes::Buf as _;

    fn wire() -> Vec<u8> {
        let mut wire = vec![0x0a, 0x8e, 0];
        wire.extend_from_slice(b"synthetic text");
        let current = wa::Message::decode_from_slice(&wire)
            .expect("synthetic protobuf benchmark fixture must be valid");
        let reference = <reference::Message as buffa::Message>::decode_from_slice(&wire)
            .expect("synthetic protobuf benchmark fixture must be valid");
        assert_eq!(
            serde_json::to_value(current)
                .expect("synthetic protobuf benchmark fixture must be valid"),
            serde_json::to_value(reference)
                .expect("synthetic protobuf benchmark fixture must be valid")
        );
        wire
    }

    #[divan::bench]
    fn owned_sdk(bencher: divan::Bencher) {
        let wire = wire();
        bencher.bench(|| {
            let mut input = wire[..2].chain(&wire[2..]);
            black_box(
                wa::Message::decode(black_box(&mut input))
                    .expect("synthetic protobuf benchmark fixture must be valid"),
            )
        });
    }

    #[divan::bench]
    fn owned_buffa(bencher: divan::Bencher) {
        let wire = wire();
        bencher.bench(|| {
            let mut input = wire[..2].chain(&wire[2..]);
            black_box(
                <reference::Message as buffa::Message>::decode(black_box(&mut input))
                    .expect("synthetic protobuf benchmark fixture must be valid"),
            )
        });
    }
}

#[divan::bench(args = ["text", "media"])]
fn message_owned_zero(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(
            wa::Message::decode_from_slice(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench(args = ["text", "media"])]
fn message_owned_buffa(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(
            <reference::Message as buffa::Message>::decode_from_slice(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench(args = ["text", "media"])]
fn message_view_zero(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(
            wa::MessageView::decode_view(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench(args = ["text", "media"])]
fn message_view_buffa(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    bencher.bench(|| {
        black_box(
            <reference::MessageView<'_> as buffa::MessageView>::decode_view(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench(args = ["text", "media"])]
fn message_encode_zero(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    let message = wa::Message::decode_from_slice(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    bencher.bench(|| black_box(black_box(&message).encode_to_vec()));
}

#[divan::bench(args = ["text", "media"])]
fn message_encode_buffa(bencher: divan::Bencher, shape: &str) {
    let wire = message_wire(shape);
    let message = <reference::Message as buffa::Message>::decode_from_slice(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    bencher.bench(|| black_box(buffa::Message::encode_to_vec(black_box(&message))));
}

#[divan::bench]
fn sync_owned_zero(bencher: divan::Bencher) {
    let wire = sync_wire();
    bencher.bench(|| {
        black_box(
            wa::SyncActionData::decode_from_slice(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench]
fn sync_owned_buffa(bencher: divan::Bencher) {
    let wire = sync_wire();
    bencher.bench(|| {
        black_box(
            <reference::SyncActionData as buffa::Message>::decode_from_slice(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench]
fn sync_view_zero(bencher: divan::Bencher) {
    let wire = sync_wire();
    let view = wa::SyncActionDataView::decode_view(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    assert_eq!(
        view.encode_to_vec(),
        wa::SyncActionData::decode_from_slice(&wire)
            .expect("synthetic protobuf benchmark fixture must be valid")
            .encode_to_vec()
    );
    bencher.bench(|| {
        black_box(
            wa::SyncActionDataView::decode_view(black_box(&wire))
                .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}

#[divan::bench]
fn sync_view_buffa(bencher: divan::Bencher) {
    let wire = sync_wire();
    let view = <reference::SyncActionDataView<'_> as buffa::MessageView>::decode_view(&wire)
        .expect("synthetic protobuf benchmark fixture must be valid");
    assert_eq!(
        buffa::ViewEncode::encode_to_vec(&view),
        buffa::Message::encode_to_vec(
            &<reference::SyncActionData as buffa::Message>::decode_from_slice(&wire)
                .expect("synthetic protobuf benchmark fixture must be valid")
        )
    );
    bencher.bench(|| {
        black_box(
            <reference::SyncActionDataView<'_> as buffa::MessageView>::decode_view(black_box(
                &wire,
            ))
            .expect("synthetic protobuf benchmark fixture must be valid"),
        )
    });
}
