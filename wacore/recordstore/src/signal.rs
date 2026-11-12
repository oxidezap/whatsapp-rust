// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use crate::record::{Fence, Namespace, RecordExists, RecordStore, WriteOp};
use crate::seal::{SealError, Sealer};
use async_lock::Mutex;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::BTreeMap;
use std::sync::Arc;
use wacore::store::error::{Result, StoreError};
use wacore::store::traits::SignalStore;

const AAD_DOMAIN: &[u8] = b"wacore-recordstore/v1";
const DEVICE_KEY: &str = "local";

/// Prekey plaintext starts with this flag byte, then the libsignal record.
const PREKEY_NOT_UPLOADED: u8 = 0;
const PREKEY_UPLOADED: u8 = 1;

/// Namespaces the Signal service reads and writes directly, outside the
/// [`SignalStore`] surface.
const AUX_NAMESPACES: [Namespace; 4] = [
    Namespace::Device,
    Namespace::DecryptBuffer,
    Namespace::SentMessage,
    Namespace::SenderKeyDevices,
];

/// A write before sealing.
#[derive(Debug, Clone)]
enum PlainOp {
    Put(Namespace, String, Vec<u8>),
    Insert(Namespace, String, Vec<u8>),
    Update(Namespace, String, Vec<u8>),
    Delete(Namespace, String),
}

/// An uncommitted change held by a staged store.
#[derive(Debug, Clone)]
enum Staged {
    Put(Vec<u8>),
    /// Applied only if the key is still absent when the batch commits.
    Insert(Vec<u8>),
    /// No final value, while preserving an earlier insert's absence check.
    InsertDeleted,
    /// Applied only if the key still exists when the batch commits.
    Update(Vec<u8>),
    Delete,
}

type Overlay = BTreeMap<(Namespace, String), Staged>;

/// [`SignalStore`] for one scope, sealed by `S` and fenced by the lease that
/// produced `fence`. Build a new one whenever the lease is re-acquired.
///
/// [`Self::staged`] returns a view whose writes stay in memory until
/// [`Self::commit`] seals them and applies them as one fenced batch, so a
/// decrypt's session update, consumed-prekey delete and buffered result land
/// together or not at all.
pub struct RecordSignalStore<R, S> {
    records: Arc<R>,
    sealer: Arc<S>,
    fence: Fence,
    overlay: Option<Mutex<Overlay>>,
}

impl<R: RecordStore, S: Sealer> RecordSignalStore<R, S> {
    pub fn new(records: Arc<R>, sealer: Arc<S>, fence: Fence) -> Self {
        Self {
            records,
            sealer,
            fence,
            overlay: None,
        }
    }

    pub fn fence(&self) -> &Fence {
        &self.fence
    }

    /// The record backend shared by every store built over it.
    pub fn records(&self) -> &Arc<R> {
        &self.records
    }

    /// Count surviving prekeys marked uploaded, refusing a superseded fence.
    /// Missing records consumed during this read are skipped. This is a
    /// count of local state, not confirmation of the provider's current pool.
    pub async fn uploaded_prekey_count(&self) -> Result<u32> {
        self.records.write(&self.fence, &[]).await?;
        let keys = self.scan(Namespace::PreKey, "", None).await?;
        let mut count = 0u32;
        for chunk in keys.chunks(128) {
            let refs: Vec<_> = chunk.iter().map(String::as_str).collect();
            for (_, value) in self.load_many(Namespace::PreKey, &refs).await? {
                match value.first() {
                    Some(&PREKEY_UPLOADED) => {
                        count = count
                            .checked_add(1)
                            .ok_or_else(|| StoreError::Validation("prekey count overflow".into()))?
                    }
                    Some(&PREKEY_NOT_UPLOADED) => {}
                    _ => {
                        return Err(StoreError::Validation(
                            "prekey record has no upload flag".into(),
                        ));
                    }
                }
            }
        }
        self.records.write(&self.fence, &[]).await?;
        Ok(count)
    }

    /// A view over the same scope whose writes are held until [`Self::commit`].
    /// Reads see the view's own pending writes first.
    pub fn staged(&self) -> Self {
        Self {
            records: self.records.clone(),
            sealer: self.sealer.clone(),
            fence: self.fence.clone(),
            overlay: Some(Mutex::new(Overlay::new())),
        }
    }

    /// Seal and apply every pending write of a staged view in one fenced
    /// batch. Pending writes remain available after an error or cancellation.
    /// Committing a view that is not staged is a no-op.
    ///
    /// If the future is dropped after the backend applied the batch, the
    /// pending writes survive and a retry fails with `RecordExists` on its
    /// inserts. Re-read the affected records to reconcile before retrying.
    pub async fn commit(&self) -> Result<()> {
        let mut pending = match &self.overlay {
            Some(overlay) => overlay.lock().await,
            None => return Ok(()),
        };
        let mut ops = Vec::with_capacity(pending.len());
        // Keep the async guard until the durable write succeeds. Concurrent
        // edits cannot disappear behind a successful commit or a failed retry.
        for ((ns, key), change) in pending.iter() {
            let ns = *ns;
            let key = key.clone();
            let op = match change {
                Staged::Put(plaintext) => WriteOp::Put {
                    value: self.seal(ns, &key, plaintext).await?,
                    ns,
                    key,
                },
                Staged::Insert(plaintext) => WriteOp::Insert {
                    value: self.seal(ns, &key, plaintext).await?,
                    ns,
                    key,
                },
                Staged::InsertDeleted => {
                    // The batch must still prove the slot was absent, even
                    // though this view's final state has no record there.
                    ops.push(WriteOp::Insert {
                        value: self.seal(ns, &key, &[]).await?,
                        ns,
                        key: key.clone(),
                    });
                    WriteOp::Delete { ns, key }
                }
                Staged::Update(plaintext) => WriteOp::Update {
                    value: self.seal(ns, &key, plaintext).await?,
                    ns,
                    key,
                },
                Staged::Delete => WriteOp::Delete { ns, key },
            };
            ops.push(op);
        }
        if ops.is_empty() {
            return Ok(());
        }
        self.records.write(&self.fence, &ops).await?;
        pending.clear();
        Ok(())
    }

