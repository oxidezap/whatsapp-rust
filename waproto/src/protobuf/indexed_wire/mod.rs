use alloc::vec::Vec;
use core::fmt;

mod wire;
pub use wire::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    Truncated,
    VarintOverflow,
    InvalidTag,
    InvalidWireType,
    UnexpectedEndGroup,
    MismatchedEndGroup,
    LimitExceeded,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Truncated => "truncated protobuf varint",
            Self::VarintOverflow => "protobuf varint exceeds 64 bits",
            Self::InvalidTag => "invalid protobuf field number",
            Self::InvalidWireType => "invalid protobuf wire type",
            Self::UnexpectedEndGroup => "unexpected protobuf end group",
            Self::MismatchedEndGroup => "mismatched protobuf end group",
            Self::LimitExceeded => "protobuf decoding limit exceeded",
        })
    }
}

impl core::error::Error for DecodeError {}

pub fn encode_varint(mut value: u64, output: &mut Vec<u8>) {
    while value >= 128 {
        output.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    output.push(value as u8);
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
