use super::{DecodeError, decode_varint, encode_varint};
use alloc::vec::Vec;

pub const MAX_FIELD_NUMBER: u32 = (1 << 29) - 1;
pub const MAX_GROUP_DEPTH: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldNumber(u32);

impl FieldNumber {
    pub const fn new(value: u32) -> Result<Self, DecodeError> {
        if value == 0 || value > MAX_FIELD_NUMBER {
            Err(DecodeError::InvalidTag)
        } else {
            Ok(Self(value))
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Group(&'a [u8]),
    Fixed32(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    pub number: FieldNumber,
    pub value: Value<'a>,
    pub raw: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_message_bytes: usize,
    pub max_fields: usize,
    pub max_group_depth: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_message_bytes: 16 * 1024 * 1024,
            max_fields: 1_000_000,
            max_group_depth: MAX_GROUP_DEPTH,
        }
    }
}

/// Reads one complete message. An error terminates iteration.
/// Length-delimited values remain opaque until their schema is known.
pub struct Reader<'a> {
    remaining: &'a [u8],
    fields_left: usize,
    group_depth: u8,
}

impl<'a> Reader<'a> {
    pub fn new(input: &'a [u8], limits: Limits) -> Result<Self, DecodeError> {
        if input.len() > limits.max_message_bytes || limits.max_group_depth > MAX_GROUP_DEPTH {
            return Err(DecodeError::LimitExceeded);
        }
        Ok(Self {
            remaining: input,
            fields_left: limits.max_fields,
            group_depth: limits.max_group_depth,
        })
    }

    pub fn remaining(&self) -> &'a [u8] {
        self.remaining
    }

    pub(crate) fn fields_left(&self) -> usize {
        self.fields_left
    }

