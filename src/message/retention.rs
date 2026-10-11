//! Client-owned plaintext and admission leases. Neither belongs to a socket generation.
use super::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::types::events::InboundMessage;

// SDK admission policy, not a protocol limit or a measurement of decoded heap.
const MAX_STANZAS: usize = 400;
const MAX_FRAME_BYTES: usize = 4 * 1024 * 1024;
const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);
type Key = (String, String, String);
// Ciphertext identities let a retry's undecrypted duplicates anchor new parts.
// Plaintext fingerprints alone cannot position B when only B decrypts after A,C.
pub(super) type PayloadSource = [u8; 32];
pub(super) type PayloadSources = smallvec::SmallVec<[PayloadSource; 1]>;
// Bound provenance across restarts too. At saturation, an unrecognized retry
// needs a complete fresh sequence instead of guessing from partial plaintext.
pub(super) const MAX_SOURCE_IDENTITIES: usize = MAX_STANZAS;
pub(super) fn remember_sources(sources: &mut PayloadSources, incoming: &[PayloadSource]) {
    for source in incoming {
        if sources.len() == MAX_SOURCE_IDENTITIES {
            break;
        }
        if !sources.contains(source) {
            sources.push(*source);
        }
    }
}
fn part_key(item: &InboundMessage) -> usize {
    Arc::as_ptr(&item.message) as usize
}
pub(super) fn key(info: &MessageInfo) -> Key {
    (
        info.source.chat.to_string(),
        if info.source.chat.is_group()
            || info.source.chat.is_broadcast_list()
            || info.source.chat.is_status_broadcast()
        {
            info.source.sender.to_non_ad().to_string()
        } else {
            info.source.sender.to_string()
        },
        info.id.to_string(),
    )
}
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Default, Debug)]
struct Budget {
    count: usize,
    bytes: usize,
}
#[derive(Debug)]
pub(crate) struct InboundAdmission {
    budget: Arc<Mutex<Budget>>,
    bytes: usize,
}
impl Drop for InboundAdmission {
    fn drop(&mut self) {
        let mut budget = lock(&self.budget);
        budget.count -= 1;
        budget.bytes -= self.bytes;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Collecting,
    Batched,
    Committing(u64),
    Retry,
    Scheduled,
    AwaitingOrder,
}
struct Stanza {
    id: u64,
    items: Vec<InboundMessage>,
    // Parts produced by this delivery, distinct from retained/replayed parts.
    fresh: Vec<InboundMessage>,
    pending_keys: Vec<Key>,
    sources: HashMap<usize, PayloadSources>,
    delivery: Vec<(usize, PayloadSource)>,
    current_source: Option<PayloadSource>,
    ordering_blocked: bool,
    state: State,
    receipt: bool,
    ticket: Option<InboundCommitTicket>,
    admissions: Vec<Arc<InboundAdmission>>,
    commit_waiters: Vec<futures::channel::oneshot::Sender<()>>,
}
// Match occurrences, not a set: [A,A] remains two parts, but another
// delivery of [A,A] contributes no additional copies. Reuse dispatch's SKDM
// equivalence without copying the protobuf schema or dropping unknown fields.
pub(super) fn merge_parts(items: &mut Vec<InboundMessage>, fresh: Vec<InboundMessage>) {
    if items.is_empty() {
        *items = fresh;
        return;
    }
    // Retained occurrences anchor the retry sequence. Appending a newly
    // recovered B after retained A,C would durably publish A,C,B instead of A,B,C.
    let mut prior =
        HashMap::<DispatchFingerprint, (smallvec::SmallVec<[usize; 1]>, usize, usize)>::new();
    let mut scratch = Vec::new();
    for (index, item) in items.iter().enumerate() {
        prior
            .entry(MessageDispatch::fingerprint_into(
                &item.message,
                &mut scratch,
            ))
            .or_default()
            .0
            .push(index);
    }
    let mut merged = Vec::with_capacity(items.len().max(fresh.len()));
    let mut old = std::mem::take(items).into_iter();
    let mut next_old = 0;
    let mut additions = Vec::new();
    let mut anchored = false;
    for item in fresh {
        let fingerprint = MessageDispatch::fingerprint_into(&item.message, &mut scratch);
        let position = if let Some((positions, consumed, seen)) = prior.get_mut(&fingerprint) {
            *seen += 1;
            // An earlier occurrence may already have been emitted before
            // another anchor. Match the next occurrence in sequence order.
            while positions
                .get(*consumed)
                .is_some_and(|position| *position < next_old)
            {
                *consumed += 1;
            }
            let position = positions.get(*consumed).copied();
            if position.is_some() {
                *consumed += 1;
            } else if *seen <= positions.len() {
                // Do not invent copies to reconcile contradictory orders.
                // Pending-row selection verifies that every sequence survives.
                continue;
            }
            position
        } else {
            None
        };
        if let Some(position) = position {
            if position < next_old {
                continue;
            }
            merged.extend(old.by_ref().take(position - next_old));
            merged.append(&mut additions);
            merged.push(old.next().expect("retained occurrence position exists"));
            next_old = position + 1;
            anchored = true;
        } else {
            additions.push(item);
        }
    }
    if anchored {
        merged.append(&mut additions);
        merged.extend(old);
    } else {
        // Disjoint deliveries add new parts after the already-retained sequence.
        merged.extend(old);
        merged.append(&mut additions);
    }
    *items = merged;
}
impl Stanza {
    fn reconcile(&mut self) -> bool {
        let current_position = |item: &InboundMessage| {
            self.sources.get(&part_key(item)).and_then(|sources| {
                self.delivery
                    .iter()
                    .filter(|(_, source)| sources.contains(source))
                    .map(|(index, _)| *index)
                    .min()
            })
        };
        let unsourced = self.items.iter().any(|item| {
            self.sources.get(&part_key(item)).is_none_or(|sources| {
                sources.len() == MAX_SOURCE_IDENTITIES && current_position(item).is_none()
            })
        });
        if self.ordering_blocked
            || (unsourced && !self.fresh.is_empty() && !self.delivery.is_empty())
        {
            // Legacy rows have no ciphertext provenance. A partial retry cannot
            // prove whether equal plaintext is a missing occurrence or a resend.
            // Recover only from a complete fresh sequence; otherwise withhold
            // consumer commit and keep the original row untouched.
            let mut current: Vec<_> = self
                .fresh
                .iter()
                .filter_map(|item| current_position(item).map(|index| (index, item.clone())))
                .collect();
            current.sort_by_key(|(index, _)| *index);
            current.dedup_by_key(|(index, _)| *index);
            if current.len() != self.delivery.len() {
                self.ordering_blocked = true;
                return false;
            }
            let mut candidate: Vec<_> = current.into_iter().map(|(_, item)| item).collect();
            let mut scratch = Vec::new();
            let fingerprints: Vec<_> = candidate
                .iter()
                .map(|item| MessageDispatch::fingerprint_into(&item.message, &mut scratch))
                .collect();
            let mut replacements = Vec::new();
            let mut next = 0;
            for item in &self.items {
                let fingerprint = MessageDispatch::fingerprint_into(&item.message, &mut scratch);
                let Some(offset) = fingerprints[next..].iter().position(|f| *f == fingerprint)
                else {
                    self.ordering_blocked = true;
                    return false;
                };
                next += offset;
                replacements.push((next, item.clone()));
                next += 1;
            }
            for (index, retained) in replacements {
                let incoming = self
                    .sources
                    .get(&part_key(&candidate[index]))
                    .cloned()
                    .unwrap_or_default();
                let sources = self.sources.entry(part_key(&retained)).or_default();
                remember_sources(sources, &incoming);
                candidate[index] = retained;
            }
            self.items = candidate;
            self.fresh.clear();
            self.ordering_blocked = false;
        } else if !self.items.is_empty()
            && !self.fresh.is_empty()
            && self
                .items
                .iter()
                .all(|item| current_position(item).is_some())
            && self
                .fresh
                .iter()
                .all(|item| current_position(item).is_some())
        {
            // Proven ciphertext positions distinguish a newly recovered leading
            // A from a retained equal A at the end of the same retry sequence.
            let mut ordered = std::collections::BTreeMap::new();
            for item in self.items.iter().chain(&self.fresh) {
                if let Some(index) = current_position(item) {
                    ordered.entry(index).or_insert_with(|| item.clone());
                }
            }
            self.items = ordered.into_values().collect();
            self.fresh.clear();
        }

        // A re-encrypted retry can produce equal plaintext but different wire
        // identities. Keep each observed identity on its retained occurrence.
        let mut occurrences =
            HashMap::<DispatchFingerprint, std::collections::VecDeque<usize>>::new();
        let mut scratch = Vec::new();
        for item in &self.items {
            occurrences
                .entry(MessageDispatch::fingerprint_into(
                    &item.message,
                    &mut scratch,
                ))
                .or_default()
                .push_back(part_key(item));
        }
        for item in &self.fresh {
            if let Some(key) = occurrences
                .get_mut(&MessageDispatch::fingerprint_into(
                    &item.message,
                    &mut scratch,
                ))
                .and_then(|keys| keys.pop_front())
                && let Some(incoming) = self.sources.get(&part_key(item)).cloned()
            {
                let retained = self.sources.entry(key).or_default();
                remember_sources(retained, &incoming);
            }
        }
        merge_parts(&mut self.items, std::mem::take(&mut self.fresh));
        let mut positions = HashMap::<PayloadSource, std::collections::VecDeque<usize>>::new();
        for &(index, source) in &self.delivery {
            positions.entry(source).or_default().push_back(index);
        }
        // Rearrange only parts present in this ciphertext sequence. Disjoint
        // deliveries remain appended, and re-encrypted retries use the
        // plaintext occurrence anchors above rather than guessed enc ordinals.
        let mut slots = Vec::new();
        let mut ordered = Vec::new();
        for (slot, item) in self.items.iter().enumerate() {
            if let Some(sources) = self.sources.get(&part_key(item))
                && let Some(index) = sources
                    .iter()
                    .filter_map(|source| positions.get(source).and_then(|p| p.front().copied()))
                    .min()
            {
                for source in sources {
                    if let Some(queue) = positions.get_mut(source)
                        && queue.front() == Some(&index)
                    {
                        queue.pop_front();
                    }
                }
                slots.push(slot);
                ordered.push((index, item.clone()));
            }
        }
        ordered.sort_by_key(|(index, _)| *index);
        for (slot, (_, item)) in slots.into_iter().zip(ordered) {
            self.items[slot] = item;
        }
        let live: std::collections::HashSet<_> = self.items.iter().map(part_key).collect();
        self.sources.retain(|key, _| live.contains(key));
        self.current_source = None;
        true
    }
}
#[derive(Default)]
pub(crate) struct InboundRetention {
    budget: Arc<Mutex<Budget>>,
    next_id: portable_atomic::AtomicU64,
    stanzas: Mutex<HashMap<Key, Stanza>>,
    chat_gates: Mutex<HashMap<String, std::sync::Weak<async_lock::Mutex<()>>>>,
    active: AtomicBool,
    worker_started: AtomicBool,
    drain_retry: AtomicBool,
}
impl InboundRetention {
    pub(crate) fn chat_gate(&self, info: &MessageInfo) -> Arc<async_lock::Mutex<()>> {
        let mut gates = lock(&self.chat_gates);
        gates.retain(|_, gate| gate.strong_count() != 0);
        let entry = gates.entry(info.source.chat.to_string()).or_default();
        if let Some(gate) = entry.upgrade() {
            return gate;
        }
        let gate = Arc::new(async_lock::Mutex::new(()));
        *entry = Arc::downgrade(&gate);
        gate
    }
    pub(crate) fn admit(&self, bytes: usize) -> Option<InboundAdmission> {
        let mut budget = lock(&self.budget);
        // An otherwise empty budget admits one oversized frame. This preserves
        // the transport's supported frame sizes without growing a queue of them.
        if budget.count >= MAX_STANZAS
            || (budget.count != 0 && bytes > MAX_FRAME_BYTES.saturating_sub(budget.bytes))
        {
            return None;
        }
        budget.count += 1;
        budget.bytes += bytes;
        Some(InboundAdmission {
            budget: Arc::clone(&self.budget),
            bytes,
        })
    }
    pub(crate) fn stats(&self) -> (usize, usize) {
        let budget = lock(&self.budget);
        (budget.count, budget.bytes)
    }
    pub(crate) async fn begin(
        &self,
        info: &MessageInfo,
        mut admission: Option<InboundAdmission>,
    ) -> bool {
        loop {
            match self.try_begin(info, &mut admission) {
                Ok(fresh) => return fresh,
                Err(wait) => {
                    // A drain hook can outlive the connection's semaphore.
                    // Preserve its identity until its commit guard settles.
                    let _ = wait.await;
                }
            }
        }
    }
    fn try_begin(
        &self,
        info: &MessageInfo,
        admission: &mut Option<InboundAdmission>,
    ) -> Result<bool, futures::channel::oneshot::Receiver<()>> {
        self.active.store(true, Ordering::Release);
        let mut stanzas = lock(&self.stanzas);
        if let Some(stanza) = stanzas.get_mut(&key(info))
            && matches!(stanza.state, State::Committing(_))
        {
            let (wake, wait) = futures::channel::oneshot::channel();
            stanza.commit_waiters.push(wake);
            return Err(wait);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Some(stanza) = stanzas.get_mut(&key(info)) {
            // The chat gate and processing permit exclude its commit. Preserve
            // prior parts while a re-encrypted resend is examined for new parts.
            stanza.id = id;
            stanza.state = State::Collecting;
            stanza.delivery.clear();
            stanza.current_source = None;
            stanza.admissions.extend(admission.take().map(Arc::new));
            return Ok(false);
        }
        stanzas.insert(
            key(info),
            Stanza {
                id,
                items: Vec::new(),
                fresh: Vec::new(),
                pending_keys: Vec::new(),
                sources: HashMap::new(),
                delivery: Vec::new(),
                current_source: None,
                ordering_blocked: false,
                state: State::Collecting,
                receipt: false,
                ticket: None,
                admissions: admission.take().map(Arc::new).into_iter().collect(),
                commit_waiters: Vec::new(),
            },
        );
        Ok(true)
    }
    pub(super) fn set_delivery<'a>(
        &self,
        info: &MessageInfo,
        payloads: impl Iterator<Item = &'a EncPayload>,
    ) {
        use sha2::{Digest, Sha256};
        let mut delivery: Vec<_> = payloads
            .map(|payload| {
                let mut digest = Sha256::new();
                digest.update(payload.enc_type.as_wire_str().as_bytes());
                digest.update([payload.padding_version]);
                digest.update(&payload.ciphertext);
                (payload.enc_index, digest.finalize().into())
            })
            .collect();
        delivery.sort_by_key(|(index, _)| *index);
        // Hash before taking the shared mutex: independent live chat lanes
        // must not hold each other behind the size of a ciphertext frame.
        let mut stanzas = lock(&self.stanzas);
        if let Some(stanza) = stanzas.get_mut(&key(info)) {
            stanza.delivery = delivery;
        }
    }
    pub(super) fn exclude_carrier(&self, info: &MessageInfo, enc_index: usize) {
        if let Some(stanza) = lock(&self.stanzas).get_mut(&key(info)) {
            stanza.delivery.retain(|(index, _)| *index != enc_index);
        }
    }
    pub(super) fn select_source(&self, info: &MessageInfo, enc_index: usize) {
        let mut stanzas = lock(&self.stanzas);
        if let Some(stanza) = stanzas.get_mut(&key(info)) {
            stanza.current_source = stanza
                .delivery
                .binary_search_by_key(&enc_index, |(i, _)| *i)
                .ok()
                .map(|position| stanza.delivery[position].1);
        }
    }
    pub(super) fn source(&self, item: &InboundMessage) -> PayloadSources {
        lock(&self.stanzas)
            .get(&key(&item.info))
            .and_then(|stanza| stanza.sources.get(&part_key(item)).cloned())
            .unwrap_or_default()
    }
    pub(crate) fn stage(
        &self,
        items: &[InboundMessage],
        track: bool,
    ) -> Option<InboundCommitState> {
        if !self.active.load(Ordering::Acquire) {
            return None;
        }
        let first = items.first()?;
        let mut stanzas = lock(&self.stanzas);
        let stanza = stanzas.get_mut(&key(&first.info))?;
        if stanza.state != State::Collecting {
            return None;
        }
        if let Some(source) = stanza.current_source {
            for item in items {
                stanza
                    .sources
                    .insert(part_key(item), smallvec::smallvec![source]);
            }
        }
        stanza.fresh.extend_from_slice(items);
        let ticket = track.then(|| {
            stanza
                .ticket
                .get_or_insert_with(InboundCommitTicket::new)
                .clone()
        });
        Some(InboundCommitState::Deferred(ticket))
    }
    pub(crate) fn defer_receipt(&self, info: &MessageInfo) -> bool {
        if !self.active.load(Ordering::Acquire) {
            return false;
        }
        let mut stanzas = lock(&self.stanzas);
        if let Some(stanza) = stanzas.get_mut(&key(info))
            && stanza.state == State::Collecting
        {
            stanza.receipt = true;
            return true;
        }
        false
    }
    pub(crate) fn has_plaintext(&self, info: &MessageInfo) -> bool {
        lock(&self.stanzas)
            .get(&key(info))
            .is_some_and(|s| !s.items.is_empty() || !s.fresh.is_empty())
    }
    pub(crate) fn collection_guard(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
    ) -> RetentionCollection {
        let id = lock(&self.stanzas)[&key(info)].id;
        RetentionCollection {
            retention: Arc::clone(self),
            info: Arc::clone(info),
            id,
        }
    }
    pub(crate) fn replay_items(&self, info: &MessageInfo) -> Option<Arc<[InboundMessage]>> {
        let stanzas = lock(&self.stanzas);
        let stanza = stanzas.get(&key(info))?;
        (!matches!(
            stanza.state,
            State::Collecting | State::Committing(_) | State::AwaitingOrder
        ))
        .then(|| stanza.items.clone().into())
    }
    pub(super) fn awaiting_order(&self, info: &MessageInfo) -> bool {
        lock(&self.stanzas)
            .get(&key(info))
            .is_some_and(|stanza| stanza.state == State::AwaitingOrder)
    }
    pub(crate) fn is_collecting(&self, info: &MessageInfo) -> bool {
        lock(&self.stanzas)
            .get(&key(info))
            .is_some_and(|s| s.state == State::Collecting)
    }
    pub(super) fn seed_replay(&self, replay: durability::PendingReplay) {
        let Some(first) = replay.items.first() else {
            return;
        };
        let mut stanzas = lock(&self.stanzas);
        if let Some(stanza) = stanzas.get_mut(&key(&first.info)) {
            let mut scratch = Vec::new();
            if stanza.items.len() == replay.items.len()
                && stanza.items.iter().zip(&replay.items).all(|(old, new)| {
                    let old = MessageDispatch::fingerprint_into(&old.message, &mut scratch);
                    old == MessageDispatch::fingerprint_into(&new.message, &mut scratch)
                })
            {
                for (old, new) in stanza.items.iter().zip(&replay.items) {
                    if let Some(incoming) = replay.sources.get(&part_key(new)) {
                        let sources = stanza.sources.entry(part_key(old)).or_default();
                        remember_sources(sources, incoming);
                    }
                }
            }
            merge_parts(&mut stanza.items, replay.items);
            stanza.sources.extend(replay.sources);
            stanza.pending_keys.extend(replay.keys);
            stanza.pending_keys.sort_unstable();
            stanza.pending_keys.dedup();
        }
    }
    pub(crate) fn seal(&self, info: &MessageInfo, draining: bool) -> (Arc<[InboundMessage]>, bool) {
        let mut stanzas = lock(&self.stanzas);
        let Some(stanza) = stanzas.get_mut(&key(info)) else {
            return (Arc::from([]), false);
        };
        if !stanza.reconcile() {
            stanza.state = State::AwaitingOrder;
            return (Arc::from([]), false);
        }
        if stanza.items.is_empty() {
            let receipt = stanza.receipt;
            stanzas.remove(&key(info));
            return (Arc::from([]), receipt);
        }
        stanza.state = if draining {
            State::Batched
        } else {
            State::Retry
        };
        (stanza.items.clone().into(), false)
    }
    pub(crate) fn batched(&self, items: &[InboundMessage], owner: Option<&RetentionCommit>) {
        let mut stanzas = lock(&self.stanzas);
        for item in items {
            let key = key(&item.info);
            let Some(stanza) = stanzas.get_mut(&key) else {
                continue;
            };
            match stanza.state {
                State::Collecting | State::AwaitingOrder => continue,
                State::Committing(_) => {
                    if !owner.is_some_and(|owner| owner.owns_stanza(&key, stanza)) {
                        continue;
                    }
                    // Publishing the restored stanza relinquishes this commit.
                    // A later Drop/complete must not affect a successor owner.
                    stanza.commit_waiters.clear();
                }
                _ => {}
            }
            stanza.state = State::Batched;
        }
    }
    pub(crate) fn restore_batched(
        &self,
        items: &mut Vec<InboundMessage>,
        owner: Option<&RetentionCommit>,
    ) {
        if !self.is_active() {
            return;
        }
        let mut stanzas = lock(&self.stanzas);
        items.retain(|item| {
            let key = key(&item.info);
            let Some(stanza) = stanzas.get_mut(&key) else {
                return false;
            };
            match stanza.state {
                State::Collecting | State::AwaitingOrder => return false,
                State::Committing(_) => {
                    if !owner.is_some_and(|owner| owner.owns_stanza(&key, stanza)) {
                        return false;
                    }
                    stanza.commit_waiters.clear();
                }
                _ => {}
            }
            stanza.state = State::Batched;
            true
        });
    }
    #[inline]
    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub(crate) fn commit(self: &Arc<Self>, items: &[InboundMessage]) -> Option<RetentionCommit> {
        if !self.is_active() {
            return Some(RetentionCommit(None));
        }
        self.commit_active(items)
    }

