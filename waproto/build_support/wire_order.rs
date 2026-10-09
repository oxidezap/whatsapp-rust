//! Cold occurrence journal for messages with singular enums or oneofs.
//! Generated codecs keep their ordinary path when no unknown field was read.

use ::buffa::alloc::{borrow::Cow, boxed::Box, vec::Vec};
use ::buffa::bytes::Buf;
use ::buffa::{DecodeContext, DecodeError, EncodeSink, UnknownField, UnknownFields};
use ::core::mem::ManuallyDrop;

type GroupMap = fn(u32) -> u32;
type Projection = Vec<(u32, Vec<u8>)>;

// Enum-only owners need no per-type copy of protobuf sizing and writing code.
// The stack projection retains field presence, including present zero values.
#[cold]
#[inline(never)]
pub(crate) fn enum_snapshot(
    fields: &[(u32, Option<i32>)],
    ctx: Option<DecodeContext<'_>>,
) -> Result<Vec<u8>, DecodeError> {
    let len: usize = fields
        .iter()
        .filter_map(|(tag, value)| {
            value.map(|value| {
                ::buffa::encoding::varint_len(u64::from(*tag) << 3)
                    + ::buffa::types::int32_encoded_len(value) as usize
            })
        })
        .sum();
    if let Some(ctx) = ctx {
        ctx.register_element_memory(len)?;
    }
    let mut bytes = Vec::with_capacity(len);
    for &(tag, value) in fields {
        if let Some(value) = value {
            ::buffa::types::put_int32_field(tag, value, &mut bytes);
        }
    }
    Ok(bytes)
}

// The occurrence algorithm is shared through an erased adapter only after a
// future field activates the journal. Known-only decoding keeps its generated
// static codec; hundreds of message types need not repeat this cold algorithm.
// Erase a borrow rather than the owner. A vtable for the owner also retains its
// full destructor even though this runtime never owns or drops the message.
pub(crate) struct Adapter<'a, T>(pub(crate) &'a mut T);