    /// Shares a field budget across schema-directed nested readers.
    pub fn next_bounded(&mut self, budget: &mut usize) -> Option<Result<Field<'a>, DecodeError>> {
        self.fields_left = self.fields_left.min(*budget);
        let before = self.fields_left;
        let result = self.next();
        *budget -= before - self.fields_left;
        result
    }
    pub(crate) fn next_value(&mut self) -> Option<Result<(FieldNumber, Value<'a>), DecodeError>> {
        if self.remaining.is_empty() {
            return None;
        }
        let result = read_field(&mut self.remaining, &mut self.fields_left, self.group_depth);
        if result.is_err() {
            self.remaining = &[];
        }
        Some(result)
    }
    pub(crate) fn next_indexed_value(
        &mut self,
        cutoff: u32,
    ) -> Option<Result<(FieldNumber, Value<'a>), DecodeError>> {
        let result = self.read_indexed_value::<false, false>(cutoff, &mut 0);
        if result.is_err() {
            self.remaining = &[];
        }
        result.transpose()
    }
    #[inline]
    pub(crate) fn count_ascii_prefix(
        &mut self,
        expected_tag: u8,
        cutoff: u32,
        values_left: usize,
    ) -> (usize, usize) {
        let mut remaining = self.remaining;
        let mut known = 0;
        let mut unknown = 0;
        while known + unknown < self.fields_left {
            let [tag, length, rest @ ..] = remaining else {
                break;
            };
            if *tag == expected_tag && *length <= 16 && known < values_left {
                let Some((bytes, tail)) = rest.split_at_checked(usize::from(*length)) else {
                    break;
                };
                if !bytes.is_ascii() {
                    break;
                }
                remaining = tail;
                known += 1;
            } else if let Some(width) = short_unknown_delimited_len(remaining, cutoff)
                && let Some(tail) = remaining.get(width..)
            {
                remaining = tail;
                unknown += 1;
            } else {
                break;
            }
        }
        self.remaining = remaining;
        self.fields_left -= known + unknown;
        (known, unknown)
    }
    pub(crate) fn next_counted_value(
        &mut self,
        cutoff: u32,
        omitted: &mut usize,
    ) -> Option<Result<(FieldNumber, Value<'a>), DecodeError>> {
        let result = self.read_indexed_value::<true, true>(cutoff, omitted);
        if result.is_err() {
            self.remaining = &[];
        }
        result.transpose()
    }
    pub(crate) fn next_batched_indexed_value(
        &mut self,
        cutoff: u32,
    ) -> Option<Result<(FieldNumber, Value<'a>), DecodeError>> {
        let result = self.read_indexed_value::<false, true>(cutoff, &mut 0);
        if result.is_err() {
            self.remaining = &[];
        }
        result.transpose()
    }
    fn read_indexed_value<const COUNT_OMITTED: bool, const BATCH_VARINTS: bool>(
        &mut self,
        cutoff: u32,
        omitted: &mut usize,
    ) -> Result<Option<(FieldNumber, Value<'a>)>, DecodeError> {
        while !self.remaining.is_empty() {
            if (COUNT_OMITTED || !BATCH_VARINTS)
                && let Some(len) = short_unknown_delimited_len(self.remaining, cutoff)
            {
                self.fields_left = self
                    .fields_left
                    .checked_sub(1)
                    .ok_or(DecodeError::LimitExceeded)?;
                self.remaining = self.remaining.get(len..).ok_or(DecodeError::Truncated)?;
                if COUNT_OMITTED {
                    *omitted += 1;
                }
                continue;
            }
            let (number, wire) = tag(&mut self.remaining)?;
            if number.get() > cutoff && wire == 2 {
                self.fields_left = self
                    .fields_left
                    .checked_sub(1)
                    .ok_or(DecodeError::LimitExceeded)?;
                read_delimited(&mut self.remaining)?;
                if COUNT_OMITTED {
                    *omitted += 1;
                }
                continue;
            }
            let value = read_value(
                &mut self.remaining,
                number,
                wire,
                &mut self.fields_left,
                self.group_depth,
            )?;
            if number.get() <= cutoff {
                return Ok(Some((number, value)));
            }
            if COUNT_OMITTED {
                *omitted += 1;
            }
            if BATCH_VARINTS {
                let tag = number.get() << 3;
                if wire == 0 && (1 << 14..1 << 21).contains(&tag) {
                    let prefix = (tag & 127)
                        | 128
                        | (((tag >> 7) & 127) << 8)
                        | (128 << 8)
                        | ((tag >> 14) << 16);
                    let mut rest = self.remaining;
                    let mut count = 0;
                    let word_prefix =
                        u128::from(prefix) * 0x0000_0001_0000_0001_0000_0001_0000_0001;
                    while let Some((entries, suffix)) = rest.split_first_chunk::<16>() {
                        if u128::from_le_bytes(*entries) & 0x80ff_ffff_80ff_ffff_80ff_ffff_80ff_ffff
                            != word_prefix
                        {
                            break;
                        }
                        rest = suffix;
                        count += 4;
                    }
                    while let Some((entry, suffix)) = rest.split_first_chunk::<4>() {
                        if u32::from_le_bytes(*entry) & 0x80ff_ffff != prefix {
                            if entry[0] & 0x87 == 0x80 {
                                let (suffix, mixed) = skip_mixed_unknown_varints(rest, cutoff);
                                rest = suffix;
                                count += mixed;
                            }
                            break;
                        }
                        rest = suffix;
                        count += 1;
                    }
                    self.fields_left = self
                        .fields_left
                        .checked_sub(count)
                        .ok_or(DecodeError::LimitExceeded)?;
                    self.remaining = rest;
                    if COUNT_OMITTED {
                        *omitted += count;
                    }
                }
            }
        }
        Ok(None)
    }
    pub(crate) fn next_preflighted_index_offset(
        &mut self,
        cutoff: u32,
        expected_tag: u8,
        base: usize,
    ) -> Result<Option<u16>, DecodeError> {
        while !self.remaining.is_empty() {
            if let [tag, length, rest @ ..] = self.remaining
                && *tag == expected_tag
                && *length < 128
            {
                let tail = rest
                    .get(usize::from(*length)..)
                    .ok_or(DecodeError::Truncated)?;
                self.remaining = tail;
                return Ok(Some((core::ptr::from_ref(length).addr() - base) as u16));
            }
            if let Some(len) = short_unknown_delimited_len(self.remaining, cutoff) {
                self.remaining = self.remaining.get(len..).ok_or(DecodeError::Truncated)?;
                continue;
            }
            let (number, wire) = tag(&mut self.remaining)?;
            if wire == 2 {
                let offset = (self.remaining.as_ptr().addr() - base) as u16;
                read_delimited(&mut self.remaining)?;
                if number.get() <= cutoff {
                    return Ok(Some(offset));
                }
            } else {
                read_value(
                    &mut self.remaining,
                    number,
                    wire,
                    &mut self.fields_left,
                    self.group_depth,
                )?;
            }
        }
        Ok(None)
    }
    #[inline(never)]
    pub(crate) fn next_history_value(
        &mut self,
    ) -> Result<Option<(FieldNumber, Value<'a>)>, DecodeError> {
        if self.remaining.is_empty() {
            return Ok(None);
        }
        read_field(&mut self.remaining, &mut self.fields_left, self.group_depth).map(Some)
    }
}

impl<'a> Iterator for Reader<'a> {
    type Item = Result<Field<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let start = self.remaining;
        let result = read_field(&mut self.remaining, &mut self.fields_left, self.group_depth);
        Some(match result {
            Ok((number, value)) => Ok(Field {
                number,
                value,
                raw: &start[..start.len() - self.remaining.len()],
            }),
            Err(error) => {
                self.remaining = &[];
                Err(error)
            }
        })
    }
}

