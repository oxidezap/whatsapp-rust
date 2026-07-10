//! Write-behind buffer for inbound `messageSecret` persistence.
//!
//! Capturing a secret used to upsert SQLite synchronously inside the per-chat
//! receive lane, before the ack and the `Event::Messages` dispatch. The buffer
//! splits visibility from durability: an insert is immediately readable
//! through [`MsgSecretWriteBuffer::lookup`] (so an add-on referencing the
//! secret of the stanza just processed always finds it), while the backend
//! upsert happens on a detached drain task that coalesces a burst of captures
//! into one batched `put_msg_secrets` transaction.
//!
//! Entries leave the buffer only after the backend write returns, so a reader
//! sees every secret either in the buffer or in the store, never neither. A
//! failed write drops its entries with a warning, the same data-loss semantics
//! the previous awaited write had. The only durability change is the window
//! between ack and flush: a process crash inside it loses those secrets, which
//! matches WA Web (IndexedDB persistence is asynchronous there too).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use portable_atomic::AtomicU64;
use wacore::store::traits::MsgSecretEntry;

type Key = (String, String, String);

pub(crate) struct MsgSecretWriteBuffer {
    pending: Mutex<HashMap<Key, MsgSecretEntry>>,
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
        Arc::new(Self {
            pending: Mutex::new(HashMap::new()),
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
    /// the await is a no-op flag check unless the buffer is sealed, in which
    /// case the write happens inline before returning.
    pub(crate) async fn queue(self: &Arc<Self>, entries: Vec<MsgSecretEntry>) {
        if entries.is_empty() {
            return;
        }
        {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            for entry in entries {
                Self::insert_pending(&mut pending, entry);
            }
        }
        self.schedule_or_flush().await;
    }

    /// Single-entry fast path for live sends, avoiding a temporary one-element Vec.
    pub(crate) async fn queue_one(self: &Arc<Self>, entry: MsgSecretEntry) {
        {
            let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            Self::insert_pending(&mut pending, entry);
        }
        self.schedule_or_flush().await;
    }

    fn insert_pending(pending: &mut HashMap<Key, MsgSecretEntry>, mut entry: MsgSecretEntry) {
        use wacore::store::traits::{merge_msg_secret_expiry, merge_msg_secret_message_ts};

        let key = (
            entry.chat.clone(),
            entry.sender.clone(),
            entry.msg_id.clone(),
        );
        // Coalesced captures merge retention metadata like sequential backend writes.
        if let Some(existing) = pending.get(&key) {
            entry.expires_at = merge_msg_secret_expiry(existing.expires_at, entry.expires_at);
            entry.message_ts = merge_msg_secret_message_ts(existing.message_ts, entry.message_ts);
        }
        pending.insert(key, entry);
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
            .get(&(chat.to_string(), sender.to_string(), msg_id.to_string()))
            .map(|e| (e.secret.clone(), e.message_ts))
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
        let batch: Vec<MsgSecretEntry> = {
            let pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
            pending.values().cloned().collect()
        };
        if batch.is_empty() {
            return false;
        }
        if let Err(e) = self.backend.put_msg_secrets(batch.clone()).await {
            // Same semantics as the previously awaited write: warn + drop.
            log::warn!("failed to persist messageSecrets: {e:?}");
        }
        self.flushed_batches.fetch_add(1, Ordering::Relaxed);
        self.finish_batch(&batch);
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
    fn finish_batch(&self, written: &[MsgSecretEntry]) {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        for entry in written {
            let key = (
                entry.chat.clone(),
                entry.sender.clone(),
                entry.msg_id.clone(),
            );
            let unchanged = |current: &MsgSecretEntry| {
                current.secret == entry.secret
                    && current.expires_at == entry.expires_at
                    && current.message_ts == entry.message_ts
            };
            if pending.get(&key).is_some_and(unchanged) {
                pending.remove(&key);
            }
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
            chat: chat.to_string(),
            sender: sender.to_string(),
            msg_id: id.to_string(),
            secret: vec![secret; 32],
            expires_at: 0,
            message_ts: 7,
        }
    }

    async fn buffer() -> Arc<MsgSecretWriteBuffer> {
        let backend = crate::test_utils::create_test_backend().await;
        MsgSecretWriteBuffer::new(backend, Arc::new(crate::runtime_impl::TokioRuntime))
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
        let stale = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        // Simulate the drain having snapshotted `stale` while a refresh lands.
        buf.queue(vec![entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x22)])
            .await;
        buf.finish_batch(std::slice::from_ref(&stale));
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
        let stale = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        let mut refreshed = entry("g@g.us", "a@s.whatsapp.net", "PARENT", 0x11);
        refreshed.expires_at = 999;
        buf.queue(vec![refreshed]).await;
        buf.finish_batch(std::slice::from_ref(&stale));
        assert_eq!(
            buf.pending_len(),
            1,
            "a same-secret metadata refresh must stay pending"
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
            .get(&(
                "g@g.us".to_string(),
                "a@s.whatsapp.net".to_string(),
                "PARENT".to_string(),
            ))
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
