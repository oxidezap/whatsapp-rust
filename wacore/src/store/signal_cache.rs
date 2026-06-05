use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::Result;
use async_lock::Mutex;

use crate::libsignal::protocol::{ProtocolAddress, SenderKeyRecord, SessionRecord};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::store::traits::SignalStore;

/// Evict clean (non-dirty, non-deleted) entries from a cache HashMap.
/// Negative entries (None values) are evicted first.
///
/// Amortized: the O(n) scan only runs once the map crosses the high watermark
/// (`max_entries + slack`), then it trims back down to `max_entries`. Steady
/// state over capacity therefore costs O(1) per call because a fresh scan needs
/// `slack` more growth inserts before it can fire again. Call it from every path
/// that grows the map, including read-populate (cache-miss) inserts, so the cache
/// stays bounded even under unique-key read floods; the early-out keeps it cheap.
fn evict_clean_entries<V>(
    cache: &mut HashMap<Arc<str>, Option<V>>,
    dirty: &HashSet<Arc<str>>,
    deleted: Option<&HashSet<Arc<str>>>,
    max_entries: usize,
) {
    if cache.len() <= high_watermark(max_entries) {
        return;
    }
    let overflow = cache.len().saturating_sub(max_entries);
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

/// Slack above `max_entries` the cache may grow to before an eviction scan
/// fires, expressed as a divisor of `max_entries` (1/8th here). Trimming back
/// to `max_entries` then amortizes the O(n) scan over this many inserts. A
/// floor keeps the amortization meaningful when `max_entries` is tiny (tests).
const EVICTION_SLACK_DIVISOR: usize = 8;
const EVICTION_SLACK_FLOOR: usize = 16;

/// The size the cache may reach before a scan is allowed to run. Eviction trims
/// back to `max_entries`, so the strict in-memory bound is this value.
fn high_watermark(max_entries: usize) -> usize {
    max_entries.saturating_add((max_entries / EVICTION_SLACK_DIVISOR).max(EVICTION_SLACK_FLOOR))
}

/// In-memory write-back cache for Signal protocol state.
/// Keys use `Arc<str>` for O(1) clone. Sessions cached as objects (serialized on flush).
/// Capacity-bounded: every path that grows a store (writes and read-populate
/// misses) evicts non-dirty entries once the high watermark is crossed, trimming
/// back to `max_entries` (amortized O(1) thanks to the slack early-out).
pub struct SignalStoreCache {
    sessions: Mutex<SessionStoreState>,
    identities: Mutex<ByteStoreState>,
    sender_keys: Mutex<SenderKeyStoreState>,
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
        if self.cache.len() <= high_watermark(max_entries) {
            return;
        }
        let overflow = self.cache.len().saturating_sub(max_entries);
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
    }

    // === Flush ===

    /// Flush all dirty state to the backend.
    ///
    /// Each store (sessions, identities, sender_keys) is flushed independently
    /// under its own lock. This means:
    /// - Only ONE store is locked during its I/O — the other two are free for
    ///   concurrent encrypt/decrypt operations.
    /// - No race between snapshot and clear — the lock is held throughout, so
    ///   mutations to the same store are blocked until the flush completes.
    /// - Dirty sets are cleared only after successful writes.
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
mod eviction_tests {
    use super::*;
    use crate::libsignal::protocol::{DeviceId, ProtocolAddress};
    use crate::store::in_memory::InMemoryBackend;

    fn addr(i: usize) -> ProtocolAddress {
        ProtocolAddress::new(format!("user{i}@s.whatsapp.net"), DeviceId::new(0))
    }

    #[test]
    fn high_watermark_is_above_max_and_amortizes() {
        // The watermark must sit strictly above max_entries so a scan can fire
        // only after `slack` extra inserts, otherwise the amortization is lost.
        assert!(high_watermark(2_000) > 2_000);
        assert_eq!(
            high_watermark(2_000),
            2_000 + 2_000 / EVICTION_SLACK_DIVISOR
        );
        // Tiny caps still get a meaningful slack via the floor.
        assert_eq!(high_watermark(4), 4 + EVICTION_SLACK_FLOOR);
    }

    #[tokio::test]
    async fn eviction_bounds_cache_over_many_inserts() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Flush after each put so the prior entry becomes clean (non-dirty) and
        // therefore evictable on the next put; otherwise every entry is pinned.
        for i in 0..(max * 4) {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
            cache.flush(&backend).await.unwrap();
        }

        let len = cache.identities.lock().await.cache.len();
        assert!(
            len <= high_watermark(max),
            "cache grew past the high watermark: len={len} watermark={}",
            high_watermark(max)
        );
        // It must still be doing real work, not collapsing to empty.
        assert!(
            len >= max,
            "eviction was too aggressive: len={len} max={max}"
        );
    }

    #[tokio::test]
    async fn read_over_capacity_stays_bounded() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Push the identity store right up to the watermark with clean entries.
        let watermark = high_watermark(max);
        for i in 0..watermark {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
            cache.flush(&backend).await.unwrap();
        }
        let before = cache.identities.lock().await.cache.len();
        assert_eq!(before, watermark, "setup should fill exactly to watermark");

        // A read-populate (cache-miss) that crosses the watermark must trigger the
        // amortized eviction too: read traffic populates the cache, so it cannot be
        // allowed to grow it unbounded.
        let missing = addr(watermark + 1);
        let got = cache.get_identity(&missing, &backend).await.unwrap();
        assert!(got.is_none());

        let after = cache.identities.lock().await.cache.len();
        assert!(
            after <= watermark,
            "a read over capacity must stay bounded: after={after} watermark={watermark}"
        );
    }

    #[tokio::test]
    async fn read_flood_of_unique_keys_stays_bounded() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // A flood of unique cache-miss reads each negative-cache a clean entry.
        // Without read-path eviction this grew without bound; it must stay bounded.
        for i in 0..(max * 8) {
            assert!(
                cache
                    .get_identity(&addr(i), &backend)
                    .await
                    .unwrap()
                    .is_none()
            );
        }

        let len = cache.identities.lock().await.cache.len();
        assert!(
            len <= high_watermark(max),
            "unique-read flood must stay bounded: len={len} watermark={}",
            high_watermark(max)
        );
    }

    #[tokio::test]
    async fn dirty_entries_are_never_evicted() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);

        // Every put marks the key dirty and we never flush, so all entries are
        // pinned. Even far past the watermark, none may be dropped.
        let total = high_watermark(max) * 2;
        for i in 0..total {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
        }

        let len = cache.identities.lock().await.cache.len();
        assert_eq!(
            len, total,
            "dirty (unflushed) entries must never be evicted"
        );
    }

    #[tokio::test]
    async fn checked_out_sessions_are_never_evicted() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Persist one session, then check it out (get_session leaves a CheckedOut
        // marker) so eviction must skip it.
        let pinned = addr(0);
        cache.put_session(&pinned, SessionRecord::new_fresh()).await;
        cache.flush(&backend).await.unwrap();
        let taken = cache.get_session(&pinned, &backend).await.unwrap();
        assert!(taken.is_some(), "session should be present before checkout");

        // Flood the session store with clean Absent markers (has_session misses)
        // so the watermark is crossed, then trigger eviction via a put.
        let watermark = high_watermark(max);
        for i in 1..(watermark + 8) {
            // has_session miss negative-caches an Absent entry (a read, no evict).
            assert!(!cache.has_session(&addr(i), &backend).await.unwrap());
        }
        // A put fires the eviction scan; it must drop clean Absent markers but
        // keep the CheckedOut session pinned.
        cache
            .put_session(&addr(99_999), SessionRecord::new_fresh())
            .await;

        {
            let state = cache.sessions.lock().await;
            let entry = state.cache.get(pinned.as_str());
            assert!(
                matches!(entry, Some(SessionEntry::CheckedOut)),
                "checked-out session must survive eviction"
            );
            assert!(
                state.cache.len() <= high_watermark(max) + 1,
                "eviction must bound the session cache: len={}",
                state.cache.len()
            );
        }
    }
}
