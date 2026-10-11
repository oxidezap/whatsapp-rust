//! Inbound durability hook: pending-record replay and the consumer commit barrier.
//! See
//! [`crate::types::durability_hook::InboundDurabilityHook`] for the contract.
//!
//! The first-receipt path lives in [`super::commit_batch`]: messages commit
//! per batch (buffer → hook → ack). This module keeps the redelivery replay,
//! where each replayed stanza resolves all of its stored payload parts.

use super::*;
use crate::types::durability_hook::InboundDurabilityHook;
use wacore::types::events::InboundMessage;

// Zero is not a protobuf field tag, so the envelope cannot alias an existing
// serialized Message. Legacy single-message and v1 rows remain readable.
const PARTS_HEADER: &[u8] = b"\0WAPI\x01";
const SOURCED_PARTS_HEADER: &[u8] = b"\0WAPI\x02";
const SOURCE_SETS_HEADER: &[u8] = b"\0WAPI\x03";
const DEFERRED_PARTS_HEADER: &[u8] = b"\0WAPI\x04";

#[derive(Clone)]
struct PendingPart<'a> {
    bytes: &'a [u8],
    sources: retention::PayloadSources,
}

#[cfg(test)]
fn encode_pending_sourced_parts(parts: &[(&[u8], Option<retention::PayloadSource>)]) -> Vec<u8> {
    encode_pending_source_sets(
        &parts
            .iter()
            .map(|(bytes, source)| (*bytes, source.iter().copied().collect()))
            .collect::<Vec<_>>(),
    )
}
pub(super) fn encode_pending_source_sets(parts: &[(&[u8], retention::PayloadSources)]) -> Vec<u8> {
    if parts.iter().all(|(_, sources)| sources.is_empty()) {
        return encode_pending_parts(&parts.iter().map(|(bytes, _)| *bytes).collect::<Vec<_>>());
    }
    let sets = parts.iter().any(|(_, sources)| sources.len() > 1);
    let mut record = if sets {
        SOURCE_SETS_HEADER
    } else {
        SOURCED_PARTS_HEADER
    }
    .to_vec();
    for (bytes, sources) in parts {
        if sets {
            record.extend_from_slice(&(sources.len() as u64).to_be_bytes());
        } else {
            record.push(u8::from(!sources.is_empty()));
        }
        for source in sources {
            record.extend_from_slice(source);
        }
        record.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        record.extend_from_slice(bytes);
    }
    record
}

pub(super) fn encode_pending_parts(parts: &[&[u8]]) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(PARTS_HEADER);
    for part in parts {
        // A slice cannot exceed u64 on supported targets. Lengths are decoded
        // against the remaining input before any payload allocation.
        record.extend_from_slice(&(part.len() as u64).to_be_bytes());
        record.extend_from_slice(part);
    }
    record
}

fn plain_pending_records(bytes: &[u8]) -> anyhow::Result<Vec<PendingPart<'_>>> {
    if bytes.first() != Some(&0) {
        return Ok(vec![PendingPart {
            bytes,
            sources: Default::default(),
        }]);
    }
    let sets = bytes.starts_with(SOURCE_SETS_HEADER);
    let sourced = sets || bytes.starts_with(SOURCED_PARTS_HEADER);
    let mut remaining = bytes
        .strip_prefix(if sets {
            SOURCE_SETS_HEADER
        } else if sourced {
            SOURCED_PARTS_HEADER
        } else {
            PARTS_HEADER
        })
        .ok_or_else(|| anyhow::anyhow!("unknown pending-inbound record header"))?;
    let mut parts = Vec::new();
    while !remaining.is_empty() {
        let count = if sets {
            anyhow::ensure!(remaining.len() >= 8, "truncated source count");
            let count = usize::try_from(u64::from_be_bytes(remaining[..8].try_into()?))?;
            remaining = &remaining[8..];
            anyhow::ensure!(count <= remaining.len() / 32, "truncated source identities");
            count
        } else if sourced {
            let (&flag, rest) = remaining.split_first().expect("nonempty record remainder");
            remaining = rest;
            anyhow::ensure!(flag <= 1, "invalid pending-inbound source flag");
            usize::from(flag)
        } else {
            0
        };
        anyhow::ensure!(
            count <= remaining.len() / 32,
            "truncated pending-inbound source"
        );
        anyhow::ensure!(
            count <= retention::MAX_SOURCE_IDENTITIES,
            "source history limit exceeded"
        );
        let mut sources = retention::PayloadSources::new();
        for _ in 0..count {
            sources.push(remaining[..32].try_into()?);
            remaining = &remaining[32..];
        }
        anyhow::ensure!(remaining.len() >= 8, "truncated pending-inbound length");
        let mut length = [0; 8];
        length.copy_from_slice(&remaining[..8]);
        remaining = &remaining[8..];
        let length = usize::try_from(u64::from_be_bytes(length))?;
        anyhow::ensure!(
            length <= remaining.len(),
            "truncated pending-inbound payload"
        );
        parts.push(PendingPart {
            bytes: &remaining[..length],
            sources,
        });
        remaining = &remaining[length..];
    }
    anyhow::ensure!(!parts.is_empty(), "empty pending-inbound record");
    Ok(parts)
}
pub(super) struct DeferredParts<'a> {
    pub(super) canonical: &'a [u8],
    pub(super) waiting: Vec<&'a [u8]>,
}

