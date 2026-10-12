//! Carrier-preserving journal used by the existing inbound batch pipeline.

use bytes::Bytes;
use wacore::types::events::InboundMessage;

const MAGIC: &[u8; 6] = b"\0WRPD\x03";
const LEGACY_MAGIC: &[u8; 6] = b"\0WRPD\x01";
pub(super) const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

/// Producer-local identity, never a ciphertext authentication claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct StanzaOrigin([u8; 16]);
impl StanzaOrigin {
    pub fn new() -> Self {
        Self(rand::random())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DeliveryOrigin {
    pub stanza: StanzaOrigin,
    pub position: usize,
}
#[derive(Clone)]
pub(super) struct PendingDelivery {
    pub item: InboundMessage,
    pub carrier: Option<(Option<u32>, Bytes)>,
    pub origin: Option<DeliveryOrigin>,
    pub retention: Option<std::sync::Arc<super::live_retention::Charge>>,
    pub receipt: Option<std::sync::Arc<super::live_retention::ReceiptScope>>,
    pub consumer_claimed: bool,
}
impl From<InboundMessage> for PendingDelivery {
    fn from(item: InboundMessage) -> Self {
        Self {
            item,
            carrier: None,
            origin: None,
            retention: None,
            receipt: None,
            consumer_claimed: false,
        }
    }
}
impl std::ops::Deref for PendingDelivery {
    type Target = InboundMessage;
    fn deref(&self) -> &Self::Target {
        &self.item
    }
}
impl PendingDelivery {
    // Keep journal materialization out of each async replay frame. Callers
    // retain their own position, receipt and byte-exact sharing policies.
    #[inline(never)]
    pub(super) fn from_occurrence(
        entry: Occurrence,
        position: usize,
        info: &std::sync::Arc<wacore::types::message::MessageInfo>,
        shared_message: Option<std::sync::Arc<waproto::whatsapp::Message>>,
        receipt: Option<std::sync::Arc<super::live_retention::ReceiptScope>>,
    ) -> anyhow::Result<Self> {
        let message = match shared_message {
            Some(message) => message,
            None => std::sync::Arc::new(waproto::codec::message_decode(&entry.projection)?),
        };
        Ok(Self {
            item: InboundMessage::builder()
                .message(message)
                .info(std::sync::Arc::clone(info))
                .build(),
            carrier: Some((entry.index, entry.original)),
            origin: entry
                .producer
                .map(|stanza| DeliveryOrigin { stanza, position }),
            retention: None,
            receipt,
            consumer_claimed: false,
        })
    }
    pub fn retained_wire_bytes(&self) -> usize {
        waproto::codec::message_encoded_len(&self.message)
            .saturating_add(self.carrier.as_ref().map_or(0, |(_, bytes)| bytes.len()))
    }
}

#[derive(Clone)]
pub(super) struct Occurrence {
    pub producer: Option<StanzaOrigin>,

    pub index: Option<u32>,
    pub original: Bytes,
    pub projection: Bytes,
}
pub(super) struct PendingJournal {
    generation: [u8; 16],
    pub occurrences: Vec<Occurrence>,
    backing: Option<Bytes>,
}
impl Default for PendingJournal {
    fn default() -> Self {
        Self {
            generation: rand::random(),
            occurrences: Vec::new(),
            backing: None,
        }
    }
}
impl PendingJournal {
    pub fn decode(bytes: Bytes) -> anyhow::Result<Self> {
        anyhow::ensure!(
            bytes.len() <= MAX_PENDING_BYTES,
            "pending journal exceeds bound"
        );
        if bytes.first() != Some(&0) {
            // Preserve a legacy row verbatim. It has no authenticated slot
            // index and must not be merged into an indexed occurrence group.
            let message = waproto::codec::message_decode(&bytes)?;
            return Ok(Self {
                generation: rand::random(),
                occurrences: vec![Occurrence {
                    producer: None,

                    index: None,
                    original: bytes,
                    projection: waproto::codec::message_to_vec(&message).into(),
                }],
                backing: None,
            });
        }
        let grouped = bytes.starts_with(MAGIC);
        anyhow::ensure!(
            grouped || bytes.starts_with(LEGACY_MAGIC),
            "unsupported pending journal version"
        );
        let mut cursor = Cursor {
            bytes,
            offset: MAGIC.len(),
        };
        let generation: [u8; 16] = cursor.take(16)?.as_ref().try_into()?;
        let count = cursor.number()?;
        anyhow::ensure!(
            count != 0 && count <= cursor.remaining() / if grouped { 30 } else { 13 },
            "invalid pending occurrence count"
        );
        anyhow::ensure!(
            count
                .checked_mul(size_of::<Occurrence>())
                .and_then(|metadata| metadata.checked_add(cursor.bytes.len()))
                .is_some_and(|retained| retained <= MAX_PENDING_BYTES),
            "pending journal backing and metadata exceed admission"
        );
        let mut occurrences = Vec::with_capacity(count);
        for _ in 0..count {
            let producer = if grouped {
                let present = cursor.take(1)?[0];
                let id: [u8; 16] = cursor.take(16)?.as_ref().try_into()?;
                match present {
                    0 if id == [0; 16] => None,
                    1 => Some(StanzaOrigin(id)),
                    _ => anyhow::bail!("invalid pending producer identity"),
                }
            } else {
                None
            };
            let indexed = cursor.take(1)?[0];
            let index = cursor.number()?;
            let index = match indexed {
                0 if index == 0 => None,
                1 => Some(u32::try_from(index)?),
                _ => anyhow::bail!("invalid pending index"),
            };
            let original = cursor.blob()?;
            let projection = cursor.blob()?;
            occurrences.push(Occurrence {
                producer,

                index,
                original,
                projection,
            });
        }
        anyhow::ensure!(cursor.remaining() == 0, "trailing pending journal bytes");
        let journal = Self {
            generation,
            occurrences,
            backing: grouped.then(|| cursor.bytes.clone()),
        };
        journal.validate()?;
        Ok(journal)
    }

    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.occurrences.is_empty(), "empty pending journal");
        for (position, entry) in self.occurrences.iter().enumerate() {
            if let Some(index) = entry.index {
                anyhow::ensure!(
                    self.occurrences[..position]
                        .iter()
                        .filter(|old| old.producer == entry.producer)
                        .all(|old| old.index.is_some_and(|prior| prior < index)),
                    "unordered pending producer occurrences"
                );
            } else {
                anyhow::ensure!(
                    self.occurrences
                        .iter()
                        .filter(|old| old.producer == entry.producer)
                        .all(|old| old.index.is_none()),
                    "mixed indexed and legacy producer occurrences"
                );
            }
        }
        Ok(())
    }

