use divan::black_box;
use wacore::media_retry::{MediaRetryError, MediaRetryResult, parse_media_retry_notification};
use wacore_binary::{Node, builder::NodeBuilder};
use waproto::protobuf::encoding::{put_bytes_field, put_varint};

const MEDIA_KEY: [u8; 32] = [0x42; 32];
const STANZA_ID: &str = "retry-fixture";
const CASES: [&str; 8] = [
    "short",
    "path_127",
    "path_128",
    "unknown_short",
    "unknown_long",
    "duplicate",
    "error_result",
    "invalid_utf8",
];
const PROJECTION_CASES: [&str; 4] = ["short", "path_127", "unknown_short", "invalid_utf8"];

struct Fixture {
    plaintext: Vec<u8>,
    node: Node,
}

fn fixture(case: &str) -> Fixture {
    let mut plaintext = Vec::new();
    if case == "duplicate" {
        put_bytes_field(1, b"old-id", &mut plaintext);
    }
    let id = if case == "invalid_utf8" {
        &b"\xff"[..]
    } else {
        STANZA_ID.as_bytes()
    };
    put_bytes_field(1, id, &mut plaintext);
    let path = match case {
        "path_127" | "unknown_long" => "p".repeat(127),
        "path_128" => "p".repeat(128),
        _ => "/media/synthetic.enc".to_owned(),
    };
    if matches!(case, "unknown_short" | "unknown_long") {
        let count = if case == "unknown_long" {
            256 - (6 + STANZA_ID.len() + path.len() + 4)
        } else {
            1
        };
        put_bytes_field(50_000, &vec![0x5a; count], &mut plaintext);
    }
    if case != "error_result" {
        put_bytes_field(2, path.as_bytes(), &mut plaintext);
    }
    put_varint(3 << 3, &mut plaintext);
    put_varint(if case == "error_result" { 2 } else { 1 }, &mut plaintext);

    let mut key = [0; 32];
    wacore::crypto::hkdf_sha256_into(
        &MEDIA_KEY,
        None,
        b"WhatsApp Media Retry Notification",
        &mut key,
    )
    .expect("synthetic key derivation");
    let iv = [1; 12];
    let mut ciphertext = Vec::new();
    wacore::libsignal::crypto::aes_256_gcm_encrypt(
        &key,
        &iv,
        STANZA_ID.as_bytes(),
        &plaintext,
        &mut ciphertext,
    )
    .expect("synthetic notification encryption");
    let node = NodeBuilder::new("notification")
        .attr("id", STANZA_ID)
        .children([NodeBuilder::new("encrypt")
            .children([
                NodeBuilder::new("enc_p").bytes(ciphertext).build(),
                NodeBuilder::new("enc_iv").bytes(iv.to_vec()).build(),
            ])
            .build()])
        .build();
    let result = parse_media_retry_notification(&node.as_node_ref(), &MEDIA_KEY);
    match case {
        "error_result" => assert!(matches!(result, Ok(MediaRetryResult::NotFound))),
        "invalid_utf8" => assert!(matches!(result, Err(MediaRetryError::Decode(_)))),
        _ => assert!(
            matches!(result, Ok(MediaRetryResult::Success { direct_path }) if direct_path == path)
        ),
    }
    Fixture { plaintext, node }
}

#[divan::bench(args = CASES)]
fn notification(bencher: divan::Bencher, case: &str) {
    let fixture = fixture(case);
    bencher.bench(|| {
        black_box(parse_media_retry_notification(
            black_box(&fixture.node.as_node_ref()),
            black_box(&MEDIA_KEY),
        ))
    });
}

#[divan::bench(args = PROJECTION_CASES)]
fn projection(bencher: divan::Bencher, case: &str) {
    let fixture = fixture(case);
    bencher.bench(|| {
        black_box(waproto::codec::media_retry_success(black_box(
            &fixture.plaintext,
        )))
    });
}

fn main() {
    divan::main();
}
