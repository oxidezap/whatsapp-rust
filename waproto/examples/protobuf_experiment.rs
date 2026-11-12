#![forbid(unsafe_code)]

use std::hint::black_box;
use std::io::Write as _;
use std::time::Instant;

#[path = "../experiment/wire.rs"]
#[allow(dead_code)]
mod wire;

#[cfg(not(feature = "protobuf-experiment-oracle"))]
#[allow(dead_code)]
mod candidate {
    use super::wire;
    include!(concat!(env!("OUT_DIR"), "/experiment.rs"));
}

#[cfg(feature = "protobuf-experiment-oracle")]
use buffa::{Message as _, MessageView as _};

#[allow(clippy::disallowed_methods)]
fn measure(name: &str, iterations: u64, mut run: impl FnMut()) {
    for _ in 0..1000 {
        run();
    }
    let start = Instant::now();
    for _ in 0..iterations {
        run();
    }
    writeln!(
        std::io::stdout().lock(),
        "{name}\t{iterations}\t{}",
        start.elapsed().as_nanos()
    )
    .expect("write sample");
}

#[allow(clippy::disallowed_methods)]
fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map(|v| v.parse::<u64>().expect("iteration count"))
        .unwrap_or(100_000);
    let family = std::env::args().nth(2).unwrap_or_else(|| "all".into());

    macro_rules! workload {
        ($name:literal, $owned:ident, $view:ident, $oracle:path, $oracle_view:path, $input:expr, $selected:ident) => {{
            if family == "all" || family == $name {
                let input: Vec<u8> = $input;
                #[cfg(feature = "protobuf-experiment-oracle")]
                let message = <$oracle>::decode_from_slice(&input).expect("reference fixture");
                #[cfg(not(feature = "protobuf-experiment-oracle"))]
                let message = candidate::$owned::decode(&input).expect("candidate fixture");
                #[cfg(feature = "protobuf-experiment-oracle")]
                let view = <$oracle_view>::decode_view(&input).expect("reference fixture");
                #[cfg(not(feature = "protobuf-experiment-oracle"))]
                let view = candidate::$view::decode(&input).expect("candidate fixture");
                let mut buffer = Vec::with_capacity(input.len());
                measure(concat!($name, "/length"), iterations, || {
                    black_box(black_box(&message).encoded_len());
                });
                measure(concat!($name, "/encode-reused"), iterations, || {
                    buffer.clear();
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(&message).encode(&mut buffer);
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(&message).encode_into(&mut buffer);
                    black_box(&buffer);
                });
                measure(concat!($name, "/decode-owned"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(
                        <$oracle>::decode_from_slice(black_box(&input)).expect("valid fixture"),
                    );
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(candidate::$owned::decode(black_box(&input)).expect("valid fixture"));
                });
                measure(concat!($name, "/decode-borrowed"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(
                        <$oracle_view>::decode_view(black_box(&input)).expect("valid fixture"),
                    );
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(candidate::$view::decode(black_box(&input)).expect("valid fixture"));
                });
                measure(concat!($name, "/select-existing-view"), iterations, || {
                    black_box(&black_box(&view).$selected);
                });
                measure(concat!($name, "/promote"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(black_box(&view).to_owned_message().expect("valid view"));
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(black_box(&view).to_owned_message());
                });
                let mut invalid = input.clone();
                invalid.push(0);
                measure(concat!($name, "/reject-invalid"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    assert!(black_box(<$oracle>::decode_from_slice(black_box(&invalid))).is_err());
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    assert!(black_box(candidate::$owned::decode(black_box(&invalid))).is_err());
                });
                let mut unknown = input.clone();
                unknown.extend_from_slice(&[0x80, 0xb5, 0x18, 0x81, 0]);
                measure(concat!($name, "/unknown-owned"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(
                        <$oracle>::decode_from_slice(black_box(&unknown)).expect("valid unknown"),
                    );
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(
                        candidate::$owned::decode(black_box(&unknown)).expect("valid unknown"),
                    );
                });
                measure(concat!($name, "/unknown-borrowed"), iterations, || {
                    #[cfg(feature = "protobuf-experiment-oracle")]
                    black_box(
                        <$oracle_view>::decode_view(black_box(&unknown)).expect("valid unknown"),
                    );
                    #[cfg(not(feature = "protobuf-experiment-oracle"))]
                    black_box(
                        candidate::$view::decode(black_box(&unknown)).expect("valid unknown"),
                    );
                });
            }
        }};
    }

    workload!(
        "message-key",
        MessageKey,
        MessageKeyView,
        waproto::whatsapp::MessageKey,
        waproto::whatsapp::MessageKeyView,
        b"\x0a\x1412345@s.whatsapp.net\x10\x01\x1a\x09synthetic".to_vec(),
        id
    );
    workload!(
        "signal-key",
        SessionStructureChainMessageKey,
        SessionStructureChainMessageKeyView,
        waproto::whatsapp::session_structure::chain::MessageKey,
        waproto::whatsapp::session_structure::chain::MessageKeyView,
        {
            let mut input = vec![8, 127];
            for tag in [18, 26, 34] {
                input.extend_from_slice(&[tag, 32]);
                input.extend_from_slice(&[0x42; 32]);
            }
            input.extend_from_slice(&[0xa2, 6, 32]);
            input.extend_from_slice(&[0x77; 32]);
            input
        },
        seed
    );
    workload!(
        "appstate-small",
        SyncdValue,
        SyncdValueView,
        waproto::whatsapp::SyncdValue,
        waproto::whatsapp::SyncdValueView,
        {
            let mut input = vec![10, 32];
            input.extend_from_slice(&[0x42; 32]);
            input
        },
        blob
    );
    workload!(
        "appstate-large",
        SyncdValue,
        SyncdValueView,
        waproto::whatsapp::SyncdValue,
        waproto::whatsapp::SyncdValueView,
        {
            let mut input = vec![10, 128, 128, 4];
            input.resize(65540, 0x42);
            input
        },
        blob
    );
}