    fn scope(&self) -> &str {
        &self.fence.scope
    }

    /// Authenticated data binding a value to its slot. Every component is
    /// length-prefixed so no two (scope, namespace, key) triples encode alike;
    /// scopes and keys may contain any byte, including NUL.
    fn aad(&self, ns: Namespace, key: &str) -> Vec<u8> {
        let mut aad =
            Vec::with_capacity(AAD_DOMAIN.len() + 12 + self.scope().len() + key.len() + 16);
        for part in [
            AAD_DOMAIN,
            self.scope().as_bytes(),
            ns.tag().as_bytes(),
            key.as_bytes(),
        ] {
            aad.extend_from_slice(&(part.len() as u32).to_be_bytes());
            aad.extend_from_slice(part);
        }
        aad
    }

    async fn seal(&self, ns: Namespace, key: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        self.sealer
            .seal(&self.aad(ns, key), plaintext)
            .await
            .map_err(seal_error)
    }

    async fn open(&self, ns: Namespace, key: &str, sealed: &[u8]) -> Result<Vec<u8>> {
        self.sealer
            .open(&self.aad(ns, key), sealed)
            .await
            .map_err(seal_error)
    }

    /// The staged change for one key, if this view holds one.
    async fn pending(&self, ns: Namespace, key: &str) -> Option<Staged> {
        match &self.overlay {
            Some(overlay) => overlay.lock().await.get(&(ns, key.to_owned())).cloned(),
            None => None,
        }
    }

    async fn load(&self, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        match self.pending(ns, key).await {
            Some(Staged::Put(v) | Staged::Update(v) | Staged::Insert(v)) => return Ok(Some(v)),
            Some(Staged::Delete | Staged::InsertDeleted) => return Ok(None),
            None => {}
        }
        match self.records.get(self.scope(), ns, key).await? {
            Some(sealed) => Ok(Some(self.open(ns, key, &sealed).await?)),
            None => Ok(None),
        }
    }

    async fn load_many(&self, ns: Namespace, keys: &[&str]) -> Result<Vec<(String, Vec<u8>)>> {
        let mut opened = Vec::with_capacity(keys.len());
        let mut from_store = Vec::with_capacity(keys.len());
        for key in keys {
            match self.pending(ns, key).await {
                Some(Staged::Put(v) | Staged::Update(v) | Staged::Insert(v)) => {
                    opened.push(((*key).to_owned(), v))
                }
                Some(Staged::Delete | Staged::InsertDeleted) => {}
                None => from_store.push(*key),
            }
        }
        for (key, sealed) in self.records.get_many(self.scope(), ns, &from_store).await? {
            let plaintext = self.open(ns, &key, &sealed).await?;
            opened.push((key, plaintext));
        }
        Ok(opened)
    }

    /// Keys with `prefix`, ascending, including this view's pending changes.
    async fn scan(&self, ns: Namespace, prefix: &str, limit: Option<usize>) -> Result<Vec<String>> {
        let Some(overlay) = &self.overlay else {
            return self
                .records
                .scan_keys(self.scope(), ns, prefix, limit)
                .await;
        };
        let overlay = overlay.lock().await;
        // Pending deletes can hide stored keys, so ask the backend for that
        // many extra. Inserts only add keys, so they need no headroom.
        let backend_limit = limit.map(|limit| {
            let hidden = overlay
                .iter()
                .filter(|((pns, key), change)| {
                    *pns == ns
                        && key.starts_with(prefix)
                        && matches!(change, Staged::Delete | Staged::InsertDeleted)
                })
                .count();
            limit.saturating_add(hidden)
        });
        let mut keys: std::collections::BTreeSet<String> = self
            .records
            .scan_keys(self.scope(), ns, prefix, backend_limit)
            .await?
            .into_iter()
            .collect();
        for ((pns, key), change) in overlay.iter() {
            if *pns != ns || !key.starts_with(prefix) {
                continue;
            }
            match change {
                Staged::Put(_) | Staged::Insert(_) => {
                    keys.insert(key.clone());
                }
                Staged::Delete | Staged::InsertDeleted => {
                    keys.remove(key);
                }
                Staged::Update(_) => {}
            }
        }
        let keys = keys.into_iter();
        Ok(match limit {
            Some(limit) => keys.take(limit).collect(),
            None => keys.collect(),
        })
    }

