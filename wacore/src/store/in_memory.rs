//! In-memory implementation of the [`Backend`] trait.
//!
//! Intended for testing and as a reference implementation for FFI bridges.
//! All data lives in RAM behind a single [`async_lock::Mutex`] and is lost
//! when the struct is dropped.

use hashbrown::hash_map::Entry;
use hashbrown::{Equivalent, HashMap as HbHashMap, HashSet as HbHashSet};
use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::Hash;
use std::sync::Arc;
#[cfg(any(test, feature = "test-util"))]
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::atomic::{AtomicI32, Ordering};

use crate::appstate::hash::HashState;
use crate::store::Device;
use crate::store::error::Result;
use crate::store::traits::*;
use async_lock::Mutex;
use async_trait::async_trait;
use bytes::Bytes;
use wacore_appstate::processor::AppStateMutationMAC;

/// Key for the sent-message store: `(chat_jid, message_id)`.
type SentMessageKey = (String, String);

/// Value stored alongside a sent message (includes timestamp for expiration).
struct SentMessageEntry {
    payload: Vec<u8>,
    timestamp: i64,
}

/// Key for pre-keys: `id`.
struct PreKeyEntry {
    record: Bytes,
}

/// Key for base-key collision detection: `(address, message_id)`.
type BaseKeyKey = (String, String);

/// Stored msg-secret value: `(secret_bytes, expires_at_secs, message_ts_secs)`.
type MsgSecretRow = (MessageSecret, i64, i64);

#[derive(Eq, Hash, PartialEq)]
struct MsgSecretKey {
    chat: Arc<str>,
    sender: Arc<str>,
    msg_id: Arc<str>,
}

#[derive(Hash)]
struct MsgSecretKeyRef<'a> {
    chat: &'a str,
    sender: &'a str,
    msg_id: &'a str,
}

impl Equivalent<MsgSecretKey> for MsgSecretKeyRef<'_> {
    fn equivalent(&self, key: &MsgSecretKey) -> bool {
        self.chat == key.chat.as_ref()
            && self.sender == key.sender.as_ref()
            && self.msg_id == key.msg_id.as_ref()
    }
}

type MsgSecretMap = HbHashMap<MsgSecretKey, MsgSecretRow, RandomState>;

/// One logical secret, ignoring which sender alias a row was filed under.
/// Eviction groups by this so a message's aliases are kept or dropped together.
///
/// Only `msg_id` is hashed. A stanza id already names one message across the
/// account, so mixing `chat` into the hash buys no selectivity and doubles the
/// string hashing on a path that runs over every row of the cutoff's tie
/// bucket. `chat` still decides equality, so a collision stays correct.
#[derive(Eq, PartialEq)]
struct MsgGroupKey {
    chat: Arc<str>,
    msg_id: Arc<str>,
}

impl Hash for MsgGroupKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.msg_id.hash(state);
    }
}

struct MsgGroupKeyRef<'a> {
    chat: &'a str,
    msg_id: &'a str,
}

impl Hash for MsgGroupKeyRef<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.msg_id.hash(state);
    }
}

impl Equivalent<MsgGroupKey> for MsgGroupKeyRef<'_> {
    fn equivalent(&self, key: &MsgGroupKey) -> bool {
        self.chat == key.chat.as_ref() && self.msg_id == key.msg_id.as_ref()
    }
}

/// Inner state protected by the mutex.
#[derive(Default)]
struct InMemoryState {
    // --- Signal ---
    identities: HashMap<String, [u8; 32]>,
    sessions: HashMap<String, Bytes>,
    prekeys: HashMap<u32, PreKeyEntry>,
    signed_prekeys: HashMap<u32, Vec<u8>>,
    sender_keys: HashMap<String, Vec<u8>>,

    // --- AppSync ---
    sync_keys: HashMap<Vec<u8>, AppStateSyncKey>,
    latest_sync_key_id: Option<Vec<u8>>,
    versions: HashMap<String, HashState>,
    /// `(collection_name, hex(index_mac))` -> `value_mac`
    mutation_macs: HashMap<(String, Vec<u8>), Vec<u8>>,

    // --- Protocol ---
    /// Unified per-device sender key tracking: group_jid -> (device_jid -> has_key)
    sender_key_devices: HashMap<String, HashMap<String, bool>>,
    lid_mappings: HashMap<String, LidPnMappingEntry>,
    /// Reverse index: phone_number -> lid
    pn_to_lid: HashMap<String, String>,
    base_keys: HashMap<BaseKeyKey, Vec<u8>>,
    device_lists: HashMap<String, DeviceListRecord>,
    group_metadata: HashMap<String, Vec<u8>>,
    tc_tokens: HashMap<String, TcTokenEntry>,
    sent_messages: HashMap<SentMessageKey, SentMessageEntry>,
    /// Pending inbound durability buffer: (chat, sender, id) -> (message, inserted_at).
    pending_inbound: HashMap<(String, String, String), (Vec<u8>, i64)>,

    // --- MsgSecret ---
    /// `expires_at = 0` means never expire; `message_ts = 0` means the parent
    /// event time is unknown. The keepalive cleanup prunes expired rows.
    msg_secrets: MsgSecretMap,
    /// Map length at which `trim_msg_secrets` is next allowed to do its O(n)
    /// evictable-row scan. Purely an optimisation: a stale value can only cause
    /// an extra scan, never a missed eviction.
    msg_secrets_rescan_at: usize,

    // --- Device ---
    device: Option<Device>,
}

/// Hard cap on retained sent messages, bounding memory regardless of the
/// configured retention window. Time-based pruning is the client's keepalive
/// sweep (`delete_expired_sent_messages`, driven by
/// `CacheConfig::sent_message_ttl_secs`, the single source of truth for the
/// time window); this cap only guards against a burst between sweeps.
const MAX_SENT_MESSAGES: usize = 4096;

/// Hard cap on retained message secrets, the `msg_secrets` counterpart of
/// [`MAX_SENT_MESSAGES`] and there for the same reason: time-based pruning
/// (`delete_expired_msg_secrets`, driven by the client's keepalive sweep)
/// cannot reclaim anything inside a session, because the default `Managed`
/// policy dates every row 30-90 days out. Without a cap the map is one row per
/// message for the life of the process.
///
/// That is a footprint bug specifically on wasm32, where the allocator never
/// returns pages: the table doubles by reallocation, so the old and the new
/// table are briefly live together, and the ~1.5x spike stays committed in
/// linear memory even after the rows are dropped. A 30k-message session
/// reallocated this table to 4.56 MiB (65536 buckets x 73 B) and committed
/// ~7 MiB it never gave back.
///
/// Sized at 2x [`MAX_SENT_MESSAGES`] and 2x the client's message-secret
/// write-behind high-water mark, so a burst that fills both still fits.
const MAX_MSG_SECRETS: usize = 8192;

/// In-memory implementation of the full [`Backend`] trait.
///
/// Thread-safe and runtime-agnostic (uses [`async_lock::Mutex`]).
/// All data is ephemeral — it lives only as long as this struct.
pub struct InMemoryBackend {
    state: Mutex<InMemoryState>,
    next_device_id: AtomicI32,
    /// Count of `put_sessions_batch` calls. Test hook (see `test-util`): lets a
    /// harness prove receive-path flush coalescing (N receives collapse to fewer
    /// batch writes). Gated so normal builds carry neither the field nor the
    /// per-call bookkeeping.
    #[cfg(any(test, feature = "test-util"))]
    session_batch_writes: AtomicU32,
    /// Count of `put_sender_keys_batch` calls. Test hook for sender-key lease
    /// boundaries; absent from normal builds.
    #[cfg(any(test, feature = "test-util"))]
    sender_key_batch_writes: AtomicU32,
    /// When set, `put_sessions_batch` fails. Test hook (see `test-util`): lets a
    /// harness prove the send path aborts (and never hits the wire) when the
    /// ratchet advance cannot be persisted.
    #[cfg(any(test, feature = "test-util"))]
    fail_session_writes: AtomicBool,
    /// When set, `put_sender_keys_batch` fails. Test hook: the sender-key
    /// counterpart of `fail_session_writes` (wire gate must survive a failed
    /// flush).
    #[cfg(any(test, feature = "test-util"))]
    fail_sender_key_writes: AtomicBool,
    /// Parks the next `load_signed_prekey` on a barrier. Test hook: lets a
    /// harness promote a rotated key while a lookup is already in flight,
    /// which is the window a caller's re-read exists to close.
    #[cfg(any(test, feature = "test-util"))]
    signed_prekey_read_gate: std::sync::Mutex<Option<Arc<async_lock::Barrier>>>,
}

impl InMemoryBackend {
    /// Create a new, empty in-memory store.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InMemoryState::default()),
            next_device_id: AtomicI32::new(1),
            #[cfg(any(test, feature = "test-util"))]
            session_batch_writes: AtomicU32::new(0),
            #[cfg(any(test, feature = "test-util"))]
            sender_key_batch_writes: AtomicU32::new(0),
            #[cfg(any(test, feature = "test-util"))]
            fail_session_writes: AtomicBool::new(false),
            #[cfg(any(test, feature = "test-util"))]
            fail_sender_key_writes: AtomicBool::new(false),
            #[cfg(any(test, feature = "test-util"))]
            signed_prekey_read_gate: std::sync::Mutex::new(None),
        }
    }

    /// Number of `put_sessions_batch` attempts since construction, including
    /// injected failures.
    #[cfg(any(test, feature = "test-util"))]
    pub fn session_batch_write_count(&self) -> u32 {
        self.session_batch_writes.load(Ordering::Relaxed)
    }

    /// Number of `put_sender_keys_batch` attempts since construction, including
    /// injected failures.
    #[cfg(any(test, feature = "test-util"))]
    pub fn sender_key_batch_write_count(&self) -> u32 {
        self.sender_key_batch_writes.load(Ordering::Relaxed)
    }

    /// Make every subsequent `put_sessions_batch` fail (or stop failing).
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_fail_session_writes(&self, fail: bool) {
        self.fail_session_writes.store(fail, Ordering::Relaxed);
    }

    /// Make every subsequent `put_sender_keys_batch` fail (or stop failing).
    #[cfg(any(test, feature = "test-util"))]
    pub fn set_fail_sender_key_writes(&self, fail: bool) {
        self.fail_sender_key_writes.store(fail, Ordering::Relaxed);
    }

    /// Park the next `load_signed_prekey` on `gate`, which it waits on twice:
    /// once to signal it has arrived, once to be released.
    #[cfg(any(test, feature = "test-util"))]
    pub fn gate_next_signed_prekey_read(&self, gate: Arc<async_lock::Barrier>) {
        *self
            .signed_prekey_read_gate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(gate);
    }

    /// Lets recovery tests remove only the state needed to trigger a key request.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn remove_sync_key_for_test(&self, key_id: &[u8]) -> bool {
        self.state.lock().await.sync_keys.remove(key_id).is_some()
    }

    /// Keeps readiness failures attributable without exposing key material.
    #[cfg(any(test, feature = "test-util"))]
    pub async fn sync_key_count_for_test(&self) -> usize {
        self.state.lock().await.sync_keys.len()
    }
}

impl Default for InMemoryBackend {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SignalStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignalStore for InMemoryBackend {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()> {
        self.state
            .lock()
            .await
            .identities
            .insert(address.to_string(), key);
        Ok(())
    }

    async fn load_identity(&self, address: &str) -> Result<Option<[u8; 32]>> {
        Ok(self.state.lock().await.identities.get(address).copied())
    }

