//! Write-behind buffer for inbound `messageSecret` persistence.
//!
//! Capturing a secret used to upsert SQLite synchronously inside the per-chat
//! receive lane, before the ack and the `Event::Messages` dispatch. The buffer
//! splits visibility from durability: an insert is immediately readable
//! through [`MsgSecretWriteBuffer::lookup`] (so an add-on referencing the
//! secret of the stanza just processed always finds it), while the backend
//! upsert happens on a detached drain task that coalesces a burst of captures
//! into one batched `put_msg_secrets` transaction.
//! A fixed high-water mark bounds unique pending keys; reaching it waits for
//! the in-flight batch instead of allowing a slow backend to grow the map.
//!
//! Entries leave the buffer only after the backend write returns, so a reader
//! sees every secret either in the buffer or in the store, never neither. A
//! failed write drops its entries with a warning, the same data-loss semantics
//! the previous awaited write had. The only durability change is the window
//! between ack and flush: a process crash inside it loses those secrets, which
//! matches WA Web (IndexedDB persistence is asynchronous there too).

use hashbrown::{Equivalent, HashMap};
use std::collections::hash_map::RandomState;
use std::hash::Hash;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use portable_atomic::AtomicU64;
use wacore::store::traits::MsgSecretEntry;

#[derive(Eq, Hash, PartialEq)]
struct Key {
    chat: Arc<str>,
    sender: Arc<str>,
    msg_id: Arc<str>,
}

#[derive(Hash)]
struct KeyRef<'a> {
    chat: &'a str,
    sender: &'a str,
    msg_id: &'a str,
}

impl Equivalent<Key> for KeyRef<'_> {
    fn equivalent(&self, key: &Key) -> bool {
        self.chat == key.chat.as_ref()
            && self.sender == key.sender.as_ref()
            && self.msg_id == key.msg_id.as_ref()
    }
}

type Pending = HashMap<Key, Arc<MsgSecretEntry>, RandomState>;

enum PendingInsert {
    Buffered,
    ReachedLimit,
    Full(MsgSecretEntry),
}

enum PendingEntries {
    Batch(std::vec::IntoIter<MsgSecretEntry>),
    One(Option<MsgSecretEntry>),
}

impl Iterator for PendingEntries {
    type Item = MsgSecretEntry;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Batch(entries) => entries.next(),
            Self::One(entry) => entry.take(),
        }
    }
}

/// Live captures are normally drained long before reaching this point. The
/// bound keeps a stalled backend from turning the write-behind optimization
/// into an unbounded allocation source while retaining ample batching room.
const MAX_PENDING_MSG_SECRETS: usize = 4_096;

pub(crate) struct MsgSecretWriteBuffer {
    // Values are Arc so the flush snapshot is a refcount bump. Materializing
    // the backend-owned batch later copies only Arc handles and inline bytes.
    pending: Mutex<Pending>,
    pending_limit: usize,
    /// Registered while holding `pending`, so a removal notification cannot
    /// race between a full-capacity check and the producer starting to wait.
    capacity_available: event_listener::Event,
    /// Set by the terminal disconnect. A sealed buffer writes every queue
    /// inline (the old synchronous semantics), so a lane worker still
    /// draining its backlog after the shutdown flush cannot strand a secret
    /// on the detached drain and then ack the message.
    sealed: AtomicBool,
    /// Serializes backend writes: a snapshot is taken under this lock, so a
    /// later writer always carries values at least as new as any earlier
    /// in-flight write, and a stale upsert can never land after a fresh one.
    write_lock: async_lock::Mutex<()>,
    drain_in_flight: AtomicBool,
    backend: Arc<dyn crate::store::traits::Backend>,
    runtime: Arc<dyn wacore::runtime::Runtime>,
    /// Batches written so far; test observability for the coalescing claim.
    #[cfg(test)]
    pub(crate) flushed_batches: AtomicU64,
    #[cfg(not(test))]
    flushed_batches: AtomicU64,
}

