use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use async_lock::Mutex;

use crate::libsignal::protocol::{ProtocolAddress, SenderKeyRecord, SessionRecord};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::store::traits::SignalStore;

/// Evict clean (non-dirty, non-deleted) entries from a cache HashMap.
/// Negative entries (None values) are evicted first.
fn evict_clean_entries<V>(
    cache: &mut HashMap<Arc<str>, Option<V>>,
    dirty: &HashSet<Arc<str>>,
    deleted: Option<&HashSet<Arc<str>>>,
    max_entries: usize,
) {
    let overflow = cache.len().saturating_sub(max_entries);
    if overflow == 0 {
        return;
    }
    let mut negative = Vec::with_capacity(overflow);
    let mut positive = Vec::with_capacity(overflow);
    for (k, v) in cache.iter() {
        if dirty.contains(k.as_ref()) {
            continue;
        }
        if let Some(del) = deleted
            && del.contains(k.as_ref())
        {
            continue;
        }
        if v.is_none() {
            negative.push(k.clone());
        } else {
            positive.push(k.clone());
        }
    }
    for key in negative.into_iter().chain(positive).take(overflow) {
        cache.remove(&key);
    }
}

/// Default max entries per store before clean entry eviction triggers.
const DEFAULT_MAX_CACHE_ENTRIES: usize = 2_000;

/// In-memory write-back cache for Signal protocol state.
/// Keys use `Arc<str>` for O(1) clone. Sessions cached as objects (serialized on flush).
/// Capacity-bounded: evicts non-dirty entries when max_entries is exceeded.
pub struct SignalStoreCache {
    sessions: Mutex<SessionStoreState>,
    identities: Mutex<ByteStoreState>,
    sender_keys: Mutex<SenderKeyStoreState>,
    /// Consumed one-time pre-key IDs awaiting durable deletion. Buffered on the
    /// inbound pkmsg path so the prekey is removed from the backend only in the
    /// same flush that persists the promoted session, never before it. Matches
    /// WAWebSignalProtocolStoreUnifiedApi, which buffers removePreKey and commits
    /// it alongside the session put under one storage lock. Without this the
    /// prekey could be durably gone while the new session is still volatile, so a
    /// crash in that window loses both and a redelivered pkmsg is undecryptable.
    removed_prekeys: Mutex<HashSet<u32>>,
    /// Per-(group, sender) locks serializing each sender-key chain advance.
    /// Coordination only (like the client session locks): never time-evicted.
    sender_key_locks: Mutex<HashMap<Arc<str>, Arc<Mutex<()>>>>,
    max_entries: usize,
}

// === Session object cache (no per-message serialize/deserialize) ===

/// Cache entry tracking whether a session is present, absent, or checked out
/// by an encrypt/decrypt operation.
enum SessionEntry {
    Present(Box<SessionRecord>),
    Absent,
    /// Taken by load_session; has_session treats as present, flush/eviction skip.
    CheckedOut,
}

struct SessionStoreState {
    cache: HashMap<Arc<str>, SessionEntry>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
}

impl SessionStoreState {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            dirty: HashSet::new(),
            deleted: HashSet::new(),
        }
    }

    /// Reuse the existing Arc<str> key if the address is already in the cache,
    /// avoiding a heap allocation on every call (hot path: key always exists).
    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    fn put(&mut self, address: &str, record: SessionRecord) {
        let addr = self.key_for(address);
        self.cache
            .insert(addr.clone(), SessionEntry::Present(Box::new(record)));
        self.dirty.insert(addr.clone());
        self.deleted.remove(&addr);
    }

    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), SessionEntry::Absent);
        self.deleted.insert(addr.clone());
        self.dirty.remove(&addr);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.deleted.clear();
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        let overflow = self.cache.len().saturating_sub(max_entries);
        if overflow == 0 {
            return;
        }
        let mut negative = Vec::with_capacity(overflow);
        let mut positive = Vec::with_capacity(overflow);
        for (k, v) in self.cache.iter() {
            if self.dirty.contains(k.as_ref()) || self.deleted.contains(k.as_ref()) {
                continue;
            }
            match v {
                SessionEntry::CheckedOut => continue, // never evict checked-out
                SessionEntry::Absent => negative.push(k.clone()),
                SessionEntry::Present(_) => positive.push(k.clone()),
            }
        }
        for key in negative.into_iter().chain(positive).take(overflow) {
            self.cache.remove(&key);
        }
    }
}

// === Sender key object cache (same pattern as sessions) ===

