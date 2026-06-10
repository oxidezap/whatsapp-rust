// Protobuf definitions, generated at build time into OUT_DIR from the
// committed `whatsapp.desc` descriptor (see build.rs). Consumers never need
// `protoc`; editing `whatsapp.proto` requires regenerating the descriptor via
// `scripts/regenerate-proto-desc.sh` and committing both files.

#![allow(clippy::large_enum_variant)]
pub mod whatsapp {
    include!(concat!(env!("OUT_DIR"), "/whatsapp.rs"));
}

/// Wire tags of every message field in `whatsapp.proto`, generated alongside
/// the prost code. Hand-written partial decoders must reference these consts
/// (or compile-time assert against them) instead of magic numbers, so schema
/// changes surface as compile errors rather than silent wire-format drift.
pub mod tags {
    include!(concat!(env!("OUT_DIR"), "/tags.rs"));
}