    pub fn merge(&mut self, occurrence: Occurrence) -> anyhow::Result<()> {
        // Slot identity is local to a producer group. An independent retry can
        // reuse its index while retaining a different SKDM carrier verbatim.
        let same_group = |old: &&Occurrence| old.producer == occurrence.producer;
        if let Some(index) = occurrence.index {
            if let Some(existing) = self
                .occurrences
                .iter_mut()
                .filter(|old| old.producer == occurrence.producer)
                .find(|old| old.index == Some(index))
            {
                anyhow::ensure!(
                    existing.original == occurrence.original
                        && existing.projection == occurrence.projection,
                    "pending occurrence identity conflict"
                );
                return Ok(());
            }
            anyhow::ensure!(
                self.occurrences
                    .iter()
                    .filter(same_group)
                    .all(|old| old.index.is_some()),
                "legacy producer requires replay"
            );
        } else {
            anyhow::ensure!(
                self.occurrences
                    .iter()
                    .filter(same_group)
                    .all(|old| old.index.is_none()),
                "mixed pending producer indices"
            );
            if self.occurrences.iter().filter(same_group).any(|old| {
                old.original == occurrence.original && old.projection == occurrence.projection
            }) {
                return Ok(());
            }
        }
        // Insertion keeps group arrival order and sorts only slots of that
        // producer; no content equality transfers ownership across groups.
        let position = self
            .occurrences
            .iter()
            .position(|old| old.producer == occurrence.producer && old.index > occurrence.index)
            .or_else(|| {
                self.occurrences
                    .iter()
                    .rposition(|old| old.producer == occurrence.producer)
                    .map(|last| last + 1)
            })
            .unwrap_or(self.occurrences.len());
        self.occurrences.insert(position, occurrence);
        Ok(())
    }