struct SenderKeyStoreState {
    // `Arc`-wrapped so a warm `get_sender_key` (the per-send peek reads and the
    // per-decrypt load) bumps a refcount instead of deep-cloning the record's
    // `VecDeque<SenderKeyState>` with up to `MAX_MESSAGE_KEYS` message keys each.
    cache: HashMap<Arc<str>, Option<Arc<SenderKeyRecord>>>,
    dirty: HashSet<Arc<str>>,
}

impl SenderKeyStoreState {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            dirty: HashSet::new(),
        }
    }

    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    fn put(&mut self, address: &str, record: SenderKeyRecord) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), Some(Arc::new(record)));
        self.dirty.insert(addr.clone());
    }

    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), None);
        self.dirty.insert(addr);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        evict_clean_entries(&mut self.cache, &self.dirty, None, max_entries);
    }
}

// === Byte cache for identities ===

struct ByteStoreState {
    /// Cached entries. `None` value = known-absent (negative cache).
    cache: HashMap<Arc<str>, Option<Arc<[u8]>>>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
}

impl ByteStoreState {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
            dirty: HashSet::new(),
            deleted: HashSet::new(),
        }
    }

    /// Reuse the existing Arc<str> key if the address is already in the cache.
    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    /// Insert data, skipping if bytes are identical (avoids redundant dirty marks).
    /// Use for stores where data rarely changes (identities).
    fn put_dedup(&mut self, address: &str, data: &[u8]) {
        if let Some(Some(existing)) = self.cache.get(address)
            && existing.as_ref() == data
        {
            return;
        }
        self.put(address, data);
    }

    /// Insert data unconditionally. Use for stores where data changes every
    /// message (sender keys) — the byte comparison would always fail.
    fn put(&mut self, address: &str, data: &[u8]) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), Some(Arc::from(data)));
        self.dirty.insert(addr.clone());
        self.deleted.remove(&addr);
    }

    /// Mark an entry as deleted (negative-cached).
    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), None);
        self.deleted.insert(addr.clone());
        self.dirty.remove(&addr);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.deleted.clear();
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        evict_clean_entries(
            &mut self.cache,
            &self.dirty,
            Some(&self.deleted),
            max_entries,
        );
    }
}

impl Default for SignalStoreCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalStoreCache {
    pub fn new() -> Self {
        Self::with_max_entries(DEFAULT_MAX_CACHE_ENTRIES)
    }

    pub fn with_max_entries(max_entries: usize) -> Self {
        Self {
            sessions: Mutex::new(SessionStoreState::new()),
            identities: Mutex::new(ByteStoreState::new()),
            sender_keys: Mutex::new(SenderKeyStoreState::new()),
            removed_prekeys: Mutex::new(HashSet::new()),
            sender_key_locks: Mutex::new(HashMap::new()),
            max_entries,
        }
    }

    /// Whether any session or identity is known for `user` (across device ids),
    /// checking the in-memory cache first, then the durable backend. Lets a
    /// caller skip a per-device migration scan for a user we've never had Signal
    /// state with. Conservative on the cache side: any matching key counts
    /// (even a stale/checked-out marker), so it never reports "none" when state
    /// might exist.
    pub async fn has_state_for_user(&self, user: &str, backend: &dyn SignalStore) -> Result<bool> {
        fn matches(addr: &str, user: &str) -> bool {
            addr.strip_prefix(user)
                .is_some_and(|rest| rest.starts_with('@') || rest.starts_with(':'))
        }
        {
            let state = self.sessions.lock().await;
            if state.cache.keys().any(|k| matches(k, user)) {
                return Ok(true);
            }
        }
        {
            let state = self.identities.lock().await;
            if state.cache.keys().any(|k| matches(k, user)) {
                return Ok(true);
            }
        }
        Ok(backend.has_signal_state_for_user(user).await?)
    }

    // === Sessions (object cache — serialize only during flush) ===

