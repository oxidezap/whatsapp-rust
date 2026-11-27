use alloc::vec::Vec;
mod wire;
pub use wire::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("truncated protobuf varint")]
    Truncated,
    #[error("protobuf varint exceeds 64 bits")]
    VarintOverflow,
    #[error("invalid protobuf field number")]
    InvalidTag,
    #[error("invalid protobuf wire type")]
    InvalidWireType,
    #[error("unexpected protobuf end group")]
    UnexpectedEndGroup,
    #[error("mismatched protobuf end group")]
    MismatchedEndGroup,
    #[error("protobuf decoding limit exceeded")]
    LimitExceeded,
}

#[inline]
pub(crate) fn encode_varint_with(mut value: u64, mut push: impl FnMut(u8)) {
    while value >= 128 {
        push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    push(value as u8);
}

pub fn encode_varint(value: u64, output: &mut Vec<u8>) {
    encode_varint_with(value, |byte| output.push(byte));
}

/// Returns the value and consumed byte count without changing the input.
pub fn decode_varint(input: &[u8]) -> Result<(u64, usize), DecodeError> {
    let mut value = 0u64;
    for index in 0..10 {
        let byte = *input.get(index).ok_or(DecodeError::Truncated)?;
        if index == 9 && byte > 1 {
            return Err(DecodeError::VarintOverflow);
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    Err(DecodeError::VarintOverflow)
}

#[cold]
pub(crate) fn decode_varint_buffer(input: &mut impl bytes::Buf) -> Result<u64, DecodeError> {
    let mut value = 0u64;
    for index in 0..10 {
        if !input.has_remaining() {
            return Err(DecodeError::Truncated);
        }
        let byte = input.get_u8();
        if index == 9 && byte > 1 {
            return Err(DecodeError::VarintOverflow);
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(DecodeError::VarintOverflow)
}
