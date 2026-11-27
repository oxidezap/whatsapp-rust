//! Typed views and builders for the pinned WhatsApp protobuf schema.
//!
//! Parsing indexes and validates one message without eagerly indexing nested
//! bodies. Named getters validate nested messages when accessed. `validate_deep`
//! checks every nested wire occurrence and then required fields in the effective
//! merged state, using a bounded worklist rather than the native call stack.
//!
//! Field tokens bind builders to the declaring message, value type and
//! cardinality. Scalar getters preserve presence; `get_or_default` applies the
//! declared proto2 default. Closed unknown enum numbers remain unknown fields.
//! Packed unknown enum members are exposed individually by `unknown_fields`;
//! their `raw` is absent, but `encode` writes their complete field representation.
//!
//! Builders retain untouched wire bytes, including unknown fields. Replacing a
//! field clears recognized values only. A clear of an inactive oneof member is
//! a no-op. Synthetic map entries emit default key/value slots, matching the
//! protobuf map-entry encoding contract. Map views use last-key-wins semantics.
//! `finish` checks the local schema; `finish_checked` also validates the complete
//! nested graph. Deep validation is explicit so simple field access does not
//! link every nested validator into small applications.
use super::indexed_wire::{self as protobuf, Field as WireField, Reader, Value};
use alloc::{boxed::Box, collections::BTreeMap, sync::Arc, vec, vec::Vec};
use core::{fmt, marker::PhantomData};

mod media_short;

pub(crate) fn media_retry_success(input: &[u8], limits: Limits) -> Option<(&str, &str)> {
    if let Some((success, _)) = parse_media_retry_success(input, limits) {
        return Some((success.id, success.path));
    }
    media_short::project_short_media_success(input, limits)
}

pub(crate) fn merge_generated_view<'a, V: super::MessageView<'a>>(
    view: &mut V,
    mut input: &'a [u8],
    context: super::DecodeContext<'_>,
) -> Result<(), super::DecodeError> {
    while !input.is_empty() {
        let Some((tag, tag_len)) = super::encoding::probe_tag(input) else {
            return view.merge_into_view(input, context);
        };
        let Some(payload) = input.get(tag_len..) else {
            return view.merge_into_view(input, context);
        };
        input = view.merge_view_field(tag, payload, input, context)?;
    }
    Ok(())
}

fn parse_media_retry_success(
    input: &[u8],
    limits: Limits,
) -> Option<(InlineMediaSuccess<'_>, usize)> {
    const ID: u8 = (MediaRetryNotification::STANZA_ID.spec.number() * 8 + 2) as u8;
    const PATH: u8 = (MediaRetryNotification::DIRECT_PATH.spec.number() * 8 + 2) as u8;
    const RESULT: u8 = (MediaRetryNotification::RESULT.spec.number() * 8) as u8;
    const SUCCESS: u8 = media_retry_notification::ResultType::SUCCESS.number() as u8;
    let body = input.strip_suffix(&[RESULT, SUCCESS])?;
    let [ID, id_len @ 0..=127, rest @ ..] = body else {
        return None;
    };
    let (id, rest) = rest.split_at_checked(usize::from(*id_len))?;
    let [PATH, path_len @ 0..=127, path @ ..] = rest else {
        return None;
    };
    if usize::from(*path_len) != path.len()
        || input.len() > limits.max_bytes
        || limits.max_fields < 3
        || limits.max_values < 3
        || limits.max_fragments == 0
    {
        return None;
    }
    let charge = View::index_charge(
        &Parts::one(input),
        &MediaRetryNotification::SCHEMA,
        limits,
        3,
    )
    .ok()?;
    // The one-byte tags and lengths are ASCII, so no UTF-8 sequence can span a field header.
    let text = core::str::from_utf8(body).ok()?;
    let id_end = 2 + id.len();
    Some((
        InlineMediaSuccess {
            id: text.get(2..id_end)?,
            path: text.get(id_end + 2..)?,
        },
        charge,
    ))
}

#[cold]
#[inline(never)]
fn parse_long_media_retry_success(
    input: &[u8],
    limits: Limits,
) -> Option<(InlineMediaSuccess<'_>, usize)> {
    const ID: u8 = (MediaRetryNotification::STANZA_ID.spec.number() * 8 + 2) as u8;
    const PATH: u8 = (MediaRetryNotification::DIRECT_PATH.spec.number() * 8 + 2) as u8;
    const RESULT: u8 = (MediaRetryNotification::RESULT.spec.number() * 8) as u8;
    const SUCCESS: u8 = media_retry_notification::ResultType::SUCCESS.number() as u8;
    let body = input.strip_suffix(&[RESULT, SUCCESS])?;
    let [ID, id_len @ 0..=127, rest @ ..] = body else {
        return None;
    };
    let (id, rest) = rest.split_at_checked(usize::from(*id_len))?;
    let [PATH, length_and_path @ ..] = rest else {
        return None;
    };
    let [low @ 128..=255, high @ 1..=127, path @ ..] = length_and_path else {
        return None;
    };
    let length = usize::from(*low & 127) | (usize::from(*high) << 7);
    if length != path.len() {
        return None;
    }
    if input.len() > limits.max_bytes
        || limits.max_fields < 3
        || limits.max_values < 3
        || limits.max_fragments == 0
    {
        return None;
    }
    let charge = View::index_charge(
        &Parts::one(input),
        &MediaRetryNotification::SCHEMA,
        limits,
        3,
    )
    .ok()?;
    let id = core::str::from_utf8(id).ok()?;
    let path = core::str::from_utf8(path).ok()?;
    Some((InlineMediaSuccess { id, path }, charge))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_fields: usize,
    pub max_values: usize,
    pub max_depth: usize,
    pub max_fragments: usize,
    /// Index and validation-worklist admission budget for one operation.
    pub max_index_bytes: usize,
    /// Includes output staging and conservative retained child-view charges.
    pub max_builder_bytes: usize,
}
impl Limits {
    const DEFAULT: Self = Self {
        max_bytes: 16 * 1024 * 1024,
        max_fields: 100_000,
        max_values: 100_000,
        max_depth: 64,
        max_fragments: 128,
        max_index_bytes: 16 * 1024 * 1024,
        max_builder_bytes: 32 * 1024 * 1024,
    };
}
impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Wire(protobuf::DecodeError),
    Type,
    Utf8,
    Required(u32),
    Limit,
}
impl From<protobuf::DecodeError> for Error {
    fn from(value: protobuf::DecodeError) -> Self {
        Self::Wire(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(e) => write!(f, "schema protobuf {e}"),
            Self::Type => f.write_str("schema field wire type mismatch"),
            Self::Utf8 => f.write_str("schema string is not UTF-8"),
            Self::Required(n) => write!(f, "required schema field {n} is missing"),
            Self::Limit => f.write_str("schema resource limit exceeded"),
        }
    }
}
impl core::error::Error for Error {}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Double,
    Float,
    Int64,
    Uint64,
    Int32,
    Fixed64,
    Fixed32,
    Bool,
    String,
    Group,
    Message,
    Bytes,
    Uint32,
    Enum,
    Sfixed32,
    Sfixed64,
    Sint32,
    Sint64,
}
impl Kind {
    fn wire(self) -> u8 {
        match self {
            Self::Double | Self::Fixed64 | Self::Sfixed64 => 1,
            Self::Float | Self::Fixed32 | Self::Sfixed32 => 5,
            Self::String | Self::Message | Self::Bytes => 2,
            Self::Group => 3,
            _ => 0,
        }
    }
    fn packable(self) -> bool {
        !matches!(
            self,
            Self::String | Self::Message | Self::Bytes | Self::Group
        )
    }
}
struct FieldOptions {
    repeated: bool,
    packed: bool,
    required: bool,
    open: bool,
}
#[derive(Clone, Copy)]
#[repr(C)]
struct FieldHeader {
    number: u32,
    attributes: u8,
    oneof_bytes: [u8; 2],
    kind: Kind,
}
#[derive(Clone, Copy)]
struct Spec {
    header: FieldHeader,
    enumeration: &'static [i32],
}
impl Spec {
    const fn new(
        number: u32,
        kind: Kind,
        options: FieldOptions,
        oneof: Option<u16>,
        enumeration: &'static [i32],
    ) -> Self {
        let attributes = (options.repeated as u8)
            | ((options.packed as u8) << 1)
            | ((options.required as u8) << 2)
            | ((options.open as u8) << 3)
            | ((oneof.is_some() as u8) << 4);
        let oneof_bytes = match oneof {
            Some(value) => value.to_le_bytes(),
            None => [0; 2],
        };
        Self {
            header: FieldHeader {
                number,
                attributes,
                oneof_bytes,
                kind,
            },
            enumeration,
        }
    }
    const fn number(self) -> u32 {
        self.header.number
    }
    const fn kind(self) -> Kind {
        self.header.kind
    }
    const fn repeated(self) -> bool {
        self.header.attributes & 1 != 0
    }
    const fn packed(self) -> bool {
        self.header.attributes & 2 != 0
    }
    const fn required(self) -> bool {
        self.header.attributes & 4 != 0
    }
    const fn open(self) -> bool {
        self.header.attributes & 8 != 0
    }
    const fn oneof(self) -> Option<usize> {
        if self.header.attributes & 16 != 0 {
            Some(u16::from_le_bytes(self.header.oneof_bytes) as usize)
        } else {
            None
        }
    }
    fn accepts(self, value: Value<'_>) -> bool {
        wire(value) == self.kind().wire()
            || self.repeated() && self.kind().packable() && matches!(value, Value::Bytes(_))
    }
    fn known(self, value: Value<'_>) -> bool {
        self.accepts(value) && self.known_accepted(value)
    }
    fn known_accepted(self, value: Value<'_>) -> bool {
        self.kind() != Kind::Enum
            || self.open()
            || match value {
                Value::Varint(n) => self.enumeration.contains(&(n as i32)),
                Value::Bytes(_) if self.repeated() => true,
                _ => false,
            }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct SchemaIdentity(u32);

pub struct Schema {
    map_defaults: &'static [(usize, Value<'static>)],
    identity: SchemaIdentity,
    required_prefix: u32,
    fields: &'static [Spec],
    oneofs: usize,
}
impl Schema {
    fn has_repeated(&self, number: protobuf::FieldNumber) -> bool {
        let start = self
            .fields
            .partition_point(|spec| spec.number() < number.get());
        self.fields[start..]
            .iter()
            .take_while(|spec| spec.number() == number.get())
            .any(|spec| spec.repeated())
    }

    fn classify(
        &self,
        number: protobuf::FieldNumber,
        value: Value<'_>,
    ) -> Result<Option<Spec>, Error> {
        let start = if self
            .fields
            .first()
            .is_some_and(|spec| spec.number() == number.get())
        {
            0
        } else {
            self.fields
                .partition_point(|spec| spec.number() < number.get())
        };
        let mut declared = false;
        for spec in &self.fields[start..] {
            if spec.number() != number.get() {
                break;
            }
            declared = true;
            if spec.accepts(value) {
                return Ok(spec.known_accepted(value).then_some(*spec));
            }
        }
        if declared { Err(Error::Type) } else { Ok(None) }
    }
}
fn wire(value: Value<'_>) -> u8 {
    match value {
        Value::Varint(_) => 0,
        Value::Fixed64(_) => 1,
        Value::Bytes(_) => 2,
        Value::Group(_) => 3,
        Value::Fixed32(_) => 5,
    }
}
#[derive(Clone)]
struct Parts<'a> {
    first: &'a [u8],
    rest: Vec<&'a [u8]>,
}
impl<'a> Parts<'a> {
    fn one(bytes: &'a [u8]) -> Self {
        Self {
            first: bytes,
            rest: Vec::new(),
        }
    }
    fn iter(&self) -> impl Iterator<Item = &'a [u8]> {
        core::iter::once(self.first).chain(self.rest.iter().copied())
    }
    fn len(&self) -> Result<usize, Error> {
        self.iter()
            .try_fold(0usize, |n, p| n.checked_add(p.len()).ok_or(Error::Limit))
    }
    fn push(&mut self, bytes: &'a [u8], limits: Limits) -> Result<(), Error> {
        if self.rest.len() + 1 >= limits.max_fragments {
            return Err(Error::Limit);
        }
        self.rest.push(bytes);
        Ok(())
    }
}
#[derive(Clone, Copy)]
struct ShortByteField {
    number: protobuf::FieldNumber,
    offset: u16,
    length: u16,
}
impl ShortByteField {
    fn value(self, input: &[u8]) -> Value<'_> {
        let start = usize::from(self.offset);
        Value::Bytes(&input[start..start + usize::from(self.length)])
    }
}
struct PeriodicTail {
    tag: protobuf::FieldNumber,
    last_span: u32,
    count: u16,
    stride: u16,
    lanes_shift: u8,
}
impl PeriodicTail {
    fn len(&self) -> usize {
        usize::from(self.count)
    }
    fn bytes<'a>(&self, position: usize, input: &'a [u8]) -> &'a [u8] {
        let length = self.last_span as u16 as usize;
        let lane = position & ((1 << self.lanes_shift) - 1);
        let start =
            2 + (position >> self.lanes_shift) * usize::from(self.stride) + lane * (2 + length);
        &input[start..start + length]
    }
}
enum IndexTail<'a> {
    Fragments(Vec<&'a [u8]>),
    Wire(WireIndex),
    BytePair([ShortByteField; 2]),
    Periodic(PeriodicTail),
    Text(&'a str),
    CompactText(&'a str),
    ShortByteRun([u16; 8]),
    ShortTaggedRun([u8; 8], u16),
}
struct IndexParts<'a> {
    first: &'a [u8],
    tail: IndexTail<'a>,
}
impl<'a> IndexParts<'a> {
    fn rest(&self) -> &[&'a [u8]] {
        match &self.tail {
            IndexTail::Fragments(parts) => parts,
            IndexTail::Text(_)
            | IndexTail::CompactText(_)
            | IndexTail::Wire(_)
            | IndexTail::BytePair(_)
            | IndexTail::Periodic(_)
            | IndexTail::ShortByteRun(_)
            | IndexTail::ShortTaggedRun(..) => &[],
        }
    }
    fn rest_capacity(&self) -> usize {
        match &self.tail {
            IndexTail::Fragments(parts) => parts.capacity(),
            IndexTail::Text(_)
            | IndexTail::CompactText(_)
            | IndexTail::Wire(_)
            | IndexTail::BytePair(_)
            | IndexTail::Periodic(_)
            | IndexTail::ShortByteRun(_)
            | IndexTail::ShortTaggedRun(..) => 0,
        }
    }
    fn iter(&self) -> impl Iterator<Item = &'a [u8]> {
        core::iter::once(self.first).chain(self.rest().iter().copied())
    }
    fn len(&self) -> Result<usize, Error> {
        self.iter()
            .try_fold(0usize, |n, p| n.checked_add(p.len()).ok_or(Error::Limit))
    }
}
#[derive(Clone, Copy)]
enum IndexedValue<'a> {
    Varint(protobuf::FieldNumber, u64),
    Fixed64(protobuf::FieldNumber, u64),
    Bytes(protobuf::FieldNumber, &'a [u8]),
    Group(protobuf::FieldNumber, &'a [u8]),
    Fixed32(protobuf::FieldNumber, u32),
}
#[derive(Clone, Copy)]
struct Indexed<'a> {
    entry: IndexedValue<'a>,
    order: usize,
}
impl<'a> Indexed<'a> {
    fn new(number: protobuf::FieldNumber, value: Value<'a>, order: usize) -> Self {
        let entry = match value {
            Value::Varint(v) => IndexedValue::Varint(number, v),
            Value::Fixed64(v) => IndexedValue::Fixed64(number, v),
            Value::Bytes(v) => IndexedValue::Bytes(number, v),
            Value::Group(v) => IndexedValue::Group(number, v),
            Value::Fixed32(v) => IndexedValue::Fixed32(number, v),
        };
        Self { entry, order }
    }
    fn number(&self) -> protobuf::FieldNumber {
        match self.entry {
            IndexedValue::Varint(n, _)
            | IndexedValue::Fixed64(n, _)
            | IndexedValue::Bytes(n, _)
            | IndexedValue::Group(n, _)
            | IndexedValue::Fixed32(n, _) => n,
        }
    }
    fn value(&self) -> Value<'a> {
        match self.entry {
            IndexedValue::Varint(_, v) => Value::Varint(v),
            IndexedValue::Fixed64(_, v) => Value::Fixed64(v),
            IndexedValue::Bytes(_, v) => Value::Bytes(v),
            IndexedValue::Group(_, v) => Value::Group(v),
            IndexedValue::Fixed32(_, v) => Value::Fixed32(v),
        }
    }
}
#[derive(Clone, Copy)]
struct Active {
    number: u32,
    since: usize,
}
#[derive(Clone, Copy)]
struct WireIndex {
    number: protobuf::FieldNumber,
    order_and_wire: u32,
    payload: u64,
}
impl WireIndex {
    // The upper bits retain the wire type without widening the entry.
    const ORDER_MASK: u32 = (1 << 29) - 1;

    fn new(number: protobuf::FieldNumber, value: Value<'_>, order: usize, input: &[u8]) -> Self {
        let payload = match value {
            Value::Varint(value) | Value::Fixed64(value) => value,
            Value::Fixed32(value) => u64::from(value),
            Value::Bytes(bytes) | Value::Group(bytes) => {
                // Compact indexes only borrow payloads from the one indexed fragment.
                let offset = bytes.as_ptr().addr() - input.as_ptr().addr();
                (offset as u64) << 32 | bytes.len() as u64
            }
        };
        Self {
            number,
            order_and_wire: order as u32 | (u32::from(wire(value)) << 29),
            payload,
        }
    }
    fn order(self) -> usize {
        (self.order_and_wire & Self::ORDER_MASK) as usize
    }
    fn value(self, input: &[u8]) -> Value<'_> {
        let kind = self.order_and_wire >> 29;
        match kind {
            0 => Value::Varint(self.payload),
            1 => Value::Fixed64(self.payload),
            5 => Value::Fixed32(self.payload as u32),
            2 | 3 => {
                let bytes = wire_bytes(self.payload, input);
                if kind == 2 {
                    Value::Bytes(bytes)
                } else {
                    Value::Group(bytes)
                }
            }
            _ => unreachable!(),
        }
    }
    fn indexed(self, input: &[u8]) -> Indexed<'_> {
        Indexed::new(self.number, self.value(input), self.order())
    }
}
#[cold]
#[inline(never)]
fn sort_wire_index(fields: &mut [WireIndex]) {
    fn pass(src: &[WireIndex], dst: &mut [WireIndex], shift: u32) -> bool {
        let digit = |field: &WireIndex| ((field.number.get() >> shift) & 255) as usize;
        let mut starts = [0usize; 256];
        for field in src {
            starts[digit(field)] += 1;
        }
        if starts[digit(&src[0])] == src.len() {
            return false;
        }
        let mut total = 0;
        for start in &mut starts {
            let count = *start;
            *start = total;
            total += count;
        }
        for field in src {
            let start = &mut starts[digit(field)];
            dst[*start] = *field;
            *start += 1;
        }
        true
    }

    let mut scratch = fields.to_vec();
    let mut in_fields = true;
    for shift in [0, 8, 16, 24] {
        let moved = if in_fields {
            pass(fields, &mut scratch, shift)
        } else {
            pass(&scratch, fields, shift)
        };
        in_fields ^= moved;
    }
    if !in_fields {
        fields.copy_from_slice(&scratch);
    }
}

fn wire_bytes(payload: u64, input: &[u8]) -> &[u8] {
    let offset = (payload >> 32) as usize;
    let length = payload as u32 as usize;
    &input[offset..offset + length]
}

#[inline(never)]
fn checked_text(bytes: &[u8]) -> Result<&str, Error> {
    core::str::from_utf8(bytes).map_err(|_| Error::Utf8)
}

fn valid_short_text(bytes: &[u8]) -> bool {
    if let Ok(bytes) = <[u8; 2]>::try_from(bytes) {
        let [first, second] = bytes;
        return u16::from_ne_bytes(bytes) & 0x8080 == 0
            || (0xc2..=0xdf).contains(&first) && second & 0xc0 == 0x80;
    }
    let Ok(bytes) = <[u8; 4]>::try_from(bytes) else {
        return false;
    };
    if u32::from_ne_bytes(bytes) & 0x8080_8080 == 0 {
        return true;
    }
    let [first, second, third, fourth] = bytes;
    if (0xc2..=0xdf).contains(&first) {
        return (0xc2..=0xdf).contains(&third) && second & 0xc0 == 0x80 && fourth & 0xc0 == 0x80;
    }
    [second, third, fourth]
        .iter()
        .all(|byte| byte & 0xc0 == 0x80)
        && match first {
            0xf0 => second >= 0x90,
            0xf1..=0xf3 => true,
            0xf4 => second < 0x90,
            _ => false,
        }
}

fn two_f0_scalars(chunk: u64) -> bool {
    // F0 scalars need continuation bytes and second bytes above 0x8f.
    chunk & 0xc0c0_c0ff_c0c0_c0ff == 0x8080_80f0_8080_80f0
        && chunk & 0x0000_0000_0000_3000 != 0
        && chunk & 0x0000_3000_0000_0000 != 0
}

#[inline(never)]
#[cold]
fn validate_f0_prefix(bytes: &[u8]) -> Result<(), Error> {
    let mut suffix = bytes;
    while let Some((word, tail)) = suffix.split_first_chunk::<8>() {
        if !two_f0_scalars(u64::from_le_bytes(*word)) {
            break;
        }
        suffix = tail;
    }
    checked_text(suffix).map(|_| ())
}

fn validate_preflight_text(bytes: &[u8]) -> Result<(), Error> {
    if valid_short_text(bytes) {
        return Ok(());
    }
    if bytes.len() >= 128 && bytes[0] == 0xf0 {
        return validate_f0_prefix(bytes);
    }
    let mut suffix = bytes;
    if bytes.len() == 32 && (bytes[0] < 128 || bytes[0] == 0xf0) {
        while let Some((word, tail)) = suffix.split_first_chunk::<8>() {
            let chunk = u64::from_le_bytes(*word);
            let mask = chunk & 0x8080_8080_8080_8080;
            if mask != 0 && !two_f0_scalars(chunk) {
                suffix = &suffix[mask.trailing_zeros() as usize / 8..];
                break;
            }
            suffix = tail;
        }
        if suffix.is_empty() {
            return Ok(());
        }
    }
    checked_text(suffix).map(|_| ())
}

