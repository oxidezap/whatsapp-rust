#![deny(unsafe_code)]

mod decode;
pub mod encoding;
mod enumeration;
mod error;
mod message;
mod message_field;
mod sink;
mod size_cache;

pub use decode::{
    DEFAULT_ELEMENT_MEMORY_LIMIT, DEFAULT_UNKNOWN_FIELD_LIMIT, DecodeContext, DecodeOptions,
    MAX_MESSAGE_BYTES, RECURSION_LIMIT,
};
pub use enumeration::{EnumValue, Enumeration};
pub use error::{DecodeError, EncodeError};
pub use message::{Message, MessageName, checked_encode_size, encode_size_overflow, saturate_size};
pub use message_field::{DefaultInstance, Inline, MessageField, ProtoBox};
pub use sink::EncodeSink;
pub use size_cache::SizeCache;
