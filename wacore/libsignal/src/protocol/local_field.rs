//! Shared decoder for the local-only reservation fields.
//!
//! Both the DM session record and the group sender-key record durably reserve
//! a batch of counters/iterations by appending a `u32` as a top-level protobuf
//! field the generated structures do not know about. The generated views skip
//! unknown fields, so the value is recovered by scanning the raw top-level
//! stream. Keeping that scan in one place stops the DM and group paths from
//! drifting into different validation rules for the same class of bug.

use buffa::encoding::{Tag, WireType, decode_varint, skip_field};

/// Scan a local-only `u32` varint field out of a raw protobuf byte stream.
///
/// Fails closed via `on_err`: anything unreadable (a bad tag, a non-varint wire
/// type under `field_number`, a duplicate, or an out-of-range value) is an error
/// rather than a skip. Skipping would silently yield 0, disabling the load-time
/// fast-forward and re-enabling already-spent counters/iterations, a reused
/// (key, IV) pair, the exact outcome the reservation exists to prevent. A load
/// error is recoverable (the record rebuilds); nonce reuse is not.
///
/// `on_err` is generic so each record type keeps its own error type; the caller
/// closes over a single fail-closed error for the whole scan.
pub(crate) fn decode_local_only_u32_field<E>(
    mut bytes: &[u8],
    field_number: u32,
    on_err: impl Fn() -> E,
) -> Result<u32, E> {
    let mut value = None;
    while !bytes.is_empty() {
        let tag = Tag::decode(&mut bytes).map_err(|_| on_err())?;
        if tag.field_number() == field_number {
            if tag.wire_type() != WireType::Varint || value.is_some() {
                return Err(on_err());
            }
            let raw = decode_varint(&mut bytes).map_err(|_| on_err())?;
            value = Some(u32::try_from(raw).map_err(|_| on_err())?);
        } else {
            skip_field(tag, &mut bytes).map_err(|_| on_err())?;
        }
    }
    Ok(value.unwrap_or(0))
}