fn short_byte_run(fields: &[u16; 8]) -> &[u16] {
    // Every payload follows its wire header, so zero marks an unused slot.
    let end = if fields[6] != 0 {
        7 + usize::from(fields[7] != 0)
    } else {
        5 + usize::from(fields[5] != 0)
    };
    &fields[..end]
}

fn short_byte_run_bytes(field: u16, input: &[u8]) -> &[u8] {
    let start = usize::from(field >> 8);
    &input[start..start + usize::from(field as u8)]
}

fn short_wire_bytes(payload: u32, input: &[u8]) -> &[u8] {
    let offset = (payload >> 16) as usize;
    let length = payload as u16 as usize;
    &input[offset..offset + length]
}

#[derive(Clone, Copy)]
enum SameBytesLayout {
    MonoNarrow(protobuf::FieldNumber),
    Mixed,
    SortedRuns { width: u8, periodic: bool },
}

struct SameBytesIndex {
    layout: SameBytesLayout,
    // Mono entries store the last byte range, the count, then paired length-prefix offsets.
    fields: Box<[u32]>,
}

impl SameBytesIndex {
    const DIRECTORY: usize = 33;
    const MONO_STRIDE_WORDS: usize = 5;
    fn is_narrow(&self) -> bool {
        match self.layout {
            SameBytesLayout::MonoNarrow(_) => true,
            SameBytesLayout::Mixed => self.fields[0] == 0,
            SameBytesLayout::SortedRuns { .. } => true,
        }
    }
    fn start(&self) -> usize {
        match self.layout {
            SameBytesLayout::MonoNarrow(_) => 0,
            SameBytesLayout::Mixed | SameBytesLayout::SortedRuns { .. } => Self::DIRECTORY,
        }
    }
    fn len(&self) -> usize {
        self.end() - self.start()
    }
    fn end(&self) -> usize {
        match self.layout {
            SameBytesLayout::MonoNarrow(_) => self.fields[1] as usize,
            SameBytesLayout::Mixed => self.fields[Self::DIRECTORY - 1] as usize,
            SameBytesLayout::SortedRuns { width, .. } => {
                self.fields[usize::from(width) - 1] as usize
            }
        }
    }
    fn bytes<'a>(&self, number: u32, position: usize, input: &'a [u8]) -> &'a [u8] {
        match self.layout {
            SameBytesLayout::MonoNarrow(_) => {
                if position + 1 == self.end() {
                    return short_wire_bytes(self.fields[0], input);
                }
                let offset = if self.fields.len() == Self::MONO_STRIDE_WORDS {
                    let word = self.fields[3 + (position % 4) / 2];
                    usize::from((word >> ((position % 2) * 16)) as u16)
                        + (position / 4) * self.fields[2] as usize
                } else {
                    let word = self.fields[2 + position / 2];
                    usize::from((word >> ((position % 2) * 16)) as u16)
                };
                let (length, width) = if input[offset] < 128 {
                    (usize::from(input[offset]), 1)
                } else {
                    let (length, width) =
                        protobuf::decode_varint(&input[offset..]).expect("indexed protobuf length");
                    (length as usize, width)
                };
                &input[offset + width..offset + width + length]
            }
            SameBytesLayout::Mixed if self.fields[0] == 0 => {
                let entry = position - Self::DIRECTORY;
                let word = self.fields[Self::DIRECTORY + entry / 2];
                let offset = ((word >> ((entry % 2) * 16)) as u16) as usize;
                let length = usize::from(input[offset - 1]);
                &input[offset..offset + length]
            }
            SameBytesLayout::SortedRuns { width, periodic } => self.run_bytes(
                number as usize,
                position,
                usize::from(width),
                periodic,
                input,
            ),
            _ => short_wire_bytes(self.fields[position], input),
        }
    }
    #[cold]
    #[inline(never)]
    fn run_bytes<'a>(
        &self,
        number: usize,
        position: usize,
        width: usize,
        periodic: bool,
        input: &'a [u8],
    ) -> &'a [u8] {
        let run = self.fields[width + number];
        let local = position - self.fields[number] as usize;
        let offset = if periodic {
            let base = if local.is_multiple_of(2) {
                run as u16 as usize
            } else {
                self.fields[2 * width + number] as usize
            };
            base + (run >> 16) as usize * (local / 2)
        } else {
            run as u16 as usize + (run >> 16) as usize * local
        };
        let length = usize::from(input[offset - 1]);
        &input[offset..offset + length]
    }
    fn range(&self, number: u32) -> core::ops::Range<usize> {
        let width = match self.layout {
            SameBytesLayout::MonoNarrow(tag) => {
                return if tag.get() == number {
                    0..self.end()
                } else {
                    0..0
                };
            }
            SameBytesLayout::Mixed => Self::DIRECTORY,
            SameBytesLayout::SortedRuns { width, .. } => usize::from(width),
        };
        let number = number as usize;
        if number + 1 >= width {
            return 0..0;
        }
        self.fields[number] as usize..self.fields[number + 1] as usize
    }
    fn next<'a>(&self, cursor: &mut usize, input: &'a [u8]) -> Option<Indexed<'a>> {
        let position = *cursor & 0xffff;
        let end = *cursor >> 16;
        if position == end {
            return None;
        }
        let number = match self.layout {
            SameBytesLayout::MonoNarrow(number) => number,
            SameBytesLayout::Mixed => self.directory_number(position, Self::DIRECTORY)?,
            SameBytesLayout::SortedRuns { width, .. } => {
                self.directory_number(position, usize::from(width))?
            }
        };
        *cursor += 1;
        Some(Indexed::new(
            number,
            Value::Bytes(self.bytes(number.get(), position, input)),
            0,
        ))
    }
    fn directory_number(&self, position: usize, width: usize) -> Option<protobuf::FieldNumber> {
        let number =
            self.fields[..width].partition_point(|&offset| offset as usize <= position) - 1;
        protobuf::FieldNumber::new(number as u32).ok()
    }
}

#[derive(Clone, Copy)]
struct SingleText<'a> {
    number: protobuf::FieldNumber,
    text: &'a str,
}
impl<'a> SingleText<'a> {
    fn value(self) -> Value<'a> {
        Value::Bytes(self.text.as_bytes())
    }
    fn indexed(self) -> Indexed<'a> {
        Indexed::new(self.number, self.value(), 0)
    }
}
#[derive(Clone, Copy)]
struct PackedByteField(u32);
impl PackedByteField {
    fn number(self) -> u32 {
        self.0 >> 16
    }
    fn value(self, input: &[u8]) -> Value<'_> {
        let start = usize::from(self.0 as u16 >> 8);
        Value::Bytes(&input[start..start + usize::from(self.0 as u8)])
    }
}

enum CompactIndex {
    BytePair([ShortByteField; 2]),
    ByteTriple([PackedByteField; 3]),
    SameBytes(SameBytesIndex),
    General(Box<[WireIndex]>),
    ByteQuad([PackedByteField; 4]),
}
enum IndexStorage<'a> {
    Compact(CompactIndex),
    Many {
        fields: Box<[Indexed<'a>]>,
        active: Box<[Option<Active>]>,
    },
}
struct ShortByteRunIter<'v, 'a> {
    fields: &'v [u16; 8],
    view: &'v View<'a>,
    position: usize,
}
impl<'a> ShortByteRunIter<'_, 'a> {
    fn next_bytes(&mut self) -> Option<&'a [u8]> {
        let &field = self.fields.get(self.position)?;
        if field == 0 {
            return None;
        }
        self.position += 1;
        Some(short_byte_run_bytes(field, self.view.wire_fragments().0))
    }

    fn next_indexed(&mut self) -> Option<Indexed<'a>> {
        let bytes = self.next_bytes()?;
        let number = protobuf::FieldNumber::new(self.view.schema.fields[0].number())
            .expect("generated field number");
        Some(Indexed::new(number, Value::Bytes(bytes), 0))
    }
}

struct ShortTaggedRunIter<'v, 'a> {
    offsets: &'v [u8; 8],
    input: &'a [u8],
    position: u8,
    tag: u8,
}
impl<'a> ShortTaggedRunIter<'_, 'a> {
    fn next_field(&mut self) -> Option<(u8, &'a [u8])> {
        while let Some(&offset) = self.offsets.get(usize::from(self.position)) {
            if offset == 0 {
                return None;
            }
            self.position += 1;
            let start = usize::from(offset);
            let tag = self.input[start - 2] >> 3;
            if tag == self.tag {
                let length = usize::from(self.input[start - 1]);
                return Some((tag, &self.input[start..start + length]));
            }
        }
        None
    }
}

enum GeneralIndexIter<'v, 'a> {
    BytePair(core::slice::Iter<'v, ShortByteField>, &'a [u8]),
    ShortByteRun(ShortByteRunIter<'v, 'a>),
    ShortTaggedRun(ShortTaggedRunIter<'v, 'a>),
    ByteTriple(core::slice::Iter<'v, PackedByteField>, &'a [u8]),
    Compact(core::slice::Iter<'v, WireIndex>, &'a [u8]),
    Many(core::slice::Iter<'v, Indexed<'a>>),
}
impl<'a> GeneralIndexIter<'_, 'a> {
    fn next_value(&mut self) -> Option<Value<'a>> {
        match self {
            Self::BytePair(fields, input) => fields.next().map(|field| field.value(input)),
            Self::ByteTriple(fields, input) => fields.next().map(|field| field.value(input)),
            Self::ShortByteRun(fields) => fields.next_bytes().map(Value::Bytes),
            Self::ShortTaggedRun(fields) => {
                fields.next_field().map(|(_, bytes)| Value::Bytes(bytes))
            }
            Self::Many(fields) => fields.next().map(Indexed::value),
            Self::Compact(fields, input) => fields.next().map(|field| field.value(input)),
        }
    }
}
impl<'a> Iterator for GeneralIndexIter<'_, 'a> {
    type Item = Indexed<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::BytePair(fields, input) => fields
                .next()
                .map(|field| Indexed::new(field.number, field.value(input), 0)),
            Self::ShortByteRun(fields) => fields.next_indexed(),
            Self::ShortTaggedRun(fields) => fields.next_field().map(|(tag, bytes)| {
                let number =
                    protobuf::FieldNumber::new(u32::from(tag)).expect("indexed field number");
                Indexed::new(number, Value::Bytes(bytes), 0)
            }),
            Self::ByteTriple(fields, input) => fields.next().map(|field| {
                let number =
                    protobuf::FieldNumber::new(field.number()).expect("packed field number");
                Indexed::new(number, field.value(input), 0)
            }),
            Self::Many(fields) => fields.next().copied(),
            Self::Compact(fields, input) => fields.next().map(|field| field.indexed(input)),
        }
    }
}
enum IndexIter<'v, 'a> {
    ShortTagNumbers(u16),
    MediaSuccess(&'v InlineMediaSuccess<'a>, core::ops::RangeInclusive<u32>),
    One(Option<Indexed<'a>>),
    SameBytes(&'v SameBytesIndex, usize, &'a [u8]),
    Periodic(&'v PeriodicTail, usize, &'a [u8]),
    General(GeneralIndexIter<'v, 'a>),
}
impl<'a> Iterator for IndexIter<'_, 'a> {
    type Item = Indexed<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::ShortTagNumbers(tags) => {
                if *tags == 0 {
                    return None;
                }
                let number = tags.trailing_zeros();
                *tags &= *tags - 1;
                let number = protobuf::FieldNumber::new(number).expect("indexed field number");
                Some(Indexed::new(number, Value::Bytes(&[]), 0))
            }
            Self::Periodic(index, position, input) => {
                if *position == index.len() {
                    return None;
                }
                let bytes = index.bytes(*position, input);
                *position += 1;
                Some(Indexed::new(index.tag, Value::Bytes(bytes), 0))
            }
            Self::MediaSuccess(success, fields) => {
                fields.next().and_then(|number| success.indexed(number))
            }
            Self::One(field) => field.take(),
            Self::General(fields) => fields.next(),
            Self::SameBytes(index, cursor, input) => index.next(cursor, input),
        }
    }
}
impl<'a> IndexStorage<'a> {
    fn len(&self) -> usize {
        match self {
            Self::Compact(CompactIndex::BytePair(_)) => 2,
            Self::Compact(CompactIndex::ByteTriple(_)) => 3,
            Self::Compact(CompactIndex::ByteQuad(_)) => 4,
            Self::Compact(CompactIndex::General(fields)) => fields.len(),
            Self::Compact(CompactIndex::SameBytes(index)) => index.len(),
            Self::Many { fields, .. } => fields.len(),
        }
    }
    fn iter(&self, input: &'a [u8]) -> IndexIter<'_, 'a> {
        match self {
            Self::Many { fields, .. } => IndexIter::General(GeneralIndexIter::Many(fields.iter())),
            Self::Compact(CompactIndex::SameBytes(index)) => {
                let start = index.start();
                IndexIter::SameBytes(index, (index.end() << 16) | start, input)
            }
            Self::Compact(CompactIndex::BytePair(fields)) => {
                IndexIter::General(GeneralIndexIter::BytePair(fields.iter(), input))
            }
            Self::Compact(CompactIndex::ByteTriple(fields)) => {
                IndexIter::General(GeneralIndexIter::ByteTriple(fields.iter(), input))
            }
            Self::Compact(CompactIndex::ByteQuad(fields)) => {
                IndexIter::General(GeneralIndexIter::ByteTriple(fields.iter(), input))
            }
            Self::Compact(CompactIndex::General(fields)) => {
                IndexIter::General(GeneralIndexIter::Compact(fields.iter(), input))
            }
        }
    }
}
struct Index<'a> {
    storage: IndexStorage<'a>,
    parts: IndexParts<'a>,
    work_fields: usize,
    values: usize,
    charge: usize,
}
impl Index<'_> {
    // Each charged field consumes at least two bytes of bounded input.
    const FILTERED: usize = 1 << (usize::BITS - 1);

    const CHARGE_HEADER: usize = core::mem::size_of::<Self>() + 4 * core::mem::size_of::<usize>();

    fn work_fields(&self) -> usize {
        self.work_fields & !Self::FILTERED
    }

    fn field_count(&self) -> usize {
        if self.work_fields & Self::FILTERED == 0 {
            return match self.parts.tail {
                IndexTail::Text(_) | IndexTail::Wire(_) => 1,
                IndexTail::BytePair(_) => 2,
                IndexTail::Periodic(ref index) => index.len(),
                // These runs only retain length-delimited fields, which each consume one unit of work.
                IndexTail::ShortByteRun(_) | IndexTail::ShortTaggedRun(..) => self.work_fields,
                IndexTail::Fragments(_) | IndexTail::CompactText(_) => self.storage.len(),
            };
        }
        let header = Self::CHARGE_HEADER;
        let slots = match &self.storage {
            IndexStorage::Compact(_) => 0,
            IndexStorage::Many { active, .. } => core::mem::size_of_val(active.as_ref()),
        };
        let fragments = self.parts.rest_capacity() * core::mem::size_of::<&[u8]>();
        (self.charge - header - slots - fragments) / core::mem::size_of::<(WireField<'_>, usize)>()
    }
}
#[derive(Clone, Copy)]
enum RetainedKeys {
    Other,
    MixedBytes,
    ByteTag(protobuf::FieldNumber),
}

#[derive(Clone, Copy)]
struct IndexMeasurement {
    keys: RetainedKeys,
    omitted: usize,
    count: usize,
    remaining: usize,
    values: usize,
    charge: usize,
}
#[derive(Clone, Copy)]
struct InlineMediaSuccess<'a> {
    id: &'a str,
    path: &'a str,
}

impl<'a> InlineMediaSuccess<'a> {
    const ID: u32 = MediaRetryNotification::STANZA_ID.spec.number();
    const PATH: u32 = MediaRetryNotification::DIRECT_PATH.spec.number();
    const RESULT: u32 = MediaRetryNotification::RESULT.spec.number();
    const COUNT: usize = 3;
    const SUCCESS: media_retry_notification::ResultType =
        media_retry_notification::ResultType::SUCCESS;

    fn indexed(self, number: u32) -> Option<Indexed<'a>> {
        let value = match number {
            Self::ID => Value::Bytes(self.id.as_bytes()),
            Self::PATH => Value::Bytes(self.path.as_bytes()),
            Self::RESULT => Value::Varint(Self::SUCCESS.number() as u64),
            _ => return None,
        };
        Some(Indexed::new(
            protobuf::FieldNumber::new(number).ok()?,
            value,
            number as usize - 1,
        ))
    }
}

#[derive(Clone, Copy)]
enum InlineData<'a> {
    MediaSuccess(InlineMediaSuccess<'a>),
    Text(&'a str),
    Packed {
        bytes: &'a [u8],
        number: protobuf::FieldNumber,
        values: usize,
    },
}
#[derive(Clone, Copy)]
struct InlineView<'a> {
    input: &'a [u8],
    data: InlineData<'a>,
    charge: usize,
}
impl<'a> InlineView<'a> {
    fn indexed(self, schema: &Schema) -> Indexed<'a> {
        let (number, bytes) = match self.data {
            InlineData::Text(text) => (
                protobuf::FieldNumber::new(schema.fields[0].number())
                    .expect("generated schema field"),
                text.as_bytes(),
            ),
            InlineData::Packed { bytes, number, .. } => (number, bytes),
            InlineData::MediaSuccess(_) => unreachable!(),
        };
        Indexed::new(number, Value::Bytes(bytes), 0)
    }

    fn charge(self) -> usize {
        self.charge & !Index::FILTERED
    }

    fn field_count(self) -> usize {
        match self.data {
            InlineData::MediaSuccess(_) => {
                if self.charge & Index::FILTERED == 0 {
                    InlineMediaSuccess::COUNT
                } else {
                    (self.charge() - Index::CHARGE_HEADER)
                        / core::mem::size_of::<(WireField<'_>, usize)>()
                }
            }
            // Filtered inline text has no extra index storage beyond the wire field charge.
            InlineData::Text(_) if self.charge & Index::FILTERED != 0 => {
                (self.charge() - Index::CHARGE_HEADER)
                    / core::mem::size_of::<(WireField<'_>, usize)>()
            }
            InlineData::Text(_) | InlineData::Packed { .. } => 1,
        }
    }
}

#[derive(Clone)]
enum ViewBacking<'a> {
    Shared {
        index: Arc<Index<'a>>,
        limits: Limits,
        depth: usize,
    },
    Inline(InlineView<'a>),
}

impl ViewBacking<'_> {
    fn field_count(&self) -> usize {
        match self {
            ViewBacking::Shared { index, .. } => index.field_count(),
            ViewBacking::Inline(field) => field.field_count(),
        }
    }
}

#[derive(Clone)]
pub struct View<'a> {
    index: ViewBacking<'a>,
    schema: &'static Schema,
}

