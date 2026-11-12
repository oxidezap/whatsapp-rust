// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Public upload retries recover from the sealed store, not process memory.
use async_trait::async_trait;
use futures::channel::oneshot;
use futures::executor::block_on;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU32, AtomicUsize, Ordering},
};
use wacore::store::{
    error::{Result as StoreResult, StoreError},
    traits::SignalStore,
};
use wacore_recordstore::{
    AesGcmSealer, Fence, LeaseStore, MemoryRecordStore, Namespace, RecordSignalStore, RecordStore,
    SealError, Sealer, WriteOp,
};
use wacore_signal_ops::{DeviceKeys, SignalOps};

mod common;
mod runtime;
use runtime::InlineRuntime;

const SCOPE: &str = "synthetic-upload-device";
const PENDING: &str = "prekey-upload-v1";
const MAX: u32 = 0x00ff_ffff;
type ReadGate = (oneshot::Sender<()>, oneshot::Receiver<()>);

#[derive(Default)]
struct Records {
    inner: MemoryRecordStore,
    fail: AtomicUsize,
    consume: AtomicU32,
    batches: Mutex<Vec<Vec<WriteOp>>>,
    reads: AtomicUsize,
    pending_read_gate: Mutex<Option<ReadGate>>,
}
#[async_trait]
impl RecordStore for Records {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> StoreResult<Option<Vec<u8>>> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let value = self.inner.get(scope, ns, key).await?;
        let gate = if ns == Namespace::Device && key == PENDING {
            self.pending_read_gate.lock().unwrap().take()
        } else {
            None
        };
        if let Some((entered, resume)) = gate {
            entered.send(()).unwrap();
            resume.await.unwrap();
        }
        Ok(value)
    }
    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> StoreResult<Vec<String>> {
        self.inner.scan_keys(scope, ns, prefix, limit).await
    }
    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> StoreResult<()> {
        self.batches.lock().unwrap().push(ops.to_vec());
        let failure = self.fail.swap(0, Ordering::SeqCst);
        let fails = ops.iter().any(|op| match (failure, op) {
            (
                1,
                WriteOp::Put {
                    ns: Namespace::Device,
                    key,
                    ..
                },
            ) => key == "local",
            (
                2,
                WriteOp::Insert {
                    ns: Namespace::PreKey,
                    ..
                },
            ) => true,
            (
                3,
                WriteOp::Insert {
                    ns: Namespace::Device,
                    key,
                    ..
                },
            ) => key == PENDING,
            (
                4,
                WriteOp::Delete {
                    ns: Namespace::Device,
                    key,
                },
            ) => key == PENDING,
            (
                5,
                WriteOp::Put {
                    ns: Namespace::Device,
                    key,
                    ..
                },
            ) => key == "prekey-upload-ack-v1",
            _ => false,
        });
        if fails {
            return Err(StoreError::Validation(
                "synthetic atomic commit failure".into(),
            ));
        }
        // Mode 6: a prekey batch commits, but its response is lost.
        let lose_response = failure == 6
            && ops.iter().any(|op| {
                matches!(
                    op,
                    WriteOp::Insert {
                        ns: Namespace::PreKey,
                        ..
                    }
                )
            });
        // Mode 7: the acknowledgment commits, but its response is lost.
        let lose_ack = failure == 7
            && ops.iter().any(|op| {
                matches!(
                    op,
                    WriteOp::Put {
                        ns: Namespace::Device,
                        key,
                        ..
                    } if key == "prekey-upload-ack-v1"
                )
            });
        let consumed = self.consume.swap(0, Ordering::SeqCst);
        if consumed != 0
            && ops.iter().any(|op| {
                matches!(
                    op,
                    WriteOp::Update {
                        ns: Namespace::PreKey,
                        ..
                    }
                )
            })
        {
            self.inner
                .write(
                    fence,
                    &[WriteOp::Delete {
                        ns: Namespace::PreKey,
                        key: format!("{consumed:010}"),
                    }],
                )
                .await?;
        }
        self.inner.write(fence, ops).await?;
        if lose_response || lose_ack {
            return Err(StoreError::Validation("synthetic lost response".into()));
        }
        Ok(())
    }
}

