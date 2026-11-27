#![forbid(unsafe_code)]

use super::{DecodeError, MAX_MESSAGE_BYTES};

pub const MAX_FIELD_NUMBER: u32 = (1 << 29) - 1;
pub const FIRST_RESERVED_FIELD_NUMBER: u32 = 19_000;
pub const LAST_RESERVED_FIELD_NUMBER: u32 = 19_999;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum WireType {
    Varint = 0,
    Fixed64 = 1,
    LengthDelimited = 2,
    StartGroup = 3,
    EndGroup = 4,
    Fixed32 = 5,
}

impl WireType {
    pub fn from_u32(value: u32) -> Result<Self, DecodeError> {
        match value {
            0 => Ok(Self::Varint),
            1 => Ok(Self::Fixed64),
            2 => Ok(Self::LengthDelimited),
            3 => Ok(Self::StartGroup),
            4 => Ok(Self::EndGroup),
            5 => Ok(Self::Fixed32),
            other => Err(DecodeError::InvalidWireType(other)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tag {
    number: u32,
    kind: WireType,
}

impl Tag {
    #[inline]
    pub fn new(field_number: u32, wire_type: WireType) -> Self {
        assert!(
            (1..=MAX_FIELD_NUMBER).contains(&field_number),
            "invalid protobuf field number"
        );
        Self {
            number: field_number,
            kind: wire_type,
        }
    }
    #[inline]
    pub fn field_number(&self) -> u32 {
        self.number
    }
    #[inline]
    pub fn wire_type(&self) -> WireType {
        self.kind
    }
    pub fn from_raw(value: u64) -> Result<Self, DecodeError> {
        let value = u32::try_from(value).map_err(|_| DecodeError::InvalidFieldNumber)?;
        let kind = WireType::from_u32(value & 7)?;
        let number = value >> 3;
        if number == 0 {
            return Err(DecodeError::InvalidFieldNumber);
        }
        Ok(Self { number, kind })
    }
    pub fn decode(input: &mut impl bytes::Buf) -> Result<Self, DecodeError> {
        if let Some(&byte) = input.chunk().first()
            && byte < 128
        {
            input.advance(1);
            return Self::from_raw(u64::from(byte));
        }
        Self::from_raw(decode_varint(input)?)
    }
    #[inline]
    pub fn encode(&self, output: &mut impl super::EncodeSink) {
        put_varint((u64::from(self.number) << 3) | self.kind as u64, output);
    }
}

pub(crate) fn probe_tag(input: &[u8]) -> Option<(Tag, usize)> {
    use super::indexed_wire::{Limits, Reader, Value};
    let limits = Limits {
        max_message_bytes: MAX_MESSAGE_BYTES as usize,
        max_fields: 1,
        max_group_depth: 0,
    };
    let field = Reader::new(input, limits).ok()?.next()?.ok()?;
    let kind = match field.value {
        Value::Varint(_) => WireType::Varint,
        Value::Fixed64(_) => WireType::Fixed64,
        Value::Bytes(_) => WireType::LengthDelimited,
        Value::Fixed32(_) => WireType::Fixed32,
        Value::Group(_) => return None,
    };
    Some((Tag::new(field.number.get(), kind), field.tag_len))
}

#[inline]
pub fn check_wire_type(tag: Tag, expected: WireType) -> Result<(), DecodeError> {
    if tag.kind != expected {
        return Err(wire_type_mismatch(tag, expected));
    }
    Ok(())
}

#[cold]
pub fn wire_type_mismatch(tag: Tag, expected: WireType) -> DecodeError {
    DecodeError::WireTypeMismatch {
        field_number: tag.number,
        expected: expected as u8,
        actual: tag.kind as u8,
    }
}

pub struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8]) -> Result<Self, DecodeError> {
        if input.len() > MAX_MESSAGE_BYTES as usize {
            return Err(DecodeError::MessageTooLarge);
        }
        Ok(Self { remaining: input })
    }
    pub fn remaining(&self) -> &'a [u8] {
        self.remaining
    }
    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }
    #[inline]
    pub fn varint(&mut self) -> Result<u64, DecodeError> {
        decode_varint(&mut self.remaining)
    }
    #[inline]
    pub fn tag(&mut self) -> Result<Tag, DecodeError> {
        Tag::from_raw(self.varint()?)
    }
    #[inline]
    pub fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        if len > self.remaining.len() {
            return Err(DecodeError::UnexpectedEof);
        }
        let (value, tail) = self.remaining.split_at(len);
        self.remaining = tail;
        Ok(value)
    }
    #[inline]
    pub fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = usize::try_from(self.varint()?).map_err(|_| DecodeError::MessageTooLarge)?;
        self.take(len)
    }
    pub fn message(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.varint()?;
        if len > u64::from(MAX_MESSAGE_BYTES) {
            return Err(DecodeError::MessageTooLarge);
        }
        self.take(len as usize)
    }
    #[inline]
    pub fn string(&mut self) -> Result<&'a str, DecodeError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| DecodeError::InvalidUtf8)
    }
    #[inline]
    pub fn fixed32(&mut self) -> Result<u32, DecodeError> {
        Ok(u32::from_le_bytes(
            self.take(4)?
                .try_into()
                .map_err(|_| DecodeError::UnexpectedEof)?,
        ))
    }
    #[inline]
    pub fn fixed64(&mut self) -> Result<u64, DecodeError> {
        Ok(u64::from_le_bytes(
            self.take(8)?
                .try_into()
                .map_err(|_| DecodeError::UnexpectedEof)?,
        ))
    }
    #[inline]
    pub fn sint32(&mut self) -> Result<i32, DecodeError> {
        let value = self.varint()? as u32;
        Ok((value >> 1) as i32 ^ -((value & 1) as i32))
    }
    pub fn skip(&mut self, tag: Tag, depth: u32) -> Result<(), DecodeError> {
        match tag.kind {
            WireType::Varint => {
                self.varint()?;
            }
            WireType::Fixed64 => {
                self.take(8)?;
            }
            WireType::LengthDelimited => {
                self.bytes()?;
            }
            WireType::Fixed32 => {
                self.take(4)?;
            }
            WireType::EndGroup => return Err(DecodeError::InvalidWireType(4)),
            WireType::StartGroup => {
                let depth = depth
                    .checked_sub(1)
                    .ok_or(DecodeError::RecursionLimitExceeded)?;
                loop {
                    let child = self.tag()?;
                    if child.kind == WireType::EndGroup {
                        if child.number != tag.number {
                            return Err(DecodeError::InvalidEndGroup(child.number));
                        }
                        break;
                    }
                    self.skip(child, depth)?;
                }
            }
        }
        Ok(())
    }
    pub fn group(&mut self, field_number: u32, depth: u32) -> Result<&'a [u8], DecodeError> {
        let original = self.remaining;
        loop {
            let before = self.remaining.len();
            let tag = self.tag()?;
            if tag.kind == WireType::EndGroup {
                if tag.number != field_number {
                    return Err(DecodeError::InvalidEndGroup(tag.number));
                }
                return Ok(&original[..original.len() - before]);
            }
            self.skip(tag, depth)?;
        }
    }
}