pub(crate) trait OwnedCodec {
    fn storage(&mut self) -> &mut Storage;
    fn groups(&self) -> GroupMap;
    fn known(&self, ctx: DecodeContext<'_>) -> Result<Vec<u8>, DecodeError>;
    fn merge_slice(
        &mut self,
        tag: ::buffa::encoding::Tag,
        buf: &mut &[u8],
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError>;
}

#[cold]
#[inline(never)]
pub(crate) fn begin_owned(
    codec: &mut dyn OwnedCodec,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let map = codec.groups();
    codec.storage().begin(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn reconcile_owned(
    codec: &mut dyn OwnedCodec,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let map = codec.groups();
    codec.storage().reconcile(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn merge_owned_known(
    codec: &mut dyn OwnedCodec,
    tag: ::buffa::encoding::Tag,
    buf: &mut dyn Buf,
    ctx: DecodeContext<'_>,
    check_current: bool,
) -> Result<(), DecodeError> {
    let map = codec.groups();
    let group = map(tag.field_number());
    let previous = codec.storage().len();
    debug_assert_ne!(group, 0);
    if check_current {
        let known = codec.known(ctx)?;
        codec.storage().reconcile(&known, map, ctx)?;
    }
    let mut input = buf;
    let mut captured = Capture::new(&mut input, tag, ctx)?;
    ::buffa::encoding::skip_field_depth(tag, &mut captured, ctx.depth())?;
    let raw = captured.finish()?;
    let mut payload = raw.as_slice();
    ::buffa::encoding::Tag::decode(&mut payload)?;
    // Generated decoders receive their existing slice specialization. Erasing
    // their input buffer would instantiate a second recursive codec tree.
    codec.merge_slice(tag, &mut payload, ctx)?;
    let known = if group & (1 << 31) == 0 { codec.known(ctx)? } else { Vec::new() };
    codec
        .storage()
        .finish(group, Some(raw), &known, map, previous, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn finish_owned_unknown(
    codec: &mut dyn OwnedCodec,
    previous: usize,
    ctx: DecodeContext<'_>,
    check_current: bool,
) -> Result<(), DecodeError> {
    if codec.storage().len() != previous {
        let map = codec.groups();
        if check_current {
            let known = codec.known(ctx)?;
            codec.storage().reconcile(&known, map, ctx)?;
        }
        codec.storage().finish(0, None, &[], map, previous, ctx)?;
    }
    Ok(())
}

pub(crate) trait ViewCodec<'a> {
    fn storage(&mut self) -> &mut ViewStorage<'a>;
    fn groups(&self) -> GroupMap;
    fn known(&self, ctx: DecodeContext<'_>) -> Result<Vec<u8>, DecodeError>;
    fn merge(
        &mut self,
        tag: ::buffa::encoding::Tag,
        cur: &'a [u8],
        before: &'a [u8],
        ctx: DecodeContext<'_>,
    ) -> Result<&'a [u8], DecodeError>;
}

#[cold]
#[inline(never)]
pub(crate) fn begin_view<'a>(
    codec: &mut dyn ViewCodec<'a>,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let map = codec.groups();
    codec.storage().begin(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn reconcile_view<'a>(
    codec: &mut dyn ViewCodec<'a>,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let map = codec.groups();
    codec.storage().reconcile(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn merge_view<'a>(
    codec: &mut dyn ViewCodec<'a>,
    tag: ::buffa::encoding::Tag,
    cur: &'a [u8],
    before: &'a [u8],
    ctx: DecodeContext<'_>,
    check_current: bool,
) -> Result<&'a [u8], DecodeError> {
    let map = codec.groups();
    let group = map(tag.field_number());
    let previous = codec.storage().len();
    if group != 0 && check_current {
        let known = codec.known(ctx)?;
        codec.storage().reconcile(&known, map, ctx)?;
    }
    let rest = codec.merge(tag, cur, before, ctx)?;
    let count = codec.storage().len();
    if group != 0 || count != previous {
        let known = if (group != 0 && group & (1 << 31) == 0) || (group == 0 && check_current) {
            codec.known(ctx)?
        } else {
            Vec::new()
        };
        if group == 0 && check_current {
            codec.storage().reconcile(&known, map, ctx)?;
        }
        let raw = if group & (1 << 31) != 0 {
            let mut raw = Vec::new();
            let len = ::buffa::encoding::varint_len(u64::from(tag.field_number()) << 3) + cur.len() - rest.len();
            ctx.register_element_memory(len)?;
            raw.reserve_exact(len);
            tag.encode(&mut raw);
            raw.extend_from_slice(&cur[..cur.len() - rest.len()]);
            Cow::Owned(raw)
        } else {
            Cow::Borrowed(&before[..before.len() - rest.len()])
        };
        codec.storage().finish(
            group,
            raw,
            &known,
            map,
            previous,
            ctx,
        )?;
    }
    Ok(rest)
}

// A journal must not recursively rebuild its children in both encoding passes.
// Reserve codec-private entries in the public traversal cache for its prepared
// output. Parent and sibling codecs only see their own entries, in the same
// reserve/set/consume order. Three bytes per entry keep every value below
// MAX_MESSAGE_BYTES, including buffa's debug validation of cached values.
// This scratch storage belongs to one encode, never to the retained message.
#[cold]
#[inline(never)]
pub fn cache_output(bytes: &[u8], cache: &mut ::buffa::SizeCache) -> u32 {
    let len = ::buffa::saturate_size(bytes.len() as u64);
    if len > ::buffa::MAX_MESSAGE_BYTES {
        return len;
    }
    let slot = cache.reserve();
    cache.set(slot, len);
    for chunk in bytes.chunks(3) {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        let slot = cache.reserve();
        cache.set(slot, u32::from_le_bytes(word));
    }
    len
}

#[cold]
#[inline(never)]
pub fn write_cached(cache: &mut ::buffa::SizeCache, sink: &mut impl EncodeSink) {
    let mut remaining = cache.consume_next() as usize;
    let mut scratch = [0; 192];
    while remaining != 0 {
        let len = remaining.min(scratch.len());
        for chunk in scratch[..len].chunks_mut(3) {
            let word = cache.consume_next().to_le_bytes();
            chunk.copy_from_slice(&word[..chunk.len()]);
        }
        sink.put_slice(&scratch[..len]);
        remaining -= len;
    }
}

#[derive(Clone, PartialEq, Hash)]
enum Event<'a> {
    Known(u32, Cow<'a, [u8]>),
    Unknown(usize),
}

#[derive(Clone, Default, PartialEq, Hash)]
struct Order<'a> {
    events: Vec<Event<'a>>,
    baseline: Projection,
    forced: Vec<u32>,
    unknown_count: usize,
}

fn records(mut bytes: &[u8]) -> impl Iterator<Item = (u32, &[u8])> {
    ::core::iter::from_fn(move || {
        if bytes.is_empty() {
            return None;
        }
        let start = bytes;
        let tag = ::buffa::encoding::Tag::decode(&mut bytes).expect("generated field tag");
        ::buffa::encoding::skip_field_depth(tag, &mut bytes, u32::MAX)
            .expect("generated field payload");
        Some((tag.field_number(), &start[..start.len() - bytes.len()]))
    })
}

fn projection(known: &[u8], map: GroupMap) -> Projection {
    let mut result: Projection = Vec::new();
    for (tag, raw) in records(known) {
        let group = map(tag);
        if group == 0 {
            continue;
        }
        if let Some((_, bytes)) = result.iter_mut().find(|(id, _)| *id == group) {
            canonical_record(group, raw, |record| bytes.extend_from_slice(record));
        } else {
            let bytes = if group & (1 << 31) == 0 {
                raw.to_vec()
            } else {
                let mut bytes = Vec::new();
                canonical_record(group, raw, |record| bytes.extend_from_slice(record));
                bytes
            };
            result.push((group, bytes));
        }
    }
    result
}

// A repeated enum baseline uses canonical unpacked records regardless of its
// declared packing. Appending one decoded value then touches only that value;
// a growing packed length prefix never requires copying the preceding list.
#[inline]
fn canonical_record(group: u32, raw: &[u8], mut visit: impl FnMut(&[u8])) {
    if group & (1 << 31) == 0 {
        visit(raw);
        return;
    }
    canonical_repeated_record(raw, visit);
}

// Keep normalization out of the ordinary record path. Most retained groups
// are singular enums or oneofs and already have canonical generated bytes.
#[cold]
#[inline(never)]
fn canonical_repeated_record(mut raw: &[u8], mut visit: impl FnMut(&[u8])) {
    let tag = ::buffa::encoding::Tag::decode(&mut raw).expect("validated enum tag");
    if tag.wire_type() == ::buffa::encoding::WireType::LengthDelimited {
        raw = ::buffa::types::borrow_bytes(&mut raw).expect("validated packed enum");
    }
    while !raw.is_empty() {
        let value = ::buffa::encoding::decode_varint(&mut raw).expect("validated enum value") as i32;
        let mut bytes = [0; 15];
        let mut output = bytes.as_mut_slice();
        ::buffa::types::put_int32_field(tag.field_number(), value, &mut output);
        let len = 15 - output.len();
        visit(&bytes[..len]);
    }
}

fn canonical_len(group: u32, raw: &[u8]) -> usize {
    let mut len = 0;
    canonical_record(group, raw, |bytes| len += bytes.len());
    len
}

fn projection_charge(known: &[u8], map: GroupMap, copies: usize) -> usize {
    let mut groups = Vec::new();
    let mut bytes = 0usize;
    for (tag, raw) in records(known) {
        let group = map(tag);
        if group != 0 {
            bytes = bytes.saturating_add(canonical_len(group, raw).saturating_mul(copies));
            if !groups.contains(&group) {
                groups.push(group);
            }
        }
    }
    bytes.saturating_add(groups.len().saturating_mul(
        ::core::mem::size_of::<(u32, Vec<u8>)>() + ::core::mem::size_of::<Event<'_>>(),
    ))
}

fn event_charge(count: usize) -> usize {
    count.saturating_mul(::core::mem::size_of::<Event<'_>>())
}

fn value(values: &Projection, group: u32) -> &[u8] {
    values
        .iter()
        .find(|(id, _)| *id == group)
        .map_or(&[], |(_, bytes)| bytes)
}

impl<'a> Order<'a> {
    #[inline(always)]
    fn unchanged(&self, known: &[u8], map: GroupMap) -> bool {
        if !self.forced.is_empty() {
            return false;
        }
        let mut count = 0;
        for (tag, raw) in records(known) {
            let group = map(tag);
            if group != 0 {
                count += 1;
                if value(&self.baseline, group) != raw {
                    return self.unchanged_repeated(known, map);
                }
            }
        }
        count == self.baseline.len()
    }
    #[cold]
    fn unchanged_repeated(&self, known: &[u8], map: GroupMap) -> bool {
        if records(known).any(|(tag, _)| map(tag) != 0 && !self.baseline.iter().any(|(group, _)| *group == map(tag))) {
            return false;
        }
        self.baseline.iter().all(|(group, baseline)| {
            let mut offset = 0;
            for (_, raw) in records(known).filter(|(tag, _)| map(*tag) == *group) {
                let mut equal = true;
                canonical_record(*group, raw, |bytes| {
                    equal &= baseline.get(offset..offset + bytes.len()) == Some(bytes);
                    offset += bytes.len();
                });
                if !equal { return false; }
            }
            offset == baseline.len()
        })
    }
    fn begin(known: &[u8], map: GroupMap, count: usize) -> Self {
        let baseline = projection(known, map);
        let mut events = Vec::new();
        for (group, bytes) in &baseline {
            events.push(Event::Known(*group, Cow::Owned(bytes.clone())));
        }
        events.extend((0..count).map(Event::Unknown));
        Self {
            events,
            baseline,
            forced: Vec::new(),
            unknown_count: count,
        }
    }

    fn changed(&self, current: &Projection) -> Vec<u32> {
        let mut ids = self.forced.clone();
        for (group, _) in self.baseline.iter().chain(current.iter()) {
            if !ids.contains(group) && value(&self.baseline, *group) != value(current, *group) {
                ids.push(*group);
            }
        }
        ids
    }

    fn reconcile(
        &mut self,
        known: &[u8],
        map: GroupMap,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if self.unchanged(known, map) {
            return Ok(());
        }
        ctx.register_element_memory(projection_charge(known, map, 2))?;
        let current = projection(known, map);
        let changed = self.changed(&current);
        self.events
            .retain(|event| !matches!(event, Event::Known(group, _) if changed.contains(group)));
        for group in changed {
            let bytes = value(&current, group);
            if !bytes.is_empty() {
                self.events
                    .push(Event::Known(group, Cow::Owned(bytes.to_vec())));
            }
        }
        self.baseline = current;
        self.forced.clear();
        Ok(())
    }

    fn append_unknown(&mut self, count: usize) {
        self.events
            .extend((self.unknown_count..count).map(Event::Unknown));
        self.unknown_count = count;
    }

    fn append_repeated(&mut self, group: u32, raw: &[u8], ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        let index = self.baseline.iter().position(|(id, _)| *id == group);
        let header = if index.is_none() { ::core::mem::size_of::<(u32, Vec<u8>)>() } else { 0 };
        ctx.register_element_memory(header.saturating_add(canonical_len(group, raw)))?;
        let index = index.unwrap_or_else(|| {
            self.baseline.push((group, Vec::new()));
            self.baseline.len() - 1
        });
        canonical_record(group, raw, |bytes| self.baseline[index].1.extend_from_slice(bytes));
        Ok(())
    }

    fn write(&self, known: &[u8], unknown: &[u8], map: GroupMap) -> Vec<u8> {
        let (current, changed) = if self.unchanged(known, map) {
            (Vec::new(), Vec::new())
        } else {
            let current = projection(known, map);
            let changed = self.changed(&current);
            (current, changed)
        };
        // The current fields provide an initial size without traversing the
        // journal. Older retained occurrences can still grow the output.
        let mut result = Vec::with_capacity(known.len().saturating_add(unknown.len()));
        let mut unknown = records(unknown);
        for (tag, raw) in records(known) {
            if map(tag) == 0 {
                result.extend_from_slice(raw);
            }
        }
        for event in &self.events {
            match event {
                Event::Unknown(_) => {
                    result.extend_from_slice(unknown.next().expect("retained unknown occurrence").1)
                }
                Event::Known(group, bytes) if !changed.contains(group) => {
                    result.extend_from_slice(bytes)
                }
                Event::Known(_, _) => {}
            }
        }
        for group in changed {
            result.extend_from_slice(value(&current, group));
        }
        result
    }

    fn owned(&self) -> Order<'static> {
        Order {
            events: self
                .events
                .iter()
                .map(|event| match event {
                    Event::Unknown(index) => Event::Unknown(*index),
                    Event::Known(group, bytes) => Event::Known(*group, Cow::Owned(bytes.to_vec())),
                })
                .collect(),
            baseline: self.baseline.clone(),
            forced: self.forced.clone(),
            unknown_count: self.unknown_count,
        }
    }
}

#[derive(Clone, Default, PartialEq, Hash)]
struct State {
    fields: UnknownFields,
    order: Option<Order<'static>>,
}

/// Internal storage; raw unknown-field mutation deliberately drops the journal.
#[derive(Default)]
pub struct Storage(ManuallyDrop<Option<Box<State>>>);

impl Clone for Storage {
    #[inline]
    fn clone(&self) -> Self {
        if self.0.is_none() {
            Self::default()
        } else {
            clone_storage(self)
        }
    }
}
#[cold]
#[inline(never)]
fn clone_storage(storage: &Storage) -> Storage {
    Storage(storage.0.clone())
}

impl Drop for Storage {
    #[inline(never)]
    fn drop(&mut self) {
        // Share the empty check as well as destruction of the retained state.
        *self.0 = None;
    }
}

impl PartialEq for Storage {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        if self.0.is_none() && other.0.is_none() {
            return true;
        }
        equal_storage(self, other)
    }
}
#[cold]
#[inline(never)]
fn equal_storage(left: &Storage, right: &Storage) -> bool {
    **left == **right
        && left.0.as_ref().and_then(|state| state.order.as_ref())
            == right.0.as_ref().and_then(|state| state.order.as_ref())
}
impl ::core::hash::Hash for Storage {
    fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) {
        ::core::hash::Hash::hash(&**self, state);
        ::core::hash::Hash::hash(
            &self.0.as_ref().and_then(|state| state.order.as_ref()),
            state,
        );
    }
}

impl ::core::fmt::Debug for Storage {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        ::core::fmt::Debug::fmt(&**self, f)
    }
}
impl ::core::ops::Deref for Storage {
    type Target = UnknownFields;
    fn deref(&self) -> &Self::Target {
        static EMPTY: UnknownFields = UnknownFields::new();
        self.0.as_ref().map_or(&EMPTY, |state| &state.fields)
    }
}
impl ::core::ops::DerefMut for Storage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let state = self.0.get_or_insert_with(Box::default);
        state.order = None;
        &mut state.fields
    }
}
impl From<UnknownFields> for Storage {
    fn from(fields: UnknownFields) -> Self {
        if fields.is_empty() {
            Self::default()
        } else {
            Self(ManuallyDrop::new(Some(Box::new(State {
                fields,
                order: None,
            }))))
        }
    }
}
impl From<Storage> for UnknownFields {
    fn from(mut storage: Storage) -> Self {
        storage
            .0
            .take()
            .map_or_else(UnknownFields::new, |state| state.fields)
    }
}
impl PartialEq<UnknownFields> for Storage {
    fn eq(&self, other: &UnknownFields) -> bool {
        &**self == other
    }
}
impl PartialEq<Storage> for UnknownFields {
    fn eq(&self, other: &Storage) -> bool {
        self == &**other
    }
}
impl<'a> IntoIterator for &'a Storage {
    type Item = &'a UnknownField;
    type IntoIter = ::core::slice::Iter<'a, UnknownField>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl IntoIterator for Storage {
    type Item = UnknownField;
    type IntoIter = <UnknownFields as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        UnknownFields::from(self).into_iter()
    }
}
impl Storage {
    #[inline]
    pub fn active(&self) -> bool {
        self.0.as_ref().is_some_and(|state| state.order.is_some())
    }
    #[cold]
    pub(super) fn push_decoded(
        &mut self,
        field: UnknownField,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if self.0.is_none() {
            ctx.register_element_memory(::core::mem::size_of::<State>())?;
        }
        self.0.get_or_insert_with(Box::default).fields.push(field);
        Ok(())
    }
    #[cold]
    pub fn merge_unknown(
        &mut self,
        tag: ::buffa::encoding::Tag,
        buf: &mut impl Buf,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        self.push_decoded(::buffa::encoding::decode_unknown_field(tag, buf, ctx)?, ctx)
    }
    #[cold]
    #[inline(never)]
    pub fn begin(
        &mut self,
        known: &[u8],
        map: GroupMap,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        ctx.register_element_memory(
            projection_charge(known, map, 2).saturating_add(event_charge(self.len())),
        )?;
        let state = self.0.get_or_insert_with(Box::default);
        state.order = Some(Order::begin(known, map, state.fields.len()));
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn reconcile(
        &mut self,
        known: &[u8],
        map: GroupMap,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            order.reconcile(known, map, ctx)?;
        }
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn finish(
        &mut self,
        group: u32,
        raw: Option<Vec<u8>>,
        known: &[u8],
        map: GroupMap,
        previous: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        let state = self.0.as_mut().expect("decoded unknown storage");
        let order = state.order.as_mut().expect("started occurrence journal");
        if group != 0 && previous == state.fields.len() {
            ctx.register_element_memory(
                projection_charge(known, map, 1).saturating_add(event_charge(1)),
            )?;
            let raw = raw.expect("captured known occurrence");
            if group & (1 << 31) != 0 {
                order.append_repeated(group, &raw, ctx)?;
            } else {
                order.baseline = projection(known, map);
            }
            order.events.push(Event::Known(group, Cow::Owned(raw)));
        }
        ctx.register_element_memory(event_charge(state.fields.len() - order.unknown_count))?;
        order.append_unknown(state.fields.len());
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn compose(&self, known: &[u8], map: GroupMap) -> Vec<u8> {
        let state = self.0.as_ref().expect("active occurrence journal");
        let mut unknown = Vec::new();
        state.fields.write_to(&mut unknown);
        state
            .order
            .as_ref()
            .expect("active occurrence journal")
            .write(known, &unknown, map)
    }
    pub fn force(&mut self, group: u32) {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut())
            && !order.forced.contains(&group)
        {
            order.forced.push(group);
        }
    }
    pub fn clear(&mut self) {
        *self.0 = None;
    }
    /// Rebase only representation differences during view conversion; retain
    /// forced or value-observable edits instead of blessing them as received.
    pub fn rebase(&mut self, view_known: &[u8], owned_known: &[u8], map: GroupMap) {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            order.forced = order.changed(&projection(view_known, map));
            order.baseline = projection(owned_known, map);
        }
    }
}

#[derive(Clone, Default)]
struct ViewState<'a> {
    fields: ::buffa::UnknownFieldsView<'a>,
    count: usize,
    order: Option<Order<'a>>,
}
#[derive(Default)]
pub struct ViewStorage<'a>(Option<Box<ViewState<'a>>>);
impl Clone for ViewStorage<'_> {
    #[inline]
    fn clone(&self) -> Self {
        if self.0.is_none() {
            Self::default()
        } else {
            clone_view_storage(self)
        }
    }
}
#[cold]
#[inline(never)]
fn clone_view_storage<'a>(storage: &ViewStorage<'a>) -> ViewStorage<'a> {
    ViewStorage(storage.0.clone())
}
impl ::core::fmt::Debug for ViewStorage<'_> {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("ViewStorage")
            .field("unknown_fields", &self.0.as_ref().map(|s| &s.fields))
            .finish()
    }
}
impl<'a> ViewStorage<'a> {
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|state| state.fields.is_empty())
    }
    pub fn len(&self) -> usize {
        self.0.as_ref().map_or(0, |state| state.count)
    }
    pub fn active(&self) -> bool {
        self.0.as_ref().is_some_and(|state| state.order.is_some())
    }
    pub fn force(&mut self, group: u32) {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut())
            && !order.forced.contains(&group)
        {
            order.forced.push(group);
        }
    }
    pub fn encoded_len(&self) -> usize {
        self.0
            .as_ref()
            .map_or(0, |state| state.fields.encoded_len())
    }
    pub fn write_to(&self, buf: &mut impl EncodeSink) {
        if let Some(state) = &self.0 {
            state.fields.write_to(buf);
        }
    }
    pub fn push_record(
        &mut self,
        tail: &'a [u8],
        len: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(state) = &mut self.0 {
            state.order = None;
        }
        self.push_decoded_record(tail, len, ctx)
    }
    pub(super) fn push_decoded_record(
        &mut self,
        tail: &'a [u8],
        len: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if self.0.is_none() {
            ctx.register_element_memory(::core::mem::size_of::<ViewState<'_>>())?;
        }
        let state = self.0.get_or_insert_with(Box::default);
        state.fields.push_record(tail, len, ctx)?;
        state.count += 1;
        Ok(())
    }
    pub fn push_varint(
        &mut self,
        field: u32,
        value: u64,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(state) = &mut self.0 {
            state.order = None;
        }
        self.push_decoded_varint(field, value, ctx)
    }
    pub(super) fn push_decoded_varint(
        &mut self,
        field: u32,
        value: u64,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if self.0.is_none() {
            ctx.register_element_memory(::core::mem::size_of::<ViewState<'_>>())?;
        }
        let state = self.0.get_or_insert_with(Box::default);
        state.fields.push_varint(field, value, ctx)?;
        state.count += 1;
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn to_owned(&self) -> Result<Storage, DecodeError> {
        self.0.as_ref().map_or_else(
            || Ok(Storage::default()),
            |state| {
                Ok(Storage(ManuallyDrop::new(Some(Box::new(State {
                    fields: state.fields.to_owned()?,
                    order: state.order.as_ref().map(Order::owned),
                })))))
            },
        )
    }
    #[cold]
    #[inline(never)]
    pub fn begin(
        &mut self,
        known: &[u8],
        map: GroupMap,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        ctx.register_element_memory(
            projection_charge(known, map, 2).saturating_add(event_charge(self.len())),
        )?;
        let state = self.0.get_or_insert_with(Box::default);
        state.order = Some(Order::begin(known, map, state.count));
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn reconcile(
        &mut self,
        known: &[u8],
        map: GroupMap,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            order.reconcile(known, map, ctx)?;
        }
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn finish(
        &mut self,
        group: u32,
        raw: Cow<'a, [u8]>,
        known: &[u8],
        map: GroupMap,
        previous: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        let state = self.0.as_mut().expect("decoded unknown storage");
        let order = state.order.as_mut().expect("started occurrence journal");
        if group != 0 && previous == state.count {
            // Borrowed occurrences need an eventual owned copy. Synthetic
            // repeated-enum records were already charged when materialized.
            let owned_copy = if matches!(&raw, Cow::Borrowed(_)) { raw.len() } else { 0 };
            ctx.register_element_memory(
                projection_charge(known, map, 1)
                    .saturating_add(owned_copy)
                    .saturating_add(event_charge(1)),
            )?;
            if group & (1 << 31) != 0 {
                order.append_repeated(group, &raw, ctx)?;
            } else {
                order.baseline = projection(known, map);
            }
            order.events.push(Event::Known(group, raw));
        }
        ctx.register_element_memory(event_charge(state.count - order.unknown_count))?;
        order.append_unknown(state.count);
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn compose(&self, known: &[u8], map: GroupMap) -> Vec<u8> {
        let state = self.0.as_ref().expect("active occurrence journal");
        let mut unknown = Vec::new();
        state.fields.write_to(&mut unknown);
        state
            .order
            .as_ref()
            .expect("active occurrence journal")
            .write(known, &unknown, map)
    }
}

/// Records only fields read after a message's first unknown occurrence.
pub struct Capture<'a, 'c> {
    inner: &'a mut dyn Buf,
    bytes: Vec<u8>,
    ctx: DecodeContext<'c>,
    exhausted: bool,
}
impl<'a, 'c> Capture<'a, 'c> {
    pub fn new(
        inner: &'a mut impl Buf,
        tag: ::buffa::encoding::Tag,
        ctx: DecodeContext<'c>,
    ) -> Result<Self, DecodeError> {
        ctx.register_element_memory(5)?;
        let mut bytes = Vec::new();
        tag.encode(&mut bytes);
        Ok(Self {
            inner,
            bytes,
            ctx,
            exhausted: false,
        })
    }
    pub fn finish(self) -> Result<Vec<u8>, DecodeError> {
        if self.exhausted {
            Err(DecodeError::ElementMemoryLimitExceeded)
        } else {
            Ok(self.bytes)
        }
    }
}
impl Buf for Capture<'_, '_> {
    fn remaining(&self) -> usize {
        self.inner.remaining()
    }
    fn chunk(&self) -> &[u8] {
        self.inner.chunk()
    }
    fn advance(&mut self, mut count: usize) {
        assert!(
            count <= self.inner.remaining(),
            "captured buffer advance exceeds input"
        );
        if self.ctx.register_element_memory(count).is_err() {
            self.exhausted = true;
        }
        while count > 0 {
            let chunk = self.inner.chunk();
            let len = count.min(chunk.len());
            if !self.exhausted {
                self.bytes.extend_from_slice(&chunk[..len]);
            }
            self.inner.advance(len);
            count -= len;
        }
    }
}