impl core::iter::FusedIterator for Reader<'_> {}

fn take<'a>(input: &mut &'a [u8], len: usize) -> Result<&'a [u8], DecodeError> {
    let (head, tail) = input.split_at_checked(len).ok_or(DecodeError::Truncated)?;
    *input = tail;
    Ok(head)
}

fn varint(input: &mut &[u8]) -> Result<u64, DecodeError> {
    let (value, count) = decode_varint(input)?;
    *input = &input[count..];
    Ok(value)
}

fn tag(input: &mut &[u8]) -> Result<(FieldNumber, u8), DecodeError> {
    let value = u32::try_from(varint(input)?).map_err(|_| DecodeError::InvalidTag)?;
    let number = FieldNumber::new(value >> 3)?;
    let wire = (value & 7) as u8;
    if wire > 5 {
        return Err(DecodeError::InvalidWireType);
    }
    Ok((number, wire))
}

fn read_field<'a>(
    input: &mut &'a [u8],
    fields_left: &mut usize,
    depth: u8,
) -> Result<(FieldNumber, Value<'a>), DecodeError> {
    let (number, wire) = tag(input)?;
    let value = read_value(input, number, wire, fields_left, depth)?;
    Ok((number, value))
}

#[inline(never)]
#[cold]
fn skip_mixed_unknown_varints(mut rest: &[u8], cutoff: u32) -> (&[u8], usize) {
    let mut count = 0;
    // The three tag groups increase in significance in a little-endian word.
    let fence = (cutoff & 0xf) << 3 | (cutoff & 0x7f0) << 4 | (cutoff & 0x3f800) << 5;
    if rest.len() >= 16 {
        let pair_fence = u64::from(fence + 1) * 0x0000_0001_0000_0001;
        let guards = 0x8000_0000_8000_0000;
        let expected = 0x0000_8080_0000_8080;
        while let Some((entries, suffix)) = rest.split_first_chunk::<16>() {
            let (left, right) = entries.split_at(8);
            let left = u64::from_le_bytes(*left.first_chunk::<8>().expect("first pair"));
            let right = u64::from_le_bytes(*right.first_chunk::<8>().expect("second pair"));
            let shape_error = ((left ^ expected) | (right ^ expected)) & 0x8080_8087_8080_8087;
            let left_tags = (left & 0x007f_7f78_007f_7f78) | guards;
            let right_tags = (right & 0x007f_7f78_007f_7f78) | guards;
            let valid_tags = (left_tags - pair_fence) & (right_tags - pair_fence);
            if shape_error | (!valid_tags & guards) != 0 {
                break;
            }
            rest = suffix;
            count += 4;
        }
    }
    if rest.len() >= 8 {
        let pair_fence = u64::from(fence + 1) * 0x0000_0001_0000_0001;
        let lane_high_bits = 0x8000_0000_8000_0000;
        while let Some((entries, suffix)) = rest.split_first_chunk::<8>() {
            let pair = u64::from_le_bytes(*entries);
            if pair & 0x8080_8087_8080_8087 != 0x0000_8080_0000_8080 {
                break;
            }
            // Each lane has a guard bit, so subtraction cannot borrow across lanes.
            let tags = (pair & 0x007f_7f78_007f_7f78) | lane_high_bits;
            if (tags - pair_fence) & lane_high_bits != lane_high_bits {
                break;
            }
            rest = suffix;
            count += 2;
        }
    }
    while let Some((entry, suffix)) = rest.split_first_chunk::<4>() {
        let word = u32::from_le_bytes(*entry);
        if word & 0x8080_8087 != 0x0000_8080 || word & 0x007f_7f78 <= fence {
            break;
        }
        rest = suffix;
        count += 1;
    }
    (rest, count)
}

#[inline]
pub(crate) fn short_unknown_delimited_len(input: &[u8], cutoff: u32) -> Option<usize> {
    let &[first, second, third, length] = input.first_chunk::<4>()?;
    if first & 0x87 != 0x82 || second < 128 || third >= 128 || length >= 128 {
        return None;
    }
    let number =
        u32::from((first & 0x78) >> 3) | (u32::from(second & 127) << 4) | (u32::from(third) << 11);
    (number > cutoff).then_some(4 + usize::from(length))
}