    /// Takes ownership of the cached session, leaving a `CheckedOut` marker.
    /// Callers must return the record with [`put_session`] after use.
    pub async fn get_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<SessionRecord>> {
        let key = address.as_str();
        {
            let mut state = self.sessions.lock().await;
            if let Some(entry) = state.cache.get_mut(key) {
                if matches!(entry, SessionEntry::Present(_)) {
                    let SessionEntry::Present(record) =
                        std::mem::replace(entry, SessionEntry::CheckedOut)
                    else {
                        unreachable!()
                    };
                    return Ok(Some(*record));
                }
                return Ok(None);
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.sessions.lock().await;
        match backend_result {
            Some(bytes) => {
                if state.cache.contains_key(key) {
                    // Another task populated this slot while we were loading;
                    // defer to whatever they wrote (Present, CheckedOut, etc).
                    // Deserialize and return without caching to avoid conflict.
                    return Ok(Some(SessionRecord::deserialize(&bytes)?));
                }
                let record = SessionRecord::deserialize(&bytes)?;
                state.cache.insert(Arc::from(key), SessionEntry::CheckedOut);
                state.evict_if_needed(self.max_entries);
                Ok(Some(record))
            }
            None => {
                if !state.cache.contains_key(key) {
                    state.cache.insert(Arc::from(key), SessionEntry::Absent);
                    state.evict_if_needed(self.max_entries);
                }
                Ok(None)
            }
        }
    }

    /// Non-destructive read. Clones the session without removing it from
    /// cache. Use for inspection-only paths (retry, LID migration checks).
    pub async fn peek_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<SessionRecord>> {
        let key = address.as_str();
        {
            let state = self.sessions.lock().await;
            if let Some(entry) = state.cache.get(key) {
                return match entry {
                    SessionEntry::Present(record) => Ok(Some((**record).clone())),
                    _ => Ok(None),
                };
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.sessions.lock().await;
        match backend_result {
            Some(bytes) => {
                let record = SessionRecord::deserialize(&bytes)?;
                if !state.cache.contains_key(key) {
                    state.cache.insert(
                        Arc::from(key),
                        SessionEntry::Present(Box::new(record.clone())),
                    );
                    state.evict_if_needed(self.max_entries);
                }
                Ok(Some(record))
            }
            None => {
                if !state.cache.contains_key(key) {
                    state.cache.insert(Arc::from(key), SessionEntry::Absent);
                    state.evict_if_needed(self.max_entries);
                }
                Ok(None)
            }
        }
    }

    pub async fn put_session(&self, address: &ProtocolAddress, record: SessionRecord) {
        let mut state = self.sessions.lock().await;
        state.put(address.as_str(), record);
        state.evict_if_needed(self.max_entries);
    }

    pub async fn delete_session(&self, address: &ProtocolAddress) {
        let mut state = self.sessions.lock().await;
        state.delete(address.as_str());
        state.evict_if_needed(self.max_entries);
    }

    /// Non-destructive existence check (`CheckedOut` counts as present).
    /// Backend misses are negative-cached; hits are not cached to skip
    /// deserialization (the subsequent `get_session` will cache on demand).
    pub async fn has_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<bool> {
        let key = address.as_str();
        {
            let state = self.sessions.lock().await;
            if let Some(entry) = state.cache.get(key) {
                return Ok(!matches!(entry, SessionEntry::Absent));
            }
        }
        // Backend I/O outside the lock
        let exists = backend.has_session(key).await?;
        if !exists {
            let mut state = self.sessions.lock().await;
            // Re-check: another task may have populated the cache
            if !state.cache.contains_key(key) {
                state.cache.insert(Arc::from(key), SessionEntry::Absent);
                state.evict_if_needed(self.max_entries);
            }
        }
        Ok(exists)
    }

    // === Identities ===

    pub async fn get_identity(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<Arc<[u8]>>> {
        let key = address.as_str();
        // Cache check inside scoped lock so concurrent callers don't queue on
        // the mutex during the backend roundtrip. Mirrors get_session/has_session.
        {
            let state = self.identities.lock().await;
            if let Some(cached) = state.cache.get(key) {
                return Ok(cached.clone());
            }
        }
        // Backend I/O outside the lock.
        let data = backend.load_identity(key).await?;
        let arc_data = data.map(Arc::from);
        let mut state = self.identities.lock().await;
        // Re-check: another task may have populated the cache while we awaited.
        if let Some(cached) = state.cache.get(key) {
            return Ok(cached.clone());
        }
        state.cache.insert(Arc::from(key), arc_data.clone());
        state.evict_if_needed(self.max_entries);
        Ok(arc_data)
    }

    pub async fn put_identity(&self, address: &ProtocolAddress, data: &[u8]) {
        let mut state = self.identities.lock().await;
        state.put_dedup(address.as_str(), data);
        state.evict_if_needed(self.max_entries);
    }

    pub async fn delete_identity(&self, address: &ProtocolAddress) {
        let mut state = self.identities.lock().await;
        state.delete(address.as_str());
        state.evict_if_needed(self.max_entries);
    }

    // === Sender Keys ===

    /// Returns a shared (`Arc`) handle to the cached sender-key record. A warm hit
    /// is a refcount bump, not a deep clone of the message-key backlog. Callers
    /// that need to mutate clone the inner record (e.g. via the trait
    /// `load_sender_key`), so the cache copy is never mutated through this handle.
    pub async fn get_sender_key(
        &self,
        name: &SenderKeyName,
        backend: &dyn SignalStore,
    ) -> Result<Option<Arc<SenderKeyRecord>>> {
        let key = name.cache_key();
        let mut state = self.sender_keys.lock().await;
        if let Some(cached) = state.cache.get(key) {
            return Ok(cached.clone());
        }
        let record = match backend.get_sender_key(key).await? {
            Some(bytes) => Some(Arc::new(SenderKeyRecord::deserialize(&bytes)?)),
            None => None,
        };
        state.cache.insert(Arc::from(key), record.clone());
        state.evict_if_needed(self.max_entries);
        Ok(record)
    }

    pub async fn put_sender_key(&self, name: &SenderKeyName, record: SenderKeyRecord) {
        let mut state = self.sender_keys.lock().await;
        state.put(name.cache_key(), record);
        state.evict_if_needed(self.max_entries);
    }

    /// Shared lock for the `name` chain. Same name returns the same lock so a
    /// concurrent encrypt can't read a chain iteration another is advancing.
    pub async fn sender_key_lock(&self, name: &SenderKeyName) -> Arc<Mutex<()>> {
        let mut map = self.sender_key_locks.lock().await;
        if let Some(lock) = map.get(name.cache_key()) {
            return lock.clone();
        }
        // Drop idle locks (held only by the map) once the map grows large.
        if map.len() >= self.max_entries {
            map.retain(|_, lock| Arc::strong_count(lock) > 1);
        }
        let lock = Arc::new(Mutex::new(()));
        map.insert(Arc::from(name.cache_key()), lock.clone());
        lock
    }

    pub async fn delete_sender_key(&self, cache_key: &str) {
        let mut state = self.sender_keys.lock().await;
        state.delete(cache_key);
        state.evict_if_needed(self.max_entries);
    }

    // === Consumed pre-keys ===

    /// Buffer a consumed one-time pre-key for deletion on the next flush, rather
    /// than deleting it from the backend immediately. The decrypt path promotes
    /// the session into the (volatile) session cache, so deleting the prekey
    /// durably before that session is flushed would lose both on a crash. Flush
    /// removes these only after the session put succeeds.
    pub async fn remove_prekey(&self, prekey_id: u32) {
        self.removed_prekeys.lock().await.insert(prekey_id);
    }

    // === Flush ===

    /// Flush all dirty state to the backend.
    ///
    /// Identities and sender keys are flushed independently under their own lock,
    /// so each is locked only during its own I/O while the others stay free for
    /// concurrent encrypt/decrypt. Sessions and consumed pre-keys are committed
    /// together under the single sessions lock: the prekey delete must be atomic
    /// with the session put against concurrent buffering, so they cannot use
    /// separate lock scopes. Within each scope the lock is held across snapshot,
    /// I/O, and clear, so there is no race between snapshot and clear and dirty
    /// sets are cleared only after successful writes.
    pub async fn flush(&self, backend: &dyn SignalStore) -> Result<()> {
        // Flush sessions: one batched write for all dirty puts instead of one
        // backend call (and one SQLite transaction) per session.
        {
            let mut state = self.sessions.lock().await;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();
            let deleted_keys: Vec<_> = state.deleted.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, bytes::Bytes)> = Vec::new();
            for address in &dirty_keys {
                match state.cache.get(address.as_ref()) {
                    Some(SessionEntry::Present(record)) => {
                        let mut buf = Vec::new();
                        record.serialize_into(&mut buf);
                        batch.push((address.clone(), bytes::Bytes::from(buf)));
                    }
                    Some(SessionEntry::CheckedOut) => continue,
                    _ => {}
                }
            }
            if !batch.is_empty() {
                backend.put_sessions_batch(&batch).await?;
            }
            for address in &deleted_keys {
                backend.delete_session(address).await?;
            }

            for key in &dirty_keys {
                if !matches!(
                    state.cache.get(key.as_ref()),
                    Some(SessionEntry::CheckedOut)
                ) {
                    state.dirty.remove(key);
                }
            }
            for key in &deleted_keys {
                state.deleted.remove(key);
            }
            state.evict_if_needed(self.max_entries);

            // Delete consumed one-time pre-keys only after the session batch above
            // is durable. The session was promoted on the inbound pkmsg path;
            // deleting the prekey before the session is committed risks losing both
            // on a crash and making a redelivered pkmsg undecryptable. The buffer is
            // drained and only cleared after every backend delete succeeds, so a
            // failed flush leaves the IDs buffered for the next attempt.
            //
            // This stays under the sessions lock so the session commit and the
            // prekey delete are atomic against concurrent buffering: a sender whose
            // decrypt promotes a session and buffers its prekey AFTER this snapshot
            // is blocked from inserting into removed_prekeys until the lock frees,
            // so a flush can never delete a prekey whose session it did not persist.
            // Matches WAWebSignalProtocolStoreUnifiedApi holding all cache mutexes
            // across the flush so no putSession/removePreKey interleaves the commit.
            let mut removed = self.removed_prekeys.lock().await;
            if !removed.is_empty() {
                let ids: Vec<u32> = removed.iter().copied().collect();
                for id in &ids {
                    backend.remove_prekey(*id).await?;
                }
                for id in &ids {
                    removed.remove(id);
                }
            }
        }

        // Flush identities
        {
            let mut state = self.identities.lock().await;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();
            let deleted_keys: Vec<_> = state.deleted.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, [u8; 32])> = Vec::new();
            for address in &dirty_keys {
                if let Some(Some(data)) = state.cache.get(address.as_ref()) {
                    let key: [u8; 32] = data.as_ref().try_into().map_err(|_| {
                        anyhow::anyhow!(
                            "Corrupted identity key for {address}: expected 32 bytes, got {}",
                            data.len()
                        )
                    })?;
                    batch.push((address.clone(), key));
                }
            }
            if !batch.is_empty() {
                backend.put_identities_batch(&batch).await?;
            }
            for address in &deleted_keys {
                backend.delete_identity(address).await?;
            }

            for key in &dirty_keys {
                state.dirty.remove(key);
            }
            for key in &deleted_keys {
                state.deleted.remove(key);
            }
            state.evict_if_needed(self.max_entries);
        }

        // Flush sender keys
        {
            let mut state = self.sender_keys.lock().await;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, bytes::Bytes)> = Vec::new();
            for name in &dirty_keys {
                match state.cache.get(name.as_ref()) {
                    Some(Some(record)) => {
                        let bytes = record
                            .serialize()
                            .map_err(|e| anyhow::anyhow!("sender key serialize for {name}: {e}"))?;
                        batch.push((name.clone(), bytes::Bytes::from(bytes)));
                    }
                    Some(None) => {
                        backend.delete_sender_key(name).await?;
                    }
                    None => {}
                }
            }
            if !batch.is_empty() {
                backend.put_sender_keys_batch(&batch).await?;
            }

            for key in &dirty_keys {
                state.dirty.remove(key);
            }
            state.evict_if_needed(self.max_entries);
        }

        Ok(())
    }

    /// Returns the number of entries in each store (sessions, identities, sender_keys).
    #[cfg(feature = "debug-diagnostics")]
    pub async fn entry_counts(&self) -> (usize, usize, usize) {
        let s = self.sessions.lock().await;
        let i = self.identities.lock().await;
        let sk = self.sender_keys.lock().await;
        (s.cache.len(), i.cache.len(), sk.cache.len())
    }

    /// Clear all cached state (used on disconnect/reconnect).
    /// Retains allocated capacity for reuse on reconnect.
    pub async fn clear(&self) {
        self.sessions.lock().await.clear();
        self.identities.lock().await.clear();
        self.sender_keys.lock().await.clear();
        // Drop buffered prekey removals together with the volatile sessions they
        // belong to: the promoted session is gone, so the still-durable prekey
        // must stay so a redelivered pkmsg can rebuild the session.
        self.removed_prekeys.lock().await.clear();
    }
}

#[cfg(test)]
mod sender_key_lock_tests {
    use super::*;
    use crate::libsignal::store::sender_key_name::SenderKeyName;

    #[tokio::test]
    async fn same_name_shares_one_lock() {
        let cache = SignalStoreCache::new();
        let a = SenderKeyName::from_parts("g1@g.us", "u1@s.whatsapp.net:0");
        let b = SenderKeyName::from_parts("g2@g.us", "u1@s.whatsapp.net:0");

        let l1 = cache.sender_key_lock(&a).await;
        let l2 = cache.sender_key_lock(&a).await;
        let l3 = cache.sender_key_lock(&b).await;

        assert!(Arc::ptr_eq(&l1, &l2), "same name must share one lock");
        assert!(!Arc::ptr_eq(&l1, &l3), "different names must not share");
    }

    #[tokio::test]
    async fn same_name_lock_is_mutually_exclusive() {
        let cache = SignalStoreCache::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");
        let lock = cache.sender_key_lock(&name).await;

        let guard = lock.lock().await;
        assert!(
            lock.try_lock().is_none(),
            "held lock must block a second acquire"
        );
        drop(guard);
        assert!(lock.try_lock().is_some(), "released lock must reacquire");
    }

    #[tokio::test]
    async fn warm_sender_key_hit_shares_arc_not_deep_clone() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache
            .put_sender_key(&name, SenderKeyRecord::new_empty())
            .await;

        let a = cache
            .get_sender_key(&name, &backend)
            .await
            .unwrap()
            .expect("warm hit");
        let b = cache
            .get_sender_key(&name, &backend)
            .await
            .unwrap()
            .expect("warm hit");

        // A warm sender-key hit returns a refcount bump of the same allocation,
        // not a deep copy of the message-key backlog.
        assert!(Arc::ptr_eq(&a, &b));
    }
}

#[cfg(test)]
mod consumed_prekey_atomicity_tests {
    use super::*;
    use crate::store::in_memory::InMemoryBackend;
    use crate::store::traits::SignalStore;

