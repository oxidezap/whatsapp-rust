//! Fixture-only semantic projection. Decode stores values and arena offsets;
//! canonical encoding is an explicit later operation, never a decode baseline.
use buffa::alloc::vec::Vec;
use buffa::{
    DecodeContext, DecodeError,
    bytes::Buf,
    encoding::{Tag, WireType},
};
use core::ops::Range;

#[derive(Clone, Copy)]
pub enum Kind {
    Bytes,
    Uint32,
    Message(u16),
}
#[derive(Clone, Copy)]
pub struct Field {
    pub number: u32,
    pub kind: Kind,
    pub repeated: bool,
    pub oneof: u8,
}
pub type Schema = &'static [&'static [Field]];
pub enum ValueRef<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Child(&'a dyn Visitor),
    Fixed32(u32),
    Fixed64(u64),
    Unsupported,
}
pub trait Visitor {
    fn supported(&self) -> bool {
        true
    }
    fn field(&self, number: u32, index: usize) -> Option<ValueRef<'_>>;
    fn unknown(&self, _index: usize) -> Option<(u32, ValueRef<'_>)> {
        None
    }
}
#[derive(Clone, Debug, PartialEq)]
enum Value {
    Varint(u64),
    Bytes(Range<usize>),
    Child(usize),
    Fixed32(u32),
    Fixed64(u64),
}
#[derive(Clone, Debug, PartialEq)]
struct Entry {
    number: u32,
    value: Value,
}
#[derive(Clone, Debug, Default)]
struct Node {
    entries: Vec<Entry>,
}
#[derive(Clone)]
pub struct Arena {
    schema: Schema,
    bytes: Vec<u8>,
    nodes: Vec<Node>,
    #[cfg(test)]
    encode_calls: std::cell::Cell<usize>,
}
impl Arena {
    pub fn new(schema: Schema) -> Self {
        Self {
            schema,
            bytes: Vec::new(),
            nodes: Vec::new(),
            #[cfg(test)]
            encode_calls: std::cell::Cell::new(0),
        }
    }
    // Stage independently: a rejected completed occurrence cannot bless the
    // typed receiver's legitimate partial mutation after its decoder fails.
    pub fn accept(&mut self, record: &[u8], ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        self.stage(record, ctx)?.commit();
        Ok(())
    }
    pub fn stage<'a>(
        &'a mut self,
        record: &[u8],
        ctx: DecodeContext<'_>,
    ) -> Result<Staged<'a>, DecodeError> {
        let checkpoint = (self.bytes.len(), self.nodes.len());
        ctx.register_element_memory(record.len())?;
        self.bytes.extend_from_slice(record);
        let parsed = (|| {
            if self.nodes.is_empty() {
                ctx.register_element_memory(size_of::<Node>())?;
                self.nodes.push(Node::default());
            }
            self.parse(0, record, checkpoint.0, ctx)
        })();
        match parsed {
            Ok(staged) => Ok(Staged {
                arena: self,
                checkpoint,
                staged: Some(staged),
            }),
            Err(error) => {
                self.bytes.truncate(checkpoint.0);
                self.nodes.truncate(checkpoint.1);
                Err(error)
            }
        }
    }
    pub fn is_empty(&self) -> bool {
        self.nodes
            .first()
            .is_none_or(|node| node.entries.is_empty())
    }
    pub fn record(&self, range: &Range<usize>) -> &[u8] {
        &self.bytes[range.clone()]
    }
    pub fn clone_with_context(&self, ctx: DecodeContext<'_>) -> Result<Self, DecodeError> {
        let entries = self
            .nodes
            .iter()
            .map(|node| node.entries.len())
            .fold(0usize, usize::saturating_add);
        let charge = self
            .bytes
            .len()
            .saturating_add(self.nodes.len().saturating_mul(size_of::<Node>()))
            .saturating_add(entries.saturating_mul(size_of::<Entry>()));
        ctx.register_element_memory(charge)?;
        Ok(self.clone())
    }
    fn parse(
        &mut self,
        schema: usize,
        mut input: &[u8],
        offset: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<usize, DecodeError> {
        let input_len = input.len();
        ctx.register_element_memory(size_of::<Node>())?;
        let node = self.nodes.len();
        self.nodes.push(Node::default());
        while input.has_remaining() {
            let tag = Tag::decode(&mut input)?;
            let field = self.schema[schema]
                .iter()
                .find(|f| f.number == tag.field_number())
                .copied();
            if let Some(Field {
                kind: Kind::Uint32,
                repeated: true,
                ..
            }) = field
                && tag.wire_type() == WireType::LengthDelimited
            {
                let mut packed = buffa::types::borrow_bytes(&mut input)?;
                while !packed.is_empty() {
                    let value = buffa::encoding::decode_varint(&mut packed)? as u32;
                    self.insert(
                        node,
                        schema,
                        Entry {
                            number: tag.field_number(),
                            value: Value::Varint(u64::from(value)),
                        },
                        ctx,
                    )?;
                }
                continue;
            }
            if let Some(field) = field {
                let expected = match field.kind {
                    Kind::Uint32 => WireType::Varint,
                    _ => WireType::LengthDelimited,
                };
                if tag.wire_type() != expected {
                    return Err(DecodeError::WireTypeMismatch {
                        field_number: tag.field_number(),
                        expected: expected as u8,
                        actual: tag.wire_type() as u8,
                    });
                }
            }
            let value = match (field.map(|f| f.kind), tag.wire_type()) {
                (Some(Kind::Message(child)), WireType::LengthDelimited) => {
                    let bytes = buffa::types::borrow_bytes(&mut input)?;
                    let start = offset + input_len - input.len() - bytes.len();
                    Value::Child(self.parse(usize::from(child), bytes, start, ctx.descend()?)?)
                }
                (Some(Kind::Uint32), WireType::Varint) => {
                    Value::Varint(u64::from(buffa::encoding::decode_varint(&mut input)? as u32))
                }
                (_, WireType::Varint) => Value::Varint(buffa::encoding::decode_varint(&mut input)?),
                (_, WireType::LengthDelimited) => {
                    let bytes = buffa::types::borrow_bytes(&mut input)?;
                    let start = offset + input_len - input.len() - bytes.len();
                    Value::Bytes(start..start + bytes.len())
                }
                (_, WireType::StartGroup) => return Err(DecodeError::InvalidWireType(3)),
                (_, WireType::Fixed32) if input.remaining() >= 4 => {
                    Value::Fixed32(input.get_u32_le())
                }
                (_, WireType::Fixed64) if input.remaining() >= 8 => {
                    Value::Fixed64(input.get_u64_le())
                }
                _ => return Err(DecodeError::UnexpectedEof),
            };
            self.insert(
                node,
                schema,
                Entry {
                    number: tag.field_number(),
                    value,
                },
                ctx,
            )?;
        }
        Ok(node)
    }
    fn insert(
        &mut self,
        node: usize,
        schema: usize,
        entry: Entry,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        ctx.register_element_memory(size_of::<Entry>())?;
        if self.schema[schema].iter().any(|f| f.number == entry.number) {
            self.merge_entry(node, entry, schema);
        } else {
            self.push_entry(node, entry);
        }
        Ok(())
    }
    fn push_entry(&mut self, node: usize, entry: Entry) {
        let entries = &mut self.nodes[node].entries;
        if entries.capacity() == 0 {
            entries.reserve_exact(1);
        }
        entries.push(entry);
    }
    fn merge(&mut self, target: usize, staged: usize, schema: usize) {
        let incoming = std::mem::take(&mut self.nodes[staged]);
        if self.nodes[target].entries.is_empty() {
            self.nodes[target] = incoming;
            return;
        }
        for entry in incoming.entries {
            if self.schema[schema]
                .iter()
                .any(|field| field.number == entry.number)
            {
                self.merge_entry(target, entry, schema);
            } else {
                self.push_entry(target, entry);
            }
        }
    }
    fn merge_entry(&mut self, target: usize, entry: Entry, schema: usize) {
        let field = self.schema[schema]
            .iter()
            .find(|f| f.number == entry.number)
            .expect("descriptor field");
        if field.oneof != 0 {
            self.nodes[target].entries.retain(|old| {
                old.number == entry.number
                    || !self.schema[schema]
                        .iter()
                        .any(|f| f.number == old.number && f.oneof == field.oneof)
            });
        }
        let existing = self.nodes[target]
            .entries
            .iter()
            .position(|old| old.number == entry.number);
        if !field.repeated
            && let Some(index) = existing
        {
            if let (Kind::Message(child), Value::Child(incoming)) = (field.kind, &entry.value)
                && let Value::Child(previous) = self.nodes[target].entries[index].value
            {
                self.merge(previous, *incoming, usize::from(child));
            } else {
                self.nodes[target].entries[index] = entry;
            }
        } else {
            self.push_entry(target, entry);
        }
    }
    pub fn from_visitor(
        schema: Schema,
        current: &dyn Visitor,
        ctx: DecodeContext<'_>,
    ) -> Result<Self, DecodeError> {
        ctx.register_element_memory(size_of::<Node>())?;
        let mut arena = Self::new(schema);
        arena.nodes.push(Node::default());
        arena.read_visitor(0, 0, current, ctx)?;
        Ok(arena)
    }
    fn read_visitor(
        &mut self,
        node: usize,
        schema: usize,
        current: &dyn Visitor,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if !current.supported() {
            return Err(DecodeError::InvalidWireType(3));
        }
        for field in self.schema[schema] {
            let mut index = 0;
            while let Some(value) = current.field(field.number, index) {
                let value = self.copy_value(field.kind, value, ctx)?;
                self.insert(
                    node,
                    schema,
                    Entry {
                        number: field.number,
                        value,
                    },
                    ctx,
                )?;
                index += 1;
            }
        }
        let mut index = 0;
        while let Some((number, value)) = current.unknown(index) {
            let value = self.copy_value(Kind::Bytes, value, ctx)?;
            ctx.register_element_memory(size_of::<Entry>())?;
            self.push_entry(node, Entry { number, value });
            index += 1;
        }
        Ok(())
    }
    fn copy_value(
        &mut self,
        kind: Kind,
        value: ValueRef<'_>,
        ctx: DecodeContext<'_>,
    ) -> Result<Value, DecodeError> {
        Ok(match value {
            ValueRef::Varint(value) => Value::Varint(value),
            ValueRef::Fixed32(value) => Value::Fixed32(value),
            ValueRef::Fixed64(value) => Value::Fixed64(value),
            ValueRef::Bytes(bytes) => {
                ctx.register_element_memory(bytes.len())?;
                let start = self.bytes.len();
                self.bytes.extend_from_slice(bytes);
                Value::Bytes(start..self.bytes.len())
            }
            ValueRef::Child(current) => {
                let Kind::Message(schema) = kind else {
                    return Err(DecodeError::InvalidWireType(2));
                };
                ctx.register_element_memory(size_of::<Node>())?;
                let node = self.nodes.len();
                self.nodes.push(Node::default());
                self.read_visitor(node, usize::from(schema), current, ctx.descend()?)?;
                Value::Child(node)
            }
            ValueRef::Unsupported => return Err(DecodeError::InvalidWireType(3)),
        })
    }
    pub fn matches(&self, current: &dyn Visitor) -> bool {
        self.matches_node(0, 0, current)
    }
    fn matches_node(&self, node: usize, schema: usize, current: &dyn Visitor) -> bool {
        if !current.supported() {
            return false;
        }
        if self.nodes.get(node).is_none() {
            return self.schema[schema]
                .iter()
                .all(|field| current.field(field.number, 0).is_none())
                && current.unknown(0).is_none();
        }
        for field in self.schema[schema] {
            let mut count = 0;
            for entry in self.nodes[node]
                .entries
                .iter()
                .filter(|entry| entry.number == field.number)
            {
                let Some(value) = current.field(field.number, count) else {
                    return false;
                };
                if !self.matches_value(&entry.value, field.kind, value) {
                    return false;
                }
                count += 1;
            }
            if current.field(field.number, count).is_some() {
                return false;
            }
        }
        let mut unknown_count = 0;
        for entry in self.nodes[node].entries.iter().filter(|entry| {
            !self.schema[schema]
                .iter()
                .any(|field| field.number == entry.number)
        }) {
            let index = unknown_count;
            unknown_count += 1;
            let Some((number, value)) = current.unknown(index) else {
                return false;
            };
            if number != entry.number || !self.matches_value(&entry.value, Kind::Bytes, value) {
                return false;
            }
        }
        current.unknown(unknown_count).is_none()
    }
    fn matches_value(&self, expected: &Value, kind: Kind, current: ValueRef<'_>) -> bool {
        match (expected, current) {
            (Value::Varint(a), ValueRef::Varint(b)) => *a == b,
            (Value::Fixed32(a), ValueRef::Fixed32(b)) => *a == b,
            (Value::Fixed64(a), ValueRef::Fixed64(b)) => *a == b,
            (Value::Bytes(range), ValueRef::Bytes(bytes)) => self.bytes[range.clone()] == *bytes,
            (Value::Child(node), ValueRef::Child(child)) => {
                let Kind::Message(schema) = kind else {
                    return false;
                };
                self.matches_node(*node, usize::from(schema), child)
            }
            _ => false,
        }
    }
    pub fn scalar_edit(&self, current: &dyn Visitor) -> Option<ScalarEdit> {
        let mut edit = None;
        self.find_scalar_edit(0, 0, current, &mut edit)
            .then_some(edit)
            .flatten()
    }
    fn find_scalar_edit(
        &self,
        node: usize,
        schema: usize,
        current: &dyn Visitor,
        edit: &mut Option<ScalarEdit>,
    ) -> bool {
        if !current.supported() {
            return false;
        }
        let Some(values) = self.nodes.get(node) else {
            return false;
        };
        for field in self.schema[schema] {
            let mut count = 0;
            for (index, entry) in values
                .entries
                .iter()
                .enumerate()
                .filter(|(_, entry)| entry.number == field.number)
            {
                let Some(value) = current.field(field.number, count) else {
                    return false;
                };
                count += 1;
                let changed = match (&entry.value, value) {
                    (Value::Varint(a), ValueRef::Varint(b)) => {
                        (*a != b).then_some(Value::Varint(b))
                    }
                    (Value::Fixed32(a), ValueRef::Fixed32(b)) => {
                        (*a != b).then_some(Value::Fixed32(b))
                    }
                    (Value::Fixed64(a), ValueRef::Fixed64(b)) => {
                        (*a != b).then_some(Value::Fixed64(b))
                    }
                    (Value::Child(child), ValueRef::Child(visitor)) => {
                        let Kind::Message(child_schema) = field.kind else {
                            return false;
                        };
                        if !self.find_scalar_edit(*child, usize::from(child_schema), visitor, edit)
                        {
                            return false;
                        }
                        None
                    }
                    (expected, value) => {
                        if !self.matches_value(expected, field.kind, value) {
                            return false;
                        }
                        None
                    }
                };
                if let Some(value) = changed {
                    if edit.is_some() {
                        return false;
                    }
                    *edit = Some(ScalarEdit {
                        node,
                        entry: index,
                        value,
                    });
                }
            }
            if current.field(field.number, count).is_some() {
                return false;
            }
        }
        let mut count = 0;
        for entry in values.entries.iter().filter(|entry| {
            !self.schema[schema]
                .iter()
                .any(|field| field.number == entry.number)
        }) {
            let Some((number, value)) = current.unknown(count) else {
                return false;
            };
            if number != entry.number || !self.matches_value(&entry.value, Kind::Bytes, value) {
                return false;
            }
            count += 1;
        }
        current.unknown(count).is_none()
    }
    // The plan indexes the current arena: apply it before any merge or
    // compaction can move entries. Cloning preserves these indices.
    pub fn apply_scalar(&mut self, edit: ScalarEdit) {
        self.nodes[edit.node].entries[edit.entry].value = edit.value;
    }
    // Call only after dropping raw-record references. Frozen prefixes own their
    // own arena. A bounded stack plan avoids heap work; larger graphs keep the
    // existing representation without changing their semantics.
    pub fn compact_projection(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        let mut ids = [usize::MAX; 16];
        let mut positions = [usize::MAX; 16];
        let mut count = 1;
        ids[0] = 0;
        let mut cursor = 0;
        let mut ranges = [(0usize, 0usize, 0usize); 16];
        let mut range_count = 0;
        while cursor < count {
            for entry in &self.nodes[ids[cursor]].entries {
                match &entry.value {
                    Value::Child(child) if !ids[..count].contains(child) => {
                        if count == ids.len() {
                            return;
                        }
                        ids[count] = *child;
                        count += 1;
                    }
                    Value::Bytes(range)
                        if !ranges[..range_count]
                            .iter()
                            .any(|(start, end, _)| *start == range.start && *end == range.end) =>
                    {
                        if range_count == ranges.len() {
                            return;
                        }
                        ranges[range_count] = (range.start, range.end, 0);
                        range_count += 1;
                    }
                    _ => {}
                }
            }
            cursor += 1;
        }
        ranges[..range_count].sort_unstable();
        if ranges[..range_count]
            .windows(2)
            .any(|pair| pair[0].1 > pair[1].0)
        {
            return;
        }
        let mut write = 0;
        for (start, end, target) in &mut ranges[..range_count] {
            *target = write;
            self.bytes.copy_within(*start..*end, write);
            write += *end - *start;
        }
        self.bytes.truncate(write);
        positions[..count].copy_from_slice(&ids[..count]);
        for index in 0..count {
            let previous = positions[index];
            self.nodes.swap(index, previous);
            for position in &mut positions[index + 1..count] {
                if *position == index {
                    *position = previous;
                }
            }
        }
        self.nodes.truncate(count);
        for node in &mut self.nodes {
            for entry in &mut node.entries {
                match &mut entry.value {
                    Value::Child(child) => {
                        *child = ids[..count]
                            .iter()
                            .position(|id| id == child)
                            .expect("reachable child")
                    }
                    Value::Bytes(range) => {
                        let (start, end, target) = ranges[..range_count]
                            .iter()
                            .find(|(start, end, _)| *start == range.start && *end == range.end)
                            .expect("reachable payload");
                        *range = *target..*target + (*end - *start);
                    }
                    _ => {}
                }
            }
        }
    }
    // Migration is an explicit encoding operation. Reserve a linear upper
    // bound before allocating its output; dead records only make it larger.
    pub fn encoding_bound(&self) -> usize {
        self.nodes.iter().fold(self.bytes.len(), |total, node| {
            total.saturating_add(node.entries.len().saturating_mul(25))
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::with_capacity(self.encoded_len());
        self.encode_into(&mut output);
        output
    }
    // Migration already reserved its entire destination. Writing children
    // directly keeps that reservation sufficient, without temporary buffers.
    pub fn encode_into(&self, output: &mut Vec<u8>) {
        #[cfg(test)]
        self.encode_calls.set(self.encode_calls.get() + 1);
        if !self.nodes.is_empty() {
            self.write(0, 0, output);
        }
    }
    fn encoded_len(&self) -> usize {
        if self.nodes.is_empty() {
            0
        } else {
            self.node_len(0, 0)
        }
    }
    fn node_len(&self, node: usize, schema: usize) -> usize {
        self.nodes[node]
            .entries
            .iter()
            .map(|entry| {
                let tag = buffa::encoding::varint_len(u64::from(entry.number) << 3);
                let value = match &entry.value {
                    Value::Varint(value) => buffa::encoding::varint_len(*value),
                    Value::Bytes(range) => {
                        buffa::encoding::varint_len(range.len() as u64) + range.len()
                    }
                    Value::Child(child) => {
                        let field = self.schema[schema]
                            .iter()
                            .find(|field| field.number == entry.number)
                            .expect("child descriptor");
                        let Kind::Message(child_schema) = field.kind else {
                            unreachable!("child descriptor")
                        };
                        let len = self.node_len(*child, usize::from(child_schema));
                        buffa::encoding::varint_len(len as u64) + len
                    }
                    Value::Fixed32(_) => 4,
                    Value::Fixed64(_) => 8,
                };
                tag + value
            })
            .sum()
    }
    #[cfg(test)]
    pub fn encode_calls(&self) -> usize {
        self.encode_calls.get()
    }
    fn write(&self, node: usize, schema: usize, out: &mut Vec<u8>) {
        for field in self.schema[schema] {
            for entry in &self.nodes[node].entries {
                if entry.number == field.number {
                    self.write_entry(entry, field.kind, out);
                }
            }
        }
        for entry in self.nodes[node].entries.iter().filter(|entry| {
            !self.schema[schema]
                .iter()
                .any(|field| field.number == entry.number)
        }) {
            self.write_entry(entry, Kind::Bytes, out);
        }
    }
    fn write_entry(&self, entry: &Entry, kind: Kind, out: &mut Vec<u8>) {
        let wire = match entry.value {
            Value::Varint(_) => 0,
            Value::Fixed64(_) => 1,
            Value::Bytes(_) | Value::Child(_) => 2,
            Value::Fixed32(_) => 5,
        };
        varint((u64::from(entry.number) << 3) | wire, out);
        match &entry.value {
            Value::Varint(value) => varint(*value, out),
            Value::Bytes(range) => {
                varint(range.len() as u64, out);
                out.extend_from_slice(&self.bytes[range.clone()]);
            }
            Value::Child(child) => {
                let Kind::Message(schema) = kind else {
                    unreachable!("child descriptor")
                };
                let len = self.node_len(*child, usize::from(schema));
                varint(len as u64, out);
                self.write(*child, usize::from(schema), out);
            }
            Value::Fixed32(value) => out.extend(value.to_le_bytes()),
            Value::Fixed64(value) => out.extend(value.to_le_bytes()),
        }
    }
}
fn varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}

// A pending stage holds the arena borrow, so it cannot be committed out of
// order or survive another mutation. Drop discards only staged metadata.
pub struct Staged<'a> {
    arena: &'a mut Arena,
    checkpoint: (usize, usize),
    staged: Option<usize>,
}
impl Staged<'_> {
    pub fn commit(mut self) -> Range<usize> {
        let raw = self.checkpoint.0..self.arena.bytes.len();
        if let Some(staged) = self.staged.take() {
            self.arena.merge(0, staged, 0);
        }
        raw
    }
}
impl Drop for Staged<'_> {
    fn drop(&mut self) {
        if self.staged.is_some() {
            self.arena.bytes.truncate(self.checkpoint.0);
            self.arena.nodes.truncate(self.checkpoint.1);
        }
    }
}

#[derive(Clone)]
pub struct ScalarEdit {
    node: usize,
    entry: usize,
    value: Value,
}