struct Sealing {
    inner: AesGcmSealer,
    fail_after: AtomicUsize,
}
#[async_trait]
impl Sealer for Sealing {
    async fn seal(&self, aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, SealError> {
        let remaining = self.fail_after.load(Ordering::SeqCst);
        if remaining != 0 && self.fail_after.fetch_sub(1, Ordering::SeqCst) == 1 {
            return Err(SealError::Backend("synthetic sealer failure".into()));
        }
        self.inner.seal(aad, plain).await
    }
    async fn open(&self, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, SealError> {
        self.inner.open(aad, sealed).await
    }
}
type Store = RecordSignalStore<Records, Sealing>;
struct Fixture {
    records: Arc<Records>,
    sealer: Arc<Sealing>,
    store: Arc<Store>,
    ops: SignalOps<Store>,
}
impl Fixture {
    async fn new() -> Self {
        let records = Arc::new(Records::default());
        let sealer = Arc::new(Sealing {
            inner: AesGcmSealer::new(&[42; 32]),
            fail_after: AtomicUsize::new(0),
        });
        let lease = records
            .inner
            .acquire(SCOPE, "holder-a", 0, 60_000)
            .await
            .unwrap()
            .unwrap();
        let store = Arc::new(RecordSignalStore::new(
            records.clone(),
            sealer.clone(),
            lease.fence,
        ));
        let ops = SignalOps::create(store.clone()).await.unwrap();
        Self {
            records,
            sealer,
            store,
            ops,
        }
    }
    async fn counter(&self) -> u32 {
        DeviceKeys::decode(&self.store.load_device().await.unwrap().unwrap())
            .unwrap()
            .next_prekey_id
    }
    async fn set_counter(&self, counter: u32) {
        let mut keys =
            DeviceKeys::decode(&self.store.load_device().await.unwrap().unwrap()).unwrap();
        keys.next_prekey_id = counter;
        self.store.save_device(&keys.encode()).await.unwrap();
    }
    async fn uploaded(&self, id: u32) -> Option<bool> {
        let key = format!("{id:010}");
        let sealed = self
            .records
            .get(SCOPE, Namespace::PreKey, &key)
            .await
            .unwrap()?;
        let mut aad = Vec::new();
        for part in [
            b"wacore-recordstore/v1".as_slice(),
            SCOPE.as_bytes(),
            b"prekey",
            key.as_bytes(),
        ] {
            aad.extend_from_slice(&(part.len() as u32).to_be_bytes());
            aad.extend_from_slice(part);
        }
        Some(self.sealer.open(&aad, &sealed).await.unwrap()[0] == 1)
    }
}

#[test]
fn legacy_mark_is_fresh_only_and_rejects_all_original_pending_members_atomically() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        let fresh = f.ops.generate_prekeys(&InlineRuntime, 1).await.unwrap()[0].id;
        for ids in [vec![1], vec![fresh, 2]] {
            let writes = f.records.batches.lock().unwrap().len();
            assert!(matches!(
                f.ops.mark_prekeys_uploaded(&ids).await,
                Err(wacore_signal_ops::OpsError::InvalidInput(_))
            ));
            assert_eq!(f.records.batches.lock().unwrap().len(), writes);
            assert_eq!(f.uploaded(fresh).await, Some(false));
            assert_eq!(f.uploaded(2).await, Some(false));
        }
        f.store.remove_prekey(1).await.unwrap();
        let recovered = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        assert_eq!(recovered.pre_keys, batch.pre_keys[1..]);
        assert!(f.ops.mark_prekeys_uploaded(&[fresh, 1]).await.is_err());
        assert_eq!(f.uploaded(1).await, None);
        assert_eq!(f.uploaded(fresh).await, Some(false));
        f.ops.mark_prekeys_uploaded(&[fresh]).await.unwrap();
        assert_eq!(f.uploaded(fresh).await, Some(true));
        assert_eq!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap(),
            recovered
        );
        f.ops
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .unwrap();
        assert_eq!(f.uploaded(1).await, None);
        assert_eq!(f.uploaded(2).await, Some(true));
    });
}