pub(super) fn deferred_parts(bytes: &[u8]) -> anyhow::Result<Option<DeferredParts<'_>>> {
    let Some(mut remaining) = bytes.strip_prefix(DEFERRED_PARTS_HEADER) else {
        return Ok(None);
    };
    let mut records = Vec::new();
    while !remaining.is_empty() {
        anyhow::ensure!(remaining.len() >= 8, "truncated deferred record length");
        let length = usize::try_from(u64::from_be_bytes(remaining[..8].try_into()?))?;
        remaining = &remaining[8..];
        anyhow::ensure!(
            (length > 0 || records.is_empty()) && length <= remaining.len(),
            "truncated deferred record"
        );
        anyhow::ensure!(
            records.len() <= retention::MAX_SOURCE_IDENTITIES,
            "deferred delivery limit exceeded"
        );
        let record = &remaining[..length];
        // Each group is a plain envelope, so nesting cannot amplify allocation.
        if !record.is_empty() {
            plain_pending_records(record)?;
        }
        records.push(record);
        remaining = &remaining[length..];
    }
    anyhow::ensure!(
        records.len() >= 2,
        "deferred record requires canonical and waiting parts"
    );
    let canonical = records.remove(0);
    Ok(Some(DeferredParts {
        canonical,
        waiting: records,
    }))
}

fn pending_records(bytes: &[u8]) -> anyhow::Result<Vec<PendingPart<'_>>> {
    match deferred_parts(bytes)? {
        Some(record) if record.canonical.is_empty() => Ok(Vec::new()),
        Some(record) => plain_pending_records(record.canonical),
        None => plain_pending_records(bytes),
    }
}

fn encode_deferred_parts(canonical: &[u8], waiting: &[&[u8]]) -> Vec<u8> {
    let mut record = DEFERRED_PARTS_HEADER.to_vec();
    for group in std::iter::once(canonical).chain(waiting.iter().copied()) {
        record.extend_from_slice(&(group.len() as u64).to_be_bytes());
        record.extend_from_slice(group);
    }
    record
}

fn append_deferred_parts(
    existing: Option<&[u8]>,
    canonical: &[u8],
    groups: &[Vec<u8>],
) -> anyhow::Result<Vec<u8>> {
    let prior = existing.map(deferred_parts).transpose()?.flatten();
    let original = prior.as_ref().map(|record| record.canonical).or(existing);
    let canonical = if let Some(original) = original.filter(|record| !record.is_empty()) {
        extend_pending_record(original, canonical)?.unwrap_or_else(|| original.to_vec())
    } else {
        canonical.to_vec()
    };
    let mut waiting = prior.map_or_else(Vec::new, |record| record.waiting);
    for group in groups {
        // A cancelled store may already have written this exact generation.
        if !waiting.contains(&group.as_slice()) {
            waiting.push(group);
        }
    }
    anyhow::ensure!(
        waiting.len() <= retention::MAX_SOURCE_IDENTITIES,
        "deferred delivery limit exceeded"
    );
    anyhow::ensure!(!waiting.is_empty(), "missing waiting delivery");
    Ok(encode_deferred_parts(&canonical, &waiting))
}

#[cfg(test)]
pub(super) fn pending_parts(bytes: &[u8]) -> anyhow::Result<Vec<&[u8]>> {
    Ok(pending_records(bytes)?
        .into_iter()
        .map(|part| part.bytes)
        .collect())
}
pub(super) fn decode_pending_parts(bytes: &[u8]) -> anyhow::Result<Vec<wa::Message>> {
    pending_records(bytes)?
        .into_iter()
        .map(|part| waproto::codec::message_decode(part.bytes).map_err(Into::into))
        .collect()
}

pub(super) fn extend_pending_record(
    existing: &[u8],
    proposed: &[u8],
) -> anyhow::Result<Option<Vec<u8>>> {
    if let Some(deferred) = deferred_parts(existing)? {
        let mut merged = proposed.to_vec();
        // Older stored chunks own their bytes. Merge waiting coverage and
        // histories newest-first, then restore the canonical row last.
        for waiting in deferred.waiting.into_iter().rev() {
            merged = extend_pending_record(waiting, &merged)?.unwrap_or_else(|| waiting.to_vec());
        }
        if !deferred.canonical.is_empty() {
            merged = extend_pending_record(deferred.canonical, &merged)?
                .unwrap_or_else(|| deferred.canonical.to_vec());
        }
        return Ok(Some(merged));
    }
    let original_parts = pending_records(existing)?;
    let mut scratch = Vec::new();
    // Replay selection already treats SKDM as a carrier, not a new user
    // payload. Extension must use that same occurrence identity while keeping
    // each original row's bytes, including its carrier and unknown fields.
    let existing_fingerprints: Vec<_> = decode_pending_parts(existing)?
        .iter()
        .map(|message| MessageDispatch::fingerprint_into(message, &mut scratch))
        .collect();
    let proposed_parts = pending_records(proposed)?;
    let mut next = 0;
    let mut merged = Vec::with_capacity(proposed_parts.len());
    for part in proposed_parts {
        let matches = if let Some(expected) = existing_fingerprints.get(next) {
            let message = waproto::codec::message_decode(part.bytes)?;
            *expected == MessageDispatch::fingerprint_into(&message, &mut scratch)
        } else {
            false
        };
        if matches {
            merged.push(PendingPart {
                bytes: original_parts[next].bytes,
                sources: {
                    let mut sources = original_parts[next].sources.clone();
                    retention::remember_sources(&mut sources, &part.sources);
                    sources
                },
            });
            next += 1;
        } else {
            merged.push(part);
        }
    }
    anyhow::ensure!(
        next == original_parts.len(),
        "pending identity conflicts with retained payloads"
    );
    if merged.len() == original_parts.len()
        && merged
            .iter()
            .zip(&original_parts)
            .all(|(a, b)| a.sources == b.sources)
    {
        return Ok(None);
    }
    // Existing payload bytes stay intact. New parts or ciphertext identities
    // extend only this opaque envelope; no table migration is involved.
    Ok(Some(encode_pending_source_sets(
        &merged
            .into_iter()
            .map(|part| (part.bytes, part.sources))
            .collect::<Vec<_>>(),
    )))
}

