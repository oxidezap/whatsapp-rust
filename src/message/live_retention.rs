//! Live writes own plaintext independently of the offline batch and reconnect reset.

use super::raw_pending::{MAX_PENDING_BYTES, PendingDelivery, StanzaOrigin};
use crate::types::message::MessageInfo;
use std::sync::{Arc, Mutex};
use wacore::stats::HeapSize;

// Vector growth and the fixed Arc allocation kept alive by a weak message
// reference are resident bookkeeping, even after its decoded fields drop.
const SLOT_BYTES: usize = 4
    * (size_of::<PendingDelivery>() + size_of::<RawRetained>() + size_of::<RetainedDelivery>())
    + size_of::<waproto::whatsapp::Message>()
    + size_of::<MessageInfo>()
    + size_of::<ReceiptScope>()
    + 2 * size_of::<usize>();
use wacore_binary::JidExt;

#[derive(Default)]
pub(super) struct LiveRetention {
    state: Arc<Mutex<State>>,
    budget: Arc<Mutex<Budget>>,
    #[cfg(test)]
    pub(super) pause_stage: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    pub(super) stage_ready: tokio::sync::Notify,
    #[cfg(test)]
    pub(super) forbid_access: std::sync::atomic::AtomicBool,
    #[cfg(test)]
    stage_release: tokio::sync::Notify,
}
#[derive(Default)]
struct Budget {
    bytes: usize,
    admissions: usize,
}
pub(super) struct Charge {
    budget: Arc<Mutex<Budget>>,
    bytes: usize,
}
impl Drop for Charge {
    fn drop(&mut self) {
        let mut budget = lock(&self.budget);
        budget.bytes -= self.bytes;
        budget.admissions -= 1;
    }
}
#[derive(Default)]
struct State {
    reservations: Vec<Reservation>,
    receipts: Vec<std::sync::Weak<ReceiptScope>>,
}
struct Reservation {
    origin: StanzaOrigin,
    charge: Arc<Charge>,
    receiving: bool,
    entries: Vec<RetainedDelivery>,
    raw: Vec<RawRetained>,
}

struct RetainedDelivery {
    info: Arc<MessageInfo>,
    projection: bytes::Bytes,
    message: std::sync::Weak<waproto::whatsapp::Message>,
    ephemeral_expiration: Option<u32>,
    comment_target: Option<bytes::Bytes>,
    carrier: Option<(Option<u32>, bytes::Bytes)>,
    origin: Option<super::raw_pending::DeliveryOrigin>,
    retention: Arc<Charge>,
    receipt: Option<Arc<ReceiptScope>>,
    storage_sender: Option<Arc<str>>,
    consumer_claimed: bool,
}

impl Clone for RetainedDelivery {
    // Share the resident ownership copy across replay iterator transitions.
    #[inline(never)]
    fn clone(&self) -> Self {
        Self {
            info: Arc::clone(&self.info),
            projection: self.projection.clone(),
            message: self.message.clone(),
            ephemeral_expiration: self.ephemeral_expiration,
            comment_target: self.comment_target.clone(),
            carrier: self.carrier.clone(),
            origin: self.origin,
            retention: Arc::clone(&self.retention),
            receipt: self.receipt.clone(),
            storage_sender: self.storage_sender.clone(),
            consumer_claimed: self.consumer_claimed,
        }
    }
}

impl RetainedDelivery {
    #[inline(never)]
    fn new(item: &PendingDelivery, charge: &Arc<Charge>) -> Self {
        let encoded = waproto::codec::message_to_vec(&item.message);
        let projection = match &item.carrier {
            Some((_, original)) if original.as_ref() == encoded => original.clone(),
            _ => encoded.into(),
        };
        Self {
            info: Arc::clone(&item.info),
            projection,
            message: Arc::downgrade(&item.message),
            ephemeral_expiration: item.ephemeral_expiration,
            comment_target: item
                .comment_target
                .as_ref()
                .map(|key| waproto::codec::message_key_to_vec(key).into()),
            carrier: item.carrier.clone(),
            origin: item.origin,
            retention: Arc::clone(charge),
            receipt: item.receipt.clone(),
            storage_sender: item.storage_sender.clone(),
            consumer_claimed: item.consumer_claimed,
        }
    }