const _: () = assert!(core::mem::size_of::<View<'_>>() == 10 * core::mem::size_of::<usize>());

impl<'a> View<'a> {
    const FILTER_MIN_BYTES: usize = 32;
    fn shared_index(&self) -> Option<&Arc<Index<'a>>> {
        match &self.index {
            ViewBacking::Shared { index, .. } => Some(index),
            ViewBacking::Inline(_) => None,
        }
    }
    fn limits_ref(&self) -> &Limits {
        match &self.index {
            ViewBacking::Shared { limits, .. } => limits,
            ViewBacking::Inline(_) => &Limits::DEFAULT,
        }
    }
    fn limits(&self) -> Limits {
        *self.limits_ref()
    }
    fn depth(&self) -> usize {
        match self.index {
            ViewBacking::Shared { depth, .. } => depth,
            ViewBacking::Inline(_) => 0,
        }
    }
    fn ensure_shared(&mut self) {
        if let ViewBacking::Inline(field) = self.index {
            let mut fields: Box<[Indexed<'a>]> = Box::default();
            let (tail, values) = match field.data {
                InlineData::Text(text) => (IndexTail::Text(text), 1),
                InlineData::Packed {
                    bytes,
                    number,
                    values,
                } => (
                    IndexTail::Wire(WireIndex::new(number, Value::Bytes(bytes), 0, field.input)),
                    values,
                ),
                InlineData::MediaSuccess(success) => {
                    fields = Box::new(
                        [
                            InlineMediaSuccess::ID,
                            InlineMediaSuccess::PATH,
                            InlineMediaSuccess::RESULT,
                        ]
                        .map(|number| {
                            success
                                .indexed(number)
                                .expect("projected media retry field")
                        }),
                    );
                    (IndexTail::Fragments(Vec::new()), InlineMediaSuccess::COUNT)
                }
            };
            self.index = ViewBacking::Shared {
                index: Arc::new(Index {
                    storage: IndexStorage::Many {
                        fields,
                        active: Box::default(),
                    },
                    parts: IndexParts {
                        first: field.input,
                        tail,
                    },
                    work_fields: field.field_count() | (field.charge & Index::FILTERED),
                    values,
                    charge: field.charge(),
                }),
                limits: Limits::DEFAULT,
                depth: 0,
            };
        }
    }
    fn limits_mut(&mut self) -> &mut Limits {
        self.ensure_shared();
        let ViewBacking::Shared { limits, .. } = &mut self.index else {
            unreachable!();
        };
        limits
    }
    #[cfg(test)]
    fn depth_mut(&mut self) -> &mut usize {
        self.ensure_shared();
        let ViewBacking::Shared { depth, .. } = &mut self.index else {
            unreachable!();
        };
        depth
    }
    fn wire_parts(&self) -> impl Iterator<Item = &'a [u8]> + '_ {
        let (first, rest) = self.wire_fragments();
        core::iter::once(first).chain(rest.iter().copied())
    }
    fn wire_fragments(&self) -> (&'a [u8], &[&'a [u8]]) {
        match &self.index {
            ViewBacking::Shared { index, .. } => (index.parts.first, index.parts.rest()),
            ViewBacking::Inline(field) => (field.input, &[]),
        }
    }
    fn wire_len(&self) -> Result<usize, Error> {
        match &self.index {
            ViewBacking::Shared { index, .. } => index.parts.len(),
            ViewBacking::Inline(field) => Ok(field.input.len()),
        }
    }
    fn field_count(&self) -> usize {
        self.index.field_count()
    }
    fn charge(&self) -> usize {
        match &self.index {
            ViewBacking::Shared { index, .. } => index.charge,
            ViewBacking::Inline(field) => field.charge(),
        }
    }
    fn work_fields(&self) -> usize {
        match &self.index {
            ViewBacking::Shared { index, .. } => index.work_fields(),
            ViewBacking::Inline(field) => field.field_count(),
        }
    }
    fn values(&self) -> usize {
        match &self.index {
            ViewBacking::Shared { index, .. } => index.values,
            ViewBacking::Inline(field) => match field.data {
                InlineData::Text(_) => 1,
                InlineData::Packed { values, .. } => values,
                InlineData::MediaSuccess(_) => InlineMediaSuccess::COUNT,
            },
        }
    }
    fn active(&self) -> &[Option<Active>] {
        match self.shared_index().map(|index| &index.storage) {
            Some(IndexStorage::Many { active, .. }) => active,
            _ => &[],
        }
    }

    #[cfg(test)]
    fn shared_index_for_test(&self) -> &Arc<Index<'a>> {
        self.shared_index().expect("test uses a shared view")
    }

    fn check_schema(&self, identity: SchemaIdentity) -> Result<(), Error> {
        if self.schema.identity != identity {
            return Err(Error::Type);
        }
        Ok(())
    }

    fn parse(input: &'a [u8], schema: &'static Schema, limits: Limits) -> Result<Self, Error> {
        Self::parse_parts(Parts::one(input), schema, limits, 0)
    }
    fn parse_parts(
        parts: Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
    ) -> Result<Self, Error> {
        Self::parse_parts_required(parts, schema, limits, depth, true)
    }
    fn parse_parts_required(
        parts: Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        required: bool,
    ) -> Result<Self, Error> {
        Self::parse_parts_admitted(parts, schema, limits, depth, required, |_| Ok(()))
    }
    fn parse_parts_admitted(
        parts: Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        required: bool,
        mut admit: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let measurement = Self::measure_parts(&parts, schema, limits, depth)?;
        admit(measurement.charge)?;
        Self::index_parts(parts, schema, limits, depth, required, measurement)
    }
    #[inline]
    fn parse_with_single_packed(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Self, Error> {
        if let [tag, rest @ ..] = input
            && *tag & 0x87 == 2
            && let Ok(number) = protobuf::FieldNumber::new(u32::from(*tag >> 3))
            && let Some(view) = Self::try_parse_single_packed(input, rest, number, schema, limits)?
        {
            return Ok(view);
        }
        Self::parse(input, schema, limits)
    }
    #[inline(never)]
    fn try_parse_single_packed(
        input: &'a [u8],
        rest: &'a [u8],
        number: protobuf::FieldNumber,
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Option<Self>, Error> {
        let bytes = match rest {
            [length, bytes @ ..] if *length < 128 && usize::from(*length) == bytes.len() => bytes,
            [length, ..] if *length >= 128 && input.len() <= u32::MAX as usize => {
                let Ok((len, used)) = protobuf::decode_varint(rest) else {
                    return Ok(None);
                };
                let bytes = &rest[used..];
                if len != bytes.len() as u64 {
                    return Ok(None);
                }
                bytes
            }
            _ => return Ok(None),
        };
        if let Ok(Some(spec)) = schema.classify(number, Value::Bytes(bytes))
            && spec.repeated()
            && spec.kind().packable()
        {
            if limits.max_fragments == 0 || input.len() > limits.max_bytes {
                return Err(Error::Limit);
            }
            let remaining = limits
                .max_fields
                .checked_sub(1)
                .ok_or(protobuf::DecodeError::LimitExceeded)?;
            let values = packed_count(bytes, spec.kind(), limits.max_values)?;
            let charge = Self::index_charge(&Parts::one(input), schema, limits, 1)?;
            if limits == Limits::DEFAULT
                && schema.oneofs == 0
                && schema.required_prefix == 0
                && schema
                    .fields
                    .iter()
                    .filter(|spec| spec.number() == number.get())
                    .nth(1)
                    .is_none()
            {
                return Ok(Some(Self {
                    schema,
                    index: ViewBacking::Inline(InlineView {
                        input,
                        data: InlineData::Packed {
                            bytes,
                            number,
                            values,
                        },
                        charge,
                    }),
                }));
            }
            return Self::finish_index(
                IndexParts {
                    first: input,
                    tail: IndexTail::Wire(WireIndex::new(number, Value::Bytes(bytes), 0, input)),
                },
                schema,
                limits,
                0,
                true,
                IndexMeasurement {
                    keys: RetainedKeys::ByteTag(number),
                    omitted: 0,
                    count: 1,
                    remaining,
                    values,
                    charge,
                },
                IndexStorage::Many {
                    fields: Box::default(),
                    active: Box::default(),
                },
            )
            .map(Some);
        }
        Ok(None)
    }
    fn parse_media_retry(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Self, Error> {
        if limits != Limits::DEFAULT {
            return Self::parse_with_single_text(input, schema, limits);
        }
        if let Some((success, charge)) = parse_media_retry_success(input, limits) {
            return Ok(Self {
                schema,
                index: ViewBacking::Inline(InlineView {
                    input,
                    data: InlineData::MediaSuccess(success),
                    charge,
                }),
            });
        }
        Self::parse_media_retry_fallback(input, schema)
    }

    #[inline(never)]
    fn parse_media_retry_fallback(input: &'a [u8], schema: &'static Schema) -> Result<Self, Error> {
        let limits = Limits::DEFAULT;
        if input.len() >= 135
            && let Some((success, charge)) = parse_long_media_retry_success(input, limits)
        {
            return Ok(Self {
                schema,
                index: ViewBacking::Inline(InlineView {
                    input,
                    data: InlineData::MediaSuccess(success),
                    charge,
                }),
            });
        }
        if input.len() <= u8::MAX as usize
            && media_short::short_media_header_can_project(input)
            && let Some((success, count)) =
                media_short::parse_short_media_retry_fields(input, schema)
        {
            let charge = Self::index_charge(&Parts::one(input), schema, limits, count)?;
            let charge = if count == InlineMediaSuccess::COUNT {
                charge
            } else {
                charge | Index::FILTERED
            };
            return Ok(Self {
                schema,
                index: ViewBacking::Inline(InlineView {
                    input,
                    data: InlineData::MediaSuccess(success),
                    charge,
                }),
            });
        }
        Self::parse_with_single_text(input, schema, limits)
    }

    fn media_stanza_id(&self) -> Option<&'a str> {
        match &self.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::MediaSuccess(success),
                ..
            }) => {
                return Some(success.id);
            }
            ViewBacking::Inline(InlineView {
                data: InlineData::Text(text),
                ..
            }) => {
                return Some(*text);
            }
            ViewBacking::Shared { index, .. } => {
                if let IndexTail::Text(text) | IndexTail::CompactText(text) = index.parts.tail {
                    return Some(text);
                }
            }
            ViewBacking::Inline(_) => {}
        }
        self.media_text(MediaRetryNotification::STANZA_ID.spec)
    }

    fn media_many_fields(&self, spec: Spec) -> Option<&[Indexed<'a>]> {
        let ViewBacking::Shared { index, .. } = &self.index else {
            return None;
        };
        let IndexStorage::Many { fields, .. } = &index.storage else {
            return None;
        };
        if fields.is_empty() {
            return None;
        }
        let end = fields.partition_point(|field| field.number().get() <= spec.number());
        Some(&fields[..end])
    }

    #[inline(never)]
    fn media_text(&self, spec: Spec) -> Option<&'a str> {
        let field = if let Some(prefix) = self.media_many_fields(spec) {
            *prefix
                .last()
                .filter(|field| field.number().get() == spec.number())?
        } else {
            self.range(spec).last()?
        };
        let Value::Bytes(bytes) = field.value() else {
            unreachable!("media string wire type was validated");
        };
        Some(core::str::from_utf8(bytes).expect("media string UTF-8 was validated"))
    }

    #[inline(never)]
    fn media_result_number(&self) -> i32 {
        let spec = MediaRetryNotification::RESULT.spec;
        let read = |field: Indexed<'a>| match field.value() {
            Value::Varint(number) => {
                media_retry_notification::ResultType::from_number(number as i32)
            }
            _ => None,
        };
        let value = if let Some(prefix) = self.media_many_fields(spec) {
            prefix
                .iter()
                .rev()
                .take_while(|field| field.number().get() == spec.number())
                .copied()
                .find_map(read)
        } else {
            self.range(spec).filter_map(read).last()
        };
        value.map_or(-1, media_retry_notification::ResultType::number)
    }

    fn inline_media_success(&self) -> Option<&InlineMediaSuccess<'a>> {
        if let ViewBacking::Inline(InlineView {
            data: InlineData::MediaSuccess(success),
            ..
        }) = &self.index
        {
            Some(success)
        } else {
            None
        }
    }

    #[inline(never)]
    fn parse_with_single_text(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Self, Error> {
        let parts = Parts::one(input);
        if let [tag, length, bytes @ ..] = input
            && (*length >= 128 || usize::from(*length) == bytes.len())
            && let Some(spec) = schema.fields.first()
            && *tag == (spec.number() as u8) * 8 + 2
        {
            if *length < 128 && schema.required_prefix == 0 && limits == Limits::DEFAULT {
                let text = core::str::from_utf8(bytes).map_err(|_| Error::Utf8)?;
                let charge = Self::index_charge(&parts, schema, limits, 1)?;
                return Ok(Self {
                    schema,
                    index: ViewBacking::Inline(InlineView {
                        input,
                        data: InlineData::Text(text),
                        charge,
                    }),
                });
            }
            if let Some((text, measurement)) = Self::measure_single_text(&parts, schema, limits, 0)?
            {
                return Self::finish_text(input, text, schema, limits, measurement);
            }
        }
        if (512..=u16::MAX as usize).contains(&input.len())
            && let Some(first) = schema.fields.first()
            && input[0] == (first.number() as u8) * 8 + 2
            && let Some(view) = Self::parse_periodic_ascii(&parts, schema, limits)?
        {
            return Ok(view);
        }
        if (Self::FILTER_MIN_BYTES..=128).contains(&input.len())
            && let [tag, length, rest @ ..] = input
            && let Some(spec) = schema.fields.first()
            && u32::from(*tag) == spec.number() * 8 + 2
            && let Some(bytes) = rest.get(..usize::from(*length))
        {
            let tail = &rest[bytes.len()..];
            if input.len() < 128
                && let Some(last) = schema.fields.last()
                && let Some(width) = protobuf::short_unknown_delimited_len(tail, last.number())
                && let Some(rest) = tail.get(width..)
                && let Some(omitted) = if rest.is_empty() {
                    Some(1)
                } else {
                    (protobuf::short_unknown_delimited_len(rest, last.number()) == Some(rest.len()))
                        .then_some(2)
                }
                && let Some(view) =
                    Self::parse_text_with_short_unknowns(&parts, bytes, schema, limits, omitted)?
            {
                return Ok(view);
            }
            return Self::parse_short_text_prefix(parts, bytes, schema, limits);
        }
        if (Self::FILTER_MIN_BYTES..128).contains(&input.len())
            && input[0] & 0x87 == 0x82
            && let Some(last) = schema.fields.last()
            && let Some(width) = protobuf::short_unknown_delimited_len(input, last.number())
            && let Some([tag, length, bytes @ ..]) = input.get(width..)
            && *tag == (schema.fields[0].number() as u8) * 8 + 2
            && usize::from(*length) == bytes.len()
            && let Some(view) =
                Self::parse_text_with_short_unknowns(&parts, bytes, schema, limits, 1)?
        {
            return Ok(view);
        }
        if input.len() < Self::FILTER_MIN_BYTES
            && let [tag, length, rest @ ..] = input
            && let Some(spec) = schema.fields.first()
            && *tag == (spec.number() as u8) * 8 + 2
            && let Some(bytes) = rest.get(..usize::from(*length))
            && let Some(view) = Self::parse_text_and_byte_field(&parts, bytes, schema, limits)?
        {
            return Ok(view);
        }
        Self::parse_parts(parts, schema, limits, 0)
    }

    #[inline(never)]
    fn parse_periodic_ascii(
        parts: &Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Option<Self>, Error> {
        let input = parts.first;
        if limits.max_fragments == 0 || input.len() > limits.max_bytes {
            return Ok(None);
        }
        let first = schema.fields[0];
        let expected_tag = (first.number() as u8) * 8 + 2;
        let cutoff = schema.fields.last().map_or(0, |spec| spec.number());
        let length = usize::from(input[1]);
        if length > 16 {
            return Ok(None);
        }
        let width = 2 + length;
        let mut known = 1;
        while known < 4 && input.get(known * width..known * width + 2) == Some(&input[..2]) {
            known += 1;
        }
        if known == 3 {
            return Ok(None);
        }
        let known_width = known * width;
        let Some(unknown_width) =
            protobuf::short_unknown_delimited_len(&input[known_width..], cutoff)
        else {
            return Ok(None);
        };
        let unknown_header = ((unknown_width - 4) as u32) << 24 | 0x0000_8082;
        let cutoff_word = (cutoff & 0xf) << 3 | (cutoff & 0x7f0) << 4 | (cutoff & 0x3f800) << 5;
        let stride = known_width + unknown_width;
        let records = input.len() / stride;
        let retained = records * known;
        let fields = records + retained;
        let Some(remaining) = limits.max_fields.checked_sub(fields) else {
            return Ok(None);
        };
        if retained < 32 || retained > limits.max_values || !input.len().is_multiple_of(stride) {
            return Ok(None);
        }
        let number = protobuf::FieldNumber::new(first.number()).expect("generated schema field");
        for (i, record) in input.chunks_exact(stride).enumerate() {
            let known_fields = &record[..known_width];
            let valid = if length == 4 && known == 4 {
                let header = u64::from(expected_tag) | 4 << 8;
                let ([left, middle, right], _) = known_fields.as_chunks::<8>() else {
                    unreachable!("four six-byte fields");
                };
                let left =
                    (u64::from_le_bytes(*left) ^ (header | header << 48)) & 0xffff_8080_8080_ffff;
                let middle = (u64::from_le_bytes(*middle) ^ (header << 32)) & 0x8080_ffff_8080_8080;
                let right = (u64::from_le_bytes(*right) ^ (header << 16)) & 0x8080_8080_ffff_8080;
                left | middle | right == 0
            } else if length == 4 {
                let header = u64::from(expected_tag) | 4 << 8;
                let differences =
                    record
                        .windows(8)
                        .step_by(6)
                        .take(known)
                        .fold(0, |differences, window| {
                            let word = u64::from_le_bytes(
                                *window.first_chunk::<8>().expect("eight-byte window"),
                            );
                            differences | (word ^ header)
                        });
                differences & 0x0000_8080_8080_ffff == 0
            } else {
                known_fields.chunks_exact(width).all(|field| {
                    field[0] == expected_tag
                        && usize::from(field[1]) == length
                        && field[2..].is_ascii()
                })
            };
            let unknown = u32::from_le_bytes(
                *record[known_width..]
                    .first_chunk::<4>()
                    .expect("proved unknown header width"),
            );
            if !valid
                || unknown & 0xff80_8087 != unknown_header
                || unknown & 0x007f_7f78 <= cutoff_word
            {
                if i == 0 {
                    return Ok(None);
                }
                return Self::resume_periodic_ascii(
                    parts,
                    schema,
                    limits,
                    number,
                    i * stride,
                    i * known,
                    i,
                );
            }
        }
        let last = input.len() - stride + (known - 1) * width + 2;
        let charge = Self::index_charge(parts, schema, limits, fields)?;
        Self::finish_index(
            IndexParts {
                first: input,
                tail: IndexTail::Periodic(PeriodicTail {
                    tag: number,
                    last_span: (last as u32) << 16 | length as u32,
                    count: retained as u16,
                    stride: stride as u16,
                    lanes_shift: known.trailing_zeros() as u8,
                }),
            },
            schema,
            limits,
            0,
            true,
            IndexMeasurement {
                keys: RetainedKeys::ByteTag(number),
                omitted: records,
                count: fields,
                remaining,
                values: retained,
                charge,
            },
            IndexStorage::Many {
                fields: Box::default(),
                active: Box::default(),
            },
        )
        .map(Some)
    }

    #[cold]
    #[inline(never)]
    fn resume_periodic_ascii(
        parts: &Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        number: protobuf::FieldNumber,
        start: usize,
        known: usize,
        omitted: usize,
    ) -> Result<Option<Self>, Error> {
        let measurement = Self::measure_parts_seeded::<true>(
            parts,
            schema,
            limits,
            0,
            Some((start, known, omitted, number.get())),
        )?;
        Self::index_parts(
            Parts::one(parts.first),
            schema,
            limits,
            0,
            true,
            measurement,
        )
        .map(Some)
    }

    #[cold]
    #[inline(never)]
    fn parse_text_and_byte_field(
        parts: &Parts<'a>,
        bytes: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Option<Self>, Error> {
        if schema.oneofs != 0
            || limits.max_fragments == 0
            || parts.first.len() > limits.max_bytes
            || limits.max_values < 2
        {
            return Ok(None);
        }
        let first = schema.fields[0];
        let cutoff = if parts.first.len() < Self::FILTER_MIN_BYTES {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut tail = &parts.first[2 + bytes.len()..];
        let mut omitted = 0;
        let Some((number, rest)) =
            protobuf::next_pair_bytes::<true>(&mut tail, cutoff, &mut omitted, false)
                .filter(|_| tail.is_empty())
        else {
            return Ok(None);
        };
        let start = schema
            .fields
            .partition_point(|spec| spec.number() < number.get());
        let Some(spec) = schema
            .fields
            .get(start)
            .filter(|spec| spec.number() == number.get())
        else {
            return Ok(None);
        };
        let duplicate_text = number.get() == first.number();
        if spec.repeated()
            || (!duplicate_text && !matches!(spec.kind(), Kind::Bytes | Kind::Message))
        {
            return Ok(None);
        }
        let count = omitted + 2;
        let Some(remaining) = limits.max_fields.checked_sub(count) else {
            return Ok(None);
        };
        let text = checked_text(bytes)?;
        let text = if duplicate_text {
            checked_text(rest)?
        } else {
            text
        };
        let charge = Self::index_charge(parts, schema, limits, count)?;
        let first = ShortByteField {
            number: protobuf::FieldNumber::new(first.number()).expect("generated schema field"),
            offset: 2,
            length: bytes.len() as u16,
        };
        let second = ShortByteField {
            number,
            offset: (parts.first.len() - rest.len()) as u16,
            length: rest.len() as u16,
        };
        debug_assert!(first.number.get() <= second.number.get());
        Self::finish_index(
            IndexParts {
                first: parts.first,
                tail: IndexTail::CompactText(text),
            },
            schema,
            limits,
            0,
            true,
            IndexMeasurement {
                keys: RetainedKeys::MixedBytes,
                omitted,
                count,
                remaining,
                values: 2,
                charge,
            },
            IndexStorage::Compact(CompactIndex::BytePair([first, second])),
        )
        .map(Some)
    }

    #[cold]
    #[inline(never)]
    fn parse_text_with_short_unknowns(
        parts: &Parts<'a>,
        bytes: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
        omitted: usize,
    ) -> Result<Option<Self>, Error> {
        let count = 1 + omitted;
        let Some(remaining) = limits.max_fields.checked_sub(count) else {
            return Ok(None);
        };
        if limits.max_fragments == 0
            || parts.first.len() > limits.max_bytes
            || limits.max_values == 0
        {
            return Ok(None);
        }
        let text = checked_text(bytes)?;
        let charge = Self::index_charge(parts, schema, limits, count)?;
        Self::finish_filtered_text(
            parts.first,
            text,
            schema,
            limits,
            IndexMeasurement {
                keys: RetainedKeys::MixedBytes,
                omitted,
                count,
                remaining,
                values: 1,
                charge,
            },
        )
        .map(Some)
    }

    #[cold]
    #[inline(never)]
    fn parse_short_text_prefix(
        parts: Parts<'a>,
        bytes: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
    ) -> Result<Self, Error> {
        if parts.first.len() < 128
            && let Some(view) = Self::parse_text_and_byte_field(&parts, bytes, schema, limits)?
        {
            return Ok(view);
        }
        let measurement = Self::measure_parts(&parts, schema, limits, 0)?;
        if measurement.omitted != 0
            && measurement.count - measurement.omitted == 1
            && let Ok(text) = core::str::from_utf8(bytes)
        {
            return Self::finish_filtered_text(parts.first, text, schema, limits, measurement);
        }
        Self::index_parts(parts, schema, limits, 0, true, measurement)
    }
    fn finish_text(
        input: &'a [u8],
        text: &'a str,
        schema: &'static Schema,
        limits: Limits,
        measurement: IndexMeasurement,
    ) -> Result<Self, Error> {
        if limits == Limits::DEFAULT && schema.required_prefix == 0 && measurement.count == 1 {
            return Ok(Self {
                schema,
                index: ViewBacking::Inline(InlineView {
                    input,
                    data: InlineData::Text(text),
                    charge: measurement.charge,
                }),
            });
        }
        Self::finish_index(
            IndexParts {
                first: input,
                tail: IndexTail::Text(text),
            },
            schema,
            limits,
            0,
            true,
            measurement,
            IndexStorage::Many {
                fields: Box::default(),
                active: Box::default(),
            },
        )
    }
    #[inline(never)]
    fn finish_filtered_text(
        input: &'a [u8],
        text: &'a str,
        schema: &'static Schema,
        limits: Limits,
        measurement: IndexMeasurement,
    ) -> Result<Self, Error> {
        if limits == Limits::DEFAULT
            && schema.required_prefix == 0
            && schema.oneofs == 0
            && measurement.count - measurement.omitted == 1
            && limits.max_fields - measurement.remaining == measurement.count
        {
            return Ok(Self {
                schema,
                index: ViewBacking::Inline(InlineView {
                    input,
                    data: InlineData::Text(text),
                    charge: measurement.charge | Index::FILTERED,
                }),
            });
        }
        Self::finish_text(input, text, schema, limits, measurement)
    }

    #[inline(never)]
    fn measure_single_text(
        parts: &Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
    ) -> Result<Option<(&'a str, IndexMeasurement)>, Error> {
        if depth > limits.max_depth
            || limits.max_fragments == 0
            || parts.first.len() > limits.max_bytes
        {
            return Err(Error::Limit);
        }
        let (text, remaining) = if let [tag, length, bytes @ ..] = parts.first
            && *tag < 128
            && usize::from(*length) == bytes.len()
        {
            let remaining = limits
                .max_fields
                .checked_sub(1)
                .ok_or(protobuf::DecodeError::LimitExceeded)?;
            (
                core::str::from_utf8(bytes).map_err(|_| Error::Utf8)?,
                remaining,
            )
        } else {
            let mut reader = Reader::new(
                parts.first,
                protobuf::Limits {
                    max_message_bytes: limits.max_bytes,
                    max_fields: limits.max_fields,
                    max_group_depth: protobuf::MAX_GROUP_DEPTH,
                },
            )?;
            let (number, Value::Bytes(bytes)) = reader.next_value().ok_or(Error::Type)?? else {
                return Ok(None);
            };
            if !reader.remaining().is_empty() || schema.has_repeated(number) {
                return Ok(None);
            }
            (checked_text(bytes)?, reader.fields_left())
        };
        if limits.max_values == 0 {
            return Err(Error::Limit);
        }
        let charge = Self::index_charge(parts, schema, limits, 1)?;
        Ok(Some((
            text,
            IndexMeasurement {
                keys: RetainedKeys::MixedBytes,
                omitted: 0,
                count: 1,
                remaining,
                values: 1,
                charge,
            },
        )))
    }
    fn measure_parts(
        parts: &Parts<'_>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
    ) -> Result<IndexMeasurement, Error> {
        Self::measure_parts_with_utf8::<true>(parts, schema, limits, depth)
    }
    fn measure_parts_with_utf8<const CHECK_UTF8: bool>(
        parts: &Parts<'_>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
    ) -> Result<IndexMeasurement, Error> {
        Self::measure_parts_seeded::<CHECK_UTF8>(parts, schema, limits, depth, None)
    }

    #[inline(never)]
    fn measure_parts_seeded<const CHECK_UTF8: bool>(
        parts: &Parts<'_>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        prefix: Option<(usize, usize, usize, u32)>,
    ) -> Result<IndexMeasurement, Error> {
        if depth > limits.max_depth
            || parts
                .rest
                .len()
                .checked_add(1)
                .is_none_or(|n| n > limits.max_fragments)
            || parts.len()? > limits.max_bytes
        {
            return Err(Error::Limit);
        }
        let (mut skip, mut count, mut omitted, mut keys_or) = prefix.unwrap_or_default();
        let mut keys_and = if count == 0 { u32::MAX } else { keys_or };
        let mut remaining = limits.max_fields - count - omitted;
        let mut values_left = limits.max_values - count;
        for bytes in parts.iter() {
            let last_tag = if bytes.len() >= Self::FILTER_MIN_BYTES {
                schema.fields.last().map_or(0, |spec| spec.number())
            } else {
                u32::MAX
            };
            let bytes = &bytes[core::mem::take(&mut skip)..];
            let mut reader = Reader::new(
                bytes,
                protobuf::Limits {
                    max_message_bytes: limits.max_bytes,
                    max_fields: remaining,
                    max_group_depth: protobuf::MAX_GROUP_DEPTH,
                },
            )?;
            if CHECK_UTF8
                && parts.rest.is_empty()
                && bytes.len() >= 128
                && let Some(spec) = schema.fields.first()
                && spec.kind() == Kind::String
                && spec.number() < 16
                && let [tag, length, rest @ ..] = bytes
                && *tag == (spec.number() as u8) << 3 | 2
                && *length <= 16
                && (*length == 0 || rest.first().is_some_and(|byte| byte.is_ascii()))
            {
                let tag = (spec.number() as u8) << 3 | 2;
                let (known, unknown) = reader.count_ascii_prefix(tag, last_tag, values_left);
                count += known;
                omitted += unknown;
                values_left -= known;
                if known != 0 {
                    keys_or |= spec.number();
                    keys_and &= spec.number();
                }
            }
            while let Some(field) = reader.next_counted_value(last_tag, &mut omitted) {
                let (number, value) = field?;
                count += 1;
                let key = number.get() | (u32::from(!matches!(value, Value::Bytes(_))) << 31);
                keys_or |= key;
                keys_and &= key;
                if let Some(spec) = schema.classify(number, value)? {
                    if CHECK_UTF8
                        && spec.kind() == Kind::String
                        && let Value::Bytes(bytes) = value
                    {
                        validate_preflight_text(bytes)?;
                    }
                    let n = if spec.repeated() && spec.kind().packable() {
                        if let Value::Bytes(bytes) = value {
                            packed_count(bytes, spec.kind(), values_left)?
                        } else {
                            1
                        }
                    } else {
                        1
                    };
                    values_left = values_left.checked_sub(n).ok_or(Error::Limit)?;
                }
            }
            remaining = reader.fields_left();
        }
        count += omitted;
        let charge = Self::index_charge(parts, schema, limits, count)?;
        Ok(IndexMeasurement {
            keys: if keys_or >= (1 << 31) {
                RetainedKeys::Other
            } else if keys_or == keys_and
                && let Ok(number) = protobuf::FieldNumber::new(keys_or)
            {
                RetainedKeys::ByteTag(number)
            } else {
                RetainedKeys::MixedBytes
            },
            omitted,
            count,
            remaining,
            values: limits.max_values - values_left,
            charge,
        })
    }
    #[inline(always)]
    fn index_charge(
        parts: &Parts<'_>,
        schema: &'static Schema,
        limits: Limits,
        count: usize,
    ) -> Result<usize, Error> {
        // Keep admission based on the complete wire field and Vec-backed index header.
        let charge = count
            .checked_mul(core::mem::size_of::<(WireField<'_>, usize)>())
            .and_then(|n| {
                n.checked_add(
                    schema
                        .oneofs
                        .checked_mul(core::mem::size_of::<Option<Active>>())?,
                )
            })
            .and_then(|n| {
                n.checked_add(
                    parts
                        .rest
                        .capacity()
                        .checked_mul(core::mem::size_of::<&[u8]>())?,
                )
            })
            .and_then(|n| n.checked_add(Index::CHARGE_HEADER))
            .ok_or(Error::Limit)?;
        if charge > limits.max_index_bytes {
            return Err(Error::Limit);
        }
        Ok(charge)
    }
    #[inline(never)]
    fn index_parts(
        parts: Parts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        required: bool,
        measurement: IndexMeasurement,
    ) -> Result<Self, Error> {
        let IndexMeasurement {
            keys,
            omitted,
            count,
            remaining,
            values,
            charge,
        } = measurement;
        let retained = count - omitted;
        let compact = omitted != 0
            && retained >= 128
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= u32::MAX as usize
            && count <= WireIndex::ORDER_MASK as usize;
        if retained == 2
            && !matches!(keys, RetainedKeys::Other)
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= 128
            && let Some(view) = Self::try_finish_byte_pair(
                parts.first,
                schema,
                limits,
                depth,
                required,
                measurement,
            )?
        {
            return Ok(view);
        }
        if retained == 3
            && !matches!(keys, RetainedKeys::Other)
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= 128
            && let Some(fields) = Self::index_short_byte_triple(parts.first, schema, omitted)
        {
            return Self::finish_index(
                IndexParts {
                    first: parts.first,
                    tail: IndexTail::Fragments(Vec::new()),
                },
                schema,
                limits,
                depth,
                required,
                measurement,
                IndexStorage::Compact(CompactIndex::ByteTriple(fields)),
            );
        }
        if retained == 4
            && !matches!(keys, RetainedKeys::Other)
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= 128
            && let Some(fields) = Self::index_short_byte_quad(parts.first, schema, omitted)
        {
            return Self::finish_index(
                IndexParts {
                    first: parts.first,
                    tail: IndexTail::Fragments(Vec::new()),
                },
                schema,
                limits,
                depth,
                required,
                measurement,
                IndexStorage::Compact(CompactIndex::ByteQuad(fields)),
            );
        }
        if (5..=8).contains(&retained)
            && let RetainedKeys::ByteTag(number) = keys
            && schema
                .fields
                .first()
                .is_some_and(|spec| spec.number() == number.get())
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= u8::MAX as usize
            && let Some(fields) = Self::index_short_byte_run(parts.first, schema, omitted, retained)
        {
            return Self::finish_index(
                IndexParts {
                    first: parts.first,
                    tail: IndexTail::ShortByteRun(fields),
                },
                schema,
                limits,
                depth,
                required,
                measurement,
                IndexStorage::Many {
                    fields: Box::default(),
                    active: Box::default(),
                },
            );
        }
        if (5..=8).contains(&retained)
            && matches!(keys, RetainedKeys::MixedBytes)
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= u8::MAX as usize
            && let Some((offsets, tags)) =
                Self::index_short_tagged_run(parts.first, schema, omitted, retained)
        {
            return Self::finish_index(
                IndexParts {
                    first: parts.first,
                    tail: IndexTail::ShortTaggedRun(offsets, tags),
                },
                schema,
                limits,
                depth,
                required,
                measurement,
                IndexStorage::Many {
                    fields: Box::default(),
                    active: Box::default(),
                },
            );
        }
        let mut inline = None;
        let storage = if count == 1
            && omitted == 0
            && schema.oneofs == 0
            && parts.rest.is_empty()
            && parts.first.len() <= 128
        {
            let (number, value) = if let RetainedKeys::ByteTag(number) = keys
                && let [tag, length, bytes @ ..] = parts.first
                && *tag < 128
                && usize::from(*length) == bytes.len()
            {
                (number, Value::Bytes(bytes))
            } else {
                let mut reader = Reader::new(
                    parts.first,
                    protobuf::Limits {
                        max_message_bytes: limits.max_bytes,
                        max_fields: limits.max_fields,
                        max_group_depth: protobuf::MAX_GROUP_DEPTH,
                    },
                )?;
                reader
                    .next_value()
                    .ok_or(protobuf::DecodeError::Truncated)??
            };
            inline = Some(WireIndex::new(number, value, 0, parts.first));
            IndexStorage::Many {
                fields: Box::default(),
                active: Box::default(),
            }
        } else if compact {
            if let RetainedKeys::ByteTag(number) = keys
                && parts.first.len() <= u16::MAX as usize
                && !schema.fields[schema
                    .fields
                    .partition_point(|spec| spec.number() < number.get())..]
                    .iter()
                    .take_while(|spec| spec.number() == number.get())
                    .any(|spec| spec.repeated())
            {
                Self::index_compact_bytes(parts.first, schema, limits, retained, number)?
            } else {
                let storage = Self::index_compact(parts.first, schema, limits, retained)?;
                if let IndexStorage::Compact(CompactIndex::SameBytes(index)) = &storage
                    && let Some(text) = Self::compact_text(index, parts.first, schema)
                {
                    return Self::finish_index(
                        IndexParts {
                            first: parts.first,
                            tail: IndexTail::CompactText(text),
                        },
                        schema,
                        limits,
                        depth,
                        required,
                        measurement,
                        storage,
                    );
                }
                storage
            }
        } else {
            let mut fields = Vec::with_capacity(retained);
            let mut active = vec![None; schema.oneofs];
            for bytes in parts.iter() {
                let cutoff = if omitted != 0 && bytes.len() >= Self::FILTER_MIN_BYTES {
                    schema.fields.last().map_or(0, |spec| spec.number())
                } else {
                    u32::MAX
                };
                if cutoff == u32::MAX {
                    Self::collect_indexed_fields::<false>(
                        bytes,
                        cutoff,
                        schema,
                        limits,
                        &mut fields,
                        &mut active,
                    )?;
                } else {
                    Self::collect_indexed_fields::<true>(
                        bytes,
                        cutoff,
                        schema,
                        limits,
                        &mut fields,
                        &mut active,
                    )?;
                }
            }
            fields.sort_unstable_by_key(|v| {
                ((v.number().get() as u128) << usize::BITS) | v.order as u128
            });
            IndexStorage::Many {
                fields: fields.into_boxed_slice(),
                active: active.into_boxed_slice(),
            }
        };
        let view = Self {
            index: ViewBacking::Shared {
                index: Arc::new(Index {
                    storage,
                    parts: IndexParts {
                        first: parts.first,
                        tail: match inline {
                            Some(field) => IndexTail::Wire(field),
                            None => IndexTail::Fragments(parts.rest),
                        },
                    },
                    work_fields: (limits.max_fields - remaining)
                        | if omitted != 0 { Index::FILTERED } else { 0 },
                    values,
                    charge,
                }),
                limits,
                depth,
            },
            schema,
        };
        if required {
            for spec in &schema.fields[..schema.required_prefix as usize] {
                if spec.required() && !view.has(*spec) {
                    return Err(Error::Required(spec.number()));
                }
            }
        }
        Ok(view)
    }
    #[inline(never)]
    fn compact_text(index: &SameBytesIndex, input: &'a [u8], schema: &Schema) -> Option<&'a str> {
        let spec = *schema.fields.first()?;
        if spec.kind() != Kind::String {
            return None;
        }
        let number = protobuf::FieldNumber::new(spec.number()).ok()?;
        let position = index.range(number.get()).next_back()?;
        core::str::from_utf8(index.bytes(number.get(), position, input)).ok()
    }
    fn collect_indexed_fields<const FILTERED: bool>(
        bytes: &'a [u8],
        cutoff: u32,
        schema: &'static Schema,
        limits: Limits,
        fields: &mut Vec<Indexed<'a>>,
        active: &mut [Option<Active>],
    ) -> Result<(), Error> {
        let mut reader = Reader::new(
            bytes,
            protobuf::Limits {
                max_message_bytes: limits.max_bytes,
                max_fields: limits.max_fields,
                max_group_depth: protobuf::MAX_GROUP_DEPTH,
            },
        )?;
        while let Some(field) = if FILTERED {
            reader.next_batched_indexed_value(cutoff)
        } else {
            reader.next_value()
        } {
            let (number, value) = field?;
            let order = fields.len();
            if schema.oneofs != 0
                && let Some(spec) = schema.classify(number, value)?
                && let Some(group) = spec.oneof()
            {
                let slot = active.get_mut(group).ok_or(Error::Type)?;
                if slot.is_none_or(|prior: Active| prior.number != spec.number()) {
                    *slot = Some(Active {
                        number: spec.number(),
                        since: order,
                    });
                }
            }
            fields.push(Indexed::new(number, value, order));
        }
        Ok(())
    }
    #[inline(always)]
    fn finish_index(
        parts: IndexParts<'a>,
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        required: bool,
        measurement: IndexMeasurement,
        storage: IndexStorage<'a>,
    ) -> Result<Self, Error> {
        let IndexMeasurement {
            omitted,
            remaining,
            values,
            charge,
            ..
        } = measurement;
        let view = Self {
            index: ViewBacking::Shared {
                index: Arc::new(Index {
                    storage,
                    parts,
                    work_fields: (limits.max_fields - remaining)
                        | if omitted != 0 { Index::FILTERED } else { 0 },
                    values,
                    charge,
                }),
                limits,
                depth,
            },
            schema,
        };
        if required {
            for spec in &schema.fields[..schema.required_prefix as usize] {
                if spec.required() && !view.has(*spec) {
                    return Err(Error::Required(spec.number()));
                }
            }
        }
        Ok(view)
    }
    #[cold]
    #[inline(never)]
    fn try_finish_byte_pair(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
        depth: usize,
        required: bool,
        measurement: IndexMeasurement,
    ) -> Result<Option<Self>, Error> {
        let Some(pair) = Self::index_short_byte_pair(input, schema, measurement.omitted) else {
            return Ok(None);
        };
        Self::finish_index(
            IndexParts {
                first: input,
                tail: IndexTail::BytePair(pair),
            },
            schema,
            limits,
            depth,
            required,
            measurement,
            IndexStorage::Many {
                fields: Box::default(),
                active: Box::default(),
            },
        )
        .map(Some)
    }
    fn index_short_byte_pair(
        input: &'a [u8],
        schema: &Schema,
        omitted: usize,
    ) -> Option<[ShortByteField; 2]> {
        let cutoff = if omitted == 0 {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut remaining = input;
        let mut next = || -> Option<ShortByteField> {
            let (number, bytes) =
                protobuf::next_pair_bytes::<false>(&mut remaining, cutoff, &mut 0, false)?;
            Some(ShortByteField {
                number,
                offset: (bytes.as_ptr().addr() - input.as_ptr().addr()) as u16,
                length: bytes.len() as u16,
            })
        };
        let first = next()?;
        let second = next()?;
        Some(if first.number.get() <= second.number.get() {
            [first, second]
        } else {
            [second, first]
        })
    }
    #[cold]
    #[inline(never)]
    fn index_short_byte_triple(
        input: &'a [u8],
        schema: &Schema,
        omitted: usize,
    ) -> Option<[PackedByteField; 3]> {
        let cutoff = if omitted == 0 {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut remaining = input;
        let mut next = || {
            let (number, bytes) =
                protobuf::next_pair_bytes::<false>(&mut remaining, cutoff, &mut 0, false)?;
            let number = u32::from(u16::try_from(number.get()).ok()?);
            let offset = (bytes.as_ptr().addr() - input.as_ptr().addr()) as u32;
            Some(PackedByteField(
                number << 16 | offset << 8 | bytes.len() as u32,
            ))
        };
        let mut fields = [next()?, next()?, next()?];
        if fields[0].number() > fields[1].number() {
            fields.swap(0, 1);
        }
        if fields[1].number() > fields[2].number() {
            fields.swap(1, 2);
        }
        if fields[0].number() > fields[1].number() {
            fields.swap(0, 1);
        }
        Some(fields)
    }

    #[cold]
    #[inline(never)]
    fn index_short_byte_quad(
        input: &'a [u8],
        schema: &Schema,
        omitted: usize,
    ) -> Option<[PackedByteField; 4]> {
        let cutoff = if omitted == 0 {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut remaining = input;
        let mut next = || {
            let (number, bytes) =
                protobuf::next_pair_bytes::<false>(&mut remaining, cutoff, &mut 0, false)?;
            let number = u32::from(u16::try_from(number.get()).ok()?);
            let offset = (bytes.as_ptr().addr() - input.as_ptr().addr()) as u32;
            Some(PackedByteField(
                number << 16 | offset << 8 | bytes.len() as u32,
            ))
        };
        let mut fields = [next()?, next()?, next()?, next()?];
        if fields[0].number() > fields[1].number() {
            fields.swap(0, 1);
        }
        if fields[1].number() > fields[2].number() {
            fields.swap(1, 2);
        }
        if fields[0].number() > fields[1].number() {
            fields.swap(0, 1);
        }
        if fields[2].number() > fields[3].number() {
            fields.swap(2, 3);
        }
        if fields[1].number() > fields[2].number() {
            fields.swap(1, 2);
        }
        if fields[0].number() > fields[1].number() {
            fields.swap(0, 1);
        }
        Some(fields)
    }

    #[cold]
    #[inline(never)]
    fn index_short_byte_run(
        input: &'a [u8],
        schema: &Schema,
        omitted: usize,
        retained: usize,
    ) -> Option<[u16; 8]> {
        let cutoff = if omitted == 0 {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut remaining = input;
        let mut fields = [0; 8];
        for field in &mut fields[..retained] {
            let (_, bytes) =
                protobuf::next_pair_bytes::<false>(&mut remaining, cutoff, &mut 0, false)?;
            let offset = (bytes.as_ptr().addr() - input.as_ptr().addr()) as u16;
            *field = offset << 8 | bytes.len() as u16;
        }
        Some(fields)
    }

    fn short_byte_run_range<'v>(
        &'v self,
        fields: &'v [u16; 8],
        number: u32,
    ) -> GeneralIndexIter<'v, 'a> {
        let position = if self.schema.fields[0].number() == number {
            0
        } else {
            fields.len()
        };
        GeneralIndexIter::ShortByteRun(ShortByteRunIter {
            fields,
            view: self,
            position,
        })
    }

    #[cold]
    #[inline(never)]
    fn index_short_tagged_run(
        input: &'a [u8],
        schema: &Schema,
        omitted: usize,
        retained: usize,
    ) -> Option<([u8; 8], u16)> {
        let cutoff = if omitted == 0 {
            u32::MAX
        } else {
            schema.fields.last().map_or(0, |spec| spec.number())
        };
        let mut remaining = input;
        let mut offsets = [0; 8];
        let mut tags = 0;
        for slot in &mut offsets[..retained] {
            let (number, bytes) =
                protobuf::next_pair_bytes::<false>(&mut remaining, cutoff, &mut 0, true)?;
            *slot = (bytes.as_ptr().addr() - input.as_ptr().addr()) as u8;
            tags |= 1 << number.get();
        }
        Some((offsets, tags))
    }

    fn short_tagged_run_range<'v>(
        &'v self,
        offsets: &'v [u8; 8],
        number: u32,
    ) -> GeneralIndexIter<'v, 'a> {
        let tag = u8::try_from(number).unwrap_or(u8::MAX);
        GeneralIndexIter::ShortTaggedRun(ShortTaggedRunIter {
            offsets,
            input: self.wire_fragments().0,
            position: 0,
            tag,
        })
    }

    #[inline(never)]
    fn index_compact(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
        retained: usize,
    ) -> Result<IndexStorage<'a>, Error> {
        let mut fields = Vec::with_capacity(retained);
        let cutoff = schema.fields.last().map_or(0, |spec| spec.number());
        let mut reader = Reader::new(
            input,
            protobuf::Limits {
                max_message_bytes: limits.max_bytes,
                max_fields: limits.max_fields,
                max_group_depth: protobuf::MAX_GROUP_DEPTH,
            },
        )?;
        while let Some(field) = reader.next_indexed_value(cutoff) {
            let (number, value) = field?;
            fields.push(WireIndex::new(number, value, fields.len(), input));
        }
        if fields.is_sorted_by_key(|field| field.number.get()) {
            if input.len() <= u16::MAX as usize
                && let Some(index) = Self::stride_index(&fields, schema, input)
                    .or_else(|| Self::short_bytes(&fields, schema, input))
            {
                return Ok(IndexStorage::Compact(CompactIndex::SameBytes(index)));
            }
        } else {
            if input.len() <= u16::MAX as usize
                && let Some(short) = Self::short_bytes(&fields, schema, input)
            {
                return Ok(IndexStorage::Compact(CompactIndex::SameBytes(short)));
            }
            sort_wire_index(&mut fields);
        }
        Ok(IndexStorage::Compact(CompactIndex::General(
            fields.into_boxed_slice(),
        )))
    }
    #[inline(never)]
    fn index_compact_bytes(
        input: &'a [u8],
        schema: &'static Schema,
        limits: Limits,
        retained: usize,
        number: protobuf::FieldNumber,
    ) -> Result<IndexStorage<'a>, Error> {
        let cutoff = schema.fields.last().map_or(0, |spec| spec.number());
        let mut reader = Reader::new(
            input,
            protobuf::Limits {
                max_message_bytes: limits.max_bytes,
                max_fields: limits.max_fields,
                max_group_depth: protobuf::MAX_GROUP_DEPTH,
            },
        )?;
        let base = input.as_ptr().addr();
        let expected_tag = if number.get() < 16 {
            (number.get() as u8) << 3 | 2
        } else {
            0
        };
        let mut offsets = vec![0u32; 2 + retained.div_ceil(2)];
        let mut entry = 0;
        while let Some(offset) = reader.next_preflighted_index_offset(cutoff, expected_tag, base)? {
            let offset = usize::from(offset);
            offsets[2 + entry / 2] |= (offset as u32) << ((entry % 2) * 16);
            entry += 1;
        }
        let last = entry
            .checked_sub(1)
            .ok_or(protobuf::DecodeError::Truncated)?;
        let last_start = ((offsets[2 + last / 2] >> ((last % 2) * 16)) as u16) as usize;
        let (length, width) = protobuf::decode_varint(&input[last_start..])?;
        offsets[0] = ((last_start + width) as u32) << 16 | length as u32;
        offsets[1] = retained as u32;
        let fields = if retained >= 256 {
            Self::periodic_mono_offsets(&offsets, retained, last_start)
                .unwrap_or_else(|| offsets.into_boxed_slice())
        } else {
            offsets.into_boxed_slice()
        };
        Ok(IndexStorage::Compact(CompactIndex::SameBytes(
            SameBytesIndex {
                layout: SameBytesLayout::MonoNarrow(number),
                fields,
            },
        )))
    }
    #[inline(never)]
    fn periodic_mono_offsets(
        offsets: &[u32],
        retained: usize,
        last_start: usize,
    ) -> Option<Box<[u32]>> {
        let words = &offsets[2..];
        let stride = (words[2] as u16 - words[0] as u16) as u32;
        let first_step = (words[0] >> 16) - u32::from(words[0] as u16);
        if stride == 4 * first_step
            && words[1] == words[0].wrapping_add((2 * first_step) | (2 * first_step) << 16)
        {
            return None;
        }
        let last = retained - 1;
        let lane = last % 4;
        let base = (words[lane / 2] >> ((lane % 2) * 16)) as u16;
        if u32::from(base) + stride * (last / 4) as u32 != last_start as u32 {
            return None;
        }
        let pair_stride = stride | stride << 16;
        let full_words = &words[..retained / 2];
        let difference = full_words
            .iter()
            .zip(&full_words[2..])
            .fold(0, |difference, (&prior, &next)| {
                difference | (prior.wrapping_add(pair_stride) ^ next)
            });
        if difference != 0 {
            return None;
        }
        Some(Box::from([
            offsets[0],
            retained as u32,
            stride,
            words[0],
            words[1],
        ]))
    }
    #[inline(never)]
    fn short_bytes(fields: &[WireIndex], schema: &Schema, input: &[u8]) -> Option<SameBytesIndex> {
        let mut starts = [0; SameBytesIndex::DIRECTORY];
        let mut highest = 0;
        let mut narrow = true;
        for field in fields {
            if field.order_and_wire >> 29 != 2 {
                return None;
            }
            *starts.get_mut(field.number.get() as usize + 1)? += 1;
            highest = highest.max(field.number.get());
            let offset = (field.payload >> 32) as usize;
            let length = field.payload as u32 as usize;
            narrow &= length < 128 && usize::from(input[offset - 1]) == length;
        }
        if schema
            .fields
            .iter()
            .take_while(|spec| spec.number() <= highest)
            .any(|spec| spec.repeated())
        {
            return None;
        }
        let mut start = SameBytesIndex::DIRECTORY;
        for offset in &mut starts {
            start += *offset;
            *offset = start;
        }
        if narrow
            && fields.len() >= 256
            && let Some(index) = Self::mixed_stride_index(fields, &starts, highest as usize + 2)
        {
            return Some(index);
        }
        let mut sorted = vec![
            0u32;
            if narrow {
                SameBytesIndex::DIRECTORY + fields.len().div_ceil(2)
            } else {
                start
            }
        ];
        for (entry, offset) in sorted.iter_mut().zip(starts) {
            *entry = offset as u32;
        }
        for field in fields {
            let offset = &mut starts[field.number.get() as usize];
            let wire_offset = (field.payload >> 32) as u32;
            if narrow {
                let entry = *offset - SameBytesIndex::DIRECTORY;
                sorted[SameBytesIndex::DIRECTORY + entry / 2] |= wire_offset << ((entry % 2) * 16);
            } else {
                sorted[*offset] = wire_offset << 16 | field.payload as u32;
            }
            *offset += 1;
        }
        if narrow {
            // Protobuf has no field zero. Its directory slot marks packed 16-bit offsets.
            sorted[0] = 0;
        }
        Some(SameBytesIndex {
            layout: SameBytesLayout::Mixed,
            fields: sorted.into_boxed_slice(),
        })
    }
    #[inline(never)]
    fn mixed_stride_index(
        fields: &[WireIndex],
        starts: &[usize; SameBytesIndex::DIRECTORY],
        width: usize,
    ) -> Option<SameBytesIndex> {
        const WIDTH: usize = SameBytesIndex::DIRECTORY;
        let mut entries = [0u32; 2 * WIDTH];
        let mut counts = [0u16; WIDTH];
        for (index, field) in fields.iter().enumerate() {
            let tag = field.number.get() as usize;
            let offset = (field.payload >> 32) as u32;
            let position = counts[tag];
            let run = &mut entries[WIDTH + tag];
            let base = *run & 0xffff;
            match position {
                0 => *run = offset,
                1 => *run |= (offset - base) << 16,
                _ if offset == base + (*run >> 16) * u32::from(position) => {}
                _ if index >= 32 => {
                    return Some(Self::finish_mixed_stride_fallback(
                        fields, index, starts, &counts, &entries,
                    ));
                }
                _ => return None,
            }
            counts[tag] += 1;
        }
        for (entry, &start) in entries[..width].iter_mut().zip(starts) {
            *entry = start as u32;
        }
        entries.copy_within(WIDTH..WIDTH + width, width);
        Some(SameBytesIndex {
            layout: SameBytesLayout::SortedRuns {
                width: width as u8,
                periodic: false,
            },
            fields: Box::from(&entries[..2 * width]),
        })
    }
    #[cold]
    #[inline(never)]
    fn finish_mixed_stride_fallback(
        fields: &[WireIndex],
        consumed: usize,
        starts: &[usize; SameBytesIndex::DIRECTORY],
        counts: &[u16; SameBytesIndex::DIRECTORY],
        runs: &[u32],
    ) -> SameBytesIndex {
        const WIDTH: usize = SameBytesIndex::DIRECTORY;
        let mut sorted = vec![0u32; WIDTH + fields.len().div_ceil(2)];
        for (entry, &start) in sorted[1..WIDTH].iter_mut().zip(&starts[1..]) {
            *entry = start as u32;
        }
        let mut cursors = *starts;
        for tag in 1..WIDTH - 1 {
            let run = runs[WIDTH + tag];
            let mut offset = run & 0xffff;
            let stride = run >> 16;
            let start = starts[tag] - WIDTH;
            let count = usize::from(counts[tag]);
            for entry in start..start + count {
                sorted[WIDTH + entry / 2] |= offset << ((entry % 2) * 16);
                offset += stride;
            }
            cursors[tag] += count;
        }
        for field in &fields[consumed..] {
            let cursor = &mut cursors[field.number.get() as usize];
            let entry = *cursor - WIDTH;
            sorted[WIDTH + entry / 2] |= ((field.payload >> 32) as u32) << ((entry % 2) * 16);
            *cursor += 1;
        }
        SameBytesIndex {
            layout: SameBytesLayout::Mixed,
            fields: sorted.into_boxed_slice(),
        }
    }
    #[inline(never)]
    #[cold]
    fn stride_index(fields: &[WireIndex], schema: &Schema, input: &[u8]) -> Option<SameBytesIndex> {
        const WIDTH: usize = SameBytesIndex::DIRECTORY;
        let highest = fields.last()?.number.get();
        if highest as usize + 1 >= WIDTH
            || schema
                .fields
                .iter()
                .take_while(|spec| spec.number() <= highest)
                .any(|spec| spec.repeated())
        {
            return None;
        }

        let width = highest as usize + 2;
        let mut entries = [0u32; WIDTH * 3];
        let mut periodic = false;
        entries[0] = WIDTH as u32;
        let mut rest = fields;
        for number in 1..width - 1 {
            entries[number] = (WIDTH + fields.len() - rest.len()) as u32;
            let count = rest.partition_point(|field| field.number.get() <= number as u32);
            let (run, tail) = rest.split_at(count);
            rest = tail;
            let Some(first) = run.first() else { continue };
            let base = (first.payload >> 32) as usize;
            let odd = run
                .get(1)
                .map_or(base, |field| (field.payload >> 32) as usize);
            let stride = odd.checked_sub(base)?;
            let pair_stride = if let Some(third) = run.get(2) {
                ((third.payload >> 32) as usize).checked_sub(base)?
            } else {
                0
            };
            if base.max(odd).max(pair_stride) > u16::MAX as usize {
                return None;
            }
            let paired = run.len() > 2 && pair_stride != 2 * stride;
            periodic |= paired;
            for (i, field) in run.iter().enumerate() {
                let offset = (field.payload >> 32) as usize;
                let length = field.payload as u32 as usize;
                let expected = if paired {
                    let lane = if i % 2 == 0 { base } else { odd };
                    lane + (i / 2) * pair_stride
                } else {
                    base + i * stride
                };
                if field.order_and_wire >> 29 != 2
                    || offset != expected
                    || length >= 128
                    || usize::from(input[offset - 1]) != length
                {
                    return None;
                }
            }
            entries[width + number] = (pair_stride as u32) << 16 | base as u32;
            entries[2 * width + number] = odd as u32;
        }
        entries[width - 1] = (WIDTH + fields.len()) as u32;
        let fields = if periodic {
            Box::from(&entries[..3 * width])
        } else {
            for number in 1..width - 1 {
                let base = entries[width + number] as u16 as u32;
                let odd = entries[2 * width + number];
                entries[width + number] = (odd - base) << 16 | base;
            }
            Box::from(&entries[..2 * width])
        };
        Some(SameBytesIndex {
            layout: SameBytesLayout::SortedRuns {
                width: width as u8,
                periodic,
            },
            fields,
        })
    }
    fn single_text(&self) -> Option<SingleText<'a>> {
        let text = match &self.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::Text(text),
                ..
            }) => *text,
            ViewBacking::Inline(_) => return None,
            ViewBacking::Shared { index, .. } => {
                let IndexTail::Text(text) = index.parts.tail else {
                    return None;
                };
                text
            }
        };
        let number = protobuf::FieldNumber::new(self.schema.fields[0].number())
            .expect("generated schema field");
        Some(SingleText { number, text })
    }
    fn field_numbers(&self) -> impl Iterator<Item = u32> + '_ {
        let mut prior = 0;
        let fields = match &self.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::MediaSuccess(success),
                ..
            }) => IndexIter::MediaSuccess(
                success,
                InlineMediaSuccess::ID..=InlineMediaSuccess::RESULT,
            ),
            ViewBacking::Inline(field) => IndexIter::One(Some(field.indexed(self.schema))),
            ViewBacking::Shared { index, .. } => match &index.parts.tail {
                IndexTail::Wire(field) => IndexIter::General(GeneralIndexIter::Compact(
                    core::slice::from_ref(field).iter(),
                    index.parts.first,
                )),
                IndexTail::BytePair(fields) => {
                    IndexIter::General(GeneralIndexIter::BytePair(fields.iter(), index.parts.first))
                }
                IndexTail::Periodic(periodic) => {
                    IndexIter::Periodic(periodic, 0, index.parts.first)
                }
                IndexTail::ShortByteRun(fields) => IndexIter::General(
                    self.short_byte_run_range(fields, self.schema.fields[0].number()),
                ),
                IndexTail::ShortTaggedRun(_, tags) => IndexIter::ShortTagNumbers(*tags),
                IndexTail::Text(_) => IndexIter::One(self.single_text().map(SingleText::indexed)),
                IndexTail::CompactText(_) | IndexTail::Fragments(_) => {
                    index.storage.iter(index.parts.first)
                }
            },
        };
        fields.filter_map(move |indexed| {
            let number = indexed.number().get();
            if number == prior {
                None
            } else {
                prior = number;
                Some(number)
            }
        })
    }
    fn range(&self, spec: Spec) -> IndexIter<'_, 'a> {
        let index = match &self.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::MediaSuccess(success),
                ..
            }) => {
                return IndexIter::One(success.indexed(spec.number()));
            }
            ViewBacking::Inline(field) => {
                let field = field.indexed(self.schema);
                return IndexIter::One((field.number().get() == spec.number()).then_some(field));
            }
            ViewBacking::Shared { index, .. } => index,
        };
        if let Some(text) = self.single_text() {
            return IndexIter::One((text.number.get() == spec.number()).then(|| text.indexed()));
        }
        if let IndexTail::Periodic(periodic) = &index.parts.tail {
            let start = if periodic.tag.get() == spec.number() {
                0
            } else {
                periodic.len()
            };
            return IndexIter::Periodic(periodic, start, index.parts.first);
        }
        if let IndexStorage::Compact(CompactIndex::SameBytes(bytes)) = &index.storage {
            let range = bytes.range(spec.number());
            return IndexIter::SameBytes(bytes, (range.end << 16) | range.start, index.parts.first);
        }
        IndexIter::General(self.general_range(index, spec))
    }
    fn general_range<'v>(&'v self, index: &'v Index<'a>, spec: Spec) -> GeneralIndexIter<'v, 'a> {
        match &index.storage {
            IndexStorage::Compact(CompactIndex::SameBytes(_)) => GeneralIndexIter::Many([].iter()),
            IndexStorage::Compact(CompactIndex::BytePair(pair)) => {
                let start = usize::from(pair[0].number.get() < spec.number())
                    + usize::from(pair[1].number.get() < spec.number());
                let end = start
                    + usize::from(pair[0].number.get() == spec.number())
                    + usize::from(pair[1].number.get() == spec.number());
                GeneralIndexIter::BytePair(pair[start..end].iter(), index.parts.first)
            }
            IndexStorage::Compact(CompactIndex::ByteTriple(fields)) => {
                let start = fields.partition_point(|field| field.number() < spec.number());
                let end = fields.partition_point(|field| field.number() <= spec.number());
                GeneralIndexIter::ByteTriple(fields[start..end].iter(), index.parts.first)
            }
            IndexStorage::Compact(CompactIndex::ByteQuad(fields)) => {
                let start = fields.partition_point(|field| field.number() < spec.number());
                let end = fields.partition_point(|field| field.number() <= spec.number());
                GeneralIndexIter::ByteTriple(fields[start..end].iter(), index.parts.first)
            }
            IndexStorage::Compact(CompactIndex::General(fields)) => {
                let start = fields.partition_point(|v| v.number.get() < spec.number());
                let end = fields.partition_point(|v| v.number.get() <= spec.number());
                GeneralIndexIter::Compact(fields[start..end].iter(), index.parts.first)
            }
            IndexStorage::Many { fields, .. } => {
                if let IndexTail::ShortTaggedRun(offsets, _) = &index.parts.tail {
                    return self.short_tagged_run_range(offsets, spec.number());
                }
                if let IndexTail::ShortByteRun(fields) = &index.parts.tail {
                    return self.short_byte_run_range(fields, spec.number());
                }
                if fields.is_empty()
                    && let IndexTail::BytePair(pair) = &index.parts.tail
                {
                    let start = usize::from(pair[0].number.get() < spec.number())
                        + usize::from(pair[1].number.get() < spec.number());
                    let end = start
                        + usize::from(pair[0].number.get() == spec.number())
                        + usize::from(pair[1].number.get() == spec.number());
                    return GeneralIndexIter::BytePair(pair[start..end].iter(), index.parts.first);
                }
                if fields.is_empty()
                    && let IndexTail::Wire(field) = &index.parts.tail
                {
                    let selected = if field.number.get() == spec.number() {
                        core::slice::from_ref(field)
                    } else {
                        &[]
                    };
                    return GeneralIndexIter::Compact(selected.iter(), index.parts.first);
                }
                let start = fields.partition_point(|v| v.number().get() < spec.number());
                let end = fields.partition_point(|v| v.number().get() <= spec.number());
                GeneralIndexIter::Many(fields[start..end].iter())
            }
        }
    }
    fn since(&self, spec: Spec) -> Option<usize> {
        let Some(group) = spec.oneof() else {
            return Some(0);
        };
        self.active()
            .get(group)
            .copied()
            .flatten()
            .filter(|a| a.number == spec.number())
            .map(|a| a.since)
    }
    fn has(&self, spec: Spec) -> bool {
        let Some(since) = self.since(spec) else {
            return false;
        };
        self.range(spec)
            .any(|v| v.order >= since && spec.known(v.value()))
    }
    fn scalar<T: Scalar<'a>>(&self, spec: Spec) -> Result<Option<T>, Error> {
        let Some(since) = self.since(spec) else {
            return Ok(None);
        };
        let shared = match &self.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::MediaSuccess(success),
                ..
            }) => {
                let Some(field) = success.indexed(spec.number()) else {
                    return Ok(None);
                };
                let value = field.value();
                return if spec.known(value) {
                    T::read(value, spec.kind())
                } else {
                    Ok(None)
                };
            }
            ViewBacking::Inline(field) => {
                let field = field.indexed(self.schema);
                let value = field.value();
                return if spec.number() == field.number().get() && spec.known(value) {
                    T::read(value, spec.kind())
                } else {
                    Ok(None)
                };
            }
            ViewBacking::Shared { index, .. } => index,
        };
        let fields = match &shared.storage {
            IndexStorage::Compact(CompactIndex::SameBytes(index)) => {
                if let SameBytesLayout::MonoNarrow(number) = index.layout {
                    if number.get() != spec.number() {
                        return Ok(None);
                    }
                    let input = shared.parts.first;
                    let last = Value::Bytes(short_wire_bytes(index.fields[0], input));
                    if spec.known(last)
                        && let Some(value) = T::read(last, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                    for position in (0..index.fields[1] as usize - 1).rev() {
                        let value = Value::Bytes(index.bytes(number.get(), position, input));
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                if index.is_narrow() {
                    for position in index.range(spec.number()).rev() {
                        let value =
                            Value::Bytes(index.bytes(spec.number(), position, shared.parts.first));
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                for &payload in index.fields[index.range(spec.number())].iter().rev() {
                    let value = Value::Bytes(short_wire_bytes(payload, shared.parts.first));
                    if spec.known(value)
                        && let Some(value) = T::read(value, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                }
                return Ok(None);
            }
            IndexStorage::Compact(CompactIndex::BytePair(pair)) => {
                for field in pair
                    .iter()
                    .rev()
                    .filter(|field| field.number.get() == spec.number())
                {
                    let value = field.value(shared.parts.first);
                    if spec.known(value)
                        && let Some(value) = T::read(value, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                }
                return Ok(None);
            }
            IndexStorage::Compact(CompactIndex::ByteTriple(fields)) => {
                for field in fields
                    .iter()
                    .rev()
                    .filter(|field| field.number() == spec.number())
                {
                    let value = field.value(shared.parts.first);
                    if spec.known(value)
                        && let Some(value) = T::read(value, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                }
                return Ok(None);
            }
            IndexStorage::Compact(CompactIndex::ByteQuad(fields)) => {
                for field in fields
                    .iter()
                    .rev()
                    .filter(|field| field.number() == spec.number())
                {
                    let value = field.value(shared.parts.first);
                    if spec.known(value)
                        && let Some(value) = T::read(value, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                }
                return Ok(None);
            }
            IndexStorage::Compact(CompactIndex::General(fields)) => {
                let number = spec.number();
                let end = if fields
                    .last()
                    .is_some_and(|field| field.number.get() == number)
                {
                    fields.len()
                } else {
                    fields.partition_point(|field| field.number.get() <= number)
                };
                for field in fields[..end].iter().rev() {
                    if field.number.get() != spec.number() {
                        break;
                    }
                    let value = field.value(shared.parts.first);
                    if spec.known(value)
                        && let Some(value) = T::read(value, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                }
                return Ok(None);
            }
            IndexStorage::Many { fields, .. } => {
                if let IndexTail::ShortTaggedRun(offsets, _) = &shared.parts.tail {
                    let input = shared.parts.first;
                    for &offset in offsets.iter().rev().filter(|&&offset| offset != 0) {
                        let start = usize::from(offset);
                        if u32::from(input[start - 2] >> 3) != spec.number() {
                            continue;
                        }
                        let length = usize::from(input[start - 1]);
                        let value = Value::Bytes(&input[start..start + length]);
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                if let IndexTail::ShortByteRun(fields) = &shared.parts.tail {
                    if spec.number() != self.schema.fields[0].number() {
                        return Ok(None);
                    }
                    for &field in short_byte_run(fields).iter().rev() {
                        let value = Value::Bytes(short_byte_run_bytes(field, shared.parts.first));
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                if fields.is_empty()
                    && let Some(text) = self.single_text()
                {
                    return if text.number.get() == spec.number() && spec.known(text.value()) {
                        T::read(text.value(), spec.kind())
                    } else {
                        Ok(None)
                    };
                }
                if fields.is_empty()
                    && let IndexTail::Periodic(index) = &shared.parts.tail
                {
                    if index.tag.get() != spec.number() {
                        return Ok(None);
                    }
                    let input = shared.parts.first;
                    let last = Value::Bytes(short_wire_bytes(index.last_span, input));
                    if spec.known(last)
                        && let Some(value) = T::read(last, spec.kind())?
                    {
                        return Ok(Some(value));
                    }
                    for position in (0..index.len() - 1).rev() {
                        let value = Value::Bytes(index.bytes(position, input));
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                if fields.is_empty()
                    && let IndexTail::BytePair(pair) = &shared.parts.tail
                {
                    for field in pair
                        .iter()
                        .rev()
                        .filter(|field| field.number.get() == spec.number())
                    {
                        let value = field.value(shared.parts.first);
                        if spec.known(value)
                            && let Some(value) = T::read(value, spec.kind())?
                        {
                            return Ok(Some(value));
                        }
                    }
                    return Ok(None);
                }
                if fields.is_empty()
                    && let IndexTail::Wire(field) = &shared.parts.tail
                {
                    if field.number.get() == spec.number() {
                        let value = field.value(shared.parts.first);
                        if spec.known(value) {
                            return T::read(value, spec.kind());
                        }
                    }
                    return Ok(None);
                }
                fields
            }
        };
        let number = spec.number();
        let end = fields.partition_point(|field| field.number().get() <= number);
        // Preflight validates every string, including overwritten values.
        for field in fields[..end].iter().rev() {
            if field.number().get() != number || field.order < since {
                break;
            }
            if spec.known(field.value())
                && let Some(value) = T::read(field.value(), spec.kind())?
            {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
    fn repeated<T: Scalar<'a>>(&self, spec: Spec) -> Repeated<'_, 'a, T> {
        let (fields, packed) = match &self.index {
            ViewBacking::Inline(field) => {
                let packed = match field.data {
                    InlineData::Packed { bytes, number, .. }
                        if number.get() == spec.number() && spec.kind().packable() =>
                    {
                        bytes
                    }
                    _ => &[],
                };
                (GeneralIndexIter::Many([].iter()), packed)
            }
            ViewBacking::Shared { index, .. } => {
                if let IndexTail::Wire(field) = index.parts.tail
                    && field.number.get() == spec.number()
                    && field.order_and_wire >> 29 == 2
                    && spec.kind().packable()
                {
                    (
                        GeneralIndexIter::Many([].iter()),
                        wire_bytes(field.payload, index.parts.first),
                    )
                } else {
                    (self.general_range(index, spec), &[][..])
                }
            }
        };
        Repeated {
            fields,
            spec,
            packed,
            _value: PhantomData,
        }
    }
    fn child(&self, spec: Spec, schema: &'static Schema) -> Result<Option<Self>, Error> {
        self.child_admitted(spec, schema, |_| Ok(()))
    }
    fn child_admitted(
        &self,
        spec: Spec,
        schema: &'static Schema,
        admit: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Option<Self>, Error> {
        let Some(since) = self.since(spec) else {
            return Ok(None);
        };
        let mut parts = None;
        for v in self.range(spec) {
            if v.order < since || !spec.known(v.value()) {
                continue;
            }
            let bytes = match v.value() {
                Value::Bytes(bytes) | Value::Group(bytes) => bytes,
                _ => return Err(Error::Type),
            };
            match &mut parts {
                None => parts = Some(Parts::one(bytes)),
                Some(parts) => parts.push(bytes, self.limits())?,
            }
        }
        parts
            .map(|parts| {
                Self::parse_parts_admitted(
                    parts,
                    schema,
                    self.limits(),
                    self.depth().checked_add(1).ok_or(Error::Limit)?,
                    true,
                    admit,
                )
            })
            .transpose()
    }
    fn children(
        &self,
        spec: Spec,
        schema: &'static Schema,
    ) -> impl Iterator<Item = Result<Self, Error>> + '_ {
        self.range(spec)
            .filter(move |v| spec.known(v.value()))
            .map(move |v| {
                let bytes = match v.value() {
                    Value::Bytes(bytes) | Value::Group(bytes) => bytes,
                    _ => return Err(Error::Type),
                };
                Self::parse_parts(
                    Parts::one(bytes),
                    schema,
                    self.limits(),
                    self.depth().checked_add(1).ok_or(Error::Limit)?,
                )
            })
    }
    fn unknown_fields(&self) -> UnknownFields<'_, 'a> {
        UnknownFields {
            view: self,
            part: 0,
            reader: None,
            packed: None,
            failed: false,
        }
    }
}
fn packed_varint_next(bytes: &mut &[u8]) -> Result<u64, Error> {
    let (&first, tail) = bytes
        .split_first()
        .ok_or(protobuf::DecodeError::Truncated)?;
    if first < 128 {
        *bytes = tail;
        return Ok(u64::from(first));
    }
    let mut input = tail;
    let mut value = u64::from(first & 127);
    for shift in (7..63).step_by(7) {
        let (&byte, tail) = input
            .split_first()
            .ok_or(protobuf::DecodeError::Truncated)?;
        input = tail;
        value |= u64::from(byte & 127) << shift;
        if byte < 128 {
            *bytes = input;
            return Ok(value);
        }
    }
    let (&byte, tail) = input
        .split_first()
        .ok_or(protobuf::DecodeError::Truncated)?;
    if byte > 1 {
        return Err(protobuf::DecodeError::VarintOverflow.into());
    }
    *bytes = tail;
    Ok(value | (u64::from(byte) << 63))
}

fn packed_next<'a, const ITERATE: bool>(
    bytes: &mut &'a [u8],
    kind: Kind,
) -> Result<Value<'a>, Error> {
    let take = match kind.wire() {
        0 if ITERATE => return packed_varint_next(bytes).map(Value::Varint),
        0 => {
            let (value, n) = protobuf::decode_varint(bytes)?;
            *bytes = &bytes[n..];
            return Ok(Value::Varint(value));
        }
        1 => 8,
        5 => 4,
        _ => return Err(Error::Type),
    };
    let (head, tail) = bytes
        .split_at_checked(take)
        .ok_or(protobuf::DecodeError::Truncated)?;
    *bytes = tail;
    if take == 8 {
        Ok(Value::Fixed64(u64::from_le_bytes(
            head.try_into().map_err(|_| Error::Type)?,
        )))
    } else {
        Ok(Value::Fixed32(u32::from_le_bytes(
            head.try_into().map_err(|_| Error::Type)?,
        )))
    }
}
fn packed_varint_count(bytes: &[u8], limit: usize) -> Result<usize, Error> {
    if limit >= bytes.len() {
        let prefix = bytes
            .as_chunks::<32>()
            .0
            .iter()
            .take_while(|word| word.is_ascii())
            .count()
            * 32;
        packed_varint_count_inner::<false>(&bytes[prefix..], limit).map(|count| prefix + count)
    } else {
        packed_varint_count_inner::<true>(bytes, limit)
    }
}

#[inline(never)]
fn packed_varint_count_inner<const CHECK_LIMIT: bool>(
    mut bytes: &[u8],
    limit: usize,
) -> Result<usize, Error> {
    let mut count = 0;
    while !bytes.is_empty() {
        if bytes.len() >= 8 {
            let mut consumed = 0;
            for group in bytes.as_chunks::<8>().0 {
                let word = u64::from_le_bytes(*group);
                let terminators = !word & 0x8080_8080_8080_8080;
                if terminators as u32 == 0 || terminators >> 32 == 0 {
                    break;
                }
                let complete =
                    ((terminators >> 7).wrapping_mul(0x0101_0101_0101_0101) >> 56) as usize;
                if CHECK_LIMIT && limit - count < complete {
                    return Err(Error::Limit);
                }
                count += complete;
                consumed += 8;
            }
            if consumed != 0 {
                let tail = bytes[..consumed]
                    .iter()
                    .rev()
                    .take_while(|&&byte| byte >= 128)
                    .count();
                bytes = &bytes[consumed - tail..];
                if bytes.is_empty() {
                    break;
                }
            }
        }
        let (&first, tail) = bytes
            .split_first()
            .ok_or(protobuf::DecodeError::Truncated)?;
        if CHECK_LIMIT && count == limit {
            return Err(Error::Limit);
        }
        bytes = tail;
        count += 1;
        if first < 128 {
            continue;
        }
        for index in 1..10 {
            let (&byte, tail) = bytes
                .split_first()
                .ok_or(protobuf::DecodeError::Truncated)?;
            if index == 9 && byte > 1 {
                return Err(protobuf::DecodeError::VarintOverflow.into());
            }
            bytes = tail;
            if byte < 128 {
                break;
            }
        }
    }
    Ok(count)
}

fn packed_count(mut bytes: &[u8], kind: Kind, limit: usize) -> Result<usize, Error> {
    if kind.wire() == 0 {
        if let [byte] = bytes {
            return if limit == 0 {
                Err(Error::Limit)
            } else if *byte >= 128 {
                Err(protobuf::DecodeError::Truncated.into())
            } else {
                Ok(1)
            };
        }
        if bytes.len() <= 8 {
            let count = if let Ok(word) = <[u8; 8]>::try_from(bytes) {
                let ends = !u64::from_le_bytes(word) & 0x8080_8080_8080_8080;
                ((ends >> 7).wrapping_mul(0x0101_0101_0101_0101) >> 56) as usize
            } else if let [a, b, c, d] = bytes {
                let continuations = usize::from(a >> 7)
                    + usize::from(b >> 7)
                    + usize::from(c >> 7)
                    + usize::from(d >> 7);
                4 - continuations
            } else {
                bytes.iter().map(|byte| usize::from(byte >> 7 == 0)).sum()
            };
            let truncated = bytes.last().is_some_and(|&byte| byte >= 128);
            return if count > limit || (count == limit && truncated) {
                Err(Error::Limit)
            } else if truncated {
                Err(protobuf::DecodeError::Truncated.into())
            } else {
                Ok(count)
            };
        }
        if bytes.len() == 32 && bytes.is_ascii() {
            return if bytes.len() <= limit {
                Ok(bytes.len())
            } else {
                Err(Error::Limit)
            };
        }
        return packed_varint_count(bytes, limit);
    }
    let mut n = 0usize;
    while !bytes.is_empty() {
        if n == limit {
            return Err(Error::Limit);
        }
        packed_next::<false>(&mut bytes, kind)?;
        n = n.checked_add(1).ok_or(Error::Limit)?;
    }
    Ok(n)
}
pub trait Scalar<'a>: Sized {
    fn read(value: Value<'a>, kind: Kind) -> Result<Option<Self>, Error>;
    fn write(self, kind: Kind) -> Result<Value<'a>, Error>;
}
macro_rules! scalar {
    ($ty:ty,$read:expr,$write:expr) => {
        impl<'a> Scalar<'a> for $ty {
            fn read(value: Value<'a>, kind: Kind) -> Result<Option<Self>, Error> {
                ($read)(value, kind).map(Some)
            }
            fn write(self, kind: Kind) -> Result<Value<'a>, Error> {
                ($write)(self, kind)
            }
        }
    };
}
scalar!(
    u64,
    |v, k| match (v, k) {
        (Value::Varint(n), Kind::Uint64) => Ok(n),
        (Value::Fixed64(n), Kind::Fixed64) => Ok(n),
        _ => Err(Error::Type),
    },
    |v, k| match k {
        Kind::Uint64 => Ok(Value::Varint(v)),
        Kind::Fixed64 => Ok(Value::Fixed64(v)),
        _ => Err(Error::Type),
    }
);
scalar!(
    u32,
    |v, k| match (v, k) {
        (Value::Varint(n), Kind::Uint32) => Ok(n as u32),
        (Value::Fixed32(n), Kind::Fixed32) => Ok(n),
        _ => Err(Error::Type),
    },
    |v, k| match k {
        Kind::Uint32 => Ok(Value::Varint(u64::from(v))),
        Kind::Fixed32 => Ok(Value::Fixed32(v)),
        _ => Err(Error::Type),
    }
);
scalar!(
    i64,
    |v, k| match (v, k) {
        (Value::Varint(n), Kind::Int64) => Ok(n as i64),
        (Value::Varint(n), Kind::Sint64) => Ok(((n >> 1) as i64) ^ -((n & 1) as i64)),
        (Value::Fixed64(n), Kind::Sfixed64) => Ok(n as i64),
        _ => Err(Error::Type),
    },
    |v: i64, k| match k {
        Kind::Int64 => Ok(Value::Varint(v as u64)),
        Kind::Sint64 => Ok(Value::Varint(((v << 1) ^ (v >> 63)) as u64)),
        Kind::Sfixed64 => Ok(Value::Fixed64(v as u64)),
        _ => Err(Error::Type),
    }
);
scalar!(
    i32,
    |v, k| match (v, k) {
        (Value::Varint(n), Kind::Int32) => Ok(n as i32),
        (Value::Varint(n), Kind::Sint32) => {
            let n = n as u32;
            Ok(((n >> 1) as i32) ^ -((n & 1) as i32))
        }
        (Value::Fixed32(n), Kind::Sfixed32) => Ok(n as i32),
        _ => Err(Error::Type),
    },
    |v: i32, k| match k {
        Kind::Int32 => Ok(Value::Varint(v as i64 as u64)),
        Kind::Sint32 => Ok(Value::Varint(u64::from(((v << 1) ^ (v >> 31)) as u32))),
        Kind::Sfixed32 => Ok(Value::Fixed32(v as u32)),
        _ => Err(Error::Type),
    }
);
scalar!(
    bool,
    |v, _| match v {
        Value::Varint(n) => Ok(n != 0),
        _ => Err(Error::Type),
    },
    |v: bool, _| Ok(Value::Varint(u64::from(v)))
);
scalar!(
    f32,
    |v, _| match v {
        Value::Fixed32(n) => Ok(f32::from_bits(n)),
        _ => Err(Error::Type),
    },
    |v: f32, _| Ok(Value::Fixed32(v.to_bits()))
);
scalar!(
    f64,
    |v, _| match v {
        Value::Fixed64(n) => Ok(f64::from_bits(n)),
        _ => Err(Error::Type),
    },
    |v: f64, _| Ok(Value::Fixed64(v.to_bits()))
);
scalar!(
    &'a str,
    |v, _| match v {
        Value::Bytes(bytes) => core::str::from_utf8(bytes).map_err(|_| Error::Utf8),
        _ => Err(Error::Type),
    },
    |v: &'a str, _| Ok(Value::Bytes(v.as_bytes()))
);
scalar!(
    &'a [u8],
    |v, _| match v {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(Error::Type),
    },
    |v: &'a [u8], _| Ok(Value::Bytes(v))
);
pub trait Enumeration: Copy {
    fn from_number(number: i32) -> Option<Self>;
    fn number(self) -> i32;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnumValue<E> {
    Known(E),
    Unknown(i32),
}
impl<E: Enumeration> EnumValue<E> {
    pub fn number(self) -> i32 {
        match self {
            Self::Known(v) => v.number(),
            Self::Unknown(n) => n,
        }
    }
}
impl<'a, E: Enumeration> Scalar<'a> for EnumValue<E> {
    fn read(v: Value<'a>, _: Kind) -> Result<Option<Self>, Error> {
        let Value::Varint(n) = v else {
            return Err(Error::Type);
        };
        Ok(Some(match E::from_number(n as i32) {
            Some(v) => Self::Known(v),
            None => Self::Unknown(n as i32),
        }))
    }
    fn write(self, _: Kind) -> Result<Value<'a>, Error> {
        Ok(Value::Varint(self.number() as i64 as u64))
    }
}
pub struct Repeated<'v, 'a, T> {
    fields: GeneralIndexIter<'v, 'a>,
    spec: Spec,
    packed: &'a [u8],
    _value: PhantomData<T>,
}
impl<'a, T: Scalar<'a>> Iterator for Repeated<'_, 'a, T> {
    type Item = Result<T, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let value = if !self.packed.is_empty() {
                match packed_next::<true>(&mut self.packed, self.spec.kind()) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                }
            } else {
                let value = self.fields.next_value()?;
                if !self.spec.accepts(value) {
                    continue;
                }
                if self.spec.kind().packable()
                    && let Value::Bytes(bytes) = value
                {
                    self.packed = bytes;
                    continue;
                }
                value
            };
            match T::read(value, self.spec.kind()) {
                Ok(Some(v)) => return Some(Ok(v)),
                Ok(None) => continue,
                Err(e) => return Some(Err(e)),
            }
        }
    }

    fn fold<B, F>(mut self, mut accumulator: B, mut fold: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let kind = self.spec.kind();
        loop {
            let mut packed = core::mem::take(&mut self.packed);
            if kind.wire() == 0 {
                if packed.len() >= 8 {
                    (accumulator, packed) =
                        fold_varint_prefix::<T, _, _>(packed, kind, accumulator, &mut fold);
                }
                while !packed.is_empty() {
                    let value = packed_varint_next(&mut packed)
                        .and_then(|value| T::read(Value::Varint(value), kind));
                    match value {
                        Ok(Some(value)) => accumulator = fold(accumulator, Ok(value)),
                        Ok(None) => {}
                        Err(error) => accumulator = fold(accumulator, Err(error)),
                    }
                }
            } else {
                while !packed.is_empty() {
                    match packed_next::<true>(&mut packed, kind)
                        .and_then(|value| T::read(value, kind))
                    {
                        Ok(Some(value)) => accumulator = fold(accumulator, Ok(value)),
                        Ok(None) => {}
                        Err(error) => accumulator = fold(accumulator, Err(error)),
                    }
                }
            }
            let Some(value) = self.next() else {
                return accumulator;
            };
            accumulator = fold(accumulator, value);
        }
    }
}
#[inline(never)]
fn fold_varint_prefix<'a, T: Scalar<'a>, B, F>(
    mut packed: &'a [u8],
    kind: Kind,
    mut accumulator: B,
    fold: &mut F,
) -> (B, &'a [u8])
where
    F: FnMut(B, Result<T, Error>) -> B,
{
    if let Some((group, rest)) = packed.split_first_chunk::<4>() {
        let [a, b, c, d] = group.map(u64::from);
        let pair = |lead, tail| (lead & 0x7f) | tail << 7;
        let prefix = match u32::from_le_bytes(*group) & 0x8080_8080 {
            0x0000_0080 => Some([pair(a, b), c, d]),
            0x0000_8000 => Some([a, pair(b, c), d]),
            0x0080_0000 => Some([a, b, pair(c, d)]),
            _ => None,
        };
        if let Some(prefix) = prefix {
            for value in prefix {
                match T::read(Value::Varint(value), kind) {
                    Ok(Some(value)) => accumulator = fold(accumulator, Ok(value)),
                    Ok(None) => {}
                    Err(error) => accumulator = fold(accumulator, Err(error)),
                }
            }
            packed = rest;
        }
    }
    while let Some((group, rest)) = packed.split_first_chunk::<4>() {
        let [a, b, c, d] = *group;
        match u32::from_le_bytes(*group) & 0x8080_8080 {
            0 => {
                for byte in [a, b, c, d] {
                    match T::read(Value::Varint(u64::from(byte)), kind) {
                        Ok(Some(value)) => accumulator = fold(accumulator, Ok(value)),
                        Ok(None) => {}
                        Err(error) => accumulator = fold(accumulator, Err(error)),
                    }
                }
            }
            0x0080_0080 => {
                for (lead, tail) in [(a, b), (c, d)] {
                    let value = u64::from(lead & 0x7f) | u64::from(tail) << 7;
                    match T::read(Value::Varint(value), kind) {
                        Ok(Some(value)) => accumulator = fold(accumulator, Ok(value)),
                        Ok(None) => {}
                        Err(error) => accumulator = fold(accumulator, Err(error)),
                    }
                }
            }
            _ => break,
        }
        packed = rest;
    }
    (accumulator, packed)
}

enum Encoded<'a> {
    Scalar(Value<'a>),
    Message(View<'a>),
}
enum EditValues<'a> {
    BorrowedBytes(&'a [u8]),
    SingleVarint(u64),
    List(Vec<Encoded<'a>>),
}
impl<'a> EditValues<'a> {
    fn len(&self) -> usize {
        match self {
            Self::BorrowedBytes(_) | Self::SingleVarint(_) => 1,
            Self::List(values) => values.len(),
        }
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn push(&mut self, value: Encoded<'a>, kind: Kind) {
        let first = match self {
            Self::List(values) => {
                if values.is_empty() {
                    match value {
                        Encoded::Scalar(Value::Bytes(bytes))
                            if matches!(kind, Kind::String | Kind::Bytes) =>
                        {
                            *self = Self::BorrowedBytes(bytes);
                            return;
                        }
                        Encoded::Scalar(Value::Varint(number)) => {
                            *self = Self::SingleVarint(number);
                            return;
                        }
                        _ => {}
                    }
                }
                values.push(value);
                return;
            }
            Self::BorrowedBytes(bytes) => Value::Bytes(bytes),
            Self::SingleVarint(number) => Value::Varint(*number),
        };
        let mut values = Vec::with_capacity(4);
        values.push(Encoded::Scalar(first));
        values.push(value);
        *self = Self::List(values);
    }
}
struct Edit<'a, Encoding = bool> {
    packed: Encoding,
    clear: bool,
    values: EditValues<'a>,
    charge: usize,
}
impl Edit<'_> {
    // Keep admission based on the original record containing a full field descriptor.
    const ADMISSION_BYTES: usize = core::mem::size_of::<Edit<'static, Spec>>();
}
struct Builder<'a> {
    schema: &'static Schema,
    base: Option<View<'a>>,
    edits: BTreeMap<(u32, u8), Edit<'a>>,
    limits: Limits,
    values: usize,
    charge: usize,
    selected: BTreeMap<usize, (u32, u8)>,
}
impl<'a> Builder<'a> {
    fn new(schema: &'static Schema, limits: Limits) -> Self {
        Self {
            schema,
            base: None,
            edits: BTreeMap::new(),
            limits,
            values: 0,
            charge: 0,
            selected: BTreeMap::new(),
        }
    }
    fn from_view(view: &View<'a>) -> Self {
        let mut result = Self::new(view.schema, view.limits());
        result.charge = view.charge();
        result.base = Some(view.clone());
        result
    }
    fn scalar<T: Scalar<'a>>(&mut self, spec: Spec, value: T, push: bool) -> Result<(), Error> {
        let value = value.write(spec.kind())?;
        if !spec.known(value) {
            return Err(Error::Type);
        }
        self.edit(spec, Some(Encoded::Scalar(value)), push)
    }
    fn message(&mut self, spec: Spec, value: &View<'a>, push: bool) -> Result<(), Error> {
        self.edit(spec, Some(Encoded::Message(value.clone())), push)
    }
    fn clear(&mut self, spec: Spec) -> Result<(), Error> {
        if let Some(group) = spec.oneof() {
            let selected = if let Some(key) = self.selected.get(&group) {
                self.edits
                    .get(key)
                    .filter(|edit| !edit.values.is_empty())
                    .map(|_| key.0)
            } else {
                self.base.as_ref().and_then(|view| {
                    view.active()
                        .get(group)
                        .copied()
                        .flatten()
                        .map(|active| active.number)
                })
            };
            if selected != Some(spec.number()) {
                return Ok(());
            }
        }
        self.edit(spec, None, false)
    }
    fn edit(&mut self, spec: Spec, value: Option<Encoded<'a>>, push: bool) -> Result<(), Error> {
        let key = (spec.number(), spec.kind().wire());
        let existing = self.edits.get(&key);
        let group = spec.oneof();
        let old = if let Some(group) = group {
            self.selected.get(&group).copied()
        } else if push {
            None
        } else {
            existing.map(|_| key)
        };
        let entry_charge = Edit::ADMISSION_BYTES
            .checked_add(128)
            .and_then(|n| n.checked_add(4 * core::mem::size_of::<Encoded<'_>>()))
            .ok_or(Error::Limit)?;
        let removed = old.and_then(|old| {
            if old == key {
                existing
            } else {
                self.edits.get(&old)
            }
        });
        let removed_count = removed.map_or(0, |edit| edit.values.len());
        let removed_charge = removed.map_or(0, |edit| edit.charge + entry_charge);
        let add_entry = existing.is_none() || old == Some(key);
        let new_group = group.is_some() && old.is_none();
        let added_value = match &value {
            None => 0,
            Some(value) => 2usize
                .checked_mul(core::mem::size_of::<Encoded<'_>>())
                .and_then(|n| {
                    n.checked_add(match value {
                        Encoded::Scalar(_) => 0,
                        Encoded::Message(v) => v.charge(),
                    })
                })
                .ok_or(Error::Limit)?,
        };
        let count = self
            .values
            .checked_sub(removed_count)
            .and_then(|n| n.checked_add(usize::from(value.is_some())))
            .filter(|n| *n <= self.limits.max_values)
            .ok_or(Error::Limit)?;
        if self.edits.len() - usize::from(removed.is_some()) + usize::from(add_entry)
            > self.limits.max_fields
        {
            return Err(Error::Limit);
        }
        let charge = self
            .charge
            .checked_sub(removed_charge)
            .and_then(|n| n.checked_add(if add_entry { entry_charge } else { 0 }))
            .and_then(|n| n.checked_add(if new_group { 256 } else { 0 }))
            .and_then(|n| n.checked_add(added_value))
            .filter(|n| *n <= self.limits.max_builder_bytes)
            .ok_or(Error::Limit)?;
        if let Some(old) = old
            && old != key
        {
            self.edits.remove(&old);
        }
        if let Some(group) = group {
            self.selected.insert(group, key);
        }
        let empty = || Edit {
            packed: spec.packed(),
            clear: !push,
            values: EditValues::List(Vec::new()),
            charge: 0,
        };
        let entry = match self.edits.entry(key) {
            alloc::collections::btree_map::Entry::Occupied(mut entry) => {
                if old == Some(key) {
                    entry.insert(empty());
                }
                entry.into_mut()
            }
            alloc::collections::btree_map::Entry::Vacant(entry) => entry.insert(empty()),
        };
        if let Some(value) = value {
            entry.values.push(value, spec.kind());
            entry.charge = entry.charge.checked_add(added_value).ok_or(Error::Limit)?;
        }
        self.values = count;
        self.charge = charge;
        Ok(())
    }
    fn retained_field(
        &self,
        field: WireField<'a>,
        only_edit: Option<u32>,
    ) -> Result<RetainedField<'a>, Error> {
        // The validated base can keep every other tag when the edit selects no oneof.
        if only_edit.is_some_and(|tag| tag != field.number.get()) {
            return Ok(RetainedField::Raw(field.raw));
        }
        let Some(spec) = self.schema.classify(field.number, field.value)? else {
            return Ok(RetainedField::Raw(field.raw));
        };
        if !self
            .selected
            .contains_key(&spec.oneof().unwrap_or(usize::MAX))
            && !self
                .edits
                .get(&(spec.number(), spec.kind().wire()))
                .is_some_and(|edit| edit.clear)
        {
            return Ok(RetainedField::Raw(field.raw));
        }
        if let Some(unknown) = unknown_packed_len(spec, field.value)?
            && unknown != 0
        {
            let Value::Bytes(bytes) = field.value else {
                return Err(Error::Type);
            };
            return if unknown == bytes.len() {
                Ok(RetainedField::Raw(field.raw))
            } else {
                Ok(RetainedField::Packed {
                    spec,
                    bytes,
                    unknown,
                })
            };
        }
        Ok(RetainedField::Removed)
    }
    fn retained(&self) -> Result<usize, Error> {
        Ok(self.charge)
    }
    fn finish(self) -> Result<Vec<u8>, Error> {
        let schema = self.schema;
        let limits = self.limits;
        // Retained strings passed preflight and string edits came from typed &str tokens.
        let typed_strings = schema.map_defaults.is_empty();
        let output = self.encode()?;
        if schema.oneofs == 0 && schema.required_prefix == 0 {
            let parts = Parts::one(&output);
            if typed_strings {
                View::measure_parts_with_utf8::<false>(&parts, schema, limits, 0)?;
            } else {
                View::measure_parts(&parts, schema, limits, 0)?;
            }
        } else {
            View::parse(&output, schema, limits)?;
        }
        Ok(output)
    }
    fn encode(mut self) -> Result<Vec<u8>, Error> {
        if let Some(base) = &self.base
            && self.edits.is_empty()
            && self.selected.is_empty()
            && self.schema.map_defaults.is_empty()
        {
            let length = base.wire_len()?;
            if length > self.limits.max_bytes
                || length
                    .checked_add(self.retained()?)
                    .is_none_or(|n| n > self.limits.max_builder_bytes)
            {
                return Err(Error::Limit);
            }
            let mut output = Vec::with_capacity(length);
            for part in base.wire_parts() {
                output.extend_from_slice(part);
            }
            return Ok(output);
        }
        for (index, value) in self.schema.map_defaults {
            let spec = self.schema.fields[*index];
            let key = (spec.number(), spec.kind().wire());
            let present = self.edits.get(&key).is_some_and(|edit| {
                !edit.values.is_empty()
                    || !edit.clear && self.base.as_ref().is_some_and(|base| base.has(spec))
            }) || !self.edits.contains_key(&key)
                && self.base.as_ref().is_some_and(|base| base.has(spec));
            if !present {
                self.edit(spec, Some(Encoded::Scalar(*value)), false)?;
            }
        }
        let only_edit = if self.base.is_some() && self.edits.len() == 1 && self.selected.is_empty()
        {
            self.edits.first_key_value().map(|(&(tag, _), _)| tag)
        } else {
            None
        };
        let mut length = 0usize;
        if let Some(base) = &self.base {
            for part in base.wire_parts() {
                for field in Reader::new(
                    part,
                    protobuf::Limits {
                        max_message_bytes: self.limits.max_bytes,
                        max_fields: self.limits.max_fields,
                        max_group_depth: protobuf::MAX_GROUP_DEPTH,
                    },
                )? {
                    let field = field?;
                    length = length
                        .checked_add(self.retained_field(field, only_edit)?.len()?)
                        .ok_or(Error::Limit)?;
                }
            }
        }
        for (&key, edit) in &self.edits {
            length = length
                .checked_add(edit_len(key, edit)?)
                .ok_or(Error::Limit)?;
        }
        if length > self.limits.max_bytes
            || length
                .checked_add(self.retained()?)
                .is_none_or(|n| n > self.limits.max_builder_bytes)
        {
            return Err(Error::Limit);
        }
        let mut output = Vec::with_capacity(length);
        if let Some(base) = &self.base {
            for part in base.wire_parts() {
                for field in Reader::new(
                    part,
                    protobuf::Limits {
                        max_message_bytes: self.limits.max_bytes,
                        max_fields: self.limits.max_fields,
                        max_group_depth: protobuf::MAX_GROUP_DEPTH,
                    },
                )? {
                    let field = field?;
                    self.retained_field(field, only_edit)?.emit(&mut output)?;
                }
            }
        }
        for (&key, edit) in &self.edits {
            emit_edit(key, edit, &mut output)?;
        }
        Ok(output)
    }
}
fn varint_len(mut n: u64) -> usize {
    let mut len = 1;
    while n >= 128 {
        len += 1;
        n >>= 7;
    }
    len
}
fn unknown_packed_len(spec: Spec, value: Value<'_>) -> Result<Option<usize>, Error> {
    if spec.kind() != Kind::Enum || spec.open() || !spec.repeated() {
        return Ok(None);
    }
    let Value::Bytes(mut bytes) = value else {
        return Ok(None);
    };
    let mut length = 0usize;
    while !bytes.is_empty() {
        let (number, used) = protobuf::decode_varint(bytes)?;
        if !spec.enumeration.contains(&(number as i32)) {
            length = length.checked_add(used).ok_or(Error::Limit)?;
        }
        bytes = &bytes[used..];
    }
    Ok(Some(length))
}
enum RetainedField<'a> {
    Raw(&'a [u8]),
    Removed,
    Packed {
        spec: Spec,
        bytes: &'a [u8],
        unknown: usize,
    },
}

impl RetainedField<'_> {
    fn len(&self) -> Result<usize, Error> {
        match self {
            Self::Raw(bytes) => Ok(bytes.len()),
            Self::Removed => Ok(0),
            Self::Packed { spec, unknown, .. } => varint_len((u64::from(spec.number()) << 3) | 2)
                .checked_add(varint_len(*unknown as u64))
                .and_then(|n| n.checked_add(*unknown))
                .ok_or(Error::Limit),
        }
    }
    fn emit(self, output: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Raw(bytes) => output.extend_from_slice(bytes),
            Self::Removed => {}
            Self::Packed {
                spec,
                mut bytes,
                unknown,
            } => {
                protobuf::encode_varint((u64::from(spec.number()) << 3) | 2, output);
                protobuf::encode_varint(unknown as u64, output);
                while !bytes.is_empty() {
                    let (number, used) = protobuf::decode_varint(bytes)?;
                    if !spec.enumeration.contains(&(number as i32)) {
                        output.extend_from_slice(&bytes[..used]);
                    }
                    bytes = &bytes[used..];
                }
            }
        }
        Ok(())
    }
}
fn scalar_len(value: Value<'_>) -> usize {
    match value {
        Value::Varint(n) => varint_len(n),
        Value::Fixed64(_) => 8,
        Value::Fixed32(_) => 4,
        Value::Bytes(b) | Value::Group(b) => b.len(),
    }
}
fn value_len(value: &Encoded<'_>) -> Result<usize, Error> {
    match value {
        Encoded::Scalar(v) => Ok(scalar_len(*v)),
        Encoded::Message(v) => v.wire_len(),
    }
}
fn edit_len((number, wire): (u32, u8), edit: &Edit<'_>) -> Result<usize, Error> {
    let tag = varint_len((u64::from(number) << 3) | u64::from(wire));
    let values = match &edit.values {
        EditValues::BorrowedBytes(bytes) => {
            return tag
                .checked_add(varint_len(bytes.len() as u64))
                .and_then(|n| n.checked_add(bytes.len()))
                .ok_or(Error::Limit);
        }
        EditValues::SingleVarint(number) => {
            let body = varint_len(*number);
            return Ok(tag + body + usize::from(edit.packed));
        }
        EditValues::List(values) => values,
    };
    if edit.packed && !values.is_empty() {
        let body = values.iter().try_fold(0usize, |n, v| {
            n.checked_add(value_len(v)?).ok_or(Error::Limit)
        })?;
        return tag
            .checked_add(varint_len(body as u64))
            .and_then(|n| n.checked_add(body))
            .ok_or(Error::Limit);
    }
    values.iter().try_fold(0usize, |n, v| {
        let body = value_len(v)?;
        n.checked_add(tag)
            .and_then(|n| {
                n.checked_add(if wire == 2 {
                    varint_len(body as u64)
                } else if wire == 3 {
                    tag
                } else {
                    0
                })
            })
            .and_then(|n| n.checked_add(body))
            .ok_or(Error::Limit)
    })
}
fn emit_value(value: &Encoded<'_>, output: &mut Vec<u8>) {
    match value {
        Encoded::Scalar(v) => match v {
            Value::Varint(n) => protobuf::encode_varint(*n, output),
            Value::Fixed64(n) => output.extend_from_slice(&n.to_le_bytes()),
            Value::Fixed32(n) => output.extend_from_slice(&n.to_le_bytes()),
            Value::Bytes(b) | Value::Group(b) => output.extend_from_slice(b),
        },
        Encoded::Message(v) => {
            for part in v.wire_parts() {
                output.extend_from_slice(part);
            }
        }
    }
}
fn emit_edit(
    (number, wire): (u32, u8),
    edit: &Edit<'_>,
    output: &mut Vec<u8>,
) -> Result<(), Error> {
    let values = match &edit.values {
        EditValues::BorrowedBytes(bytes) => {
            protobuf::encode_varint((u64::from(number) << 3) | 2, output);
            protobuf::encode_varint(bytes.len() as u64, output);
            output.extend_from_slice(bytes);
            return Ok(());
        }
        EditValues::SingleVarint(value) => {
            let packed = edit.packed;
            protobuf::encode_varint(
                (u64::from(number) << 3) | if packed { 2 } else { 0 },
                output,
            );
            if packed {
                protobuf::encode_varint(varint_len(*value) as u64, output);
            }
            protobuf::encode_varint(*value, output);
            return Ok(());
        }
        EditValues::List(values) => values,
    };
    if edit.packed && !values.is_empty() {
        let body = values.iter().try_fold(0usize, |n, v| {
            n.checked_add(value_len(v)?).ok_or(Error::Limit)
        })?;
        protobuf::encode_varint((u64::from(number) << 3) | 2, output);
        protobuf::encode_varint(body as u64, output);
        for value in values {
            emit_value(value, output);
        }
        return Ok(());
    }
    for value in values {
        protobuf::encode_varint((u64::from(number) << 3) | u64::from(wire), output);
        if matches!(wire, 2) {
            protobuf::encode_varint(value_len(value)? as u64, output);
        }
        emit_value(value, output);
        if wire == 3 {
            protobuf::encode_varint((u64::from(number) << 3) | 4, output);
        }
    }
    Ok(())
}

#[doc(hidden)]
pub trait MessageType<'a>: Sized {
    type Reborrow<'b>: MessageType<'b>;
    fn schema() -> &'static Schema;
    fn from_view(view: View<'a>) -> Result<Self, Error>;
    fn view(&self) -> &View<'a>;
    fn expand_validation(&self, work: &mut ValidationWork<'a>) -> Result<(), Error>;
}
pub(crate) fn parse_parts<'a, M: MessageType<'a>>(
    parts: impl IntoIterator<Item = &'a [u8]>,
    limits: Limits,
) -> Result<M, Error> {
    parse_parts_admitted(parts, limits, |_| Ok(()))
}
pub(crate) fn parse_parts_admitted<'a, M: MessageType<'a>>(
    parts: impl IntoIterator<Item = &'a [u8]>,
    limits: Limits,
    admit: impl FnMut(usize) -> Result<(), Error>,
) -> Result<M, Error> {
    let mut input = parts.into_iter();
    let first = input.next().ok_or(Error::Type)?;
    let mut parts = Parts::one(first);
    for part in input {
        parts.push(part, limits)?;
    }
    M::from_view(View::parse_parts_admitted(
        parts,
        M::schema(),
        limits,
        0,
        true,
        admit,
    )?)
}
type CachedScalarRead<M, C> = fn(&M) -> Option<C>;

pub struct FieldToken<M, C> {
    spec: Spec,
    codec: C,
    read_cached: Option<CachedScalarRead<M, C>>,
    owner: PhantomData<fn(M) -> M>,
}
impl<M, C: Copy> Copy for FieldToken<M, C> {}
impl<M, C: Copy> Clone for FieldToken<M, C> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M, C> FieldToken<M, C> {
    const fn new(spec: Spec, codec: C) -> Self {
        Self {
            spec,
            codec,
            read_cached: None,
            owner: PhantomData,
        }
    }
    const fn with_cached_read(mut self, callback: CachedScalarRead<M, C>) -> Self {
        self.read_cached = Some(callback);
        self
    }
}
#[derive(Clone, Copy)]
pub struct ScalarField<T> {
    default: T,
}
fn cached_scalar_text<'a, M: MessageType<'a>>(owner: &M) -> Option<ScalarField<&'a str>> {
    let default = owner.view().single_text()?.text;
    Some(ScalarField { default })
}

pub struct MessageField<T>(PhantomData<fn() -> T>);
impl<T> Copy for MessageField<T> {}
impl<T> Clone for MessageField<T> {
    fn clone(&self) -> Self {
        *self
    }
}
pub struct ScalarList<T>(PhantomData<fn() -> T>);
impl<T> Copy for ScalarList<T> {}
impl<T> Clone for ScalarList<T> {
    fn clone(&self) -> Self {
        *self
    }
}
pub struct MessageList<T>(PhantomData<fn() -> T>);
impl<T> Copy for MessageList<T> {}
impl<T> Clone for MessageList<T> {
    fn clone(&self) -> Self {
        *self
    }
}
pub struct MessageBuilder<'a, M> {
    inner: Builder<'a>,
    owner: PhantomData<fn(M) -> M>,
}
impl<'a, M: MessageType<'a>> MessageBuilder<'a, M> {
    fn new(limits: Limits) -> Self {
        Self {
            inner: Builder::new(M::schema(), limits),
            owner: PhantomData,
        }
    }
    pub fn set<T: Scalar<'a>>(
        &mut self,
        field: FieldToken<M, ScalarField<T>>,
        value: T,
    ) -> Result<&mut Self, Error> {
        self.inner.scalar(field.spec, value, false)?;
        Ok(self)
    }
    pub fn push<T: Scalar<'a>>(
        &mut self,
        field: FieldToken<M, ScalarList<T>>,
        value: T,
    ) -> Result<&mut Self, Error> {
        self.inner.scalar(field.spec, value, true)?;
        Ok(self)
    }
    pub fn set_message<T: MessageType<'a>>(
        &mut self,
        field: FieldToken<M, MessageField<T>>,
        value: &T,
    ) -> Result<&mut Self, Error> {
        self.inner.message(field.spec, value.view(), false)?;
        Ok(self)
    }
    pub fn push_message<T: MessageType<'a>>(
        &mut self,
        field: FieldToken<M, MessageList<T>>,
        value: &T,
    ) -> Result<&mut Self, Error> {
        self.inner.message(field.spec, value.view(), true)?;
        Ok(self)
    }
    pub fn clear<C>(&mut self, field: FieldToken<M, C>) -> Result<&mut Self, Error> {
        self.inner.clear(field.spec)?;
        Ok(self)
    }
    pub fn finish(self) -> Result<Vec<u8>, Error> {
        self.inner.finish()
    }
    pub fn finish_checked(self) -> Result<Vec<u8>, Error> {
        let limits = self.inner.limits;
        let schema = self.inner.schema;
        let bytes = self.inner.encode()?;
        {
            let initial = View::parse(&bytes, schema, limits)?;
            let checked_schema = M::schema();
            let view = if core::ptr::eq(schema, checked_schema) {
                initial
            } else {
                drop(initial);
                View::parse(&bytes, checked_schema, limits)?
            };
            ValidationWork::run::<M::Reborrow<'_>>(&view, ValidationMode::Wire)?;
            ValidationWork::run::<M::Reborrow<'_>>(&view, ValidationMode::State)?;
        }
        Ok(bytes)
    }
}

pub struct Access<'a, M> {
    view: View<'a>,
    owner: PhantomData<fn(M) -> M>,
}
impl<M> Clone for Access<'_, M> {
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            owner: PhantomData,
        }
    }
}
impl<'a, M: MessageType<'a>> Access<'a, M> {
    /// Tests recognized wire presence without validating a nested message body.
    pub fn has<C>(&self, field: FieldToken<M, C>) -> bool {
        self.view.has(field.spec)
    }
    fn new(view: View<'a>) -> Self {
        Self {
            view,
            owner: PhantomData,
        }
    }
    pub fn to_builder(&self) -> MessageBuilder<'a, M> {
        MessageBuilder {
            inner: Builder::from_view(&self.view),
            owner: PhantomData,
        }
    }
    pub(crate) fn limits(&self) -> Limits {
        self.view.limits()
    }
    pub(crate) fn message_parts<T: MessageType<'a>>(
        &self,
        field: FieldToken<M, MessageField<T>>,
    ) -> impl Iterator<Item = &'a [u8]> + '_ {
        let since = self.view.since(field.spec);
        self.view.range(field.spec).filter_map(move |v| {
            if !since.is_some_and(|since| v.order >= since) || !field.spec.known(v.value()) {
                return None;
            }
            match v.value() {
                Value::Bytes(bytes) | Value::Group(bytes) => Some(bytes),
                _ => None,
            }
        })
    }
    pub(crate) fn get_message_admitted<T: MessageType<'a>>(
        &self,
        field: FieldToken<M, MessageField<T>>,
        admit: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Option<T>, Error> {
        self.view
            .child_admitted(field.spec, T::schema(), admit)?
            .map(T::from_view)
            .transpose()
    }
    pub fn wire_parts(&self) -> impl Iterator<Item = &'a [u8]> {
        self.view.wire_parts()
    }
    pub fn unknown_fields(&self) -> UnknownFields<'_, 'a> {
        self.view.unknown_fields()
    }
    pub fn field_count(&self) -> usize {
        self.view.field_count()
    }
    pub fn get<T: Scalar<'a>>(
        &self,
        field: FieldToken<M, ScalarField<T>>,
    ) -> Result<Option<T>, Error> {
        if let Some(read_cached) = field.read_cached
            && let Some(text) = self.view.single_text()
            && text.number.get() == field.spec.number()
            && let Some(value) = self.cached_scalar(text.text, read_cached)?
        {
            return Ok(Some(value));
        }
        self.view.scalar(field.spec)
    }
    #[inline(never)]
    fn cached_scalar<T>(
        &self,
        text: &'a str,
        read_cached: CachedScalarRead<M, ScalarField<T>>,
    ) -> Result<Option<T>, Error> {
        // Cached readers only need the validated text, so they can borrow it from an inline view.
        let owner = M::from_view(View {
            schema: self.view.schema,
            index: ViewBacking::Inline(InlineView {
                input: self.view.wire_fragments().0,
                data: InlineData::Text(text),
                charge: self.view.charge(),
            }),
        })?;
        Ok(read_cached(&owner).map(|value| value.default))
    }
    fn get_first_text(
        &self,
        field: FieldToken<M, ScalarField<&'a str>>,
    ) -> Result<Option<&'a str>, Error> {
        debug_assert_eq!(self.view.schema.fields[0].number(), field.spec.number());
        match &self.view.index {
            ViewBacking::Inline(InlineView {
                data: InlineData::Text(text),
                ..
            }) => {
                return Ok(Some(*text));
            }
            ViewBacking::Shared { index, .. } => {
                if let IndexTail::Text(text) | IndexTail::CompactText(text) = index.parts.tail {
                    return Ok(Some(text));
                }
            }
            ViewBacking::Inline(_) => {}
        }
        self.view.scalar(field.spec)
    }
    pub fn get_or_default<T: Scalar<'a> + Copy>(
        &self,
        field: FieldToken<M, ScalarField<T>>,
    ) -> Result<T, Error> {
        Ok(self.get(field)?.unwrap_or(field.codec.default))
    }
    pub fn get_message<T: MessageType<'a>>(
        &self,
        field: FieldToken<M, MessageField<T>>,
    ) -> Result<Option<T>, Error> {
        self.view
            .child(field.spec, T::schema())?
            .map(T::from_view)
            .transpose()
    }
    pub fn values<T: Scalar<'a>>(
        &self,
        field: FieldToken<M, ScalarList<T>>,
    ) -> Repeated<'_, 'a, T> {
        self.view.repeated(field.spec)
    }
    pub fn messages<T: MessageType<'a>>(
        &self,
        field: FieldToken<M, MessageList<T>>,
    ) -> impl Iterator<Item = Result<T, Error>> + '_ {
        self.view
            .children(field.spec, T::schema())
            .map(|v| v.and_then(T::from_view))
    }
    pub fn validate_deep(&self) -> Result<(), Error> {
        ValidationWork::run::<M>(&self.view, ValidationMode::Wire)?;
        ValidationWork::run::<M>(&self.view, ValidationMode::State)
    }
}

