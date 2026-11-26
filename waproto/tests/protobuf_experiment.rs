#![cfg(feature = "protobuf-experiment")]
#![forbid(unsafe_code)]

use waproto::experiment::wire;

#[allow(dead_code)]
mod candidate {
    pub use waproto::experiment::*;
    include!(concat!(env!("OUT_DIR"), "/experiment_tests.rs"));
}

trait Candidate {
    type Reference;
    fn encode_bytes(&self) -> Vec<u8>;
    fn borrowed_to_owned(input: &[u8]) -> Result<Self, wire::Error>
    where
        Self: Sized;
    fn assert_reference(&self, reference: &Self::Reference);
    fn assert_borrowed(input: &[u8], reference: &Self::Reference);
}

fn error(error: buffa::DecodeError) -> wire::Error {
    use buffa::DecodeError as B;
    use wire::Error as E;
    match error {
        B::UnexpectedEof => E::Eof,
        B::VarintTooLong => E::Varint,
        B::InvalidWireType(value) => E::Wire(value),
        B::InvalidFieldNumber => E::Tag,
        B::WireTypeMismatch {
            field_number,
            expected,
            actual,
        } => E::Mismatch {
            field: field_number,
            expected,
            actual,
        },
        B::InvalidUtf8 => E::Utf8,
        B::RecursionLimitExceeded => E::Depth,
        B::InvalidEndGroup(value) => E::EndGroup(value),
        B::MessageTooLarge => E::Size,
        other => panic!("unmapped reference error: {other:?}"),
    }
}

#[allow(clippy::disallowed_methods)]
fn differential<M, C>(decode: impl Fn(&[u8]) -> Result<C, wire::Error>, fields: &[(u32, u8)])
where
    M: buffa::Message + buffa::HasMessageView,
    C: Candidate<Reference = M>,
{
    use buffa::MessageView as _;

    let compare = |input: &[u8]| {
        let reference = M::decode_from_slice(input).map_err(error);
        let actual = decode(input);
        if let (Ok(actual), Ok(reference)) = (&actual, &reference) {
            actual.assert_reference(reference);
        }
        let reference = reference.map(|v| v.encode_to_vec());
        let actual = actual.map(|v| v.encode_bytes());
        assert_eq!(
            actual,
            reference,
            "owned {} input={input:02x?}",
            std::any::type_name::<M>()
        );
        let reference_view = <M as buffa::HasMessageView>::decode_view(input)
            .and_then(|v| v.to_owned_message())
            .map_err(error);
        let actual_view = C::borrowed_to_owned(input);
        if let (Ok(actual), Ok(reference)) = (&actual_view, &reference_view) {
            actual.assert_reference(reference);
            C::assert_borrowed(input, reference);
        }
        let reference_view = reference_view.map(|v| v.encode_to_vec());
        let actual_view = actual_view.map(|v| v.encode_bytes());
        assert_eq!(
            actual_view,
            reference_view,
            "view {} input={input:02x?}",
            std::any::type_name::<M>()
        );
    };

    compare(&[]);
    let mut combined = Vec::new();
    for &(field, kind) in fields.iter().rev() {
        for payload in 0..8 {
            let mut bytes = Vec::new();
            wire::varint(u64::from(field) << 3 | u64::from(kind), &mut bytes);
            match kind {
                0 => wire::varint(
                    [0, 1, 127, 128, 16384, u32::MAX as u64, 1 << 63, u64::MAX][payload],
                    &mut bytes,
                ),
                1 => bytes.extend_from_slice(
                    &[
                        0u64,
                        1,
                        u64::MAX,
                        0x8000_0000_0000_0000,
                        0x7ff0_0000_0000_0000,
                        0x7ff8_0000_0000_0000,
                        123,
                        456,
                    ][payload]
                        .to_le_bytes(),
                ),
                2 => {
                    let text: &[u8] = [
                        b"".as_slice(),
                        b"x",
                        "olá".as_bytes(),
                        b"12345@s.whatsapp.net",
                        b"synthetic",
                        b"\0",
                        b"abc",
                        b"last",
                    ][payload];
                    wire::varint(text.len() as u64, &mut bytes);
                    bytes.extend_from_slice(text);
                }
                5 => bytes.extend_from_slice(
                    &[
                        0u32,
                        1,
                        u32::MAX,
                        0x8000_0000,
                        0x7f80_0000,
                        0x7fc0_0000,
                        123,
                        456,
                    ][payload]
                        .to_le_bytes(),
                ),
                _ => unreachable!(),
            }
            compare(&bytes);
            for len in 1..bytes.len() {
                compare(&bytes[..len]);
            }
            combined.extend_from_slice(&bytes);
        }
        for invalid in [0, 1, 2, 3, 4, 5, 6, 7] {
            let mut bytes = Vec::new();
            wire::varint(u64::from(field) << 3 | invalid, &mut bytes);
            bytes.extend_from_slice(&[0; 8]);
            compare(&bytes);
        }
    }
    compare(&combined);

    let mut unknowns = vec![0x80, 0xb5, 0x18, 0x81, 0];
    unknowns.extend_from_slice(&combined);
    compare(&unknowns);
    for depth in [1, 99, 100, 101] {
        let mut input = [0x83, 0xb5, 0x18].repeat(depth);
        input.extend([0x84, 0xb5, 0x18].repeat(depth));
        compare(&input);
    }
    for input in [
        vec![0],
        vec![6],
        vec![7],
        vec![8, 128],
        vec![255; 10],
        vec![128, 128, 128, 128, 16],
        vec![0xa4, 6],
        vec![0xa3, 6, 0xac, 6],
        vec![0xa2, 6, 255, 255, 255, 255, 255, 255, 255, 255, 255, 1],
    ] {
        compare(&input);
    }

    let mut state = 0xf133_7a83_341c_932bu64;
    for len in 0..64 {
        for _ in 0..128 {
            let input: Vec<_> = (0..len)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            compare(&input);
        }
    }
}

