//! Storage traits for the WhatsApp client.
//!
//! This module defines 4 domain-grouped traits that together form the `Backend` trait:
//!
//! - [`SignalStore`]: Signal protocol cryptographic operations (identity, sessions, keys)
//! - [`AppSyncStore`]: WhatsApp app state synchronization
//! - [`ProtocolStore`]: WhatsApp Web protocol alignment (SKDM, LID mapping, device registry)
//! - [`DeviceStore`]: Device persistence operations

use crate::appstate::hash::HashState;
use crate::store::error::Result;
use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wacore_appstate::processor::AppStateMutationMAC;

/// Heap-backed, protocol-sized message secret. The thin `Box` keeps
/// [`MsgSecretEntry`] compact inside large batches while making invalid secret
/// lengths unrepresentable.
pub type MessageSecret = Box<[u8; crate::reporting_token::MESSAGE_SECRET_SIZE]>;

/// App state synchronization key for WhatsApp's app state protocol.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppStateSyncKey {
    pub key_data: Vec<u8>,
    pub fingerprint: Vec<u8>,
    pub timestamp: i64,
}

/// Entry representing a LID to Phone Number mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LidPnMappingEntry {
    /// The LID user part (e.g., "100000012345678")
    pub lid: String,
    /// The phone number user part (e.g., "559980000001")
    pub phone_number: String,
    /// Unix timestamp when the mapping was first learned
    pub created_at: i64,
    /// Unix timestamp when the mapping was last updated
    pub updated_at: i64,
    /// The source from which this mapping was learned (e.g., "usync", "peer_pn_message")
    pub learning_source: String,
}

/// Trusted contact privacy token entry.
///
/// Matches WhatsApp Web's Chat.tcToken / tcTokenTimestamp / tcTokenSenderTimestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcTokenEntry {
    /// Raw token bytes received from the server.
    pub token: Vec<u8>,
    /// Unix timestamp (seconds) when the token was received.
    pub token_timestamp: i64,
    /// Unix timestamp (seconds) when we last issued our token to this contact.
    pub sender_timestamp: Option<i64>,
}

/// Message-secret write entry keyed by chat, sender, and message ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MsgSecretEntry {
    /// Canonical non-AD chat JID. Shared across entries from the same history
    /// conversation instead of allocating one identical string per message.
    pub chat: Arc<str>,
    /// Canonical non-AD sender JID. Often aliases `chat` for direct messages.
    pub sender: Arc<str>,
    /// Message identifier. `Arc<str>` keeps entry clones used by buffered
    /// persistence cheap without changing the serialized representation.
    pub msg_id: Arc<str>,
    pub secret: MessageSecret,
    /// Absolute unix-seconds retention deadline. `0` means never expire.
    /// Computed by the caller from the parent message's event time plus a
    /// per-add-on-kind horizon (see `MsgSecretRetention`). The store prunes
    /// rows whose deadline has passed; it does not know the horizon itself.
    #[serde(default)]
    pub expires_at: i64,
    /// Parent message event time (unix seconds), or `0` when unknown. Kept so
    /// the receive path can enforce the edit-processing window
    /// (`editTs < message_ts + window`) the same way WhatsApp Web does.
    #[serde(default)]
    pub message_ts: i64,
}

/// Device information for registry tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// The device ID (0 = primary device, 1+ = companion devices)
    pub device_id: u32,
    /// The key index, if known
    pub key_index: Option<u32>,
}

/// Device list record matching WhatsApp Web's DeviceListRecord structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceListRecord {
    /// The user part of the JID (phone number or LID)
    pub user: String,
    /// List of known devices for this user
    pub devices: Vec<DeviceInfo>,
    /// Timestamp when this record was last updated
    pub timestamp: i64,
    /// Participant hash from usync, if available
    pub phash: Option<String>,
    /// ADV raw_id from `ADVKeyIndexList` — used to detect identity changes.
    /// When this changes, all sessions and sender keys for the user must be cleared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_id: Option<u32>,
}

