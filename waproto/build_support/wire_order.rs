//! Cold occurrence journal for messages with singular enums or oneofs.
//! Generated codecs keep their ordinary path when no unknown field was read.

use ::buffa::alloc::{borrow::Cow, boxed::Box, vec::Vec};
use ::buffa::bytes::Buf;
use ::buffa::{DecodeContext, DecodeError, EncodeSink, UnknownField, UnknownFields};
use ::core::mem::ManuallyDrop;

type GroupMap = fn(u32) -> u32;
type Projection = Vec<(u32, Vec<u8>)>;
pub(crate) type Replay = fn(&[u8], Option<DecodeContext<'_>>) -> Result<Vec<u8>, DecodeError>;

// These immutable callbacks are fetched together on the cold journal path.
// One adapter method avoids three per-owner vtable slots and return thunks.
pub(crate) struct Policy {
    pub(crate) groups: GroupMap,
    pub(crate) growth: fn(u32) -> usize,
    pub(crate) replay: Replay,
    pub(crate) canonical: fn(&[u8]) -> bool,
}

pub(crate) fn no_canonical_projection(_: &[u8]) -> bool { false }

// A field decoder can mutate its receiver before journal insertion. Prepay
// insertion as well as finalization so an exhausted caller budget cannot erase
// a completed known occurrence behind an earlier future value.
fn reserve_record(group: u32, raw: &[u8], view: bool, ctx: DecodeContext<'_>) -> Result<usize, DecodeError> {
    let mut charge = event_charge(1).saturating_add(if view { raw.len() } else { 0 });
    if group & (1 << 31) != 0 {
        // Packed input is split into individual values before this entrypoint.
        // A synthetic view record has no tag in `raw`; allow its five bytes,
        // plus a canonical tag and ten-byte signed enum in the projection.
        charge = charge.saturating_add(if view { 5 } else { 0 })
            .saturating_add(::core::mem::size_of::<(u32, Vec<u8>)>())
            .saturating_add(15);
    }
    ctx.register_element_memory(charge)?;
    Ok(charge)
}

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

// Every occurrence in this replay was already accepted as a declared enum.
// Keep the last value per field, including present zero and signed values,
// without constructing an owner and retaining its unrelated codec tree.
// begin seeds nonoptional defaults as known records, so they are present here
// even when the corresponding field was absent from the received message.
#[cold]
#[inline(never)]
pub(crate) fn enum_replay(raw: &[u8], fields: &[u32], ctx: Option<DecodeContext<'_>>) -> Result<Vec<u8>, DecodeError> {
    if let Some(ctx) = ctx {
        ctx.register_element_memory(fields.len().saturating_mul(::core::mem::size_of::<(u32, Option<i32>)>()))?;
    }
    let values: Vec<_> = fields.iter().map(|&field| {
        let value = records(raw).filter(|(tag, _)| *tag == field).last().map(|(_, mut bytes)| {
            ::buffa::encoding::Tag::decode(&mut bytes).expect("completed enum tag");
            ::buffa::encoding::decode_varint(&mut bytes).expect("completed enum value") as i32
        });
        (field, value)
    }).collect();
    enum_snapshot(&values, ctx)
}

// The occurrence algorithm is shared through an erased adapter only after a
// future field activates the journal. Known-only decoding keeps its generated
// static codec; hundreds of message types need not repeat this cold algorithm.
// Erase a borrow rather than the owner. A vtable for the owner also retains its
// full destructor even though this runtime never owns or drops the message.
pub(crate) struct Adapter<'a, T>(pub(crate) &'a mut T);