#[test]
fn legacy_mark_refuses_corrupt_pending_metadata_and_stale_fences() {
    block_on(async {
        let f = Fixture::new().await;
        let fresh = f.ops.generate_prekeys(&InlineRuntime, 1).await.unwrap()[0].id;
        f.store
            .put_aux(Namespace::Device, PENDING, b"invalid-batch")
            .await
            .unwrap();
        assert!(f.ops.mark_prekeys_uploaded(&[fresh]).await.is_err());
        assert_eq!(f.uploaded(fresh).await, Some(false));
        f.store
            .delete_aux(Namespace::Device, &[PENDING])
            .await
            .unwrap();
        f.records
            .inner
            .acquire(SCOPE, "holder-b", 60_001, 60_000)
            .await
            .unwrap()
            .unwrap();
        assert!(
            f.ops
                .mark_prekeys_uploaded(&[fresh])
                .await
                .unwrap_err()
                .is_fence_lost()
        );
        assert_eq!(f.uploaded(fresh).await, Some(false));
    });
}

#[test]
fn legacy_mark_and_prepare_share_the_device_mutex() {
    block_on(async {
        let f = Fixture::new().await;
        let (entered, observed) = oneshot::channel();
        let (resume, wait) = oneshot::channel();
        *f.records.pending_read_gate.lock().unwrap() = Some((entered, wait));
        // The mark reads no pending batch, then pauses before its flag write.
        let mark = f.ops.mark_prekeys_uploaded(&[1]);
        futures::pin_mut!(mark);
        assert!(futures::poll!(&mut mark).is_pending());
        observed.await.unwrap();
        let prepare = f.ops.prepare_prekey_upload(&InlineRuntime, 1);
        futures::pin_mut!(prepare);
        assert!(futures::poll!(&mut prepare).is_pending());
        assert!(f.store.load_prekey(1).await.unwrap().is_none());
        resume.send(()).unwrap();
        let (marked, batch) = futures::join!(mark, prepare);
        marked.unwrap();
        assert_eq!(batch.unwrap().pre_keys[0].id, 1);
        // Prepare could not allocate between the guard read and legacy mark.
        assert_eq!(f.uploaded(1).await, Some(false));
    });
}

#[test]
fn reopen_reuses_the_batch_and_repeated_requests_do_not_allocate_more() {
    block_on(async {
        let f = Fixture::new().await;
        let first = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 3)
            .await
            .unwrap();
        assert_eq!(first.pre_keys.len(), 3);
        for _ in 0..20 {
            assert_eq!(
                f.ops
                    .prepare_prekey_upload(&InlineRuntime, 812)
                    .await
                    .unwrap(),
                first
            );
        }
        let reopened = SignalOps::open(f.store.clone()).await.unwrap();
        assert_eq!(
            reopened
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap(),
            first
        );
        assert_eq!(f.counter().await, 4);
        assert_eq!(
            f.records
                .scan_keys(SCOPE, Namespace::PreKey, "", None)
                .await
                .unwrap()
                .len(),
            3
        );
        let ids: Vec<_> = first.pre_keys.iter().map(|key| key.id).collect();
        reopened
            .acknowledge_prekey_upload(&first.batch_id, &ids)
            .await
            .unwrap();
        for id in ids {
            assert_eq!(f.uploaded(id).await, Some(true));
        }
        reopened
            .acknowledge_prekey_upload(&first.batch_id, &[1, 2, 3])
            .await
            .unwrap();
        let next = reopened
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        assert_ne!(next.batch_id, first.batch_id);
        assert_eq!(
            next.pre_keys.iter().map(|key| key.id).collect::<Vec<_>>(),
            [4, 5]
        );
    });
}