pub(crate) fn next_pair_bytes<'a, const COUNT_OMITTED: bool>(
    remaining: &mut &'a [u8],
    cutoff: u32,
    omitted: &mut usize,
    short_known_only: bool,
) -> Option<(FieldNumber, &'a [u8])> {
    let fence = (cutoff & 0xf) << 3 | (cutoff & 0x7f0) << 4 | (cutoff & 0x3f800) << 5;
    loop {
        if let [tag, length, rest @ ..] = *remaining
            && tag & 0x87 == 2
            && *length < 128
            && let Ok(number) = FieldNumber::new(u32::from(tag >> 3))
            && number.get() <= cutoff
        {
            let (bytes, tail) = rest.split_at_checked(usize::from(*length))?;
            *remaining = tail;
            return Some((number, bytes));
        }
        if cutoff < 1 << 18
            && let Some(word) = remaining.first_chunk::<4>()
        {
            let word = u32::from_le_bytes(*word);
            if word & 0x8080_8087 == 0x0000_8080 && word & 0x007f_7f78 > fence {
                *remaining = &remaining[4..];
                if COUNT_OMITTED {
                    *omitted += 1;
                }
                continue;
            }
        }
        if let Some(width) = short_unknown_delimited_len(remaining, cutoff) {
            *remaining = remaining.get(width..)?;
            if COUNT_OMITTED {
                *omitted += 1;
            }
            continue;
        }
        let (number, wire) = tag(remaining).ok()?;
        if number.get() <= cutoff {
            if short_known_only || wire != 2 {
                return None;
            }
            return Some((number, read_delimited(remaining).ok()?));
        }
        if COUNT_OMITTED {
            return None;
        }
        let mut fields_left = usize::MAX;
        read_value(remaining, number, wire, &mut fields_left, MAX_GROUP_DEPTH).ok()?;
    }
}

fn read_delimited<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], DecodeError> {
    let (&first, rest) = input.split_first().ok_or(DecodeError::Truncated)?;
    let length = if first < 128 {
        *input = rest;
        usize::from(first)
    } else {
        usize::try_from(varint(input)?).map_err(|_| DecodeError::LimitExceeded)?
    };
    take(input, length)
}

fn read_value<'a>(
    input: &mut &'a [u8],
    number: FieldNumber,
    wire: u8,
    fields_left: &mut usize,
    depth: u8,
) -> Result<Value<'a>, DecodeError> {
    *fields_left = fields_left
        .checked_sub(1)
        .ok_or(DecodeError::LimitExceeded)?;
    Ok(match wire {
        0 => Value::Varint(varint(input)?),
        1 => {
            let mut bytes = [0; 8];
            bytes.copy_from_slice(take(input, 8)?);
            Value::Fixed64(u64::from_le_bytes(bytes))
        }
        2 => Value::Bytes(read_delimited(input)?),
        3 => {
            let next_depth = depth.checked_sub(1).ok_or(DecodeError::LimitExceeded)?;
            let body = *input;
            loop {
                let before_tag = *input;
                let (nested_number, nested_wire) = tag(input)?;
                if nested_wire == 4 {
                    if nested_number != number {
                        return Err(DecodeError::MismatchedEndGroup);
                    }
                    break Value::Group(&body[..body.len() - before_tag.len()]);
                }
                read_value(input, nested_number, nested_wire, fields_left, next_depth)?;
            }
        }
        4 => return Err(DecodeError::UnexpectedEndGroup),
        5 => {
            let mut bytes = [0; 4];
            bytes.copy_from_slice(take(input, 4)?);
            Value::Fixed32(u32::from_le_bytes(bytes))
        }
        _ => return Err(DecodeError::InvalidWireType),
    })
}

/// Encodes scalar fields into a reusable caller-owned buffer.
pub struct Writer<'a> {
    output: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub fn new(output: &'a mut Vec<u8>) -> Self {
        Self { output }
    }

    fn tag(&mut self, number: FieldNumber, wire: u8) {
        encode_varint(
            (u64::from(number.get()) << 3) | u64::from(wire),
            self.output,
        );
    }

    pub fn varint(&mut self, number: FieldNumber, value: u64) {
        self.tag(number, 0);
        encode_varint(value, self.output);
    }

    pub fn fixed64(&mut self, number: FieldNumber, value: u64) {
        self.tag(number, 1);
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub fn bytes(&mut self, number: FieldNumber, value: &[u8]) {
        self.tag(number, 2);
        encode_varint(value.len() as u64, self.output);
        self.output.extend_from_slice(value);
    }

    pub fn fixed32(&mut self, number: FieldNumber, value: u32) {
        self.tag(number, 5);
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub fn raw_field(&mut self, field: Field<'_>) {
        self.output.extend_from_slice(field.raw);
    }
}

pub const fn encode_zigzag32(value: i32) -> u32 {
    ((value as u32) << 1) ^ ((value >> 31) as u32)
}
pub const fn decode_zigzag32(value: u32) -> i32 {
    ((value >> 1) as i32) ^ -((value & 1) as i32)
}
pub const fn encode_zigzag64(value: i64) -> u64 {
    ((value as u64) << 1) ^ ((value >> 63) as u64)
}
pub const fn decode_zigzag64(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}