    pub fn fits_additional(&self, original: usize, projection: usize) -> bool {
        let current = self
            .occurrences
            .iter()
            .try_fold(MAGIC.len() + 16 + 4, |size, entry| {
                size.checked_add(30 + size_of::<Occurrence>())
                    .and_then(|size| size.checked_add(entry.original.len()))
                    .and_then(|size| size.checked_add(entry.projection.len()))
            });
        current
            .and_then(|size| size.checked_add(30 + size_of::<Occurrence>()))
            .and_then(|size| size.checked_add(original))
            .and_then(|size| size.checked_add(projection))
            .is_some_and(|size| size <= MAX_PENDING_BYTES)
    }

    fn matches_backing(&self, bytes: &Bytes) -> anyhow::Result<bool> {
        let mut cursor = Cursor {
            bytes: bytes.clone(),
            offset: MAGIC.len(),
        };
        if cursor.take(16)?.as_ref() != self.generation
            || cursor.number()? != self.occurrences.len()
        {
            return Ok(false);
        }
        for entry in &self.occurrences {
            if cursor.take(1)?[0] != u8::from(entry.producer.is_some())
                || cursor.take(16)?.as_ref() != entry.producer.map_or([0; 16], |id| id.0)
                || cursor.take(1)?[0] != u8::from(entry.index.is_some())
                || cursor.number()? != usize::try_from(entry.index.unwrap_or(0))?
            {
                return Ok(false);
            }
            for current in [&entry.original, &entry.projection] {
                let saved = cursor.blob()?;
                if saved.len() != current.len()
                    || (saved.as_ptr() != current.as_ptr() && saved != *current)
                {
                    return Ok(false);
                }
            }
        }
        Ok(cursor.remaining() == 0)
    }