#[test]
fn counter_private_keys_and_pending_batch_fail_together_without_exposure() {
    block_on(async {
        for failure in 1..=3 {
            let f = Fixture::new().await;
            let before = f.store.load_device().await.unwrap();
            f.records.fail.store(failure, Ordering::SeqCst);
            assert!(
                f.ops
                    .prepare_prekey_upload(&InlineRuntime, 4)
                    .await
                    .is_err()
            );
            assert_eq!(f.store.load_device().await.unwrap(), before);
            assert!(
                f.store
                    .load_aux(Namespace::Device, PENDING)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                f.records
                    .scan_keys(SCOPE, Namespace::PreKey, "", None)
                    .await
                    .unwrap()
                    .is_empty()
            );
            let failed = f.records.batches.lock().unwrap().last().unwrap().clone();
            assert_eq!(
                failed
                    .iter()
                    .filter(|op| matches!(
                        op,
                        WriteOp::Insert {
                            ns: Namespace::PreKey,
                            ..
                        }
                    ))
                    .count(),
                4
            );
            assert!(failed.iter().any(
                |op| matches!(op, WriteOp::Put { ns: Namespace::Device, key, .. } if key == "local")
            ));
            assert!(failed.iter().any(|op| matches!(op, WriteOp::Insert { ns: Namespace::Device, key, .. } if key == PENDING)));
            let reopened = SignalOps::open(f.store.clone()).await.unwrap();
            assert_eq!(
                reopened
                    .prepare_prekey_upload(&InlineRuntime, 1)
                    .await
                    .unwrap()
                    .pre_keys[0]
                    .id,
                1
            );
        }
        let f = Fixture::new().await;
        f.sealer.fail_after.store(2, Ordering::SeqCst);
        assert!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .is_err()
        );
        assert_eq!(f.counter().await, 1);
        assert!(f.store.load_prekey(1).await.unwrap().is_none());
        assert!(
            f.store
                .load_aux(Namespace::Device, PENDING)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .unwrap()
                .pre_keys[0]
                .id,
            1
        );
    });
}

#[tokio::test]
async fn concurrent_prepare_is_bounded_and_retry_generation_stays_fresh() {
    let f = Fixture::new().await;
    let batches =
        futures::future::join_all((0..24).map(|_| f.ops.prepare_prekey_upload(&InlineRuntime, 8)))
            .await;
    let batch = batches[0].as_ref().unwrap();
    for result in &batches {
        assert_eq!(result.as_ref().unwrap(), batch);
    }
    let retries =
        futures::future::join_all((0..12).map(|_| f.ops.generate_prekeys(&InlineRuntime, 2))).await;
    let ids: std::collections::BTreeSet<_> = retries
        .into_iter()
        .flat_map(|keys| keys.unwrap().into_iter().map(|key| key.id))
        .collect();
    assert_eq!(ids.len(), 24);
    assert!(batch.pre_keys.iter().all(|key| !ids.contains(&key.id)));
    assert_eq!(f.counter().await, 33);
}

#[test]
fn a_prepare_in_progress_holds_back_other_prepares_and_fresh_generation() {
    block_on(async {
        let f = Fixture::new().await;
        let (entered, observed) = oneshot::channel();
        let (resume, wait) = oneshot::channel();
        *f.records.pending_read_gate.lock().unwrap() = Some((entered, wait));
        // The first prepare pauses at its pending-batch read.
        let first = f.ops.prepare_prekey_upload(&InlineRuntime, 4);
        futures::pin_mut!(first);
        assert!(futures::poll!(&mut first).is_pending());
        observed.await.unwrap();
        let second = f.ops.prepare_prekey_upload(&InlineRuntime, 4);
        futures::pin_mut!(second);
        let fresh = f.ops.generate_prekeys(&InlineRuntime, 2);
        futures::pin_mut!(fresh);
        assert!(futures::poll!(&mut second).is_pending());
        assert!(futures::poll!(&mut fresh).is_pending());
        assert_eq!(f.counter().await, 1, "nothing allocated while held back");
        assert!(f.store.load_prekey(1).await.unwrap().is_none());
        resume.send(()).unwrap();
        let (first, second, fresh) = futures::join!(first, second, fresh);
        let (first, second, fresh) = (first.unwrap(), second.unwrap(), fresh.unwrap());
        assert_eq!(first, second, "the second prepare reuses the batch");
        assert!(
            fresh
                .iter()
                .all(|key| first.pre_keys.iter().all(|batch| batch.id != key.id)),
            "fresh keys never reuse batch ids"
        );
        assert_eq!(f.counter().await, 7);
    });
}