    #[inline(never)]
    fn materialize(&self) -> anyhow::Result<PendingDelivery> {
        let mut item = PendingDelivery::from_occurrence(
            super::raw_pending::Occurrence {
                producer: self.origin.map(|origin| origin.stanza),
                index: self.carrier.as_ref().and_then(|(index, _)| *index),
                original: self
                    .carrier
                    .as_ref()
                    .map_or_else(bytes::Bytes::new, |(_, bytes)| bytes.clone()),
                projection: self.projection.clone(),
            },
            self.origin.map_or(0, |origin| origin.position),
            &self.info,
            self.message.upgrade(),
            self.receipt.clone(),
        )?;
        item.item.ephemeral_expiration = self.ephemeral_expiration;
        item.item.comment_target = self
            .comment_target
            .as_ref()
            .map(|bytes| waproto::codec::message_key_decode(bytes))
            .transpose()?
            .map(Box::new);
        item.carrier = self.carrier.clone();
        item.origin = self.origin;
        item.retention = Some(Arc::clone(&self.retention));
        item.storage_sender = self.storage_sender.clone();
        item.consumer_claimed = self.consumer_claimed;
        Ok(item)
    }

    fn retained_wire_bytes(&self) -> usize {
        self.projection
            .len()
            .saturating_add(self.carrier.as_ref().map_or(0, |(_, bytes)| bytes.len()))
            .saturating_add(self.comment_target.as_ref().map_or(0, |bytes| bytes.len()))
    }
}
#[derive(Clone)]
pub(super) struct RawRetained {
    pub info: Arc<MessageInfo>,
    pub origin: super::raw_pending::DeliveryOrigin,
    pub plaintext: bytes::Bytes,
    pub enc_type: &'static str,
    pub padding_version: u8,
    pub charge: Arc<Charge>,
}
pub(super) struct Admission {
    state: Arc<Mutex<State>>,
    origin: StanzaOrigin,
}
#[inline(never)]
fn lock<T>(state: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}
fn remove(state: &mut State, index: usize) {
    state.reservations.remove(index);
}
impl Drop for Admission {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        if let Some(index) = state
            .reservations
            .iter()
            .position(|entry| entry.origin == self.origin)
        {
            state.reservations[index].receiving = false;
            if state.reservations[index].entries.is_empty()
                && state.reservations[index].raw.is_empty()
            {
                remove(&mut state, index);
            }
        }
    }
}
impl Admission {
    pub fn attach_receipts(&self, producer: &ReceiptProducer) {
        // A receiving reservation cannot retire before this guard drops.
        // It owns the charge; the guard needs no second strong reference.
        let charge = lock(&self.state)
            .reservations
            .iter()
            .find(|reservation| reservation.origin == self.origin)
            .map(|reservation| Arc::downgrade(&reservation.charge))
            .expect("active admission owns its reservation");
        lock(&producer.0.state).charge = charge;
    }
}

#[derive(Default)]
struct ReceiptState {
    closed: bool,
    failed: bool,
    requested: bool,
    sent: bool,
    // A queued replay may use an older producer origin with this arrival's
    // metadata. Capture its admission in the delivery before the producer
    // releases it; a receipt scope alone does not own resident payload data.
    charge: std::sync::Weak<Charge>,
}
pub(super) struct ReceiptScope {
    arrival: std::sync::Weak<MessageInfo>,
    state: Mutex<ReceiptState>,
}
impl ReceiptScope {
    pub fn admission(&self) -> Option<Arc<Charge>> {
        lock(&self.state).charge.upgrade()
    }
    pub fn defer(&self) -> bool {
        let mut state = lock(&self.state);
        state.requested = true;
        if state.failed || !state.closed || state.sent {
            return true;
        }
        state.sent = true;
        false
    }
}
// The active producer owns its receipt envelope. Detached/resident delivery
// scopes keep only a weak arrival identity, so they cannot retain an uncharged
// metadata graph after the last corresponding delivery is released.
pub(super) struct ReceiptProducer(Arc<ReceiptScope>, Arc<MessageInfo>);
impl ReceiptProducer {
    pub fn info(&self) -> &Arc<MessageInfo> {
        &self.1
    }
    pub fn finish(&self) -> bool {
        let mut state = lock(&self.0.state);
        state.closed = true;
        if !state.failed && state.requested && !state.sent {
            state.sent = true;
            return true;
        }
        false
    }
}
impl Drop for ReceiptProducer {
    fn drop(&mut self) {
        let mut state = lock(&self.0.state);
        if !state.closed {
            state.closed = true;
            state.failed = true;
        }
    }
}
impl LiveRetention {
    #[inline(never)]
    pub fn has_owned_for_message(&self, info: &MessageInfo) -> bool {
        let state = lock(&self.state);
        state.reservations.iter().any(|reservation| {
            reservation
                .raw
                .iter()
                .any(|entry| same_message(&entry.info, info))
                || reservation
                    .entries
                    .iter()
                    .any(|entry| same_message(&entry.info, info))
        })
    }

