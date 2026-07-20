//! Per-message utilities on realistic shapes: the participant hash that
//! runs on every group send (1600 devices = a large LID group), the
//! pad/encode steps every outgoing message pays before encryption, and the
//! unpad/decode steps every incoming message pays after decryption.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

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
        extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("Benchmark message with a realistic amount of text content.".into()),
            context_info: buffa::MessageField::some(wa::ContextInfo {
                stanza_id: Some("3EB0F4E1D2C3B4A59687".into()),
                participant: Some("5511999990000@s.whatsapp.net".into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
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
            use buffa::Message as _;
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
            image_message: buffa::MessageField::some(wa::message::ImageMessage {
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
            }),
            ..Default::default()
        },
        "large_text" => wa::Message {
            conversation: Some("Lorem ipsum dolor sit amet 0123456789 ".repeat(108)),
            ..Default::default()
        },
        other => unreachable!("unknown shape {other}"),
    }
}

fn recv_shape(shape: &str) -> wa::Message {
    match shape {
        // The first group message from a sender carries the SKDM inline
        // alongside the content.
        "group_skdm_text" => wa::Message {
            sender_key_distribution_message: buffa::MessageField::some(
                wa::message::SenderKeyDistributionMessage {
                    group_id: Some("120363000000000001@g.us".into()),
                    axolotl_sender_key_distribution_message: Some(vec![0x33; 350]),
                },
            ),
            conversation: Some("Benchmark group message with realistic text.".into()),
            ..Default::default()
        },
        other => dm_shape(other),
    }
}

/// Unpad + prost decode of a received padded plaintext: the pure tail of every
/// inbound message decryption, and the inbound mirror of `encode_and_pad`.
/// Shapes match the send-side bench plus the inline-SKDM group first-message.
#[divan::bench(args = ["text_reply", "media_refs", "large_text", "group_skdm_text"])]
fn bench_decode_plaintext(bencher: divan::Bencher, shape: &str) {
    bencher
        .with_inputs(|| {
            use buffa::Message as _;
            MessageUtils::pad_message_v2(recv_shape(shape).encode_to_vec())
        })
        .bench_refs(|padded| {
            black_box(wacore::messages::decode_plaintext(black_box(padded), 2).unwrap())
        });
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

/// A `<message>` stanza with the attributes each inbound shape carries, so
/// `parse_message_info` exercises every arm of its addressing-mode dispatch.
/// Each carries an `<enc>` child like every real message, so the
/// `get_optional_child` scans run instead of returning instant-None.
fn msg_info_node(shape: &str) -> wacore_binary::node::Node {
    use wacore_binary::builder::NodeBuilder;
    let enc = || {
        NodeBuilder::new("enc")
            .attr("type", "msg")
            .attr("v", "2")
            .bytes(vec![0u8; 64])
            .build()
    };
    match shape {
        // Plain 1:1 DM from a PN peer; sender_lid carries the LID fallback a
        // LID-migrated peer sends, exercising the self/other-DM sender_alt arm.
        "dm_pn" => NodeBuilder::new("message")
            .attr("from", "5511888887777@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "3EB0BENCH000000000001")
            .attr("t", "1777415965")
            .attr("notify", "Bench Peer")
            .attr("sender_lid", "100000012345678@lid")
            .children([enc()])
            .build(),
        // LID-addressed group message: participant is a LID JID, participant_pn
        // carries the phone fallback the LID-PN cache re-warms from.
        "group_lid" => NodeBuilder::new("message")
            .attr("from", "120363000000000001@g.us")
            .attr("type", "text")
            .attr("id", "3EB0BENCH000000000002")
            .attr("t", "1777415965")
            .attr("addressing_mode", "lid")
            .attr("participant", "100000012345678@lid")
            .attr("participant_pn", "5511888887777@s.whatsapp.net")
            .attr("notify", "Bench Member")
            .children([enc()])
            .build(),
        // Status broadcast: from=status@broadcast, participant is the author,
        // participant_lid warms the LID-PN cache.
        "status_broadcast" => NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "3EB0BENCH000000000003")
            .attr("t", "1777415965")
            .attr("participant", "5511999990001@s.whatsapp.net")
            .attr("participant_lid", "100000011111111@lid")
            .children([enc()])
            .build(),
        // Self-sent message (from == own JID): recipient drives chat resolution.
        "self_sent" => NodeBuilder::new("message")
            .attr("from", "5511999990000:7@s.whatsapp.net")
            .attr("recipient", "5511888887777@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "3EB0BENCH000000000004")
            .attr("t", "1777415965")
            .children([enc()])
            .build(),
        other => unreachable!("unknown shape {other}"),
    }
}

/// Stanza-to-MessageInfo parse: the metadata extraction that runs once per
/// inbound message before any Signal work. Exercises typed-JID attribute
/// materialization and the addressing-mode dispatch. The input is a marshal
/// round-trip decoded back into an `OwnedNodeRef` (untimed in `with_inputs`),
/// so attributes arrive wire-typed (`ValueRef::Jid`) exactly as the decoder
/// hands them to the receive path — not string-parsed, which production never
/// does. Only the parse is measured; the whole `MessageInfo` is observed.
#[divan::bench(args = ["dm_pn", "group_lid", "status_broadcast", "self_sent"])]
fn bench_parse_message_info(bencher: divan::Bencher, shape: &str) {
    use wacore_binary::node::OwnedNodeRef;
    let own_jid: Jid = "5511999990000@s.whatsapp.net".parse().unwrap();
    let own_lid: Jid = "100000000000000@lid".parse().unwrap();

    bencher
        .with_inputs(|| {
            let bytes = wacore_binary::marshal::marshal(&msg_info_node(shape)).unwrap();
            // marshal prefixes a flag byte; the receive path strips it in unpack.
            OwnedNodeRef::new(bytes[1..].to_vec()).unwrap()
        })
        .bench_refs(|owned| {
            black_box(
                wacore::messages::parse_message_info(
                    black_box(owned.get()),
                    &own_jid,
                    Some(&own_lid),
                )
                .unwrap(),
            )
        });
}