struct MapValue<V> {
    value: V,
    charge: usize,
}
pub struct Map<K, V> {
    entries: BTreeMap<K, MapValue<V>>,
}
impl<K: Ord, V> Map<K, V> {
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get<Q: Ord + ?Sized>(&self, key: &Q) -> Option<&V>
    where
        K: core::borrow::Borrow<Q>,
    {
        self.entries.get(key).map(|v| &v.value)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&K, &V)> {
        self.entries.iter().map(|(k, v)| (k, &v.value))
    }
}
impl<K, V> fmt::Debug for Map<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Map")
            .field("entries", &self.entries.len())
            .finish()
    }
}
impl<'a, M: MessageType<'a>> Access<'a, M> {
    fn map_values<T: MessageType<'a>, K: Ord + Copy, V>(
        &self,
        field: FieldToken<M, MessageList<T>>,
        mut key: impl FnMut(&T) -> Result<K, Error>,
        mut value: impl FnMut(T) -> Result<(V, usize), Error>,
    ) -> Result<Map<K, V>, Error> {
        let mut entries: BTreeMap<K, MapValue<V>> = BTreeMap::new();
        let base = self.view.charge();
        let overhead = core::mem::size_of::<K>()
            .checked_add(core::mem::size_of::<MapValue<V>>())
            .and_then(|n| n.checked_add(128))
            .ok_or(Error::Limit)?;
        let mut used = 0usize;
        for indexed in self.view.range(field.spec) {
            if !field.spec.known(indexed.value()) {
                continue;
            }
            let Value::Bytes(bytes) = indexed.value() else {
                return Err(Error::Type);
            };
            let mut limits = self.view.limits();
            limits.max_index_bytes = limits
                .max_index_bytes
                .checked_sub(base)
                .and_then(|n| n.checked_sub(used))
                .ok_or(Error::Limit)?;
            let mut raw = View::parse_parts(
                Parts::one(bytes),
                T::schema(),
                limits,
                self.view.depth().checked_add(1).ok_or(Error::Limit)?,
            )?;
            let entry_key = key(&T::from_view(raw.clone())?)?;
            if let Some(old) = entries.remove(&entry_key) {
                used = used.checked_sub(old.charge).ok_or(Error::Limit)?;
            }
            raw.limits_mut().max_index_bytes = self
                .view
                .limits_ref()
                .max_index_bytes
                .checked_sub(base)
                .and_then(|n| n.checked_sub(used))
                .and_then(|n| n.checked_sub(raw.charge()))
                .and_then(|n| n.checked_sub(overhead))
                .ok_or(Error::Limit)?;
            let (entry_value, retained) = value(T::from_view(raw)?)?;
            let charge = overhead.checked_add(retained).ok_or(Error::Limit)?;
            used = used
                .checked_add(charge)
                .filter(|n| {
                    base.checked_add(*n)
                        .is_some_and(|n| n <= self.view.limits().max_index_bytes)
                })
                .ok_or(Error::Limit)?;
            entries.insert(
                entry_key,
                MapValue {
                    value: entry_value,
                    charge,
                },
            );
        }
        Ok(Map { entries })
    }
}

