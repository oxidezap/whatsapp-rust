#![deny(unsafe_code)]

pub use bytes;

mod decode;
pub mod encoding;
mod enumeration;
mod error;
mod message;
mod message_field;
pub mod open_enum_serde;
#[allow(unsafe_code)]
mod owned_view;
mod sink;
mod size_cache;
pub mod types;
pub mod view;
pub use owned_view::{OwnedView, OwnedViewSource};

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
pub use view::{
    DefaultViewInstance, HasMessageView, MapView, MessageFieldView, MessageView, RepeatedView,
    ViewEncode, ViewReborrow,
};

#[allow(
    non_camel_case_types,
    non_snake_case,
    unused_qualifications,
    unused_imports,
    clippy::derivable_impls,
    clippy::disallowed_methods,
    clippy::let_and_return
)]
pub mod generated {
    pub mod whatsapp {
        include!("../generated/whatsapp.rs");
    }
}