    async fn delete_identity(&self, address: &str) -> Result<()> {
        self.state.lock().await.identities.remove(address);
        Ok(())
    }

    async fn get_session(&self, address: &str) -> Result<Option<Bytes>> {
        Ok(self.state.lock().await.sessions.get(address).cloned())
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()> {
        self.state
            .lock()
            .await
            .sessions
            .insert(address.to_string(), Bytes::copy_from_slice(session));
        Ok(())
    }

    async fn put_sessions_batch(&self, sessions: &[(Arc<str>, Bytes)]) -> Result<()> {
        #[cfg(any(test, feature = "test-util"))]
        {
            self.session_batch_writes.fetch_add(1, Ordering::Relaxed);
            if self.fail_session_writes.load(Ordering::Relaxed) {
                return Err(crate::store::error::StoreError::Io(std::io::Error::other(
                    "put_sessions_batch failing (test hook)",
                )));
            }
        }
        let mut state = self.state.lock().await;
        state.sessions.reserve(sessions.len());
        for (address, session) in sessions {
            if let Some(stored) = state.sessions.get_mut(address.as_ref()) {
                *stored = session.clone();
            } else {
                state.sessions.insert(address.to_string(), session.clone());
            }
        }
        Ok(())
    }

    async fn has_session(&self, address: &str) -> Result<bool> {
        Ok(self.state.lock().await.sessions.contains_key(address))
    }

    async fn has_signal_state_for_user(&self, user: &str) -> Result<bool> {
        fn matches(addr: &str, user: &str) -> bool {
            addr.strip_prefix(user)
                .is_some_and(|rest| rest.starts_with('@') || rest.starts_with(':'))
        }
        let state = self.state.lock().await;
        Ok(state.sessions.keys().any(|k| matches(k, user))
            || state.identities.keys().any(|k| matches(k, user)))
    }

    async fn delete_session(&self, address: &str) -> Result<()> {
        self.state.lock().await.sessions.remove(address);
        Ok(())
    }

    async fn store_prekey(&self, id: u32, record: &[u8], _uploaded: bool) -> Result<()> {
        self.state.lock().await.prekeys.insert(
            id,
            PreKeyEntry {
                record: Bytes::copy_from_slice(record),
            },
        );
        Ok(())
    }

    async fn mark_prekeys_uploaded(&self, _ids: &[u32]) -> Result<()> {
        // The in-memory store does not track the uploaded flag (see
        // store_prekey); the contract that matters is NOT resurrecting
        // deleted rows, which a no-op trivially satisfies.
        Ok(())
    }

    async fn store_prekeys_batch(&self, keys: &[(u32, Bytes)], _uploaded: bool) -> Result<()> {
        let mut state = self.state.lock().await;
        // Growing one insert at a time allocates and copies a whole table per
        // rehash, and a connect-sized batch arriving at an empty map crosses
        // the load factor eight times. The batch length is known, so the table
        // can reach its final size in one allocation instead.
        //
        // Two things stop that reservation from over-growing a table, because a
        // table grown for rows that were never added does not shrink back and
        // this is meant to cost no retained bytes:
        //
        // 1. Subtract the rows already stored. A batch may legally overwrite
        //    ids, and no batch can overwrite more rows than exist, so
        //    `keys.len() - len()` is the floor on how many ids must be new.
        // 2. Only reserve at all when the batch is strictly ascending, which
        //    proves its ids are distinct. Without that, 812 entries sharing one
        //    id would reserve a 1024-bucket table to hold a single row. Testing
        //    the order costs one pass of integer compares and no allocation;
        //    deduplicating properly would need a set, whose own allocation and
        //    812 hashes cost more than the eight allocations being saved.
        //
        // Both are one-sided: they can only under-reserve and fall back to
        // incremental growth, never inflate the resident table. The connect path
        // satisfies both — the map is empty and `upload_pre_keys_pass` emits
        // `gen_start + i`, so the whole batch is reserved and gets the full win.
        //
        // This does NOT shrink the table that stays resident: the final
        // capacity is the same either way, so it buys allocator traffic and
        // in-call headroom, not retained bytes.
        let ascending = keys.windows(2).all(|pair| pair[0].0 < pair[1].0);
        if ascending {
            let at_least_new = keys.len().saturating_sub(state.prekeys.len());
            state.prekeys.reserve(at_least_new);
        }
        for (id, record) in keys {
            state.prekeys.insert(
                *id,
                PreKeyEntry {
                    record: record.clone(),
                },
            );
        }
        Ok(())
    }

    async fn load_prekey(&self, id: u32) -> Result<Option<Bytes>> {
        Ok(self
            .state
            .lock()
            .await
            .prekeys
            .get(&id)
            .map(|e| e.record.clone()))
    }

    async fn load_prekeys_batch(&self, ids: &[u32]) -> Result<Vec<(u32, Bytes)>> {
        let state = self.state.lock().await;
        let mut result = Vec::with_capacity(ids.len());
        for &id in ids {
            if let Some(entry) = state.prekeys.get(&id) {
                result.push((id, entry.record.clone()));
            }
        }
        Ok(result)
    }

    async fn remove_prekey(&self, id: u32) -> Result<()> {
        self.state.lock().await.prekeys.remove(&id);
        Ok(())
    }

    async fn get_max_prekey_id(&self) -> Result<u32> {
        Ok(self
            .state
            .lock()
            .await
            .prekeys
            .keys()
            .copied()
            .max()
            .unwrap_or(0))
    }

    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()> {
        self.state
            .lock()
            .await
            .signed_prekeys
            .insert(id, record.to_vec());
        Ok(())
    }

    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        #[cfg(any(test, feature = "test-util"))]
        {
            // Taken, not borrowed, so the guard never crosses the await and
            // only the first read after arming is gated.
            let gate = self
                .signed_prekey_read_gate
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .take();
            if let Some(gate) = gate {
                gate.wait().await;
                gate.wait().await;
            }
        }
        Ok(self.state.lock().await.signed_prekeys.get(&id).cloned())
    }

    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>> {
        Ok(self
            .state
            .lock()
            .await
            .signed_prekeys
            .iter()
            .map(|(id, rec)| (*id, rec.clone()))
            .collect())
    }

    async fn remove_signed_prekey(&self, id: u32) -> Result<()> {
        self.state.lock().await.signed_prekeys.remove(&id);
        Ok(())
    }

    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()> {
        #[cfg(any(test, feature = "test-util"))]
        if self.fail_sender_key_writes.load(Ordering::Relaxed) {
            return Err(crate::store::error::StoreError::Io(std::io::Error::other(
                "put_sender_key failing (test hook)",
            )));
        }
        self.state
            .lock()
            .await
            .sender_keys
            .insert(address.to_string(), record.to_vec());
        Ok(())
    }