pub(crate) trait OwnedCodec {
    fn storage(&mut self) -> &mut Storage;
    fn policy(&self) -> Policy;
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
    let policy = codec.policy();
    let map = policy.groups;
    codec.storage().begin(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn reconcile_owned(
    codec: &mut dyn OwnedCodec,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let policy = codec.policy();
    let map = policy.groups;
    let replay = policy.replay;
    codec.storage().reconcile(&known, map, replay, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn complete_owned_batch(codec: &mut dyn OwnedCodec, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
    let reserved = ::core::cell::Cell::new(codec.storage().take_completion_credit());
    if !codec.storage().needs_baseline() { return Ok(()); }
    let ctx = ctx.with_element_memory(&reserved);
    let policy = codec.policy();
    let known = match codec.storage().completed_canonical(policy.canonical, ctx)? {
        Some(known) => known,
        None => codec.known(ctx)?,
    };
    codec.storage().complete_batch(known, policy.groups, ctx)
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
    let policy = codec.policy();
    let map = policy.groups;
    let group = map(tag.field_number());
    let previous = codec.storage().len();
    debug_assert_ne!(group, 0);
    if check_current {
        let known = codec.known(ctx)?;
        let replay = policy.replay;
        codec.storage().reconcile(&known, map, replay, ctx)?;
    }
    let raw = capture_known_field(buf, tag, ctx)?;
    let mut payload = raw.as_slice();
    ::buffa::encoding::Tag::decode(&mut payload)?;
    let record_credit = ::core::cell::Cell::new(if !check_current {
        reserve_record(group, &raw, false, ctx)?
    } else { 0 });
    if !check_current {
        let encoded_bound = raw.len().saturating_mul((policy.growth)(tag.field_number()));
        codec.storage().reserve_baseline(group, encoded_bound, ctx)?;
    }
    // Generated decoders receive their existing slice specialization. Erasing
    // their input buffer would instantiate a second recursive codec tree.
    codec.merge_slice(tag, &mut payload, ctx)?;
    let finish_ctx = if check_current { ctx } else { ctx.with_element_memory(&record_credit) };
    let known = if check_current && group & (1 << 31) == 0 { Some(codec.known(ctx)?) } else { None };
    codec
        .storage()
        .finish(group, Some(raw), known.as_deref(), map, previous, finish_ctx)
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
        let policy = codec.policy();
        let map = policy.groups;
        if check_current {
            let known = codec.known(ctx)?;
            let replay = policy.replay;
            codec.storage().reconcile(&known, map, replay, ctx)?;
        }
        codec.storage().finish(0, None, None, map, previous, ctx)?;
    }
    Ok(())
}

pub(crate) trait ViewCodec<'a> {
    fn storage(&mut self) -> &mut ViewStorage<'a>;
    fn policy(&self) -> Policy;
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
    let policy = codec.policy();
    let map = policy.groups;
    codec.storage().begin(&known, map, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn reconcile_view<'a>(
    codec: &mut dyn ViewCodec<'a>,
    ctx: DecodeContext<'_>,
) -> Result<(), DecodeError> {
    let known = codec.known(ctx)?;
    let policy = codec.policy();
    let map = policy.groups;
    let replay = policy.replay;
    codec.storage().reconcile(&known, map, replay, ctx)
}

#[cold]
#[inline(never)]
pub(crate) fn complete_view_batch<'a>(codec: &mut dyn ViewCodec<'a>, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
    let reserved = ::core::cell::Cell::new(codec.storage().take_completion_credit());
    if !codec.storage().needs_baseline() { return Ok(()); }
    let ctx = ctx.with_element_memory(&reserved);
    let policy = codec.policy();
    let known = match codec.storage().completed_canonical(policy.canonical, ctx)? {
        Some(known) => known,
        None => codec.known(ctx)?,
    };
    codec.storage().complete_batch(known, policy.groups, ctx)
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
    let policy = codec.policy();
    let map = policy.groups;
    let group = map(tag.field_number());
    let previous = codec.storage().len();
    if group != 0 && check_current {
        let known = codec.known(ctx)?;
        let replay = policy.replay;
        codec.storage().reconcile(&known, map, replay, ctx)?;
    }
    let record_credit = ::core::cell::Cell::new(if group != 0 && !check_current {
        let mut after = cur;
        ::buffa::encoding::skip_field_depth(tag, &mut after, ctx.depth())?;
        let raw = &before[..before.len() - after.len()];
        let record_len = raw.len().saturating_add(if group & (1 << 31) != 0 {
            ::buffa::encoding::varint_len(u64::from(tag.field_number()) << 3)
        } else { 0 });
        let encoded_bound = record_len.saturating_mul((policy.growth)(tag.field_number()));
        codec.storage().reserve_baseline(group, encoded_bound, ctx)?;
        reserve_record(group, raw, true, ctx)?
    } else { 0 });
    let rest = codec.merge(tag, cur, before, ctx)?;
    let finish_ctx = if group != 0 && !check_current { ctx.with_element_memory(&record_credit) } else { ctx };
    let count = codec.storage().len();
    if group != 0 || count != previous {
        let known = if check_current && group & (1 << 31) == 0 {
            Some(codec.known(ctx)?)
        } else {
            None
        };
        if group == 0 && check_current {
            let replay = policy.replay;
            codec.storage().reconcile(known.as_deref().unwrap_or_default(), map, replay, ctx)?;
        }
        let raw = if group & (1 << 31) != 0 {
            let mut raw = Vec::new();
            let len = ::buffa::encoding::varint_len(u64::from(tag.field_number()) << 3) + cur.len() - rest.len();
            finish_ctx.register_element_memory(len)?;
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
            known.as_deref(),
            map,
            previous,
            finish_ctx,
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
    baseline_pending: bool,
    completion_credit: usize,
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
        let index = result.iter().position(|(id, _)| *id == group).unwrap_or_else(|| {
            result.push((group, Vec::new()));
            result.len() - 1
        });
        canonical_record(group, raw, |bytes| result[index].1.extend_from_slice(bytes));
    }
    result
}

fn owned_projection(known: Vec<u8>, map: GroupMap) -> Projection {
    let singular_group = {
        let mut fields = records(&known);
        fields.next().and_then(|(tag, _)| {
            let group = map(tag);
            (group != 0 && group & (1 << 31) == 0 && fields.next().is_none())
                .then_some(group)
        })
    };
    if let Some(group) = singular_group {
        // A single singular record is already canonical. Keep the completed
        // snapshot allocation instead of copying its entire nested payload.
        return ::buffa::alloc::vec![(group, known)];
    }
    projection(&known, map)
}

// A repeated enum baseline uses canonical unpacked records regardless of its
// declared packing. Appending one decoded value then touches only that value;
// a growing packed length prefix never requires copying the preceding list.
fn canonical_record(group: u32, mut raw: &[u8], mut visit: impl FnMut(&[u8])) {
    if group & (1 << 31) == 0 {
        visit(raw);
        return;
    }
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
    fn completed_canonical(&self, validate: fn(&[u8]) -> bool, ctx: DecodeContext<'_>) -> Result<Option<Vec<u8>>, DecodeError> {
        if !self.baseline_pending || !self.baseline.is_empty() || !self.forced.is_empty() {
            return Ok(None);
        }
        let mut known = self.events.iter().filter_map(|event| match event {
            Event::Known(_, raw) => Some(raw.as_ref()),
            Event::Unknown(_) => None,
        });
        let Some(raw) = known.next() else { return Ok(None); };
        if known.next().is_some() || !validate(raw) { return Ok(None); }
        // The successful batch held an exclusive receiver borrow. A single
        // canonical occurrence starting from an empty projection is its exact
        // final snapshot. Retain a concrete baseline for later public edits,
        // with the same pre-allocation debit as the original snapshot path.
        ctx.register_element_memory(raw.len())?;
        Ok(Some(raw.to_vec()))
    }

    // A failed nested decoder can mutate after the last completed record.
    // Reconstruct the expected pre-failure value from completed occurrences;
    // snapshotting the receiver would bless that unrecorded partial mutation.
    fn expected(&self, map: GroupMap, replay: Replay, ctx: Option<DecodeContext<'_>>) -> Result<Projection, DecodeError> {
        let len = self.events.iter().fold(0usize, |len, event| match event {
            Event::Known(_, bytes) => len.saturating_add(bytes.len()),
            Event::Unknown(_) => len,
        });
        if let Some(ctx) = ctx { ctx.register_element_memory(len)?; }
        let mut raw = Vec::with_capacity(len);
        for event in &self.events {
            if let Event::Known(_, bytes) = event { raw.extend_from_slice(bytes); }
        }
        let known = replay(&raw, ctx)?;
        if let Some(ctx) = ctx { ctx.register_element_memory(projection_charge(&known, map, 1))?; }
        Ok(owned_projection(known, map))
    }
    fn materialize(&mut self, map: GroupMap, replay: Replay, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        if self.baseline_pending {
            let expected = self.expected(map, replay, Some(ctx))?;
            self.baseline = expected;
            self.baseline_pending = false;
        }
        Ok(())
    }

    fn reserve_baseline(&mut self, group: u32, encoded_bound: usize, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        if group & (1 << 31) != 0 && !self.baseline_pending {
            return Ok(());
        }
        // Reserve before mutation: completed occurrences must remain encodable
        // even if a later decode exhausts the caller's budget. A canonical
        // field's schema bounds packed and signed-integer growth; two copies cover the
        // temporary encoding and the final projection. Existing values are
        // charged once per batch, not once per fragment.
        let header = ::core::mem::size_of::<(u32, Vec<u8>)>() + ::core::mem::size_of::<Event<'_>>();
        let initial = if self.completion_credit == 0 {
            self.baseline.iter().fold(0usize, |total, (_, bytes)| {
                total.saturating_add(bytes.len().saturating_add(20).saturating_mul(2)).saturating_add(header)
            })
        } else { 0 };
        let new_group = !self.baseline.iter().any(|(id, _)| *id == group)
            && !self.events.iter().any(|event| matches!(event, Event::Known(id, _) if *id == group));
        let charge = initial.saturating_add(encoded_bound.saturating_mul(2))
            .saturating_add(if new_group { header } else { 0 });
        ctx.register_element_memory(charge)?;
        self.completion_credit = self.completion_credit.saturating_add(charge);
        Ok(())
    }
    fn unchanged(&self, known: &[u8], map: GroupMap) -> bool {
        self.unchanged_against(&self.baseline, known, map)
    }
    fn unchanged_against(&self, baseline: &Projection, known: &[u8], map: GroupMap) -> bool {
        if !self.forced.is_empty() {
            return false;
        }
        let mut count = 0;
        for (tag, raw) in records(known) {
            let group = map(tag);
            if group != 0 {
                if group & (1 << 31) != 0 { return self.unchanged_repeated(baseline, known, map); }
                count += 1;
                if value(baseline, group) != raw {
                    return self.unchanged_repeated(baseline, known, map);
                }
            }
        }
        count == baseline.len()
    }
    #[cold]
    fn unchanged_repeated(&self, baseline: &Projection, known: &[u8], map: GroupMap) -> bool {
        if records(known).any(|(tag, _)| map(tag) != 0 && !baseline.iter().any(|(group, _)| *group == map(tag))) {
            return false;
        }
        baseline.iter().all(|(group, baseline)| {
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
            baseline_pending: false,
            completion_credit: 0,
        }
    }

    fn changed(&self, current: &Projection) -> Vec<u32> {
        self.changed_against(&self.baseline, current)
    }
    fn changed_against(&self, baseline: &Projection, current: &Projection) -> Vec<u32> {
        let mut ids = self.forced.clone();
        for (group, _) in baseline.iter().chain(current.iter()) {
            if !ids.contains(group) && value(baseline, *group) != value(current, *group) {
                ids.push(*group);
            }
        }
        ids
    }

    fn reconcile(
        &mut self,
        known: &[u8],
        map: GroupMap,
        replay: Replay,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        self.materialize(map, replay, ctx)?;
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
        self.baseline_pending = false;
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

    fn write(&self, known: &[u8], unknown: &[u8], map: GroupMap, replay: Replay) -> Vec<u8> {
        let expected = self.baseline_pending.then(|| self.expected(map, replay, None).expect("completed occurrence replay"));
        let baseline = expected.as_ref().unwrap_or(&self.baseline);
        let (current, changed) = if self.unchanged_against(baseline, known, map) {
            (Vec::new(), Vec::new())
        } else {
            let current = projection(known, map);
            let changed = self.changed_against(baseline, &current);
            (current, changed)
        };
        let mut unknown = records(unknown);
        let mut result = Vec::new();
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
        // A decoded unknown can exhaust the event budget before its index is
        // appended. Decode then stops, so these records are the received tail.
        for (_, raw) in unknown {
            result.extend_from_slice(raw);
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
            baseline_pending: self.baseline_pending,
            completion_credit: self.completion_credit,
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
    #[inline]
    fn drop(&mut self) {
        // The helper empties the option. Its recursive drop must stay shared;
        // automatic field drop would emit a second copy in every owner.
        if self.0.is_some() {
            drop_storage(&mut self.0);
        }
    }
}
#[cold]
#[inline(never)]
fn drop_storage(state: &mut Option<Box<State>>) {
    *state = None;
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
        replay: Replay,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(state) = self.0.as_mut()
            && let Some(order) = state.order.as_mut()
        {
            ctx.register_element_memory(event_charge(state.fields.len() - order.unknown_count))?;
            order.append_unknown(state.fields.len());
            order.reconcile(known, map, replay, ctx)?;
        }
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn finish(
        &mut self,
        group: u32,
        raw: Option<Vec<u8>>,
        known: Option<&[u8]>,
        map: GroupMap,
        previous: usize,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        let state = self.0.as_mut().expect("decoded unknown storage");
        let order = state.order.as_mut().expect("started occurrence journal");
        if group != 0 && previous == state.fields.len() {
            ctx.register_element_memory(
                known.map_or(0, |known| projection_charge(known, map, 1)).saturating_add(event_charge(1)),
            )?;
            let raw = raw.expect("captured known occurrence");
            if group & (1 << 31) != 0 {
                order.append_repeated(group, &raw, ctx)?;
            } else if let Some(known) = known {
                order.baseline = projection(known, map);
            } else {
                order.baseline_pending = true;
            }
            order.events.push(Event::Known(group, Cow::Owned(raw)));
        }
        ctx.register_element_memory(event_charge(state.fields.len() - order.unknown_count))?;
        order.append_unknown(state.fields.len());
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn complete_batch(&mut self, known: Vec<u8>, map: GroupMap, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            ctx.register_element_memory(projection_charge(&known, map, 1))?;
            order.baseline = owned_projection(known, map);
            order.baseline_pending = false;
        }
        Ok(())
    }
    fn completed_canonical(&self, validate: fn(&[u8]) -> bool, ctx: DecodeContext<'_>) -> Result<Option<Vec<u8>>, DecodeError> {
        match self.0.as_ref().and_then(|state| state.order.as_ref()) {
            Some(order) => order.completed_canonical(validate, ctx),
            None => Ok(None),
        }
    }
    pub fn needs_baseline(&self) -> bool {
        self.0.as_ref().and_then(|state| state.order.as_ref()).is_some_and(|order| order.baseline_pending)
    }
    fn reserve_baseline(&mut self, group: u32, raw_len: usize, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        self.0.as_mut().and_then(|state| state.order.as_mut()).expect("active journal").reserve_baseline(group, raw_len, ctx)
    }
    fn take_completion_credit(&mut self) -> usize {
        self.0.as_mut().and_then(|state| state.order.as_mut()).map_or(0, |order| ::core::mem::take(&mut order.completion_credit))
    }
    #[cold]
    #[inline(never)]
    pub fn compose(&self, known: &[u8], map: GroupMap, replay: Replay) -> Vec<u8> {
        let state = self.0.as_ref().expect("active occurrence journal");
        let mut unknown = Vec::new();
        state.fields.write_to(&mut unknown);
        state
            .order
            .as_ref()
            .expect("active occurrence journal")
            .write(known, &unknown, map, replay)
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
    pub fn rebase(&mut self, view_known: &[u8], owned_known: &[u8], map: GroupMap, replay: Replay) {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            if order.baseline_pending {
                order.baseline = order.expected(map, replay, None).expect("completed view occurrence replay");
            }
            order.forced = order.changed(&projection(view_known, map));
            order.baseline = projection(owned_known, map);
            order.baseline_pending = false;
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
        self.push_decoded_record(tail, len, ctx)?;
        if let Some(state) = &mut self.0 {
            state.order = None;
        }
        Ok(())
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
        self.push_decoded_varint(field, value, ctx)?;
        if let Some(state) = &mut self.0 {
            state.order = None;
        }
        Ok(())
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
        replay: Replay,
        ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        if let Some(state) = self.0.as_mut()
            && let Some(order) = state.order.as_mut()
        {
            ctx.register_element_memory(event_charge(state.count - order.unknown_count))?;
            order.append_unknown(state.count);
            order.reconcile(known, map, replay, ctx)?;
        }
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn finish(
        &mut self,
        group: u32,
        raw: Cow<'a, [u8]>,
        known: Option<&[u8]>,
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
                known.map_or(0, |known| projection_charge(known, map, 1))
                    .saturating_add(owned_copy)
                    .saturating_add(event_charge(1)),
            )?;
            if group & (1 << 31) != 0 {
                order.append_repeated(group, &raw, ctx)?;
            } else if let Some(known) = known {
                order.baseline = projection(known, map);
            } else {
                order.baseline_pending = true;
            }
            order.events.push(Event::Known(group, raw));
        }
        ctx.register_element_memory(event_charge(state.count - order.unknown_count))?;
        order.append_unknown(state.count);
        Ok(())
    }
    #[cold]
    #[inline(never)]
    pub fn complete_batch(&mut self, known: Vec<u8>, map: GroupMap, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        if let Some(order) = self.0.as_mut().and_then(|state| state.order.as_mut()) {
            ctx.register_element_memory(projection_charge(&known, map, 1))?;
            order.baseline = owned_projection(known, map);
            order.baseline_pending = false;
        }
        Ok(())
    }
    fn completed_canonical(&self, validate: fn(&[u8]) -> bool, ctx: DecodeContext<'_>) -> Result<Option<Vec<u8>>, DecodeError> {
        match self.0.as_ref().and_then(|state| state.order.as_ref()) {
            Some(order) => order.completed_canonical(validate, ctx),
            None => Ok(None),
        }
    }
    pub fn needs_baseline(&self) -> bool {
        self.0.as_ref().and_then(|state| state.order.as_ref()).is_some_and(|order| order.baseline_pending)
    }
    fn reserve_baseline(&mut self, group: u32, raw_len: usize, ctx: DecodeContext<'_>) -> Result<(), DecodeError> {
        self.0.as_mut().and_then(|state| state.order.as_mut()).expect("active journal").reserve_baseline(group, raw_len, ctx)
    }
    fn take_completion_credit(&mut self) -> usize {
        self.0.as_mut().and_then(|state| state.order.as_mut()).map_or(0, |order| ::core::mem::take(&mut order.completion_credit))
    }
    #[cold]
    #[inline(never)]
    pub fn compose(&self, known: &[u8], map: GroupMap, replay: Replay) -> Vec<u8> {
        let state = self.0.as_ref().expect("active occurrence journal");
        let mut unknown = Vec::new();
        state.fields.write_to(&mut unknown);
        state
            .order
            .as_ref()
            .expect("active occurrence journal")
            .write(known, &unknown, map, replay)
    }
}

// A contiguous completed field needs one allocation. Pay the initial tag
// before probing, just as incremental capture rejects a too-small allowance
// before reading input. Fragmented, malformed and quota-limited payloads keep
// the original cursor and partial debit through the incremental fallback.
#[cold]
#[inline(never)]
fn capture_known_field(
    buf: &mut dyn Buf,
    tag: ::buffa::encoding::Tag,
    ctx: DecodeContext<'_>,
) -> Result<Vec<u8>, DecodeError> {
    ctx.register_element_memory(5)?;
    let input = buf.chunk();
    let mut remaining = input;
    if ::buffa::encoding::skip_field_depth(tag, &mut remaining, ctx.depth()).is_ok() {
        let consumed = input.len() - remaining.len();
        if let Some(charge) = consumed.checked_add(5)
            && ctx.register_element_memory(consumed).is_ok()
        {
            let mut raw = Vec::with_capacity(charge);
            tag.encode(&mut raw);
            raw.extend_from_slice(&input[..consumed]);
            buf.advance(consumed);
            return Ok(raw);
        }
    }
    let mut input = buf;
    let mut captured = Capture::with_prepaid_tag(&mut input, tag, ctx);
    ::buffa::encoding::skip_field_depth(tag, &mut captured, ctx.depth())?;
    captured.finish()
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
        Ok(Self::with_prepaid_tag(inner, tag, ctx))
    }
    fn with_prepaid_tag(
        inner: &'a mut impl Buf,
        tag: ::buffa::encoding::Tag,
        ctx: DecodeContext<'c>,
    ) -> Self {
        let mut bytes = Vec::new();
        tag.encode(&mut bytes);
        Self { inner, bytes, ctx, exhausted: false }
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

#[cfg(test)]
mod canonical_completion_tests {
    use super::*;

    fn canonical_empty_image(raw: &[u8]) -> bool { raw == [0x22, 0] }

    #[test]
    fn completion_preserves_allocation_debit_and_requires_a_fresh_single_occurrence() {
        let raw = [0x22, 0];
        let order = Order {
            events: ::buffa::alloc::vec![Event::Unknown(0), Event::Known(1, Cow::Borrowed(&raw))],
            baseline_pending: true,
            ..Default::default()
        };
        let unknown = ::core::cell::Cell::new(0);
        for extra in [0, 1] {
            let allowance = ::core::cell::Cell::new(raw.len() + extra);
            let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
            assert_eq!(order.completed_canonical(canonical_empty_image, ctx).unwrap(), Some(raw.to_vec()));
            assert_eq!(allowance.get(), extra);
        }
        let allowance = ::core::cell::Cell::new(raw.len() - 1);
        let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
        assert!(matches!(order.completed_canonical(canonical_empty_image, ctx), Err(DecodeError::ElementMemoryLimitExceeded)));
        assert_eq!(allowance.get(), raw.len() - 1);
        let mut previous = order.clone();
        previous.baseline.push((1, raw.to_vec()));
        assert_eq!(previous.completed_canonical(canonical_empty_image, ctx).unwrap(), None);
        previous = order.clone();
        previous.events.push(Event::Known(1, Cow::Borrowed(&raw)));
        assert_eq!(previous.completed_canonical(canonical_empty_image, ctx).unwrap(), None);
        previous = order;
        previous.forced.push(1);
        assert_eq!(previous.completed_canonical(canonical_empty_image, ctx).unwrap(), None);
        assert_eq!(unknown.get(), 0);
    }
}

#[cfg(test)]
mod contiguous_capture_tests {
    use super::*;
    use ::buffa::encoding::{Tag, WireType};

    fn original(buf: &mut dyn Buf, tag: Tag, ctx: DecodeContext<'_>) -> Result<Vec<u8>, DecodeError> {
        let mut input = buf;
        let mut captured = Capture::new(&mut input, tag, ctx)?;
        ::buffa::encoding::skip_field_depth(tag, &mut captured, ctx.depth())?;
        captured.finish()
    }

    #[test]
    fn insufficient_tag_allowance_rejects_before_reading_input() {
        struct Counted<'a> { bytes: &'a [u8], chunks: &'a ::core::cell::Cell<usize> }
        impl Buf for Counted<'_> {
            fn remaining(&self) -> usize { self.bytes.len() }
            fn chunk(&self) -> &[u8] {
                self.chunks.set(self.chunks.get() + 1);
                self.bytes
            }
            fn advance(&mut self, count: usize) { self.bytes.advance(count); }
        }
        let payload = [0x08, 1, 0x1c];
        for quota in 0..5 {
            let chunks = ::core::cell::Cell::new(0);
            let allowance = ::core::cell::Cell::new(quota);
            let unknown = ::core::cell::Cell::new(0);
            let mut input = Counted { bytes: &payload, chunks: &chunks };
            let ctx = DecodeContext::new(100, &unknown).with_element_memory(&allowance);
            assert!(matches!(capture_known_field(&mut input, Tag::new(3, WireType::StartGroup), ctx), Err(DecodeError::ElementMemoryLimitExceeded)));
            assert_eq!(chunks.get(), 0);
            assert_eq!(input.bytes, payload);
            assert_eq!(allowance.get(), quota);
            assert_eq!(unknown.get(), 0);
        }
    }

    #[test]
    fn contiguous_capture_matches_incremental_bytes_debits_and_errors() {
        for (wire, payload) in [
            (WireType::Varint, &[1, 7][..]),
            (WireType::Varint, &[0x81, 0, 7][..]),
            (WireType::Varint, &[0x80][..]),
            (WireType::Fixed32, &[1, 2, 3, 4, 7][..]),
            (WireType::Fixed32, &[1, 2][..]),
            (WireType::Fixed64, &[1, 2, 3, 4, 5, 6, 7, 8, 9][..]),
            (WireType::LengthDelimited, &[3, 11, 22, 33, 7][..]),
            (WireType::LengthDelimited, &[0x83, 0, 11, 22, 33, 7][..]),
            (WireType::LengthDelimited, &[3, 11][..]),
            (WireType::StartGroup, &[0x08, 1, 0x1c, 7][..]),
            (WireType::StartGroup, &[0x08, 1, 0x24, 7][..]),
        ] {
            let tag = Tag::new(3, wire);
            for depth in [0, 1, 100] {
                for quota in 0..=payload.len() + 7 {
                    let a = ::core::cell::Cell::new(quota);
                    let b = ::core::cell::Cell::new(quota);
                    let unknown = ::core::cell::Cell::new(0);
                    let mut before = payload;
                    let mut after = payload;
                    let expected = original(&mut before, tag, DecodeContext::new(depth, &unknown).with_element_memory(&a));
                    let actual = capture_known_field(&mut after, tag, DecodeContext::new(depth, &unknown).with_element_memory(&b));
                    assert_eq!(format!("{actual:?}"), format!("{expected:?}"), "wire={wire:?} depth={depth} quota={quota}");
                    assert_eq!(after, before);
                    assert_eq!(b.get(), a.get());
                    assert_eq!(unknown.get(), 0);
                }
            }
        }
    }

    #[test]
    fn contiguous_capture_matches_records_crossing_buffer_chunks() {
        let payload = [3, 11, 22, 33, 7];
        let tag = Tag::new(3, WireType::LengthDelimited);
        for split in 0..=payload.len() {
            for quota in 0..=12 {
                let a = ::core::cell::Cell::new(quota);
                let b = ::core::cell::Cell::new(quota);
                let unknown = ::core::cell::Cell::new(0);
                let mut before = (&payload[..split]).chain(&payload[split..]);
                let mut after = (&payload[..split]).chain(&payload[split..]);
                let expected = original(&mut before, tag, DecodeContext::new(100, &unknown).with_element_memory(&a));
                let actual = capture_known_field(&mut after, tag, DecodeContext::new(100, &unknown).with_element_memory(&b));
                assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
                assert_eq!(after.remaining(), before.remaining());
                assert_eq!(after.copy_to_bytes(after.remaining()), before.copy_to_bytes(before.remaining()));
                assert_eq!(b.get(), a.get());
                assert_eq!(unknown.get(), 0);
            }
        }
    }
}