fn is_subsequence(sequence: &[DispatchFingerprint], candidate: &[DispatchFingerprint]) -> bool {
    let mut remaining = sequence;
    for fingerprint in candidate {
        if remaining.first() == Some(fingerprint) {
            remaining = &remaining[1..];
        }
    }
    remaining.is_empty()
}

// Earliest and latest embeddings agree exactly when every occurrence's
// position is unambiguous. Equal payloads alone never select an occurrence.
fn unique_subsequence_positions(
    sequence: &[DispatchFingerprint],
    candidate: &[DispatchFingerprint],
) -> Option<Vec<usize>> {
    let mut positions = Vec::with_capacity(sequence.len());
    let mut next = 0;
    for fingerprint in sequence {
        let offset = candidate[next..]
            .iter()
            .position(|part| part == fingerprint)?;
        next += offset;
        positions.push(next);
        next += 1;
    }
    let mut end = candidate.len();
    for (fingerprint, earliest) in sequence.iter().zip(&positions).rev() {
        let latest = candidate[..end]
            .iter()
            .rposition(|part| part == fingerprint)?;
        if latest != *earliest {
            return None;
        }
        end = latest;
    }
    Some(positions)
}

pub(super) struct PendingReplay {
    pub(super) items: Vec<InboundMessage>,
    pub(super) waiting: Vec<Vec<InboundMessage>>,
    pub(super) keys: Vec<(String, String, String)>,
    pub(super) sources: std::collections::HashMap<usize, retention::PayloadSources>,
}

// Keep the string comparisons out of the stable sort's generated inner loops.
#[inline(never)]
fn compare_pending_senders(a: &str, b: &str, preferred: &str) -> std::cmp::Ordering {
    (a != preferred, a).cmp(&(b != preferred, b))
}

impl Client {
    /// Load all matching original keys before decrypting another delivery.
    /// Other namespaces/participants are filtered before decoding; their rows
    /// cannot block or be removed by this consumer's successful commit.
    pub(super) async fn load_pending_inbound(
        &self,
        info: &Arc<MessageInfo>,
    ) -> anyhow::Result<PendingReplay> {
        let backend = self.persistence_manager.backend();
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        let group = info.source.chat.is_group()
            || info.source.chat.is_broadcast_list()
            || info.source.chat.is_status_broadcast();
        let mut rows = if group {
            backend
                .get_pending_inbound_for_message(&chat, &info.id)
                .await?
        } else {
            backend
                .get_pending_inbound(&chat, &sender, &info.id)
                .await?
                .map(|bytes| (sender.clone(), bytes))
                .into_iter()
                .collect()
        };
        rows.retain(|(stored_sender, _)| {
            stored_sender == &sender
                || group
                    && stored_sender
                        .parse::<Jid>()
                        .is_ok_and(|stored| stored.to_non_ad() == info.source.sender.to_non_ad())
        });
        // Prefer exact spelling when equivalent rows have equal source coverage.
        rows.sort_by(|a, b| compare_pending_senders(&a.0, &b.0, &sender));
        let mut replay = PendingReplay {
            items: Vec::new(),
            waiting: Vec::new(),
            keys: Vec::new(),
            sources: std::collections::HashMap::new(),
        };
        let mut sequences = Vec::new();
        let mut candidates = Vec::new();
        let mut scratch = Vec::new();
        for (stored_sender, bytes) in rows {
            if let Some(deferred) = deferred_parts(&bytes)? {
                for group in deferred.waiting {
                    let mut waiting = Vec::new();
                    for part in plain_pending_records(group)? {
                        let item = InboundMessage::builder()
                            .message(Arc::new(waproto::codec::message_decode(part.bytes)?))
                            .info(Arc::clone(info))
                            .build();
                        if !part.sources.is_empty() {
                            replay
                                .sources
                                .insert(Arc::as_ptr(&item.message) as usize, part.sources);
                        }
                        waiting.push(item);
                    }
                    replay.waiting.push(waiting);
                }
            }
            let mut items = Vec::new();
            for part in pending_records(&bytes)? {
                let item = InboundMessage::builder()
                    .message(Arc::new(waproto::codec::message_decode(part.bytes)?))
                    .info(Arc::clone(info))
                    .build();
                if !part.sources.is_empty() {
                    replay
                        .sources
                        .insert(Arc::as_ptr(&item.message) as usize, part.sources);
                }
                items.push(item);
            }
            sequences.push(
                items
                    .iter()
                    .map(|item| MessageDispatch::fingerprint_into(&item.message, &mut scratch))
                    .collect::<Vec<_>>(),
            );
            candidates.push(items);
            replay
                .keys
                .push((chat.clone(), stored_sender, info.id.to_string()));
        }
        // Prefer a complete sequence with source metadata for partial retries. An exact
        // alias can be only a suffix: [B] must not make stored [A,B] become [B,A].
        // Compare occurrences so [A,A,B] still contains two copies of A.
        if let Some(complete) = (0..sequences.len())
            .filter(|&index| {
                sequences
                    .iter()
                    .all(|sequence| is_subsequence(sequence, &sequences[index]))
            })
            .max_by_key(|&index| {
                (
                    candidates[index]
                        .iter()
                        .filter(|item| {
                            replay
                                .sources
                                .contains_key(&(Arc::as_ptr(&item.message) as usize))
                        })
                        .count(),
                    std::cmp::Reverse(index),
                )
            })
        {
            // Partial aliases can also prove ciphertext identities, provided
            // there is exactly one placement for every occurrence.
            for index in 0..candidates.len() {
                if index == complete {
                    continue;
                }
                let Some(positions) =
                    unique_subsequence_positions(&sequences[index], &sequences[complete])
                else {
                    continue;
                };
                for (old, position) in candidates[index].iter().zip(positions) {
                    let selected = &candidates[complete][position];
                    if let Some(incoming) = replay
                        .sources
                        .get(&(Arc::as_ptr(&old.message) as usize))
                        .cloned()
                    {
                        let sources = replay
                            .sources
                            .entry(Arc::as_ptr(&selected.message) as usize)
                            .or_default();
                        retention::remember_sources(sources, &incoming);
                    }
                }
            }
            replay.items = candidates.swap_remove(complete);
        } else {
            for items in candidates {
                retention::merge_parts(&mut replay.items, items);
            }
            let canonical: Vec<_> = replay
                .items
                .iter()
                .map(|item| MessageDispatch::fingerprint_into(&item.message, &mut scratch))
                .collect();
            anyhow::ensure!(
                sequences
                    .iter()
                    .all(|sequence| is_subsequence(sequence, &canonical)),
                "conflicting pending-inbound part order across sender keys"
            );
        }
        let live: std::collections::HashSet<_> = replay
            .items
            .iter()
            .chain(replay.waiting.iter().flatten())
            .map(|item| Arc::as_ptr(&item.message) as usize)
            .collect();
        replay.sources.retain(|key, _| live.contains(key));
        Ok(replay)
    }