    async fn put_sender_keys_batch(&self, sender_keys: &[(Arc<str>, Bytes)]) -> Result<()> {
        #[cfg(any(test, feature = "test-util"))]
        {
            self.sender_key_batch_writes.fetch_add(1, Ordering::Relaxed);
            if self.fail_sender_key_writes.load(Ordering::Relaxed) {
                return Err(crate::store::error::StoreError::Io(std::io::Error::other(
                    "put_sender_keys_batch failing (test hook)",
                )));
            }
        }
        let mut state = self.state.lock().await;
        state.sender_keys.reserve(sender_keys.len());
        for (address, record) in sender_keys {
            if let Some(stored) = state.sender_keys.get_mut(address.as_ref()) {
                stored.clear();
                stored.extend_from_slice(record);
            } else {
                state
                    .sender_keys
                    .insert(address.to_string(), record.to_vec());
            }
        }
        Ok(())
    }

    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().await.sender_keys.get(address).cloned())
    }

    async fn delete_sender_key(&self, address: &str) -> Result<()> {
        self.state.lock().await.sender_keys.remove(address);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppSyncStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl AppSyncStore for InMemoryBackend {
    async fn get_sync_key(&self, key_id: &[u8]) -> Result<Option<AppStateSyncKey>> {
        Ok(self.state.lock().await.sync_keys.get(key_id).cloned())
    }

    async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> Result<()> {
        let mut s = self.state.lock().await;
        s.sync_keys.insert(key_id.to_vec(), key);
        s.latest_sync_key_id = Some(key_id.to_vec());
        Ok(())
    }

    async fn get_version(&self, name: &str) -> Result<HashState> {
        Ok(self
            .state
            .lock()
            .await
            .versions
            .get(name)
            .cloned()
            .unwrap_or_default())
    }

    async fn set_version(&self, name: &str, state: HashState) -> Result<()> {
        self.state
            .lock()
            .await
            .versions
            .insert(name.to_string(), state);
        Ok(())
    }

    async fn put_mutation_macs(
        &self,
        name: &str,
        _version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> Result<()> {
        let mut s = self.state.lock().await;
        for m in mutations {
            s.mutation_macs
                .insert((name.to_string(), m.index_mac.clone()), m.value_mac.clone());
        }
        Ok(())
    }

    async fn get_mutation_mac(&self, name: &str, index_mac: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .state
            .lock()
            .await
            .mutation_macs
            .get(&(name.to_string(), index_mac.to_vec()))
            .cloned())
    }

    async fn delete_mutation_macs(&self, name: &str, index_macs: &[Vec<u8>]) -> Result<()> {
        let mut s = self.state.lock().await;
        for im in index_macs {
            s.mutation_macs.remove(&(name.to_string(), im.clone()));
        }
        Ok(())
    }

    async fn clear_mutation_macs(&self, name: &str) -> Result<()> {
        self.state
            .lock()
            .await
            .mutation_macs
            .retain(|(n, _), _| n != name);
        Ok(())
    }

    async fn get_latest_sync_key_id(&self) -> Result<Option<Vec<u8>>> {
        Ok(self.state.lock().await.latest_sync_key_id.clone())
    }
}

// ---------------------------------------------------------------------------
// ProtocolStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ProtocolStore for InMemoryBackend {
    // --- Per-Device Sender Key Tracking ---

    async fn get_sender_key_devices(&self, group_jid: &str) -> Result<Vec<(String, bool)>> {
        Ok(self
            .state
            .lock()
            .await
            .sender_key_devices
            .get(group_jid)
            .map(|map| map.iter().map(|(k, v)| (k.clone(), *v)).collect())
            .unwrap_or_default())
    }

    async fn set_sender_key_status(&self, group_jid: &str, entries: &[(&str, bool)]) -> Result<()> {
        let mut s = self.state.lock().await;
        let map = s
            .sender_key_devices
            .entry(group_jid.to_string())
            .or_default();
        for (device_jid, has_key) in entries {
            map.insert(device_jid.to_string(), *has_key);
        }
        Ok(())
    }

    async fn clear_sender_key_devices(&self, group_jid: &str) -> Result<()> {
        self.state.lock().await.sender_key_devices.remove(group_jid);
        Ok(())
    }

    async fn clear_all_sender_key_devices(&self) -> Result<()> {
        self.state.lock().await.sender_key_devices.clear();
        Ok(())
    }

    async fn delete_sender_key_device_rows(&self, device_jids: &[&str]) -> Result<()> {
        if device_jids.is_empty() {
            return Ok(());
        }
        let mut state = self.state.lock().await;
        for group_map in state.sender_key_devices.values_mut() {
            group_map.retain(|jid, _| !device_jids.contains(&jid.as_str()));
        }
        Ok(())
    }

    // --- LID-PN Mapping ---

    async fn get_lid_mapping(&self, lid: &str) -> Result<Option<LidPnMappingEntry>> {
        Ok(self.state.lock().await.lid_mappings.get(lid).cloned())
    }

    async fn get_pn_mapping(&self, phone: &str) -> Result<Option<LidPnMappingEntry>> {
        let s = self.state.lock().await;
        let entry = s
            .pn_to_lid
            .get(phone)
            .and_then(|lid| s.lid_mappings.get(lid))
            .cloned();
        Ok(entry)
    }

    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> Result<()> {
        let mut s = self.state.lock().await;
        // Remove stale reverse entry if the LID was previously mapped to a different phone number
        if let Some(old_phone) = s
            .lid_mappings
            .get(&entry.lid)
            .filter(|old| old.phone_number != entry.phone_number)
            .map(|old| old.phone_number.clone())
        {
            s.pn_to_lid.remove(&old_phone);
        }
        s.pn_to_lid
            .insert(entry.phone_number.clone(), entry.lid.clone());
        s.lid_mappings.insert(entry.lid.clone(), entry.clone());
        Ok(())
    }

    async fn get_all_lid_mappings(&self) -> Result<Vec<LidPnMappingEntry>> {
        Ok(self
            .state
            .lock()
            .await
            .lid_mappings
            .values()
            .cloned()
            .collect())
    }

    // --- Base Key Collision Detection ---

    async fn save_base_key(&self, address: &str, message_id: &str, base_key: &[u8]) -> Result<()> {
        self.state.lock().await.base_keys.insert(
            (address.to_string(), message_id.to_string()),
            base_key.to_vec(),
        );
        Ok(())
    }

    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> Result<bool> {
        let s = self.state.lock().await;
        let same = s
            .base_keys
            .get(&(address.to_string(), message_id.to_string()))
            .is_some_and(|stored| stored == current_base_key);
        Ok(same)
    }

    async fn delete_base_key(&self, address: &str, message_id: &str) -> Result<()> {
        self.state
            .lock()
            .await
            .base_keys
            .remove(&(address.to_string(), message_id.to_string()));
        Ok(())
    }

    // --- Device Registry ---

    async fn update_device_list(&self, record: DeviceListRecord) -> Result<()> {
        self.state
            .lock()
            .await
            .device_lists
            .insert(record.user.clone(), record);
        Ok(())
    }

    async fn get_devices(&self, user: &str) -> Result<Option<DeviceListRecord>> {
        Ok(self.state.lock().await.device_lists.get(user).cloned())
    }

    async fn delete_devices(&self, user: &str) -> Result<()> {
        self.state.lock().await.device_lists.remove(user);
        Ok(())
    }

    async fn get_group_metadata(&self, group_jid: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .state
            .lock()
            .await
            .group_metadata
            .get(group_jid)
            .cloned())
    }

    async fn put_group_metadata(&self, group_jid: &str, blob: &[u8]) -> Result<()> {
        self.state
            .lock()
            .await
            .group_metadata
            .insert(group_jid.to_string(), blob.to_vec());
        Ok(())
    }

    async fn delete_group_metadata(&self, group_jid: &str) -> Result<()> {
        self.state.lock().await.group_metadata.remove(group_jid);
        Ok(())
    }

    // --- TcToken Storage ---

    async fn get_tc_token(&self, jid: &str) -> Result<Option<TcTokenEntry>> {
        Ok(self.state.lock().await.tc_tokens.get(jid).cloned())
    }

    async fn put_tc_token(&self, jid: &str, entry: &TcTokenEntry) -> Result<()> {
        self.state
            .lock()
            .await
            .tc_tokens
            .insert(jid.to_string(), entry.clone());
        Ok(())
    }

    async fn delete_tc_token(&self, jid: &str) -> Result<()> {
        self.state.lock().await.tc_tokens.remove(jid);
        Ok(())
    }

    async fn get_all_tc_token_jids(&self) -> Result<Vec<String>> {
        Ok(self.state.lock().await.tc_tokens.keys().cloned().collect())
    }

    async fn delete_expired_tc_tokens(&self, token_cutoff: i64, sender_cutoff: i64) -> Result<u32> {
        let mut s = self.state.lock().await;
        let before = s.tc_tokens.len();
        // Keep a row while either window is still live: the received token or the
        // sender bucket. A row is dropped only when both are stale.
        s.tc_tokens.retain(|_, entry| {
            let token_live = !entry.token.is_empty() && entry.token_timestamp >= token_cutoff;
            let sender_live = entry.sender_timestamp.is_some_and(|ts| ts >= sender_cutoff);
            token_live || sender_live
        });
        Ok((before - s.tc_tokens.len()) as u32)
    }

    async fn touch_tc_token_sender_timestamp(
        &self,
        jid: &str,
        sender_timestamp: i64,
    ) -> Result<()> {
        let mut s = self.state.lock().await;
        match s.tc_tokens.get_mut(jid) {
            Some(entry) => {
                entry.sender_timestamp = Some(
                    entry
                        .sender_timestamp
                        .map_or(sender_timestamp, |e| e.max(sender_timestamp)),
                );
            }
            None => {
                s.tc_tokens.insert(
                    jid.to_string(),
                    TcTokenEntry {
                        token: Vec::new(),
                        token_timestamp: sender_timestamp,
                        sender_timestamp: Some(sender_timestamp),
                    },
                );
            }
        }
        Ok(())
    }

    async fn store_received_tc_token(
        &self,
        jid: &str,
        token: &[u8],
        token_timestamp: i64,
    ) -> Result<()> {
        let mut s = self.state.lock().await;
        match s.tc_tokens.get_mut(jid) {
            Some(entry) => {
                // Newer-wins (see the trait doc): don't let a stale write
                // clobber a fresher token.
                if entry.token.is_empty() || token_timestamp >= entry.token_timestamp {
                    entry.token = token.to_vec();
                    entry.token_timestamp = token_timestamp;
                    // sender_timestamp left untouched
                }
            }
            None => {
                s.tc_tokens.insert(
                    jid.to_string(),
                    TcTokenEntry {
                        token: token.to_vec(),
                        token_timestamp,
                        sender_timestamp: None,
                    },
                );
            }
        }
        Ok(())
    }

    // --- Sent Message Store ---

    async fn store_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<()> {
        let now = crate::time::now_secs();
        let mut s = self.state.lock().await;

        // Memory bound only: when the map hits the cap, drop the oldest entries
        // (by timestamp) down to 3/4 of it so this scan amortizes across many
        // inserts. Time-based pruning is the caller's keepalive sweep.
        //
        // Only the timestamps are collected: cloning every key to sort them
        // allocated two Strings per retained entry on each eviction (4096 keys
        // per 1024 inserts under load) while holding the state lock, which
        // showed up both as per-message churn and as a latency spike.
        // `select_nth_unstable` finds the cutoff in O(n) without ordering the
        // rest, then two passes apply it: everything strictly older goes, and
        // the cutoff's own bucket tops the removal up to the exact count. The
        // split is what keeps the policy oldest-first, since map iteration
        // order is arbitrary and a single pass could evict an entry AT the
        // cutoff while keeping one below it. The exact count matters because a
        // flood puts every entry in the same second: with one bucket for the
        // whole map, dropping all of "timestamp <= cutoff" would clear it.
        if s.sent_messages.len() >= MAX_SENT_MESSAGES {
            let target = MAX_SENT_MESSAGES * 3 / 4;
            let drop_count = s.sent_messages.len().saturating_sub(target);
            if drop_count > 0 {
                let mut ages: Vec<i64> = s.sent_messages.values().map(|e| e.timestamp).collect();
                let (_, &mut cutoff, _) = ages.select_nth_unstable(drop_count - 1);
                let mut removed = 0usize;
                s.sent_messages.retain(|_, e| {
                    if e.timestamp < cutoff {
                        removed += 1;
                        false
                    } else {
                        true
                    }
                });
                let mut remaining = drop_count.saturating_sub(removed);
                if remaining > 0 {
                    s.sent_messages.retain(|_, e| {
                        if remaining > 0 && e.timestamp == cutoff {
                            remaining -= 1;
                            false
                        } else {
                            true
                        }
                    });
                }
            }
        }

        s.sent_messages.insert(
            (chat_jid.to_string(), message_id.to_string()),
            SentMessageEntry {
                payload: payload.to_vec(),
                timestamp: now,
            },
        );
        Ok(())
    }

    async fn take_sent_message(&self, chat_jid: &str, message_id: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .state
            .lock()
            .await
            .sent_messages
            .remove(&(chat_jid.to_string(), message_id.to_string()))
            .map(|e| e.payload))
    }

    async fn delete_expired_sent_messages(&self, cutoff_timestamp: i64) -> Result<u32> {
        let mut s = self.state.lock().await;
        let before = s.sent_messages.len();
        s.sent_messages
            .retain(|_, entry| entry.timestamp >= cutoff_timestamp);
        Ok((before - s.sent_messages.len()) as u32)
    }

    async fn store_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
        message: &[u8],
    ) -> Result<()> {
        let now = crate::time::now_secs();
        self.state.lock().await.pending_inbound.insert(
            (chat.to_string(), sender.to_string(), id.to_string()),
            (message.to_vec(), now),
        );
        Ok(())
    }

    async fn get_pending_inbound(
        &self,
        chat: &str,
        sender: &str,
        id: &str,
    ) -> Result<Option<Vec<u8>>> {
        let key = (chat.to_string(), sender.to_string(), id.to_string());
        Ok(self
            .state
            .lock()
            .await
            .pending_inbound
            .get(&key)
            .map(|(bytes, _)| bytes.clone()))
    }

    async fn delete_pending_inbound(&self, chat: &str, sender: &str, id: &str) -> Result<()> {
        let key = (chat.to_string(), sender.to_string(), id.to_string());
        self.state.lock().await.pending_inbound.remove(&key);
        Ok(())
    }

    async fn delete_expired_pending_inbound(&self, cutoff_timestamp: i64) -> Result<u32> {
        let mut s = self.state.lock().await;
        let before = s.pending_inbound.len();
        s.pending_inbound
            .retain(|_, (_, inserted_at)| *inserted_at >= cutoff_timestamp);
        Ok((before - s.pending_inbound.len()) as u32)
    }
}

