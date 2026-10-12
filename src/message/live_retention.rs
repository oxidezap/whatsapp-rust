//! Live writes own plaintext independently of the offline batch and reconnect reset.

use super::raw_pending::{MAX_PENDING_BYTES, PendingDelivery, StanzaOrigin};
use crate::types::message::MessageInfo;
use std::sync::{Arc, Mutex};
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
    entries: Vec<PendingDelivery>,
    raw: Vec<RawRetained>,
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
    _charge: Arc<Charge>,
}
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

#[derive(Default)]
struct ReceiptState {
    closed: bool,
    failed: bool,
    requested: bool,
    sent: bool,
}
pub(super) struct ReceiptScope {
    info: Arc<MessageInfo>,
    state: Mutex<ReceiptState>,
}
impl ReceiptScope {
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
pub(super) struct ReceiptProducer(Arc<ReceiptScope>);
impl ReceiptProducer {
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
    pub fn begin_receipts(&self, info: &Arc<MessageInfo>) -> anyhow::Result<ReceiptProducer> {
        let mut state = lock(&self.state);
        state.receipts.retain(|scope| scope.strong_count() != 0);
        anyhow::ensure!(
            state.receipts.len() < 400,
            "pending receipt capacity exhausted"
        );
        let scope = Arc::new(ReceiptScope {
            info: Arc::clone(info),
            state: Mutex::new(ReceiptState::default()),
        });
        state.receipts.push(Arc::downgrade(&scope));
        Ok(ReceiptProducer(scope))
    }
    pub fn receipt_for(&self, info: &Arc<MessageInfo>) -> Option<Arc<ReceiptScope>> {
        lock(&self.state)
            .receipts
            .iter()
            .filter_map(std::sync::Weak::upgrade)
            .find(|scope| Arc::ptr_eq(&scope.info, info))
    }
    pub fn fail_receipt(&self, info: &Arc<MessageInfo>) {
        if let Some(scope) = self.receipt_for(info) {
            lock(&scope.state).failed = true;
        }
    }
    pub fn defer_receipt(&self, info: &Arc<MessageInfo>) -> bool {
        self.receipt_for(info).is_some_and(|scope| scope.defer())
    }
    pub fn reserve(
        &self,
        origin: StanzaOrigin,
        wire_bytes: usize,
        slots: usize,
    ) -> anyhow::Result<Admission> {
        // Reserve carrier + projection backing and per-slot bookkeeping, before
        // decrypt. This accounting is separate from transient decoded heap.
        let bytes = wire_bytes
            .checked_mul(2)
            .and_then(|bytes| {
                slots
                    .checked_mul(size_of::<PendingDelivery>() + 256)
                    .and_then(|metadata| bytes.checked_add(metadata))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<Reservation>()))
            .ok_or_else(|| anyhow::anyhow!("live reservation overflow"))?;
        let charge = self.charge(bytes)?;
        let mut state = lock(&self.state);
        state.reservations.push(Reservation {
            origin,
            charge: Arc::clone(&charge),
            receiving: true,
            entries: Vec::new(),
            raw: Vec::new(),
        });
        Ok(Admission {
            state: Arc::clone(&self.state),
            origin,
            _charge: charge,
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
    pub fn retain(&self, item: &mut PendingDelivery) -> anyhow::Result<()> {
        let mut state = lock(&self.state);
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
                let bytes = item
                    .retained_wire_bytes()
                    .checked_add(size_of::<PendingDelivery>() + size_of::<Reservation>())
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
            .map(PendingDelivery::retained_wire_bytes)
            .sum::<usize>();
        anyhow::ensure!(
            used.checked_add(item.retained_wire_bytes())
                .is_some_and(|bytes| bytes <= reservation.charge.bytes),
            "live projection exceeds admission"
        );
        reservation
            .raw
            .retain(|raw| Some(raw.origin) != item.origin);
        reservation.entries.push(item.clone());
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
    pub fn stage(
        &self,
        info: &Arc<MessageInfo>,
        producer: StanzaOrigin,
        slot: (usize, &'static str, u8),
        plaintext: &[u8],
    ) {
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
            plaintext: bytes::Bytes::copy_from_slice(plaintext),
            enc_type: slot.1,
            padding_version: slot.2,
            charge: Arc::clone(&reservation.charge),
        });
    }
    pub fn finish_raw(&self, origin: super::raw_pending::DeliveryOrigin) {
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
    pub fn has_raw_for_message(&self, info: &MessageInfo) -> bool {
        lock(&self.state).reservations.iter().any(|reservation| {
            reservation
                .raw
                .iter()
                .any(|entry| same_message(&entry.info, info))
        })
    }
    pub fn raw_for_message(&self, info: &MessageInfo) -> Vec<RawRetained> {
        lock(&self.state)
            .reservations
            .iter()
            .flat_map(|reservation| &reservation.raw)
            .filter(|entry| same_message(&entry.info, info))
            .cloned()
            .collect()
    }
    pub fn for_message(&self, info: &MessageInfo) -> Vec<PendingDelivery> {
        lock(&self.state)
            .reservations
            .iter()
            .flat_map(|reservation| &reservation.entries)
            .filter(|entry| same_message(&entry.info, info))
            .cloned()
            .collect()
    }
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
fn same_message(first: &MessageInfo, second: &MessageInfo) -> bool {
    first.source.chat == second.source.chat
        && first.id == second.id
        && (first.source.sender == second.source.sender
            || (first.source.chat.is_group()
                && first.source.sender.to_non_ad() == second.source.sender.to_non_ad()))
}

fn same_entry(first: &PendingDelivery, second: &PendingDelivery) -> bool {
    first.origin == second.origin
        && first.storage_sender == second.storage_sender
        && same_message(&first.info, &second.info)
        && Arc::ptr_eq(&first.message, &second.message)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn raw_pending_cancelled_empty_admission_returns_capacity() {
        let retention = LiveRetention::default();
        let admission = retention.reserve(StanzaOrigin::new(), 1024, 1).unwrap();
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