    async fn apply(&self, ops: Vec<PlainOp>) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }
        if let Some(overlay) = &self.overlay {
            let mut overlay = overlay.lock().await;
            // Validate the entire call before changing the view. A second
            // exclusive insert must not replace an earlier staged private key.
            {
                let mut inserts = std::collections::BTreeSet::new();
                for op in &ops {
                    if let PlainOp::Insert(ns, key, _) = op
                        && (!inserts.insert((*ns, key))
                            || overlay.get(&(*ns, key.clone())).is_some_and(|change| {
                                !matches!(change, Staged::Delete | Staged::InsertDeleted)
                            }))
                    {
                        return Err(RecordExists {
                            ns: *ns,
                            key: key.clone(),
                        }
                        .into_store_error());
                    }
                }
            }
            for op in ops {
                let (slot, change) = match op {
                    PlainOp::Put(ns, key, v) => {
                        // A later write may replace the staged value, but it
                        // must retain the original absence check at commit.
                        let change = match overlay.get(&(ns, key.clone())) {
                            Some(Staged::Insert(_) | Staged::InsertDeleted) => Staged::Insert(v),
                            _ => Staged::Put(v),
                        };
                        ((ns, key), change)
                    }
                    PlainOp::Insert(ns, key, v) => ((ns, key), Staged::Insert(v)),
                    PlainOp::Update(ns, key, v) => {
                        // Updating a key this view already wrote keeps it a Put.
                        let staged = match overlay.get(&(ns, key.clone())) {
                            Some(Staged::Put(_)) => Staged::Put(v),
                            Some(Staged::Insert(_)) => Staged::Insert(v),
                            Some(Staged::Delete | Staged::InsertDeleted) => continue,
                            _ => Staged::Update(v),
                        };
                        ((ns, key), staged)
                    }
                    PlainOp::Delete(ns, key) => {
                        let change = match overlay.get(&(ns, key.clone())) {
                            Some(Staged::Insert(_) | Staged::InsertDeleted) => {
                                Staged::InsertDeleted
                            }
                            _ => Staged::Delete,
                        };
                        ((ns, key), change)
                    }
                };
                overlay.insert(slot, change);
            }
            return Ok(());
        }
        let mut sealed = Vec::with_capacity(ops.len());
        for op in ops {
            sealed.push(match op {
                PlainOp::Put(ns, key, v) => WriteOp::Put {
                    value: self.seal(ns, &key, &v).await?,
                    ns,
                    key,
                },
                PlainOp::Insert(ns, key, v) => WriteOp::Insert {
                    value: self.seal(ns, &key, &v).await?,
                    ns,
                    key,
                },
                PlainOp::Update(ns, key, v) => WriteOp::Update {
                    value: self.seal(ns, &key, &v).await?,
                    ns,
                    key,
                },
                PlainOp::Delete(ns, key) => WriteOp::Delete { ns, key },
            });
        }
        self.records.write(&self.fence, &sealed).await
    }

    async fn put_one(&self, ns: Namespace, key: &str, plaintext: &[u8]) -> Result<()> {
        self.apply(vec![PlainOp::Put(ns, key.to_owned(), plaintext.to_vec())])
            .await
    }

    async fn delete_keys<K: AsRef<str>>(&self, ns: Namespace, keys: &[K]) -> Result<()> {
        self.apply(
            keys.iter()
                .map(|key| PlainOp::Delete(ns, key.as_ref().to_owned()))
                .collect(),
        )
        .await
    }

    fn check_aux(ns: Namespace) -> Result<()> {
        if AUX_NAMESPACES.contains(&ns) {
            Ok(())
        } else {
            Err(StoreError::Validation(format!(
                "{} records are only reachable through SignalStore",
                ns.tag()
            )))
        }
    }

    /// Read a record in one of the service's own namespaces
    /// ([`Namespace::Device`], [`Namespace::DecryptBuffer`],
    /// [`Namespace::SentMessage`]).
    pub async fn load_aux(&self, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        Self::check_aux(ns)?;
        self.load(ns, key).await
    }

    /// Write a record in one of the service's own namespaces.
    pub async fn put_aux(&self, ns: Namespace, key: &str, plaintext: &[u8]) -> Result<()> {
        Self::check_aux(ns)?;
        self.put_one(ns, key, plaintext).await
    }

    /// Insert an auxiliary record exclusively, including in a staged batch.
    pub async fn insert_aux(&self, ns: Namespace, key: &str, plaintext: &[u8]) -> Result<()> {
        Self::check_aux(ns)?;
        self.apply(vec![PlainOp::Insert(
            ns,
            key.to_owned(),
            plaintext.to_vec(),
        )])
        .await
    }

    /// Reserve fresh prekey IDs without overwriting an existing private key.
    pub async fn insert_prekeys_batch(&self, keys: &[(u32, Bytes)]) -> Result<()> {
        // A repeated ID would replace the first staged insert, so commit would
        // succeed with fewer private keys stored than the caller supplied.
        let mut seen = std::collections::HashSet::with_capacity(keys.len());
        if let Some((id, _)) = keys.iter().find(|(id, _)| !seen.insert(*id)) {
            return Err(StoreError::Validation(format!(
                "prekey batch repeats id {id}"
            )));
        }
        self.apply(
            keys.iter()
                .map(|(id, record)| {
                    PlainOp::Insert(
                        Namespace::PreKey,
                        id_key(*id),
                        prekey_plaintext(record, false),
                    )
                })
                .collect(),
        )
        .await
    }

    /// Delete records in one of the service's own namespaces.
    pub async fn delete_aux(&self, ns: Namespace, keys: &[&str]) -> Result<()> {
        Self::check_aux(ns)?;
        self.delete_keys(ns, keys).await
    }

    /// Keys in one of the service's own namespaces, ascending.
    pub async fn scan_aux(
        &self,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        Self::check_aux(ns)?;
        self.scan(ns, prefix, limit).await
    }

    /// Peer addresses with a stored session or identity, ascending. Only the
    /// keys are read, so callers can find every device of a user.
    pub async fn scan_peer_addresses(
        &self,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        if !matches!(ns, Namespace::Session | Namespace::Identity) {
            return Err(StoreError::Validation(format!(
                "{} records do not hold peer addresses",
                ns.tag()
            )));
        }
        self.scan(ns, prefix, limit).await
    }

    /// The local device's sealed key blob, if the scope has been initialised.
    pub async fn load_device(&self) -> Result<Option<Vec<u8>>> {
        self.load(Namespace::Device, DEVICE_KEY).await
    }

    /// Write the local device's first key blob. Fails with
    /// [`crate::RecordExists`] if the scope already has one, so two concurrent
    /// initialisers cannot both succeed.
    pub async fn create_device(&self, blob: &[u8]) -> Result<()> {
        self.apply(vec![PlainOp::Insert(
            Namespace::Device,
            DEVICE_KEY.to_owned(),
            blob.to_vec(),
        )])
        .await
    }

    /// Replace the local device's key blob, fenced like every other write.
    pub async fn save_device(&self, blob: &[u8]) -> Result<()> {
        self.put_one(Namespace::Device, DEVICE_KEY, blob).await
    }

    async fn any_key_with_prefix(&self, ns: Namespace, prefix: &str) -> Result<bool> {
        Ok(!self.scan(ns, prefix, Some(1)).await?.is_empty())
    }
}