#[inline]
pub const fn varint_len(value: u64) -> usize {
    if value < 128 {
        1
    } else {
        (64 - value.leading_zeros()).div_ceil(7) as usize
    }
}

#[inline]
pub fn encode_varint(value: u64, output: &mut Vec<u8>) {
    super::indexed_wire::encode_varint(value, output);
}

#[inline]
pub fn put_varint(value: u64, output: &mut impl super::EncodeSink) {
    super::indexed_wire::encode_varint_with(value, |byte| output.put_u8(byte));
}

#[inline]
pub fn decode_varint(input: &mut impl bytes::Buf) -> Result<u64, DecodeError> {
    let chunk = input.chunk();
    let first = *chunk.first().ok_or(DecodeError::UnexpectedEof)?;
    if first < 128 {
        input.advance(1);
        return Ok(u64::from(first));
    }
    if chunk.len() > 10 || chunk.last().is_some_and(|byte| *byte < 128) {
        let (value, length) = super::indexed_wire::decode_varint(chunk).map_err(varint_error)?;
        input.advance(length);
        return Ok(value);
    }
    super::indexed_wire::decode_varint_buffer(input).map_err(varint_error)
}

fn varint_error(error: super::indexed_wire::DecodeError) -> DecodeError {
    match error {
        super::indexed_wire::DecodeError::Truncated => DecodeError::UnexpectedEof,
        super::indexed_wire::DecodeError::VarintOverflow => DecodeError::VarintTooLong,
        _ => unreachable!("varint decoder returned a field error"),
    }
}

