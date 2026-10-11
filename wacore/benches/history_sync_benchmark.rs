//! Realistic history-sync ingest: zlib inflate + full protobuf scan of a
//! mid-size InitialBootstrap. This is the heaviest single-shot pipeline in the
//! library and the hottest consumer of the varint scan.
//!
//! The fixture emulates the shape a real bootstrap actually has, not a uniform
//! run of 1:1 text chats: PN- and LID-addressed DMs carrying `pnJid`/`lidJid`
//! metadata and tctokens, group chats whose messages rotate through a
//! participant roster, a bulk `phoneNumberToLidMappings` block, and a message
//! mix of plain text, extended text with context, polls, bot invocations and
//! forwards. Each of those drives a different extraction branch — the PN/LID
//! harvest, the tctoken candidate, the per-participant secret rows — so a
//! uniform fixture measures the varint scan and nothing else.
//!
//! `divan::AllocProfiler` is wired as the global allocator so every row reports
//! allocation count and bytes next to wall time: the extraction walk borrows
//! from the inflated buffer, so what it allocates *per conversation, mapping
//! and message* is the number that moves. It costs some absolute throughput on
//! every row, which is fine as long as before/after are both measured with it.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use buffa::Message;
use bytes::Bytes;
use divan::black_box;
use divan::counter::BytesCount;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use std::sync::OnceLock;
use waproto::whatsapp as wa;

/// 1:1 chats, split between PN- and LID-addressed threads.
const DM_CONVERSATIONS: usize = 400;
/// Group chats, each with its own participant roster.
const GROUP_CONVERSATIONS: usize = 100;
const MESSAGES_PER_CONVERSATION: usize = 40;
/// `HistorySync.phoneNumberToLidMappings` (field 15). Deliberately wider than
/// `DM_CONVERSATIONS` so the block both overlaps the pairs the conversations
/// supply (exercising the dedupe conflict path) and adds pairs of its own.
const BULK_LID_MAPPINGS: usize = 500;
/// Roster sizes cycle over `[GROUP_MIN_PARTICIPANTS, GROUP_MAX_PARTICIPANTS]`.
const GROUP_MIN_PARTICIPANTS: usize = 5;
const GROUP_MAX_PARTICIPANTS: usize = 20;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

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

fn pn_jid(index: usize) -> String {
    format!("55119{index:08}@s.whatsapp.net")
}

fn lid_jid(index: usize) -> String {
    format!("1{index:014}@lid")
}

fn group_jid(index: usize) -> String {
    format!("12036{index:013}@g.us")
}

/// One message body, cycling through the payload shapes a real history carries.
/// The `message_secret` on all but the plain-text arm is what makes the
/// per-message secret extraction (and, in groups, the per-participant sender
/// resolution) actually run instead of bailing at the first field.
fn build_message_body(seed: u64, variant: usize) -> wa::Message {
    match variant {
        // Plain text, no secret: the cheapest record, and the one that must
        // still be walked to be rejected.
        0 => {
            let mut proto = wa::Message::default();
            proto.conversation = Some(pseudo_text(seed, 130));
            proto
        }
        // Extended text with context info.
        1 => {
            let mut proto = wa::Message::default();
            proto.extended_text_message = buffa::MessageField::some({
                let mut proto = wa::message::ExtendedTextMessage::default();
                proto.text = Some(pseudo_text(seed, 64));
                proto.context_info = buffa::MessageField::some({
                    let mut proto = wa::ContextInfo::default();
                    proto.is_forwarded = Some(false);
                    proto.forwarding_score = Some(0);
                    proto.stanza_id = Some(format!("QUOTE{seed:012X}"));
                    proto
                });
                proto
            });
            proto.message_context_info = buffa::MessageField::some({
                let mut proto = wa::MessageContextInfo::default();
                proto.message_secret = Some(vec![(seed & 0xff) as u8; 32]);
                proto
            });
            proto
        }
        // Poll: classified as `is_poll_or_event`, so it survives a bot-only
        // retention policy differently from the text arms.
        2 => {
            let mut proto = wa::Message::default();
            proto.poll_creation_message = buffa::MessageField::some({
                let mut proto = wa::message::PollCreationMessage::default();
                proto.name = Some(pseudo_text(seed, 24));
                proto.selectable_options_count = Some(1);
                proto
            });
            proto.message_context_info = buffa::MessageField::some({
                let mut proto = wa::MessageContextInfo::default();
                proto.message_secret = Some(vec![(seed & 0xff) as u8; 32]);
                proto
            });
            proto
        }
        // Bot invocation: `botMetadata` presence is the flag the classifier
        // reads, and the one class every retention policy keeps.
        3 => {
            let mut proto = wa::Message::default();
            proto.extended_text_message = buffa::MessageField::some({
                let mut proto = wa::message::ExtendedTextMessage::default();
                proto.text = Some(pseudo_text(seed, 48));
                proto
            });
            proto.message_context_info = buffa::MessageField::some({
                let mut proto = wa::MessageContextInfo::default();
                proto.message_secret = Some(vec![(seed & 0xff) as u8; 32]);
                proto.bot_metadata = buffa::MessageField::some({
                    let mut proto = wa::BotMetadata::default();
                    proto.persona_id = Some("persona".to_string());
                    proto
                });
                proto
            });
            proto
        }
        // Forwarded: extraction skips its secret entirely, so it measures the
        // walk that proves the message is forwarded.
        _ => {
            let mut proto = wa::Message::default();
            proto.extended_text_message = buffa::MessageField::some({
                let mut proto = wa::message::ExtendedTextMessage::default();
                proto.text = Some(pseudo_text(seed, 90));
                proto.context_info = buffa::MessageField::some({
                    let mut proto = wa::ContextInfo::default();
                    proto.is_forwarded = Some(true);
                    proto.forwarding_score = Some(3);
                    proto
                });
                proto
            });
            proto.message_context_info = buffa::MessageField::some({
                let mut proto = wa::MessageContextInfo::default();
                proto.message_secret = Some(vec![(seed & 0xff) as u8; 32]);
                proto
            });
            proto
        }
    }
}