#[test]
fn wrap_reserves_consumed_pending_ids_and_late_ack_never_marks_retry_keys() {
    block_on(async {
        let f = Fixture::new().await;
        let old = f.ops.generate_prekeys(&InlineRuntime, 1).await.unwrap();
        let old_record = f.store.load_prekey(old[0].id).await.unwrap();
        f.set_counter(MAX - 1).await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 3)
            .await
            .unwrap();
        let ids: Vec<_> = batch.pre_keys.iter().map(|key| key.id).collect();
        assert_eq!(ids, [MAX - 1, MAX, 2]);
        f.store.remove_prekey(MAX - 1).await.unwrap();
        f.set_counter(MAX - 1).await;
        let fresh = f.ops.generate_prekeys(&InlineRuntime, 3).await.unwrap();
        assert_eq!(
            fresh.iter().map(|key| key.id).collect::<Vec<_>>(),
            [3, 4, 5]
        );
        assert_eq!(f.store.load_prekey(1).await.unwrap(), old_record);
        let filtered = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 3)
            .await
            .unwrap();
        assert_eq!(filtered.batch_id, batch.batch_id);
        assert_eq!(
            filtered
                .pre_keys
                .iter()
                .map(|key| key.id)
                .collect::<Vec<_>>(),
            [MAX, 2]
        );
        f.ops
            .acknowledge_prekey_upload(&batch.batch_id, &ids)
            .await
            .unwrap();
        assert!(f.store.load_prekey(MAX - 1).await.unwrap().is_none());
        for key in fresh {
            assert_eq!(f.uploaded(key.id).await, Some(false));
        }
        assert_eq!(f.uploaded(1).await, Some(false));
        assert_eq!(f.uploaded(MAX).await, Some(true));
        assert_eq!(f.uploaded(2).await, Some(true));
    });
}

#[test]
fn consumed_keys_stay_gone_even_between_ack_read_and_atomic_commit() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 3)
            .await
            .unwrap();
        f.store.remove_prekey(1).await.unwrap();
        let filtered = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 3)
            .await
            .unwrap();
        assert_eq!(
            filtered
                .pre_keys
                .iter()
                .map(|key| key.id)
                .collect::<Vec<_>>(),
            [2, 3]
        );
        f.records.consume.store(2, Ordering::SeqCst);
        f.ops
            .acknowledge_prekey_upload(&batch.batch_id, &[2, 3])
            .await
            .unwrap();
        assert_eq!(f.uploaded(1).await, None);
        assert_eq!(f.uploaded(2).await, None);
        assert_eq!(f.uploaded(3).await, Some(true));
        assert!(
            f.store
                .load_aux(Namespace::Device, PENDING)
                .await
                .unwrap()
                .is_none()
        );
    });
}

#[test]
fn ack_rejects_unknown_duplicate_missing_and_stale_batches_and_recovers_failed_commit() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        for (id, ids) in [
            ("unknown", vec![1, 2]),
            (batch.batch_id.as_str(), vec![1]),
            (batch.batch_id.as_str(), vec![1, 1, 2]),
            (batch.batch_id.as_str(), vec![1, 2, 3]),
        ] {
            assert!(f.ops.acknowledge_prekey_upload(id, &ids).await.is_err());
            assert_eq!(
                f.ops
                    .prepare_prekey_upload(&InlineRuntime, 2)
                    .await
                    .unwrap(),
                batch
            );
            assert_eq!(f.uploaded(1).await, Some(false));
        }
        f.records.fail.store(4, Ordering::SeqCst);
        assert!(
            f.ops
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
                .await
                .is_err()
        );
        assert_eq!(f.uploaded(1).await, Some(false));
        let reopened = SignalOps::open(f.store.clone()).await.unwrap();
        assert_eq!(
            reopened
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .unwrap(),
            batch
        );
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .unwrap();
        let next = reopened
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .unwrap();
        assert!(
            reopened
                .acknowledge_prekey_upload(&batch.batch_id, &[1])
                .await
                .is_err()
        );
        assert_eq!(
            reopened
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap(),
            next
        );
        reopened
            .acknowledge_prekey_upload(&next.batch_id, &[next.pre_keys[0].id])
            .await
            .unwrap();
        assert!(
            reopened
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
                .await
                .is_err()
        );
    });
}