    #[inline(never)]
    pub fn encoded_projection(&self, item: &PendingDelivery) -> Option<bytes::Bytes> {
        let state = lock(&self.state);
        for reservation in &state.reservations {
            for entry in &reservation.entries {
                if entry.message.as_ptr() == Arc::as_ptr(&item.message) {
                    return Some(entry.projection.clone());
                }
            }
        }
        None
    }
    #[cfg(test)]
    fn assert_access_allowed(&self) {
        assert!(
            !self
                .forbid_access
                .load(std::sync::atomic::Ordering::Relaxed),
            "durability-disabled processing must not access live retention"
        );
    }
    pub fn begin_receipts(&self, info: &Arc<MessageInfo>) -> anyhow::Result<ReceiptProducer> {
        let mut state = lock(&self.state);
        state.receipts.retain(|scope| scope.strong_count() != 0);
        anyhow::ensure!(
            state.receipts.len() < 400,
            "pending receipt capacity exhausted"
        );
        let original = state
            .reservations
            .iter()
            .flat_map(|entry| &entry.raw)
            .find(|raw| same_message(&raw.info, info))
            .map_or(info, |raw| &raw.info);
        let original = Arc::clone(original);
        let scope = Arc::new(ReceiptScope {
            arrival: Arc::downgrade(info),
            state: Mutex::new(ReceiptState::default()),
        });
        state.receipts.push(Arc::downgrade(&scope));
        Ok(ReceiptProducer(scope, original))
    }
    pub fn receipt_for(&self, info: &Arc<MessageInfo>) -> Option<Arc<ReceiptScope>> {
        #[cfg(test)]
        self.assert_access_allowed();
        lock(&self.state)
            .receipts
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .find(|scope| scope.arrival.as_ptr() == Arc::as_ptr(info))
    }
    pub fn fail_receipt(&self, info: &Arc<MessageInfo>) {
        if let Some(scope) = self.receipt_for(info) {
            lock(&scope.state).failed = true;
        }
    }
    pub fn defer_receipt(&self, info: &Arc<MessageInfo>) -> bool {
        self.receipt_for(info).is_some_and(|scope| scope.defer())
    }
    #[cfg(test)]
    pub fn reserve(
        &self,
        origin: StanzaOrigin,
        wire_bytes: usize,
        slots: usize,
    ) -> anyhow::Result<Admission> {
        self.reserve_sized(origin, wire_bytes, slots, 0)
    }

    #[inline(never)]
    pub fn reserve_with_info(
        &self,
        origin: StanzaOrigin,
        wire_bytes: usize,
        slots: usize,
        info: &MessageInfo,
    ) -> anyhow::Result<Admission> {
        self.reserve_sized(origin, wire_bytes, slots, info_bytes(info))
    }

    fn reserve_sized(
        &self,
        origin: StanzaOrigin,
        wire_bytes: usize,
        slots: usize,
        metadata: usize,
    ) -> anyhow::Result<Admission> {
        // Canonical encoding may expand packed scalar fields to unpacked tags.
        // Message's repeated scalar tags occupy at most two bytes, so a one-byte
        // value can expand to three bytes. Reserve carrier + that projection
        // before decrypt; this remains separate from transient decoded heap.
        let bytes = wire_bytes
            .checked_mul(4)
            .and_then(|bytes| {
                slots
                    .checked_mul(SLOT_BYTES)
                    .and_then(|metadata| bytes.checked_add(metadata))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<Reservation>()))
            .and_then(|bytes| bytes.checked_add(metadata))
            .ok_or_else(|| anyhow::anyhow!("live reservation overflow"))?;
        let charge = self.charge(bytes)?;
        let mut state = lock(&self.state);
        state.reservations.push(Reservation {
            origin,
            charge,
            receiving: true,
            entries: Vec::new(),
            raw: Vec::new(),
        });
        Ok(Admission {
            state: Arc::clone(&self.state),
            origin,
        })
    }
    fn charge(&self, bytes: usize) -> anyhow::Result<Arc<Charge>> {
        let mut budget = lock(&self.budget);
        anyhow::ensure!(
            budget
                .bytes
                .checked_add(bytes)
                .is_some_and(|total| total <= MAX_PENDING_BYTES)
                && budget.admissions < 400,
            "live pending capacity exhausted"
        );
        budget.bytes += bytes;
        budget.admissions += 1;
        Ok(Arc::new(Charge {
            budget: Arc::clone(&self.budget),
            bytes,
        }))
    }
    // Capture the producer reservation once, before receipt fallback can attach
    // the current arrival's charge to a replay from a different producer.
    #[inline(never)]
    pub fn prepare_delivery(
        &self,
        item: &mut PendingDelivery,
        draining: bool,
    ) -> anyhow::Result<()> {
        if item.receipt.is_none() {
            item.receipt = self.receipt_for(&item.info);
        }
        let origin_charge = self.charge_for_origin(item.origin);
        let admitted_origin = origin_charge.is_some();
        if item.retention.is_none() {
            item.retention =
                origin_charge.or_else(|| item.receipt.as_ref().and_then(|scope| scope.admission()));
        }
        if !draining || admitted_origin {
            self.retain(item)?;
        }
        Ok(())
    }

