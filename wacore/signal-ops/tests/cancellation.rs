// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Submitted storage writes may outlive the caller that awaited them.
#![cfg(not(target_arch = "wasm32"))]

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::libsignal::protocol::{DeviceId, ProtocolAddress};
use wacore::store::error::{Result, StoreError};
use wacore_recordstore::{
    AesGcmSealer, Fence, LeaseStore, MemoryRecordStore, Namespace, RecordSignalStore, RecordStore,
    WriteOp, is_fence_lost,
};
use wacore_signal_ops::{RemoteBundle, SignalOps};

mod runtime;
use runtime::InlineRuntime;

const TTL: u64 = 60_000;
const DATA_KEY: [u8; 32] = [42; 32];

struct DelayedRecords {
    inner: Arc<MemoryRecordStore>,
    pause: AtomicBool,
    submitted: tokio::sync::Notify,
    resume: Arc<tokio::sync::Notify>,
    finished: Arc<tokio::sync::Notify>,
    committed: Arc<AtomicBool>,
    refused: Arc<AtomicBool>,
}

impl DelayedRecords {
    fn new() -> Self {
        Self {
            inner: Arc::new(MemoryRecordStore::new()),
            pause: AtomicBool::new(false),
            submitted: tokio::sync::Notify::new(),
            resume: Arc::new(tokio::sync::Notify::new()),
            finished: Arc::new(tokio::sync::Notify::new()),
            committed: Arc::new(AtomicBool::new(false)),
            refused: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait]
impl RecordStore for DelayedRecords {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        self.inner.get(scope, ns, key).await
    }

    async fn get_many(
        &self,
        scope: &str,
        ns: Namespace,
        keys: &[&str],
    ) -> Result<Vec<(String, Vec<u8>)>> {
        self.inner.get_many(scope, ns, keys).await
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        self.inner.scan_keys(scope, ns, prefix, limit).await
    }

    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()> {
        let changes_session = ops.iter().any(|op| {
            matches!(
                op,
                WriteOp::Put {
                    ns: Namespace::Session,
                    ..
                }
            )
        });
        if !changes_session || !self.pause.swap(false, Ordering::SeqCst) {
            return self.inner.write(fence, ops).await;
        }
        let (send, receive) = tokio::sync::oneshot::channel();
        let inner = self.inner.clone();
        let fence = fence.clone();
        let ops = ops.to_vec();
        let resume = self.resume.clone();
        let finished = self.finished.clone();
        let committed = self.committed.clone();
        let refused = self.refused.clone();
        // Model a submitted database operation: dropping its receiver does
        // not cancel the work already handed to the database worker.
        tokio::spawn(async move {
            resume.notified().await;
            let result = inner.write(&fence, &ops).await;
            committed.store(result.is_ok(), Ordering::SeqCst);
            refused.store(result.as_ref().is_err_and(is_fence_lost), Ordering::SeqCst);
            let _ = send.send(result);
            finished.notify_one();
        });
        self.submitted.notify_one();
        receive
            .await
            .map_err(|_| StoreError::Validation("fixture write worker stopped".into()))?
    }
}

async fn cancellation_case(release_before_takeover: bool) {
    let records = Arc::new(DelayedRecords::new());
    let lease = records
        .inner
        .acquire("alice", "first", 0, TTL)
        .await
        .unwrap()
        .unwrap();
    let alice = Arc::new(
        SignalOps::create(Arc::new(RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&DATA_KEY)),
            lease.fence,
        )))
        .await
        .unwrap(),
    );
    let bob_records = Arc::new(MemoryRecordStore::new());
    let bob_lease = bob_records
        .acquire("bob", "holder", 0, TTL)
        .await
        .unwrap()
        .unwrap();
    let bob = SignalOps::create(Arc::new(RecordSignalStore::new(
        bob_records,
        Arc::new(AesGcmSealer::new(&DATA_KEY)),
        bob_lease.fence,
    )))
    .await
    .unwrap();
    let public = bob.public_identity().await;
    let prekey = bob
        .generate_prekeys(&InlineRuntime, 1)
        .await
        .unwrap()
        .remove(0);
    let bundle = RemoteBundle {
        registration_id: public.registration_id,
        device_id: 1,
        identity_key: public.identity_key,
        signed_prekey_id: public.signed_prekey.id,
        signed_prekey: public.signed_prekey.public_key,
        signed_prekey_signature: public.signed_prekey.signature,
        prekey: Some((prekey.id, prekey.public_key)),
    };
    let alice_address = ProtocolAddress::new("111@s.whatsapp.net", DeviceId::from(1));
    let bob_address = ProtocolAddress::new("222@s.whatsapp.net", DeviceId::from(1));
    alice
        .establish_session(&bob_address, &bundle)
        .await
        .unwrap();
    records.pause.store(true, Ordering::SeqCst);
    let encrypt = {
        let alice = alice.clone();
        let bob_address = bob_address.clone();
        tokio::spawn(async move { alice.encrypt(&bob_address, b"lost caller").await })
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        records.submitted.notified(),
    )
    .await
    .expect("ratchet write was not submitted");
    let (started, waiting) = tokio::sync::oneshot::channel();
    let waiter = {
        let alice = alice.clone();
        let bob_address = bob_address.clone();
        tokio::spawn(async move {
            let _ = started.send(());
            alice
                .encrypt(&bob_address, b"queued behind the uncertain write")
                .await
        })
    };
    waiting.await.unwrap();
    encrypt.abort();
    assert!(encrypt.await.unwrap_err().is_cancelled());
    assert!(!alice.is_usable());
    assert!(waiter.await.unwrap().unwrap_err().is_fence_lost());
    let refused = alice
        .encrypt(&bob_address, b"must not publish under the uncertain fence")
        .await
        .unwrap_err();
    assert!(refused.is_fence_lost());
    if release_before_takeover {
        records.resume.notify_one();
        records.finished.notified().await;
        assert!(records.committed.load(Ordering::SeqCst));
    }
    let higher = records
        .inner
        .acquire("alice", "replacement", TTL + 1, TTL)
        .await
        .unwrap()
        .unwrap();
    let fresh = SignalOps::open(Arc::new(RecordSignalStore::new(
        records.clone(),
        Arc::new(AesGcmSealer::new(&DATA_KEY)),
        higher.fence,
    )))
    .await
    .unwrap();
    let first = fresh
        .encrypt(&bob_address, b"published first")
        .await
        .unwrap();
    assert_eq!(
        bob.decrypt(&alice_address, first.kind, &first.ciphertext)
            .await
            .unwrap()
            .plaintext,
        b"published first"
    );
    if !release_before_takeover {
        records.resume.notify_one();
        records.finished.notified().await;
        assert!(
            records.refused.load(Ordering::SeqCst),
            "the delayed old-generation write was accepted after takeover"
        );
    }
    let second = fresh
        .encrypt(&bob_address, b"published second")
        .await
        .unwrap();
    assert_eq!(
        bob.decrypt(&alice_address, second.kind, &second.ciphertext)
            .await
            .unwrap()
            .plaintext,
        b"published second"
    );
}