#[test]
fn consumed_batch_retires_and_stale_holder_cannot_prepare_or_ack() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        f.store.remove_prekey(batch.pre_keys[0].id).await.unwrap();
        let replacement = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        assert_ne!(replacement.batch_id, batch.batch_id);
        assert!(
            f.ops
                .acknowledge_prekey_upload(&batch.batch_id, &[1])
                .await
                .is_err()
        );
        let lease = f
            .records
            .inner
            .acquire(SCOPE, "holder-b", 60_001, 60_000)
            .await
            .unwrap()
            .unwrap();
        assert!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap_err()
                .is_fence_lost()
        );
        assert!(
            f.ops
                .acknowledge_prekey_upload(&replacement.batch_id, &[replacement.pre_keys[0].id])
                .await
                .unwrap_err()
                .is_fence_lost()
        );
        assert!(
            f.ops
                .generate_prekeys(&InlineRuntime, 1)
                .await
                .unwrap_err()
                .is_fence_lost()
        );
        let store = Arc::new(RecordSignalStore::new(
            f.records.clone(),
            f.sealer.clone(),
            lease.fence,
        ));
        let new = SignalOps::open(store).await.unwrap();
        assert_eq!(
            new.prepare_prekey_upload(&InlineRuntime, 1).await.unwrap(),
            replacement
        );
    });
}

#[test]
fn corrupted_pending_metadata_or_private_keys_fail_closed_without_replacement() {
    block_on(async {
        for kind in 0..5 {
            let f = Fixture::new().await;
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .unwrap();
            let mut pending = f
                .store
                .load_aux(Namespace::Device, PENDING)
                .await
                .unwrap()
                .unwrap();
            match kind {
                0 => pending[0] = 99,
                1 => {
                    pending.pop();
                }
                2 => pending[19..23].copy_from_slice(&0u32.to_be_bytes()),
                3 => {
                    let entry = pending[19..55].to_vec();
                    pending[55..91].copy_from_slice(&entry);
                }
                _ => pending[23] ^= 1,
            }
            if kind >= 2 {
                use sha2::Digest;
                let data_len = pending.len() - 32;
                let digest = sha2::Sha256::digest(&pending[..data_len]);
                pending[data_len..].copy_from_slice(&digest);
            }
            f.store
                .put_aux(Namespace::Device, PENDING, &pending)
                .await
                .unwrap();
            let before = f.store.load_device().await.unwrap();
            assert!(
                f.ops
                    .prepare_prekey_upload(&InlineRuntime, 2)
                    .await
                    .is_err()
            );
            assert!(f.ops.generate_prekeys(&InlineRuntime, 1).await.is_err());
            assert_eq!(f.store.load_device().await.unwrap(), before);
        }
        let f = Fixture::new().await;
        f.ops
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        f.store
            .store_prekey(1, b"corrupt-record", false)
            .await
            .unwrap();
        assert!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .is_err()
        );
        // Count validation, on a healthy device so no other check can be
        // what refuses.
        let f = Fixture::new().await;
        let count_error = |error: wacore_signal_ops::OpsError| {
            assert!(
                error.to_string().contains("prekey count must be 1 to 812"),
                "{error}"
            )
        };
        for count in [0, 813, u32::MAX] {
            count_error(
                f.ops
                    .prepare_prekey_upload(&InlineRuntime, count)
                    .await
                    .unwrap_err(),
            );
            count_error(
                f.ops
                    .generate_prekeys(&InlineRuntime, count)
                    .await
                    .unwrap_err(),
            );
        }
        assert_eq!(f.counter().await, 1, "a refused count allocates nothing");
        assert!(f.store.load_prekey(1).await.unwrap().is_none());
    });
}

#[test]
fn lost_ack_response_replays_after_restart_without_touching_new_pending_or_crossing_fence() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        f.records.fail.store(5, Ordering::SeqCst);
        assert!(
            f.ops
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
                .await
                .is_err()
        );
        assert_eq!(f.uploaded(1).await, Some(false));
        assert_eq!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .unwrap(),
            batch
        );
        f.ops
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .unwrap();
        let reopened = SignalOps::open(f.store.clone()).await.unwrap();
        let next = reopened
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[2, 1])
            .await
            .unwrap();
        assert_eq!(
            reopened
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap(),
            next
        );
        assert!(
            reopened
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 1, 2])
                .await
                .is_err()
        );
        f.records
            .inner
            .acquire(SCOPE, "holder-b", 60_001, 60_000)
            .await
            .unwrap()
            .unwrap();
        assert!(
            reopened
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
                .await
                .unwrap_err()
                .is_fence_lost()
        );
    });
}

