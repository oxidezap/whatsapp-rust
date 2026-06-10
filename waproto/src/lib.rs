// This module contains the auto-generated protobuf definitions.
// The code is generated from `whatsapp.proto` and checked into version control.
// To regenerate, run `cargo build -p waproto --features generate`.
// See `build.rs` for the full proto compilation config.

#![allow(clippy::large_enum_variant)]
pub mod whatsapp {
    #[rustfmt::skip]
    include!("whatsapp.rs");
}

/// Wire tags of every message field in `whatsapp.proto`, generated alongside
/// the prost code. Hand-written partial decoders must reference these consts
/// (or compile-time assert against them) instead of magic numbers, so schema
/// changes surface as compile errors rather than silent wire-format drift.
pub mod tags {
    #[rustfmt::skip]
    include!("tags.rs");
}