    const PREKEY_ID: u32 = 4242;

    /// Seed a durable prekey in the backend and return the address the inbound
    /// pkmsg promotes a session for.
    async fn seed(backend: &InMemoryBackend) -> ProtocolAddress {
        backend
            .store_prekey(PREKEY_ID, b"durable-prekey", false)
            .await
            .unwrap();
        ProtocolAddress::new("bob".to_string(), 1.into())
    }

    /// The inbound pkmsg decrypt promotes the session into the volatile cache and
    /// then "removes" the consumed prekey. The removal must NOT touch the backend
    /// until the session-bearing flush runs, so a crash in the window between
    /// decrypt and flush can never leave the prekey durably deleted while its new
    /// session is still only in memory.
    #[tokio::test]
    async fn consumed_prekey_stays_durable_until_session_flush() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        // Decrypt path: session into cache (volatile), prekey buffered for removal.
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID).await;

        // Pre-flush invariant: the prekey is still durable in the backend, so even
        // if everything volatile is lost the redelivered pkmsg can rebuild.
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "consumed prekey must remain in the backend until the session flush"
        );
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_none(),
            "session is only volatile before flush"
        );

        // Flush commits the session AND the prekey deletion together.
        cache.flush(&backend).await.unwrap();

        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_some(),
            "session must be durable after flush"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "prekey must be deleted once the session it produced is durable"
        );
    }

    /// A disconnect (cache clear) before the flush drops the volatile session, so
    /// the still-durable prekey must be kept (its buffered removal dropped) to let
    /// a redelivered pkmsg rebuild the session.
    #[tokio::test]
    async fn clear_before_flush_keeps_prekey_so_pkmsg_can_rebuild() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID).await;

        cache.clear().await;

        // The session never reached the backend, so the prekey must survive.
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_none(),
            "volatile session is dropped on clear"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "prekey must survive a clear that discarded its unflushed session"
        );

        // A subsequent flush of the now-empty buffer is a no-op for the prekey.
        cache.flush(&backend).await.unwrap();
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "cleared buffer must not delete the prekey on a later flush"
        );
    }

    /// A failed session write must abort the flush before the prekey deletion, and
    /// the buffered ID must remain so the next flush retries it. This guards the
    /// exact regression: the prekey lane running before/independently of a durable
    /// session.
    #[tokio::test]
    async fn failed_session_flush_does_not_delete_prekey() {
        struct FailingSessions(InMemoryBackend);

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl SignalStore for FailingSessions {
            async fn put_sessions_batch(
                &self,
                _sessions: &[(Arc<str>, bytes::Bytes)],
            ) -> crate::store::error::Result<()> {
                Err(crate::store::error::StoreError::Validation(
                    "simulated session write failure".to_string(),
                ))
            }

            async fn put_identity(
                &self,
                address: &str,
                key: [u8; 32],
            ) -> crate::store::error::Result<()> {
                self.0.put_identity(address, key).await
            }
            async fn load_identity(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<[u8; 32]>> {
                self.0.load_identity(address).await
            }
            async fn delete_identity(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_identity(address).await
            }
            async fn get_session(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.0.get_session(address).await
            }
            async fn put_session(
                &self,
                address: &str,
                session: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.put_session(address, session).await
            }
            async fn delete_session(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_session(address).await
            }
            async fn store_prekey(
                &self,
                id: u32,
                record: &[u8],
                uploaded: bool,
            ) -> crate::store::error::Result<()> {
                self.0.store_prekey(id, record, uploaded).await
            }
            async fn load_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.0.load_prekey(id).await
            }
            async fn remove_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.0.remove_prekey(id).await
            }
            async fn get_max_prekey_id(&self) -> crate::store::error::Result<u32> {
                self.0.get_max_prekey_id().await
            }
            async fn store_signed_prekey(
                &self,
                id: u32,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.store_signed_prekey(id, record).await
            }
            async fn load_signed_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.0.load_signed_prekey(id).await
            }
            async fn load_all_signed_prekeys(
                &self,
            ) -> crate::store::error::Result<Vec<(u32, Vec<u8>)>> {
                self.0.load_all_signed_prekeys().await
            }
            async fn remove_signed_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.0.remove_signed_prekey(id).await
            }
            async fn put_sender_key(
                &self,
                address: &str,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.put_sender_key(address, record).await
            }
            async fn get_sender_key(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.0.get_sender_key(address).await
            }
            async fn delete_sender_key(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_sender_key(address).await
            }
        }

        let inner = InMemoryBackend::new();
        let addr = seed(&inner).await;
        let backend = FailingSessions(inner);
        let cache = SignalStoreCache::new();

        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID).await;

        // The session write fails, so flush errors out before the prekey lane.
        assert!(cache.flush(&backend).await.is_err());

        // The prekey must still be durable: it must never be deleted while its
        // session is not committed.
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "prekey must not be deleted when the session write fails"
        );

        // The buffered removal must remain so a later successful flush retries it.
        assert!(
            cache.removed_prekeys.lock().await.contains(&PREKEY_ID),
            "buffered prekey removal must persist across a failed flush"
        );
    }

    /// A decrypt racing a flush must never lose the session<->prekey atomicity.
    ///
    /// Sender A's flush holds the sessions lock across both the session commit AND
    /// the consumed-prekey drain. While it is mid-flush, sender B's decrypt tries to
    /// promote B's session and buffer B's consumed prekey. Because the prekey buffer
    /// is drained under that same sessions lock, B cannot reach the buffer until A's
    /// flush has fully committed and released the lock, so A's flush can never delete
    /// B's prekey while B's session is still volatile. The buggy form (prekey drain
    /// in a separate lock scope) releases the sessions lock first, leaving a window
    /// where B buffers its prekey and A then durably deletes it with B's session
    /// unflushed. The backend asserts the sessions lock is held at the moment the
    /// prekey is deleted, which directly distinguishes the fixed and buggy forms.
    #[tokio::test]
    async fn concurrent_decrypt_does_not_lose_prekey_during_flush() {
        use std::sync::Arc as StdArc;
        use std::sync::atomic::{AtomicBool, Ordering};

        const PREKEY_A: u32 = 1001;
        const PREKEY_B: u32 = 1002;

        /// Wraps an InMemoryBackend. `put_sessions_batch` yields the executor many
        /// times before doing the real write, so a concurrently spawned decrypt has
        /// every chance to reach (and block on) the sessions lock while A's flush
        /// holds it. `remove_prekey` records whether the sessions lock was actually
        /// held (the core invariant the fix establishes) and flags any prekey delete
        /// whose owning session is not yet durable.
        struct GatedBackend {
            inner: InMemoryBackend,
            // The cache under flush, so the backend can probe the sessions lock.
            cache: StdArc<SignalStoreCache>,
            // Set if a prekey was deleted while the sessions lock was NOT held: that
            // is the regression (prekey drain outside the sessions lock scope).
            drained_without_sessions_lock: StdArc<AtomicBool>,
            // Set if a prekey delete ever ran while its session was still volatile.
            violation: StdArc<AtomicBool>,
            addr_b: String,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl SignalStore for GatedBackend {
            async fn put_sessions_batch(
                &self,
                sessions: &[(Arc<str>, bytes::Bytes)],
            ) -> crate::store::error::Result<()> {
                // A's flush holds the sessions lock here; yield repeatedly so B's
                // spawned decrypt gets scheduled and blocks on that lock before the
                // session commit (and the prekey drain) completes.
                for _ in 0..64 {
                    tokio::task::yield_now().await;
                }
                self.inner.put_sessions_batch(sessions).await
            }
            async fn remove_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                // The fix drains prekeys under the sessions lock, so a try_lock here
                // must fail while a flush is deleting. If it succeeds, the drain ran
                // outside the sessions lock: the exact regression.
                if self.cache.sessions.try_lock().is_some() {
                    self.drained_without_sessions_lock
                        .store(true, Ordering::SeqCst);
                }
                // B's prekey may only be deleted once B's session is durable.
                if id == PREKEY_B
                    && self
                        .inner
                        .get_session(&self.addr_b)
                        .await
                        .unwrap()
                        .is_none()
                {
                    self.violation.store(true, Ordering::SeqCst);
                }
                self.inner.remove_prekey(id).await
            }

            async fn put_identity(
                &self,
                address: &str,
                key: [u8; 32],
            ) -> crate::store::error::Result<()> {
                self.inner.put_identity(address, key).await
            }
            async fn load_identity(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<[u8; 32]>> {
                self.inner.load_identity(address).await
            }
            async fn delete_identity(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_identity(address).await
            }
            async fn get_session(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.inner.get_session(address).await
            }
            async fn put_session(
                &self,
                address: &str,
                session: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.put_session(address, session).await
            }
            async fn delete_session(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_session(address).await
            }
            async fn store_prekey(
                &self,
                id: u32,
                record: &[u8],
                uploaded: bool,
            ) -> crate::store::error::Result<()> {
                self.inner.store_prekey(id, record, uploaded).await
            }
            async fn load_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.inner.load_prekey(id).await
            }
            async fn get_max_prekey_id(&self) -> crate::store::error::Result<u32> {
                self.inner.get_max_prekey_id().await
            }
            async fn store_signed_prekey(
                &self,
                id: u32,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.store_signed_prekey(id, record).await
            }
            async fn load_signed_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.inner.load_signed_prekey(id).await
            }
            async fn load_all_signed_prekeys(
                &self,
            ) -> crate::store::error::Result<Vec<(u32, Vec<u8>)>> {
                self.inner.load_all_signed_prekeys().await
            }
            async fn remove_signed_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.inner.remove_signed_prekey(id).await
            }
            async fn put_sender_key(
                &self,
                address: &str,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.put_sender_key(address, record).await
            }
            async fn get_sender_key(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.inner.get_sender_key(address).await
            }
            async fn delete_sender_key(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_sender_key(address).await
            }
        }

        let inner = InMemoryBackend::new();
        inner
            .store_prekey(PREKEY_A, b"prekey-a", false)
            .await
            .unwrap();
        inner
            .store_prekey(PREKEY_B, b"prekey-b", false)
            .await
            .unwrap();

        let addr_a = ProtocolAddress::new("alice".to_string(), 1.into());
        let addr_b = ProtocolAddress::new("bob".to_string(), 1.into());

        let cache = StdArc::new(SignalStoreCache::new());
        let violation = StdArc::new(AtomicBool::new(false));
        let drained_without_sessions_lock = StdArc::new(AtomicBool::new(false));

        let backend = StdArc::new(GatedBackend {
            inner,
            cache: cache.clone(),
            drained_without_sessions_lock: drained_without_sessions_lock.clone(),
            violation: violation.clone(),
            addr_b: addr_b.as_str().to_string(),
        });

        // Sender A's decrypt: promote A's session, buffer A's consumed prekey.
        cache.put_session(&addr_a, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_A).await;

        // Sender B's decrypt races A's flush: it promotes B's session and buffers
        // B's consumed prekey. put_session must take the sessions lock, so while A's
        // flush holds it (yielding inside put_sessions_batch) B blocks here and can
        // only buffer once A's flush has committed and released the lock.
        let b_cache = cache.clone();
        let addr_b_task = addr_b.clone();
        let b_task = tokio::spawn(async move {
            b_cache
                .put_session(&addr_b_task, SessionRecord::new_fresh())
                .await;
            b_cache.remove_prekey(PREKEY_B).await;
        });

        // A's flush runs concurrently with B's spawned decrypt. It holds the
        // sessions lock across its yielding I/O and the prekey drain, so B cannot
        // insert into removed_prekeys until A is done: A can never delete B's prekey.
        cache.flush(backend.as_ref()).await.unwrap();
        b_task.await.unwrap();

        // The core invariant: every prekey delete during the flush ran while the
        // sessions lock was held, so no concurrent decrypt could have buffered a
        // prekey into the same drain. This is what makes session+prekey atomic.
        assert!(
            !drained_without_sessions_lock.load(Ordering::SeqCst),
            "prekey was drained without holding the sessions lock (regression)"
        );

        // The flush must never have deleted B's prekey while B's session was
        // volatile.
        assert!(
            !violation.load(Ordering::SeqCst),
            "flush deleted B's prekey while B's session was still volatile"
        );

        // A's commit is durable: its session is persisted and its prekey gone.
        assert!(
            backend
                .get_session(addr_a.as_str())
                .await
                .unwrap()
                .is_some(),
            "sender A's session must be durable after its flush"
        );
        assert!(
            backend.load_prekey(PREKEY_A).await.unwrap().is_none(),
            "sender A's consumed prekey must be deleted with its session"
        );

        // B buffered its prekey only after A's flush completed, so B's prekey is
        // still durable and still buffered for B's own next flush.
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_some(),
            "B's prekey must survive a concurrent flush that did not persist B's session"
        );
        assert!(
            cache.removed_prekeys.lock().await.contains(&PREKEY_B),
            "B's prekey removal stays buffered for B's own flush"
        );

        // B's own flush then commits B's session and B's prekey atomically.
        cache.flush(backend.as_ref()).await.unwrap();
        assert!(
            backend
                .get_session(addr_b.as_str())
                .await
                .unwrap()
                .is_some(),
            "B's session must be durable after B's flush"
        );
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_none(),
            "B's prekey is deleted only once B's session is durable"
        );
        assert!(
            !violation.load(Ordering::SeqCst),
            "B's prekey delete must coincide with B's durable session"
        );
    }
}