#[derive(Clone, Copy)]
enum ValidationMode {
    Wire,
    State,
}
#[derive(Clone, Copy)]
struct ValidationType<'a> {
    schema: &'static Schema,
    visit: fn(View<'a>, &mut ValidationWork<'a>) -> Result<(), Error>,
}
impl<'a> ValidationType<'a> {
    fn of<M: MessageType<'a>>() -> Self {
        Self {
            schema: M::schema(),
            visit: visit::<M>,
        }
    }
}
struct ValidationTask<'a> {
    view: View<'a>,
    visit: fn(View<'a>, &mut ValidationWork<'a>) -> Result<(), Error>,
    charge: usize,
}
#[doc(hidden)]
pub struct ValidationWork<'a> {
    limits: Limits,
    mode: ValidationMode,
    fields: usize,
    values: usize,
    baseline: usize,
    queued: usize,
    current: usize,
    tasks: Vec<ValidationTask<'a>>,
}
impl<'a> ValidationWork<'a> {
    fn run<M: MessageType<'a>>(root: &View<'a>, mode: ValidationMode) -> Result<(), Error> {
        Self::run_type(root, mode, visit::<M>)
    }
    #[inline(never)]
    fn run_type(
        root: &View<'a>,
        mode: ValidationMode,
        root_visit: fn(View<'a>, &mut ValidationWork<'a>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut work = Self {
            limits: root.limits(),
            mode,
            fields: root.limits().max_fields,
            values: root.limits().max_values,
            baseline: root.charge(),
            queued: 0,
            current: 0,
            tasks: Vec::new(),
        };
        work.charge(root)?;
        if root
            .charge()
            .checked_add(4 * core::mem::size_of::<ValidationTask<'_>>())
            .is_none_or(|n| n > root.limits().max_index_bytes)
        {
            return Err(Error::Limit);
        }
        root_visit(root.clone(), &mut work)?;
        while let Some(task) = work.tasks.pop() {
            work.queued = work.queued.checked_sub(task.charge).ok_or(Error::Limit)?;
            work.current = task.charge;
            (task.visit)(task.view, &mut work)?;
            work.current = 0;
        }
        Ok(())
    }
    fn charge(&mut self, view: &View<'_>) -> Result<(), Error> {
        self.fields = self
            .fields
            .checked_sub(view.work_fields())
            .ok_or(Error::Limit)?;
        self.values = self.values.checked_sub(view.values()).ok_or(Error::Limit)?;
        Ok(())
    }
    fn available(&self) -> Result<usize, Error> {
        let slots = if self.tasks.len() < self.tasks.capacity() {
            self.tasks.capacity()
        } else {
            self.tasks
                .capacity()
                .checked_mul(2)
                .ok_or(Error::Limit)?
                .max(4)
        };
        self.limits
            .max_index_bytes
            .checked_sub(self.baseline)
            .and_then(|n| n.checked_sub(self.current))
            .and_then(|n| n.checked_sub(self.queued))
            .and_then(|n| {
                n.checked_sub(slots.checked_mul(core::mem::size_of::<ValidationTask<'_>>())?)
            })
            .ok_or(Error::Limit)
    }
    #[inline(never)]
    fn child(
        &mut self,
        parent: &View<'a>,
        parts: Parts<'a>,
        required: bool,
        child: ValidationType<'a>,
    ) -> Result<(), Error> {
        let mut limits = self.limits;
        limits.max_fields = self.fields;
        limits.max_values = self.values;
        limits.max_index_bytes = self.available()?;
        let view = View::parse_parts_required(
            parts,
            child.schema,
            limits,
            parent.depth().checked_add(1).ok_or(Error::Limit)?,
            required,
        )?;
        self.charge(&view)?;
        self.queued = self.queued.checked_add(view.charge()).ok_or(Error::Limit)?;
        self.tasks.push(ValidationTask {
            charge: view.charge(),
            view,
            visit: child.visit,
        });
        Ok(())
    }
    fn field<M: MessageType<'a>>(&mut self, parent: &View<'a>, spec: Spec) -> Result<(), Error> {
        self.field_type(parent, spec, ValidationType::of::<M>())
    }
    #[inline(never)]
    fn field_type(
        &mut self,
        parent: &View<'a>,
        spec: Spec,
        child: ValidationType<'a>,
    ) -> Result<(), Error> {
        if matches!(self.mode, ValidationMode::Wire) || spec.repeated() {
            for field in parent.range(spec) {
                if !spec.known(field.value()) {
                    continue;
                }
                let bytes = match field.value() {
                    Value::Bytes(bytes) | Value::Group(bytes) => bytes,
                    _ => return Err(Error::Type),
                };
                self.child(
                    parent,
                    Parts::one(bytes),
                    matches!(self.mode, ValidationMode::State),
                    child,
                )?;
            }
        } else if let Some(since) = parent.since(spec) {
            let mut parts = None;
            for field in parent.range(spec) {
                if field.order < since || !spec.known(field.value()) {
                    continue;
                }
                let bytes = match field.value() {
                    Value::Bytes(bytes) | Value::Group(bytes) => bytes,
                    _ => return Err(Error::Type),
                };
                match &mut parts {
                    None => parts = Some(Parts::one(bytes)),
                    Some(parts) => parts.push(bytes, self.limits)?,
                }
            }
            if let Some(parts) = parts {
                self.child(parent, parts, true, child)?;
            }
        }
        Ok(())
    }
}
fn visit<'a, M: MessageType<'a>>(
    view: View<'a>,
    work: &mut ValidationWork<'a>,
) -> Result<(), Error> {
    M::from_view(view)?.expand_validation(work)
}

enum UnknownEncoding<'a> {
    Whole(&'a [u8]),
    PackedVarint(&'a [u8]),
}
pub struct UnknownField<'a> {
    pub number: protobuf::FieldNumber,
    pub value: Value<'a>,
    encoding: UnknownEncoding<'a>,
}
impl UnknownField<'_> {
    pub fn raw(&self) -> Option<&[u8]> {
        match self.encoding {
            UnknownEncoding::Whole(bytes) => Some(bytes),
            UnknownEncoding::PackedVarint(_) => None,
        }
    }
    pub fn encoded_len(&self) -> usize {
        match self.encoding {
            UnknownEncoding::Whole(bytes) => bytes.len(),
            UnknownEncoding::PackedVarint(bytes) => {
                varint_len(u64::from(self.number.get()) << 3) + bytes.len()
            }
        }
    }
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, Error> {
        let length = self.encoded_len();
        if output.len() < length {
            return Err(Error::Limit);
        }
        match self.encoding {
            UnknownEncoding::Whole(bytes) => output[..length].copy_from_slice(bytes),
            UnknownEncoding::PackedVarint(bytes) => {
                let mut tag = u64::from(self.number.get()) << 3;
                let mut n = 0;
                while tag >= 128 {
                    output[n] = (tag as u8 & 127) | 128;
                    tag >>= 7;
                    n += 1;
                }
                output[n] = tag as u8;
                n += 1;
                output[n..length].copy_from_slice(bytes);
            }
        }
        Ok(length)
    }
}
impl fmt::Debug for UnknownField<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnknownField")
            .field("number", &self.number.get())
            .field("wire", &wire(self.value))
            .field("encoded_bytes", &self.encoded_len())
            .finish()
    }
}
pub struct UnknownFields<'v, 'a> {
    view: &'v View<'a>,
    part: usize,
    reader: Option<Reader<'a>>,
    packed: Option<(Spec, &'a [u8])>,
    failed: bool,
}
impl<'a> Iterator for UnknownFields<'_, 'a> {
    type Item = Result<UnknownField<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        loop {
            if let Some((spec, mut bytes)) = self.packed.take() {
                while !bytes.is_empty() {
                    let (number, used) = match protobuf::decode_varint(bytes) {
                        Ok(v) => v,
                        Err(e) => {
                            self.failed = true;
                            return Some(Err(e.into()));
                        }
                    };
                    let raw = &bytes[..used];
                    bytes = &bytes[used..];
                    if !spec.enumeration.contains(&(number as i32)) {
                        self.packed = Some((spec, bytes));
                        return Some(Ok(UnknownField {
                            number: protobuf::FieldNumber::new(spec.number())
                                .expect("generated schema field"),
                            value: Value::Varint(number),
                            encoding: UnknownEncoding::PackedVarint(raw),
                        }));
                    }
                }
            }
            let field = loop {
                if let Some(reader) = &mut self.reader
                    && let Some(field) = reader.next()
                {
                    break field;
                }
                let (first, rest) = self.view.wire_fragments();
                let bytes = if self.part == 0 {
                    Some(first)
                } else {
                    rest.get(self.part - 1).copied()
                };
                let bytes = bytes?;
                self.part += 1;
                self.reader = match Reader::new(
                    bytes,
                    protobuf::Limits {
                        max_message_bytes: self.view.limits().max_bytes,
                        max_fields: self.view.limits().max_fields,
                        max_group_depth: protobuf::MAX_GROUP_DEPTH,
                    },
                ) {
                    Ok(reader) => Some(reader),
                    Err(e) => {
                        self.failed = true;
                        return Some(Err(e.into()));
                    }
                };
            };
            let field = match field {
                Ok(field) => field,
                Err(e) => {
                    self.failed = true;
                    return Some(Err(e.into()));
                }
            };
            match self.view.schema.classify(field.number, field.value) {
                Err(e) => {
                    self.failed = true;
                    return Some(Err(e));
                }
                Ok(None) => {
                    return Some(Ok(UnknownField {
                        number: field.number,
                        value: field.value,
                        encoding: UnknownEncoding::Whole(field.raw),
                    }));
                }
                Ok(Some(spec)) => {
                    if spec.kind() == Kind::Enum
                        && !spec.open()
                        && spec.repeated()
                        && let Value::Bytes(bytes) = field.value
                    {
                        self.packed = Some((spec, bytes));
                    }
                }
            }
        }
    }
}
impl core::iter::FusedIterator for UnknownFields<'_, '_> {}

