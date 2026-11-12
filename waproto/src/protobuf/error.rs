#![forbid(unsafe_code)]

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeError {
    UnexpectedEof,
    VarintTooLong,
    InvalidWireType(u32),
    InvalidFieldNumber,
    MessageTooLarge,
    WireTypeMismatch {
        field_number: u32,
        expected: u8,
        actual: u8,
    },
    InvalidUtf8,
    RecursionLimitExceeded,
    InvalidEndGroup(u32),
    InvalidMessageSet(&'static str),
    UnknownFieldLimitExceeded,
    ElementMemoryLimitExceeded,
    Custom(&'static str),
}

impl fmt::Display for DecodeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => output.write_str("unexpected end of buffer"),
            Self::VarintTooLong => output.write_str("varint exceeded maximum length of 10 bytes"),
            Self::InvalidWireType(value) => write!(output, "invalid wire type: {value}"),
            Self::InvalidFieldNumber => output.write_str("invalid field number"),
            Self::MessageTooLarge => output.write_str("message length exceeds the size limit (2 GiB protobuf maximum, or a configured DecodeOptions limit)"),
            Self::WireTypeMismatch { field_number, expected, actual } => write!(output, "wire type mismatch on field {field_number}: expected {expected}, got {actual}"),
            Self::InvalidUtf8 => output.write_str("invalid UTF-8 in string field"),
            Self::RecursionLimitExceeded => output.write_str("recursion limit exceeded"),
            Self::InvalidEndGroup(number) => write!(output, "invalid end-group tag: field number {number}"),
            Self::InvalidMessageSet(reason) => write!(output, "invalid MessageSet item: {reason}"),
            Self::UnknownFieldLimitExceeded => output.write_str("unknown field limit exceeded"),
            Self::ElementMemoryLimitExceeded => output.write_str("element memory limit exceeded"),
            Self::Custom(reason) => write!(output, "custom representation rejected the field value: {reason}"),
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EncodeError {
    MessageTooLarge,
    ExceedsBudget { len: u32, max_bytes: u32 },
}

impl fmt::Display for EncodeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MessageTooLarge => {
                output.write_str("message encoded size exceeds the 2 GiB protobuf limit")
            }
            Self::ExceedsBudget { len, max_bytes } => write!(
                output,
                "message encoded size {len} exceeds the caller budget of {max_bytes} bytes"
            ),
        }
    }
}

impl std::error::Error for EncodeError {}
