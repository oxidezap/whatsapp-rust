/// Ciphertext whose cache advance has not yet crossed its durability barrier.
///
/// ```compile_fail
/// use wacore::session_engine::publication::PreparedSignal;
/// let pending = PreparedSignal::new(vec![1, 2, 3]);
/// let bytes = pending.into_payload();
/// ```
#[must_use]
#[repr(transparent)]
pub struct PreparedSignal<T>(T);

#[must_use]
#[repr(transparent)]
pub struct CommittingSignal<T>(T);

#[must_use]
#[repr(transparent)]
pub struct DurableSignal<T>(T);

/// Evidence supplied by the owning persistence adapter, never by the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDurability {
    DurableLease,
    CommittedWrite,
}

impl<T> PreparedSignal<T> {
    pub fn new(payload: T) -> Self {
        Self(payload)
    }
    pub fn payload(&self) -> &T {
        &self.0
    }
    pub fn begin_commit(self) -> CommittingSignal<T> {
        CommittingSignal(self.0)
    }
}

impl<T> CommittingSignal<T> {
    pub fn confirm(self, _: SignalDurability) -> DurableSignal<T> {
        DurableSignal(self.0)
    }
}

impl<T> DurableSignal<T> {
    pub fn payload(&self) -> &T {
        &self.0
    }
    pub fn into_payload(self) -> T {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_add_no_payload_storage_or_allocation() {
        assert_eq!(size_of::<PreparedSignal<Vec<u8>>>(), size_of::<Vec<u8>>());
        assert_eq!(size_of::<CommittingSignal<Vec<u8>>>(), size_of::<Vec<u8>>());
        assert_eq!(size_of::<DurableSignal<Vec<u8>>>(), size_of::<Vec<u8>>());
        let bytes = vec![7; 1024];
        let allocation = bytes.as_ptr();
        let ready = PreparedSignal::new(bytes)
            .begin_commit()
            .confirm(SignalDurability::CommittedWrite);
        assert_eq!(ready.payload().as_ptr(), allocation);
        assert_eq!(ready.into_payload().len(), 1024);
    }

    #[test]
    fn cancellation_drops_unpublished_payload_once() {
        struct Payload(std::sync::Arc<std::sync::atomic::AtomicUsize>);
        impl Drop for Payload {
            fn drop(&mut self) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let drops = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        drop(PreparedSignal::new(Payload(drops.clone())).begin_commit());
        assert_eq!(drops.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
