#![forbid(unsafe_code)]

use std::hint::black_box;
use std::io::Write as _;
use std::ops::Range;
use std::time::Instant;

use buffa::MessageView as _;
use bytes::Bytes;
use waproto::reference::{MessageOwnedView, MessageView};

struct OffsetOwner {
    bytes: Bytes,
    conversation: Option<Range<usize>>,
}

impl OffsetOwner {
    fn decode(bytes: Bytes) -> Result<Self, buffa::DecodeError> {
        let conversation = {
            let view = MessageView::decode_view(&bytes)?;
            view.conversation.map(|value| {
                let start = (value.as_ptr() as usize)
                    .checked_sub(bytes.as_ptr() as usize)
                    .expect("view borrows from its source");
                let end = start.checked_add(value.len()).expect("bounded span");
                assert!(end <= bytes.len());
                start..end
            })
        };
        Ok(Self {
            bytes,
            conversation,
        })
    }

    fn conversation(&self) -> Option<&str> {
        self.conversation.as_ref().map(|span| {
            std::str::from_utf8(&self.bytes[span.clone()]).expect("validated immutable source")
        })
    }
}

#[allow(clippy::disallowed_methods)]
fn measure(iterations: usize, mut read: impl FnMut() -> usize) -> f64 {
    for _ in 0..1000 {
        black_box(read());
    }
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(read());
    }
    start.elapsed().as_nanos() as f64 / iterations as f64
}

fn main() -> std::io::Result<()> {
    let mut rows = Vec::new();
    for len in [16, 1024, 65_536] {
        let mut input = vec![10];
        waproto::experiment::wire::varint(len as u64, &mut input);
        input.resize(input.len() + len, b'x');
        let bytes = Bytes::from(input);
        let incumbent = MessageOwnedView::decode(bytes.clone()).expect("synthetic fixture");
        let offsets = OffsetOwner::decode(bytes).expect("synthetic fixture");
        assert_eq!(incumbent.conversation(), offsets.conversation());
        let iterations = (128 * 1024 * 1024 / len).max(1000);
        for repetition in 0..5 {
            let (reference, safe) = if repetition % 2 == 0 {
                let reference = measure(iterations, || {
                    black_box(&incumbent)
                        .conversation()
                        .expect("present field")
                        .len()
                });
                let safe = measure(iterations, || {
                    black_box(&offsets)
                        .conversation()
                        .expect("present field")
                        .len()
                });
                (reference, safe)
            } else {
                let safe = measure(iterations, || {
                    black_box(&offsets)
                        .conversation()
                        .expect("present field")
                        .len()
                });
                let reference = measure(iterations, || {
                    black_box(&incumbent)
                        .conversation()
                        .expect("present field")
                        .len()
                });
                (reference, safe)
            };
            rows.push(serde_json::json!({
                "text_bytes": len, "iterations": iterations, "repetition": repetition,
                "incumbent_getter_ns": reference, "safe_offset_getter_ns": safe,
            }));
        }
    }
    writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&rows)?
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offset_owner_rejects_malformed_wire_and_invalid_utf8() {
        for input in [vec![10, 2, b'x'], vec![10, 1, 0xff]] {
            assert!(OffsetOwner::decode(Bytes::from(input)).is_err());
        }
    }

    #[test]
    fn empty_presence_and_absence_are_distinct() {
        assert_eq!(
            OffsetOwner::decode(Bytes::new()).unwrap().conversation(),
            None
        );
        assert_eq!(
            OffsetOwner::decode(Bytes::from_static(&[10, 0]))
                .unwrap()
                .conversation(),
            Some("")
        );
    }
}
