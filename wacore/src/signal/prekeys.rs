// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Durable public upload batches, separate from fresh retry prekeys.

use super::*;
use crate::runtime::Runtime;
use crate::store::error::StoreError;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

const PENDING: &str = "prekey-upload-v1";
const LAST_ACK: &str = "prekey-upload-ack-v1";
const MAX_SCAN_IDS: u32 = 4096;
const SCAN_CHUNK: u32 = 128;
const HEADER: usize = 1 + 16 + 2;
const ENTRY: usize = 4 + 32;

pub use crate::signal::types::PreKeyUploadBatch;

fn corrupt() -> OpsError {
    StoreError::Validation("stored prekey upload batch is invalid".into()).into()
}

fn count_valid(count: u32) -> Result<(), OpsError> {
    if !(1..=812).contains(&count) {
        return Err(OpsError::InvalidInput(
            "prekey count must be 1 to 812".into(),
        ));
    }
    Ok(())
}

impl PreKeyUploadBatch {
    fn encode(&self) -> Result<Vec<u8>, OpsError> {
        let id = hex::decode(&self.batch_id).map_err(|_| corrupt())?;
        if id.len() != 16 || self.pre_keys.is_empty() || self.pre_keys.len() > 812 {
            return Err(corrupt());
        }
        let mut out = Vec::with_capacity(HEADER + ENTRY * self.pre_keys.len());
        out.push(1);
        out.extend_from_slice(&id);
        out.extend_from_slice(&(self.pre_keys.len() as u16).to_be_bytes());
        for key in &self.pre_keys {
            out.extend_from_slice(&key.id.to_be_bytes());
            out.extend_from_slice(&key.public_key);
        }
        out.extend_from_slice(&Sha256::digest(&out));
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Self, OpsError> {
        if bytes.len() < HEADER || bytes[0] != 1 {
            return Err(corrupt());
        }
        let count = usize::from(u16::from_be_bytes([bytes[17], bytes[18]]));
        let data_len = HEADER + count * ENTRY;
        if count == 0
            || count > 812
            || bytes.len() != data_len + 32
            || Sha256::digest(&bytes[..data_len])[..] != bytes[data_len..]
        {
            return Err(corrupt());
        }
        let mut seen = BTreeSet::new();
        let mut pre_keys = Vec::with_capacity(count);
        for entry in bytes[HEADER..data_len].chunks_exact(ENTRY) {
            let id = u32::from_be_bytes(entry[..4].try_into().map_err(|_| corrupt())?);
            if !(1..=MAX_PREKEY_ID).contains(&id) || !seen.insert(id) {
                return Err(corrupt());
            }
            pre_keys.push(PublicPreKey {
                id,
                public_key: entry[4..].try_into().map_err(|_| corrupt())?,
            });
        }
        Ok(Self {
            batch_id: hex::encode(&bytes[1..17]),
            pre_keys,
        })
    }
}

impl<S: SignalOperationStore> SignalOps<S> {
    async fn pending_upload(&self) -> Result<Option<PreKeyUploadBatch>, OpsError> {
        self.store
            .load_aux(Namespace::Device, PENDING)
            .await?
            .map(|bytes| PreKeyUploadBatch::decode(&bytes))
            .transpose()
    }

    async fn live_upload_keys(
        &self,
        batch: &PreKeyUploadBatch,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        let ids: Vec<_> = batch.pre_keys.iter().map(|key| key.id).collect();
        let records: BTreeMap<_, _> = self
            .store
            .load_prekeys_batch(&ids)
            .await?
            .into_iter()
            .collect();
        let mut live = Vec::new();
        for key in &batch.pre_keys {
            if let Some(bytes) = records.get(&key.id) {
                let structure =
                    waproto::codec::pre_key_record_decode(bytes).map_err(|_| corrupt())?;
                let record =
                    record_helpers::prekey_structure_to_record(structure).map_err(|_| corrupt())?;
                let pair = record.key_pair().map_err(|_| corrupt())?;
                if u32::from(record.id().map_err(|_| corrupt())?) != key.id
                    || public_bytes(&pair.public_key) != key.public_key
                    || public_bytes(&pair.private_key.public_key().map_err(|_| corrupt())?)
                        != key.public_key
                {
                    return Err(corrupt());
                }
                live.push(key.clone());
            }
        }
        Ok(live)
    }