fn build_history_msg(
    chat: &str,
    participant: Option<&str>,
    from_me: bool,
    convo: usize,
    index: usize,
) -> wa::HistorySyncMsg {
    let seed = (convo * 4099 + index * 31) as u64;
    {
        let mut proto = wa::HistorySyncMsg::default();
        proto.message = buffa::MessageField::some({
            let mut proto = wa::WebMessageInfo::default();
            proto.key = buffa::MessageField::some({
                let mut proto = wa::MessageKey::default();
                proto.remote_jid = Some(chat.to_string());
                proto.from_me = Some(from_me);
                proto.id = Some(format!("MSGID{convo:04}{index:04}ABCDEF"));
                proto.participant = participant.map(str::to_string);
                proto
            });
            proto.message = buffa::MessageField::some(build_message_body(seed, index % 5));
            proto.message_timestamp =
                Some(1_700_000_000 + (convo * MESSAGES_PER_CONVERSATION + index) as u64);
            proto
        });
        proto
    }
}

/// A 1:1 conversation, addressed by PN or by LID, carrying the opposite
/// namespace as metadata plus (usually) a tctoken.
fn build_dm_conversation(index: usize, msgs_per_convo: usize) -> wa::Conversation {
    let pn = pn_jid(index);
    let lid = lid_jid(index);
    let addressed_by_lid = index.is_multiple_of(3);
    let (chat, pn_meta, lid_meta) = if addressed_by_lid {
        (lid.clone(), Some(pn), None)
    } else {
        (pn.clone(), None, Some(lid))
    };

    let messages = (0..msgs_per_convo)
        .map(|m| build_history_msg(&chat, None, m.is_multiple_of(2), index, m))
        .collect();

    // Three DMs in four carry a token; one in two also carries the sender
    // bucket, which is how the wire mixes them.
    let has_token = index % 4 != 3;
    {
        let mut proto = wa::Conversation::default();
        proto.id = chat;
        proto.messages = messages;
        proto.pn_jid = pn_meta;
        proto.lid_jid = lid_meta;
        proto.tc_token = has_token.then(|| vec![(index % 251) as u8; 32]);
        proto.tc_token_timestamp = has_token.then_some(1_700_000_000 + index as u64);
        proto.tc_token_sender_timestamp =
            (has_token && index.is_multiple_of(2)).then_some(1_700_000_500 + index as u64);
        proto.conversation_timestamp = Some(1_700_100_000 + index as u64);
        proto.unread_count = Some((index % 7) as u32);
        proto
    }
}

