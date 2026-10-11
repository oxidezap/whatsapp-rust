//! Carrier-preserving journal used by the existing inbound batch pipeline.

use bytes::Bytes;
use wacore::types::events::InboundMessage;

const MAGIC: &[u8; 6] = b"\0WRPD\x03";
const LEGACY_MAGIC: &[u8; 6] = b"\0WRPD\x01";
pub(super) const MAX_PENDING_BYTES: usize = 4 * 1024 * 1024;

/// Producer-local identity, never a ciphertext authentication claim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
}
impl From<InboundMessage> for PendingDelivery {
    fn from(item: InboundMessage) -> Self {
        Self {
            item,
            carrier: None,
            origin: None,
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
}
impl Default for PendingJournal {
    fn default() -> Self {
        Self {
            generation: rand::random(),
            occurrences: Vec::new(),
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

    pub fn encode(&self) -> anyhow::Result<Bytes> {
        self.validate()?;
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
}
