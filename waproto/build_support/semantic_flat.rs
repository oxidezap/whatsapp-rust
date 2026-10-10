//! Frozen prefix for the bounded Header projection. Its wire conversion moves
//! payloads toward the front of the same allocation; no nested encoder runs.
use crate::semantic::{ValueRef, Visitor};
use buffa::{DecodeContext, DecodeError, alloc::vec::Vec, encoding::varint_len};

pub const METADATA: usize = 40;
const ABSENT: u32 = u32::MAX;

pub struct Prefix {
    bytes: Vec<u8>,
}

fn bytes(current: &dyn Visitor, tag: u32) -> Result<Option<&[u8]>, DecodeError> {
    match current.field(tag, 0) {
        None => Ok(None),
        Some(ValueRef::Bytes(bytes)) => Ok(Some(bytes)),
        _ => Err(DecodeError::InvalidWireType(2)),
    }
}
fn child(current: &dyn Visitor, tag: u32) -> Result<Option<&dyn Visitor>, DecodeError> {
    match current.field(tag, 0) {
        None => Ok(None),
        Some(ValueRef::Child(child)) if child.supported() => Ok(Some(child)),
        _ => Err(DecodeError::InvalidWireType(2)),
    }
}
fn get_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}
fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

impl Prefix {
    // The caller supplies its prepaid allocation balance, not the caller's
    // decode budget a second time. Admission precedes this allocation.
    // Existing fields are bounded by the descriptor closure, so snapshotting
    // them does not spend the recursion allowance of an incoming merge.
    pub fn freeze(current: &dyn Visitor, ctx: DecodeContext<'_>) -> Result<Self, DecodeError> {
        if !current.supported() {
            return Err(DecodeError::InvalidWireType(3));
        }
        let mut flags = 0u8;
        let mut leaves = [None; 3];
        if let Some(image) = child(current, 4)? {
            flags = 1;
            leaves[0] = bytes(image, 3)?;
            leaves[1] = bytes(image, 16)?;
            if let Some(context) = child(image, 17)? {
                flags |= 4;
                if let Some(quoted) = child(context, 3)? {
                    flags |= 8;
                    leaves[2] = bytes(quoted, 1)?;
                }
            }
        } else if let Some(jpeg) = bytes(current, 6)? {
            flags = 2;
            leaves[1] = Some(jpeg);
        }
        let len = leaves
            .iter()
            .flatten()
            .try_fold(METADATA, |len, bytes| len.checked_add(bytes.len()))
            .ok_or(DecodeError::MessageTooLarge)?;
        let _ = u32::try_from(len).map_err(|_| DecodeError::MessageTooLarge)?;
        ctx.register_element_memory(len)?;
        let mut blob = Vec::with_capacity(len);
        blob.resize(METADATA, 0);
        blob[0] = flags;
        for (index, leaf) in leaves.into_iter().enumerate() {
            let offset = 4 + 12 * index;
            if let Some(leaf) = leaf {
                put_u32(&mut blob, offset, 0);
                let start = u32::try_from(blob.len()).map_err(|_| DecodeError::MessageTooLarge)?;
                put_u32(&mut blob, offset + 4, start);
                put_u32(
                    &mut blob,
                    offset + 8,
                    u32::try_from(leaf.len()).map_err(|_| DecodeError::MessageTooLarge)?,
                );
                blob.extend_from_slice(leaf);
            } else {
                put_u32(&mut blob, offset, ABSENT);
            }
        }
        Ok(Self { bytes: blob })
    }
    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
    pub fn allocation_address(&self) -> usize {
        self.bytes.as_ptr() as usize
    }
    pub fn into_wire(mut self) -> Result<Vec<u8>, DecodeError> {
        let mut ranges = [None; 3];
        for (index, range) in ranges.iter_mut().enumerate() {
            let offset = 4 + 12 * index;
            if get_u32(&self.bytes, offset) != ABSENT {
                *range = Some((
                    get_u32(&self.bytes, offset + 4) as usize,
                    get_u32(&self.bytes, offset + 8) as usize,
                ));
            }
        }
        let flags = self.bytes[0];
        let field_len = |tag: u32, range: Option<(usize, usize)>| {
            range.map_or(0, |(_, len)| frame_len(tag, len))
        };
        let conversation = field_len(1, ranges[2]);
        let quote = if flags & 8 != 0 {
            frame_len(3, conversation)
        } else {
            0
        };
        let context = if flags & 4 != 0 {
            frame_len(17, quote)
        } else {
            0
        };
        let image = field_len(3, ranges[0]) + field_len(16, ranges[1]) + context;
        let mut plan = Plan::default();
        match flags & 3 {
            0 => {}
            1 => {
                plan.header(4, image)?;
                plan.leaf(3, ranges[0])?;
                plan.leaf(16, ranges[1])?;
                if flags & 4 != 0 {
                    plan.header(17, quote)?;
                    if flags & 8 != 0 {
                        plan.header(3, conversation)?;
                        plan.leaf(1, ranges[2])?;
                    }
                }
            }
            2 => plan.leaf(6, ranges[1])?,
            _ => return Err(DecodeError::InvalidWireType(3)),
        }
        let mut output = 0usize;
        for part in &plan.parts[..plan.count] {
            match *part {
                Part::Header(start, len) => {
                    let end = output
                        .checked_add(len)
                        .ok_or(DecodeError::MessageTooLarge)?;
                    if end > self.bytes.len() {
                        return Err(DecodeError::UnexpectedEof);
                    }
                    self.bytes[output..end].copy_from_slice(&plan.headers[start..start + len]);
                    output = end;
                }
                Part::Leaf(start, len) => {
                    let end = start.checked_add(len).ok_or(DecodeError::MessageTooLarge)?;
                    // All headers together fit in the metadata prefix, so a
                    // move cannot overwrite a later payload's source bytes.
                    if output > start || end > self.bytes.len() {
                        return Err(DecodeError::UnexpectedEof);
                    }
                    self.bytes.copy_within(start..end, output);
                    output = output
                        .checked_add(len)
                        .ok_or(DecodeError::MessageTooLarge)?;
                }
            }
        }
        self.bytes.truncate(output);
        Ok(self.bytes)
    }
}
fn frame_len(tag: u32, len: usize) -> usize {
    varint_len((u64::from(tag) << 3) | 2) + varint_len(len as u64) + len
}
#[derive(Clone, Copy)]
enum Part {
    Header(usize, usize),
    Leaf(usize, usize),
}
struct Plan {
    headers: [u8; METADATA],
    header_len: usize,
    parts: [Part; 10],
    count: usize,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            headers: [0; METADATA],
            header_len: 0,
            parts: [Part::Header(0, 0); 10],
            count: 0,
        }
    }
}
impl Plan {
    fn append(&mut self, part: Part) -> Result<(), DecodeError> {
        let Some(slot) = self.parts.get_mut(self.count) else {
            return Err(DecodeError::MessageTooLarge);
        };
        *slot = part;
        self.count += 1;
        Ok(())
    }
    fn varint(&mut self, mut value: u64) -> Result<(), DecodeError> {
        loop {
            let Some(slot) = self.headers.get_mut(self.header_len) else {
                return Err(DecodeError::MessageTooLarge);
            };
            *slot = (value as u8 & 0x7f) | if value > 0x7f { 0x80 } else { 0 };
            self.header_len += 1;
            value >>= 7;
            if value == 0 {
                return Ok(());
            }
        }
    }
    fn header(&mut self, tag: u32, len: usize) -> Result<(), DecodeError> {
        let start = self.header_len;
        self.varint((u64::from(tag) << 3) | 2)?;
        self.varint(len as u64)?;
        self.append(Part::Header(start, self.header_len - start))
    }
    fn leaf(&mut self, tag: u32, range: Option<(usize, usize)>) -> Result<(), DecodeError> {
        if let Some((start, len)) = range {
            self.header(tag, len)?;
            self.append(Part::Leaf(start, len))?;
        }
        Ok(())
    }
}