// ---------------------------------------------------------------------------
// MsgSecretStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MsgSecretStore for InMemoryBackend {
    async fn put_msg_secrets(&self, mut entries: Vec<MsgSecretEntry>) -> Result<usize> {
        use crate::store::traits::{merge_msg_secret_expiry, merge_msg_secret_message_ts};
        let stored = entries.len();
        // Only a batch long enough to be chunked below can have a chunk boundary
        // fall inside a message, and only then does the order matter. Sorting
        // groups a message's sender alias rows together so the boundary check
        // can see them: the client's write-behind buffer snapshots its pending
        // set from a `HashMap`, so the aliases it queued back to back reach the
        // store scattered. In place, so this costs no allocation on the one
        // path -- a history-sync seed -- that is ever long enough to reach it.
        if stored > MAX_MSG_SECRETS / 4 {
            entries.sort_unstable_by(|a, b| {
                (a.chat.as_ref(), a.msg_id.as_ref()).cmp(&(b.chat.as_ref(), b.msg_id.as_ref()))
            });
        }
        let mut state = self.state.lock().await;
        // Initial history batches are overwhelmingly new rows, so reserve
        // once. Once populated, a batch may be mostly overwrites; reserving its
        // full length then would grow the table without adding any rows.
        // Clamped to the cap: a seed batch larger than it would otherwise size
        // the table for rows this store is about to evict anyway.
        if state.msg_secrets.is_empty() {
            state.msg_secrets.reserve(stored.min(MAX_MSG_SECRETS));
        }
        // Evict between chunks rather than once at the end. A batch bigger than
        // the cap -- a history-sync seed goes straight to the backend, skipping
        // the write-behind buffer's own high-water mark -- would otherwise be
        // inserted whole, and by the time the eviction ran the table would
        // already have doubled past the bound. `retain` frees rows but not the
        // allocation, and on wasm32 that allocation is never returned, so the
        // footprint bound has to hold going up, not just coming down.
        let mut entries = entries.into_iter().peekable();
        loop {
            let mut inserted = 0usize;
            while let Some(entry) = entries.next() {
                inserted += 1;
                // The chunk boundary must not fall between a message's sender
                // alias rows. Eviction runs as soon as the chunk closes, and it
                // would see the first alias with the second not yet inserted --
                // free to drop the one it can see, after which the other lands
                // and survives alone. That is the identity-dependent decryption
                // failure the grouping in `trim_msg_secrets` exists to prevent,
                // reintroduced one level up. The sort above put a message's rows
                // next to each other, so holding the chunk open while the next
                // entry names the same message is enough.
                let boundary_group = (inserted >= MAX_MSG_SECRETS / 4)
                    .then(|| (Arc::clone(&entry.chat), Arc::clone(&entry.msg_id)));
                let key = MsgSecretKey {
                    chat: entry.chat,
                    sender: entry.sender,
                    msg_id: entry.msg_id,
                };
                match state.msg_secrets.entry(key) {
                    Entry::Occupied(mut occupied) => {
                        let (secret, expires_at, message_ts) = occupied.get_mut();
                        *secret = entry.secret;
                        *expires_at = merge_msg_secret_expiry(*expires_at, entry.expires_at);
                        *message_ts = merge_msg_secret_message_ts(*message_ts, entry.message_ts);
                    }
                    Entry::Vacant(vacant) => {
                        vacant.insert((entry.secret, entry.expires_at, entry.message_ts));
                    }
                }
                if let Some((chat, msg_id)) = boundary_group
                    && !entries
                        .peek()
                        .is_some_and(|next| next.chat == chat && next.msg_id == msg_id)
                {
                    break;
                }
            }
            if inserted == 0 {
                break;
            }
            let state = &mut *state;
            trim_msg_secrets(&mut state.msg_secrets, &mut state.msg_secrets_rescan_at);
        }
        Ok(stored)
    }

    async fn get_msg_secret(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<Vec<u8>>> {
        Ok(self
            .get_msg_secret_with_ts(chat, sender, msg_id)
            .await?
            .map(|(secret, _)| secret))
    }

    async fn get_msg_secret_with_ts(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<(Vec<u8>, i64)>> {
        Ok(self
            .state
            .lock()
            .await
            .msg_secrets
            .get(&MsgSecretKeyRef {
                chat,
                sender,
                msg_id,
            })
            .map(|(secret, _, message_ts)| (secret.to_vec(), *message_ts)))
    }

    async fn delete_expired_msg_secrets(&self, cutoff_timestamp: i64) -> Result<u32> {
        let mut state = self.state.lock().await;
        let before = state.msg_secrets.len();
        // Keep rows with no deadline (0 = never) or a deadline still in the future.
        state
            .msg_secrets
            .retain(|_, (_, expires_at, _)| *expires_at == 0 || *expires_at > cutoff_timestamp);
        Ok((before - state.msg_secrets.len()) as u32)
    }
}

// ---------------------------------------------------------------------------
// DeviceStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl DeviceStore for InMemoryBackend {
    async fn save(&self, device: &Device) -> Result<()> {
        self.state.lock().await.device = Some(device.clone());
        Ok(())
    }

    async fn load(&self) -> Result<Option<Device>> {
        Ok(self.state.lock().await.device.clone())
    }

    async fn exists(&self) -> Result<bool> {
        Ok(self.state.lock().await.device.is_some())
    }

    async fn create(&self) -> Result<i32> {
        let id = self.next_device_id.fetch_add(1, Ordering::Relaxed);
        // Materialize a default Device so that `exists()` returns true after `create()`.
        let mut state = self.state.lock().await;
        if state.device.is_none() {
            state.device = Some(Device::new());
        }
        Ok(id)
    }

    /// Every byte this backend holds is process heap in this process, so unlike
    /// a file- or network-backed store it can report a real figure rather than a
    /// cap. `memory_bytes` sums each map's table allocation plus the heap its
    /// keys and values own; `pages` carries the total row count.
    ///
    /// The `Device` row counts as its flat `size_of` (its key material is
    /// fixed-size arrays) and allocator overhead is excluded, matching
    /// [`HeapSize`](crate::stats::HeapSize) semantics. Both understate.
    async fn resource_report(&self) -> crate::stats::StorageResourceReport {
        let state = self.state.lock().await;
        let mut bytes = 0usize;
        let mut rows = 0u64;

        macro_rules! account {
            ($map:expr, $payload:expr) => {{
                let map = &$map;
                bytes += table_bytes(map);
                rows += map.len() as u64;
                #[allow(clippy::redundant_closure_call)]
                for (k, v) in map.iter() {
                    bytes += $payload(k, v);
                }
            }};
        }

        account!(state.identities, |k: &String, _: &[u8; 32]| k.capacity());
        account!(state.sessions, |k: &String, v: &Bytes| k.capacity()
            + v.len());
        // The batch encoder hands every record a slice of one shared buffer, so
        // summing the slice lengths reproduces that buffer exactly once.
        account!(state.prekeys, |_: &u32, v: &PreKeyEntry| v.record.len());
        account!(state.signed_prekeys, |_: &u32, v: &Vec<u8>| v.capacity());
        account!(state.sender_keys, |k: &String, v: &Vec<u8>| k.capacity()
            + v.capacity());
        account!(state.sync_keys, |k: &Vec<u8>, v: &AppStateSyncKey| k
            .capacity()
            + v.key_data.capacity()
            + v.fingerprint.capacity());
        account!(state.versions, |k: &String, v: &HashState| {
            k.capacity()
                + table_bytes(&v.index_value_map)
                + v.index_value_map
                    .iter()
                    .map(|(ik, iv)| ik.capacity() + iv.capacity())
                    .sum::<usize>()
        });
        account!(state.mutation_macs, |k: &(String, Vec<u8>), v: &Vec<u8>| k
            .0
            .capacity()
            + k.1.capacity()
            + v.capacity());
        account!(
            state.sender_key_devices,
            |k: &String, v: &HashMap<String, bool>| {
                k.capacity() + table_bytes(v) + v.keys().map(|dk| dk.capacity()).sum::<usize>()
            }
        );
        account!(state.lid_mappings, |k: &String, v: &LidPnMappingEntry| {
            k.capacity()
                + v.lid.capacity()
                + v.phone_number.capacity()
                + v.learning_source.capacity()
        });
        account!(state.pn_to_lid, |k: &String, v: &String| k.capacity()
            + v.capacity());
        account!(state.base_keys, |k: &BaseKeyKey, v: &Vec<u8>| k
            .0
            .capacity()
            + k.1.capacity()
            + v.capacity());
        account!(state.device_lists, |k: &String, v: &DeviceListRecord| {
            k.capacity()
                + v.user.capacity()
                + v.devices.capacity() * size_of::<DeviceInfo>()
                + v.phash.as_ref().map_or(0, String::capacity)
        });
        account!(state.group_metadata, |k: &String, v: &Vec<u8>| k.capacity()
            + v.capacity());
        account!(state.tc_tokens, |k: &String, v: &TcTokenEntry| k.capacity()
            + v.token.capacity());
        account!(
            state.sent_messages,
            |k: &SentMessageKey, v: &SentMessageEntry| {
                k.0.capacity() + k.1.capacity() + v.payload.capacity()
            }
        );
        account!(
            state.pending_inbound,
            |k: &(String, String, String), v: &(Vec<u8>, i64)| {
                k.0.capacity() + k.1.capacity() + k.2.capacity() + v.0.capacity()
            }
        );

        // `chat` and `sender` are deliberately one `Arc<str>` shared by every
        // row of a conversation, and `sender` often aliases `chat` outright, so
        // a per-row sum would bill one allocation once per message. Dedup those
        // two by data pointer; the set grows with distinct conversations, not
        // with rows. `msg_id` names one message, so it is counted where found.
        bytes += hb_table_bytes(&state.msg_secrets);
        rows += state.msg_secrets.len() as u64;
        let mut conversations: std::collections::HashSet<*const u8> =
            std::collections::HashSet::new();
        for key in state.msg_secrets.keys() {
            bytes += key.msg_id.len();
            for shared in [&key.chat, &key.sender] {
                if conversations.insert(shared.as_ptr()) {
                    bytes += shared.len();
                }
            }
        }

        if state.device.is_some() {
            bytes += size_of::<Device>();
            rows += 1;
        }
        bytes += state.latest_sync_key_id.as_ref().map_or(0, Vec::capacity);

        crate::stats::StorageResourceReport {
            memory_bytes: Some(bytes as u64),
            pages: Some(rows),
            ..Default::default()
        }
    }
}