    pub fn retain(&self, item: &mut PendingDelivery) -> anyhow::Result<()> {
        let retained_bytes = item
            .retained_wire_bytes()
            .checked_add(
                item.comment_target
                    .as_ref()
                    .map_or(0, |key| waproto::codec::message_key_encoded_len(key)),
            )
            .ok_or_else(|| anyhow::anyhow!("live retention overflow"))?;
        let mut state = lock(&self.state);
        // Legacy replay has no producer origin. Reuse its existing charge
        // instead of adding another reservation on each failed alias retry.
        if let Some(reservation) = state.reservations.iter().find(|reservation| {
            reservation
                .entries
                .iter()
                .any(|entry| same_entry(entry, item))
        }) {
            anyhow::ensure!(
                item.retention
                    .as_ref()
                    .is_none_or(|charge| Arc::ptr_eq(charge, &reservation.charge)),
                "live delivery has a different reservation owner"
            );
            item.retention = Some(Arc::clone(&reservation.charge));
            return Ok(());
        }
        let index = item.origin.and_then(|origin| {
            state
                .reservations
                .iter()
                .position(|entry| entry.origin == origin.stanza)
        });
        let index = match index {
            Some(index) => index,
            None => {
                // Direct replay/test entry points have no classified admission.
                // Replays already have a durable row; first-receipt production
                // entries use the reservation made before Signal decryption.
                let bytes = retained_bytes
                    .checked_add(info_bytes(&item.info))
                    .and_then(|bytes| bytes.checked_add(SLOT_BYTES + size_of::<Reservation>()))
                    .ok_or_else(|| anyhow::anyhow!("live retention overflow"))?;
                let charge = match item.retention.as_ref() {
                    Some(charge) => Arc::clone(charge),
                    None => self.charge(bytes)?,
                };
                state.reservations.push(Reservation {
                    origin: item
                        .origin
                        .map_or_else(StanzaOrigin::new, |origin| origin.stanza),
                    charge,
                    receiving: false,
                    entries: Vec::new(),
                    raw: Vec::new(),
                });
                state.reservations.len() - 1
            }
        };
        let reservation = &mut state.reservations[index];
        if let Some(charge) = &item.retention {
            anyhow::ensure!(
                Arc::ptr_eq(charge, &reservation.charge),
                "live delivery has a different reservation owner"
            );
        }
        // Storage workers and replay clones keep the reservation charged until
        // their final owner drops, even after the in-memory entry is removed.
        item.retention = Some(Arc::clone(&reservation.charge));
        if reservation
            .entries
            .iter()
            .any(|entry| same_entry(entry, item))
        {
            return Ok(());
        }
        let used = reservation
            .entries
            .iter()
            .map(RetainedDelivery::retained_wire_bytes)
            .sum::<usize>();
        let remaining_raw = reservation
            .raw
            .iter()
            .filter(|raw| Some(raw.origin) != item.origin);
        let raw_bytes = remaining_raw
            .clone()
            .map(|raw| raw.plaintext.len())
            .sum::<usize>();
        let slots = reservation.entries.len() + remaining_raw.count() + 1;
        let mut metadata = 0usize;
        let mut seen: Vec<&Arc<MessageInfo>> = Vec::new();
        for info in reservation
            .entries
            .iter()
            .map(|entry| &entry.info)
            .chain(
                reservation
                    .raw
                    .iter()
                    .filter(|raw| Some(raw.origin) != item.origin)
                    .map(|raw| &raw.info),
            )
            .chain(std::iter::once(&item.info))
        {
            if !seen.iter().any(|previous| Arc::ptr_eq(previous, info)) {
                metadata = metadata
                    .checked_add(info_bytes(info))
                    .ok_or_else(|| anyhow::anyhow!("live metadata overflow"))?;
                seen.push(info);
            }
        }
        drop(seen);
        anyhow::ensure!(
            used.checked_add(retained_bytes)
                .and_then(|bytes| bytes.checked_add(raw_bytes))
                .and_then(|bytes| bytes.checked_add(metadata))
                .and_then(|bytes| slots
                    .checked_mul(SLOT_BYTES)
                    .and_then(|slots| bytes.checked_add(slots)))
                .and_then(|bytes| bytes.checked_add(size_of::<Reservation>()))
                .is_some_and(|bytes| bytes <= reservation.charge.bytes),
            "live projection exceeds admission"
        );
        reservation
            .raw
            .retain(|raw| Some(raw.origin) != item.origin);
        reservation
            .entries
            .push(RetainedDelivery::new(item, &reservation.charge));
        Ok(())
    }
    #[cfg(test)]
    pub fn occupy_remaining_for_tests(&self) -> Arc<Charge> {
        let bytes = MAX_PENDING_BYTES - lock(&self.budget).bytes;
        self.charge(bytes).unwrap()
    }
    #[cfg(test)]
    pub async fn pause_after_stage(&self) {
        if self.pause_stage.load(std::sync::atomic::Ordering::Acquire) {
            self.stage_ready.notify_one();
            self.stage_release.notified().await;
        }
    }
    #[cfg(test)]
    pub fn release_stage_for_tests(&self) {
        self.pause_stage
            .store(false, std::sync::atomic::Ordering::Release);
        self.stage_release.notify_one();
    }
    #[cfg(test)]
    pub fn stage(
        &self,
        info: &Arc<MessageInfo>,
        producer: StanzaOrigin,
        slot: (usize, &'static str, u8),
        plaintext: &[u8],
    ) {
        self.stage_owned(
            info,
            producer,
            slot,
            bytes::Bytes::copy_from_slice(plaintext),
        );
    }

    pub fn stage_owned(
        &self,
        info: &Arc<MessageInfo>,
        producer: StanzaOrigin,
        slot: (usize, &'static str, u8),
        plaintext: bytes::Bytes,
    ) {
        #[cfg(test)]
        self.assert_access_allowed();
        let mut state = lock(&self.state);
        let Some(reservation) = state
            .reservations
            .iter_mut()
            .find(|entry| entry.origin == producer)
        else {
            // Direct test/bench decrypt entry points do not mutate a classified
            // live admission. Production has already reserved this plaintext.
            return;
        };
        let origin = super::raw_pending::DeliveryOrigin {
            stanza: producer,
            position: slot.0,
        };
        if reservation.raw.iter().any(|raw| raw.origin == origin) {
            return;
        }
        reservation.raw.push(RawRetained {
            info: Arc::clone(info),
            origin,
            plaintext,
            enc_type: slot.1,
            padding_version: slot.2,
            charge: Arc::clone(&reservation.charge),
        });
    }
    pub fn finish_raw(&self, origin: super::raw_pending::DeliveryOrigin) {
        #[cfg(test)]
        self.assert_access_allowed();
        let mut state = lock(&self.state);
        for index in (0..state.reservations.len()).rev() {
            let reservation = &mut state.reservations[index];
            reservation.raw.retain(|raw| raw.origin != origin);
            if !reservation.receiving
                && reservation.raw.is_empty()
                && reservation.entries.is_empty()
            {
                remove(&mut state, index);
            }
        }
    }
    #[inline(never)]
    pub fn has_raw_for_message(&self, info: &MessageInfo) -> bool {
        lock(&self.state).reservations.iter().any(|reservation| {
            reservation
                .raw
                .iter()
                .any(|entry| same_message(&entry.info, info))
        })
    }
    #[inline(never)]
    pub fn has_raw_origin(&self, origin: super::raw_pending::DeliveryOrigin) -> bool {
        lock(&self.state)
            .reservations
            .iter()
            .any(|reservation| reservation.raw.iter().any(|entry| entry.origin == origin))
    }
    #[inline(never)]
    pub fn raw_for_message(&self, info: &MessageInfo) -> Vec<RawRetained> {
        lock(&self.state)
            .reservations
            .iter()
            .flat_map(|reservation| &reservation.raw)
            .filter(|entry| same_message(&entry.info, info))
            .cloned()
            .collect()
    }
    #[inline(never)]
    pub fn for_message(&self, info: &MessageInfo) -> anyhow::Result<Vec<PendingDelivery>> {
        #[cfg(test)]
        self.assert_access_allowed();
        let mut entries = Vec::new();
        {
            let state = lock(&self.state);
            for reservation in &state.reservations {
                for entry in &reservation.entries {
                    if same_message(&entry.info, info) {
                        entries.push(entry.clone());
                    }
                }
            }
        }
        let mut deliveries = Vec::with_capacity(entries.len());
        for entry in entries {
            deliveries.push(entry.materialize()?);
        }
        Ok(deliveries)
    }
    #[inline(never)]
    pub fn charge_for_origin(
        &self,
        origin: Option<super::raw_pending::DeliveryOrigin>,
    ) -> Option<Arc<Charge>> {
        let origin = origin?;
        lock(&self.state)
            .reservations
            .iter()
            .find(|reservation| reservation.origin == origin.stanza)
            .map(|reservation| Arc::clone(&reservation.charge))
    }

    pub fn published(&self, items: &[PendingDelivery]) {
        for reservation in &mut lock(&self.state).reservations {
            for entry in &mut reservation.entries {
                if items.iter().any(|item| same_entry(entry, item)) {
                    entry.consumer_claimed = true;
                }
            }
        }
    }

    pub fn stored(&self, items: &[PendingDelivery]) {
        let mut state = lock(&self.state);
        for index in (0..state.reservations.len()).rev() {
            let reservation = &mut state.reservations[index];
            reservation
                .entries
                .retain(|entry| !items.iter().any(|item| same_entry(entry, item)));
            // A replay queued during drain may still own its raw slot. Only
            // release that exact slot after storage succeeded; detached workers
            // retain the same charge through their delivery's storage lease.
            reservation
                .raw
                .retain(|raw| !items.iter().any(|item| item.origin == Some(raw.origin)));
            if !reservation.receiving
                && reservation.entries.is_empty()
                && reservation.raw.is_empty()
            {
                remove(&mut state, index);
            }
        }
    }
}
#[inline(never)]
fn info_bytes(info: &MessageInfo) -> usize {
    size_of::<MessageInfo>() + 2 * size_of::<usize>() + info.heap_bytes()
}
#[inline(never)]
fn same_message(first: &MessageInfo, second: &MessageInfo) -> bool {
    first.source.chat == second.source.chat
        && first.id == second.id
        && if first.source.chat.is_group() {
            first.source.sender.user == second.source.sender.user
                && first.source.sender.server == second.source.sender.server
                && first.source.sender.integrator == second.source.sender.integrator
        } else {
            first.source.sender == second.source.sender
        }
}

#[inline(never)]
fn same_entry(first: &RetainedDelivery, second: &PendingDelivery) -> bool {
    first.origin == second.origin
        && first.storage_sender == second.storage_sender
        && Arc::ptr_eq(&first.info, &second.info)
        && match (&first.carrier, &second.carrier) {
            (None, None) => true,
            (Some((first_index, first_bytes)), Some((second_index, second_bytes))) => {
                first_index == second_index
                    && (first_bytes.len() == second_bytes.len()
                        && first_bytes.as_ptr() == second_bytes.as_ptr()
                        || first_bytes == second_bytes)
            }
            _ => false,
        }
        && (first.message.as_ptr() == Arc::as_ptr(&second.message)
            || second.retention.as_ref().is_some_and(|charge| {
                Arc::ptr_eq(&first.retention, charge)
                    && first.projection.len()
                        == waproto::codec::message_encoded_len(&second.message)
                    && first.projection.as_ref() == waproto::codec::message_to_vec(&second.message)
            }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_pending_projection_preserves_other_slots_backing_budget() {
        let retention = LiveRetention::default();
        let info = Arc::new(MessageInfo::default());
        let producer = StanzaOrigin::new();
        let admission = retention
            .reserve_with_info(producer, 1024, 2, &info)
            .unwrap();
        let plaintext = vec![0; 512];
        retention.stage(&info, producer, (0, "msg", 3), &plaintext);
        retention.stage(&info, producer, (1, "msg", 3), &plaintext);
        let origin = super::super::raw_pending::DeliveryOrigin {
            stanza: producer,
            position: 0,
        };
        let make_item = |length| {
            let mut message = waproto::whatsapp::Message::default();
            message.conversation = Some("a".repeat(length));
            let mut item = PendingDelivery::from(
                wacore::types::events::InboundMessage::builder()
                    .info(Arc::clone(&info))
                    .message(Arc::new(message))
                    .build(),
            );
            item.carrier = Some((Some(0), bytes::Bytes::copy_from_slice(&plaintext)));
            item.origin = Some(origin);
            item.retention = retention.charge_for_origin(Some(origin));
            item
        };
        let mut oversized = make_item(3072);
        assert!(retention.retain(&mut oversized).is_err());
        assert_eq!(retention.raw_for_message(&info).len(), 2);
        assert!(retention.for_message(&info).unwrap().is_empty());
        let mut exact = make_item(3069);
        assert_eq!(exact.retained_wire_bytes() + plaintext.len(), 4096);
        retention.retain(&mut exact).unwrap();
        assert_eq!(retention.raw_for_message(&info).len(), 1);
        assert_eq!(retention.for_message(&info).unwrap().len(), 1);
        drop(admission);
    }

    #[test]
    fn raw_pending_admission_counts_shared_envelope_metadata_once() {
        let retention = LiveRetention::default();
        let mut info = MessageInfo {
            bcl_participants: vec!["100@lid".parse().unwrap(); 1024],
            ..Default::default()
        };
        let bare = retention.reserve(StanzaOrigin::new(), 1024, 3).unwrap();
        let backing = lock(&retention.budget).bytes;
        drop(bare);
        let admission = retention
            .reserve_with_info(StanzaOrigin::new(), 1024, 3, &info)
            .unwrap();
        assert_eq!(lock(&retention.budget).bytes, backing + info_bytes(&info));
        drop(admission);
        info.bcl_participants = vec![
            "100@lid".parse().unwrap();
            MAX_PENDING_BYTES / size_of::<wacore_binary::Jid>() + 1
        ];
        assert!(
            retention
                .reserve_with_info(StanzaOrigin::new(), 1, 1, &info)
                .is_err()
        );
        assert!(lock(&retention.state).reservations.is_empty());
        assert_eq!(lock(&retention.budget).bytes, 0);
    }

    #[test]
    fn raw_pending_resident_projection_does_not_keep_decoded_graph_alive() {
        let retention = LiveRetention::default();
        let info = Arc::new(MessageInfo::default());
        let mut context = waproto::whatsapp::ContextInfo::default();
        context.mentioned_jid = vec![String::new(); 8192];
        let mut text = waproto::whatsapp::message::ExtendedTextMessage::default();
        text.context_info = buffa::MessageField::some(context);
        let mut message = waproto::whatsapp::Message::default();
        message.extended_text_message = buffa::MessageField::some(text);
        let mut target = waproto::whatsapp::MessageKey::default();
        target.id = Some("RETAINED_PARENT".into());
        let mut item = PendingDelivery::from(
            wacore::types::events::InboundMessage::builder()
                .info(Arc::clone(&info))
                .message(Arc::new(message))
                .ephemeral_expiration(3600)
                .comment_target(Box::new(target))
                .build(),
        );
        let weak = Arc::downgrade(&item.message);
        retention.retain(&mut item).unwrap();
        assert_eq!(
            Arc::strong_count(&item.message),
            1,
            "resident retention must not own the decoded graph"
        );
        drop(item);
        assert!(weak.upgrade().is_none());
        let recovered = retention.for_message(&info).unwrap();
        assert_eq!(recovered[0].ephemeral_expiration, Some(3600));
        assert_eq!(
            recovered[0].comment_target.as_ref().unwrap().id.as_deref(),
            Some("RETAINED_PARENT")
        );
        assert_eq!(
            recovered[0]
                .message
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
        retention.published(&recovered);
        drop(recovered);
        let recovered = retention.for_message(&info).unwrap();
        assert!(
            recovered[0].consumer_claimed,
            "rehydration must preserve completed publication"
        );
        retention.stored(&recovered);
        drop(recovered);
        assert!(lock(&retention.state).reservations.is_empty());
        assert_eq!(lock(&retention.budget).bytes, 0);
    }

    #[test]
    fn raw_pending_legacy_alias_retry_reuses_its_reservation() {
        let retention = LiveRetention::default();
        let mut item = PendingDelivery::from(
            wacore::types::events::InboundMessage::builder()
                .info(Arc::new(MessageInfo::default()))
                .message(Arc::new(waproto::whatsapp::Message::default()))
                .build(),
        );
        for _ in 0..500 {
            retention.retain(&mut item).unwrap();
        }
        assert_eq!(lock(&retention.state).reservations.len(), 1);
        assert_eq!(lock(&retention.budget).admissions, 1);
        retention.stored(std::slice::from_ref(&item));
        assert!(lock(&retention.state).reservations.is_empty());
    }

    #[test]
    fn raw_pending_legacy_shared_projection_keeps_distinct_carriers() {
        let retention = LiveRetention::default();
        let mut first = PendingDelivery::from(
            wacore::types::events::InboundMessage::builder()
                .info(Arc::new(MessageInfo::default()))
                .message(Arc::new(waproto::whatsapp::Message::default()))
                .build(),
        );
        first.carrier = Some((None, bytes::Bytes::from_static(b"first")));
        let mut second = first.clone();
        second.carrier = Some((None, bytes::Bytes::from_static(b"second")));
        retention.retain(&mut first).unwrap();
        retention.retain(&mut second).unwrap();
        retention.stored(std::slice::from_ref(&first));
        assert_eq!(retention.for_message(&second.info).unwrap().len(), 1);
        assert_eq!(
            retention.for_message(&second.info).unwrap()[0].carrier,
            second.carrier
        );
    }

    #[test]
    fn raw_pending_drain_cleanup_keeps_detached_worker_charge() {
        let retention = LiveRetention::default();
        let info = Arc::new(MessageInfo::default());
        let producer = StanzaOrigin::new();
        let admission = retention.reserve(producer, 1024, 1).unwrap();
        retention.stage(&info, producer, (0, "skmsg", 3), b"synthetic raw");
        drop(admission);
        let origin = super::super::raw_pending::DeliveryOrigin {
            stanza: producer,
            position: 0,
        };
        let mut message = waproto::whatsapp::Message::default();
        message.conversation = Some("synthetic projection".into());
        let item = PendingDelivery {
            item: wacore::types::events::InboundMessage::builder()
                .info(info.clone())
                .message(Arc::new(message))
                .build(),
            carrier: Some((Some(0), bytes::Bytes::from_static(b"synthetic raw"))),
            origin: Some(origin),
            retention: retention.charge_for_origin(Some(origin)),
            receipt: None,
            storage_sender: None,
            consumer_claimed: false,
        };
        let worker = item.retention.clone().expect("existing live charge");
        assert_eq!(lock(&retention.budget).admissions, 1);
        assert!(!retention.raw_for_message(&info).is_empty());
        retention.stored(std::slice::from_ref(&item));
        assert!(retention.raw_for_message(&info).is_empty());
        assert!(lock(&retention.state).reservations.is_empty());
        drop(item);
        assert_eq!(lock(&retention.budget).admissions, 1);
        drop(worker);
        assert_eq!(lock(&retention.budget).admissions, 0);
        assert_eq!(lock(&retention.budget).bytes, 0);
    }

    #[test]
    fn raw_pending_charge_survives_detached_owner_until_final_drop() {
        let retention = LiveRetention::default();
        let charge = retention.charge(MAX_PENDING_BYTES).unwrap();
        let detached = Arc::clone(&charge);
        drop(charge);
        assert!(retention.charge(1).is_err());
        assert_eq!(lock(&retention.budget).admissions, 1);
        drop(detached);
        let next = retention.charge(MAX_PENDING_BYTES).unwrap();
        assert_eq!(lock(&retention.budget).bytes, MAX_PENDING_BYTES);
        drop(next);
        assert_eq!(lock(&retention.budget).bytes, 0);
        assert_eq!(lock(&retention.budget).admissions, 0);
    }

    #[test]
    fn raw_pending_direct_lookup_preserves_device_and_namespace_identity() {
        let mut info = MessageInfo {
            id: "DIRECT_RAW_IDENTITY".into(),
            ..Default::default()
        };
        info.source.chat = "100@s.whatsapp.net".parse().unwrap();
        info.source.sender = "100:7@s.whatsapp.net".parse().unwrap();
        assert!(same_message(&info, &info.clone()));
        let mut other = info.clone();
        other.source.sender = "100:8@s.whatsapp.net".parse().unwrap();
        assert!(!same_message(&info, &other));
        other.source.sender = "100@s.whatsapp.net".parse().unwrap();
        assert!(!same_message(&info, &other));
        other.source.chat = "100@lid".parse().unwrap();
        other.source.sender = "100:7@lid".parse().unwrap();
        assert!(!same_message(&info, &other));
    }

    #[test]
    fn raw_pending_oversized_admission_does_not_consume_capacity() {
        let retention = LiveRetention::default();
        assert!(
            retention
                .reserve(StanzaOrigin::new(), MAX_PENDING_BYTES, 1)
                .is_err()
        );
        assert!(lock(&retention.state).reservations.is_empty());
        assert_eq!(lock(&retention.budget).bytes, 0);
        assert_eq!(lock(&retention.budget).admissions, 0);
        assert!(retention.reserve(StanzaOrigin::new(), 1024, 1).is_ok());
    }

    #[test]
    fn raw_pending_cancelled_empty_admission_returns_capacity() {
        let retention = LiveRetention::default();
        let admission = retention.reserve(StanzaOrigin::new(), 1024, 1).unwrap();
        // A completed cleanup sweep cannot retire an empty receiving producer.
        retention.stored(&[]);
        assert_eq!(lock(&retention.state).reservations.len(), 1);
        assert_eq!(lock(&retention.budget).admissions, 1);
        drop(admission);
        assert!(lock(&retention.state).reservations.is_empty());
        assert_eq!(lock(&retention.budget).bytes, 0);
        assert_eq!(lock(&retention.budget).admissions, 0);
    }
    #[test]
    fn raw_pending_receipt_cancel_keeps_late_commit_closed() {
        let retention = LiveRetention::default();
        let info = Arc::new(MessageInfo::default());
        let producer = retention.begin_receipts(&info).unwrap();
        let detached_commit = retention.receipt_for(&info).unwrap();
        assert!(retention.defer_receipt(&info));
        drop(producer);
        assert!(retention.defer_receipt(&info));
        assert!(lock(&detached_commit.state).failed);
    }

    #[test]
    fn raw_pending_receipt_late_commit_emits_once_after_success() {
        let retention = LiveRetention::default();
        let info = Arc::new(MessageInfo::default());
        let producer = retention.begin_receipts(&info).unwrap();
        let detached_commit = retention.receipt_for(&info).unwrap();
        assert!(!producer.finish());
        drop(producer);
        assert!(!retention.defer_receipt(&info));
        assert!(retention.defer_receipt(&info));
        assert!(lock(&detached_commit.state).sent);
    }
}
