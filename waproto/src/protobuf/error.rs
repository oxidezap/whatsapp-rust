#![forbid(unsafe_code)]

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum DecodeError {
    #[error("unexpected end of buffer")]
    UnexpectedEof,
    #[error("varint exceeded maximum length of 10 bytes")]
    VarintTooLong,
    #[error("invalid wire type: {0}")]
    InvalidWireType(u32),
    #[error("invalid field number")]
    InvalidFieldNumber,
    #[error(
        "message length exceeds the size limit (2 GiB protobuf maximum, or a configured DecodeOptions limit)"
    )]
    MessageTooLarge,
    #[error("wire type mismatch on field {field_number}: expected {expected}, got {actual}")]
    WireTypeMismatch {
        field_number: u32,
        expected: u8,
        actual: u8,
    },
    #[error("invalid UTF-8 in string field")]
    InvalidUtf8,
    #[error("recursion limit exceeded")]
    RecursionLimitExceeded,
    #[error("invalid end-group tag: field number {0}")]
    InvalidEndGroup(u32),
    #[error("invalid MessageSet item: {0}")]
    InvalidMessageSet(&'static str),
    #[error("unknown field limit exceeded")]
    UnknownFieldLimitExceeded,
    #[error("element memory limit exceeded")]
    ElementMemoryLimitExceeded,
    #[error("custom representation rejected the field value: {0}")]
    Custom(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    #[error("message encoded size exceeds the 2 GiB protobuf limit")]
    MessageTooLarge,
    #[error("message encoded size {len} exceeds the caller budget of {max_bytes} bytes")]
    ExceedsBudget { len: u32, max_bytes: u32 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_match_the_reference() {
        let cases = [
            (
                DecodeError::UnexpectedEof,
                buffa::DecodeError::UnexpectedEof,
            ),
            (
                DecodeError::VarintTooLong,
                buffa::DecodeError::VarintTooLong,
            ),
            (
                DecodeError::InvalidWireType(7),
                buffa::DecodeError::InvalidWireType(7),
            ),
            (
                DecodeError::InvalidFieldNumber,
                buffa::DecodeError::InvalidFieldNumber,
            ),
            (
                DecodeError::MessageTooLarge,
                buffa::DecodeError::MessageTooLarge,
            ),
            (
                DecodeError::WireTypeMismatch {
                    field_number: 2,
                    expected: 1,
                    actual: 3,
                },
                buffa::DecodeError::WireTypeMismatch {
                    field_number: 2,
                    expected: 1,
                    actual: 3,
                },
            ),
            (DecodeError::InvalidUtf8, buffa::DecodeError::InvalidUtf8),
            (
                DecodeError::RecursionLimitExceeded,
                buffa::DecodeError::RecursionLimitExceeded,
            ),
            (
                DecodeError::InvalidEndGroup(4),
                buffa::DecodeError::InvalidEndGroup(4),
            ),
            (
                DecodeError::InvalidMessageSet("reason"),
                buffa::DecodeError::InvalidMessageSet("reason"),
            ),
            (
                DecodeError::UnknownFieldLimitExceeded,
                buffa::DecodeError::UnknownFieldLimitExceeded,
            ),
            (
                DecodeError::ElementMemoryLimitExceeded,
                buffa::DecodeError::ElementMemoryLimitExceeded,
            ),
            (
                DecodeError::Custom("reason"),
                buffa::DecodeError::Custom("reason"),
            ),
        ];
        for (actual, expected) in cases {
            assert_eq!(actual.to_string(), expected.to_string());
        }
        assert_eq!(
            EncodeError::MessageTooLarge.to_string(),
            buffa::EncodeError::MessageTooLarge.to_string()
        );
        assert_eq!(
            EncodeError::ExceedsBudget {
                len: 5,
                max_bytes: 3
            }
            .to_string(),
            buffa::EncodeError::ExceedsBudget {
                len: 5,
                max_bytes: 3
            }
            .to_string()
        );
    }
}