    // Persist successful but unpositioned parts before their Signal keys can
    // reach storage. The consumer cannot replay this envelope without order proof.
    pub(super) async fn persist_awaiting_inbound_order(
        &self,
        info: &Arc<MessageInfo>,
    ) -> anyhow::Result<()> {
        let retention = &self.inbound_commit_batch.retention;
        let Some(snapshot) = retention.pending_order_snapshot(info) else {
            return Ok(());
        };
        let backend = self.persistence_manager.backend();
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        let existing = backend
            .get_pending_inbound(&chat, &sender, &info.id)
            .await?;
        let id = snapshot.id;
        let fresh_len = snapshot.fresh_len;
        let (send, receive) = futures::channel::oneshot::channel();
        self.runtime
            .spawn_blocking(Box::new(move || {
                let records: Vec<_> = snapshot
                    .groups
                    .into_iter()
                    .map(|group| {
                        if group.is_empty() {
                            return Vec::new();
                        }
                        let encoded: Vec<_> = group
                            .into_iter()
                            .map(|(item, sources)| {
                                let mut bytes = Vec::new();
                                waproto::codec::message_encode_into(&item.message, &mut bytes);
                                (bytes, sources)
                            })
                            .collect();
                        encode_pending_source_sets(
                            &encoded
                                .iter()
                                .map(|(bytes, sources)| (bytes.as_slice(), sources.clone()))
                                .collect::<Vec<_>>(),
                        )
                    })
                    .collect();
                let result = append_deferred_parts(existing.as_deref(), &records[0], &records[1..]);
                let _ = send.send(result);
            }))
            .await;
        let record = receive.await??;
        backend
            .store_pending_inbound(&chat, &sender, &info.id, &record)
            .await?;
        retention.pending_order_persisted(info, id, fresh_len);
        Ok(())
    }

    /// The registered inbound durability hook, if any. `None` (default) keeps
    /// the existing at-most-once acknowledgement path.
    pub(crate) fn inbound_durability_hook(&self) -> Option<Arc<dyn InboundDurabilityHook>> {
        self.inbound_durability_hook.get().cloned()
    }

