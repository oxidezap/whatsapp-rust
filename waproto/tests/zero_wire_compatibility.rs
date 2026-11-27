extern crate alloc;

#[allow(dead_code)]
#[path = "../src/protobuf/indexed_wire/mod.rs"]
mod zero_wire;

use waproto::protobuf::encoding::{Tag, skip_field_depth};

#[test]
fn zero_wire_errors_keep_their_messages() {
    use zero_wire::DecodeError::*;
    for (error, expected) in [
        (Truncated, "truncated protobuf varint"),
        (VarintOverflow, "protobuf varint exceeds 64 bits"),
        (InvalidTag, "invalid protobuf field number"),
        (InvalidWireType, "invalid protobuf wire type"),
        (UnexpectedEndGroup, "unexpected protobuf end group"),
        (MismatchedEndGroup, "mismatched protobuf end group"),
        (LimitExceeded, "protobuf decoding limit exceeded"),
    ] {
        assert_eq!(error.to_string(), expected);
    }
}

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
fn zero_reader_retains_original_tag_width() {
    for (input, width) in [
        (&[0x08, 1][..], 1),
        (&[0x88, 0, 1][..], 2),
        (&[0x82, 0xb5, 0x18, 0][..], 3),
    ] {
        let field = zero_wire::Reader::new(input, zero_wire::Limits::default())
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(field.tag_len, width);
        assert_eq!(field.raw, input);
    }
}

#[test]
fn varint_encoders_match_reference_and_append_to_sinks() {
    for value in [0, 1, 127, 128, 16_383, 16_384, u32::MAX as u64, u64::MAX] {
        let mut reference = vec![42];
        buffa::encoding::encode_varint(value, &mut reference);
        let mut vector = vec![42];
        waproto::protobuf::encoding::encode_varint(value, &mut vector);
        let mut sink = bytes::BytesMut::from(&[42][..]);
        waproto::protobuf::encoding::put_varint(value, &mut sink);
        let mut zero = vec![42];
        zero_wire::encode_varint(value, &mut zero);
        assert_eq!(vector, reference);
        assert_eq!(sink.as_ref(), reference);
        assert_eq!(zero, reference);
    }
}

#[test]
fn zero_writer_encodes_each_wire_value() {
    let mut output = vec![42];
    let mut writer = zero_wire::Writer::new(&mut output);
    writer.varint(zero_wire::FieldNumber::new(1).unwrap(), 150);
    writer.fixed64(
        zero_wire::FieldNumber::new(2).unwrap(),
        0x0807_0605_0403_0201,
    );
    writer.bytes(zero_wire::FieldNumber::new(3).unwrap(), b"hi");
    writer.fixed32(zero_wire::FieldNumber::new(4).unwrap(), 0x0403_0201);
    assert_eq!(
        output,
        [
            42, 8, 0x96, 1, 0x11, 1, 2, 3, 4, 5, 6, 7, 8, 0x1a, 2, b'h', b'i', 0x25, 1, 2, 3, 4
        ]
    );
}

#[test]
fn zero_writer_adapter_supports_existing_sinks() {
    use waproto::protobuf::encoding::{
        put_bytes_field, put_fixed32_field, put_fixed32_value, put_fixed64_field,
        put_fixed64_value, put_varint_field,
    };
    fn write(output: &mut impl waproto::protobuf::EncodeSink) {
        put_varint_field(1, 150, output);
        put_fixed64_field(2, 0x0807_0605_0403_0201, output);
        put_bytes_field(3, b"hi", output);
        put_fixed32_field(4, 0x0403_0201, output);
    }
    let expected = [
        42, 8, 0x96, 1, 0x11, 1, 2, 3, 4, 5, 6, 7, 8, 0x1a, 2, b'h', b'i', 0x25, 1, 2, 3, 4,
    ];
    let mut vector = vec![42];
    write(&mut vector);
    assert_eq!(vector, expected);
    let mut bytes = bytes::BytesMut::from(&[42][..]);
    write(&mut bytes);
    assert_eq!(bytes.as_ref(), expected);
    struct CustomSink(Vec<u8>);
    impl waproto::protobuf::EncodeSink for CustomSink {
        fn put_u8(&mut self, value: u8) {
            self.0.push(value);
        }
        fn put_slice(&mut self, value: &[u8]) {
            self.0.extend_from_slice(value);
        }
        fn put_u32_le(&mut self, value: u32) {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
        fn put_u64_le(&mut self, value: u64) {
            self.0.extend_from_slice(&value.to_le_bytes());
        }
    }
    let mut custom = CustomSink(vec![42]);
    write(&mut custom);
    assert_eq!(custom.0, expected);
    let mut packed = Vec::new();
    put_fixed32_value(0x0403_0201, &mut packed);
    put_fixed64_value(0x0807_0605_0403_0201, &mut packed);
    assert_eq!(packed, [1, 2, 3, 4, 1, 2, 3, 4, 5, 6, 7, 8]);
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
    assert!(matches!(
        DecodeOptions::new()
            .with_unknown_field_limit(0)
            .decode_from_slice::<waproto::whatsapp::Message>(&input),
        Err(DecodeError::UnknownFieldLimitExceeded)
    ));
}
