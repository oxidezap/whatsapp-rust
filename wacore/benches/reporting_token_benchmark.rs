use divan::black_box;
use prost::Message;
use wacore::reporting_token::{
    MESSAGE_SECRET_SIZE, REPORTING_TOKEN_KEY_SIZE, calculate_reporting_token,
    derive_reporting_token_key, generate_reporting_token, generate_reporting_token_content,
};
use wacore_binary::jid::Jid;
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

fn create_simple_message() -> wa::Message {
    wa::Message {
        conversation: Some("Hello, World!".to_string()),
        ..Default::default()
    }
}

fn create_extended_message() -> wa::Message {
    wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("Test message with context info".to_string()),
            context_info: Some(Box::new(wa::ContextInfo {
                is_forwarded: Some(true),
                forwarding_score: Some(5),
                ..Default::default()
            })),
            ..Default::default()
        })),
        ..Default::default()
    }
}

fn create_test_jid(user: &str) -> Jid {
    Jid::pn(user)
}

// Setup functions
fn setup_simple_message() -> wa::Message {
    create_simple_message()
}

fn setup_extended_message() -> wa::Message {
    create_extended_message()
}

// Content extraction benchmarks
#[divan::bench]
fn bench_content_extraction_simple(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_simple_message)
        .bench_refs(|msg| black_box(generate_reporting_token_content(msg)));
}

#[divan::bench]
fn bench_content_extraction_extended(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_extended_message)
        .bench_refs(|msg| black_box(generate_reporting_token_content(msg)));
}

// Key derivation benchmark
#[divan::bench]
fn bench_key_derivation() {
    let secret = [0x42u8; MESSAGE_SECRET_SIZE];
    let stanza_id = "3EB0E0E5F2D4F618589C0B";
    let sender_jid = "5511999887766@s.whatsapp.net";
    let remote_jid = "5511888776655@s.whatsapp.net";

    let _ = black_box(derive_reporting_token_key(
        &secret, stanza_id, sender_jid, remote_jid,
    ));
}

// Token calculation benchmark
#[divan::bench]
fn bench_token_calculation() {
    let key = [0x55u8; REPORTING_TOKEN_KEY_SIZE];
    let content = b"Hello, World! This is test content for HMAC.";

    let _ = black_box(calculate_reporting_token(&key, content));
}

// Full token generation - setup data
struct FullGenSetup {
    msg: wa::Message,
    sender: Jid,
    remote: Jid,
    secret: [u8; MESSAGE_SECRET_SIZE],
}

fn setup_full_gen_simple() -> FullGenSetup {
    FullGenSetup {
        msg: create_simple_message(),
        sender: create_test_jid("sender"),
        remote: create_test_jid("remote"),
        secret: [0xAAu8; MESSAGE_SECRET_SIZE],
    }
}

fn setup_full_gen_extended() -> FullGenSetup {
    FullGenSetup {
        msg: create_extended_message(),
        sender: create_test_jid("sender"),
        remote: create_test_jid("remote"),
        secret: [0xAAu8; MESSAGE_SECRET_SIZE],
    }
}

#[divan::bench]
fn bench_full_token_generation_simple(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_full_gen_simple)
        .bench_refs(|data| {
            black_box(generate_reporting_token(
                &data.msg,
                "STANZA123",
                &data.sender,
                &data.remote,
                Some(&data.secret),
            ))
        });
}

#[divan::bench]
fn bench_full_token_generation_extended(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_full_gen_extended)
        .bench_refs(|data| {
            black_box(generate_reporting_token(
                &data.msg,
                "STANZA123",
                &data.sender,
                &data.remote,
                Some(&data.secret),
            ))
        });
}

// Message encoding benchmarks
#[divan::bench]
fn bench_message_encoding_simple(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_simple_message)
        .bench_refs(|msg| black_box(msg.encode_to_vec()));
}

#[divan::bench]
fn bench_message_encoding_extended(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_extended_message)
        .bench_refs(|msg| black_box(msg.encode_to_vec()));
}