/// Drop the soonest-to-expire secrets once the map exceeds
/// [`MAX_MSG_SECRETS`], down to 3/4 of the cap so the scan amortizes across
/// many inserts (same shape as `store_sent_message`'s eviction).
///
/// Ordering by `expires_at` rather than by insertion evicts the row closest to
/// being pruned anyway, which also keeps the longer horizons: a poll/event
/// secret (90 days) outlives a text secret (30 days) of the same age.
///
/// Rows with no deadline are what `MsgSecretPolicy::Full` writes, and its
/// documented contract is unbounded retention, so they are never candidates.
/// A store holding nothing but those still grows without bound -- that is the
/// policy the caller asked for.
///
/// For the same reason the cap is measured against the evictable rows alone,
/// not the map length. Counting the never-expire rows toward it would make a
/// store that holds many of them (a backend reused across a `Full` -> `Managed`
/// switch) evict every finite row it has and still not reach the bound.
///
/// # Where the alias grouping stops
///
/// A message's sender alias rows are kept together at the cutoff, which is
/// where an arbitrary choice would otherwise be made. Rows *below* the cutoff
/// go individually. Those two rows are separate keys, so nothing merges their
/// deadlines, and a write that dated one of them differently -- a later capture
/// under another retention class -- can leave a pair straddling the cutoff and
/// split it.
///
/// That gap is deliberate. Closing it means grouping every evictable row, not
/// just the cutoff's tie bucket, and the index that needs costs about 1.0 MiB
/// of committed linear memory on wasm32 and roughly half again the eviction's
/// CPU (measured, 30k sends: 5.88 -> 6.88 MiB). Linear memory is never returned
/// there, so the fix permanently spends an eighth of what this cap is here to
/// reclaim, against a split that needs one message's aliases to be written at
/// different times under different classes. If that trade ever stops holding --
/// a producer that routinely dates aliases apart -- the group index is the
/// answer, and it belongs in the eviction, not in another guard above it.
fn trim_msg_secrets(map: &mut MsgSecretMap, rescan_at: &mut usize) {
    // Evictable rows are a subset of the map, so this O(1) test is a sound
    // early-out for the O(n) one below and keeps the common insert allocation-
    // free.
    //
    // `rescan_at` is the second guard, and it is what keeps a `Full`-policy
    // store off the O(n) path. There `map.len()` sits above the cap forever
    // while nothing is ever evictable, so the length test alone would scan the
    // whole map on every single write -- O(n) per insert, O(n^2) over a
    // session, all under the state lock.
    if map.len() <= MAX_MSG_SECRETS || map.len() < *rescan_at {
        return;
    }
    // Only the deadlines are collected: cloning every key to sort them would
    // allocate three `Arc<str>` bumps per retained row on each eviction while
    // holding the state lock. `select_nth_unstable` finds the cutoff in O(n)
    // without ordering the rest.
    let mut deadlines: Vec<i64> = map
        .values()
        .filter(|(_, expires_at, _)| *expires_at != 0)
        .map(|(_, expires_at, _)| *expires_at)
        .collect();
    if deadlines.len() <= MAX_MSG_SECRETS {
        // Nothing to evict yet. Every row added between now and the next scan
        // adds at most one evictable row, so the cap cannot be reached before
        // that many more arrive -- exact, not a heuristic.
        *rescan_at = map.len() + (MAX_MSG_SECRETS - deadlines.len()) + 1;
        return;
    }
    *rescan_at = 0;
    let drop_count = deadlines.len() - MAX_MSG_SECRETS * 3 / 4;
    let (_, &mut cutoff, _) = deadlines.select_nth_unstable(drop_count - 1);
    // Two passes apply the cutoff, because map iteration order is arbitrary and
    // a single pass could evict a row AT the cutoff while keeping one below it.
    // `cutoff` is never 0, so the never-expire rows stay out of both passes.
    let mut removed = 0usize;
    map.retain(|_, (_, expires_at, _)| {
        if *expires_at != 0 && *expires_at < cutoff {
            removed += 1;
            false
        } else {
            true
        }
    });
    let mut remaining = drop_count.saturating_sub(removed);
    if remaining == 0 {
        return;
    }
    // The rows sitting exactly on the cutoff. One message can own two of them:
    // history seeding and inbound bot capture persist a secret under two sender
    // aliases (`MAX_HISTORY_SECRET_SENDERS`), and those rows carry the same
    // deadline because it is derived from the same parent event. The pair
    // exists so a lookup succeeds under either identity, so dropping an
    // arbitrary subset of the bucket -- keeping one alias, losing the other --
    // would make decryption depend on which identity the later stanza happens
    // to carry. Choose whole messages instead of whole rows.
    //
    // Each group is counted in full before anything is committed to. Charging
    // the budget per visited row instead would undercount every group whose
    // partner iteration had not reached yet -- and since a message's two rows
    // hash independently, most of them -- so a 2049-row budget could remove
    // close to 4098 rows and leave the store at half the target. That is not a
    // bound being overshot, it is thousands of retainable secrets thrown away.
    let mut bucket: HbHashMap<MsgGroupKey, usize, RandomState> = HbHashMap::default();
    for (key, (_, expires_at, _)) in map.iter() {
        if *expires_at != cutoff {
            continue;
        }
        if let Some(rows) = bucket.get_mut(&MsgGroupKeyRef {
            chat: &key.chat,
            msg_id: &key.msg_id,
        }) {
            *rows += 1;
        } else {
            bucket.insert(
                MsgGroupKey {
                    chat: Arc::clone(&key.chat),
                    msg_id: Arc::clone(&key.msg_id),
                },
                1,
            );
        }
    }
    // Skipping a group that does not fit rather than stopping outright lets a
    // smaller one still use the remainder. Falling a row or two short of the
    // target is fine: the next insert re-enters through the length guard.
    let mut doomed: HbHashSet<MsgGroupKey, RandomState> = HbHashSet::default();
    for (group, rows) in bucket {
        if remaining == 0 {
            break;
        }
        if rows <= remaining {
            remaining -= rows;
            doomed.insert(group);
        }
    }
    map.retain(|key, (_, expires_at, _)| {
        *expires_at != cutoff
            || !doomed.contains(&MsgGroupKeyRef {
                chat: &key.chat,
                msg_id: &key.msg_id,
            })
    });
}

/// Bytes a hash table's single allocation occupies. hashbrown reserves a
/// power-of-two bucket count at 7/8 load and pairs every bucket with a control
/// byte, so counting `capacity()` bare slots understates a full table by about
/// a fifth, which on the prekey map is 8 KiB. `std::collections::HashMap` is a
/// hashbrown table, so both map types take the same shape.
fn slots_bytes<K, V>(capacity: usize) -> usize {
    // A map that has never been inserted into owns no allocation at all, and
    // `next_power_of_two()` rounds 0 up to 1 rather than down to nothing.
    if capacity == 0 {
        return 0;
    }
    let buckets = (capacity + capacity / 7).next_power_of_two();
    buckets * (size_of::<(K, V)>() + 1)
}

fn table_bytes<K, V, S>(map: &HashMap<K, V, S>) -> usize {
    slots_bytes::<K, V>(map.capacity())
}