#[test]
fn allocation_work_is_bounded_and_collision_refuses_the_entire_staged_batch() {
    block_on(async {
        let f = Fixture::new().await;
        let occupied: Vec<_> = (1..=4096)
            .map(|id| (id, bytes::Bytes::from_static(b"occupied-private-slot")))
            .collect();
        f.store.store_prekeys_batch(&occupied, false).await.unwrap();
        let before = f.store.load_device().await.unwrap();
        f.records.reads.store(0, Ordering::SeqCst);
        let error = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("4096-ID scan limit"));
        assert!(f.records.reads.load(Ordering::SeqCst) <= 4098);
        assert_eq!(f.store.load_device().await.unwrap(), before);
        assert!(
            f.store
                .load_aux(Namespace::Device, PENDING)
                .await
                .unwrap()
                .is_none()
        );
        let staged = f.store.staged();
        staged
            .insert_prekeys_batch(&[
                (5000, bytes::Bytes::from_static(b"new")),
                (1, bytes::Bytes::from_static(b"replacement")),
            ])
            .await
            .unwrap();
        let mut keys = DeviceKeys::decode(before.as_deref().unwrap()).unwrap();
        keys.next_prekey_id = 5001;
        staged.save_device(&keys.encode()).await.unwrap();
        assert!(wacore_recordstore::is_record_exists(
            &staged.commit().await.unwrap_err()
        ));
        assert_eq!(f.store.load_device().await.unwrap(), before);
        assert!(f.store.load_prekey(5000).await.unwrap().is_none());
        assert_eq!(
            f.store.load_prekey(1).await.unwrap().unwrap(),
            occupied[0].1
        );
    });
}

#[test]
fn actual_signal_consumption_filters_the_recovered_public_batch() {
    block_on(async {
        use wacore::libsignal::protocol::{DeviceId, ProtocolAddress};
        use wacore_signal_ops::RemoteBundle;
        let sender = Fixture::new().await;
        let receiver = Fixture::new().await;
        let batch = receiver
            .ops
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        let device_identity = common::vouch(&sender.ops, &receiver.ops, "222@s.whatsapp.net").await;
        let sender_identity = common::vouch(&receiver.ops, &sender.ops, "111@s.whatsapp.net").await;
        let identity = receiver.ops.public_identity().await;
        let sender_address = ProtocolAddress::new("111@s.whatsapp.net", DeviceId::from(1));
        let receiver_address = ProtocolAddress::new("222@s.whatsapp.net", DeviceId::from(1));
        sender
            .ops
            .establish_session(
                &receiver_address,
                &RemoteBundle {
                    registration_id: identity.registration_id,
                    device_id: 1,
                    identity_key: identity.identity_key,
                    signed_prekey_id: identity.signed_prekey.id,
                    signed_prekey: identity.signed_prekey.public_key,
                    signed_prekey_signature: identity.signed_prekey.signature,
                    prekey: Some((batch.pre_keys[0].id, batch.pre_keys[0].public_key)),
                    device_identity: Some(device_identity),
                },
            )
            .await
            .unwrap();
        let sent = sender
            .ops
            .encrypt(&receiver_address, b"synthetic-upload-roundtrip")
            .await
            .unwrap();
        let received = receiver
            .ops
            .decrypt(
                &sender_address,
                sent.kind,
                &sent.ciphertext,
                Some(&sender_identity),
            )
            .await
            .unwrap();
        assert_eq!(received.plaintext, b"synthetic-upload-roundtrip");
        let reopened = SignalOps::open(receiver.store.clone()).await.unwrap();
        let filtered = reopened
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        assert_eq!(filtered.batch_id, batch.batch_id);
        assert_eq!(filtered.pre_keys, batch.pre_keys[1..]);
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .unwrap();
        assert_eq!(receiver.uploaded(1).await, None);
        assert_eq!(receiver.uploaded(2).await, Some(true));
    });
}