impl crate::stats::HeapSize for DeviceListRecord {
    fn heap_bytes(&self) -> usize {
        self.user.capacity()
            + self.devices.capacity() * size_of::<DeviceInfo>()
            + self.phash.as_ref().map_or(0, |p| p.capacity())
    }
}

/// Signal protocol cryptographic storage operations.
///
/// Handles identity keys, sessions, pre-keys, signed pre-keys, and sender keys
/// for end-to-end encryption.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignalStore: Send + Sync {
    // --- Identity Operations ---

    /// Store an identity key for a remote address.
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()>;

    /// Store multiple identity keys in a single batch operation.
    /// Default implementation falls back to individual `put_identity` calls.
    /// Addresses are `Arc<str>` so callers (the flush path) pass shared keys
    /// without allocating a `String` per entry.
    async fn put_identities_batch(
        &self,
        identities: &[(std::sync::Arc<str>, [u8; 32])],
    ) -> Result<()> {
        for (address, key) in identities {
            self.put_identity(address, *key).await?;
        }
        Ok(())
    }

    /// Load an identity key for a remote address (always 32 bytes).
    async fn load_identity(&self, address: &str) -> Result<Option<[u8; 32]>>;

    /// Delete an identity key.
    async fn delete_identity(&self, address: &str) -> Result<()>;

    // --- Session Operations ---

    /// Get an encrypted session for an address.
    async fn get_session(&self, address: &str) -> Result<Option<Bytes>>;

    /// Store an encrypted session.
    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()>;

    /// Store multiple encrypted sessions in a single batch operation.
    /// Default implementation falls back to individual `put_session` calls.
    async fn put_sessions_batch(&self, sessions: &[(std::sync::Arc<str>, Bytes)]) -> Result<()> {
        for (address, session) in sessions {
            self.put_session(address, session).await?;
        }
        Ok(())
    }

    /// Delete a session.
    async fn delete_session(&self, address: &str) -> Result<()>;

    /// Check if a session exists. Default implementation uses `get_session`.
    async fn has_session(&self, address: &str) -> Result<bool> {
        Ok(self.get_session(address).await?.is_some())
    }

    /// Whether any session or identity exists for `user` across all device ids.
    /// Addresses are keyed `user@server` (device 0) or `user:dev@server`. Used
    /// to skip the per-device PN->LID migration scan for users we've never had
    /// Signal state with. Default is conservative (`true`) so a backend that
    /// doesn't implement it keeps the caller's full per-device scan.
    async fn has_signal_state_for_user(&self, user: &str) -> Result<bool> {
        let _ = user;
        Ok(true)
    }

    // --- PreKey Operations ---

    /// Store a pre-key.
    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> Result<()>;

    /// Store multiple pre-keys in a single batch operation.
    /// Default implementation falls back to individual `store_prekey` calls.
    async fn store_prekeys_batch(&self, keys: &[(u32, Bytes)], uploaded: bool) -> Result<()> {
        for (id, record) in keys {
            self.store_prekey(*id, record, uploaded).await?;
        }
        Ok(())
    }

    /// Load a pre-key by ID.
    async fn load_prekey(&self, id: u32) -> Result<Option<Bytes>>;

    /// Load multiple pre-keys by ID in a single batch operation.
    /// Returns only the keys that exist.
    async fn load_prekeys_batch(&self, ids: &[u32]) -> Result<Vec<(u32, Bytes)>> {
        let mut result = Vec::with_capacity(ids.len());
        for &id in ids {
            if let Some(record) = self.load_prekey(id).await? {
                result.push((id, record));
            }
        }
        Ok(result)
    }

    /// Mark already-stored pre-keys as uploaded WITHOUT inserting. UPDATE
    /// semantics on purpose: a key consumed (deleted) between the upload
    /// snapshot and this call must stay deleted, never be resurrected by an
    /// upsert.
    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<()>;

    /// Remove a pre-key.
    async fn remove_prekey(&self, id: u32) -> Result<()>;

    /// Get the maximum pre-key ID currently stored, or 0 if none exist.
    /// Used for migration when `next_pre_key_id` counter is not yet initialized.
    async fn get_max_prekey_id(&self) -> Result<u32>;

    // --- Signed PreKey Operations ---

    /// Store a signed pre-key.
    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()>;

    /// Load a signed pre-key by ID.
    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>>;

    /// Load all signed pre-keys. Returns (id, record) pairs.
    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>>;

    /// Remove a signed pre-key.
    async fn remove_signed_prekey(&self, id: u32) -> Result<()>;

    // --- Sender Key Operations ---

    /// Store a sender key for group messaging.
    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()>;

    /// Store multiple sender keys in a single batch operation.
    /// Default implementation falls back to individual `put_sender_key` calls.
    async fn put_sender_keys_batch(
        &self,
        sender_keys: &[(std::sync::Arc<str>, Bytes)],
    ) -> Result<()> {
        for (address, record) in sender_keys {
            self.put_sender_key(address, record).await?;
        }
        Ok(())
    }

    /// Get a sender key.
    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>>;

    /// Delete a sender key.
    async fn delete_sender_key(&self, address: &str) -> Result<()>;
}

