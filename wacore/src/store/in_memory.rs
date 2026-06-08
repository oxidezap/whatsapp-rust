//! In-memory implementation of the [`Backend`] trait.
//!
//! Intended for testing and as a reference implementation for FFI bridges.
//! All data lives in RAM behind a single [`async_lock::Mutex`] and is lost
//! when the struct is dropped.

use std::collections::HashMap;
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
type MsgSecretRow = (Vec<u8>, i64, i64);

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

    // --- MsgSecret ---
    /// `expires_at = 0` means never expire; `message_ts = 0` means the parent
    /// event time is unknown. The keepalive cleanup prunes expired rows.
    msg_secrets: HashMap<(String, String, String), MsgSecretRow>,

    // --- Device ---
    device: Option<Device>,
}

/// Hard cap on retained sent messages, bounding memory regardless of the
/// configured retention window. Time-based pruning is the client's keepalive
/// sweep (`delete_expired_sent_messages`, driven by
/// `CacheConfig::sent_message_ttl_secs`, the single source of truth for the
/// time window); this cap only guards against a burst between sweeps.
const MAX_SENT_MESSAGES: usize = 4096;

/// In-memory implementation of the full [`Backend`] trait.
///
/// Thread-safe and runtime-agnostic (uses [`async_lock::Mutex`]).
/// All data is ephemeral — it lives only as long as this struct.
pub struct InMemoryBackend {
    state: Mutex<InMemoryState>,
    next_device_id: AtomicI32,
}

impl InMemoryBackend {
    /// Create a new, empty in-memory store.
    pub fn new() -> Self {
        Self {
            state: Mutex::new(InMemoryState::default()),
            next_device_id: AtomicI32::new(1),
        }
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

    async fn store_prekeys_batch(&self, keys: &[(u32, Bytes)], _uploaded: bool) -> Result<()> {
        let mut state = self.state.lock().await;
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
        self.state
            .lock()
            .await
            .sender_keys
            .insert(address.to_string(), record.to_vec());
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
        let targets: std::collections::HashSet<&str> = device_jids.iter().copied().collect();
        for group_map in state.sender_key_devices.values_mut() {
            group_map.retain(|jid, _| !targets.contains(jid.as_str()));
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

    async fn delete_expired_tc_tokens(&self, cutoff_timestamp: i64) -> Result<u32> {
        let mut s = self.state.lock().await;
        let before = s.tc_tokens.len();
        s.tc_tokens
            .retain(|_, entry| entry.token_timestamp >= cutoff_timestamp);
        Ok((before - s.tc_tokens.len()) as u32)
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
        // (by timestamp) down to 3/4 of it so this O(n log n) scan amortizes
        // across many inserts. Time-based pruning is the caller's keepalive sweep.
        if s.sent_messages.len() >= MAX_SENT_MESSAGES {
            let target = MAX_SENT_MESSAGES * 3 / 4;
            let drop_count = s.sent_messages.len().saturating_sub(target);
            let mut by_age: Vec<_> = s
                .sent_messages
                .iter()
                .map(|(k, e)| (e.timestamp, k.clone()))
                .collect();
            by_age.sort_unstable_by_key(|(ts, _)| *ts);
            for (_, k) in by_age.into_iter().take(drop_count) {
                s.sent_messages.remove(&k);
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
}

// ---------------------------------------------------------------------------
// MsgSecretStore
// ---------------------------------------------------------------------------

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MsgSecretStore for InMemoryBackend {
    async fn put_msg_secrets(&self, entries: Vec<MsgSecretEntry>) -> Result<usize> {
        use crate::store::traits::{merge_msg_secret_expiry, merge_msg_secret_message_ts};
        let stored = entries.len();
        let mut state = self.state.lock().await;
        for entry in entries {
            let key = (entry.chat, entry.sender, entry.msg_id);
            let (expires_at, message_ts) = match state.msg_secrets.get(&key) {
                Some((_, existing_exp, existing_ts)) => (
                    merge_msg_secret_expiry(*existing_exp, entry.expires_at),
                    merge_msg_secret_message_ts(*existing_ts, entry.message_ts),
                ),
                None => (entry.expires_at, entry.message_ts),
            };
            state
                .msg_secrets
                .insert(key, (entry.secret, expires_at, message_ts));
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
            .get(&(chat.to_string(), sender.to_string(), msg_id.to_string()))
            .map(|(secret, _, message_ts)| (secret.clone(), *message_ts)))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_backend<T: crate::store::traits::Backend>() {}

    #[test]
    fn in_memory_backend_implements_backend() {
        is_backend::<InMemoryBackend>();
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
                    secret: vec![1u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "chat".into(),
                    sender: "sender".into(),
                    msg_id: "M2".into(),
                    secret: vec![2u8; 32],
                    expires_at: 0,
                    message_ts: 0,
                },
                MsgSecretEntry {
                    chat: "chat".into(),
                    sender: "sender".into(),
                    msg_id: "M1".into(),
                    secret: vec![9u8; 32],
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
                .get_mut(&("c".into(), "s".into(), "OLD".into()))
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
}