    pub(crate) fn commit_active(
        self: &Arc<Self>,
        items: &[InboundMessage],
    ) -> Option<RetentionCommit> {
        debug_assert!(self.is_active());
        let mut stanzas = lock(&self.stanzas);
        let mut keys = Vec::new();
        for item in items {
            let key = key(&item.info);
            if keys.contains(&key) {
                continue;
            }
            let stanza = stanzas.get(&key)?;
            if stanza.ordering_blocked
                || matches!(
                    stanza.state,
                    State::Committing(_) | State::Collecting | State::AwaitingOrder
                )
            {
                return None;
            }
            keys.push(key);
        }
        // A restored entry can be committed again without a new producer.
        // Give each acquisition its own authority over Drop and completion.
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        for key in &keys {
            stanzas.get_mut(key).expect("key checked under lock").state = State::Committing(id);
        }
        let admissions = keys
            .iter()
            .flat_map(|key| stanzas[key].admissions.iter().cloned())
            .collect();
        Some(RetentionCommit(Some(ActiveRetentionCommit {
            retention: Arc::clone(self),
            keys,
            id,
            complete: false,
            _admissions: admissions,
        })))
    }
    #[cfg(test)]
    fn retry_one(self: &Arc<Self>, info: &MessageInfo) -> Option<RetryAttempt> {
        let mut stanzas = lock(&self.stanzas);
        let key = key(info);
        let stanza = stanzas.get_mut(&key)?;
        if stanza.state != State::Retry {
            return None;
        }
        stanza.state = State::Scheduled;
        Some(RetryAttempt::new(self, key, stanza))
    }
    fn retry_items(self: &Arc<Self>) -> Vec<RetryAttempt> {
        let mut stanzas = lock(&self.stanzas);
        stanzas
            .iter_mut()
            .filter_map(|(key, stanza)| {
                if stanza.state != State::Retry {
                    return None;
                }
                stanza.state = State::Scheduled;
                Some(RetryAttempt::new(self, key.clone(), stanza))
            })
            .collect()
    }
}

pub(crate) struct RetentionCollection {
    retention: Arc<InboundRetention>,
    info: Arc<MessageInfo>,
    id: u64,
}
impl Drop for RetentionCollection {
    fn drop(&mut self) {
        let mut stanzas = lock(&self.retention.stanzas);
        let key = key(&self.info);
        if let Some(stanza) = stanzas.get_mut(&key)
            && stanza.id == self.id
            && stanza.state == State::Collecting
        {
            if !stanza.reconcile() {
                stanza.state = State::AwaitingOrder;
                return;
            }
            if stanza.items.is_empty() {
                stanzas.remove(&key);
            } else {
                stanza.state = State::Retry;
            }
        }
    }
}

struct RetryAttempt {
    retention: Arc<InboundRetention>,
    key: Key,
    id: u64,
    items: Arc<[InboundMessage]>,
    _admissions: Vec<Arc<InboundAdmission>>,
}
impl RetryAttempt {
    fn new(retention: &Arc<InboundRetention>, key: Key, stanza: &Stanza) -> Self {
        Self {
            retention: Arc::clone(retention),
            key,
            id: stanza.id,
            items: stanza.items.clone().into(),
            _admissions: stanza.admissions.clone(),
        }
    }
    fn is_current(&self) -> bool {
        lock(&self.retention.stanzas)
            .get(&self.key)
            .is_some_and(|stanza| stanza.id == self.id && stanza.state == State::Scheduled)
    }
}
impl Drop for RetryAttempt {
    fn drop(&mut self) {
        let mut stanzas = lock(&self.retention.stanzas);
        if let Some(stanza) = stanzas.get_mut(&self.key)
            && stanza.id == self.id
            && stanza.state == State::Scheduled
        {
            stanza.state = State::Retry;
        }
    }
}

// An inactive acquisition owns no stanza, admission or ticket. Keep it as an
// empty marker instead of retaining an Arc and locking the map on completion.
pub(crate) struct RetentionCommit(Option<ActiveRetentionCommit>);

impl RetentionCommit {
    #[inline]
    fn owns_stanza(&self, key: &Key, stanza: &Stanza) -> bool {
        self.0
            .as_ref()
            .is_some_and(|commit| commit.owns_stanza(key, stanza))
    }
    #[inline]
    pub(crate) fn owns(&self, info: &MessageInfo) -> bool {
        self.0.as_ref().is_some_and(|commit| commit.owns(info))
    }
    #[inline]
    pub(crate) fn canonical_items(&self, items: Arc<[InboundMessage]>) -> Arc<[InboundMessage]> {
        match &self.0 {
            Some(commit) => commit.canonical_items(items),
            None => items,
        }
    }
    #[inline]
    pub(crate) fn pending_keys(&self) -> Vec<(String, String, String)> {
        self.0
            .as_ref()
            .map_or_else(Vec::new, ActiveRetentionCommit::pending_keys)
    }
    #[inline]
    pub(crate) fn durable(&self) {
        if let Some(commit) = &self.0 {
            commit.durable();
        }
    }
    #[inline]
    pub(crate) fn complete(&mut self) {
        if let Some(commit) = &mut self.0 {
            commit.complete();
        }
    }
}

struct ActiveRetentionCommit {
    retention: Arc<InboundRetention>,
    keys: Vec<Key>,
    id: u64,
    complete: bool,
    _admissions: Vec<Arc<InboundAdmission>>,
}
impl ActiveRetentionCommit {
    fn owns_stanza(&self, key: &Key, stanza: &Stanza) -> bool {
        stanza.state == State::Committing(self.id) && self.keys.contains(key)
    }
    pub(crate) fn owns(&self, info: &MessageInfo) -> bool {
        let key = key(info);
        lock(&self.retention.stanzas)
            .get(&key)
            .is_some_and(|stanza| self.owns_stanza(&key, stanza))
    }
    pub(crate) fn canonical_items(&self, items: Arc<[InboundMessage]>) -> Arc<[InboundMessage]> {
        if self.keys.is_empty() {
            return items;
        }
        let stanzas = lock(&self.retention.stanzas);
        let mut emitted = std::collections::HashSet::new();
        let mut canonical = Vec::new();
        for item in items.iter() {
            let key = key(&item.info);
            if let Some(stanza) = stanzas.get(&key)
                && self.owns_stanza(&key, stanza)
            {
                if emitted.insert(key.clone()) {
                    canonical.extend_from_slice(&stanza.items);
                }
            } else {
                canonical.push(item.clone());
            }
        }
        canonical.into()
    }
    pub(crate) fn pending_keys(&self) -> Vec<(String, String, String)> {
        let stanzas = lock(&self.retention.stanzas);
        self.keys
            .iter()
            .filter_map(|key| {
                stanzas
                    .get(key)
                    .filter(|stanza| self.owns_stanza(key, stanza))
            })
            .flat_map(|stanza| stanza.pending_keys.iter().cloned())
            .collect()
    }
    pub(crate) fn durable(&self) {
        if self.keys.is_empty() {
            return;
        }
        let mut stanzas = lock(&self.retention.stanzas);
        for key in &self.keys {
            if let Some(stanza) = stanzas.get_mut(key)
                && self.owns_stanza(key, stanza)
                && let Some(ticket) = stanza.ticket.take()
            {
                ticket.mark_durable();
            }
        }
    }
    pub(crate) fn complete(&mut self) {
        self.complete = true;
        let mut stanzas = lock(&self.retention.stanzas);
        for key in &self.keys {
            if stanzas
                .get(key)
                .is_some_and(|stanza| self.owns_stanza(key, stanza))
            {
                let mut stanza = stanzas.remove(key).expect("owner checked under lock");
                if let Some(ticket) = stanza.ticket.take() {
                    ticket.mark_durable();
                }
            }
        }
    }
}
impl Drop for ActiveRetentionCommit {
    fn drop(&mut self) {
        if self.complete || self.keys.is_empty() {
            return;
        }
        let mut stanzas = lock(&self.retention.stanzas);
        for key in &self.keys {
            if let Some(stanza) = stanzas.get_mut(key)
                && self.owns_stanza(key, stanza)
            {
                stanza.state = State::Retry;
                stanza.commit_waiters.clear();
            }
        }
    }
}

impl Client {
    pub(crate) fn admit_inbound_stanza(
        &self,
        node: &OwnedNodeRef,
    ) -> Result<Option<InboundAdmission>, ()> {
        if self.inbound_durability_hook().is_none()
            || node
                .attrs()
                .optional_jid("from")
                .is_some_and(|jid| jid.is_newsletter())
        {
            return Ok(None);
        }
        self.inbound_commit_batch
            .retention
            .admit(node.backing_bytes().len())
            .map(Some)
            .ok_or(())
    }