/// WhatsApp app state synchronization storage.
///
/// Handles sync keys, version tracking, and mutation MACs for the app state protocol.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait AppSyncStore: Send + Sync {
    /// Get an app state sync key by ID.
    async fn get_sync_key(&self, key_id: &[u8]) -> Result<Option<AppStateSyncKey>>;

    /// Set an app state sync key.
    async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> Result<()>;

    /// Get the app state version for a collection.
    async fn get_version(&self, name: &str) -> Result<HashState>;

    /// Set the app state version for a collection.
    async fn set_version(&self, name: &str, state: HashState) -> Result<()>;

    /// Store mutation MACs for a version.
    async fn put_mutation_macs(
        &self,
        name: &str,
        version: u64,
        mutations: &[AppStateMutationMAC],
    ) -> Result<()>;

    /// Get a mutation MAC by index.
    async fn get_mutation_mac(&self, name: &str, index_mac: &[u8]) -> Result<Option<Vec<u8>>>;

    /// Batch variant of [`get_mutation_mac`]: fetch many previous-MAC values in a
    /// single backend round-trip. The default delegates to per-item lookups;
    /// backends with a set-membership query (SQL `IN (...)`) should override to
    /// avoid an N+1 (one DB round-trip per mutation in appstate sync).
    ///
    /// Index MACs are full HMAC-SHA256 outputs, so the batch path passes them as
    /// inline `[u8; 32]` arrays ([`crate::appstate_sync::IndexMac`]) — no per-MAC
    /// heap allocation on either side of the call.
    async fn get_mutation_macs(
        &self,
        name: &str,
        index_macs: &[[u8; 32]],
    ) -> Result<std::collections::HashMap<[u8; 32], Vec<u8>>> {
        let mut out = std::collections::HashMap::with_capacity(index_macs.len());
        for index_mac in index_macs {
            if let Some(mac) = self.get_mutation_mac(name, index_mac.as_slice()).await? {
                out.insert(*index_mac, mac);
            }
        }
        Ok(out)
    }

    /// Delete mutation MACs by their index MACs.
    async fn delete_mutation_macs(&self, name: &str, index_macs: &[Vec<u8>]) -> Result<()>;

    /// Delete every mutation MAC for a collection. Called on snapshot re-sync so the
    /// MAC store is rebuilt from the snapshot, matching the ltHash baseline; leftover
    /// entries would corrupt the next patch's ltHash.
    async fn clear_mutation_macs(&self, name: &str) -> Result<()>;

    /// Get the most recently stored app state sync key ID.
    async fn get_latest_sync_key_id(&self) -> Result<Option<Vec<u8>>>;
}