impl MsgSecretWriteBuffer {
    pub(crate) fn new(
        backend: Arc<dyn crate::store::traits::Backend>,
        runtime: Arc<dyn wacore::runtime::Runtime>,
    ) -> Arc<Self> {
        Self::with_pending_limit(backend, runtime, MAX_PENDING_MSG_SECRETS)
    }

    fn with_pending_limit(
        backend: Arc<dyn crate::store::traits::Backend>,
        runtime: Arc<dyn wacore::runtime::Runtime>,
        pending_limit: usize,
    ) -> Arc<Self> {
        assert!(
            pending_limit > 0,
            "pending message-secret limit must be positive"
        );
        Arc::new(Self {
            pending: Mutex::new(HashMap::with_hasher(RandomState::new())),
            pending_limit,
            capacity_available: event_listener::Event::new(),
            sealed: AtomicBool::new(false),
            write_lock: async_lock::Mutex::new(()),
            drain_in_flight: AtomicBool::new(false),
            backend,
            runtime,
            flushed_batches: AtomicU64::new(0),
        })
    }

    /// Make `entries` immediately visible to readers and schedule the durable
    /// write. The insert itself is synchronous, so the per-chat lane orders it
    /// before the ack and before any later message in the chat is processed;
    /// the await is a no-op flag check unless the buffer is sealed or reaches
    /// its high-water mark. A full buffer backpressures producers until the
    /// in-flight write releases capacity.
    pub(crate) async fn queue(self: &Arc<Self>, entries: Vec<MsgSecretEntry>) {
        if entries.is_empty() {
            return;
        }
        self.queue_iter(PendingEntries::Batch(entries.into_iter()))
            .await;
    }

    /// Single-entry fast path for live sends, avoiding a temporary one-element Vec.
    pub(crate) async fn queue_one(self: &Arc<Self>, entry: MsgSecretEntry) {
        self.queue_iter(PendingEntries::One(Some(entry))).await;
    }

    async fn queue_iter(self: &Arc<Self>, mut entries: PendingEntries) {
        let mut blocked_entry = None;

        loop {
            let capacity_wait = {
                let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
                let mut next = blocked_entry.take().or_else(|| entries.next());

                loop {
                    let Some(entry) = next else {
                        break None;
                    };
                    match Self::try_insert_pending(&mut pending, entry, self.pending_limit) {
                        PendingInsert::ReachedLimit => {
                            // Register before dropping the mutex: finish_batch
                            // takes the same mutex before notifying waiters.
                            break Some(self.capacity_available.listen());
                        }
                        PendingInsert::Buffered => next = entries.next(),
                        PendingInsert::Full(entry) => {
                            blocked_entry = Some(entry);
                            break Some(self.capacity_available.listen());
                        }
                    }
                }
            };

            let Some(capacity_wait) = capacity_wait else {
                break;
            };
            self.schedule_or_flush().await;
            capacity_wait.await;
        }

        self.schedule_or_flush().await;
    }

    /// Insert without exceeding `pending_limit`. Replacing the same logical key
    /// is always allowed because it consumes no additional buffer capacity.
    fn try_insert_pending(
        pending: &mut Pending,
        mut entry: MsgSecretEntry,
        pending_limit: usize,
    ) -> PendingInsert {
        use wacore::store::traits::{merge_msg_secret_expiry, merge_msg_secret_message_ts};

        let key = Key {
            chat: entry.chat.clone(),
            sender: entry.sender.clone(),
            msg_id: entry.msg_id.clone(),
        };
        // Coalesced captures merge retention metadata like sequential backend writes.
        if let Some(existing) = pending.get_mut(&key) {
            entry.expires_at = merge_msg_secret_expiry(existing.expires_at, entry.expires_at);
            entry.message_ts = merge_msg_secret_message_ts(existing.message_ts, entry.message_ts);
            // Always a fresh Arc: finish_batch tells a recaptured entry from the
            // one it wrote by pointer identity, so this must not use Arc::make_mut.
            *existing = Arc::new(entry);
            return PendingInsert::Buffered;
        }
        if pending.len() >= pending_limit {
            return PendingInsert::Full(entry);
        }
        pending.insert(key, Arc::new(entry));
        if pending.len() == pending_limit {
            PendingInsert::ReachedLimit
        } else {
            PendingInsert::Buffered
        }
    }

