//! Where the WAM runtime keeps the state a buffer outlives.
//!
//! Two things do: the per-channel sequence number, and the buffers that were
//! built but not yet accepted by the server. The plugin host grants no storage
//! capability and this batch does not propose one: a capability is a promise
//! about every plugin, and one plugin needing a key-value store is not that. So
//! the plugin owns a trait, ships an in-memory implementation, and an embedder
//! that wants durability writes one against its own database.
//!
//! The third thing the official client persists, the daily beaconing roll, is
//! deliberately absent. See the note on [`crate`] for why.

use std::collections::BTreeMap;
use std::sync::Mutex;

use whatsapp_rust::async_trait;
use whatsapp_rust_wam_catalog::Channel;

/// One buffer waiting for the server to accept it.
#[derive(Debug, Clone)]
pub struct PendingBuffer {
    /// Identifies this buffer for as long as it is pending; not a wire value.
    pub key: u64,
    pub channel: Channel,
    pub bytes: Vec<u8>,
}

/// Errors a store can report. Every one is non-fatal to the client: telemetry
/// that cannot be stored is telemetry that is dropped, never a failed send.
#[derive(Debug, thiserror::Error)]
#[error("wam store: {0}")]
pub struct WamStoreError(pub String);

impl WamStoreError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

type Result<T> = std::result::Result<T, WamStoreError>;

/// Persistence for the WAM runtime.
///
/// # What an implementation must guarantee
///
/// - [`next_sequence`](Self::next_sequence) is monotonic within a run and never
///   returns 0: the official client starts at 1 and wraps back to 1 past
///   65535, and a server that has seen a sequence twice cannot order the
///   buffers between them.
/// - [`put_pending`](Self::put_pending) stores a buffer under its `key` without
///   displacing one already held under a different key. The runtime picks a key
///   no retained buffer is using, reading the set it gets back from
///   [`pending`](Self::pending) rather than a counter, so a durable store that
///   survives a restart keeps the buffers it restored. An implementation that
///   would rather own identity may key its rows however it likes, as long as
///   [`pending`](Self::pending) reports back the `key` it was given and
///   [`remove_pending`](Self::remove_pending) accepts it.
/// - Nothing else. Every other method may fail; the runtime treats a failure as
///   "this buffer is lost" and carries on. A [`pending`](Self::pending) that
///   fails is not read as an empty store: the runtime drops the buffer it was
///   about to retain rather than guessing at a key.
///
/// # What it must not do
///
/// Block. Every method is called from a plugin task that also drives the flush
/// timer, so a store that stalls stalls telemetry, which is harmless, but a
/// store that blocks an executor thread is not.
#[async_trait]
pub trait WamStore: Send + Sync + 'static {
    /// Whether this store outlives the process.
    ///
    /// Nothing in the runtime branches on it; it is reported through
    /// [`crate::WamApi::stats`] so an operator can tell a run that will lose its
    /// pending buffers from one that will not.
    fn is_durable(&self) -> bool {
        false
    }

    /// The next sequence number for `channel`, in 1..=65535.
    async fn next_sequence(&self, channel: Channel) -> Result<u16>;

    /// Retain a buffer that has not been accepted yet.
    async fn put_pending(&self, buffer: PendingBuffer) -> Result<()>;

    /// Every retained buffer, oldest first.
    async fn pending(&self) -> Result<Vec<PendingBuffer>>;

    /// Forget a retained buffer, because the server accepted it or the runtime
    /// gave up on it.
    async fn remove_pending(&self, key: u64) -> Result<()>;
}

/// The default store: everything lives in memory and dies with the process.
///
/// Correct for a client that is happy to lose the telemetry of a crashed run,
/// which is most of them, since the buffers are already best-effort. What it costs is
/// sequence continuity, which restarts at 1 exactly as a fresh browser profile
/// does.
#[derive(Debug, Default)]
pub struct InMemoryWamStore {
    inner: Mutex<InMemoryState>,
}

#[derive(Debug, Default)]
struct InMemoryState {
    sequences: BTreeMap<Channel, u16>,
    pending: BTreeMap<u64, PendingBuffer>,
}

impl InMemoryWamStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, InMemoryState> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl WamStore for InMemoryWamStore {
    async fn next_sequence(&self, channel: Channel) -> Result<u16> {
        let mut state = self.lock();
        let slot = state.sequences.entry(channel).or_insert(0);
        // Wraps back to 1, not to 0: the header field is 16 bits and the
        // official client reserves 0 for "no sequence yet".
        *slot = if *slot == u16::MAX { 1 } else { *slot + 1 };
        Ok(*slot)
    }

    async fn put_pending(&self, buffer: PendingBuffer) -> Result<()> {
        self.lock().pending.insert(buffer.key, buffer);
        Ok(())
    }

    async fn pending(&self) -> Result<Vec<PendingBuffer>> {
        Ok(self.lock().pending.values().cloned().collect())
    }

    async fn remove_pending(&self, key: u64) -> Result<()> {
        self.lock().pending.remove(&key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_sequence_starts_at_one_and_never_returns_zero() {
        let store = InMemoryWamStore::new();
        assert_eq!(
            store.next_sequence(Channel::Regular).await.expect("first"),
            1
        );
        assert_eq!(
            store.next_sequence(Channel::Regular).await.expect("second"),
            2
        );
        // Channels number independently.
        assert_eq!(
            store.next_sequence(Channel::Realtime).await.expect("other"),
            1
        );
    }

    #[tokio::test]
    async fn the_sequence_wraps_to_one_rather_than_zero() {
        let store = InMemoryWamStore::new();
        store
            .lock()
            .sequences
            .insert(Channel::Regular, u16::MAX - 1);
        assert_eq!(
            store.next_sequence(Channel::Regular).await.expect("max"),
            u16::MAX
        );
        assert_eq!(
            store.next_sequence(Channel::Regular).await.expect("wrap"),
            1
        );
    }

    #[tokio::test]
    async fn pending_buffers_round_trip_and_can_be_removed() {
        let store = InMemoryWamStore::new();
        store
            .put_pending(PendingBuffer {
                key: 7,
                channel: Channel::Regular,
                bytes: vec![1, 2, 3],
            })
            .await
            .expect("put");
        assert_eq!(store.pending().await.expect("list").len(), 1);
        store.remove_pending(7).await.expect("remove");
        assert!(store.pending().await.expect("list").is_empty());
    }

    #[test]
    fn the_in_memory_store_reports_that_it_does_not_persist() {
        assert!(!InMemoryWamStore::new().is_durable());
    }
}