#[test]
fn upload_ceiling_and_corrupt_ack_receipt_are_enforced() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 812)
            .await
            .unwrap();
        assert_eq!(batch.pre_keys.len(), 812);
        f.ops
            .acknowledge_prekey_upload(
                &batch.batch_id,
                &batch.pre_keys.iter().map(|key| key.id).collect::<Vec<_>>(),
            )
            .await
            .unwrap();
        f.store
            .put_aux(
                Namespace::Device,
                "prekey-upload-ack-v1",
                b"corrupt-receipt",
            )
            .await
            .unwrap();
        assert!(
            f.ops
                .acknowledge_prekey_upload(&batch.batch_id, &[1])
                .await
                .is_err()
        );
        assert_eq!(f.counter().await, 813);
    });
}

/// Counts blocking jobs and the thread each ran on.
struct CountingRuntime {
    jobs: std::sync::atomic::AtomicUsize,
}

impl wacore::runtime::Runtime for CountingRuntime {
    fn spawn(
        &self,
        future: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>,
    ) -> wacore::runtime::AbortHandle {
        InlineRuntime.spawn(future)
    }

    fn sleep(
        &self,
        duration: std::time::Duration,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
        InlineRuntime.sleep(duration)
    }

    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>> {
        self.jobs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        InlineRuntime.spawn_blocking(f)
    }

    fn yield_now(&self) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>> {
        None
    }
}

#[test]
fn prekey_generation_runs_as_blocking_work() {
    block_on(async {
        let f = Fixture::new().await;
        let runtime = CountingRuntime {
            jobs: std::sync::atomic::AtomicUsize::new(0),
        };
        let batch = f.ops.prepare_prekey_upload(&runtime, 812).await.unwrap();
        assert_eq!(batch.pre_keys.len(), 812);
        assert_eq!(runtime.jobs.load(std::sync::atomic::Ordering::SeqCst), 1);
        f.ops.generate_prekeys(&runtime, 3).await.unwrap();
        assert_eq!(runtime.jobs.load(std::sync::atomic::Ordering::SeqCst), 2);
    });
}

#[test]
fn a_lost_prekey_commit_response_cannot_rewind_the_counter() {
    block_on(async {
        let f = Fixture::new().await;
        let before = f.counter().await;
        f.records.fail.store(6, Ordering::SeqCst);
        assert!(
            f.ops
                .prepare_prekey_upload(&InlineRuntime, 2)
                .await
                .is_err(),
            "the caller sees the lost response"
        );
        let committed = f.counter().await;
        assert_eq!(committed, before + 2, "the batch did commit");

        // A cached write after the uncertain commit must not write the old
        // counter back.
        f.ops
            .set_own_jids("111:1@s.whatsapp.net", None)
            .await
            .unwrap();
        f.ops.rotate_signed_prekey().await.unwrap();
        assert_eq!(f.counter().await, committed);
    });
}

#[test]
fn an_acknowledgment_that_committed_but_lost_its_response_replays_after_reopening() {
    block_on(async {
        let f = Fixture::new().await;
        let batch = f
            .ops
            .prepare_prekey_upload(&InlineRuntime, 2)
            .await
            .unwrap();
        f.records.fail.store(7, Ordering::SeqCst);
        assert!(
            f.ops
                .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
                .await
                .is_err(),
            "the caller sees the lost response"
        );
        assert_eq!(
            f.uploaded(1).await,
            Some(true),
            "the acknowledgment did commit"
        );
        assert_eq!(f.uploaded(2).await, Some(true));

        // The process restarts: only the sealed store remains, and no live
        // instance is left for the reopen to join.
        let Fixture { store, ops, .. } = f;
        drop(ops);
        let reopened = SignalOps::open(store).await.unwrap();
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .expect("the exact acknowledgment replays");
        let next = reopened
            .prepare_prekey_upload(&InlineRuntime, 1)
            .await
            .unwrap();
        assert_ne!(next.batch_id, batch.batch_id);
        reopened
            .acknowledge_prekey_upload(&batch.batch_id, &[1, 2])
            .await
            .expect("a second replay is accepted too");
        assert_eq!(
            reopened
                .prepare_prekey_upload(&InlineRuntime, 1)
                .await
                .unwrap(),
            next,
            "the replay left the new pending batch unchanged"
        );
    });
}