pub fn skip_field_depth(
    tag: Tag,
    input: &mut impl bytes::Buf,
    depth: u32,
) -> Result<(), DecodeError> {
    if let Some(length) =
        super::indexed_wire::probe_value(input.chunk(), tag.number, tag.kind as u8)
    {
        input.advance(length);
        return Ok(());
    }
    let count = match tag.kind {
        WireType::Varint => {
            decode_varint(input)?;
            return Ok(());
        }
        WireType::Fixed64 => 8,
        WireType::Fixed32 => 4,
        WireType::LengthDelimited => return skip_length_delimited(input),
        WireType::EndGroup => return Err(DecodeError::InvalidWireType(4)),
        WireType::StartGroup => {
            let depth = depth
                .checked_sub(1)
                .ok_or(DecodeError::RecursionLimitExceeded)?;
            loop {
                let child = Tag::decode(input)?;
                if child.kind == WireType::EndGroup {
                    if child.number != tag.number {
                        return Err(DecodeError::InvalidEndGroup(child.number));
                    }
                    return Ok(());
                }
                skip_field_depth(child, input, depth)?;
            }
        }
    };
    if input.remaining() < count {
        return Err(DecodeError::UnexpectedEof);
    }
    input.advance(count);
    Ok(())
}

#[inline]
fn skip_length_delimited(input: &mut impl bytes::Buf) -> Result<(), DecodeError> {
    let length =
        usize::try_from(decode_varint(input)?).map_err(|_| DecodeError::MessageTooLarge)?;
    if input.remaining() < length {
        return Err(DecodeError::UnexpectedEof);
    }
    input.advance(length);
    Ok(())
}

pub fn skip_unknown_field(
    tag: Tag,
    input: &mut impl bytes::Buf,
    context: super::DecodeContext<'_>,
) -> Result<(), DecodeError> {
    context.register_unknown_field()?;
    if tag.kind != WireType::StartGroup {
        return skip_field_depth(tag, input, context.depth());
    }
    let child_context = context.descend()?;
    loop {
        let child = Tag::decode(input)?;
        if child.kind == WireType::EndGroup {
            return if child.number == tag.number {
                Ok(())
            } else {
                Err(DecodeError::InvalidEndGroup(child.number))
            };
        }
        skip_unknown_field(child, input, child_context)?;
    }
}

