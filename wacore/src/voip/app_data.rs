//! RTC app-data payloads carried on group media PT 119.

const MAX_APP_DATA_BYTES: usize = 4096;
const MAX_REACTION_BYTES: usize = 256;
pub(crate) const APP_DATA_RTP_TIMESTAMP_STRIDE: u32 = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallReaction {
    pub transaction_id: u64,
    pub emoji: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppDataError {
    #[error("RTC app-data payload is malformed")]
    Malformed,
    #[error("RTC reaction payload is too large")]
    TooLarge,
    #[error("RTC reaction text is not valid UTF-8")]
    InvalidUtf8,
    #[error("RTC reaction transaction id must be non-zero")]
    MissingTransaction,
}

pub fn encode_reaction(transaction_id: u64, emoji: &str) -> Result<Vec<u8>, AppDataError> {
    if transaction_id == 0 {
        return Err(AppDataError::MissingTransaction);
    }
    if emoji.len() > MAX_REACTION_BYTES {
        return Err(AppDataError::TooLarge);
    }
    let mut reaction = Vec::with_capacity(emoji.len() + 8);
    append_varint_field(&mut reaction, 1, transaction_id);
    append_bytes_field(&mut reaction, 2, emoji.as_bytes());

    let mut message = Vec::with_capacity(reaction.len() + 4);
    append_bytes_field(&mut message, 1, &reaction);
    let mut payload = Vec::with_capacity(message.len() + 4);
    append_bytes_field(&mut payload, 1, &message);
    Ok(payload)
}

pub fn decode_reactions(payload: &[u8]) -> Result<Vec<CallReaction>, AppDataError> {
    if payload.len() > MAX_APP_DATA_BYTES {
        return Err(AppDataError::TooLarge);
    }
    let mut reactions = Vec::new();
    visit_fields(payload, |field, wire, value| {
        if field == 1 && wire == 2 {
            let message = bytes_value(value)?;
            if let Some(reaction) = decode_message(message)? {
                reactions.push(reaction);
            }
        }
        Ok(())
    })?;
    Ok(reactions)
}

fn decode_message(message: &[u8]) -> Result<Option<CallReaction>, AppDataError> {
    let mut result = None;
    visit_fields(message, |field, wire, value| {
        if result.is_none() && field == 1 && wire == 2 {
            result = Some(decode_reaction(bytes_value(value)?)?);
        }
        Ok(())
    })?;
    Ok(result)
}

fn decode_reaction(message: &[u8]) -> Result<CallReaction, AppDataError> {
    let mut transaction_id = 0;
    let mut emoji = String::new();
    visit_fields(message, |field, wire, value| {
        match (field, wire) {
            (1, 0) => transaction_id = varint_value(value)?,
            (2, 2) => {
                let bytes = bytes_value(value)?;
                if bytes.len() > MAX_REACTION_BYTES {
                    return Err(AppDataError::TooLarge);
                }
                emoji = core::str::from_utf8(bytes)
                    .map_err(|_| AppDataError::InvalidUtf8)?
                    .to_string();
            }
            _ => {}
        }
        Ok(())
    })?;
    if transaction_id == 0 {
        return Err(AppDataError::MissingTransaction);
    }
    Ok(CallReaction {
        transaction_id,
        emoji,
    })
}

/// Visits raw field values including their length/varint encoding so the decoder can stay tiny
/// while still skipping unknown protobuf fields.
fn visit_fields(
    mut data: &[u8],
    mut visit: impl FnMut(u64, u8, &[u8]) -> Result<(), AppDataError>,
) -> Result<(), AppDataError> {
    while !data.is_empty() {
        let (key, key_len) = consume_varint(data)?;
        data = &data[key_len..];
        let field = key >> 3;
        let wire = (key & 7) as u8;
        if field == 0 {
            return Err(AppDataError::Malformed);
        }
        let value_len = match wire {
            0 => consume_varint(data)?.1,
            1 => 8,
            2 => {
                let (len, prefix) = consume_varint(data)?;
                prefix
                    .checked_add(usize::try_from(len).map_err(|_| AppDataError::TooLarge)?)
                    .ok_or(AppDataError::TooLarge)?
            }
            5 => 4,
            _ => return Err(AppDataError::Malformed),
        };
        let (value, rest) = data
            .split_at_checked(value_len)
            .ok_or(AppDataError::Malformed)?;
        visit(field, wire, value)?;
        data = rest;
    }
    Ok(())
}

fn bytes_value(value: &[u8]) -> Result<&[u8], AppDataError> {
    let (len, prefix) = consume_varint(value)?;
    let len = usize::try_from(len).map_err(|_| AppDataError::TooLarge)?;
    value
        .get(prefix..prefix.checked_add(len).ok_or(AppDataError::TooLarge)?)
        .ok_or(AppDataError::Malformed)
}

fn varint_value(value: &[u8]) -> Result<u64, AppDataError> {
    consume_varint(value).map(|(value, _)| value)
}

fn consume_varint(data: &[u8]) -> Result<(u64, usize), AppDataError> {
    let mut value = 0u64;
    for (index, byte) in data.iter().copied().take(10).enumerate() {
        if index == 9 && byte > 1 {
            return Err(AppDataError::Malformed);
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
    }
    Err(AppDataError::Malformed)
}

fn append_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn append_varint_field(out: &mut Vec<u8>, field: u64, value: u64) {
    append_varint(out, field << 3);
    append_varint(out, value);
}

fn append_bytes_field(out: &mut Vec<u8>, field: u64, value: &[u8]) {
    append_varint(out, (field << 3) | 2);
    append_varint(out, value.len() as u64);
    out.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reaction_matches_captured_wire_and_round_trips() {
        let encoded = encode_reaction(1, "👍").unwrap();
        assert_eq!(
            encoded,
            [
                0x0a, 0x0a, 0x0a, 0x08, 0x08, 0x01, 0x12, 0x04, 0xf0, 0x9f, 0x91, 0x8d
            ]
        );
        assert_eq!(
            decode_reactions(&encoded).unwrap(),
            [CallReaction {
                transaction_id: 1,
                emoji: "👍".to_string(),
            }]
        );
    }

    #[test]
    fn decoder_skips_unknown_fields_and_rejects_invalid_inputs() {
        let mut encoded = encode_reaction(7, "").unwrap();
        encoded.extend_from_slice(&[0x28, 0x01]);
        assert_eq!(decode_reactions(&encoded).unwrap()[0].transaction_id, 7);
        assert_eq!(
            decode_reactions(&[0x0a, 0x02, 0x0a]).unwrap_err(),
            AppDataError::Malformed
        );
        assert_eq!(
            encode_reaction(0, "x").unwrap_err(),
            AppDataError::MissingTransaction
        );

        assert_eq!(
            encode_reaction(1, &"x".repeat(MAX_REACTION_BYTES + 1)).unwrap_err(),
            AppDataError::TooLarge
        );
        assert_eq!(
            decode_reactions(&vec![0; MAX_APP_DATA_BYTES + 1]).unwrap_err(),
            AppDataError::TooLarge
        );

        let wrap_reaction = |emoji: &[u8]| {
            let mut reaction = Vec::new();
            append_varint_field(&mut reaction, 1, 1);
            append_bytes_field(&mut reaction, 2, emoji);
            let mut message = Vec::new();
            append_bytes_field(&mut message, 1, &reaction);
            let mut payload = Vec::new();
            append_bytes_field(&mut payload, 1, &message);
            payload
        };
        assert_eq!(
            decode_reactions(&wrap_reaction(&vec![b'x'; MAX_REACTION_BYTES + 1])).unwrap_err(),
            AppDataError::TooLarge
        );
        assert_eq!(
            decode_reactions(&wrap_reaction(&[0xff])).unwrap_err(),
            AppDataError::InvalidUtf8
        );

        let mut oversized_length = vec![0x0a];
        append_varint(&mut oversized_length, u64::MAX);
        assert_eq!(
            decode_reactions(&oversized_length).unwrap_err(),
            AppDataError::TooLarge
        );
        assert_eq!(
            decode_reactions(&[0x80; 10]).unwrap_err(),
            AppDataError::Malformed
        );
    }
}