    async fn schedule_or_flush(self: &Arc<Self>) {
        // The insertion mutex orders this load against seal(): an insert that the
        // shutdown flush's snapshot missed observes sealed and writes inline.
        if self.sealed.load(Ordering::Acquire) {
            self.flush().await;
        } else {
            self.schedule_drain();
        }
    }

    /// Switch the buffer to inline writes. Terminal: called once by
    /// `disconnect()` right before its final flush.
    pub(crate) fn seal(&self) {
        self.sealed.store(true, Ordering::Release);
    }

    /// Buffered-first read. Returns `(secret, message_ts)` like
    /// `get_msg_secret_with_ts`.
    pub(crate) fn lookup(&self, chat: &str, sender: &str, msg_id: &str) -> Option<(Vec<u8>, i64)> {
        let pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        pending
            .get(&KeyRef {
                chat,
                sender,
                msg_id,
            })
            .map(|e| (e.secret.to_vec(), e.message_ts))
    }

    fn schedule_drain(self: &Arc<Self>) {
        if self.drain_in_flight.swap(true, Ordering::AcqRel) {
            return;
        }
        let buffer = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                buffer.drain_loop().await;
            }))
            .detach();
    }

    async fn drain_loop(self: Arc<Self>) {
        loop {
            if self.flush_pending_once().await {
                continue;
            }
            self.drain_in_flight.store(false, Ordering::Release);
            // An insert may have raced the flag clear; reclaim the drain
            // only if work exists and nobody else took it.
            let has_work = !self
                .pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .is_empty();
            if has_work && !self.drain_in_flight.swap(true, Ordering::AcqRel) {
                continue;
            }
            return;
        }
    }

    /// Write one snapshot of the pending map. Returns whether anything was
    /// pending. Idempotent against a concurrent drain: the upsert repeats
    /// harmlessly and [`Self::finish_batch`] only removes what was written.
    async fn flush_pending_once(&self) -> bool {
        let _write_guard = self.write_lock.lock().await;
        let mut batch: Vec<Arc<MsgSecretEntry>> = {
            let pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            pending.values().cloned().collect()
        };
        if batch.is_empty() {
            return false;
        }
        // put_msg_secrets owns its Vec; materialize it once here. Entry clones
        // only bump the three Arc handles and copy the inline fixed-size data,
        // with no per-entry heap allocation.
        let owned: Vec<MsgSecretEntry> = batch.iter().map(|e| (**e).clone()).collect();
        if let Err(e) = self.backend.put_msg_secrets(owned).await {
            // Same semantics as the previously awaited write: warn + drop.
            log::warn!("failed to persist messageSecrets: {e:?}");
        }
        self.flushed_batches.fetch_add(1, Ordering::Relaxed);
        self.finish_batch(&mut batch);
        true
    }

    /// Drain everything pending before returning. For graceful shutdown: the
    /// detached drain task is not awaited anywhere, so disconnect calls this
    /// to make sure a just-captured secret is not lost on a clean exit.
    pub(crate) async fn flush(&self) {
        while self.flush_pending_once().await {}
    }

    /// Remove the flushed entries, but only where the pending value is still
    /// the one that was written: an edit recapture stores a NEW secret under
    /// the same (chat, sender, id), so a refresh queued while its predecessor
    /// was in flight must survive for the next drain iteration.
    fn finish_batch(&self, written: &mut [Arc<MsgSecretEntry>]) {
        // Remove a written entry only where pending still holds the exact Arc we
        // snapshotted. A recapture during the flush replaces the value with a
        // fresh Arc (try_insert_pending always allocates a new one), so pointer
        // identity separates "written and untouched" from "superseded, must
        // survive" without rebuilding the (chat, sender, id) key or deep-comparing
        // the secret per entry. Sorting pointers in place makes cleanup
        // O(n log n) without allocating an auxiliary set; the previous nested
        // scan was quadratic at the high-water mark. Relies on
        // try_insert_pending storing a NEW Arc on every replacement; if that
        // ever mutates in place (Arc::make_mut), this must return to a content
        // comparison.
        written.sort_unstable_by_key(Arc::as_ptr);
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        let previous_len = pending.len();
        pending.retain(|_key, current| {
            written
                .binary_search_by_key(&Arc::as_ptr(current), Arc::as_ptr)
                .is_err()
        });
        let removed = previous_len - pending.len();
        drop(pending);
        if removed > 0 {
            // Wake all waiters: some are producers that filled the final slot
            // and need no new capacity, while others have entries left to add.
            // Every woken producer rechecks the bound under `pending`.
            self.capacity_available.notify(usize::MAX);
        }
    }

    #[cfg(test)]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