include!(concat!(env!("OUT_DIR"), "/generated-indexed-schema.rs"));

struct MediaRetryNotification;
struct MediaToken {
    spec: Spec,
}
impl MediaRetryNotification {
    const STANZA_ID: MediaToken = MediaToken { spec: MEDIA_STANZA };
    const DIRECT_PATH: MediaToken = MediaToken { spec: MEDIA_PATH };
    const RESULT: MediaToken = MediaToken { spec: MEDIA_RESULT };
    const SCHEMA: Schema = MEDIA_RETRY_SCHEMA;
}

mod media_retry_notification {
    use crate::protobuf::Enumeration as _;
    use crate::whatsapp::media_retry_notification::ResultType as ProtocolResult;

    #[derive(Clone, Copy)]
    pub struct ResultType(ProtocolResult);
    impl ResultType {
        pub const SUCCESS: Self = Self(ProtocolResult::SUCCESS);
        pub const fn number(self) -> i32 {
            self.0 as i32
        }
        pub fn from_number(value: i32) -> Option<Self> {
            ProtocolResult::from_i32(value).map(Self)
        }
    }
}

#[cfg(test)]
mod migration_tests {
    use super::*;
    use crate::protobuf::MessageView as _;

    #[test]
    fn indexes_known_and_unknown_fields_using_generated_schema() {
        let generated = SCHEMAS
            .iter()
            .find(|schema| schema.identity == MEDIA_RETRY_SCHEMA.identity)
            .unwrap();
        assert_eq!(
            generated.fields.len(),
            MediaRetryNotification::SCHEMA.fields.len()
        );
        assert_eq!(
            generated
                .fields
                .iter()
                .map(|field| field.number())
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
        let input = b"\x0a\x01a\x0a\x01b\x28\x05";
        let view = View::parse(input, &MediaRetryNotification::SCHEMA, Limits::default()).unwrap();
        let values: Vec<_> = view
            .range(MEDIA_STANZA)
            .map(|field| field.value())
            .collect();
        assert_eq!(values, vec![Value::Bytes(b"a"), Value::Bytes(b"b")]);
        assert_eq!(view.unknown_fields().count(), 1);
    }

    #[test]
    fn message_conversation_index_matches_generated_view() {
        let index = SCHEMA_NAMES
            .iter()
            .position(|name| *name == ".whatsapp.Message")
            .unwrap();
        let schema = &SCHEMAS[index];
        let conversation = *schema
            .fields
            .iter()
            .find(|field| field.number() == 1)
            .unwrap();
        for input in [
            &[][..],
            &b"\x0a\x01a"[..],
            &b"\x0a\x01a\x0a\x01b"[..],
            &b"\x0a\x02\xc3\xa1\x80\xb5\x18\x01"[..],
        ] {
            let indexed = View::parse(input, schema, Limits::default()).unwrap();
            let value = indexed
                .range(conversation)
                .last()
                .map(|field| match field.value() {
                    Value::Bytes(bytes) => core::str::from_utf8(bytes).unwrap(),
                    other => panic!("unexpected field value {other:?}"),
                });
            let reference = crate::whatsapp::MessageView::decode_view(input).unwrap();
            assert_eq!(value, reference.conversation);
        }
    }

    #[test]
    fn media_success_projection_agrees_with_generated_decoder() {
        let cases: &[&[u8]] = &[
            b"\x0a\x02id\x12\x05/path\x18\x01",
            b"\x12\x05/path\x0a\x02id\x18\x01",
            b"\x0a\x02id\xa2\x06\x01x\x12\x05/path\x18\x01",
            b"\x0a\x01\xff\x12\x05/path\x18\x01",
            b"\x0a\x02id\x0a\x03new\x12\x05/path\x18\x01",
            b"\x0a\x02id\x12\x05/path\x18\x02",
        ];
        let mut accepted = 0;
        for input in cases {
            if let Some((id, path)) = media_retry_success(input, Limits::default()) {
                accepted += 1;
                let reference =
                    crate::whatsapp::MediaRetryNotificationView::decode_view(input).unwrap();
                assert_eq!(reference.stanza_id, Some(id));
                assert_eq!(reference.direct_path, Some(path));
                assert_eq!(
                    reference.result,
                    Some(crate::whatsapp::media_retry_notification::ResultType::SUCCESS)
                );
            }
        }
        assert!(accepted >= 2);
    }

    #[test]
    fn media_projection_matches_on_text_and_length_boundaries() {
        use crate::protobuf::encoding::encode_varint;
        let strings = [
            String::new(),
            "a".into(),
            "á🙂".into(),
            "x".repeat(127),
            "x".repeat(128),
        ];
        for id in &strings {
            for path in &strings {
                let mut input = Vec::new();
                for (tag, value) in [(0x0a, id), (0x12, path)] {
                    input.push(tag);
                    encode_varint(value.len() as u64, &mut input);
                    input.extend_from_slice(value.as_bytes());
                }
                input.extend_from_slice(&[0x18, 1]);
                if let Some((actual_id, actual_path)) =
                    media_retry_success(&input, Limits::default())
                {
                    let reference =
                        crate::whatsapp::MediaRetryNotificationView::decode_view(&input).unwrap();
                    assert_eq!(reference.stanza_id, Some(actual_id));
                    assert_eq!(reference.direct_path, Some(actual_path));
                    assert_eq!(
                        reference.result,
                        Some(crate::whatsapp::media_retry_notification::ResultType::SUCCESS)
                    );
                }
            }
        }
    }

    #[test]
    fn media_projection_accepts_only_inputs_the_generated_view_accepts() {
        let original = b"\x0a\x02id\x12\x05/path\x18\x01";
        let mut state = 0xdad1_4e73_02c9_4c51u64;
        let mut accepted = 0;
        for iteration in 0..20_000 {
            let mut input = original.to_vec();
            for _ in 0..iteration % 4 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let position = state as usize % input.len();
                input[position] = (state >> 32) as u8;
            }
            if let Some((id, path)) = media_retry_success(&input, Limits::default()) {
                accepted += 1;
                let reference =
                    crate::whatsapp::MediaRetryNotificationView::decode_view(&input).unwrap();
                assert_eq!(reference.stanza_id, Some(id));
                assert_eq!(reference.direct_path, Some(path));
                assert_eq!(
                    reference.result,
                    Some(crate::whatsapp::media_retry_notification::ResultType::SUCCESS)
                );
            }
        }
        assert!(accepted > 5_000);
    }
}
