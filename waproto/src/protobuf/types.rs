#![forbid(unsafe_code)]

use super::{DecodeError, EncodeSink, encoding};
use bytes::Buf;

pub fn decode_bytes(input: &mut impl Buf) -> Result<Vec<u8>, DecodeError> {
    let length = usize::try_from(encoding::decode_varint(input)?)
        .map_err(|_| DecodeError::MessageTooLarge)?;
    if input.remaining() < length {
        return Err(DecodeError::UnexpectedEof);
    }
    let mut output = Vec::with_capacity(length);
    let mut remaining = length;
    while remaining != 0 {
        let count = remaining.min(input.chunk().len());
        if count == 0 {
            return Err(DecodeError::UnexpectedEof);
        }
        output.extend_from_slice(&input.chunk()[..count]);
        input.advance(count);
        remaining -= count;
    }
    Ok(output)
}

pub fn decode_shared_bytes(input: &mut impl Buf) -> Result<bytes::Bytes, DecodeError> {
    let length = usize::try_from(encoding::decode_varint(input)?)
        .map_err(|_| DecodeError::MessageTooLarge)?;
    if input.remaining() < length {
        return Err(DecodeError::UnexpectedEof);
    }
    Ok(input.copy_to_bytes(length))
}

pub fn decode_string(input: &mut impl Buf) -> Result<String, DecodeError> {
    let length = usize::try_from(encoding::decode_varint(input)?)
        .map_err(|_| DecodeError::MessageTooLarge)?;
    if input.remaining() < length {
        return Err(DecodeError::UnexpectedEof);
    }
    if input.chunk().len() >= length {
        let result = std::str::from_utf8(&input.chunk()[..length])
            .map(str::to_owned)
            .map_err(|_| DecodeError::InvalidUtf8);
        input.advance(length);
        return result;
    }
    let mut output = Vec::with_capacity(length);
    let mut remaining = length;
    while remaining != 0 {
        let count = remaining.min(input.chunk().len());
        if count == 0 {
            return Err(DecodeError::UnexpectedEof);
        }
        output.extend_from_slice(&input.chunk()[..count]);
        input.advance(count);
        remaining -= count;
    }
    String::from_utf8(output).map_err(|_| DecodeError::InvalidUtf8)
}

pub fn decode_fixed32(input: &mut impl Buf) -> Result<u32, DecodeError> {
    if input.remaining() < 4 {
        return Err(DecodeError::UnexpectedEof);
    }
    Ok(input.get_u32_le())
}
pub fn decode_fixed64(input: &mut impl Buf) -> Result<u64, DecodeError> {
    if input.remaining() < 8 {
        return Err(DecodeError::UnexpectedEof);
    }
    Ok(input.get_u64_le())
}

#[inline]
pub fn put_bytes(field: u32, value: &[u8], output: &mut impl EncodeSink) {
    encoding::put_varint((u64::from(field) << 3) | 2, output);
    encoding::put_varint(value.len() as u64, output);
    output.put_slice(value);
}

#[inline]
pub fn bytes_size(field: u32, length: usize) -> u64 {
    encoding::varint_len((u64::from(field) << 3) | 2) as u64
        + encoding::varint_len(length as u64) as u64
        + length as u64
}

pub fn borrow_bytes<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], DecodeError> {
    let length = usize::try_from(encoding::decode_varint(input)?)
        .map_err(|_| DecodeError::MessageTooLarge)?;
    if input.len() < length {
        return Err(DecodeError::UnexpectedEof);
    }
    let (value, tail) = input.split_at(length);
    *input = tail;
    Ok(value)
}

pub fn borrow_str<'a>(input: &mut &'a [u8]) -> Result<&'a str, DecodeError> {
    std::str::from_utf8(borrow_bytes(input)?).map_err(|_| DecodeError::InvalidUtf8)
}

pub fn borrow_group<'a>(
    input: &mut &'a [u8],
    number: u32,
    depth: u32,
) -> Result<&'a [u8], DecodeError> {
    let original = *input;
    loop {
        let before = input.len();
        let tag = encoding::Tag::decode(input)?;
        if tag.wire_type() == encoding::WireType::EndGroup {
            if tag.field_number() != number {
                return Err(DecodeError::InvalidEndGroup(tag.field_number()));
            }
            return Ok(&original[..original.len() - before]);
        }
        encoding::skip_field_depth(tag, input, depth)?;
    }
}
