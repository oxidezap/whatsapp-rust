//! Per-send message utilities on realistic shapes: the participant hash that
//! runs on every group send (1600 devices = a large LID group) and the
//! pad/encode steps every outgoing message pays before encryption.

use divan::black_box;
use wacore::messages::MessageUtils;
use wacore_binary::jid::Jid;
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

fn setup_device_list(users: usize, devices_per_user: u16) -> Vec<Jid> {
    let mut out = Vec::with_capacity(users * devices_per_user as usize);
    for u in 0..users {
        for d in 0..devices_per_user {
            let mut jid = Jid::lid(format!("1003{u:011}"));
            jid.device = d;
            out.push(jid);
        }
    }
    out
}

/// 800 members x 2 devices: the phash input of a large group fan-out.
#[divan::bench]
fn bench_participant_list_hash_1600(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| setup_device_list(800, 2))
        .bench_refs(|devices| {
            black_box(MessageUtils::participant_list_hash(black_box(&**devices)).unwrap())
        });
}

fn text_message() -> wa::Message {
    wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("Benchmark message with a realistic amount of text content.".into()),
            context_info: Some(Box::new(wa::ContextInfo {
                stanza_id: Some("3EB0F4E1D2C3B4A59687".into()),
                participant: Some("5511999990000@s.whatsapp.net".into()),
                ..Default::default()
            })),
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// Proto encode + random padding: runs once per outgoing message before
/// every Signal encryption.
#[divan::bench]
fn bench_encode_and_pad(bencher: divan::Bencher) {
    bencher
        .with_inputs(text_message)
        .bench_refs(|msg| black_box(MessageUtils::encode_and_pad(black_box(msg))));
}

/// Unpad of a received plaintext: runs once per decrypted message.
#[divan::bench]
fn bench_unpad_message_ref(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            use prost::Message as _;
            MessageUtils::pad_message_v2(text_message().encode_to_vec())
        })
        .bench_refs(|padded| {
            black_box(
                MessageUtils::unpad_message_ref(black_box(padded), 2)
                    .unwrap()
                    .len(),
            )
        });
}

fn dm_shape(shape: &str) -> wa::Message {
    match shape {
        "text_reply" => text_message(),
        "media_refs" => wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                url: Some("https://mmg.whatsapp.net/v/t62.7118-24/abc123".into()),
                direct_path: Some("/v/t62.7118-24/abc123".into()),
                mimetype: Some("image/jpeg".into()),
                caption: Some("Benchmark media caption".into()),
                media_key: Some(vec![0xA5; 32]),
                file_sha256: Some(vec![0x11; 32]),
                file_enc_sha256: Some(vec![0x22; 32]),
                file_length: Some(184_320),
                height: Some(1280),
                width: Some(960),
                jpeg_thumbnail: Some(vec![0x7F; 6 * 1024]),
                ..Default::default()
            })),
            ..Default::default()
        },
        "large_text" => wa::Message {
            conversation: Some("Lorem ipsum dolor sit amet 0123456789 ".repeat(108)),
            ..Default::default()
        },
        other => unreachable!("unknown shape {other}"),
    }
}

/// The CPU a single DM send pays in the encode/token department, mirroring
/// `wacore::send::dm` plus the retry-cache serialization the client does:
/// reporting token (full content encode + HKDF + HMAC), the splice into the
/// recipient and DeviceSentMessage plaintexts, and the recent-message bytes.
/// Exists so flamegraphs keep the byte-identical encode pair (reporting
/// content vs retry bytes) visible while it remains deduplicable.
#[divan::bench(args = ["text_reply", "media_refs", "large_text"])]
fn bench_dm_send_encode_work(bencher: divan::Bencher, shape: &str) {
    use wacore::reporting_token::{
        extract_message_secret, generate_reporting_token, reporting_context_info,
    };

    let own_jid: Jid = "5511999990000:7@s.whatsapp.net".parse().unwrap();
    let to_jid: Jid = "5511888887777@s.whatsapp.net".parse().unwrap();

    bencher
        .with_inputs(|| dm_shape(shape))
        .bench_refs(|message| {
            let reporting_result = generate_reporting_token(
                black_box(message),
                "3EB0BENCHBENCHBENCH01",
                &own_jid,
                &to_jid,
                extract_message_secret(message),
            );
            let extra_context = reporting_result.as_ref().map(reporting_context_info);
            let plaintexts = MessageUtils::encode_dm_plaintexts(
                black_box(message),
                extra_context.as_ref(),
                "5511888887777@s.whatsapp.net",
            );
            let retry_bytes = waproto::codec::message_to_vec(black_box(message));
            // Observe the whole result, not just the secret reporting_context_info
            // reads: reporting_token feeds nothing downstream, so without this the
            // HMAC (and, by cascade, the HKDF and the content encode this bench
            // exists to profile) is dead under the bench profile's thin LTO.
            black_box((plaintexts, retry_bytes, reporting_result))
        });
}
