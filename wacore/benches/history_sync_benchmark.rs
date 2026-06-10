//! Realistic history-sync ingest: zlib inflate + full protobuf scan of a
//! mid-size InitialBootstrap (500 conversations x 40 messages, multi-MB
//! decompressed). This is the heaviest single-shot pipeline in the library
//! and the hottest consumer of the varint scan.

use divan::black_box;
use flate2::{Compression, write::ZlibEncoder};
use prost::Message;
use std::io::Write;
use waproto::whatsapp as wa;

fn main() {
    divan::main();
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
                    conversation: Some(format!(
                        "Mensagem de teste numero {m} no chat {c} com um pouco mais de texto \
                         para variar o tamanho do campo e gerar varints de 2 bytes ocasionais."
                    )),
                    ..Default::default()
                }
            } else {
                wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some(format!("Texto estendido curto {m}")),
                        context_info: Some(Box::new(wa::ContextInfo {
                            is_forwarded: Some(m % 4 == 0),
                            forwarding_score: Some((m % 7) as u32),
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    message_context_info: Some(wa::MessageContextInfo {
                        message_secret: Some(vec![m as u8; 32]),
                        ..Default::default()
                    }),
                    ..Default::default()
                }
            };
            messages.push(wa::HistorySyncMsg {
                message: Some(wa::WebMessageInfo {
                    key: wa::MessageKey {
                        remote_jid: Some(chat.clone()),
                        from_me: Some(from_me),
                        id: Some(format!("MSGID{c:04}{m:04}ABCDEF")),
                        participant: None,
                    },
                    message: Some(inner),
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
        sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
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
            // retain_blob = true exercises the full-buffer path: count pass +
            // main loop + per-conversation scan, the maximum varint volume.
            let _ = black_box(wacore::history_sync::process_history_sync(
                black_box(blob),
                None,
                true,
                None,
            ));
        });
}