    pub fn encode(&self) -> anyhow::Result<Bytes> {
        self.validate()?;
        // A retry of the same physical occurrences needs the existing bytes,
        // not another row-sized allocation. Compare metadata and every slice
        // so even an edited occurrence cannot reuse stale backing.
        if let Some(backing) = &self.backing
            && self.matches_backing(backing)?
        {
            return Ok(backing.clone());
        }
        // Do the complete wire-bound check before allocating the replacement.
        let mut length = MAGIC.len() + 16 + 4;
        for occurrence in &self.occurrences {
            u32::try_from(occurrence.original.len())?;
            u32::try_from(occurrence.projection.len())?;
            length = length
                .checked_add(30)
                .and_then(|n| n.checked_add(occurrence.original.len()))
                .and_then(|n| n.checked_add(occurrence.projection.len()))
                .ok_or_else(|| anyhow::anyhow!("pending journal size overflow"))?;
        }
        anyhow::ensure!(
            self.occurrences
                .len()
                .checked_mul(size_of::<Occurrence>())
                .and_then(|metadata| metadata.checked_add(length))
                .is_some_and(|retained| retained <= MAX_PENDING_BYTES),
            "pending journal backing and metadata exceed admission"
        );
        let mut out = Vec::with_capacity(length);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&self.generation);
        out.extend_from_slice(&u32::try_from(self.occurrences.len())?.to_le_bytes());
        for occurrence in &self.occurrences {
            out.push(u8::from(occurrence.producer.is_some()));
            out.extend_from_slice(&occurrence.producer.map_or([0; 16], |id| id.0));
            out.push(u8::from(occurrence.index.is_some()));
            out.extend_from_slice(&occurrence.index.unwrap_or(0).to_le_bytes());
            for bytes in [&occurrence.original, &occurrence.projection] {
                out.extend_from_slice(&u32::try_from(bytes.len())?.to_le_bytes());
                out.extend_from_slice(bytes);
            }
        }
        Ok(out.into())
    }
}
struct Cursor {
    bytes: Bytes,
    offset: usize,
}
impl Cursor {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
    fn take(&mut self, count: usize) -> anyhow::Result<Bytes> {
        let end = self
            .offset
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| anyhow::anyhow!("truncated pending journal"))?;
        let bytes = self.bytes.slice(self.offset..end);
        self.offset = end;
        Ok(bytes)
    }
    fn number(&mut self) -> anyhow::Result<usize> {
        Ok(usize::try_from(u32::from_le_bytes(
            self.take(4)?.as_ref().try_into()?,
        ))?)
    }
    fn blob(&mut self) -> anyhow::Result<Bytes> {
        let length = self.number()?;
        self.take(length)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn occurrence(index: u32, value: &'static [u8]) -> Occurrence {
        Occurrence {
            producer: None,

            index: Some(index),
            original: Bytes::from_static(value),
            projection: Bytes::from_static(value),
        }
    }

    #[test]
    fn raw_pending_journal_keeps_equal_occurrences_and_rejects_slot_conflict() {
        let mut journal = PendingJournal::default();
        journal.merge(occurrence(2, b"B")).unwrap();
        journal.merge(occurrence(0, b"A")).unwrap();
        journal.merge(occurrence(1, b"A")).unwrap();
        journal.merge(occurrence(1, b"A")).unwrap();
        assert!(journal.merge(occurrence(1, b"other")).is_err());
        let restored = PendingJournal::decode(journal.encode().unwrap()).unwrap();
        assert_eq!(restored.occurrences.len(), 3);
        assert_eq!(
            restored
                .occurrences
                .iter()
                .map(|entry| entry.original.as_ref())
                .collect::<Vec<_>>(),
            [b"A".as_slice(), b"A".as_slice(), b"B".as_slice()]
        );
    }

    #[test]
    fn raw_pending_retry_groups_keep_distinct_skdm_carriers_at_the_same_slot() {
        let first = StanzaOrigin::new();
        let retry = StanzaOrigin::new();
        let mut journal = PendingJournal::default();
        let entry = |producer, raw: &'static [u8]| Occurrence {
            producer: Some(producer),

            index: Some(0),
            original: Bytes::from_static(raw),
            projection: Bytes::from_static(b"same consumer"),
        };
        journal.merge(entry(first, b"original SKDM")).unwrap();
        journal.merge(entry(retry, b"rotated SKDM")).unwrap();
        journal.merge(entry(first, b"original SKDM")).unwrap();
        assert!(journal.merge(entry(first, b"conflicting slot")).is_err());
        let restored = PendingJournal::decode(journal.encode().unwrap()).unwrap();
        assert_eq!(restored.occurrences.len(), 2);
        assert_eq!(restored.occurrences[0].original.as_ref(), b"original SKDM");
        assert_eq!(restored.occurrences[1].original.as_ref(), b"rotated SKDM");
        assert_eq!(restored.occurrences[0].producer, Some(first));
        assert_eq!(restored.occurrences[1].producer, Some(retry));
    }

    #[test]
    fn raw_pending_new_generation_cannot_reuse_predecessor_revision() {
        let mut first = PendingJournal::default();
        first.merge(occurrence(0, b"A")).unwrap();
        let bytes = first.encode().unwrap();
        let same_owner = PendingJournal::decode(bytes.clone()).unwrap();
        assert_eq!(same_owner.encode().unwrap(), bytes);
        let mut successor = PendingJournal::default();
        successor.merge(occurrence(0, b"A")).unwrap();
        assert_ne!(
            wacore::store::traits::PendingInboundRevision::for_bytes(&bytes),
            wacore::store::traits::PendingInboundRevision::for_bytes(&successor.encode().unwrap())
        );
    }

    #[test]
    fn raw_pending_parser_budget_includes_backing_and_occurrence_metadata() {
        let count = MAX_PENDING_BYTES / 40;
        let mut wire = Vec::with_capacity(MAGIC.len() + 20 + count * 30);
        wire.extend_from_slice(MAGIC);
        wire.extend_from_slice(&[7; 16]);
        wire.extend_from_slice(&u32::try_from(count).unwrap().to_le_bytes());
        for index in 0..count {
            wire.extend_from_slice(&[0; 17]);
            wire.push(1);
            wire.extend_from_slice(&u32::try_from(index).unwrap().to_le_bytes());
            wire.extend_from_slice(&[0; 8]);
        }
        assert!(
            wire.len() < MAX_PENDING_BYTES,
            "wire limit alone admits the frame"
        );
        assert!(count * size_of::<Occurrence>() + wire.len() > MAX_PENDING_BYTES);
        assert!(
            PendingJournal::decode(wire.into()).is_err(),
            "refuse before allocating the oversized metadata vector"
        );
    }

    #[test]
    fn raw_pending_encoder_and_decoder_share_backing_metadata_bound() {
        let metadata = size_of::<Occurrence>();
        let fixed = MAGIC.len() + 16 + 4 + 30;
        let mut journal = PendingJournal::default();
        let mut entry = occurrence(0, b"");
        entry.original = vec![7; MAX_PENDING_BYTES - fixed - metadata].into();
        journal.merge(entry).unwrap();
        let admitted = journal.encode().unwrap();
        assert_eq!(admitted.len() + metadata, MAX_PENDING_BYTES);
        assert_eq!(
            PendingJournal::decode(admitted).unwrap().occurrences.len(),
            1
        );
        journal.occurrences[0].original = vec![7; MAX_PENDING_BYTES - fixed - metadata + 1].into();
        assert!(
            journal.encode().is_err(),
            "refuse a frame the decoder cannot retain"
        );
    }

    #[test]
    fn raw_pending_journal_rejects_truncation_future_version_and_oversize() {
        let mut journal = PendingJournal::default();
        journal.merge(occurrence(0, b"A")).unwrap();
        let bytes = journal.encode().unwrap();
        for end in 1..bytes.len() {
            assert!(PendingJournal::decode(bytes.slice(..end)).is_err());
        }
        let mut future = bytes.to_vec();
        future[5] = 4;
        assert!(PendingJournal::decode(future.into()).is_err());
        journal.occurrences[0].original = vec![0; MAX_PENDING_BYTES].into();
        assert!(journal.encode().is_err());
    }
    #[test]
    fn raw_pending_integral_retained_heap_sample() {
        use crate::test_alloc::{current_heap_sample, heap_sample};
        use std::sync::Arc;
        use wacore::store::traits::{PendingInboundLease, PendingInboundRecord};
        for payload in [128, 1800 * 1024] {
            let (baseline, baseline_held) = heap_sample(|| {
                let mut stages = [crate::test_alloc::HeapSample::default(); 6];
                let mut message = waproto::whatsapp::Message::default();
                message.conversation = Some("x".repeat(payload));
                stages[0] = current_heap_sample();
                let projection: Bytes = waproto::codec::message_to_vec(&message).into();
                let mut journal = PendingJournal::default();
                journal
                    .merge(Occurrence {
                        producer: Some(StanzaOrigin::new()),
                        index: Some(0),
                        original: projection.clone(),
                        projection,
                    })
                    .unwrap();
                stages[1] = current_heap_sample();
                let row = journal.encode().unwrap();
                stages[2] = current_heap_sample();
                let copied = Bytes::copy_from_slice(&row);
                let decoded = PendingJournal::decode(copied).unwrap();
                stages[3] = current_heap_sample();
                let graphs: Vec<_> = decoded
                    .occurrences
                    .iter()
                    .map(|entry| waproto::codec::message_decode(&entry.projection).unwrap())
                    .collect();
                stages[4] = current_heap_sample();
                // Reference the old copying handoff, including its newly
                // allocated replacement even when the row was unchanged.
                let replacement = Bytes::copy_from_slice(&decoded.encode().unwrap());
                stages[5] = current_heap_sample();
                (message, journal, row, decoded, graphs, replacement, stages)
            });
            eprintln!(
                "copying reference payload={payload}: {baseline:?}, stages={:?}",
                baseline_held.6
            );
            drop(baseline_held);
            for (append, cold) in [(false, false), (true, false), (false, true), (true, true)] {
                let (optimized, held) = heap_sample(|| {
                    let mut stages = [crate::test_alloc::HeapSample::default(); 6];
                    let mut message = waproto::whatsapp::Message::default();
                    message.conversation = Some("x".repeat(payload));
                    let message = Arc::new(message);
                    stages[0] = current_heap_sample();
                    let projection: Bytes = waproto::codec::message_to_vec(&message).into();
                    let mut journal = PendingJournal::default();
                    journal
                        .merge(Occurrence {
                            producer: Some(StanzaOrigin::new()),
                            index: Some(0),
                            original: projection.clone(),
                            projection: projection.clone(),
                        })
                        .unwrap();
                    stages[1] = current_heap_sample();
                    let row = journal.encode().unwrap();
                    let row_ptr = row.as_ptr();
                    let owned_row = Vec::from(row);
                    assert_eq!(
                        owned_row.as_ptr(),
                        row_ptr,
                        "unique row transfers without copying"
                    );
                    let record = PendingInboundRecord::new(
                        owned_row,
                        PendingInboundLease::new(Arc::new(())),
                    );
                    drop(journal);
                    stages[2] = current_heap_sample();
                    let (backing, lease) = record.into_bytes();
                    let mut decoded = PendingJournal::decode(backing).unwrap();
                    stages[3] = current_heap_sample();
                    assert_eq!(decoded.occurrences[0].projection, projection);
                    // A cold client has only the stored row. Release fixture
                    // setup objects before decoding; the peak still includes
                    // their earlier transient coexistence during preparation.
                    let (message, projection, replay_graph) = if cold {
                        drop(message);
                        drop(projection);
                        let graph = Arc::new(
                            waproto::codec::message_decode(&decoded.occurrences[0].projection)
                                .unwrap(),
                        );
                        (None, None, graph)
                    } else {
                        let graph = Arc::clone(&message);
                        (Some(message), Some(projection), graph)
                    };
                    stages[4] = current_heap_sample();
                    if append {
                        decoded
                            .merge(Occurrence {
                                producer: Some(StanzaOrigin::new()),
                                index: Some(1),
                                original: Bytes::from_static(b"\x0a\x83\x00new"),
                                projection: Bytes::from_static(b"\x0a\x03new"),
                            })
                            .unwrap();
                    }
                    let replacement = decoded.encode().unwrap();
                    stages[5] = current_heap_sample();
                    (
                        message,
                        projection,
                        decoded,
                        replay_graph,
                        replacement,
                        lease,
                        stages,
                    )
                });
                eprintln!(
                    "shared ownership payload={payload}, append={append}, cold={cold}: {optimized:?}, stages={:?}",
                    held.6
                );
                // This flat conversation fixture accounts for current graph,
                // projection, prior row and replacement. Three admission-sized
                // buffers plus 64 KiB cover their transient bookkeeping. This
                // is not a global heap limit for arbitrary protobuf graphs,
                // concurrent receives or SQLite's own internal allocations.
                let scenario_budget = 3 * MAX_PENDING_BYTES + 64 * 1024;
                assert!(optimized.peak <= scenario_budget as i64);
                assert!(optimized.retained <= optimized.peak);
                // Only unchanged warm retry has the same work as the
                // copying reference. Cold decode and append are separate
                // peak scenarios, not equivalent allocation comparisons.
                if !append && !cold {
                    assert!(optimized.allocations < baseline.allocations);
                }
                if payload > 1024 && !append && !cold {
                    assert!(
                        optimized.peak * 4 <= baseline.peak * 3,
                        "large payload must eliminate copies, not only raise a limit"
                    );
                }
                drop(held);
            }
        }
    }

    #[test]
    fn raw_pending_dense_graph_heap_sample() {
        use crate::test_alloc::{current_heap_sample, heap_sample};
        let (sample, held) = heap_sample(|| {
            let mut message = waproto::whatsapp::Message::default();
            let mut text = waproto::whatsapp::message::ExtendedTextMessage::default();
            let mut context = waproto::whatsapp::ContextInfo::default();
            context.mentioned_jid = vec![String::new(); 8192];
            text.context_info = buffa::MessageField::some(context);
            message.extended_text_message = buffa::MessageField::some(text);
            let graph = current_heap_sample();
            let projection: Bytes = waproto::codec::message_to_vec(&message).into();
            let mut journal = PendingJournal::default();
            journal
                .merge(Occurrence {
                    producer: Some(StanzaOrigin::new()),
                    index: Some(0),
                    original: projection.clone(),
                    projection: projection.clone(),
                })
                .unwrap();
            let row = journal.encode().unwrap();
            let decoded = waproto::codec::message_decode(&projection).unwrap();
            (message, projection, journal, row, decoded, graph)
        });
        eprintln!(
            "dense graph: {sample:?}, graph={:?}, projection={}, row={}",
            held.5,
            held.1.len(),
            held.3.len()
        );
        assert!(held.3.len() < MAX_PENDING_BYTES);
        // Empty repeated strings still allocate a Vec of String headers.
        // Wire admission alone therefore cannot bound the decoded graph.
        assert!(held.5.retained > held.1.len() as i64);
        assert_eq!(
            held.4
                .extended_text_message
                .as_option()
                .unwrap()
                .context_info
                .as_option()
                .unwrap()
                .mentioned_jid
                .len(),
            8192
        );
        drop(held);
    }

    #[test]
    fn raw_pending_backing_reuse_checks_direct_edits() {
        let mut initial = PendingJournal::default();
        initial.merge(occurrence(0, b"old")).unwrap();
        let bytes = initial.encode().unwrap();
        for edit in 0..4 {
            let mut edited = PendingJournal::decode(bytes.clone()).unwrap();
            match edit {
                0 => edited.generation[0] ^= 1,
                1 => edited.occurrences[0].producer = Some(StanzaOrigin::new()),
                2 => edited.occurrences[0].index = Some(1),
                _ => edited.occurrences[0].projection = Bytes::from_static(b"new projection"),
            }
            let replacement = edited.encode().unwrap();
            assert_ne!(replacement.as_ptr(), bytes.as_ptr());
            let restored = PendingJournal::decode(replacement).unwrap();
            assert_eq!(restored.generation, edited.generation);
            assert_eq!(
                restored.occurrences[0].producer,
                edited.occurrences[0].producer
            );
            assert_eq!(restored.occurrences[0].index, edited.occurrences[0].index);
            assert_eq!(
                restored.occurrences[0].projection,
                edited.occurrences[0].projection
            );
        }
        let mut decoded = PendingJournal::decode(bytes.clone()).unwrap();
        assert_eq!(decoded.encode().unwrap().as_ptr(), bytes.as_ptr());
        decoded.occurrences[0].original = Bytes::from_static(b"edited");
        let replacement = decoded.encode().unwrap();
        assert_ne!(replacement.as_ptr(), bytes.as_ptr());
        let restored = PendingJournal::decode(replacement).unwrap();
        assert_eq!(restored.occurrences[0].original.as_ref(), b"edited");
    }
}