/// Error returned by the default pending-inbound methods so a backend that does
/// not implement the durability buffer fails closed (no silent at-most-once).
fn unsupported_pending_inbound() -> crate::store::error::StoreError {
    crate::store::error::StoreError::Validation(
        "backend does not support the pending inbound buffer required by the durability hook"
            .to_string(),
    )
}

/// WhatsApp Web protocol alignment storage.
///
/// Handles SKDM tracking, LID-PN mapping, base key collision detection,
/// device registry, and sender key status.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait ProtocolStore: Send + Sync {
    // --- Per-Device Sender Key Tracking (matches WA Web's participant.senderKey Map) ---

    /// Get the sender key distribution status for all known devices in a group.
    /// Returns `(device_jid_string, has_key)` pairs where `has_key` indicates
    /// whether the device has a valid sender key (`true`) or needs fresh SKDM (`false`).
    async fn get_sender_key_devices(&self, group_jid: &str) -> Result<Vec<(String, bool)>>;

    /// Set sender key status for devices. Called with `has_key=true` after successful
    /// SKDM distribution (WA Web: `markHasSenderKey`), or `has_key=false` to mark
    /// devices as needing fresh SKDM (WA Web: `markForgetSenderKey`).
    async fn set_sender_key_status(&self, group_jid: &str, entries: &[(&str, bool)]) -> Result<()>;

    /// Clear all sender key device tracking for a group (on sender key rotation).
    async fn clear_sender_key_devices(&self, group_jid: &str) -> Result<()>;

    /// Delete specific `sender_key_devices` rows by device JID across all groups.
    /// Mirrors WA Web's per-group `senderKey.delete(deviceJid)` cleanup.
    async fn delete_sender_key_device_rows(&self, device_jids: &[&str]) -> Result<()>;

    /// Clear all sender key device tracking across ALL groups.
    /// Called on identity change (raw_id mismatch) to force SKDM redistribution.
    async fn clear_all_sender_key_devices(&self) -> Result<()>;

    // --- LID-PN Mapping ---

    /// Get a mapping by LID.
    async fn get_lid_mapping(&self, lid: &str) -> Result<Option<LidPnMappingEntry>>;

    /// Get a mapping by phone number (returns the most recent LID for that phone).
    async fn get_pn_mapping(&self, phone: &str) -> Result<Option<LidPnMappingEntry>>;

    /// Store or update a LID-PN mapping.
    async fn put_lid_mapping(&self, entry: &LidPnMappingEntry) -> Result<()>;

    /// Batched variant of `put_lid_mapping`. Backends should override with a
    /// single transaction; the default loops for correctness. Mirrors WA Web's
    /// `WAWebDBCreateLidPnMappings.createLidPnMappings({ mappings, … })`.
    async fn put_lid_mappings(&self, entries: &[LidPnMappingEntry]) -> Result<()> {
        for entry in entries {
            self.put_lid_mapping(entry).await?;
        }
        Ok(())
    }

    /// Get all LID-PN mappings (for cache warm-up).
    async fn get_all_lid_mappings(&self) -> Result<Vec<LidPnMappingEntry>>;

    // --- Base Key Collision Detection ---

    /// Save the base key for a session address during retry collision detection.
    async fn save_base_key(&self, address: &str, message_id: &str, base_key: &[u8]) -> Result<()>;

    /// Check if the current session has the same base key as the saved one.
    async fn has_same_base_key(
        &self,
        address: &str,
        message_id: &str,
        current_base_key: &[u8],
    ) -> Result<bool>;

    /// Delete a base key entry.
    async fn delete_base_key(&self, address: &str, message_id: &str) -> Result<()>;

    // --- Device Registry ---

    /// Update the device list for a user (called after usync responses).
    async fn update_device_list(&self, record: DeviceListRecord) -> Result<()>;

    /// Batched variant of `update_device_list`. Backends should override with
    /// a single transaction; the default loops for correctness. Important on
    /// usync of large groups, where the per-row commit + spawn_blocking
    /// overhead dominates wall-clock time when called once per participant.
    async fn update_device_lists(&self, records: Vec<DeviceListRecord>) -> Result<()> {
        for record in records {
            self.update_device_list(record).await?;
        }
        Ok(())
    }

    /// Get all known devices for a user.
    async fn get_devices(&self, user: &str) -> Result<Option<DeviceListRecord>>;

    /// Delete a device list record, forcing a network re-fetch on next query.
    async fn delete_devices(&self, user: &str) -> Result<()>;

    // --- Group Metadata Cache (WA Web participant-phash re-query skip) ---

    /// Get the persisted, opaque serialized group metadata blob for `group_jid`.
    /// The blob is a caller-serialized GroupInfo snapshot; backends without group
    /// persistence return `None` (the group is then re-queried in full).
    async fn get_group_metadata(&self, _group_jid: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    /// Persist (upsert) the serialized group metadata blob for `group_jid`.
    /// No-op by default; backends override to enable the phash re-query skip.
    async fn put_group_metadata(&self, _group_jid: &str, _blob: &[u8]) -> Result<()> {
        Ok(())
    }

    /// Remove the persisted group metadata blob for `group_jid` (e.g. on leave),
    /// so the next query re-fetches in full instead of comparing a stale phash.
    /// No-op by default.
    async fn delete_group_metadata(&self, _group_jid: &str) -> Result<()> {
        Ok(())
    }

    // --- TcToken Storage ---

    /// Get a trusted contact token for a JID (stored under LID).
    async fn get_tc_token(&self, jid: &str) -> Result<Option<TcTokenEntry>>;

    /// Store or update a trusted contact token for a JID.
    async fn put_tc_token(&self, jid: &str, entry: &TcTokenEntry) -> Result<()>;

    /// Delete a trusted contact token for a JID.
    async fn delete_tc_token(&self, jid: &str) -> Result<()>;

    /// Get all JIDs that have stored tc tokens.
    async fn get_all_tc_token_jids(&self) -> Result<Vec<String>>;

    /// Delete tc tokens that have no live state left. A row is removed only when
    /// its received token is expired-or-absent (`token_timestamp < token_cutoff`
    /// or empty) **and** its sender bucket is expired-or-absent
    /// (`sender_timestamp < sender_cutoff` or null), so recent sender state is
    /// never dropped just because the received token expired. Returns count deleted.
    async fn delete_expired_tc_tokens(&self, token_cutoff: i64, sender_cutoff: i64) -> Result<u32>;

    /// Advance `sender_timestamp` toward `sender_timestamp` for a contact,
    /// inserting a byte-less placeholder when absent and preserving any existing
    /// token bytes. The stored value only ever moves forward (max), so
    /// concurrent writers (post-send issuance, history sync) converge regardless
    /// of ordering and never regress the sender bucket.
    ///
    /// Must be atomic w.r.t. [`put_tc_token`](Self::put_tc_token): the sender-side
    /// issuance path and the notification writer both touch the same row, so a
    /// non-atomic read-modify-write could drop a real token for a placeholder.
    /// The default is a read-modify-write for third-party backends; the built-in
    /// stores override it with a single atomic upsert.
    async fn touch_tc_token_sender_timestamp(
        &self,
        jid: &str,
        sender_timestamp: i64,
    ) -> Result<()> {
        let entry = match self.get_tc_token(jid).await? {
            Some(existing) => TcTokenEntry {
                sender_timestamp: Some(
                    existing
                        .sender_timestamp
                        .map_or(sender_timestamp, |e| e.max(sender_timestamp)),
                ),
                ..existing
            },
            None => TcTokenEntry {
                token: Vec::new(),
                token_timestamp: sender_timestamp,
                sender_timestamp: Some(sender_timestamp),
            },
        };
        self.put_tc_token(jid, &entry).await
    }

    /// Store a token received from a contact, preserving any existing
    /// `sender_timestamp`. The symmetric counterpart of
    /// [`touch_tc_token_sender_timestamp`](Self::touch_tc_token_sender_timestamp):
    /// each writer owns its own field, so the notification path never drops a
    /// sender bucket that the issuance path wrote concurrently.
    ///
    /// **Newer-wins**: the token pair is overwritten only when the stored token
    /// is a byte-less placeholder or the incoming `token_timestamp` is at least
    /// as new — a stale write must not clobber a fresher real token. Doing this
    /// in the store (atomically for the built-in backends) is what lets the
    /// concurrent history-sync and privacy-notification writers converge without
    /// a lock. Same atomicity requirement as the sender bucket — the default
    /// read-modify-write here is a best-effort for third-party backends.
    async fn store_received_tc_token(
        &self,
        jid: &str,
        token: &[u8],
        token_timestamp: i64,
    ) -> Result<()> {
        let existing = self.get_tc_token(jid).await?;
        // Keep a fresher real token; a placeholder never blocks the first real one.
        if let Some(existing) = &existing
            && !existing.token.is_empty()
            && token_timestamp < existing.token_timestamp
        {
            return Ok(());
        }
        let sender_timestamp = existing.and_then(|existing| existing.sender_timestamp);
        self.put_tc_token(
            jid,
            &TcTokenEntry {
                token: token.to_vec(),
                token_timestamp,
                sender_timestamp,
            },
        )
        .await
    }

    // --- Sent Message Store (retry support, matches WA Web's getMessageTable) ---

    /// Store a sent message's serialized payload for retry handling.
    /// Called after each send_message(); the payload is the protobuf-encoded Message.
    async fn store_sent_message(
        &self,
        chat_jid: &str,
        message_id: &str,
        payload: &[u8],
    ) -> Result<()>;

    /// Retrieve and delete a sent message (atomic take). Returns serialized payload.
    /// Called when a retry receipt arrives; consuming prevents double-retry.
    async fn take_sent_message(&self, chat_jid: &str, message_id: &str) -> Result<Option<Vec<u8>>>;

    /// Delete sent messages older than cutoff (unix timestamp seconds). Returns count deleted.
    async fn delete_expired_sent_messages(&self, cutoff_timestamp: i64) -> Result<u32>;

    // --- Pending Inbound Buffer (inbound durability hook) ---
    //
    // Backs the at-least-once inbound durability hook: a decrypted message is
    // buffered here (keyed by its stanza id) before the Signal ratchet is
    // flushed, so a crash or failed commit before the hook acks replays the
    // message on redelivery instead of dropping it. The defaults are non-breaking
    // for backends that do not implement the hook, but fail CLOSED rather than
    // no-op: an unsupported backend used with a hook surfaces an error (and the
    // message stays unacked) instead of silently degrading to at-most-once.

    /// Persist a decrypted inbound message awaiting a durability-hook commit.
    /// Scoped by `(chat, sender, id)` because stanza ids are only unique within
    /// a `(chat, sender)`.
    async fn store_pending_inbound(
        &self,
        _chat: &str,
        _sender: &str,
        _id: &str,
        _message: &[u8],
    ) -> Result<()> {
        Err(unsupported_pending_inbound())
    }

    /// Read a buffered inbound message by `(chat, sender, id)` without removing it.
    async fn get_pending_inbound(
        &self,
        _chat: &str,
        _sender: &str,
        _id: &str,
    ) -> Result<Option<Vec<u8>>> {
        Err(unsupported_pending_inbound())
    }

    /// Remove a buffered inbound message once its durability hook has committed.
    async fn delete_pending_inbound(&self, _chat: &str, _sender: &str, _id: &str) -> Result<()> {
        Err(unsupported_pending_inbound())
    }

    /// Delete buffered inbound messages older than cutoff (unix seconds). Returns
    /// count deleted. Unlike the other defaults this is a benign `Ok(0)`: the
    /// keepalive sweep calls it unconditionally for every backend, so it must not
    /// error when the buffer is unsupported.
    async fn delete_expired_pending_inbound(&self, _cutoff_timestamp: i64) -> Result<u32> {
        Ok(0)
    }

    /// Batched [`store_pending_inbound`](Self::store_pending_inbound): the
    /// offline drain buffers one commit-batch of messages per call, so backends
    /// should override this with a single transaction (the bundled SqliteStore
    /// does). The default iterates the single-row method, preserving behavior
    /// for third-party backends.
    async fn store_pending_inbound_batch(&self, rows: &[PendingInboundRow<'_>]) -> Result<()> {
        for row in rows {
            self.store_pending_inbound(row.chat, row.sender, row.id, row.message)
                .await?;
        }
        Ok(())
    }

    /// Batched [`delete_pending_inbound`](Self::delete_pending_inbound); same
    /// override guidance as [`store_pending_inbound_batch`](Self::store_pending_inbound_batch).
    async fn delete_pending_inbound_batch(&self, keys: &[PendingInboundKey<'_>]) -> Result<()> {
        for key in keys {
            self.delete_pending_inbound(key.chat, key.sender, key.id)
                .await?;
        }
        Ok(())
    }
}

/// One row of a pending-inbound batch write. Fields borrow from the in-flight
/// commit batch so building a batch allocates nothing per row.
#[derive(Debug, Clone, Copy)]
pub struct PendingInboundRow<'a> {
    pub chat: &'a str,
    pub sender: &'a str,
    pub id: &'a str,
    pub message: &'a [u8],
}

/// Key of a buffered pending-inbound row, for batched deletes.
#[derive(Debug, Clone, Copy)]
pub struct PendingInboundKey<'a> {
    pub chat: &'a str,
    pub sender: &'a str,
    pub id: &'a str,
}