#[cfg(test)]
impl MsgSecretWriteBuffer {
    /// Deterministically wait until every queued entry reached the backend.
    /// Yields so the current-thread test runtime can poll the drain task.
    pub(crate) async fn wait_flushed(&self) {
        while self.pending_len() > 0 {
            tokio::task::yield_now().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(chat: &str, sender: &str, id: &str, secret: u8) -> MsgSecretEntry {
        MsgSecretEntry {
            chat: Arc::from(chat),
            sender: Arc::from(sender),
            msg_id: Arc::from(id),
            secret: [secret; wacore::reporting_token::MESSAGE_SECRET_SIZE],
            expires_at: 0,
            message_ts: 7,
        }
    }

    async fn buffer() -> Arc<MsgSecretWriteBuffer> {
        let backend = crate::test_utils::create_test_backend().await;
        MsgSecretWriteBuffer::new(backend, Arc::new(crate::runtime_impl::TokioRuntime))
    }

    async fn buffer_with_pending_limit(pending_limit: usize) -> Arc<MsgSecretWriteBuffer> {
        let backend = crate::test_utils::create_test_backend().await;
        MsgSecretWriteBuffer::with_pending_limit(
            backend,
            Arc::new(crate::runtime_impl::TokioRuntime),
            pending_limit,
        )
    }

    /// The point of the buffer: a queued secret is readable BEFORE any flush
    /// ran, and after the flush it lives in the backend and leaves the buffer.
    #[tokio::test]
    async fn read_your_write_before_flush() {
        let buf = buffer().await;
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "M1", 0x11)])
            .await;

        // Current-thread runtime: the drain task cannot have polled yet, so
        // this read is served by the buffer, not the store.
        assert_eq!(
            buf.lookup("g@g.us", "a@s.whatsapp.net", "M1"),
            Some((vec![0x11; 32], 7)),
            "queued entry must be visible before the flush"
        );

