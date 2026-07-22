//! Realistic history-sync ingest: zlib inflate + full protobuf scan of a
//! mid-size InitialBootstrap (500 conversations x 40 messages, multi-MB
//! decompressed). This is the heaviest single-shot pipeline in the library
//! and the hottest consumer of the varint scan.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use buffa::Message;
use bytes::Bytes;
use divan::black_box;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use std::sync::OnceLock;
use waproto::whatsapp as wa;

const HISTORY_CONVERSATIONS: usize = 500;
const MESSAGES_PER_CONVERSATION: usize = 40;

fn main() {
    divan::main();
}

/// Deterministic xorshift-based filler: real chat text compresses ~2-4x;
/// repeated literal filler compressed ~24x and masked the inflate cost.
fn pseudo_text(mut seed: u64, len: usize) -> String {
    seed = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15).max(1);
    let mut out = String::with_capacity(len + 17);
    while out.len() < len {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        out.push_str(&format!("{seed:016x} "));
    }
    out.truncate(len);
    out
}

fn build_realistic_history_sync(n_convos: usize, msgs_per_convo: usize) -> Vec<u8> {
    let mut conversations = Vec::with_capacity(n_convos);
    for c in 0..n_convos {
        let chat = format!("55119{c:08}@s.whatsapp.net");
        let mut messages = Vec::with_capacity(msgs_per_convo);
        for m in 0..msgs_per_convo {
            let from_me = m % 2 == 0;
            // Vary content/length so the scan sees a realistic mix of 1- and
            // 2-byte length varints, matching real chat history.
            let inner = if m % 3 == 0 {
                wa::Message {
                    conversation: Some(pseudo_text((c * 41 + m) as u64, 130)),
                    ..Default::default()
                }
            } else {
                wa::Message {
                    extended_text_message: buffa::MessageField::some(
                        wa::message::ExtendedTextMessage {
                            text: Some(pseudo_text((c * 43 + m) as u64, 24)),
                            context_info: buffa::MessageField::some(wa::ContextInfo {
                                is_forwarded: Some(m % 4 == 0),
                                forwarding_score: Some((m % 7) as u32),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                    ),
                    message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
                        message_secret: Some(vec![m as u8; 32]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            };
            messages.push(wa::HistorySyncMsg {
                message: buffa::MessageField::some(wa::WebMessageInfo {
                    key: buffa::MessageField::some(wa::MessageKey {
                        remote_jid: Some(chat.clone()),
                        from_me: Some(from_me),
                        id: Some(format!("MSGID{c:04}{m:04}ABCDEF")),
                        participant: None,
                    }),
                    message: buffa::MessageField::some(inner),
                    message_timestamp: Some(1_700_000_000 + (c * msgs_per_convo + m) as u64),
                    ..Default::default()
                }),
                ..Default::default()
            });
        }
        conversations.push(wa::Conversation {
            id: chat,
            messages,
            ..Default::default()
        });
    }
    let hs = wa::HistorySync {
        sync_type: wa::history_sync::HistorySyncType::InitialBootstrap,
        conversations,
        ..Default::default()
    };
    let proto = hs.encode_to_vec();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&proto).unwrap();
    enc.finish().unwrap()
}

fn setup_history_sync_blob() -> Bytes {
    // ~500 conversations x 40 messages = 20k messages, a realistic
    // mid-size InitialBootstrap (multi-MB decompressed).
    static COMPRESSED: OnceLock<Bytes> = OnceLock::new();
    COMPRESSED
        .get_or_init(|| {
            Bytes::from(build_realistic_history_sync(
                HISTORY_CONVERSATIONS,
                MESSAGES_PER_CONVERSATION,
            ))
        })
        .clone()
}

#[divan::bench(sample_count = 5)]
fn bench_process_history_sync(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            // retain_blob = true also hands the compressed input back. The
            // result (records + retained blob) is returned so the harness
            // drops it outside the measured window, like a consumer would.
            black_box(wacore::history_sync::process_history_sync_bytes(
                black_box(blob),
                None,
                true,
            ))
        });
}

/// Translation-oriented consumer path: observe each borrowed message-secret
/// record without first materializing the core's owned record vector.
#[divan::bench(sample_count = 5)]
fn bench_process_history_sync_visit_records(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            let mut records = 0usize;
            let result = wacore::history_sync::process_history_sync_bytes_with_record_visitor(
                black_box(blob),
                None,
                true,
                |record| {
                    records += 1;
                    black_box(record);
                },
            )
            .unwrap();
            black_box((result, records))
        });
}

/// Consumer-side pass over the retained blob: drain every conversation through
/// the public stream and decode the remainder, the path an Event::HistorySync
/// handler pays per chunk.
#[divan::bench(sample_count = 5)]
fn bench_history_sync_stream_drain(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            let mut stream = wacore::history_sync::HistorySyncStream::new(
                black_box(&blob),
                wacore::history_sync::MAX_DECOMPRESSED,
            );
            let mut messages = 0usize;
            let mut conversation = waproto::whatsapp::Conversation::default();
            while stream.next_conversation_into(&mut conversation).unwrap() {
                messages += conversation.messages.len();
            }
            black_box((messages, stream.remainder().unwrap()))
        });
}

/// Consumer-side wire path: frame every conversation without decoding an
/// owned Rust protobuf. This isolates the second inflate + wire walk from the
/// host's protobuf decoder and from `Conversation` allocation reuse.
#[divan::bench(sample_count = 5)]
fn bench_history_sync_wire_stream_drain(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            let mut stream = wacore::history_sync::HistorySyncStream::new(
                black_box(&blob),
                wacore::history_sync::MAX_DECOMPRESSED,
            );
            let mut conversations = 0usize;
            let mut wire_bytes = 0usize;
            while let Some(conversation) = stream.next_conversation_bytes().unwrap() {
                conversations += 1;
                wire_bytes += conversation.len();
            }
            black_box((conversations, wire_bytes, stream.remainder().unwrap()))
        });
}

/// End-to-end core cost paid when internal extraction retains a lazy event and
/// a wire-oriented consumer subsequently drains it. Keeping this composition
/// beside the component benches makes a one-pass design's maximum possible
/// win explicit without conflating it with host-side decoding.
#[divan::bench(sample_count = 5)]
fn bench_history_sync_extract_then_wire_drain(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            let result =
                wacore::history_sync::process_history_sync_bytes(black_box(blob), None, true)
                    .unwrap();
            let compressed = result.compressed_bytes.as_ref().unwrap();
            let mut stream = wacore::history_sync::HistorySyncStream::new(
                compressed,
                result.decompressed_size as u64,
            );
            let mut conversations = 0usize;
            let mut wire_bytes = 0usize;
            while let Some(conversation) = stream.next_conversation_bytes().unwrap() {
                conversations += 1;
                wire_bytes += conversation.len();
            }
            let remainder = stream.remainder().unwrap();
            black_box((result, conversations, wire_bytes, remainder))
        });
}