/// Device data persistence operations.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait DeviceStore: Send + Sync {
    /// Save device data.
    async fn save(&self, device: &crate::store::Device) -> Result<()>;

    /// Load device data.
    async fn load(&self) -> Result<Option<crate::store::Device>>;

    /// Check if a device exists.
    async fn exists(&self) -> Result<bool>;

    /// Create a new device row and return its generated device_id.
    async fn create(&self) -> Result<i32>;

    /// Create a snapshot of the database state.
    /// The argument `name` can be used to label the snapshot file.
    /// `extra_content` can be used to save a related binary blob (e.g. the message that caused the failure).
    async fn snapshot_db(&self, _name: &str, _extra_content: Option<&[u8]>) -> Result<()> {
        Ok(())
    }

    /// Best-effort process-local memory this backend attributes to the session
    /// (e.g. a SQLite page cache — often the single largest per-session chunk,
    /// living entirely outside the `Client`). Defaults to an all-`None`
    /// [`StorageResourceReport`] ("not reported"); backends that can introspect
    /// their memory override it, and remote/store-backed backends report
    /// `memory_bytes: Some(0)` (their data isn't process memory).
    ///
    /// This is a defaulted method on `DeviceStore` — an already-implemented,
    /// non-blanket sub-trait of `Backend` — rather than an inherent method
    /// (which wouldn't compose through the `Arc<dyn Backend>` the client holds)
    /// or a new `Backend` supertrait (which would force *every* backend,
    /// including external ones, to add an impl). The default keeps it fully
    /// non-breaking, exactly like [`Self::snapshot_db`].
    async fn resource_report(&self) -> crate::stats::StorageResourceReport {
        crate::stats::StorageResourceReport::default()
    }
}