pub fn skip_field(tag: Tag, input: &mut impl bytes::Buf) -> Result<(), DecodeError> {
    skip_field_depth(tag, input, super::RECURSION_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_boundaries_and_cursor_match_reference() {
        for value in [0, 1, 127, 128, 16_383, 16_384, u32::MAX as u64, u64::MAX] {
            let mut input = Vec::new();
            encode_varint(value, &mut input);
            input.push(42);
            let mut reader = Reader::new(&input).unwrap();
            assert_eq!(reader.varint(), Ok(value));
            assert_eq!(reader.remaining(), &[42]);
            let mut buf = input.as_slice();
            assert_eq!(decode_varint(&mut buf), Ok(value));
            assert_eq!(buf, &[42]);
        }
        for input in [
            vec![0x80; 9],
            vec![0x80; 10],
            vec![0xff; 10],
            vec![0x80; 11],
        ] {
            let mut reference = input.as_slice();
            let mut reader = Reader::new(&input).unwrap();
            assert_eq!(
                reader.varint(),
                buffa::encoding::decode_varint(&mut reference).map_err(reference_error)
            );
            assert_eq!(reader.remaining(), reference);
        }
    }

    fn reference_error(error: buffa::DecodeError) -> DecodeError {
        match error {
            buffa::DecodeError::UnexpectedEof => DecodeError::UnexpectedEof,
            buffa::DecodeError::VarintTooLong => DecodeError::VarintTooLong,
            buffa::DecodeError::InvalidWireType(n) => DecodeError::InvalidWireType(n),
            buffa::DecodeError::InvalidFieldNumber => DecodeError::InvalidFieldNumber,
            buffa::DecodeError::MessageTooLarge => DecodeError::MessageTooLarge,
            other => panic!("unqualified primitive error: {other:?}"),
        }
    }

    #[test]
    fn malformed_varints_tags_and_cursor_consumption_match_the_reference() {
        let mut state = 0x6a31_17c3_001f_aa3bu64;
        for len in 0..=12 {
            for _ in 0..1024 {
                let input: Vec<u8> = (0..len)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        state as u8
                    })
                    .collect();
                let mut reference = input.as_slice();
                let mut candidate = Reader::new(&input).unwrap();
                assert_eq!(
                    candidate.varint(),
                    buffa::encoding::decode_varint(&mut reference).map_err(reference_error)
                );
                assert_eq!(candidate.remaining(), reference);
                let mut reference = input.as_slice();
                let mut candidate = input.as_slice();
                assert_eq!(
                    decode_varint(&mut candidate),
                    buffa::encoding::decode_varint(&mut reference).map_err(reference_error)
                );
                assert_eq!(candidate, reference);
                let mut reference = input.as_slice();
                let mut candidate = input.as_slice();
                let actual = Tag::decode(&mut candidate)
                    .map(|tag| (tag.field_number(), tag.wire_type() as u8));
                let expected = buffa::encoding::Tag::decode(&mut reference)
                    .map(|tag| (tag.field_number(), tag.wire_type() as u8))
                    .map_err(reference_error);
                assert_eq!(actual, expected);
                assert_eq!(candidate, reference);
                let mut reference = input.as_slice();
                let mut candidate = Reader::new(&input).unwrap();
                let actual = candidate
                    .tag()
                    .map(|tag| (tag.field_number(), tag.wire_type() as u8));
                let expected = buffa::encoding::Tag::decode(&mut reference)
                    .map(|tag| (tag.field_number(), tag.wire_type() as u8))
                    .map_err(reference_error);
                assert_eq!(actual, expected);
                assert_eq!(candidate.remaining(), reference);
            }
        }
    }

    #[test]
    fn fragmented_tags_match_the_reference() {
        use bytes::Buf as _;

        for (first, second) in [
            (&[0x08][..], &[0x2a][..]),
            (&[0x80][..], &[0x01, 0x2a][..]),
            (&[0x80][..], &[][..]),
            (&[0x00][..], &[0x2a][..]),
            (&[0x0f][..], &[0x2a][..]),
        ] {
            let mut candidate = first.chain(second);
            let mut reference = first.chain(second);
            let actual =
                Tag::decode(&mut candidate).map(|tag| (tag.field_number(), tag.wire_type() as u8));
            let expected = buffa::encoding::Tag::decode(&mut reference)
                .map(|tag| (tag.field_number(), tag.wire_type() as u8))
                .map_err(reference_error);
            assert_eq!(actual, expected);
            assert_eq!(candidate.remaining(), reference.remaining());
            while candidate.has_remaining() {
                assert_eq!(candidate.get_u8(), reference.get_u8());
            }
        }
    }

    #[test]
    fn fragmented_varints_match_the_reference_at_every_split() {
        use bytes::Buf as _;
        let cases: &[&[u8]] = &[
            &[],
            &[0],
            &[0x80, 1],
            &[0x80, 0],
            &[0x80],
            &[0x80; 10],
            &[0xff; 10],
            &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 1, 42],
        ];
        for input in cases {
            for split in 0..=input.len() {
                let mut candidate = input[..split].chain(&input[split..]);
                let mut reference = input[..split].chain(&input[split..]);
                assert_eq!(
                    decode_varint(&mut candidate),
                    buffa::encoding::decode_varint(&mut reference).map_err(reference_error)
                );
                assert_eq!(candidate.remaining(), reference.remaining());
                while candidate.has_remaining() {
                    assert_eq!(candidate.get_u8(), reference.get_u8());
                }
            }
        }
    }

    #[test]
    fn skipped_values_preserve_fragmented_input_and_error_order() {
        use bytes::Buf as _;
        let tag = Tag::new(7, WireType::Varint);
        let mut fragmented = (&[0x80][..]).chain(&[1, 42][..]);
        skip_field_depth(tag, &mut fragmented, 100).unwrap();
        assert_eq!(fragmented.remaining(), 1);
        assert_eq!(fragmented.get_u8(), 42);

        let tag = Tag::new(7, WireType::LengthDelimited);
        let mut input = &[2, 0xff][..];
        let unknown = std::cell::Cell::new(0);
        let context = super::super::DecodeContext::new(100, &unknown);
        assert_eq!(
            skip_unknown_field(tag, &mut input, context),
            Err(DecodeError::UnknownFieldLimitExceeded)
        );
        assert_eq!(input, &[2, 0xff]);
        unknown.set(1);
        assert_eq!(
            skip_unknown_field(tag, &mut input, context),
            Err(DecodeError::UnexpectedEof)
        );
        assert_eq!(input, &[0xff]);
    }

    #[test]
    fn skipped_complete_values_leave_the_following_field_untouched() {
        for (kind, bytes, consumed) in [
            (WireType::Varint, &[0x96, 1, 42][..], 2),
            (WireType::Fixed32, &[1, 2, 3, 4, 42][..], 4),
            (WireType::Fixed64, &[1, 2, 3, 4, 5, 6, 7, 8, 42][..], 8),
            (WireType::LengthDelimited, &[2, 7, 8, 42][..], 3),
        ] {
            let mut input = bytes;
            skip_field_depth(Tag::new(7, kind), &mut input, 100).unwrap();
            assert_eq!(input, &bytes[consumed..]);
        }
    }

    #[test]
    fn strings_consume_invalid_payloads_and_bytes_stay_borrowed() {
        let input = [1, 255, 0];
        let mut reader = Reader::new(&input).unwrap();
        assert_eq!(reader.string(), Err(DecodeError::InvalidUtf8));
        assert_eq!(reader.remaining(), &[0]);
        let mut reader = Reader::new(&input).unwrap();
        let bytes = reader.bytes().unwrap();
        assert_eq!(bytes.as_ptr(), input[1..].as_ptr());
    }

    #[test]
    fn signed_extremes_and_group_boundaries_are_preserved() {
        for value in [i32::MIN, -1, 0, 1, i32::MAX] {
            let raw = ((value as u32) << 1) ^ ((value >> 31) as u32);
            let mut bytes = Vec::new();
            encode_varint(u64::from(raw), &mut bytes);
            assert_eq!(Reader::new(&bytes).unwrap().sint32(), Ok(value));
        }
        let bytes = [8, 1, 0x14, 9];
        let mut reader = Reader::new(&bytes).unwrap();
        assert_eq!(reader.group(2, 100), Ok([8, 1].as_slice()));
        assert_eq!(reader.remaining(), &[9]);
        assert_eq!(
            Reader::new(&[0x1c]).unwrap().group(2, 100),
            Err(DecodeError::InvalidEndGroup(3))
        );
    }
}
