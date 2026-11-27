extern crate alloc;

#[allow(dead_code)]
#[path = "../src/protobuf/indexed_wire/mod.rs"]
mod zero_wire;

use waproto::protobuf::encoding::{Tag, skip_field_depth};

fn reference_fields(mut input: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, ()> {
    let mut fields = Vec::new();
    while !input.is_empty() {
        let before = input;
        let tag = Tag::decode(&mut input).map_err(|_| ())?;
        skip_field_depth(tag, &mut input, 64).map_err(|_| ())?;
        fields.push((
            tag.field_number(),
            before[..before.len() - input.len()].to_vec(),
        ));
    }
    Ok(fields)
}

fn zero_fields(input: &[u8]) -> Result<Vec<(u32, Vec<u8>)>, ()> {
    zero_wire::Reader::new(input, zero_wire::Limits::default())
        .map_err(|_| ())?
        .map(|field| {
            field
                .map(|field| (field.number.get(), field.raw.to_vec()))
                .map_err(|_| ())
        })
        .collect()
}

#[test]
fn wire_reader_matches_generated_decoder_on_field_framing() {
    let cases: &[&[u8]] = &[
        &[],
        &[0x08, 0],
        &[0x08, 0x80, 0x01],
        &[0x11, 1, 2, 3, 4, 5, 6, 7, 8],
        &[0x1a, 3, 1, 2, 3],
        &[0x25, 1, 2, 3, 4],
        &[0x2b, 0x30, 1, 0x2c],
        &[0x08, 1, 0x12, 0, 0x1d, 1, 2, 3, 4],
        &[0],
        &[0x08],
        &[0x1a, 2, 1],
        &[0x2b, 0x34],
        &[0x0f],
        &[0x80; 10],
    ];
    for input in cases {
        assert_eq!(zero_fields(input), reference_fields(input), "{input:?}");
    }
}

#[test]
fn wire_reader_matches_generated_decoder_on_random_input() {
    let mut state = 0xdad1_4e73_02c9_4c51u64;
    for length in 0..=32 {
        for _ in 0..1024 {
            let input: Vec<u8> = (0..length)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state as u8
                })
                .collect();
            assert_eq!(zero_fields(&input), reference_fields(&input), "{input:?}");
        }
    }
}

#[test]
fn zero_reader_enforces_field_and_group_limits() {
    let limits = zero_wire::Limits {
        max_fields: 1,
        max_group_depth: 1,
        ..zero_wire::Limits::default()
    };
    let mut reader = zero_wire::Reader::new(&[0x08, 1, 0x10, 2], limits).unwrap();
    assert_eq!(reader.next().unwrap().unwrap().number.get(), 1);
    assert_eq!(
        reader.next().unwrap().unwrap_err(),
        zero_wire::DecodeError::LimitExceeded
    );
    assert!(reader.next().is_none());

    let limits = zero_wire::Limits {
        max_fields: 8,
        ..limits
    };
    let mut reader = zero_wire::Reader::new(&[0x0b, 0x13, 0x14, 0x0c], limits).unwrap();
    assert_eq!(
        reader.next().unwrap().unwrap_err(),
        zero_wire::DecodeError::LimitExceeded
    );
    assert!(reader.next().is_none());
}

#[test]
fn borrowed_length_delimited_fields_preserve_cursor_on_errors() {
    use waproto::protobuf::{DecodeError, types::borrow_bytes};
    for (input, expected, remaining) in [
        (&[0][..], Ok(&[][..]), &[][..]),
        (&[2, 7, 8, 9][..], Ok(&[7, 8][..]), &[9][..]),
        (&[2, 7][..], Err(DecodeError::UnexpectedEof), &[7][..]),
        (&[0x80][..], Err(DecodeError::UnexpectedEof), &[][..]),
    ] {
        let mut cursor = input;
        assert_eq!(borrow_bytes(&mut cursor), expected);
        assert_eq!(cursor, remaining);
    }
}

#[test]
fn unknown_field_budget_precedes_payload_validation() {
    use waproto::protobuf::{DecodeError, DecodeOptions};
    let input = [0x82, 0xb5, 0x18, 2, 1];
    assert!(zero_fields(&input).is_err());
    assert!(matches!(
        DecodeOptions::new()
            .with_unknown_field_limit(0)
            .decode_view::<waproto::whatsapp::MessageView<'_>>(&input),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}