    /// Recover the outstanding batch after restart, excluding consumed keys.
    /// `count` applies only when no live batch remains. Keep one SignalOps
    /// instance per held scope; its device lock serializes upload and retry work.
    /// New batches accept 1..=812 keys and inspect at most 4096 candidate IDs;
    /// allocation exhaustion at that bound leaves the counter and keys unchanged.
    /// Key generation runs through `runtime`'s blocking pool.
    pub async fn prepare_prekey_upload(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<PreKeyUploadBatch, OpsError> {
        count_valid(count)?;
        let held = self.lock_device().await?;
        held.run(async {
            if let Some(batch) = self.pending_upload().await? {
                let live = self.live_upload_keys(&batch).await?;
                let staged = self.store.staged();
                if !live.is_empty() {
                    // The backend must refuse a superseded writer before any
                    // public batch is returned, including a recovered one.
                    staged
                        .put_aux(Namespace::Device, PENDING, &batch.encode()?)
                        .await?;
                    staged.commit().await?;
                    return Ok(PreKeyUploadBatch {
                        pre_keys: live,
                        ..batch
                    });
                }
                staged.delete_aux(Namespace::Device, &[PENDING]).await?;
                staged.commit().await?;
            }
            let mut batch_id = [0; 16];
            rng().fill_bytes(&mut batch_id);
            let batch_id = hex::encode(batch_id);
            let public = self
                .allocate_prekeys(runtime, count, &BTreeSet::new(), Some(&batch_id))
                .await?;
            Ok(PreKeyUploadBatch {
                batch_id,
                pre_keys: public,
            })
        })
        .await
    }

    /// Acknowledge exactly the uploaded IDs. They must be unique members of
    /// this batch and cover every surviving private key. Consumed members may
    /// be included so an in-flight upload can still be acknowledged safely.
    /// The most recent exact acknowledgment remains replayable after restart.
    pub async fn acknowledge_prekey_upload(
        &self,
        batch_id: &str,
        exact_ids: &[u32],
    ) -> Result<(), OpsError> {
        let held = self.lock_device().await?;
        held.run(async {
            let invalid = || {
                OpsError::InvalidInput(
                    "prekey upload batch or acknowledgment does not match".into(),
                )
            };
            let ids: BTreeSet<_> = exact_ids.iter().copied().collect();
            if ids.is_empty() || ids.len() != exact_ids.len() {
                return Err(invalid());
            }
            if let Some(bytes) = self.store.load_aux(Namespace::Device, LAST_ACK).await? {
                let acknowledged = PreKeyUploadBatch::decode(&bytes)?;
                if acknowledged.batch_id == batch_id {
                    if acknowledged
                        .pre_keys
                        .iter()
                        .map(|key| key.id)
                        .collect::<BTreeSet<_>>()
                        != ids
                    {
                        return Err(invalid());
                    }
                    let staged = self.store.staged();
                    staged.put_aux(Namespace::Device, LAST_ACK, &bytes).await?;
                    staged.commit().await?;
                    return Ok(());
                }
            }
            let batch = self.pending_upload().await?.ok_or_else(invalid)?;
            if batch.batch_id != batch_id {
                return Err(invalid());
            }
            let members: BTreeSet<_> = batch.pre_keys.iter().map(|key| key.id).collect();
            let live = self.live_upload_keys(&batch).await?;
            if ids.len() != exact_ids.len()
                || !ids.is_subset(&members)
                || live.iter().any(|key| !ids.contains(&key.id))
            {
                return Err(invalid());
            }
            let staged = self.store.staged();
            staged.mark_prekeys_uploaded(exact_ids).await?;
            let receipt = PreKeyUploadBatch {
                batch_id: batch.batch_id,
                pre_keys: batch
                    .pre_keys
                    .into_iter()
                    .filter(|key| ids.contains(&key.id))
                    .collect(),
            };
            staged
                .put_aux(Namespace::Device, LAST_ACK, &receipt.encode()?)
                .await?;
            staged.delete_aux(Namespace::Device, &[PENDING]).await?;
            staged.commit().await?;
            Ok(())
        })
        .await
    }

    pub(super) async fn generate_fresh_prekeys(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        count_valid(count)?;
        let held = self.lock_device().await?;
        held.run(async {
            let reserved = if let Some(batch) = self.pending_upload().await? {
                self.live_upload_keys(&batch).await?;
                batch.pre_keys.into_iter().map(|key| key.id).collect()
            } else {
                BTreeSet::new()
            };
            self.allocate_prekeys(runtime, count, &reserved, None).await
        })
        .await
    }

    pub(super) async fn mark_fresh_prekeys_uploaded(&self, ids: &[u32]) -> Result<(), OpsError> {
        let held = self.lock_device().await?;
        held.run(async {
            if let Some(batch) = self.pending_upload().await? {
                let reserved: BTreeSet<_> = batch.pre_keys.iter().map(|key| key.id).collect();
                // Original membership, including consumed keys, remains batch
                // authority until ACK or retirement. Check before any flag write.
                if ids.iter().any(|id| reserved.contains(id)) {
                    return Err(OpsError::InvalidInput(
                        "pending upload members require batch acknowledgment".into(),
                    ));
                }
            }
            Ok(self.store.mark_prekeys_uploaded(ids).await?)
        })
        .await
    }

    async fn allocate_prekeys(
        &self,
        runtime: &dyn Runtime,
        count: u32,
        reserved: &BTreeSet<u32>,
        batch_id: Option<&str>,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        let mut keys = self.durable_keys().await?;
        if !(1..=MAX_PREKEY_ID).contains(&keys.next_prekey_id) {
            return Err(corrupt());
        }
        let mut ids = Vec::with_capacity(count as usize);
        let mut next = keys.next_prekey_id;
        for _ in 0..MAX_SCAN_IDS / SCAN_CHUNK {
            let mut candidates = Vec::with_capacity(SCAN_CHUNK as usize);
            for _ in 0..SCAN_CHUNK {
                let id = next;
                next = if next == MAX_PREKEY_ID { 1 } else { next + 1 };
                if !reserved.contains(&id) {
                    candidates.push(id);
                }
            }
            let occupied: BTreeSet<_> = self
                .store
                .load_prekeys_batch(&candidates)
                .await?
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            for id in candidates {
                if !occupied.contains(&id) {
                    ids.push(id);
                    if ids.len() == count as usize {
                        next = if id == MAX_PREKEY_ID { 1 } else { id + 1 };
                        break;
                    }
                }
            }
            if ids.len() == count as usize {
                break;
            }
        }
        if ids.len() != count as usize {
            return Err(OpsError::InvalidInput(
                "prekey allocation exceeded the 4096-ID scan limit; no keys were created".into(),
            ));
        }
        keys.next_prekey_id = next;
        // Up to 812 key generations and encodes: off the async executor, as
        // the client's own prekey upload does.
        let (records, public) = crate::runtime::blocking(runtime, move || {
            let mut random = rng();
            let mut records = Vec::with_capacity(ids.len());
            let mut public = Vec::with_capacity(ids.len());
            for id in ids {
                let pair = KeyPair::generate(&mut random);
                let record = record_helpers::new_pre_key_record(id, &pair);
                records.push((
                    id,
                    bytes::Bytes::from(waproto::codec::pre_key_record_to_vec(&record)),
                ));
                public.push(PublicPreKey {
                    id,
                    public_key: public_bytes(&pair.public_key),
                });
            }
            (records, public)
        })
        .await;
        let staged = self.store.staged();
        staged.insert_prekeys_batch(&records).await?;
        staged.save_device_keys(&keys.encode()?).await?;
        if let Some(batch_id) = batch_id {
            let batch = PreKeyUploadBatch {
                batch_id: batch_id.to_owned(),
                pre_keys: public.clone(),
            };
            staged
                .insert_aux(Namespace::Device, PENDING, &batch.encode()?)
                .await?;
        }
        if let Err(error) = staged.commit().await {
            self.refresh_keys().await;
            return Err(error.into());
        }
        *self.authority.keys.write().await = Arc::new(keys);
        Ok(public)
    }
}