fn hb_table_bytes<K, V, S>(map: &HbHashMap<K, V, S>) -> usize {
    slots_bytes::<K, V>(map.capacity())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_backend<T: Backend>() {}

    #[test]
    fn in_memory_backend_implements_backend() {
        is_backend::<InMemoryBackend>();
    }

    #[tokio::test]
    async fn put_sessions_batch_inserts_and_updates() {
        let backend = InMemoryBackend::new();
        let first: Arc<str> = "15550000001:1@s.whatsapp.net".into();
        let second: Arc<str> = "15550000002:2@s.whatsapp.net".into();

        backend
            .put_sessions_batch(&[
                (first.clone(), Bytes::from_static(b"first")),
                (second.clone(), Bytes::from_static(b"second")),
            ])
            .await
            .unwrap();
        backend
            .put_sessions_batch(&[(first.clone(), Bytes::from_static(b"updated"))])
            .await
            .unwrap();

        assert_eq!(
            backend.get_session(&first).await.unwrap().unwrap(),
            Bytes::from_static(b"updated")
        );
        assert_eq!(
            backend.get_session(&second).await.unwrap().unwrap(),
            Bytes::from_static(b"second")
        );
    }

    /// A connect-sized batch: every id must be readable back, and a later batch
    /// repeating an id must overwrite it rather than duplicate or drop it. This
    /// is what pins `store_prekeys_batch` idempotent per id across the reserve.
    #[tokio::test]
    async fn store_prekeys_batch_stores_every_key() {
        const COUNT: u32 = 812;
        let backend = InMemoryBackend::new();

        let batch: Vec<(u32, Bytes)> = (1..=COUNT)
            .map(|id| (id, Bytes::from(format!("record-{id}"))))
            .collect();
        backend.store_prekeys_batch(&batch, false).await.unwrap();

        for id in 1..=COUNT {
            assert_eq!(
                backend.load_prekey(id).await.unwrap(),
                Some(Bytes::from(format!("record-{id}"))),
                "prekey {id} must survive the batch write"
            );
        }
        assert_eq!(backend.get_max_prekey_id().await.unwrap(), COUNT);

        backend
            .store_prekeys_batch(&[(7, Bytes::from_static(b"rewritten"))], true)
            .await
            .unwrap();
        assert_eq!(
            backend.load_prekey(7).await.unwrap(),
            Some(Bytes::from_static(b"rewritten"))
        );
        assert_eq!(
            backend.state.lock().await.prekeys.len(),
            COUNT as usize,
            "re-storing an existing id must not add a row"
        );
    }

    /// Reserving for the batch length must leave the table exactly the size the
    /// row count alone demands — the point of the reserve is to reach that size
    /// in one allocation, not to reach a bigger one. The control map is grown
    /// one insert at a time, which is the un-reserved shape; hashbrown sizes a
    /// table from the element count alone, so the two must agree. The second
    /// pass covers the reserve landing on an already-populated map, where
    /// reserving the full batch length on top of the existing rows would be
    /// visible as a doubled table.
    #[tokio::test]
    async fn store_prekeys_batch_reserve_does_not_over_grow_the_table() {
        const COUNT: u32 = 812;
        let backend = InMemoryBackend::new();
        let mut control: HashMap<u32, ()> = HashMap::new();

        for pass in 0..2u32 {
            let first = pass * COUNT + 1;
            let batch: Vec<(u32, Bytes)> = (first..first + COUNT)
                .map(|id| (id, Bytes::from_static(b"record")))
                .collect();
            backend.store_prekeys_batch(&batch, false).await.unwrap();
            for id in first..first + COUNT {
                control.insert(id, ());
            }

            let state = backend.state.lock().await;
            assert_eq!(state.prekeys.len(), control.len());
            assert_eq!(
                state.prekeys.capacity(),
                control.capacity(),
                "pass {pass}: the reserved table must match an incrementally grown one"
            );
        }
    }

    /// Replaying a stored window must not grow the table by one bucket. The
    /// trait permits a batch to overwrite ids, and a table grown for rows that
    /// were only overwritten never shrinks back — so a reservation taken on the
    /// bare batch length would retain an extra table forever, which is exactly
    /// the residency this change claims not to touch.
    #[tokio::test]
    async fn replaying_a_stored_batch_does_not_grow_the_table() {
        const COUNT: u32 = 812;
        let backend = InMemoryBackend::new();
        let batch: Vec<(u32, Bytes)> = (1..=COUNT)
            .map(|id| (id, Bytes::from_static(b"record")))
            .collect();

        backend.store_prekeys_batch(&batch, false).await.unwrap();
        let settled = backend.state.lock().await.prekeys.capacity();

        // Same ids twice more: every row is an overwrite, so nothing is added.
        backend.store_prekeys_batch(&batch, true).await.unwrap();
        backend.store_prekeys_batch(&batch, true).await.unwrap();

        let state = backend.state.lock().await;
        assert_eq!(state.prekeys.len(), COUNT as usize, "no rows were added");
        assert_eq!(
            state.prekeys.capacity(),
            settled,
            "an all-overwrite batch must not enlarge the table"
        );
    }

    /// A batch whose ids repeat stores one row per distinct id, so sizing the
    /// table from the batch length would leave it holding a table for rows that
    /// never existed. The reservation is skipped unless the batch is strictly
    /// ascending, which is what makes its ids provably distinct.
    #[tokio::test]
    async fn a_batch_of_repeated_ids_does_not_reserve_for_them() {
        const COUNT: usize = 812;
        let backend = InMemoryBackend::new();
        let batch: Vec<(u32, Bytes)> = (0..COUNT)
            .map(|i| (7, Bytes::from(format!("record-{i}"))))
            .collect();

        backend.store_prekeys_batch(&batch, false).await.unwrap();

        // One row survives — the last write for id 7 — so the table must be
        // sized for one row, not for the 812 entries that were handed over.
        let mut control: HashMap<u32, ()> = HashMap::new();
        control.insert(7, ());

        let state = backend.state.lock().await;
        assert_eq!(state.prekeys.len(), 1, "last write wins per id");
        assert_eq!(
            state.prekeys.capacity(),
            control.capacity(),
            "a repeated-id batch must not size the table by its length"
        );
        drop(state);
        assert_eq!(
            backend.load_prekey(7).await.unwrap(),
            Some(Bytes::from(format!("record-{}", COUNT - 1)))
        );
    }

    #[tokio::test]
    async fn group_metadata_round_trip() {
        use crate::store::traits::ProtocolStore;
        let backend = InMemoryBackend::new();
        let jid = "120363000000000001@g.us";

        assert!(backend.get_group_metadata(jid).await.unwrap().is_none());
        backend.put_group_metadata(jid, b"blob-v1").await.unwrap();
        assert_eq!(
            backend.get_group_metadata(jid).await.unwrap().as_deref(),
            Some(&b"blob-v1"[..])
        );
        backend.put_group_metadata(jid, b"blob-v2").await.unwrap();
        assert_eq!(
            backend.get_group_metadata(jid).await.unwrap().as_deref(),
            Some(&b"blob-v2"[..])
        );
        // Delete drops the blob so the next query re-fetches in full.
        backend.delete_group_metadata(jid).await.unwrap();
        assert!(backend.get_group_metadata(jid).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn clear_mutation_macs_wipes_only_named_collection() {
        use crate::store::traits::AppSyncStore;
        let backend = InMemoryBackend::new();
        let mac = |i: u8, v: u8| AppStateMutationMAC {
            index_mac: vec![i],
            value_mac: vec![v],
        };
        backend
            .put_mutation_macs("regular", 1, &[mac(1, 10)])
            .await
            .unwrap();
        backend
            .put_mutation_macs("critical", 1, &[mac(2, 20)])
            .await
            .unwrap();

        backend.clear_mutation_macs("regular").await.unwrap();

        assert!(
            backend
                .get_mutation_mac("regular", &[1])
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            backend.get_mutation_mac("critical", &[2]).await.unwrap(),
            Some(vec![20])
        );
    }

    #[tokio::test]
    async fn has_signal_state_for_user_matches_by_user_prefix() {
        let backend = InMemoryBackend::new();
        let user = "5511999990000";

        assert!(!backend.has_signal_state_for_user(user).await.unwrap());

        // Device 0 is keyed `user@server`.
        backend
            .put_session("5511999990000@s.whatsapp.net", b"sess")
            .await
            .unwrap();
        assert!(backend.has_signal_state_for_user(user).await.unwrap());

        // A different user that this one is a prefix of must NOT match.
        let other = InMemoryBackend::new();
        other
            .put_session("55119999900001@s.whatsapp.net", b"sess")
            .await
            .unwrap();
        assert!(!other.has_signal_state_for_user(user).await.unwrap());

        // Non-zero device is keyed `user:dev@server`; identity-only also counts.
        let dev = InMemoryBackend::new();
        dev.put_identity("5511999990000:5@s.whatsapp.net", [7u8; 32])
            .await
            .unwrap();
        assert!(dev.has_signal_state_for_user(user).await.unwrap());
    }

    #[tokio::test]
    async fn store_sent_message_is_memory_bounded() {
        let backend = InMemoryBackend::new();
        for i in 0..(MAX_SENT_MESSAGES + 500) {
            backend
                .store_sent_message("chat@g.us", &format!("m{i}"), b"payload")
                .await
                .unwrap();
        }
        let len = backend.state.lock().await.sent_messages.len();
        assert!(
            len <= MAX_SENT_MESSAGES,
            "sent_messages must stay within the hard cap, got {len}"
        );
        // The most recently stored message is inserted after eviction, so it
        // always survives.
        let last = format!("m{}", MAX_SENT_MESSAGES + 500 - 1);
        assert!(
            backend
                .take_sent_message("chat@g.us", &last)
                .await
                .unwrap()
                .is_some(),
            "the newest message must survive count-cap eviction"
        );
    }

    /// Under a flood every entry lands in the same second, so the eviction
    /// cutoff is a timestamp shared by the whole map. Dropping everything at or
    /// below it would clear the store instead of trimming it, losing the
    /// retry/receipt payloads of messages that were just sent.
    #[tokio::test]
    async fn store_sent_message_eviction_trims_when_all_timestamps_tie() {
        let backend = InMemoryBackend::new();
        for i in 0..MAX_SENT_MESSAGES {
            backend
                .store_sent_message("chat@g.us", &format!("m{i}"), b"payload")
                .await
                .unwrap();
        }
        // Pin the tie instead of relying on the loop finishing inside one
        // second: the clock advancing mid-run would silently exercise the
        // ordinary multi-bucket path rather than the case under test.
        {
            let mut s = backend.state.lock().await;
            for entry in s.sent_messages.values_mut() {
                entry.timestamp = 1_000;
            }
        }

        // The map is at the cap, so this insert is the one that evicts.
        backend
            .store_sent_message("chat@g.us", "trigger", b"payload")
            .await
            .unwrap();

        let target = MAX_SENT_MESSAGES * 3 / 4;
        let s = backend.state.lock().await;
        assert_eq!(
            s.sent_messages.len(),
            target + 1,
            "eviction must trim to 3/4 of the cap plus the insert that triggered it"
        );
        assert!(
            s.sent_messages
                .contains_key(&("chat@g.us".to_string(), "trigger".to_string())),
            "the insert that triggered eviction must survive it"
        );
    }

    /// With distinct timestamps the cutoff bucket must not shield older
    /// entries: arbitrary map iteration order used to decide who went first.
    #[tokio::test]
    async fn store_sent_message_eviction_drops_the_oldest_first() {
        let backend = InMemoryBackend::new();
        for i in 0..MAX_SENT_MESSAGES {
            backend
                .store_sent_message("chat@g.us", &format!("m{i}"), b"payload")
                .await
                .unwrap();
        }
        // Two buckets: a minority strictly older than the rest. Every one of the
        // old bucket has to go before anything from the newer bucket does.
        let old_ids: Vec<String> = (0..16).map(|i| format!("m{i}")).collect();
        {
            let mut s = backend.state.lock().await;
            for (key, entry) in s.sent_messages.iter_mut() {
                entry.timestamp = if old_ids.contains(&key.1) { 500 } else { 1_000 };
            }
        }

        backend
            .store_sent_message("chat@g.us", "trigger", b"payload")
            .await
            .unwrap();

        let s = backend.state.lock().await;
        for id in &old_ids {
            assert!(
                !s.sent_messages
                    .contains_key(&("chat@g.us".to_string(), id.clone())),
                "entry {id} is older than the cutoff and must have been evicted"
            );
        }
    }

    #[tokio::test]
    async fn msg_secret_round_trip() {
        let backend = InMemoryBackend::new();
        let secret = [7u8; 32];
        backend
            .put_msg_secret("12345@s.whatsapp.net", "9999@lid", "MID1", &secret)
            .await
            .unwrap();
        let got = backend
            .get_msg_secret("12345@s.whatsapp.net", "9999@lid", "MID1")
            .await
            .unwrap();
        assert_eq!(got.as_deref(), Some(&secret[..]));
    }

    #[tokio::test]
    async fn msg_secret_miss_returns_none() {
        let backend = InMemoryBackend::new();
        assert!(
            backend
                .get_msg_secret("12345@s.whatsapp.net", "9999@lid", "MID1")
                .await
                .unwrap()
                .is_none(),
            "absent secret must return None"
        );
    }

    #[tokio::test]
    async fn msg_secret_keyed_by_all_three_columns() {
        // Same chat+sender, different msg_id → independent entries.
        // Same chat+msg_id, different sender → independent entries.
        // Same sender+msg_id, different chat → independent entries.
        let backend = InMemoryBackend::new();
        backend
            .put_msg_secret("chatA", "senderX", "M1", &[1u8; 32])
            .await
            .unwrap();
        backend
            .put_msg_secret("chatA", "senderX", "M2", &[2u8; 32])
            .await
            .unwrap();
        backend
            .put_msg_secret("chatA", "senderY", "M1", &[3u8; 32])
            .await
            .unwrap();
        backend
            .put_msg_secret("chatB", "senderX", "M1", &[4u8; 32])
            .await
            .unwrap();

        assert_eq!(
            backend
                .get_msg_secret("chatA", "senderX", "M1")
                .await
                .unwrap()
                .unwrap(),
            vec![1u8; 32]
        );
        assert_eq!(
            backend
                .get_msg_secret("chatA", "senderX", "M2")
                .await
                .unwrap()
                .unwrap(),
            vec![2u8; 32]
        );
        assert_eq!(
            backend
                .get_msg_secret("chatA", "senderY", "M1")
                .await
                .unwrap()
                .unwrap(),
            vec![3u8; 32]
        );
        assert_eq!(
            backend
                .get_msg_secret("chatB", "senderX", "M1")
                .await
                .unwrap()
                .unwrap(),
            vec![4u8; 32]
        );
    }

    #[tokio::test]
    async fn msg_secret_batch_round_trip_and_overwrite() {
        let backend = InMemoryBackend::new();
        let stored = backend
            .put_msg_secrets(vec![
                MsgSecretEntry {
                    chat: "chat".into(),
                    sender: "sender".into(),
                    msg_id: "M1".into(),
                    secret: [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "chat".into(),
                    sender: "sender".into(),
                    msg_id: "M2".into(),
                    secret: [2u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "chat".into(),
                    sender: "sender".into(),
                    msg_id: "M1".into(),
                    secret: [9u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    expires_at: 0,
                    message_ts: 0,
                },
            ])
            .await
            .unwrap();

        assert_eq!(stored, 3);
        assert_eq!(
            backend
                .get_msg_secret("chat", "sender", "M1")
                .await
                .unwrap()
                .unwrap(),
            vec![9u8; 32]
        );
        assert_eq!(
            backend
                .get_msg_secret("chat", "sender", "M2")
                .await
                .unwrap()
                .unwrap(),
            vec![2u8; 32]
        );
    }

    #[tokio::test]
    async fn delete_expired_msg_secrets_removes_only_old_rows() {
        let backend = InMemoryBackend::new();
        backend
            .put_msg_secret("c", "s", "OLD", &[1u8; 32])
            .await
            .unwrap();
        // Set a deadline already in the past to simulate an expired row.
        {
            let mut state = backend.state.lock().await;
            let entry = state
                .msg_secrets
                .get_mut(&MsgSecretKeyRef {
                    chat: "c",
                    sender: "s",
                    msg_id: "OLD",
                })
                .unwrap();
            entry.1 = crate::time::now_secs() - 86_400 * 30;
        }
        // NEW keeps the default `expires_at = 0` (never), so it survives.
        backend
            .put_msg_secret("c", "s", "NEW", &[2u8; 32])
            .await
            .unwrap();

        let cutoff = crate::time::now_secs() - 86_400 * 14;
        let removed = backend.delete_expired_msg_secrets(cutoff).await.unwrap();
        assert_eq!(removed, 1);
        assert!(
            backend
                .get_msg_secret("c", "s", "OLD")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            backend
                .get_msg_secret("c", "s", "NEW")
                .await
                .unwrap()
                .is_some()
        );
    }

    /// The cap is a footprint bound, so what it must guarantee is that a
    /// session sending far more messages than the cap never grows the table
    /// past it -- on wasm32 the doubling realloc commits linear memory that is
    /// never returned.
    #[tokio::test]
    async fn msg_secrets_stay_bounded_and_evict_soonest_deadline_first() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let now = crate::time::now_secs();

        // 4x the cap, each row dated further out than the last.
        let total = MAX_MSG_SECRETS * 4;
        for i in 0..total {
            backend
                .put_msg_secrets(vec![MsgSecretEntry::new(
                    &chat,
                    &chat,
                    &format!("m{i:07}"),
                    [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    now + i as i64,
                    now,
                )])
                .await
                .unwrap();
            assert!(
                backend.state.lock().await.msg_secrets.len() <= MAX_MSG_SECRETS,
                "msg_secrets exceeded the cap at insert {i}"
            );
        }

        // The survivors are the latest deadlines, i.e. the most recent sends.
        let len = backend.state.lock().await.msg_secrets.len();
        assert!(len > MAX_MSG_SECRETS * 3 / 4 - 1 && len <= MAX_MSG_SECRETS);
        assert!(
            backend
                .get_msg_secret(
                    &chat.to_string(),
                    &chat.to_string(),
                    &format!("m{:07}", total - 1)
                )
                .await
                .unwrap()
                .is_some(),
            "newest row must survive"
        );
        assert!(
            backend
                .get_msg_secret(&chat.to_string(), &chat.to_string(), "m0000000")
                .await
                .unwrap()
                .is_none(),
            "oldest deadline must be evicted first"
        );
    }

    /// A history-sync seed reaches `put_msg_secrets` as one oversized batch.
    /// Trimming only after the whole batch landed would bound the row count but
    /// not the table, and on wasm32 that allocation is never returned -- so what
    /// this asserts is the allocation, not the length: one big batch must cost
    /// no more table than the same rows trickling in one at a time.
    #[tokio::test]
    async fn one_oversized_msg_secret_batch_costs_no_more_table_than_trickling() {
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let now = crate::time::now_secs();
        let total = MAX_MSG_SECRETS * 4;
        let row = |i: usize| {
            MsgSecretEntry::new(
                &chat,
                &chat,
                &format!("m{i:07}"),
                [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                now + i as i64,
                now,
            )
        };

        let batched = InMemoryBackend::new();
        batched
            .put_msg_secrets((0..total).map(row).collect())
            .await
            .unwrap();

        let trickled = InMemoryBackend::new();
        for i in 0..total {
            trickled.put_msg_secrets(vec![row(i)]).await.unwrap();
        }

        let batched_capacity = batched.state.lock().await.msg_secrets.capacity();
        let trickled_capacity = trickled.state.lock().await.msg_secrets.capacity();
        assert!(
            batched_capacity <= trickled_capacity,
            "an oversized batch grew the table past the bound: \
             batched {batched_capacity} > trickled {trickled_capacity}"
        );
        assert!(batched.state.lock().await.msg_secrets.len() <= MAX_MSG_SECRETS);
    }

    /// History seeding and inbound bot capture file one secret under two sender
    /// aliases, and both rows carry the same deadline because it comes from the
    /// same parent event. Every row here shares one deadline, so the whole map
    /// is the cutoff's tie bucket -- the case where eviction picks an arbitrary
    /// subset. A surviving half-pair would make decryption depend on which
    /// identity the later stanza carries, so each message must be all or none.
    #[tokio::test]
    async fn msg_secret_eviction_keeps_a_message_s_sender_aliases_together() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let alias_a: wacore_binary::Jid = "2@s.whatsapp.net".parse().unwrap();
        let alias_b: wacore_binary::Jid = "3@lid".parse().unwrap();
        let now = crate::time::now_secs();
        let deadline = now + 30 * 86_400;

        let total = MAX_MSG_SECRETS * 2;
        // The low-water mark right after an eviction, which is what says
        // whether the budget was overcharged. The length at the end of the run
        // is not: it lands anywhere in the oscillation between evictions.
        let mut low_water = usize::MAX;
        let mut evicted_once = false;
        let mut previous_len = 0usize;
        for i in 0..total {
            let id = format!("m{i:07}");
            // Both aliases of one message land in the same batch, exactly as
            // the history seed collector emits them.
            backend
                .put_msg_secrets(
                    [&alias_a, &alias_b]
                        .into_iter()
                        .map(|sender| {
                            MsgSecretEntry::new(
                                &chat,
                                sender,
                                &id,
                                [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                                deadline,
                                now,
                            )
                        })
                        .collect(),
                )
                .await
                .unwrap();
            let len = backend.state.lock().await.msg_secrets.len();
            assert!(
                len <= MAX_MSG_SECRETS,
                "msg_secrets exceeded the cap after message {i}"
            );
            // A drop in length is the only proof trimming actually ran.
            // Reaching the cap is not: a multi-alias batch that stopped being
            // trimmed at all would sail past it and still satisfy every other
            // assertion here.
            if len < previous_len {
                evicted_once = true;
                low_water = low_water.min(len);
            }
            previous_len = len;
        }
        assert!(evicted_once, "eviction never ran");

        let c = chat.to_string();
        let (a, b) = (alias_a.to_string(), alias_b.to_string());
        let mut pairs = 0usize;
        for i in 0..total {
            let id = format!("m{i:07}");
            let got_a = backend.get_msg_secret(&c, &a, &id).await.unwrap().is_some();
            let got_b = backend.get_msg_secret(&c, &b, &id).await.unwrap().is_some();
            assert_eq!(
                got_a, got_b,
                "message {i} kept one sender alias but not the other"
            );
            pairs += usize::from(got_a);
        }
        assert!(pairs > 0, "eviction removed every message");
        // And the budget must be charged per group, not per row visited. Every
        // group here holds two rows, so undercounting them would evict about
        // twice the intended number and leave the store near half the target.
        assert!(
            low_water >= MAX_MSG_SECRETS * 3 / 4,
            "eviction overshot: dropped to {low_water} rows, target is {}",
            MAX_MSG_SECRETS * 3 / 4
        );
    }

    /// The same alias invariant, but across the insertion chunk boundary rather
    /// than the eviction cutoff. One oversized batch is split into fixed-size
    /// chunks with an eviction after each, so a boundary landing between a
    /// message's two rows would let the first be evicted before the second is
    /// even inserted. Every row shares a deadline so eviction has to choose, and
    /// every third message carries a single alias so the pairs fall out of step
    /// with the chunk size instead of aligning to it.
    #[tokio::test]
    async fn msg_secret_chunking_does_not_split_alias_groups() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let alias_a: wacore_binary::Jid = "2@s.whatsapp.net".parse().unwrap();
        let alias_b: wacore_binary::Jid = "3@lid".parse().unwrap();
        let now = crate::time::now_secs();
        let deadline = now + 30 * 86_400;

        let total = MAX_MSG_SECRETS * 3;
        let mut batch = Vec::new();
        for i in 0..total {
            let id = format!("m{i:07}");
            // Message 0 contributes a single row. That one-row offset puts every
            // pair on an odd boundary, so every fixed-size chunk boundary lands
            // between a message's two rows rather than between messages.
            let senders: &[&wacore_binary::Jid] = if i == 0 {
                &[&alias_a]
            } else {
                &[&alias_a, &alias_b]
            };
            for sender in senders {
                batch.push(MsgSecretEntry::new(
                    &chat,
                    sender,
                    &id,
                    [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    // Deadlines fall as the batch goes on, so the row sitting on
                    // a chunk boundary is always among the soonest to expire and
                    // is evicted by the very next trim -- deterministically,
                    // rather than depending on where the cutoff's tie bucket
                    // happens to be walked.
                    deadline - i as i64,
                    now,
                ));
            }
        }
        backend.put_msg_secrets(batch).await.unwrap();

        let c = chat.to_string();
        let (a, b) = (alias_a.to_string(), alias_b.to_string());
        let mut survivors = 0usize;
        for i in 0..total {
            if i == 0 {
                continue;
            }
            let id = format!("m{i:07}");
            let got_a = backend.get_msg_secret(&c, &a, &id).await.unwrap().is_some();
            let got_b = backend.get_msg_secret(&c, &b, &id).await.unwrap().is_some();
            assert_eq!(
                got_a, got_b,
                "message {i} kept one sender alias but not the other"
            );
            survivors += usize::from(got_a);
        }
        assert!(survivors > 0, "eviction removed every paired message");
        assert!(
            backend.state.lock().await.msg_secrets.len() <= MAX_MSG_SECRETS,
            "the oversized batch escaped the cap"
        );
    }

    /// The write-behind buffer snapshots its pending set from a `HashMap`, so
    /// the aliases the inbound capture queued back to back reach the store in
    /// arbitrary order. Worst case of that: every first alias, then every
    /// second. Chunking must still not evict one half of a message before the
    /// other half has been inserted.
    #[tokio::test]
    async fn msg_secret_chunking_groups_aliases_that_arrive_scattered() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let alias_a: wacore_binary::Jid = "2@s.whatsapp.net".parse().unwrap();
        let alias_b: wacore_binary::Jid = "3@lid".parse().unwrap();
        let now = crate::time::now_secs();
        let deadline = now + 30 * 86_400;

        let total = MAX_MSG_SECRETS * 3;
        let row = |i: usize, sender: &wacore_binary::Jid| {
            MsgSecretEntry::new(
                &chat,
                sender,
                &format!("m{i:07}"),
                [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                deadline - i as i64,
                now,
            )
        };
        let mut batch: Vec<_> = (0..total).map(|i| row(i, &alias_a)).collect();
        batch.extend((0..total).map(|i| row(i, &alias_b)));
        backend.put_msg_secrets(batch).await.unwrap();

        let c = chat.to_string();
        let (a, b) = (alias_a.to_string(), alias_b.to_string());
        let mut survivors = 0usize;
        for i in 0..total {
            let id = format!("m{i:07}");
            let got_a = backend.get_msg_secret(&c, &a, &id).await.unwrap().is_some();
            let got_b = backend.get_msg_secret(&c, &b, &id).await.unwrap().is_some();
            assert_eq!(
                got_a, got_b,
                "message {i} kept one sender alias but not the other"
            );
            survivors += usize::from(got_a);
        }
        assert!(survivors > 0, "eviction removed every message");
        assert!(
            backend.state.lock().await.msg_secrets.len() <= MAX_MSG_SECRETS,
            "the scattered batch escaped the cap"
        );
    }

    /// A backend reused across a `Full` -> `Managed` switch holds enough
    /// never-expire rows to exceed the cap on their own. Measuring the cap
    /// against the map length there would evict every finite row and still not
    /// reach the bound, so the managed secrets have to survive.
    #[tokio::test]
    async fn msg_secrets_cap_does_not_wipe_finite_rows_behind_permanent_ones() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let c = chat.to_string();
        let now = crate::time::now_secs();
        let put = async |id: String, expires_at: i64| {
            backend
                .put_msg_secrets(vec![MsgSecretEntry::new(
                    &chat,
                    &chat,
                    &id,
                    [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    expires_at,
                    now,
                )])
                .await
                .unwrap();
        };

        // Permanent rows alone already exceed the cap.
        for i in 0..MAX_MSG_SECRETS + 1000 {
            put(format!("perm{i:07}"), 0).await;
        }
        for i in 0..100 {
            put(format!("fin{i:07}"), now + 1 + i as i64).await;
        }

        for i in 0..100 {
            assert!(
                backend
                    .get_msg_secret(&c, &c, &format!("fin{i:07}"))
                    .await
                    .unwrap()
                    .is_some(),
                "finite row {i} was evicted to make room for un-evictable rows"
            );
        }
    }

    /// `MsgSecretPolicy::Full` writes `expires_at = 0` and promises unbounded
    /// retention, so the cap must not touch those rows.
    #[tokio::test]
    async fn msg_secrets_cap_never_evicts_never_expire_rows() {
        let backend = InMemoryBackend::new();
        let chat: wacore_binary::Jid = "1@s.whatsapp.net".parse().unwrap();
        let c = chat.to_string();

        let total = MAX_MSG_SECRETS * 2;
        for i in 0..total {
            backend
                .put_msg_secrets(vec![MsgSecretEntry::new(
                    &chat,
                    &chat,
                    &format!("m{i:07}"),
                    [1u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    0,
                    0,
                )])
                .await
                .unwrap();
        }

        assert_eq!(backend.state.lock().await.msg_secrets.len(), total);
        assert!(
            backend
                .get_msg_secret(&c, &c, "m0000000")
                .await
                .unwrap()
                .is_some(),
            "a never-expire row is not an eviction candidate"
        );
    }

    #[tokio::test]
    async fn msg_secret_overwrite_on_same_key() {
        let backend = InMemoryBackend::new();
        backend
            .put_msg_secret("chat", "sender", "M", &[1u8; 32])
            .await
            .unwrap();
        backend
            .put_msg_secret("chat", "sender", "M", &[9u8; 32])
            .await
            .unwrap();
        assert_eq!(
            backend
                .get_msg_secret("chat", "sender", "M")
                .await
                .unwrap()
                .unwrap(),
            vec![9u8; 32],
            "last write wins for the same composite key"
        );
    }

    #[tokio::test]
    async fn touch_tc_token_creates_placeholder_then_preserves_real_token() {
        let backend = InMemoryBackend::new();

        backend
            .touch_tc_token_sender_timestamp("u1", 1000)
            .await
            .unwrap();
        let placeholder = backend.get_tc_token("u1").await.unwrap().unwrap();
        assert!(placeholder.token.is_empty());
        assert_eq!(placeholder.sender_timestamp, Some(1000));

        // A real token stored by the notification path must survive a later touch.
        backend
            .put_tc_token(
                "u1",
                &TcTokenEntry {
                    token: vec![7, 8, 9],
                    token_timestamp: 2000,
                    sender_timestamp: None,
                },
            )
            .await
            .unwrap();
        backend
            .touch_tc_token_sender_timestamp("u1", 3000)
            .await
            .unwrap();

        let merged = backend.get_tc_token("u1").await.unwrap().unwrap();
        assert_eq!(
            merged.token,
            vec![7, 8, 9],
            "touch must not clobber the real token"
        );
        assert_eq!(merged.token_timestamp, 2000);
        assert_eq!(merged.sender_timestamp, Some(3000));
    }

    #[tokio::test]
    async fn touch_sender_timestamp_only_advances() {
        let backend = InMemoryBackend::new();
        backend
            .touch_tc_token_sender_timestamp("uadv", 5000)
            .await
            .unwrap();
        // An older touch (e.g. a stale history-sync sender epoch) must not regress.
        backend
            .touch_tc_token_sender_timestamp("uadv", 3000)
            .await
            .unwrap();
        assert_eq!(
            backend
                .get_tc_token("uadv")
                .await
                .unwrap()
                .unwrap()
                .sender_timestamp,
            Some(5000)
        );
    }

    #[tokio::test]
    async fn store_received_tc_token_preserves_sender_timestamp() {
        let backend = InMemoryBackend::new();
        // Placeholder from the issuance path.
        backend
            .touch_tc_token_sender_timestamp("u2", 5000)
            .await
            .unwrap();

        // Notification stores the real token; the sender bucket must survive.
        backend
            .store_received_tc_token("u2", &[1, 2, 3], 4000)
            .await
            .unwrap();

        let entry = backend.get_tc_token("u2").await.unwrap().unwrap();
        assert_eq!(entry.token, vec![1, 2, 3]);
        assert_eq!(entry.token_timestamp, 4000);
        assert_eq!(
            entry.sender_timestamp,
            Some(5000),
            "store_received_tc_token must not drop the sender bucket"
        );

        // No prior entry: sender_timestamp starts unset.
        backend
            .store_received_tc_token("u3", &[9], 4000)
            .await
            .unwrap();
        let fresh = backend.get_tc_token("u3").await.unwrap().unwrap();
        assert_eq!(fresh.sender_timestamp, None);
    }

    #[tokio::test]
    async fn store_received_tc_token_is_newer_wins() {
        let backend = InMemoryBackend::new();

        // First real token at t=5000.
        backend
            .store_received_tc_token("c", &[1, 1, 1], 5000)
            .await
            .unwrap();

        // A stale write (older timestamp) must not clobber the fresher token —
        // this is what lets concurrent history-sync chunks converge lock-free.
        backend
            .store_received_tc_token("c", &[2, 2, 2], 3000)
            .await
            .unwrap();
        let e = backend.get_tc_token("c").await.unwrap().unwrap();
        assert_eq!(e.token, vec![1, 1, 1], "older write must not overwrite");
        assert_eq!(e.token_timestamp, 5000);

        // A newer write wins.
        backend
            .store_received_tc_token("c", &[3, 3, 3], 7000)
            .await
            .unwrap();
        let e = backend.get_tc_token("c").await.unwrap().unwrap();
        assert_eq!(e.token, vec![3, 3, 3]);
        assert_eq!(e.token_timestamp, 7000);

        // A byte-less placeholder (sender epoch t=9000) never blocks a real token,
        // even when the real token's timestamp is older than the placeholder's.
        backend
            .touch_tc_token_sender_timestamp("p", 9000)
            .await
            .unwrap();
        backend
            .store_received_tc_token("p", &[4, 4, 4], 6000)
            .await
            .unwrap();
        let e = backend.get_tc_token("p").await.unwrap().unwrap();
        assert_eq!(
            e.token,
            vec![4, 4, 4],
            "placeholder must accept first real token"
        );
        assert_eq!(e.token_timestamp, 6000);
        assert_eq!(e.sender_timestamp, Some(9000), "sender bucket preserved");
    }

    #[tokio::test]
    async fn prune_respects_sender_and_token_windows() {
        let backend = InMemoryBackend::new();
        // token_cutoff = 1000, sender_cutoff = 2000 (wider sender window).

        // Recent placeholder: sender bucket still live → kept.
        backend
            .touch_tc_token_sender_timestamp("recent_ph", 2500)
            .await
            .unwrap();
        // Stale placeholder: both windows passed → pruned.
        backend
            .touch_tc_token_sender_timestamp("stale_ph", 100)
            .await
            .unwrap();
        // Expired token but recent sender bucket → kept (issuance state survives).
        backend
            .put_tc_token(
                "expired_tok_live_sender",
                &TcTokenEntry {
                    token: vec![1],
                    token_timestamp: 1,
                    sender_timestamp: Some(2500),
                },
            )
            .await
            .unwrap();
        // Expired token, no sender state → pruned.
        backend
            .put_tc_token(
                "orphan_expired",
                &TcTokenEntry {
                    token: vec![2],
                    token_timestamp: 1,
                    sender_timestamp: None,
                },
            )
            .await
            .unwrap();
        // Fresh received token → kept.
        backend
            .put_tc_token(
                "fresh_tok",
                &TcTokenEntry {
                    token: vec![3],
                    token_timestamp: 5000,
                    sender_timestamp: None,
                },
            )
            .await
            .unwrap();

        let removed = backend.delete_expired_tc_tokens(1000, 2000).await.unwrap();
        assert_eq!(removed, 2, "only fully-stale rows are pruned");
        assert!(backend.get_tc_token("recent_ph").await.unwrap().is_some());
        assert!(backend.get_tc_token("stale_ph").await.unwrap().is_none());
        assert!(
            backend
                .get_tc_token("expired_tok_live_sender")
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            backend
                .get_tc_token("orphan_expired")
                .await
                .unwrap()
                .is_none()
        );
        assert!(backend.get_tc_token("fresh_tok").await.unwrap().is_some());
    }

    /// An empty backend holds no rows, and says so with a positive `Some(0)`:
    /// it *can* introspect and the answer is genuinely zero.
    #[tokio::test]
    async fn resource_report_of_an_empty_backend_is_a_positive_zero() {
        let report = InMemoryBackend::new().resource_report().await;
        assert_eq!(report.memory_bytes, Some(0));
        assert_eq!(report.pages, Some(0));
        // Neither direction of I/O is counted, so both stay unreported.
        assert_eq!(report.io_read_bytes, None);
        assert_eq!(report.io_write_bytes, None);
    }

    /// The reported figure is checked against an independent source: the exact
    /// payload bytes handed to the backend. A prekey batch slices one shared
    /// buffer, so the payload total is `count * record_len` and the reported
    /// total must cover it without wandering far above the table overhead that
    /// carries it (measured: 812 records report ~1% over live heap).
    #[tokio::test]
    async fn resource_report_tracks_the_bytes_actually_stored() {
        const COUNT: usize = 812;
        const RECORD_LEN: usize = 74;

        let backend = InMemoryBackend::new();
        let empty = backend.resource_report().await.memory_bytes.unwrap();

        let shared = Bytes::from(vec![7u8; COUNT * RECORD_LEN]);
        let batch: Vec<(u32, Bytes)> = (0..COUNT)
            .map(|i| {
                (
                    i as u32 + 1,
                    shared.slice(i * RECORD_LEN..(i + 1) * RECORD_LEN),
                )
            })
            .collect();
        backend.store_prekeys_batch(&batch, false).await.unwrap();

        let report = backend.resource_report().await;
        assert_eq!(report.pages, Some(COUNT as u64), "every row is counted");

        let payload = (COUNT * RECORD_LEN) as u64;
        let growth = report.memory_bytes.unwrap() - empty;
        assert!(
            growth > payload,
            "reported {growth} must exceed the {payload} payload bytes it stores"
        );
        assert!(
            growth < payload * 2,
            "reported {growth} is implausibly far above the {payload} payload bytes"
        );
    }

    /// Rows of one conversation share a single `chat`/`sender` allocation, so
    /// the report must bill it once however many messages reference it. Two
    /// backends differing only in whether that `Arc` is shared isolate the
    /// dedup from table growth, which is identical on both sides.
    #[tokio::test]
    async fn shared_msg_secret_strings_are_counted_once() {
        const ROWS: usize = 40;
        const JID: &str = "12025550111@s.whatsapp.net";

        async fn report_for(chat_for: impl Fn(usize) -> Arc<str>) -> u64 {
            let backend = InMemoryBackend::new();
            let rows = (0..ROWS)
                .map(|i| MsgSecretEntry {
                    chat: chat_for(i),
                    sender: chat_for(i),
                    msg_id: format!("m{i:04}").into(),
                    secret: [9u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
                    expires_at: 0,
                    message_ts: 0,
                })
                .collect();
            backend.put_msg_secrets(rows).await.unwrap();
            backend.resource_report().await.memory_bytes.unwrap()
        }

        let one_arc: Arc<str> = JID.into();
        let shared = report_for(|_| one_arc.clone()).await;
        // Same bytes, same row count, but one allocation per row.
        let distinct = report_for(|_| Arc::from(JID)).await;

        assert!(
            distinct > shared,
            "distinct allocations ({distinct}) must cost more than one shared one ({shared})"
        );
        // Sharing saves every copy but the one the report still counts.
        let saved = (distinct - shared) as usize;
        assert_eq!(saved, (2 * ROWS - 1) * JID.len());
    }

    /// The device row is a deliberate lower bound: its flat `size_of` only, with
    /// its owned heap left unwalked. Pinned so the choice stays a decision.
    #[tokio::test]
    async fn the_device_row_counts_as_its_flat_size() {
        let backend = InMemoryBackend::new();
        let empty = backend.resource_report().await;
        backend.create().await.unwrap();
        let stored = backend.resource_report().await;

        assert_eq!(stored.pages, Some(empty.pages.unwrap() + 1));
        assert_eq!(
            stored.memory_bytes.unwrap() - empty.memory_bytes.unwrap(),
            size_of::<Device>() as u64,
        );
    }

    /// The contract the default guards: a store that cannot introspect itself
    /// reports `None`, never a fabricated `Some(0)`.
    #[tokio::test]
    async fn a_store_that_cannot_introspect_reports_none() {
        struct OpaqueStore;

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl DeviceStore for OpaqueStore {
            async fn save(&self, _device: &Device) -> Result<()> {
                Ok(())
            }
            async fn load(&self) -> Result<Option<Device>> {
                Ok(None)
            }
            async fn exists(&self) -> Result<bool> {
                Ok(false)
            }
            async fn create(&self) -> Result<i32> {
                Ok(1)
            }
        }

        let report = OpaqueStore.resource_report().await;
        assert_eq!(report.memory_bytes, None);
        assert_eq!(report.pages, None);
        assert_eq!(report.total_bytes(), 0, "an absent figure totals as zero");
    }
}