/// A group conversation: a participant roster plus messages that rotate
/// through it, so consecutive messages usually repeat the previous sender and
/// occasionally switch.
fn build_group_conversation(index: usize, msgs_per_convo: usize) -> wa::Conversation {
    let chat = group_jid(index);
    let roster_size =
        GROUP_MIN_PARTICIPANTS + index % (GROUP_MAX_PARTICIPANTS - GROUP_MIN_PARTICIPANTS + 1);
    let roster: Vec<String> = (0..roster_size)
        .map(|p| {
            let member = index * GROUP_MAX_PARTICIPANTS + p;
            if p.is_multiple_of(2) {
                pn_jid(DM_CONVERSATIONS + member)
            } else {
                lid_jid(DM_CONVERSATIONS + member)
            }
        })
        .collect();

    let messages = (0..msgs_per_convo)
        .map(|m| {
            // Two consecutive messages per sender before rotating: real group
            // histories are bursty, and a roster that changed every message
            // would measure the worst case rather than the common one.
            let sender = &roster[(m / 2) % roster.len()];
            build_history_msg(&chat, Some(sender), m.is_multiple_of(11), index, m)
        })
        .collect();

    {
        let mut proto = wa::Conversation::default();
        proto.id = chat;
        proto.messages = messages;
        proto.participant = roster
            .iter()
            .enumerate()
            .map(|(p, jid)| {
                let mut proto = wa::GroupParticipant::default();
                proto.user_jid = jid.clone();
                proto.rank = Some(if p == 0 {
                    wa::group_participant::Rank::SUPERADMIN
                } else {
                    wa::group_participant::Rank::REGULAR
                });
                proto
            })
            .collect();
        proto.name = Some(format!("Group {index}"));
        proto.conversation_timestamp = Some(1_700_200_000 + index as u64);
        proto
    }
}

fn build_realistic_history_sync(
    dm_convos: usize,
    group_convos: usize,
    msgs_per_convo: usize,
    bulk_mappings: usize,
) -> Vec<u8> {
    let mut conversations = Vec::with_capacity(dm_convos + group_convos);
    for c in 0..dm_convos {
        conversations.push(build_dm_conversation(c, msgs_per_convo));
    }
    for g in 0..group_convos {
        conversations.push(build_group_conversation(g, msgs_per_convo));
    }

    let phone_number_to_lid_mappings = (0..bulk_mappings)
        .map(|i| {
            let mut proto = wa::PhoneNumberToLIDMapping::default();
            proto.pn_jid = Some(pn_jid(i));
            proto.lid_jid = Some(lid_jid(i));
            proto
        })
        .collect();

    let hs = {
        let mut proto = wa::HistorySync::default();
        proto.sync_type = wa::history_sync::HistorySyncType::InitialBootstrap;
        proto.conversations = conversations;
        proto.phone_number_to_lid_mappings = phone_number_to_lid_mappings;
        proto
    };
    let proto = hs.encode_to_vec();
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&proto).unwrap();
    enc.finish().unwrap()
}

struct HistorySyncFixture {
    compressed: Bytes,
    /// Inflated size, so every row reports throughput over the bytes the walk
    /// actually scans rather than over the compressed input.
    decompressed_len: usize,
}

fn fixture() -> &'static HistorySyncFixture {
    // 500 conversations x 40 messages = 20k messages, a realistic mid-size
    // InitialBootstrap (multi-MB decompressed).
    static FIXTURE: OnceLock<HistorySyncFixture> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let compressed = Bytes::from(build_realistic_history_sync(
            DM_CONVERSATIONS,
            GROUP_CONVERSATIONS,
            MESSAGES_PER_CONVERSATION,
            BULK_LID_MAPPINGS,
        ));
        let decompressed_len =
            wacore::history_sync::process_history_sync_bytes(compressed.clone(), None, false)
                .expect("fixture must decode")
                .decompressed_size;
        HistorySyncFixture {
            compressed,
            decompressed_len,
        }
    })
}

fn setup_history_sync_blob() -> Bytes {
    fixture().compressed.clone()
}

fn scanned_bytes() -> BytesCount {
    BytesCount::new(fixture().decompressed_len)
}

#[divan::bench(sample_count = 5)]
fn bench_process_history_sync(bencher: divan::Bencher) {
    bencher
        .counter(scanned_bytes())
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
        .counter(scanned_bytes())
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
        .counter(scanned_bytes())
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
        .counter(scanned_bytes())
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
        .counter(scanned_bytes())
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