    pub(crate) fn start_inbound_recovery(self: &Arc<Self>) {
        if self
            .inbound_commit_batch
            .retention
            .worker_started
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let weak = Arc::downgrade(self);
        let runtime = self.runtime.clone();
        self.runtime
            .spawn(Box::pin(async move {
                loop {
                    runtime.sleep(RETRY_INTERVAL).await;
                    let Some(client) = weak.upgrade() else {
                        return;
                    };
                    if client.shutdown_signal().is_fired() {
                        return;
                    }
                    let retention = &client.inbound_commit_batch.retention;
                    // A failed old drain can restore entries after a newer
                    // connection has already entered live mode.
                    if client.inbound_commit_batch.has_entries()
                        && !retention.drain_retry.swap(true, Ordering::AcqRel)
                    {
                        let client = Arc::clone(&client);
                        client
                            .clone()
                            .runtime
                            .spawn(Box::pin(async move {
                                let _reset = scopeguard::guard(
                                    Arc::clone(&client.inbound_commit_batch.retention),
                                    |retention| {
                                        retention.drain_retry.store(false, Ordering::Release);
                                    },
                                );
                                let _ = client
                                    .flush_inbound_commits_under_permit(false, None, None)
                                    .await;
                            }))
                            .detach();
                    }
                    for attempt in retention.retry_items() {
                        let client = Arc::clone(&client);
                        client
                            .clone()
                            .runtime
                            .spawn(Box::pin(async move {
                                // The existing semaphore retains drain/live concurrency;
                                // a slow hook does not own a separate global commit lock.
                                let _chat = client
                                    .inbound_commit_batch
                                    .retention
                                    .chat_gate(&attempt.items[0].info)
                                    .lock_arc()
                                    .await;
                                let _permit = client.acquire_message_processing_permit().await;
                                if attempt.is_current() {
                                    client
                                        .commit_or_batch_inbound_items(
                                            Arc::clone(&attempt.items),
                                            false,
                                        )
                                        .await;
                                }
                            }))
                            .detach();
                    }
                }
            }))
            .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn item(id: &str, body: &str) -> InboundMessage {
        let mut message = wa::Message::default();
        message.conversation = Some(body.to_owned());
        let info = MessageInfo {
            id: id.into(),
            ..Default::default()
        };
        InboundMessage::builder()
            .message(Arc::new(message))
            .info(Arc::new(info))
            .build()
    }
    #[tokio::test]
    async fn saturated_history_requires_complete_retry_and_keeps_retained_payload() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("saturated-source", "A");
        let second = item("saturated-source", "B");
        retention.begin(&first.info, None).await;
        retention.stage(std::slice::from_ref(&first), false);
        retention.seal(&first.info, false);
        {
            let mut stanzas = lock(&retention.stanzas);
            let stanza = stanzas.get_mut(&key(&first.info)).unwrap();
            stanza.sources.insert(
                part_key(&first),
                (0..MAX_SOURCE_IDENTITIES)
                    .map(|index| {
                        let mut source = [0; 32];
                        source[..8].copy_from_slice(&(index as u64).to_be_bytes());
                        source
                    })
                    .collect(),
            );
        }
        retention.begin(&first.info, None).await;
        {
            let mut stanzas = lock(&retention.stanzas);
            let stanza = stanzas.get_mut(&key(&first.info)).unwrap();
            stanza.delivery = vec![(0, [254; 32]), (1, [255; 32])];
            stanza.current_source = Some([255; 32]);
        }
        retention.stage(std::slice::from_ref(&second), false);
        assert!(retention.seal(&first.info, false).0.is_empty());
        retention.begin(&first.info, None).await;
        {
            let mut stanzas = lock(&retention.stanzas);
            let stanza = stanzas.get_mut(&key(&first.info)).unwrap();
            stanza.delivery = vec![(0, [254; 32]), (1, [255; 32])];
            stanza.current_source = Some([254; 32]);
        }
        let fresh_first = item("saturated-source", "A");
        retention.stage(std::slice::from_ref(&fresh_first), false);
        lock(&retention.stanzas)
            .get_mut(&key(&first.info))
            .unwrap()
            .current_source = Some([255; 32]);
        retention.stage(std::slice::from_ref(&second), false);
        let (items, _) = retention.seal(&first.info, false);
        assert_eq!(items.len(), 2);
        assert!(Arc::ptr_eq(&items[0].message, &first.message));
        assert_eq!(items[1].message.conversation.as_deref(), Some("B"));
        assert_eq!(retention.source(&items[0]).len(), MAX_SOURCE_IDENTITIES);
    }