    /// Redelivery path: when the server replays an already-decrypted message
    /// (`DuplicatedMessage`), re-commit it from the buffered copy instead of
    /// acking. The replay routes through the commit batcher: during a drain it
    /// joins the accumulating batch, so its hook commit, ack and event keep
    /// arrival order with the fresh stanzas around it; live it commits
    /// immediately with all stored parts. A successful batch commit clears its
    /// pending row, and consumers observe the message there.
    /// Usually its original batch never dispatched (the hook failed then); if
    /// it did (post-commit row cleanup failed AND the ack was lost), event
    /// consumers can see it twice, so commits must tolerate duplicate
    /// `Event::Messages` with a hook registered. A plain ack is sent only for
    /// a genuine duplicate (no buffered copy). A read failure fails closed
    /// (no ack) so a transient storage error cannot drop a message that still
    /// needs its hook to commit.
    ///
    /// Returns `true` when a buffered copy was replayed *and* its commit will
    /// dispatch it, which hands the message to consumers again: a caller
    /// counting suppressions must not count that one. A replay whose commit
    /// failed returns `false`, because nothing reached a consumer.
    pub(crate) async fn ack_or_replay_to_hook(self: &Arc<Self>, info: &Arc<MessageInfo>) -> bool {
        if self.inbound_durability_hook().is_some() {
            if self.inbound_commit_batch.retention.has_plaintext(info) {
                if let Some(items) = self.inbound_commit_batch.retention.replay_items(info) {
                    return !matches!(
                        self.commit_or_batch_inbound_items(items, false).await,
                        InboundCommitState::Failed
                    );
                }
                // Its complete in-memory stanza will commit at the end of this
                // receive, even when a storage failure left no pending row.
                return false;
            }
            match self.load_pending_inbound(info).await {
                Ok(replay) if !replay.items.is_empty() => {
                    if self.inbound_commit_batch.retention.is_collecting(info) {
                        self.inbound_commit_batch.retention.seed_replay(replay);
                        return false; // The outer producer seals every part together.
                    }
                    self.inbound_commit_batch.retention.begin(info, None).await;
                    let _collection = self.inbound_commit_batch.retention.collection_guard(info);
                    self.inbound_commit_batch.retention.seed_replay(replay);
                    let (items, _) = self
                        .inbound_commit_batch
                        .retention
                        .seal(info, self.inbound_commit_batch.is_active());
                    return !matches!(
                        self.commit_or_batch_inbound_items(items, false).await,
                        InboundCommitState::Failed
                    );
                }
                Ok(_) => {
                    if !self.inbound_commit_batch.retention.has_plaintext(info) {
                        self.ack_received_message(info);
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Pending inbound lookup failed; preserving all records and withholding receipt: {error:?}"
                    );
                }
            }
        } else {
            self.ack_received_message(info);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn deferred_waiting_only_chunk_keeps_original_bytes() {
        let a = [10, 1, b'A'];
        let b = [10, 1, b'B', 192, 62, 7];
        let with_carrier = [&b[..], &[0x12, 0]].concat();
        let c = [10, 1, b'C'];
        let waiting = encode_pending_parts(&[&with_carrier]);
        let deferred = append_deferred_parts(None, &[], &[waiting]).unwrap();
        let complete = extend_pending_record(&deferred, &encode_pending_parts(&[&a, &b, &c]))
            .unwrap()
            .unwrap();
        assert_eq!(
            pending_parts(&complete).unwrap(),
            [&a[..], &with_carrier[..], &c[..]]
        );
    }

    #[test]
    fn deferred_waiting_carrier_cannot_replace_canonical_chunk() {
        let a = [10, 1, b'A'];
        let b = [10, 1, b'B', 192, 62, 7];
        let with_carrier = [&b[..], &[0x12, 0]].concat();
        let c = [10, 1, b'C'];
        let canonical = encode_pending_source_sets(&[(&b, smallvec::smallvec![[1; 32]])]);
        let waiting = encode_pending_source_sets(&[(&with_carrier, smallvec::smallvec![[2; 32]])]);
        let deferred = append_deferred_parts(None, &canonical, &[waiting]).unwrap();
        let complete = extend_pending_record(&deferred, &encode_pending_parts(&[&a, &b, &c]))
            .unwrap()
            .unwrap();
        assert_eq!(pending_parts(&complete).unwrap(), [&a[..], &b[..], &c[..]]);
        assert_eq!(
            pending_records(&complete).unwrap()[1].sources.as_slice(),
            &[[1; 32], [2; 32]]
        );
    }

    #[test]
    fn deferred_completion_keeps_unknown_chunks_and_rejects_missing_parts() {
        let a = [10, 1, b'A'];
        let b = [10, 1, b'B', 192, 62, 7];
        let c = [10, 1, b'C'];
        let canonical = encode_pending_parts(&[&a]);
        let waiting = encode_pending_source_sets(&[(&b, smallvec::smallvec![[3; 32]])]);
        let deferred =
            append_deferred_parts(None, &canonical, std::slice::from_ref(&waiting)).unwrap();
        assert_eq!(
            deferred_parts(&deferred).unwrap().unwrap().canonical,
            canonical
        );
        assert!(extend_pending_record(&deferred, &encode_pending_parts(&[&a, &c])).is_err());
        let wrong_future = [10, 1, b'B', 192, 62, 8];
        assert!(
            extend_pending_record(&deferred, &encode_pending_parts(&[&a, &wrong_future, &c]))
                .is_err()
        );
        let complete = extend_pending_record(&deferred, &encode_pending_parts(&[&a, &b, &c]))
            .unwrap()
            .unwrap();
        assert_eq!(pending_parts(&complete).unwrap(), [&a[..], &b[..], &c[..]]);
        assert!(deferred_parts(&complete).unwrap().is_none());
        let repeated_store =
            append_deferred_parts(Some(&deferred), &canonical, &[waiting]).unwrap();
        assert_eq!(
            repeated_store, deferred,
            "cancellation after a successful write must not duplicate the exact waiting generation"
        );
    }

    #[test]
    fn deferred_parser_bounds_groups_and_rejects_nesting() {
        let canonical = encode_pending_parts(&[&[10, 1, b'A']]);
        let waiting = encode_pending_parts(&[&[10, 1, b'B']]);
        let deferred = encode_deferred_parts(&canonical, &[&waiting]);
        assert!(deferred_parts(&encode_deferred_parts(&canonical, &[&deferred])).is_err());
        let mut truncated = DEFERRED_PARTS_HEADER.to_vec();
        truncated.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(deferred_parts(&truncated).is_err());
        assert!(
            deferred_parts(&encode_deferred_parts(
                &canonical,
                &vec![waiting.as_slice(); retention::MAX_SOURCE_IDENTITIES + 1]
            ))
            .is_err()
        );
        assert_eq!(
            pending_parts(&encode_deferred_parts(&[], &[&waiting]))
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            pending_parts(&[]).unwrap().len(),
            1,
            "empty legacy protobuf remains one message"
        );
    }

    use super::*;
    use crate::test_utils::create_test_client_with_failing_http;
    use crate::types::message::MessageInfo;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use wacore::types::events::BatchOrigin;

    #[test]
    fn pending_record_reads_legacy_and_preserves_opaque_parts() {
        // Framing never interprets protobuf tags or enum values.
        let unknown = [0xc0, 0x3e, 7];
        assert_eq!(pending_parts(&unknown).unwrap(), [&unknown[..]]);
        let opaque = encode_pending_parts(&[&unknown, &[], &unknown]);
        assert_eq!(
            pending_parts(&opaque).unwrap(),
            [&unknown[..], &[], &unknown[..]]
        );
        let wire = [10, 1, b'x'];
        let legacy = decode_pending_parts(&wire).unwrap();
        let record = encode_pending_parts(&[&wire, &[], &wire]);
        let decoded = decode_pending_parts(&record).unwrap();
        assert_eq!(decoded.len(), 3);
        for message in [&legacy[0], &decoded[0], &decoded[2]] {
            let mut encoded = Vec::new();
            waproto::codec::message_encode_into(message, &mut encoded);
            assert_eq!(encoded, wire);
        }
        assert!(decode_pending_parts(PARTS_HEADER).is_err());
        assert!(decode_pending_parts(b"\0WAPI\x02").is_err());
        assert!(decode_pending_parts(&record[..record.len() - 1]).is_err());
        let mut oversized = PARTS_HEADER.to_vec();
        oversized.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(decode_pending_parts(&oversized).is_err());
    }

    #[test]
    fn sourced_pending_records_preserve_bytes_and_metadata_on_extension() {
        let first = [10, 1, b'a', 0xc0, 0x3e, 7];
        let second = [10, 1, b'b'];
        let third = [10, 1, b'c'];
        let original =
            encode_pending_sourced_parts(&[(&first, Some([1; 32])), (&third, Some([3; 32]))]);
        let proposed = encode_pending_sourced_parts(&[
            (&first, Some([9; 32])),
            (&second, Some([2; 32])),
            (&third, Some([8; 32])),
        ]);
        let extended = extend_pending_record(&original, &proposed)
            .unwrap()
            .unwrap();
        let parts = pending_records(&extended).unwrap();
        assert_eq!(
            parts.iter().map(|p| p.bytes).collect::<Vec<_>>(),
            [&first[..], &second[..], &third[..]]
        );
        assert_eq!(
            parts
                .iter()
                .map(|p| p.sources.first().copied())
                .collect::<Vec<_>>(),
            [Some([1; 32]), Some([2; 32]), Some([3; 32])]
        );
        assert!(
            extend_pending_record(&extended, &extended)
                .unwrap()
                .is_none()
        );
        for length in 0..33 {
            let mut invalid = SOURCED_PARTS_HEADER.to_vec();
            invalid.push(1);
            invalid.extend_from_slice(&[0; 32][..length.min(32)]);
            assert!(pending_records(&invalid).is_err());
        }
        let mut invalid = SOURCED_PARTS_HEADER.to_vec();
        invalid.push(2);
        assert!(pending_records(&invalid).is_err());
    }

    #[test]
    fn pending_source_history_preserves_both_ciphertext_generations() {
        let bytes = [10, 1, b'a', 0xc0, 0x3e, 7];
        let existing = encode_pending_sourced_parts(&[(&bytes, Some([1; 32]))]);
        let proposed = encode_pending_sourced_parts(&[(&bytes, Some([2; 32]))]);
        let extended = extend_pending_record(&existing, &proposed)
            .unwrap()
            .unwrap();
        assert!(extended.starts_with(SOURCE_SETS_HEADER));
        let parts = pending_records(&extended).unwrap();
        assert_eq!(parts[0].bytes, bytes);
        assert_eq!(parts[0].sources.as_slice(), &[[1; 32], [2; 32]]);
        assert!(
            extend_pending_record(&extended, &proposed)
                .unwrap()
                .is_none()
        );
        let mut malformed = SOURCE_SETS_HEADER.to_vec();
        malformed.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(pending_records(&malformed).is_err());
    }

    #[test]
    fn saturated_source_history_keeps_original_bytes_without_growth() {
        let bytes = [10, 1, b'a', 0xc0, 0x3e, 7];
        let sources: retention::PayloadSources = (0..retention::MAX_SOURCE_IDENTITIES)
            .map(|index| {
                let mut source = [0; 32];
                source[..8].copy_from_slice(&(index as u64).to_be_bytes());
                source
            })
            .collect();
        let existing = encode_pending_source_sets(&[(&bytes, sources.clone())]);
        let proposed = encode_pending_sourced_parts(&[(&bytes, Some([255; 32]))]);
        assert!(
            extend_pending_record(&existing, &proposed)
                .unwrap()
                .is_none()
        );
        let parts = pending_records(&existing).unwrap();
        assert_eq!(parts[0].bytes, bytes);
        assert_eq!(parts[0].sources, sources);
        let mut oversized = SOURCE_SETS_HEADER.to_vec();
        oversized.extend_from_slice(&((retention::MAX_SOURCE_IDENTITIES + 1) as u64).to_be_bytes());
        oversized.extend(vec![0; (retention::MAX_SOURCE_IDENTITIES + 1) * 32]);
        oversized.extend_from_slice(&0u64.to_be_bytes());
        assert!(
            pending_records(&oversized)
                .err()
                .unwrap()
                .to_string()
                .contains("source history limit")
        );
    }

    #[test]
    fn pending_records_differing_only_in_future_fields_do_not_alias() {
        let original = [10, 1, b'b', 0xc0, 0x3e, 7];
        let different = [10, 1, b'b', 0xc0, 0x3e, 8];
        assert!(extend_pending_record(&original, &different).is_err());
        assert!(extend_pending_record(&different, &original).is_err());
        assert!(
            extend_pending_record(&original, &original)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn pending_extension_uses_replay_carrier_equivalence_and_keeps_bytes() {
        let original = [10, 1, b'b', 0xc0, 0x3e, 7];
        let with_carrier = [&original[..], &[0x12, 0]].concat();
        let first = [10, 1, b'a'];
        let proposed = encode_pending_parts(&[&first, &with_carrier]);
        let extended = extend_pending_record(&original, &proposed)
            .unwrap()
            .unwrap();
        assert_eq!(
            pending_parts(&extended).unwrap(),
            [&first[..], &original[..]]
        );
        assert!(
            extend_pending_record(&original, &with_carrier)
                .unwrap()
                .is_none()
        );
        let duplicate = encode_pending_parts(&[&original, &original]);
        assert!(extend_pending_record(&duplicate, &proposed).is_err());
        assert!(extend_pending_record(&original, &first).is_err());
    }

    async fn partial_alias_sources(repeated: bool) {
        let client = create_test_client_with_failing_http("partial_alias_sources").await;
        let mut info = (*test_info("PARTIAL_ALIAS_SOURCES")).clone();
        info.source.is_group = true;
        let info = Arc::new(info);
        let a = [10, 1, b'a', 0xc0, 0x3e, 7];
        let b = [10, 1, b'b'];
        let canonical = if repeated {
            encode_pending_parts(&[&a, &a, &b])
        } else {
            encode_pending_parts(&[&a, &b])
        };
        let partial = encode_pending_sourced_parts(&[(&a, Some([9; 32]))]);
        let backend = client.persistence_manager.backend();
        for (sender, bytes) in [
            ("200@s.whatsapp.net", canonical.as_slice()),
            ("200:1@s.whatsapp.net", partial.as_slice()),
        ] {
            backend
                .store_pending_inbound("100@g.us", sender, &info.id, bytes)
                .await
                .unwrap();
        }
        let replay = client.load_pending_inbound(&info).await.unwrap();
        assert_eq!(replay.items.len(), if repeated { 3 } else { 2 });
        let first = &replay.items[0];
        let sources = replay.sources.get(&(Arc::as_ptr(&first.message) as usize));
        if repeated {
            assert!(
                sources.is_none(),
                "a partial A cannot choose either retained A"
            );
        } else {
            assert_eq!(sources.unwrap().as_slice(), &[[9; 32]]);
        }
        let mut encoded = Vec::new();
        waproto::codec::message_encode_into(&first.message, &mut encoded);
        assert_eq!(encoded, a);
        assert_eq!(replay.keys.len(), 2);
    }
    #[tokio::test]
    async fn compatible_partial_alias_preserves_unique_ciphertext_provenance() {
        partial_alias_sources(false).await;
    }
    #[tokio::test]
    async fn repeated_partial_alias_does_not_guess_ciphertext_provenance() {
        partial_alias_sources(true).await;
    }

    #[tokio::test]
    async fn restarted_alias_replay_with_carrier_difference_reaches_hook() {
        let client = create_test_client_with_failing_http("durability_carrier_alias").await;
        let mut info = (*test_info("CARRIER_ALIAS")).clone();
        info.source.is_group = true;
        let info = Arc::new(info);
        let backend = client.persistence_manager.backend();
        let original = [10, 1, b'b', 0xc0, 0x3e, 7];
        let with_carrier = [&original[..], &[0x12, 0]].concat();
        let combined = encode_pending_parts(&[&[10, 1, b'a'], &with_carrier]);
        for (sender, bytes) in [
            ("200@s.whatsapp.net", &original[..]),
            ("200:1@s.whatsapp.net", &combined[..]),
        ] {
            backend
                .store_pending_inbound("100@g.us", sender, &info.id, bytes)
                .await
                .unwrap();
        }
        let hook = counting_hook(true);
        assert!(client.inbound_durability_hook.set(hook.clone()).is_ok());
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
        assert_eq!(hook.messages.load(Ordering::SeqCst), 2);
        for sender in ["200@s.whatsapp.net", "200:1@s.whatsapp.net"] {
            assert!(
                backend
                    .get_pending_inbound("100@g.us", sender, &info.id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn extending_a_pending_record_keeps_legacy_bytes_and_part_multiplicity() {
        let original = [10, 1, b'x', 0xc0, 0x3e, 7];
        let mut canonical = Vec::new();
        waproto::codec::message_encode_into(
            &decode_pending_parts(&original).unwrap()[0],
            &mut canonical,
        );
        let proposed = encode_pending_parts(&[&canonical, &[10, 1, b'y'], &[10, 1, b'y']]);
        let extended = extend_pending_record(&original, &proposed)
            .unwrap()
            .unwrap();
        assert_eq!(
            pending_parts(&extended).unwrap(),
            [&original[..], &[10, 1, b'y'], &[10, 1, b'y']]
        );
        assert!(extend_pending_record(&original, &[10, 1, b'y']).is_err());
        assert!(
            extend_pending_record(&original, &canonical)
                .unwrap()
                .is_none()
        );
    }

    struct CountingHook {
        calls: AtomicUsize,
        messages: AtomicUsize,
        succeed: AtomicBool,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for CountingHook {
        async fn on_messages(
            &self,
            _client: Arc<Client>,
            batch: &[InboundMessage],
        ) -> anyhow::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.messages.fetch_add(batch.len(), Ordering::SeqCst);
            if self.succeed.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("commit failed"))
            }
        }
    }

    fn counting_hook(succeed: bool) -> Arc<CountingHook> {
        Arc::new(CountingHook {
            calls: AtomicUsize::new(0),
            messages: AtomicUsize::new(0),
            succeed: AtomicBool::new(succeed),
        })
    }

    fn test_info(id: &str) -> Arc<MessageInfo> {
        use crate::types::message::MessageSource;
        Arc::new(MessageInfo {
            id: id.into(),
            source: MessageSource {
                chat: "100@g.us".parse().unwrap(),
                sender: "200@s.whatsapp.net".parse().unwrap(),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn test_item(id: &str) -> InboundMessage {
        InboundMessage::builder()
            .message(Arc::new({
                let mut proto = wa::Message::default();
                proto.conversation = Some("hello".to_string());
                proto
            }))
            .info(test_info(id))
            .build()
    }

    // A successful batch commit acks the messages and clears every buffered copy.
    #[tokio::test]
    async fn commit_ok_clears_buffer() {
        let client = create_test_client_with_failing_http("durability_ok").await;
        let hook = counting_hook(true);
        let _ = client.inbound_durability_hook.set(hook.clone());

        let items: Arc<[InboundMessage]> =
            Arc::from([test_item("MSG_OK_1"), test_item("MSG_OK_2")]);
        let infos: Vec<_> = items.iter().map(|i| Arc::clone(&i.info)).collect();
        client
            .commit_inbound_batch(Arc::clone(&items), BatchOrigin::OfflineDrain, None)
            .await;

        assert_eq!(hook.calls.load(Ordering::SeqCst), 1, "one commit per batch");
        assert_eq!(hook.messages.load(Ordering::SeqCst), 2);
        let backend = client.persistence_manager.backend();
        for info in &infos {
            assert!(
                backend
                    .get_pending_inbound(
                        &info.source.chat.to_string(),
                        &info.source.sender.to_string(),
                        &info.id,
                    )
                    .await
                    .unwrap()
                    .is_none(),
                "a committed message must not stay buffered"
            );
        }
    }

    // A failing batch commit suppresses the acks and keeps every buffered copy;
    // later per-message redeliveries replay each one and, once the hook
    // succeeds, clear them.
    #[tokio::test]
    async fn commit_err_keeps_buffer_then_replays() {
        let client = create_test_client_with_failing_http("durability_err").await;
        let hook = counting_hook(false);
        let _ = client.inbound_durability_hook.set(hook.clone());
        let backend = client.persistence_manager.backend();

        let info = test_info("MSG_ERR");
        client
            .commit_inbound_batch(
                Arc::from([InboundMessage::builder()
                    .message(Arc::new({
                        let mut proto = wa::Message::default();
                        proto.conversation = Some("hello".to_string());
                        proto
                    }))
                    .info(Arc::clone(&info))
                    .build()]),
                BatchOrigin::OfflineDrain,
                None,
            )
            .await;

        assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_some(),
            "a failed commit must keep the message buffered for redelivery"
        );

        // Redelivery while the hook still fails: re-runs but keeps the buffer.
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            2,
            "redelivery must re-run the hook"
        );
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_some(),
            "a still-failing hook keeps the buffered copy"
        );

        // Redelivery once the commit succeeds clears the buffer AND finally
        // dispatches the event (the original batch never did).
        let (handler, rx) = wacore::types::events::ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        hook.succeed.store(true, Ordering::SeqCst);
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(hook.calls.load(Ordering::SeqCst), 3);
        let event = rx.try_recv().expect("successful replay must dispatch");
        assert_eq!(
            event
                .messages()
                .map(|m| m.info.id.as_str())
                .collect::<Vec<_>>(),
            ["MSG_ERR"],
            "consumers must observe a message whose hook only succeeded on replay"
        );
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_none(),
            "a successful replay must clear the buffered copy"
        );
    }

    #[tokio::test]
    async fn pending_replay_preserves_distinct_parts_with_the_same_identity() {
        let client = create_test_client_with_failing_http("durability_parts").await;
        let hook = counting_hook(false);
        let _ = client.inbound_durability_hook.set(hook.clone());
        let first = test_item("MULTIPART");
        let info = Arc::clone(&first.info);
        let mut message = wa::Message::default();
        message.conversation = Some("second distinct part".to_owned());
        let second = InboundMessage::builder()
            .message(Arc::new(message))
            .info(Arc::clone(&info))
            .build();
        assert!(
            client
                .commit_inbound_batch(Arc::from([first, second]), BatchOrigin::OfflineDrain, None,)
                .await
        );
        assert_eq!(hook.messages.swap(0, Ordering::SeqCst), 2);
        hook.succeed.store(true, Ordering::SeqCst);
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert_eq!(
            hook.messages.load(Ordering::SeqCst),
            2,
            "replay must retain every distinct part of one message identity"
        );
    }

    #[tokio::test]
    async fn failed_replay_preserves_original_legacy_wire_bytes() {
        let client = create_test_client_with_failing_http("durability_legacy_wire").await;
        let _ = client.inbound_durability_hook.set(counting_hook(false));
        let info = test_info("LEGACY_WIRE");
        // A known conversation and an opaque unknown field in an existing row.
        let original = [10, 1, b'x', 0xc0, 0x3e, 7];
        let backend = client.persistence_manager.backend();
        backend
            .store_pending_inbound(
                &info.source.chat.to_string(),
                &info.source.sender.to_string(),
                &info.id,
                &original,
            )
            .await
            .unwrap();
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    &info.id
                )
                .await
                .unwrap()
                .unwrap(),
            original
        );
    }

    // A genuine duplicate (no buffered copy) just acks without invoking the hook.
    #[tokio::test]
    async fn replay_without_buffer_just_acks() {
        let client = create_test_client_with_failing_http("durability_dup").await;
        let hook = counting_hook(true);
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.ack_or_replay_to_hook(&test_info("MSG_NONE")).await;
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            0,
            "no buffered copy means the hook must not run"
        );
    }
}