/// Per-outbound-message secret storage for addon-style decryption.
///
/// Persists the 32-byte `MessageContextInfo.messageSecret` we send out so that
/// later inbound replies (poll votes, reactions, msmsg bot responses, edits)
/// referencing the original message ID can be decrypted. Mirrors WA Web's
/// `WAWebMsmsgMsgSecretCache` + the `messageSecret` field on the DB message row.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait MsgSecretStore: Send + Sync {
    /// Persist the protocol-sized `secret` under the composite key with NO
    /// expiry (`expires_at = 0`). Convenience wrapper over [`put_msg_secrets`].
    /// `chat`, `sender`, and `msg_id` are JID strings / message ID strings;
    /// callers should pass non-AD (no-device) form for the JIDs so lookups
    /// match regardless of which device echo'd the stanza back.
    ///
    /// Real call sites that compute a retention deadline build
    /// [`MsgSecretEntry`] directly and call [`put_msg_secrets`].
    ///
    /// [`put_msg_secrets`]: MsgSecretStore::put_msg_secrets
    async fn put_msg_secret(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
        secret: &[u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
    ) -> Result<()> {
        self.put_msg_secrets(vec![MsgSecretEntry {
            chat: Arc::from(chat),
            sender: Arc::from(sender),
            msg_id: Arc::from(msg_id),
            secret: Box::new(*secret),
            expires_at: 0,
            message_ts: 0,
        }])
        .await?;
        Ok(())
    }

    /// Batched upsert carrying a per-row `expires_at` deadline. On key conflict
    /// implementations merge deterministically via [`merge_msg_secret_expiry`]
    /// (later deadline wins, `0` = "never" = infinity) so a redelivery or edit
    /// re-persist never shortens a window, and via [`merge_msg_secret_message_ts`]
    /// (the later non-zero parent time wins; a `0` never clobbers a known one).
    async fn put_msg_secrets(&self, entries: Vec<MsgSecretEntry>) -> Result<usize>;

    /// Fetch the persisted secret; returns `None` if absent.
    async fn get_msg_secret(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<Vec<u8>>>;

    /// Fetch the secret together with the parent message's event time
    /// (`message_ts`, `0` when unknown), so the receive path can enforce the
    /// edit-processing window. Default pairs `get_msg_secret` with `0`;
    /// backends that store `message_ts` override this.
    async fn get_msg_secret_with_ts(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Result<Option<(Vec<u8>, i64)>> {
        Ok(self
            .get_msg_secret(chat, sender, msg_id)
            .await?
            .map(|secret| (secret, 0)))
    }

    /// Delete rows whose non-zero `expires_at` is at or before
    /// `cutoff_timestamp` (absolute unix seconds; callers pass "now"). Rows
    /// with `expires_at = 0` (never) are kept. Returns the number removed so
    /// the keepalive cleanup can log/throttle.
    async fn delete_expired_msg_secrets(&self, cutoff_timestamp: i64) -> Result<u32>;
}

/// Merge two `expires_at` deadlines on key conflict: `0` ("never") wins,
/// otherwise the later (larger) deadline is kept so windows never shrink.
pub fn merge_msg_secret_expiry(existing: i64, incoming: i64) -> i64 {
    if existing == 0 || incoming == 0 {
        0
    } else {
        existing.max(incoming)
    }
}

/// Merge two parent `message_ts` values on key conflict: the later (larger)
/// non-zero value wins, so a `0` ("unknown") never clobbers a known parent
/// time. `max` already yields this because every real timestamp is `> 0`.
pub fn merge_msg_secret_message_ts(existing: i64, incoming: i64) -> i64 {
    existing.max(incoming)
}

/// Combined storage backend trait.
///
/// Any type implementing all domain traits automatically implements `Backend`.
pub trait Backend:
    SignalStore + AppSyncStore + ProtocolStore + MsgSecretStore + DeviceStore + Send + Sync
{
}

impl<T> Backend for T where
    T: SignalStore + AppSyncStore + ProtocolStore + MsgSecretStore + DeviceStore + Send + Sync
{
}