    #[tokio::test]
    async fn unsourced_fresh_parts_survive_a_sourced_retry() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("mixed-source", "A");
        let second = item("mixed-source", "B");
        retention.begin(&first.info, None).await;
        {
            let mut stanzas = lock(&retention.stanzas);
            let stanza = stanzas.get_mut(&key(&first.info)).unwrap();
            stanza.delivery = vec![(0, [1; 32])];
            stanza.current_source = Some([1; 32]);
        }
        retention.stage(std::slice::from_ref(&first), false);
        retention.seal(&first.info, false);
        retention.begin(&first.info, None).await;
        lock(&retention.stanzas)
            .get_mut(&key(&first.info))
            .unwrap()
            .delivery = vec![(0, [1; 32])];
        retention.stage(std::slice::from_ref(&second), false);
        let (items, _) = retention.seal(&first.info, false);
        assert_eq!(
            items
                .iter()
                .map(|item| item.message.conversation.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["A", "B"]
        );
    }

    #[tokio::test]
    async fn inactive_commit_does_not_own_or_settle_a_later_stanza() {
        let retention = Arc::new(InboundRetention::default());
        let items: Arc<[InboundMessage]> = Arc::from([item("later-active", "first")]);
        let mut inactive = retention.commit(&items).unwrap();
        assert_eq!(Arc::strong_count(&retention), 1);
        assert!(Arc::ptr_eq(
            &inactive.canonical_items(Arc::clone(&items)),
            &items
        ));
        assert!(inactive.pending_keys().is_empty());
        assert!(!inactive.owns(&items[0].info));

        assert!(retention.begin(&items[0].info, retention.admit(123)).await);
        assert!(retention.stage(&items, false).is_some());
        let (sealed, _) = retention.seal(&items[0].info, false);
        inactive.durable();
        inactive.complete();
        drop(inactive);
        assert_eq!(retention.stats(), (1, 123));
        assert!(retention.replay_items(&items[0].info).is_some());
        let mut active = retention.commit(&sealed).unwrap();
        active.complete();
        drop(active);
        assert_eq!(retention.stats(), (0, 0));
    }
    #[test]
    fn retention_identity_preserves_direct_devices_and_pn_lid_namespaces() {
        fn info(chat: &str, sender: &str) -> MessageInfo {
            MessageInfo {
                id: "same-id".into(),
                source: crate::types::message::MessageSource {
                    chat: chat.parse().unwrap(),
                    sender: sender.parse().unwrap(),
                    ..Default::default()
                },
                ..Default::default()
            }
        }
        let group = "120363000000001@g.us";
        assert_eq!(
            key(&info(group, "100000001:75@lid")),
            key(&info(group, "100000001@lid"))
        );
        assert_ne!(
            key(&info(group, "100000001@lid")),
            key(&info(group, "100000001@s.whatsapp.net"))
        );
        let direct = "100000001@lid";
        assert_ne!(
            key(&info(direct, "100000001:75@lid")),
            key(&info(direct, "100000001@lid"))
        );
    }
    #[test]
    fn admission_bounds_count_bytes_and_one_oversized_frame() {
        let retention = InboundRetention::default();
        let mut leases: Vec<_> = (0..MAX_STANZAS)
            .map(|_| retention.admit(1).unwrap())
            .collect();
        assert!(retention.admit(1).is_none());
        leases.pop();
        assert!(retention.admit(1).is_some());
        drop(leases);
        assert_eq!(retention.stats(), (0, 0));
        let full = retention.admit(MAX_FRAME_BYTES).unwrap();
        assert!(retention.admit(1).is_none());
        drop(full);
        let oversized = retention.admit(MAX_FRAME_BYTES + 123).unwrap();
        assert!(retention.admit(1).is_none());
        drop(oversized);
        assert_eq!(retention.stats(), (0, 0));
    }
    #[tokio::test]
    async fn cancelled_commit_and_retry_keep_parts_and_reservation() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("multipart", "first");
        let mut second = item("multipart", "second");
        second.info = Arc::clone(&first.info);
        assert!(retention.begin(&first.info, retention.admit(123)).await);
        assert!(retention.defer_receipt(&first.info));
        retention.stage(&[first.clone(), second], true).unwrap();
        let (items, receipt) = retention.seal(&first.info, false);
        assert!(!receipt);
        assert_eq!(items.len(), 2);
        drop(retention.commit(&items).unwrap());
        assert_eq!(retention.stats(), (1, 123));
        let attempt = retention.retry_one(&first.info).unwrap();
        assert!(retention.retry_one(&first.info).is_none());
        drop(attempt); // Cancellation while waiting for a processing permit.
        let attempt = retention.retry_one(&first.info).unwrap();
        let mut commit = retention.commit(&attempt.items).unwrap();
        commit.complete();
        drop(commit);
        assert_eq!(
            retention.stats(),
            (1, 123),
            "retry still owns its plaintext"
        );
        drop(attempt);
        assert_eq!(retention.stats(), (0, 0));
    }
    #[tokio::test]
    async fn slow_commit_does_not_lock_unrelated_stanzas() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("first", "first");
        let second = item("second", "second");
        for item in [&first, &second] {
            assert!(retention.begin(&item.info, retention.admit(100)).await);
            retention.stage(std::slice::from_ref(item), false).unwrap();
            retention.seal(&item.info, false);
        }
        let stalled = retention.commit(&[first]).unwrap();
        let mut independent = retention.commit(&[second]).unwrap();
        independent.complete();
        drop(independent);
        assert_eq!(retention.stats(), (1, 100));
        drop(stalled);
    }
    #[tokio::test]
    async fn producer_waits_until_prior_commit_succeeds_or_is_cancelled() {
        use futures::FutureExt;
        for completed in [false, true] {
            let retention = Arc::new(InboundRetention::default());
            let first = item("same-identity", "first");
            let second = item("same-identity", "second");
            assert!(retention.begin(&first.info, retention.admit(100)).await);
            retention
                .stage(std::slice::from_ref(&first), false)
                .unwrap();
            let (items, _) = retention.seal(&first.info, false);
            let mut commit = retention.commit(&items).unwrap();
            let mut begin = Box::pin(retention.begin(&first.info, retention.admit(200)));
            assert!(begin.as_mut().now_or_never().is_none());
            assert_eq!(retention.stats(), (2, 300));
            if completed {
                commit.complete();
            }
            drop(commit);
            assert_eq!(begin.await, completed);
            retention.stage(&[second], false).unwrap();
            let (items, _) = retention.seal(&first.info, false);
            let bodies: Vec<_> = items
                .iter()
                .map(|item| item.message.conversation.as_deref().unwrap())
                .collect();
            assert_eq!(
                bodies,
                if completed {
                    vec!["second"]
                } else {
                    vec!["first", "second"]
                }
            );
            let mut commit = retention.commit(&items).unwrap();
            commit.complete();
            drop(commit);
            assert_eq!(retention.stats(), (0, 0));
        }
    }
    #[tokio::test]
    async fn abandoned_collection_preserves_old_and_new_parts_and_multiplicity() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("partial", "A");
        assert!(retention.begin(&first.info, retention.admit(100)).await);
        let guard = retention.collection_guard(&first.info);
        retention
            .stage(std::slice::from_ref(&first), false)
            .unwrap();
        drop(guard);
        let old_attempt = retention.retry_one(&first.info).unwrap();
        assert!(!retention.begin(&first.info, retention.admit(200)).await);
        let guard = retention.collection_guard(&first.info);
        retention
            .stage(&[first.clone(), first.clone(), item("partial", "B")], false)
            .unwrap();
        drop(guard);
        drop(old_attempt);
        let retry = retention.retry_one(&first.info).unwrap();
        assert_eq!(
            retry
                .items
                .iter()
                .map(|item| item.message.conversation.as_deref().unwrap())
                .collect::<Vec<_>>(),
            ["A", "A", "B"]
        );
        assert_eq!(retention.stats(), (2, 300));
        let mut commit = retention.commit(&retry.items).unwrap();
        commit.complete();
        drop(commit);
        drop(retry);
        assert_eq!(retention.stats(), (0, 0));
    }
    #[tokio::test]
    async fn abandoned_empty_collection_releases_its_admission() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("empty", "A");
        retention.begin(&first.info, retention.admit(100)).await;
        drop(retention.collection_guard(&first.info));
        assert_eq!(retention.stats(), (0, 0));
        assert!(retention.retry_items().is_empty());
    }
    #[tokio::test]
    async fn completed_guard_drop_cannot_remove_a_newer_collection() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("same", "old");
        retention.begin(&first.info, retention.admit(100)).await;
        retention
            .stage(std::slice::from_ref(&first), false)
            .unwrap();
        let (items, _) = retention.seal(&first.info, false);
        let mut old = retention.commit(&items).unwrap();
        old.complete();
        retention.begin(&first.info, retention.admit(200)).await;
        retention.stage(&[item("same", "new")], false).unwrap();
        drop(old);
        assert_eq!(retention.stats(), (1, 200));
        let (new, _) = retention.seal(&first.info, false);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].message.conversation.as_deref(), Some("new"));
        let mut commit = retention.commit(&new).unwrap();
        commit.complete();
        drop(commit);
        assert_eq!(retention.stats(), (0, 0));
    }
    async fn restored_owner_cannot_mutate_successor(complete_old: bool) {
        let retention = Arc::new(InboundRetention::default());
        let first = item("owner-handoff", "A");
        retention.begin(&first.info, retention.admit(100)).await;
        let Some(InboundCommitState::Deferred(Some(ticket))) =
            retention.stage(std::slice::from_ref(&first), true)
        else {
            panic!("tracked stanza must retain its ticket")
        };
        let (items, _) = retention.seal(&first.info, true);
        let mut old = retention.commit(&items).unwrap();
        let mut admission = None;
        let waiting_old = retention
            .try_begin(&first.info, &mut admission)
            .unwrap_err();
        retention.batched(&items, Some(&old));
        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), waiting_old)
                .await
                .unwrap()
                .is_err(),
            "restoration must wake the relinquished owner's waiters"
        );
        let new = retention.commit(&items).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let task = tokio::spawn({
            let entered = entered.clone();
            async move {
                let _new = new;
                entered.notify_one();
                futures::future::pending::<()>().await;
            }
        });
        entered.notified().await;
        assert!(retention.commit(&items).is_none());
        let mut admission = None;
        let mut waiting = Box::pin(
            retention
                .try_begin(&first.info, &mut admission)
                .unwrap_err(),
        );
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        old.durable();
        assert_eq!(ticket.state(), InboundCommitTicketState::Pending);
        if complete_old {
            old.complete();
        }
        drop(old);
        assert!(
            retention.commit(&items).is_none(),
            "a restored old owner must not release its successor"
        );
        assert_eq!(retention.stats(), (1, 100));
        assert_eq!(ticket.state(), InboundCommitTicketState::Pending);
        assert!(futures::poll!(waiting.as_mut()).is_pending());
        task.abort(); // Cancellation must release only this owner for recovery.
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(waiting.await.is_err());
        let mut retry = retention.commit(&items).unwrap();
        retry.complete();
        drop(retry);
        assert_eq!(ticket.state(), InboundCommitTicketState::Durable);
        assert_eq!(retention.stats(), (0, 0));
    }
    #[tokio::test]
    async fn restored_owner_drop_cannot_release_a_new_commit() {
        restored_owner_cannot_mutate_successor(false).await;
    }
    #[tokio::test]
    async fn restored_owner_complete_cannot_remove_a_new_commit() {
        restored_owner_cannot_mutate_successor(true).await;
    }
    #[tokio::test]
    async fn restoring_an_old_snapshot_does_not_seal_a_new_producer() {
        let retention = Arc::new(InboundRetention::default());
        let first = item("same-identity", "first");
        assert!(retention.begin(&first.info, retention.admit(100)).await);
        retention
            .stage(std::slice::from_ref(&first), false)
            .unwrap();
        let (old_items, _) = retention.seal(&first.info, true);
        assert!(!retention.begin(&first.info, retention.admit(100)).await);
        assert!(retention.commit(&old_items).is_none());
        retention.batched(&old_items, None);
        retention
            .stage(&[item("same-identity", "second")], false)
            .unwrap();
        let (items, _) = retention.seal(&first.info, false);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].message.conversation.as_deref(), Some("first"));
        assert_eq!(items[1].message.conversation.as_deref(), Some("second"));
    }
}