        buf.wait_flushed().await;
        assert_eq!(buf.pending_len(), 0);
        let stored = buf
            .backend
            .get_msg_secret("g@g.us", "a@s.whatsapp.net", "M1")
            .await
            .expect("backend read");
        assert_eq!(stored.as_deref(), Some(&[0x11u8; 32][..]));
    }

    /// A buffered lookup allocates only the returned `Vec<u8>`; hashing and
    /// comparing the three borrowed key components must remain allocation-free.
    #[tokio::test]
    async fn lookup_borrows_composite_key_without_allocating() {
        let buf = buffer().await;
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "M1", 0x11))
            .await;

        // Sibling tests share the process-wide counter, so take the minimum of
        // repeated windows. The result Vec costs exactly one allocation; an
        // owned lookup key would add one allocation per component every time.
        let mut min_delta = u64::MAX;
        for _ in 0..100 {
            let before = crate::test_alloc::ALLOCS.load(Ordering::Relaxed);
            let result = buf.lookup("g@g.us", "a@s.whatsapp.net", "M1");
            std::hint::black_box(&result);
            let after = crate::test_alloc::ALLOCS.load(Ordering::Relaxed);
            min_delta = min_delta.min(after - before);
        }
        assert_eq!(min_delta, 1, "only the returned secret Vec may allocate");

        buf.wait_flushed().await;
    }

    /// Cloning a buffered entry must stay allocation-free: identifiers share
    /// their Arc allocations and the protocol-sized secret lives inline.
    #[test]
    fn entry_clone_does_not_heap_allocate() {
        let entry = entry("g@g.us", "a@s.whatsapp.net", "M1", 0x11);
        let mut min_delta = u64::MAX;
        for _ in 0..100 {
            let before = crate::test_alloc::ALLOCS.load(Ordering::Relaxed);
            let cloned = std::hint::black_box(&entry).clone();
            std::hint::black_box(cloned);
            let after = crate::test_alloc::ALLOCS.load(Ordering::Relaxed);
            min_delta = min_delta.min(after - before);
        }
        assert_eq!(min_delta, 0, "an entry clone must not allocate");
    }

    /// A stalled backend must cap the pending map exactly at the configured
    /// high-water mark. The producer that fills the final slot and every
    /// concurrent producer wait until a completed flush releases capacity.
    #[tokio::test]
    async fn high_water_mark_backpressures_concurrent_producers() {
        const PENDING_LIMIT: usize = 2;
        const CONCURRENT_PRODUCERS: usize = 8;
        const REFRESHED_SECRET: u8 = 0xfe;

        let buf = buffer_with_pending_limit(PENDING_LIMIT).await;
        let write_guard = buf.write_lock.lock().await;

        // The first insert schedules a drain, which remains blocked on the
        // write lock while producers contend for the two pending slots.
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "M0", 0))
            .await;
        let mut producers = Vec::with_capacity(CONCURRENT_PRODUCERS);
        for index in 1..=CONCURRENT_PRODUCERS {
            let producer_buf = Arc::clone(&buf);
            producers.push(tokio::spawn(async move {
                let secret = u8::try_from(index).expect("test index fits in a byte");
                producer_buf
                    .queue_one(entry(
                        "g@g.us",
                        "a@s.whatsapp.net",
                        &format!("M{index}"),
                        secret,
                    ))
                    .await;
            }));
        }

        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while buf.pending_len() < PENDING_LIMIT {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("a producer must fill the final pending slot");
        for _ in 0..CONCURRENT_PRODUCERS {
            tokio::task::yield_now().await;
        }
        assert_eq!(
            buf.pending_len(),
            PENDING_LIMIT,
            "concurrent producers must not bypass the bound"
        );
        assert!(
            producers.iter().all(|producer| !producer.is_finished()),
            "the producer filling the limit must backpressure with its peers"
        );
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "M0", REFRESHED_SECRET)),
        )
        .await
        .expect("refreshing an existing key must not require new capacity");
        assert_eq!(
            buf.lookup("g@g.us", "a@s.whatsapp.net", "M0"),
            Some((vec![REFRESHED_SECRET; 32], 7))
        );

        drop(write_guard);
        for producer in producers {
            producer.await.expect("producer task");
        }
        buf.wait_flushed().await;

        for index in 0..=CONCURRENT_PRODUCERS {
            let secret = if index == 0 {
                REFRESHED_SECRET
            } else {
                u8::try_from(index).expect("test index fits in a byte")
            };
            let stored = buf
                .backend
                .get_msg_secret("g@g.us", "a@s.whatsapp.net", &format!("M{index}"))
                .await
                .expect("backend read");
            assert_eq!(stored.as_deref(), Some(&[secret; 32][..]));
        }
    }

    /// A burst of captures queued before the drain task gets polled must land
    /// in ONE batched put_msg_secrets transaction, not one per message.
    #[tokio::test]
    async fn burst_coalesces_into_one_batch() {
        let buf = buffer().await;
        for i in 0..20u8 {
            buf.queue(vec![entry(
                "g@g.us",
                "a@s.whatsapp.net",
                &format!("M{i}"),
                i,
            )])
            .await;
        }
        buf.wait_flushed().await;

        assert_eq!(
            buf.flushed_batches.load(Ordering::Relaxed),
            1,
            "a synchronous burst must coalesce into a single batch"
        );
        for i in 0..20u8 {
            let stored = buf
                .backend
                .get_msg_secret("g@g.us", "a@s.whatsapp.net", &format!("M{i}"))
                .await
                .expect("backend read");
            assert_eq!(stored.as_deref(), Some(&[i; 32][..]), "entry M{i}");
        }
    }

    /// A secret refreshed for the same key while its predecessor is being
    /// written (edit recapture) must survive the predecessor's post-flush
    /// removal and reach the backend on the next iteration.
    #[tokio::test]
    async fn refresh_queued_during_flush_survives_removal() {
        let buf = buffer().await;
        let mut stale = [Arc::new(entry(
            "g@g.us",
            "a@s.whatsapp.net",
            "PARENT",
            0x11,
        ))];
        // Simulate the drain having snapshotted `stale` while a refresh lands.
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x22)])
            .await;
        buf.finish_batch(&mut stale);
        assert_eq!(
            buf.lookup("g@g.us", "a@s.whatsapp.net", "PARENT"),
            Some((vec![0x22; 32], 7)),
            "the refresh must not be removed by the stale batch's cleanup"
        );

        buf.wait_flushed().await;
        let stored = buf
            .backend
            .get_msg_secret("g@g.us", "a@s.whatsapp.net", "PARENT")
            .await
            .expect("backend read");
        assert_eq!(
            stored.as_deref(),
            Some(&[0x22u8; 32][..]),
            "the refresh must reach the backend"
        );
    }

    /// Same secret but refreshed retention metadata queued during the flush
    /// must also survive the cleanup (a recapture can extend expires_at).
    #[tokio::test]
    async fn metadata_refresh_during_flush_survives_removal() {
        let buf = buffer().await;
        let mut stale = [Arc::new(entry(
            "g@g.us",
            "a@s.whatsapp.net",
            "PARENT",
            0x11,
        ))];
        let mut refreshed = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        refreshed.expires_at = 999;
        buf.queue(vec![refreshed]).await;
        buf.finish_batch(&mut stale);
        assert_eq!(
            buf.pending_len(),
            1,
            "a same-secret metadata refresh must stay pending"
        );
        buf.wait_flushed().await;
    }

    /// finish_batch keys removal on Arc identity, using the REAL Arc the drain
    /// snapshots (not a hand-built stale entry): a recapture during the flush
    /// replaces the value with a fresh Arc, so the in-flight batch must not evict
    /// it. This is the exact race the pointer-identity check exists for.
    #[tokio::test]
    async fn recapture_survives_finish_batch_of_real_snapshot() {
        let buf = buffer().await;
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11))
            .await;
        // The Arc a drain iteration would carry into put + finish_batch.
        let mut snapshot: Vec<Arc<MsgSecretEntry>> =
            buf.pending.lock().unwrap().values().cloned().collect();
        // Recapture the same key with a new secret; try_insert_pending stores a fresh Arc.
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x22))
            .await;
        // The stale batch's cleanup must leave the refresh in place.
        buf.finish_batch(&mut snapshot);
        assert_eq!(
            buf.lookup("g@g.us", "a@s.whatsapp.net", "PARENT"),
            Some((vec![0x22; 32], 7)),
            "the recapture (a fresh Arc) survives the prior batch's finish_batch"
        );
        buf.wait_flushed().await;
        let stored = buf
            .backend
            .get_msg_secret("g@g.us", "a@s.whatsapp.net", "PARENT")
            .await
            .expect("backend read");
        assert_eq!(
            stored.as_deref(),
            Some(&[0x22u8; 32][..]),
            "the refresh reaches the backend"
        );
    }

    /// The invariant finish_batch relies on: try_insert_pending stores a NEW Arc on
    /// every insert, so even a byte-identical recapture is a distinct allocation
    /// and pointer identity (not content) decides removal. A regression that
    /// reused/mutated the Arc in place would fail here.
    #[tokio::test]
    async fn identical_recapture_is_a_distinct_arc() {
        let buf = buffer().await;
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "K", 0x33))
            .await;
        let mut snapshot: Vec<Arc<MsgSecretEntry>> =
            buf.pending.lock().unwrap().values().cloned().collect();
        // Re-queue byte-identical content.
        buf.queue_one(entry("g@g.us", "a@s.whatsapp.net", "K", 0x33))
            .await;
        let key = KeyRef {
            chat: "g@g.us",
            sender: "a@s.whatsapp.net",
            msg_id: "K",
        };
        let current = buf
            .pending
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .expect("entry pending");
        assert!(
            !Arc::ptr_eq(&current, &snapshot[0]),
            "an identical-content recapture must be a fresh Arc"
        );
        // Therefore the stale batch does not remove it.
        buf.finish_batch(&mut snapshot);
        assert_eq!(
            buf.pending_len(),
            1,
            "the identical recapture survives the stale batch"
        );
        buf.wait_flushed().await;
    }

    /// Coalescing duplicates must merge retention metadata like the backend
    /// upsert does for sequential writes: never-expire wins and a known
    /// parent time survives a later unknown one.
    #[tokio::test]
    async fn coalesced_duplicates_merge_retention_metadata() {
        let buf = buffer().await;
        let mut first = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        first.expires_at = 0;
        first.message_ts = 50;
        let mut second = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        second.expires_at = 100;
        second.message_ts = 0;
        buf.queue(vec![first]).await;
        buf.queue(vec![second]).await;

        let pending = buf
            .pending
            .lock()
            .unwrap()
            .get(&KeyRef {
                chat: "g@g.us",
                sender: "a@s.whatsapp.net",
                msg_id: "PARENT",
            })
            .cloned()
            .expect("entry pending");
        assert_eq!(pending.expires_at, 0, "never-expire must win");
        assert_eq!(pending.message_ts, 50, "known parent time must survive");
        buf.wait_flushed().await;
    }

    /// After seal() a queue is written inline before returning: a lane worker
    /// still draining its backlog after the shutdown flush cannot strand a
    /// secret on the detached drain and then ack the message.
    #[tokio::test]
    async fn sealed_queue_writes_inline() {
        let buf = buffer().await;
        buf.seal();
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "LATE", 0x55)])
            .await;
        // No drain dependency: durable the moment queue() returns.
        assert_eq!(buf.pending_len(), 0);
        let stored = buf
            .backend
            .get_msg_secret("g@g.us", "a@s.whatsapp.net", "LATE")
            .await
            .expect("backend read");
        assert_eq!(stored.as_deref(), Some(&[0x55u8; 32][..]));
    }

    /// The production flush drains everything synchronously, covering the
    /// graceful-shutdown path where the detached drain is never awaited.
    #[tokio::test]
    async fn explicit_flush_drains_everything() {
        let buf = buffer().await;
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "SHUTDOWN", 0x44)])
            .await;
        buf.flush().await;
        assert_eq!(buf.pending_len(), 0);
        let stored = buf
            .backend
            .get_msg_secret("g@g.us", "a@s.whatsapp.net", "SHUTDOWN")
            .await
            .expect("backend read");
        assert_eq!(stored.as_deref(), Some(&[0x44u8; 32][..]));
    }

    /// Entries queued while a flush is in flight are picked up by the same
    /// drain task (follow-up iteration), never lost.
    #[tokio::test]
    async fn entries_queued_during_drain_are_flushed() {
        let buf = buffer().await;
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "FIRST", 1)])
            .await;
        // Let the drain start (and likely finish the first batch).
        tokio::task::yield_now().await;
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "SECOND", 2)])
            .await;
        buf.wait_flushed().await;

        for (id, val) in [("FIRST", 1u8), ("SECOND", 2u8)] {
            let stored = buf
                .backend
                .get_msg_secret("g@g.us", "a@s.whatsapp.net", id)
                .await
                .expect("backend read");
            assert_eq!(stored.as_deref(), Some(&[val; 32][..]), "{id}");
        }
        assert_eq!(buf.pending_len(), 0);
    }
}