#[tokio::test]
async fn a_cancelled_ratchet_write_cannot_be_followed_by_another_write_on_the_same_instance() {
    cancellation_case(true).await;
}

#[tokio::test]
async fn a_delayed_cancelled_write_cannot_roll_back_the_replacement_fence() {
    cancellation_case(false).await;
}

/// Pauses the first device-key read after it has read, before it returns.
struct SlowDeviceRead {
    inner: Arc<MemoryRecordStore>,
    pause: AtomicBool,
    reading: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}

#[async_trait]
impl RecordStore for SlowDeviceRead {
    async fn get(&self, scope: &str, ns: Namespace, key: &str) -> Result<Option<Vec<u8>>> {
        let value = self.inner.get(scope, ns, key).await;
        if ns == Namespace::Device && self.pause.swap(false, Ordering::SeqCst) {
            self.reading.notify_one();
            self.resume.notified().await;
        }
        value
    }

    async fn get_many(
        &self,
        scope: &str,
        ns: Namespace,
        keys: &[&str],
    ) -> Result<Vec<(String, Vec<u8>)>> {
        self.inner.get_many(scope, ns, keys).await
    }

    async fn scan_keys(
        &self,
        scope: &str,
        ns: Namespace,
        prefix: &str,
        limit: Option<usize>,
    ) -> Result<Vec<String>> {
        self.inner.scan_keys(scope, ns, prefix, limit).await
    }

    async fn write(&self, fence: &Fence, ops: &[WriteOp]) -> Result<()> {
        self.inner.write(fence, ops).await
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_overlapping_a_live_instance_cannot_restore_its_older_keys() {
    let records = Arc::new(SlowDeviceRead {
        inner: Arc::new(MemoryRecordStore::new()),
        pause: AtomicBool::new(false),
        reading: tokio::sync::Notify::new(),
        resume: tokio::sync::Notify::new(),
    });
    let lease = records
        .inner
        .acquire("slow-open", "node-a", 0, TTL)
        .await
        .unwrap()
        .unwrap();
    let store = || {
        Arc::new(RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(&DATA_KEY)),
            lease.fence.clone(),
        ))
    };
    let first = SignalOps::create(store()).await.unwrap();

    // The second open starts while the first instance is live. If it reads
    // the device keys before joining the first's authority, hold that read
    // while the first rotates and goes away.
    records.pause.store(true, Ordering::SeqCst);
    let mut opening = tokio::spawn(SignalOps::open(store()));
    let opened = tokio::select! {
        () = records.reading.notified() => None,
        opened = &mut opening => Some(opened),
    };
    // The fixed open never reads; keep the pause for the open alone.
    records.pause.store(false, Ordering::SeqCst);
    let rotated = first.rotate_signed_prekey().await.unwrap();
    drop(first);
    records.resume.notify_one();
    let second = match opened {
        Some(opened) => opened,
        None => opening.await,
    }
    .unwrap()
    .unwrap();

    assert_eq!(
        second.public_identity().await.signed_prekey.id,
        rotated.id,
        "the second open must see the first instance's rotation"
    );
    let next = second.rotate_signed_prekey().await.unwrap();
    assert_ne!(next.id, rotated.id, "a rotation must not reuse the live ID");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_open_with_a_different_sealer_cannot_join_a_live_instance() {
    let records = Arc::new(MemoryRecordStore::new());
    let lease = records
        .acquire("wrong-sealer", "node-a", 0, TTL)
        .await
        .unwrap()
        .unwrap();
    let store = |key: &[u8; 32]| {
        Arc::new(RecordSignalStore::new(
            records.clone(),
            Arc::new(AesGcmSealer::new(key)),
            lease.fence.clone(),
        ))
    };
    let first = SignalOps::create(store(&DATA_KEY)).await.unwrap();
    let identity = first.public_identity().await.identity_key;

    // Same backend, scope and fence, so it would share the live authority.
    assert!(
        SignalOps::open(store(&[7; 32])).await.is_err(),
        "a store that cannot open the device keys must not join"
    );

    // Nothing was resealed: the original sealer still opens the device.
    first.rotate_signed_prekey().await.unwrap();
    drop(first);
    let reopened = SignalOps::open(store(&DATA_KEY)).await.unwrap();
    assert_eq!(reopened.public_identity().await.identity_key, identity);
}
