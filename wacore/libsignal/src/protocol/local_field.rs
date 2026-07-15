//! Local record metadata fails closed so a corrupt lease cannot re-enable spent
//! counters.

use buffa::encoding::{Tag, WireType, decode_varint, encode_varint, skip_field};

const STORE_INCARNATION_FIELD: u32 = 101;
const STORE_INCARNATION_LEN: usize = 16;
pub(crate) const STORE_INCARNATION_ENCODED_LEN: usize = 19;

pub(crate) struct LocalRecordFields {
    pub(crate) reservation: u32,
    pub(crate) incarnation: Option<[u8; STORE_INCARNATION_LEN]>,
}

/// Malformed or duplicate metadata cannot be trusted to preserve a lease.
pub(crate) fn decode_local_record_fields<E>(
    mut bytes: &[u8],
    reservation_field: u32,
    on_err: impl Fn() -> E,
) -> Result<LocalRecordFields, E> {
    let mut reservation = None;
    let mut incarnation = None;
    let mut incarnation_valid = true;
    while !bytes.is_empty() {
        let tag = Tag::decode(&mut bytes).map_err(|_| on_err())?;
        if tag.field_number() == reservation_field {
            if tag.wire_type() != WireType::Varint || reservation.is_some() {
                return Err(on_err());
            }
            let raw = decode_varint(&mut bytes).map_err(|_| on_err())?;
            reservation = Some(u32::try_from(raw).map_err(|_| on_err())?);
        } else if tag.field_number() == STORE_INCARNATION_FIELD {
            if tag.wire_type() == WireType::LengthDelimited {
                let len = usize::try_from(decode_varint(&mut bytes).map_err(|_| on_err())?)
                    .map_err(|_| on_err())?;
                if bytes.len() < len {
                    return Err(on_err());
                }
                if len == STORE_INCARNATION_LEN && incarnation.is_none() && incarnation_valid {
                    let mut value = [0; STORE_INCARNATION_LEN];
                    value.copy_from_slice(&bytes[..len]);
                    incarnation = Some(value);
                } else {
                    incarnation = None;
                    incarnation_valid = false;
                }
                bytes = &bytes[len..];
            } else {
                skip_field(tag, &mut bytes).map_err(|_| on_err())?;
                incarnation = None;
                incarnation_valid = false;
            }
        } else {
            skip_field(tag, &mut bytes).map_err(|_| on_err())?;
        }
    }
    Ok(LocalRecordFields {
        reservation: reservation.unwrap_or(0),
        incarnation: if incarnation_valid { incarnation } else { None },
    })
}

pub(crate) fn encode_store_incarnation(bytes: &mut Vec<u8>, incarnation: &[u8; 16]) {
    Tag::new(STORE_INCARNATION_FIELD, WireType::LengthDelimited).encode(bytes);
    encode_varint(STORE_INCARNATION_LEN as u64, bytes);
    bytes.extend_from_slice(incarnation);
}
