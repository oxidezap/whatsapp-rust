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
