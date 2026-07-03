//! Realistic history-sync ingest: zlib inflate + full protobuf scan of a
//! mid-size InitialBootstrap (500 conversations x 40 messages, multi-MB
//! decompressed). This is the heaviest single-shot pipeline in the library
//! and the hottest consumer of the varint scan.

use buffa::Message;
use divan::black_box;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use waproto::whatsapp as wa;

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

fn setup_history_sync_blob() -> Vec<u8> {
    // ~500 conversations x 40 messages = 20k messages, a realistic
    // mid-size InitialBootstrap (multi-MB decompressed).
    build_realistic_history_sync(500, 40)
}

#[divan::bench(sample_count = 5)]
fn bench_process_history_sync(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_history_sync_blob)
        .bench_values(|blob| {
            // retain_blob = true also hands the compressed input back. The
            // result (records + retained blob) is returned so the harness
            // drops it outside the measured window, like a consumer would.
            black_box(wacore::history_sync::process_history_sync(
                black_box(blob),
                None,
                true,
            ))
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