fn seal_error(error: SealError) -> StoreError {
    StoreError::Serialization(Box::new(error))
}

/// Zero-padded so byte order matches numeric order in every adapter.
fn id_key(id: u32) -> String {
    format!("{id:010}")
}

fn parse_id(key: &str) -> Result<u32> {
    key.parse()
        .map_err(|_| StoreError::Validation(format!("invalid record id key: {key}")))
}

fn prekey_plaintext(record: &[u8], uploaded: bool) -> Vec<u8> {
    let mut plaintext = Vec::with_capacity(record.len() + 1);
    plaintext.push(if uploaded {
        PREKEY_UPLOADED
    } else {
        PREKEY_NOT_UPLOADED
    });
    plaintext.extend_from_slice(record);
    plaintext
}

fn prekey_record(plaintext: Vec<u8>) -> Result<Bytes> {
    match plaintext.first() {
        Some(&PREKEY_NOT_UPLOADED | &PREKEY_UPLOADED) => Ok(Bytes::from(plaintext).slice(1..)),
        _ => Err(StoreError::Validation(
            "prekey record has no upload flag".into(),
        )),
    }
}

fn identity_key(plaintext: Vec<u8>) -> Result<[u8; 32]> {
    let len = plaintext.len();
    plaintext
        .try_into()
        .map_err(|_| StoreError::Validation(format!("Invalid identity key length: {len}")))
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<R: RecordStore, S: Sealer> SignalStore for RecordSignalStore<R, S> {
    async fn put_identity(&self, address: &str, key: [u8; 32]) -> Result<()> {
        self.put_one(Namespace::Identity, address, &key).await
    }

    async fn put_identities_batch(&self, identities: &[(Arc<str>, [u8; 32])]) -> Result<()> {
        self.apply(
            identities
                .iter()
                .map(|(a, k)| PlainOp::Put(Namespace::Identity, a.to_string(), k.to_vec()))
                .collect(),
        )
        .await
    }

    async fn load_identity(&self, address: &str) -> Result<Option<[u8; 32]>> {
        self.load(Namespace::Identity, address)
            .await?
            .map(identity_key)
            .transpose()
    }

    async fn delete_identity(&self, address: &str) -> Result<()> {
        self.delete_keys(Namespace::Identity, &[address]).await
    }

    async fn delete_identities_batch(&self, addresses: &[Arc<str>]) -> Result<()> {
        self.delete_keys(Namespace::Identity, addresses).await
    }

    async fn get_session(&self, address: &str) -> Result<Option<Bytes>> {
        Ok(self
            .load(Namespace::Session, address)
            .await?
            .map(Bytes::from))
    }

    async fn put_session(&self, address: &str, session: &[u8]) -> Result<()> {
        self.put_one(Namespace::Session, address, session).await
    }

    async fn put_sessions_batch(&self, sessions: &[(Arc<str>, Bytes)]) -> Result<()> {
        self.apply(
            sessions
                .iter()
                .map(|(a, s)| PlainOp::Put(Namespace::Session, a.to_string(), s.to_vec()))
                .collect(),
        )
        .await
    }

    async fn get_sessions_batch(&self, addresses: &[Arc<str>]) -> Result<Vec<(Arc<str>, Bytes)>> {
        let keys: Vec<&str> = addresses.iter().map(|a| &**a).collect();
        let found = self.load_many(Namespace::Session, &keys).await?;
        Ok(found
            .into_iter()
            .map(|(key, value)| {
                // Hand back the caller's own Arc where possible.
                let address = addresses
                    .iter()
                    .find(|a| ***a == *key)
                    .cloned()
                    .unwrap_or_else(|| Arc::from(key));
                (address, Bytes::from(value))
            })
            .collect())
    }

    async fn delete_session(&self, address: &str) -> Result<()> {
        self.delete_keys(Namespace::Session, &[address]).await
    }

    async fn delete_sessions_batch(&self, addresses: &[Arc<str>]) -> Result<()> {
        self.delete_keys(Namespace::Session, addresses).await
    }

    async fn has_session(&self, address: &str) -> Result<bool> {
        match self.pending(Namespace::Session, address).await {
            Some(Staged::Delete | Staged::InsertDeleted) => Ok(false),
            Some(_) => Ok(true),
            None => Ok(self
                .records
                .get(self.scope(), Namespace::Session, address)
                .await?
                .is_some()),
        }
    }

    async fn has_signal_state_for_user(&self, user: &str) -> Result<bool> {
        // Addresses are `user@server` (device 0) or `user:device@server`.
        let device_zero = format!("{user}@");
        let other_devices = format!("{user}:");
        for ns in [Namespace::Session, Namespace::Identity] {
            if self.any_key_with_prefix(ns, &device_zero).await?
                || self.any_key_with_prefix(ns, &other_devices).await?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn store_prekey(&self, id: u32, record: &[u8], uploaded: bool) -> Result<()> {
        self.put_one(
            Namespace::PreKey,
            &id_key(id),
            &prekey_plaintext(record, uploaded),
        )
        .await
    }

    async fn store_prekeys_batch(&self, keys: &[(u32, Bytes)], uploaded: bool) -> Result<()> {
        self.apply(
            keys.iter()
                .map(|(id, r)| {
                    PlainOp::Put(
                        Namespace::PreKey,
                        id_key(*id),
                        prekey_plaintext(r, uploaded),
                    )
                })
                .collect(),
        )
        .await
    }

    async fn load_prekey(&self, id: u32) -> Result<Option<Bytes>> {
        self.load(Namespace::PreKey, &id_key(id))
            .await?
            .map(prekey_record)
            .transpose()
    }

    async fn load_prekeys_batch(&self, ids: &[u32]) -> Result<Vec<(u32, Bytes)>> {
        let keys: Vec<String> = ids.iter().map(|id| id_key(*id)).collect();
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let mut result = Vec::with_capacity(ids.len());
        for (key, plaintext) in self.load_many(Namespace::PreKey, &key_refs).await? {
            result.push((parse_id(&key)?, prekey_record(plaintext)?));
        }
        Ok(result)
    }

    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<()> {
        let keys: Vec<String> = ids.iter().map(|id| id_key(*id)).collect();
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let mut ops = Vec::with_capacity(ids.len());
        for (key, plaintext) in self.load_many(Namespace::PreKey, &key_refs).await? {
            let record = prekey_record(plaintext)?;
            // Update, not Put: a key consumed after the read above stays
            // deleted instead of being resurrected by this write.
            ops.push(PlainOp::Update(
                Namespace::PreKey,
                key,
                prekey_plaintext(&record, true),
            ));
        }
        self.apply(ops).await
    }

    async fn remove_prekey(&self, id: u32) -> Result<()> {
        self.delete_keys(Namespace::PreKey, &[id_key(id)]).await
    }

    async fn remove_prekeys_batch(&self, ids: &[u32]) -> Result<()> {
        let keys: Vec<String> = ids.iter().map(|id| id_key(*id)).collect();
        self.delete_keys(Namespace::PreKey, &keys).await
    }

    async fn get_max_prekey_id(&self) -> Result<u32> {
        let keys = self.scan(Namespace::PreKey, "", None).await?;
        keys.last().map_or(Ok(0), |key| parse_id(key))
    }

    async fn store_signed_prekey(&self, id: u32, record: &[u8]) -> Result<()> {
        self.put_one(Namespace::SignedPreKey, &id_key(id), record)
            .await
    }

    async fn load_signed_prekey(&self, id: u32) -> Result<Option<Vec<u8>>> {
        self.load(Namespace::SignedPreKey, &id_key(id)).await
    }

    async fn load_all_signed_prekeys(&self) -> Result<Vec<(u32, Vec<u8>)>> {
        let keys = self.scan(Namespace::SignedPreKey, "", None).await?;
        let key_refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        let mut result = Vec::with_capacity(keys.len());
        for (key, plaintext) in self.load_many(Namespace::SignedPreKey, &key_refs).await? {
            result.push((parse_id(&key)?, plaintext));
        }
        result.sort_by_key(|(id, _)| *id);
        Ok(result)
    }

    async fn remove_signed_prekey(&self, id: u32) -> Result<()> {
        self.delete_keys(Namespace::SignedPreKey, &[id_key(id)])
            .await
    }

    async fn put_sender_key(&self, address: &str, record: &[u8]) -> Result<()> {
        self.put_one(Namespace::SenderKey, address, record).await
    }

    async fn put_sender_keys_batch(&self, sender_keys: &[(Arc<str>, Bytes)]) -> Result<()> {
        self.apply(
            sender_keys
                .iter()
                .map(|(a, r)| PlainOp::Put(Namespace::SenderKey, a.to_string(), r.to_vec()))
                .collect(),
        )
        .await
    }

    async fn get_sender_key(&self, address: &str) -> Result<Option<Vec<u8>>> {
        self.load(Namespace::SenderKey, address).await
    }

    async fn delete_sender_key(&self, address: &str) -> Result<()> {
        self.delete_keys(Namespace::SenderKey, &[address]).await
    }

    async fn delete_sender_keys_batch(&self, addresses: &[Arc<str>]) -> Result<()> {
        self.delete_keys(Namespace::SenderKey, addresses).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryRecordStore;
    use crate::record::{LeaseStore, is_fence_lost};
    use crate::seal::AesGcmSealer;
    use futures::executor::block_on;

    const TTL: u64 = 10_000;

    fn setup() -> (
        Arc<MemoryRecordStore>,
        RecordSignalStore<MemoryRecordStore, AesGcmSealer>,
    ) {
        let records = Arc::new(MemoryRecordStore::new());
        let lease = block_on(records.acquire("device-1", "node-a", 0, TTL))
            .unwrap()
            .unwrap();
        let store = RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&[3; 32])),
            lease.fence,
        );
        (records, store)
    }

    /// Delegates to a [`MemoryRecordStore`] and records each scan limit.
    #[derive(Default)]
    struct ScanSpy {
        inner: MemoryRecordStore,
        limits: std::sync::Mutex<Vec<Option<usize>>>,
    }

    #[async_trait::async_trait]
    impl RecordStore for ScanSpy {
        async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
            self.inner.get(scope, ns, key).await
        }

        async fn scan_keys(
            &self,
            scope: &str,
            ns: Namespace,
            prefix: &str,
            limit: Option<usize>,
        ) -> Result<Vec<String>> {
            self.limits.lock().unwrap().push(limit);
            self.inner.scan_keys(scope, ns, prefix, limit).await
        }

        async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()> {
            self.inner.write(fence, ops).await
        }
    }

    #[test]
    fn a_limited_staged_scan_stays_bounded_and_skips_pending_deletes() {
        let records = Arc::new(ScanSpy::default());
        let lease = block_on(records.inner.acquire("device-1", "node-a", 0, TTL))
            .unwrap()
            .unwrap();
        let store = RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&[3; 32])),
            lease.fence,
        );
        for key in ["s1", "s2", "s3", "s4", "s5", "s6"] {
            block_on(store.put_session(key, b"v")).unwrap();
        }
        let staged = store.staged();
        block_on(staged.delete_session("s1")).unwrap();
        block_on(staged.delete_session("s2")).unwrap();
        block_on(staged.put_session("s25", b"v")).unwrap();
        let keys = block_on(staged.scan(Namespace::Session, "s", Some(3))).unwrap();
        assert_eq!(keys, ["s25", "s3", "s4"]);
        assert_eq!(records.limits.lock().unwrap().as_slice(), [Some(5)]);
    }

    #[test]
    fn a_prekey_batch_with_a_repeated_id_is_refused_whole() {
        let (_, store) = setup();
        let keys = [
            (7, Bytes::from_static(b"a")),
            (8, Bytes::from_static(b"b")),
            (7, Bytes::from_static(b"c")),
        ];
        assert!(matches!(
            block_on(store.insert_prekeys_batch(&keys)),
            Err(StoreError::Validation(_))
        ));
        assert_eq!(block_on(store.load_prekey(7)).unwrap(), None);
        assert_eq!(block_on(store.load_prekey(8)).unwrap(), None);
    }

    #[test]
    fn repeated_staged_prekey_insert_refuses_the_whole_later_batch() {
        let (_, store) = setup();
        let staged = store.staged();
        block_on(staged.insert_prekeys_batch(&[(7, Bytes::from_static(b"first"))])).unwrap();
        let error = block_on(staged.insert_prekeys_batch(&[
            (8, Bytes::from_static(b"must-not-stage")),
            (7, Bytes::from_static(b"replacement")),
        ]))
        .unwrap_err();
        assert!(crate::is_record_exists(&error));
        assert_eq!(
            block_on(staged.load_prekey(7)).unwrap(),
            Some(Bytes::from_static(b"first"))
        );
        assert_eq!(block_on(staged.load_prekey(8)).unwrap(), None);
        block_on(staged.commit()).unwrap();
        assert_eq!(
            block_on(store.load_prekey(7)).unwrap(),
            Some(Bytes::from_static(b"first"))
        );
        assert_eq!(block_on(store.load_prekey(8)).unwrap(), None);
    }

    #[test]
    fn a_later_staged_put_preserves_the_exclusive_insert_condition() {
        let (_, store) = setup();
        let staged = store.staged();
        block_on(staged.insert_prekeys_batch(&[(7, Bytes::from_static(b"first"))])).unwrap();
        block_on(staged.store_prekey(7, b"updated", false)).unwrap();
        block_on(store.store_prekey(7, b"independent", false)).unwrap();
        let error = block_on(staged.commit()).unwrap_err();
        assert!(crate::is_record_exists(&error));
        let retry = block_on(staged.commit()).unwrap_err();
        assert!(crate::is_record_exists(&retry));
        assert_eq!(
            block_on(staged.load_prekey(7)).unwrap(),
            Some(Bytes::from_static(b"updated"))
        );
        assert_eq!(
            block_on(store.load_prekey(7)).unwrap(),
            Some(Bytes::from_static(b"independent"))
        );
    }

    #[test]
    fn staged_deletes_and_later_writes_keep_the_original_insert_condition() {
        for put_after_delete in [false, true] {
            let (_, store) = setup();
            let staged = store.staged();
            block_on(staged.insert_prekeys_batch(&[(7, Bytes::from_static(b"first"))])).unwrap();
            block_on(staged.remove_prekey(7)).unwrap();
            assert_eq!(block_on(staged.load_prekey(7)).unwrap(), None);
            if put_after_delete {
                block_on(staged.store_prekey(7, b"later", false)).unwrap();
            }
            block_on(staged.put_aux(Namespace::DecryptBuffer, "unrelated", b"must-not-commit"))
                .unwrap();
            block_on(store.store_prekey(7, b"independent", false)).unwrap();
            for _ in 0..2 {
                let error = block_on(staged.commit()).unwrap_err();
                assert!(crate::is_record_exists(&error));
                assert_eq!(
                    block_on(store.load_prekey(7)).unwrap(),
                    Some(Bytes::from_static(b"independent"))
                );
                assert_eq!(
                    block_on(store.load_aux(Namespace::DecryptBuffer, "unrelated")).unwrap(),
                    None
                );
            }
        }
    }

    #[test]
    fn inserting_then_deleting_an_absent_key_commits_without_leaving_a_record() {
        let (_, store) = setup();
        let staged = store.staged();
        block_on(staged.insert_prekeys_batch(&[(7, Bytes::from_static(b"first"))])).unwrap();
        block_on(staged.remove_prekey(7)).unwrap();
        block_on(staged.put_aux(Namespace::DecryptBuffer, "other", b"committed")).unwrap();
        block_on(staged.commit()).unwrap();
        assert_eq!(block_on(store.load_prekey(7)).unwrap(), None);
        assert_eq!(
            block_on(store.load_aux(Namespace::DecryptBuffer, "other")).unwrap(),
            Some(b"committed".to_vec())
        );
    }

    struct PausedSealer {
        paused: std::sync::atomic::AtomicBool,
        inner: AesGcmSealer,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl Sealer for PausedSealer {
        async fn seal(
            &self,
            aad: &[u8],
            plaintext: &[u8],
        ) -> std::result::Result<Vec<u8>, SealError> {
            if self.paused.load(std::sync::atomic::Ordering::SeqCst) {
                futures::future::pending::<()>().await;
            }
            self.inner.seal(aad, plaintext).await
        }

        async fn open(&self, aad: &[u8], sealed: &[u8]) -> std::result::Result<Vec<u8>, SealError> {
            self.inner.open(aad, sealed).await
        }
    }

    #[test]
    fn cancelling_a_commit_before_submission_keeps_its_pending_writes() {
        use std::future::Future;
        use std::task::{Context, Poll, Waker};
        let records = Arc::new(MemoryRecordStore::new());
        let lease = block_on(records.acquire("cancelled", "node", 0, TTL))
            .unwrap()
            .unwrap();
        let sealer = Arc::new(PausedSealer {
            paused: std::sync::atomic::AtomicBool::new(true),
            inner: AesGcmSealer::new(&[3; 32]),
        });
        let store = RecordSignalStore::new(records, sealer.clone(), lease.fence);
        let staged = store.staged();
        block_on(staged.save_device(b"pending-private-state")).unwrap();
        let mut commit = Box::pin(staged.commit());
        assert!(matches!(
            commit
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        drop(commit);
        assert_eq!(
            block_on(staged.load_device()).unwrap(),
            Some(b"pending-private-state".to_vec())
        );
        assert_eq!(block_on(store.load_device()).unwrap(), None);
        sealer
            .paused
            .store(false, std::sync::atomic::Ordering::SeqCst);
        block_on(staged.commit()).unwrap();
        assert_eq!(
            block_on(store.load_device()).unwrap(),
            Some(b"pending-private-state".to_vec())
        );
    }

    #[test]
    fn device_blob_round_trips_sealed_and_fenced() {
        let (records, store) = setup();
        assert_eq!(block_on(store.load_device()).unwrap(), None);
        block_on(store.save_device(b"identity-private")).unwrap();
        assert_eq!(
            block_on(store.load_device()).unwrap().as_deref(),
            Some(&b"identity-private"[..])
        );
        let raw = block_on(records.get("device-1", Namespace::Device, "local"))
            .unwrap()
            .unwrap();
        assert!(!raw.windows(16).any(|w| w == b"identity-private"));
    }

    #[test]
    fn staged_writes_stay_private_until_commit() {
        let (records, store) = setup();
        block_on(store.put_session("keep@s.whatsapp.net", b"old")).unwrap();
        block_on(store.store_prekey(5, b"five", false)).unwrap();

        let staged = store.staged();
        block_on(staged.put_session("keep@s.whatsapp.net", b"new")).unwrap();
        block_on(staged.put_session("fresh@s.whatsapp.net", b"s")).unwrap();
        block_on(staged.remove_prekey(5)).unwrap();
        block_on(staged.put_aux(Namespace::DecryptBuffer, "h1", b"result")).unwrap();

        // The view reads its own writes.
        assert_eq!(
            block_on(staged.get_session("keep@s.whatsapp.net"))
                .unwrap()
                .as_deref(),
            Some(&b"new"[..])
        );
        assert_eq!(block_on(staged.load_prekey(5)).unwrap(), None);
        assert_eq!(block_on(staged.get_max_prekey_id()).unwrap(), 0);
        assert!(block_on(staged.has_signal_state_for_user("fresh")).unwrap());

        // Nothing reached the store yet.
        assert_eq!(
            block_on(store.get_session("keep@s.whatsapp.net"))
                .unwrap()
                .as_deref(),
            Some(&b"old"[..])
        );
        assert!(
            block_on(records.get("device-1", Namespace::DecryptBuffer, "h1"))
                .unwrap()
                .is_none()
        );

        block_on(staged.commit()).unwrap();
        assert_eq!(
            block_on(store.get_session("keep@s.whatsapp.net"))
                .unwrap()
                .as_deref(),
            Some(&b"new"[..])
        );
        assert_eq!(block_on(store.load_prekey(5)).unwrap(), None);
        assert_eq!(
            block_on(store.load_aux(Namespace::DecryptBuffer, "h1"))
                .unwrap()
                .as_deref(),
            Some(&b"result"[..])
        );
    }

    #[test]
    fn a_stale_staged_commit_applies_nothing() {
        let (records, store) = setup();
        let staged = store.staged();
        block_on(staged.put_session("a@s.whatsapp.net", b"s")).unwrap();
        block_on(records.acquire("device-1", "node-b", TTL, TTL))
            .unwrap()
            .unwrap();
        let err = block_on(staged.commit()).unwrap_err();
        assert!(is_fence_lost(&err));
        assert!(
            block_on(records.get("device-1", Namespace::Session, "a@s.whatsapp.net"))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn signal_namespaces_are_not_reachable_through_the_aux_api() {
        let (_, store) = setup();
        assert!(block_on(store.put_aux(Namespace::Session, "x", b"v")).is_err());
        assert!(block_on(store.load_aux(Namespace::Identity, "x")).is_err());
        block_on(store.put_aux(Namespace::SentMessage, "chat\0id", b"m")).unwrap();
        assert_eq!(
            block_on(store.scan_aux(Namespace::SentMessage, "chat", None)).unwrap(),
            vec!["chat\0id".to_owned()]
        );
    }

    #[test]
    fn slots_with_shifted_separators_get_distinct_aad() {
        let records = Arc::new(MemoryRecordStore::new());
        let sealer = Arc::new(AesGcmSealer::new(&[3; 32]));
        let fence = |scope: &str| Fence {
            scope: Arc::from(scope),
            generation: 1,
        };
        let a = RecordSignalStore::new(records.clone(), sealer.clone(), fence("a"));
        let b = RecordSignalStore::new(records, sealer, fence("a\0session\0b"));
        assert_ne!(
            a.aad(Namespace::Session, "b\0session\0c"),
            b.aad(Namespace::Session, "c")
        );
    }

    #[test]
    fn values_are_sealed_at_rest() {
        let (records, store) = setup();
        block_on(store.put_session("alice@s.whatsapp.net", b"ratchet-state")).unwrap();
        let raw = block_on(records.get("device-1", Namespace::Session, "alice@s.whatsapp.net"))
            .unwrap()
            .unwrap();
        assert!(
            !raw.windows(b"ratchet-state".len())
                .any(|w| w == b"ratchet-state"),
            "plaintext reached the record store"
        );
        assert_eq!(
            block_on(store.get_session("alice@s.whatsapp.net"))
                .unwrap()
                .as_deref(),
            Some(&b"ratchet-state"[..])
        );
    }

    #[test]
    fn a_record_copied_to_another_address_fails_to_open() {
        let (records, store) = setup();
        block_on(store.put_session("alice@s.whatsapp.net", b"alice-state")).unwrap();
        let raw = block_on(records.get("device-1", Namespace::Session, "alice@s.whatsapp.net"))
            .unwrap()
            .unwrap();
        block_on(records.write(
            store.fence(),
            &[WriteOp::Put {
                ns: Namespace::Session,
                key: "mallory@s.whatsapp.net".into(),
                value: raw,
            }],
        ))
        .unwrap();
        assert!(block_on(store.get_session("mallory@s.whatsapp.net")).is_err());
    }

    #[test]
    fn a_superseded_node_cannot_write() {
        let (records, stale) = setup();
        let takeover = block_on(records.acquire("device-1", "node-b", TTL, TTL))
            .unwrap()
            .unwrap();
        let fresh = RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&[3; 32])),
            takeover.fence,
        );
        let err = block_on(stale.put_session("alice@s.whatsapp.net", b"old")).unwrap_err();
        assert!(is_fence_lost(&err));
        block_on(fresh.put_session("alice@s.whatsapp.net", b"new")).unwrap();
        assert_eq!(
            block_on(fresh.get_session("alice@s.whatsapp.net"))
                .unwrap()
                .as_deref(),
            Some(&b"new"[..])
        );
    }

    #[test]
    fn identities_round_trip_and_reject_bad_lengths() {
        let (_, store) = setup();
        block_on(store.put_identity("bob@s.whatsapp.net", [9; 32])).unwrap();
        assert_eq!(
            block_on(store.load_identity("bob@s.whatsapp.net")).unwrap(),
            Some([9; 32])
        );
        assert_eq!(
            block_on(store.load_identity("nobody@s.whatsapp.net")).unwrap(),
            None
        );
    }

    #[test]
    fn prekeys_keep_numeric_order_and_never_resurrect() {
        let (_, store) = setup();
        let keys = vec![
            (9, Bytes::from_static(b"nine")),
            (10, Bytes::from_static(b"ten")),
            (100, Bytes::from_static(b"hundred")),
        ];
        block_on(store.store_prekeys_batch(&keys, false)).unwrap();
        assert_eq!(block_on(store.uploaded_prekey_count()).unwrap(), 0);
        assert_eq!(block_on(store.get_max_prekey_id()).unwrap(), 100);

        block_on(store.remove_prekey(10)).unwrap();
        block_on(store.mark_prekeys_uploaded(&[9, 10])).unwrap();
        assert_eq!(block_on(store.uploaded_prekey_count()).unwrap(), 1);
        assert_eq!(block_on(store.load_prekey(10)).unwrap(), None);
        assert_eq!(
            block_on(store.load_prekey(9)).unwrap().as_deref(),
            Some(&b"nine"[..])
        );

        let mut loaded = block_on(store.load_prekeys_batch(&[9, 10, 100])).unwrap();
        loaded.sort_by_key(|(id, _)| *id);
        assert_eq!(
            loaded,
            vec![
                (9, Bytes::from_static(b"nine")),
                (100, Bytes::from_static(b"hundred"))
            ]
        );
    }

    #[test]
    fn signed_prekeys_list_in_order() {
        let (_, store) = setup();
        block_on(store.store_signed_prekey(12, b"twelve")).unwrap();
        block_on(store.store_signed_prekey(2, b"two")).unwrap();
        assert_eq!(
            block_on(store.load_all_signed_prekeys()).unwrap(),
            vec![(2, b"two".to_vec()), (12, b"twelve".to_vec())]
        );
        block_on(store.remove_signed_prekey(2)).unwrap();
        assert_eq!(block_on(store.load_signed_prekey(2)).unwrap(), None);
    }

    #[test]
    fn signal_state_lookup_matches_whole_user_ids() {
        let (_, store) = setup();
        block_on(store.put_session("123:4@s.whatsapp.net", b"s")).unwrap();
        assert!(block_on(store.has_signal_state_for_user("123")).unwrap());
        assert!(!block_on(store.has_signal_state_for_user("12")).unwrap());
        assert!(!block_on(store.has_signal_state_for_user("1234")).unwrap());
    }

    #[test]
    fn batches_and_sender_keys_round_trip() {
        let (_, store) = setup();
        let a: Arc<str> = Arc::from("a@s.whatsapp.net");
        let b: Arc<str> = Arc::from("b@s.whatsapp.net");
        block_on(store.put_sessions_batch(&[
            (a.clone(), Bytes::from_static(b"sa")),
            (b.clone(), Bytes::from_static(b"sb")),
        ]))
        .unwrap();
        let mut found = block_on(store.get_sessions_batch(&[a.clone(), b.clone()])).unwrap();
        found.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(found.len(), 2);
        block_on(store.delete_sessions_batch(std::slice::from_ref(&a))).unwrap();
        assert!(!block_on(store.has_session(&a)).unwrap());
        assert!(block_on(store.has_session(&b)).unwrap());

        block_on(store.put_sender_key("group:me", b"sk")).unwrap();
        assert_eq!(
            block_on(store.get_sender_key("group:me")).unwrap(),
            Some(b"sk".to_vec())
        );
        block_on(store.delete_sender_key("group:me")).unwrap();
        assert_eq!(block_on(store.get_sender_key("group:me")).unwrap(), None);
    }
}