#[test]
fn borrowed_fields_point_into_the_original_input() {
    let input = [0x0a, 3, b'o', b'l', b'd'];
    let view = candidate::MessageKeyView::decode(&input).unwrap();
    assert_eq!(view.remote_jid.unwrap().as_ptr(), input[2..].as_ptr());
    let input = [0x0a, 3, 1, 2, 3];
    let view = candidate::SyncdValueView::decode(&input).unwrap();
    assert_eq!(view.blob.unwrap().as_ptr(), input[2..].as_ptr());
}

#[test]
fn invalid_utf8_is_rejected_and_bytes_are_accepted() {
    let input = [0x0a, 1, 0xff];
    assert_eq!(
        candidate::MessageKeyView::decode(&input),
        Err(wire::Error::Utf8)
    );
    assert_eq!(
        candidate::SyncdValueView::decode(&input).unwrap().blob,
        Some([0xff].as_slice())
    );
}

#[test]
fn frozen_storage_and_wallpaper_bytes_match_the_previous_schema() {
    let seed = [0xa2, 6, 3, 1, 2, 3];
    let key = candidate::SessionStructureChainMessageKey::decode(&seed).unwrap();
    assert_eq!(key.seed.as_deref(), Some([1, 2, 3].as_slice()));
    assert_eq!(key.encode_bytes(), seed);

    let legacy = [0x0a, 3, b'r', b'e', b'd', 0x12, 4, b'b', b'l', b'u', b'e'];
    let argb = [0x0d, 0x33, 0x22, 0x11, 0xff, 0x15, 0x66, 0x55, 0x44, 0xff];
    for bytes in [&legacy[..], &argb[..]] {
        let value = candidate::MessageChatSolidColorWallpaper::decode(bytes).unwrap();
        assert_eq!(value.encode_bytes(), bytes);
    }
    let mut mixed = legacy.to_vec();
    mixed.extend(argb);
    let value = candidate::MessageChatSolidColorWallpaper::decode(&mixed).unwrap();
    assert_eq!(value.color_light.as_deref(), Some("red"));
    assert_eq!(value.color_dark.as_deref(), Some("blue"));
    assert_eq!(value.background_light_argb, Some(0xff11_2233));
    assert_eq!(value.background_dark_argb, Some(0xff44_5566));
    let canonical = [
        0x0d, 0x33, 0x22, 0x11, 0xff, 0x0a, 3, b'r', b'e', b'd', 0x15, 0x66, 0x55, 0x44, 0xff,
        0x12, 4, b'b', b'l', b'u', b'e',
    ];
    assert_eq!(value.encode_bytes(), canonical);
}
