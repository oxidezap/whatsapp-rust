//! Frozen legacy implementations reproduce the removed defaults on the same public API.
//! Atomic-mode probes delegate to the real memory backend and inject errors before
//! its commit boundary. Regression tests require preservation and replay in that mode.
#![cfg(not(target_arch = "wasm32"))]

use bytes::Bytes;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use wacore::appstate::{hash::HashState, processor::AppStateMutationMAC};
use wacore::runtime::BoxFuture;
use wacore::store::{
    InMemoryBackend,
    error::{Result, StoreError},
    traits::*,
};

struct Probe {
    legacy: bool,
    fail_after_commit: AtomicBool,
    backend: InMemoryBackend,
    park_read: AtomicBool,
    resume_tx: async_channel::Sender<()>,
    resume_rx: async_channel::Receiver<()>,
    fail_write: AtomicUsize,
    writes: AtomicUsize,
}
impl Probe {
    fn new() -> Self {
        Self::with_mode(true)
    }
    fn atomic() -> Self {
        Self::with_mode(false)
    }
    fn with_mode(legacy: bool) -> Self {
        let (resume_tx, resume_rx) = async_channel::bounded(1);
        Self {
            legacy,
            fail_after_commit: AtomicBool::new(false),
            backend: InMemoryBackend::new(),
            park_read: AtomicBool::new(false),
            resume_tx,
            resume_rx,
            fail_write: AtomicUsize::new(0),
            writes: AtomicUsize::new(0),
        }
    }
    async fn park_atomic(&self) {
        if self.park_read.swap(false, Ordering::SeqCst) {
            self.resume_rx.recv().await.unwrap();
        }
    }
    fn before_call(&self, method: &str) -> Result<()> {
        if matches!(
            method,
            "set_version" | "delete_mutation_macs" | "put_mutation_macs"
        ) {
            let n = self.writes.fetch_add(1, Ordering::SeqCst) + 1;
            if self.fail_write.load(Ordering::SeqCst) == n {
                return Err(StoreError::Validation(format!(
                    "injected write {n}: {method}"
                )));
            }
        }
        Ok(())
    }
}
macro_rules! forward_methods {
    ($(fn $method:ident<$($lt:lifetime),+>($receiver:ident: &$self_lt:lifetime Self $(, $arg:ident: $ty:ty)*) -> $out:ty;)+) => {
        $(fn $method<$($lt,)+ 'async_trait>(&$self_lt self $(, $arg: $ty)*) -> BoxFuture<'async_trait, $out>
        where $($lt: 'async_trait,)+ Self: 'async_trait {
            Box::pin(async move {
                self.before_call(stringify!($method))?;
                self.backend.$method($($arg),*).await
            })
        })+
    };
}

macro_rules! forward_domains {
    ($owner:ty) => {
        impl SignalStore for $owner {
            forward_methods! {
                fn put_identity<'s, 'a>(this: &'s Self, address: &'a str, key: [u8; 32]) -> Result<()>;
                fn put_identities_batch<'s, 'a>(this: &'s Self, identities: &'a [(Arc<str>, [u8; 32])]) -> Result<()>;
                fn load_identity<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<[u8; 32]>>;
                fn delete_identity<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_identities_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
                fn get_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<Bytes>>;
                fn put_session<'s, 'a, 'b>(this: &'s Self, address: &'a str, session: &'b [u8]) -> Result<()>;
                fn put_sessions_batch<'s, 'a>(this: &'s Self, sessions: &'a [(Arc<str>, Bytes)]) -> Result<()>;
                fn get_sessions_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<Vec<(Arc<str>, Bytes)>>;
                fn delete_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_sessions_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
                fn has_session<'s, 'a>(this: &'s Self, address: &'a str) -> Result<bool>;
                fn has_signal_state_for_user<'s, 'a>(this: &'s Self, user: &'a str) -> Result<bool>;
                fn store_prekey<'s, 'a>(this: &'s Self, id: u32, record: &'a [u8], uploaded: bool) -> Result<()>;
                fn store_prekeys_batch<'s, 'a>(this: &'s Self, keys: &'a [(u32, Bytes)], uploaded: bool) -> Result<()>;
                fn load_prekey<'s>(this: &'s Self, id: u32) -> Result<Option<Bytes>>;
                fn load_prekeys_batch<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<Vec<(u32, Bytes)>>;
                fn mark_prekeys_uploaded<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<()>;
                fn remove_prekey<'s>(this: &'s Self, id: u32) -> Result<()>;
                fn remove_prekeys_batch<'s, 'a>(this: &'s Self, ids: &'a [u32]) -> Result<()>;
                fn get_max_prekey_id<'s>(this: &'s Self) -> Result<u32>;
                fn store_signed_prekey<'s, 'a>(this: &'s Self, id: u32, record: &'a [u8]) -> Result<()>;
                fn load_signed_prekey<'s>(this: &'s Self, id: u32) -> Result<Option<Vec<u8>>>;
                fn load_all_signed_prekeys<'s>(this: &'s Self) -> Result<Vec<(u32, Vec<u8>)>>;
                fn remove_signed_prekey<'s>(this: &'s Self, id: u32) -> Result<()>;
                fn put_sender_key<'s, 'a, 'b>(this: &'s Self, address: &'a str, record: &'b [u8]) -> Result<()>;
                fn put_sender_keys_batch<'s, 'a>(this: &'s Self, sender_keys: &'a [(Arc<str>, Bytes)]) -> Result<()>;
                fn get_sender_key<'s, 'a>(this: &'s Self, address: &'a str) -> Result<Option<Vec<u8>>>;
                fn delete_sender_key<'s, 'a>(this: &'s Self, address: &'a str) -> Result<()>;
                fn delete_sender_keys_batch<'s, 'a>(this: &'s Self, addresses: &'a [Arc<str>]) -> Result<()>;
            }
        }
        impl AppSyncStore for $owner {
            fn commit_patch<'s, 'a, 'b, 'c, 'async_trait>(
                &'s self, name: &'a str, state: HashState,
                removed_index_macs: &'b [Vec<u8>], added: &'c [AppStateMutationMAC],
            ) -> BoxFuture<'async_trait, Result<()>>
            where 's: 'async_trait, 'a: 'async_trait, 'b: 'async_trait, 'c: 'async_trait, Self: 'async_trait {
                Box::pin(async move {
                    if !self.legacy {
                        self.before_call("set_version")?;
                        if !removed_index_macs.is_empty() { self.before_call("delete_mutation_macs")?; }
                        if !added.is_empty() { self.before_call("put_mutation_macs")?; }
                        self.backend.commit_patch(name, state, removed_index_macs, added).await?;
                        if self.fail_after_commit.load(Ordering::SeqCst) {
                            return Err(StoreError::Validation("synthetic post-commit barrier failure".into()));
                        }
                        return Ok(());
                    }

        let version = state.version;
        self.set_version(name, state).await?;
        if !removed_index_macs.is_empty() {
            self.delete_mutation_macs(name, removed_index_macs).await?;
        }
        if !added.is_empty() {
            self.put_mutation_macs(name, version, added).await?;
        }
        Ok(())
                })
            }

            forward_methods! {
                fn get_sync_key<'s, 'a>(this: &'s Self, key_id: &'a [u8]) -> Result<Option<AppStateSyncKey>>;
                fn set_sync_key<'s, 'a>(this: &'s Self, key_id: &'a [u8], key: AppStateSyncKey) -> Result<()>;
                fn get_version<'s, 'a>(this: &'s Self, name: &'a str) -> Result<Option<HashState>>;
                fn delete_version<'s, 'a>(this: &'s Self, name: &'a str) -> Result<()>;
                fn set_version<'s, 'a>(this: &'s Self, name: &'a str, state: HashState) -> Result<()>;
                fn put_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, version: u64, mutations: &'b [AppStateMutationMAC]) -> Result<()>;
                fn get_mutation_mac<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_mac: &'b [u8]) -> Result<Option<Vec<u8>>>;
                fn get_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_macs: &'b [[u8; 32]]) -> Result<std::collections::HashMap<[u8; 32], Vec<u8>>>;
                fn delete_mutation_macs<'s, 'a, 'b>(this: &'s Self, name: &'a str, index_macs: &'b [Vec<u8>]) -> Result<()>;
                fn clear_mutation_macs<'s, 'a>(this: &'s Self, name: &'a str) -> Result<()>;
                fn get_latest_sync_key_id<'s>(this: &'s Self) -> Result<Option<Vec<u8>>>;
            }
        }
        impl ProtocolStore for $owner {
            fn touch_tc_token_sender_timestamp<'s, 'a, 'async_trait>(
                &'s self, jid: &'a str, sender_timestamp: i64,
            ) -> BoxFuture<'async_trait, Result<()>>
            where 's: 'async_trait, 'a: 'async_trait, Self: 'async_trait {
                Box::pin(async move {
                    if !self.legacy {
                        self.park_atomic().await;
                        return self.backend.touch_tc_token_sender_timestamp(jid, sender_timestamp).await;
                    }

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
                })
            }
            fn store_received_tc_token<'s, 'a, 'b, 'async_trait>(
                &'s self, jid: &'a str, token: &'b [u8], token_timestamp: i64,
            ) -> BoxFuture<'async_trait, Result<()>>
            where 's: 'async_trait, 'a: 'async_trait, 'b: 'async_trait, Self: 'async_trait {
                Box::pin(async move {
                    if !self.legacy {
                        self.park_atomic().await;
                        return self.backend.store_received_tc_token(jid, token, token_timestamp).await;
                    }

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
                })
            }

            fn get_tc_token<'s, 'a, 'async_trait>(&'s self, jid: &'a str) -> BoxFuture<'async_trait, Result<Option<TcTokenEntry>>>
            where 's: 'async_trait, 'a: 'async_trait, Self: 'async_trait {
                Box::pin(async move {
                    let snapshot = self.backend.get_tc_token(jid).await?;
                    if self.park_read.swap(false, Ordering::SeqCst) {
                        self.resume_rx.recv().await.unwrap();
                    }
                    Ok(snapshot)
                })
            }

            forward_methods! {
                fn get_sender_key_devices<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<Vec<(String, bool)>>;
                fn set_sender_key_status<'s, 'a, 'b, 'c>(this: &'s Self, group_jid: &'a str, entries: &'b [(&'c str, bool)]) -> Result<()>;
                fn clear_sender_key_devices<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<()>;
                fn delete_sender_key_device_rows<'s, 'a, 'b>(this: &'s Self, device_jids: &'a [&'b str]) -> Result<()>;
                fn clear_all_sender_key_devices<'s>(this: &'s Self) -> Result<()>;
                fn get_lid_mapping<'s, 'a>(this: &'s Self, lid: &'a str) -> Result<Option<LidPnMappingEntry>>;
                fn get_pn_mapping<'s, 'a>(this: &'s Self, phone: &'a str) -> Result<Option<LidPnMappingEntry>>;
                fn put_lid_mapping<'s, 'a>(this: &'s Self, entry: &'a LidPnMappingEntry) -> Result<()>;
                fn put_lid_mappings<'s, 'a>(this: &'s Self, entries: &'a [LidPnMappingEntry]) -> Result<()>;
                fn get_all_lid_mappings<'s>(this: &'s Self) -> Result<Vec<LidPnMappingEntry>>;
                fn save_base_key<'s, 'a, 'b, 'c>(this: &'s Self, address: &'a str, message_id: &'b str, base_key: &'c [u8]) -> Result<()>;
                fn has_same_base_key<'s, 'a, 'b, 'c>(this: &'s Self, address: &'a str, message_id: &'b str, current_base_key: &'c [u8]) -> Result<bool>;
                fn delete_base_key<'s, 'a, 'b>(this: &'s Self, address: &'a str, message_id: &'b str) -> Result<()>;
                fn delete_expired_base_keys<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn update_device_list<'s>(this: &'s Self, record: DeviceListRecord) -> Result<()>;
                fn update_device_lists<'s>(this: &'s Self, records: Vec<DeviceListRecord>) -> Result<()>;
                fn get_devices<'s, 'a>(this: &'s Self, user: &'a str) -> Result<Option<DeviceListRecord>>;
                fn get_devices_batch<'s, 'a, 'b>(this: &'s Self, users: &'a [&'b str]) -> Result<Vec<DeviceListRecord>>;
                fn delete_devices<'s, 'a>(this: &'s Self, user: &'a str) -> Result<()>;
                fn get_group_metadata<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<Option<Vec<u8>>>;
                fn put_group_metadata<'s, 'a, 'b>(this: &'s Self, group_jid: &'a str, blob: &'b [u8]) -> Result<()>;
                fn delete_group_metadata<'s, 'a>(this: &'s Self, group_jid: &'a str) -> Result<()>;
                fn get_tc_tokens<'s, 'a>(this: &'s Self, jids: &'a [String]) -> Result<Vec<Option<TcTokenEntry>>>;
                fn put_tc_token<'s, 'a, 'b>(this: &'s Self, jid: &'a str, entry: &'b TcTokenEntry) -> Result<()>;
                fn delete_tc_token<'s, 'a>(this: &'s Self, jid: &'a str) -> Result<()>;
                fn get_all_tc_token_jids<'s>(this: &'s Self) -> Result<Vec<String>>;
                fn delete_expired_tc_tokens<'s>(this: &'s Self, token_cutoff: i64, sender_cutoff: i64) -> Result<u32>;
                fn store_sent_message<'s, 'a, 'b, 'c>(this: &'s Self, chat_jid: &'a str, message_id: &'b str, payload: &'c [u8]) -> Result<()>;
                fn get_sent_message<'s, 'a, 'b>(this: &'s Self, chat_jid: &'a str, message_id: &'b str) -> Result<Option<Vec<u8>>>;
                fn take_sent_message<'s, 'a, 'b>(this: &'s Self, chat_jid: &'a str, message_id: &'b str) -> Result<Option<Vec<u8>>>;
                fn delete_expired_sent_messages<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn store_pending_inbound<'s, 'a, 'b, 'c, 'd>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str, message: &'d [u8]) -> Result<()>;
                fn get_pending_inbound<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str) -> Result<Option<Vec<u8>>>;
                fn delete_pending_inbound<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, id: &'c str) -> Result<()>;
                fn delete_expired_pending_inbound<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
                fn store_pending_inbound_batch<'s, 'a, 'b>(this: &'s Self, rows: &'a [PendingInboundRow<'b>]) -> Result<()>;
                fn delete_pending_inbound_batch<'s, 'a, 'b>(this: &'s Self, keys: &'a [PendingInboundKey<'b>]) -> Result<()>;
            }
        }
        impl MsgSecretStore for $owner {
            forward_methods! {
                fn put_msg_secret<'s, 'a, 'b, 'c, 'd>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str, secret: &'d MessageSecretBytes) -> Result<()>;
                fn put_msg_secrets<'s>(this: &'s Self, entries: Vec<MsgSecretEntry>) -> Result<usize>;
                fn get_msg_secret<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str) -> Result<Option<Vec<u8>>>;
                fn get_stored_msg_secret<'s, 'a, 'b, 'c>(this: &'s Self, chat: &'a str, sender: &'b str, msg_id: &'c str) -> Result<Option<StoredMessageSecret>>;
                fn delete_expired_msg_secrets<'s>(this: &'s Self, cutoff_timestamp: i64) -> Result<u32>;
            }
        }
    };
}

forward_domains!(Probe);
#[async_trait::async_trait]
impl DeviceStore for Probe {
    async fn save(&self, device: &wacore::store::Device) -> Result<()> {
        self.backend.save(device).await
    }
    async fn load(&self) -> Result<Option<wacore::store::Device>> {
        self.backend.load().await
    }
    async fn exists(&self) -> Result<bool> {
        self.backend.exists().await
    }
    async fn create(&self) -> Result<i32> {
        self.backend.create().await
    }
}

#[tokio::test]
async fn legacy_sender_write_loses_concurrent_real_token() {
    let p = Probe::new();
    p.park_read.store(true, Ordering::SeqCst);
    let mut sender = Box::pin(p.touch_tc_token_sender_timestamp("100001@lid", 5000));
    assert!(futures::poll!(sender.as_mut()).is_pending());
    p.backend
        .store_received_tc_token("100001@lid", b"real", 4000)
        .await
        .unwrap();
    p.resume_tx.send(()).await.unwrap();
    sender.await.unwrap();
    let row = p.backend.get_tc_token("100001@lid").await.unwrap().unwrap();
    assert!(
        row.token.is_empty(),
        "reproduces token loss after a successful receive"
    );
    assert_eq!(row.sender_timestamp, Some(5000));
}

#[tokio::test]
async fn legacy_received_write_loses_concurrent_sender_timestamp() {
    let p = Probe::new();
    p.park_read.store(true, Ordering::SeqCst);
    let mut received = Box::pin(p.store_received_tc_token("100001@lid", b"real", 4000));
    assert!(futures::poll!(received.as_mut()).is_pending());
    p.backend
        .touch_tc_token_sender_timestamp("100001@lid", 5000)
        .await
        .unwrap();
    p.resume_tx.send(()).await.unwrap();
    received.await.unwrap();
    let row = p.backend.get_tc_token("100001@lid").await.unwrap().unwrap();
    assert_eq!(row.token, b"real");
    assert_eq!(
        row.sender_timestamp, None,
        "reproduces loss of confirmed issuance"
    );
}

#[tokio::test]
async fn legacy_received_write_regresses_newer_token() {
    let p = Probe::new();
    p.park_read.store(true, Ordering::SeqCst);
    let mut stale = Box::pin(p.store_received_tc_token("100001@lid", b"old", 3000));
    assert!(futures::poll!(stale.as_mut()).is_pending());
    p.backend
        .store_received_tc_token("100001@lid", b"new", 6000)
        .await
        .unwrap();
    p.resume_tx.send(()).await.unwrap();
    stale.await.unwrap();
    let row = p.backend.get_tc_token("100001@lid").await.unwrap().unwrap();
    assert_eq!(row.token, b"old", "reproduces violation of newer-wins");
    assert_eq!(row.token_timestamp, 3000);
}

struct InlineRuntime;
impl wacore::runtime::Runtime for InlineRuntime {
    fn spawn(&self, _: BoxFuture<'static, ()>) -> wacore::runtime::AbortHandle {
        panic!("unexpected spawn")
    }
    fn sleep(&self, _: std::time::Duration) -> BoxFuture<'static, ()> {
        panic!("unexpected sleep")
    }
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            f();
        })
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        None
    }
}
use wacore::appstate::{
    encode_record, expand_app_state_keys,
    patch_decode::{PatchList, WAPatchName},
};
use wacore::appstate_sync::{AppStateProcessor, CommittedMutationsError};
use waproto::whatsapp as wa;

fn processor(p: Arc<Probe>) -> AppStateProcessor {
    AppStateProcessor::new(p, Arc::new(InlineRuntime))
}
fn list(patches: Vec<wa::SyncdPatch>) -> PatchList {
    PatchList {
        name: WAPatchName::Regular,
        has_more_patches: false,
        patches,
        snapshot: None,
        snapshot_ref: None,
        error: None,
    }
}
// Default-based construction also compiles against A02's non-exhaustive protobufs.
#[allow(clippy::field_reassign_with_default)]
fn mutation(index: &[u8], remove: bool, time: i64) -> wa::SyncdMutation {
    let mut value = wa::SyncActionValue::default();
    value.timestamp = Some(time);
    encode_record(
        if remove {
            wa::syncd_mutation::SyncdOperation::Remove
        } else {
            wa::syncd_mutation::SyncdOperation::Set
        },
        index,
        &value,
        &expand_app_state_keys(&[7; 32]),
        b"synthetic-key",
        &[1; 16],
        1,
    )
    .0
}
#[allow(clippy::field_reassign_with_default)]
async fn patch(p: &AppStateProcessor, mutations: Vec<wa::SyncdMutation>) -> wa::SyncdPatch {
    let (bytes, version) = p.build_patch("regular", mutations).await.unwrap();
    let mut patch = waproto::codec::syncd_patch_decode(&bytes).unwrap();
    let mut patch_version = wa::SyncdVersion::default();
    patch_version.version = Some(version + 1);
    patch.version = patch_version.into();
    patch
}
const OLD: &[u8] = b"[\"synthetic-old\"]";
const NEW: &[u8] = b"[\"synthetic-new\"]";
fn index_mac(index: &[u8]) -> Vec<u8> {
    wacore::appstate::hash::generate_index_mac(index, &expand_app_state_keys(&[7; 32]).index)
}
async fn seeded() -> Arc<Probe> {
    seed(Arc::new(Probe::new())).await
}
async fn seed(p: Arc<Probe>) -> Arc<Probe> {
    p.backend
        .set_sync_key(
            b"synthetic-key",
            AppStateSyncKey {
                key_data: vec![7; 32],
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let proc = processor(p.clone());
    let first = patch(&proc, vec![mutation(OLD, false, 1)]).await;
    let (mutations, state, _) = proc
        .process_patch_list(list(vec![first]), true)
        .await
        .unwrap();
    assert_eq!(state.version, 1);
    assert_eq!(mutations.len(), 1);
    p.writes.store(0, Ordering::SeqCst);
    p
}

#[tokio::test]
async fn legacy_commit_failure_leaves_cursor_ahead_and_restart_cannot_replay() {
    for fail_at in 1..=3 {
        let p = seeded().await;
        let proc = processor(p.clone());
        let second = patch(&proc, vec![mutation(OLD, true, 2), mutation(NEW, false, 2)]).await;
        // A separate complete history produces a valid next patch. All MAC validation stays on.
        let reference = seeded().await;
        let reference_proc = processor(reference);
        reference_proc
            .process_patch_list(list(vec![second.clone()]), true)
            .await
            .unwrap();
        let third = patch(&reference_proc, vec![mutation(NEW, false, 3)]).await;

        p.fail_write.store(fail_at, Ordering::SeqCst);
        let err = proc
            .process_patch_list(list(vec![second.clone()]), true)
            .await
            .unwrap_err();
        assert!(
            err.to_string()
                .contains(&format!("injected write {fail_at}")),
            "{err:#}"
        );
        assert!(err.downcast_ref::<CommittedMutationsError>().is_none());
        let held = p.get_version("regular").await.unwrap().unwrap();
        assert_eq!(held.version, if fail_at == 1 { 1 } else { 2 });
        assert_eq!(
            p.get_mutation_mac("regular", &index_mac(OLD))
                .await
                .unwrap()
                .is_some(),
            fail_at != 3
        );
        assert!(
            p.get_mutation_mac("regular", &index_mac(NEW))
                .await
                .unwrap()
                .is_none()
        );

        drop(proc);
        p.fail_write.store(0, Ordering::SeqCst);
        let restarted = processor(p.clone());
        let replay = restarted.process_patch_list(list(vec![second]), true).await;
        if fail_at == 1 {
            let (mutations, state, _) = replay.unwrap();
            assert_eq!(mutations.len(), 2);
            assert_eq!(state.version, 2);
            restarted
                .process_patch_list(list(vec![third]), true)
                .await
                .unwrap();
        } else {
            let err = replay.unwrap_err();
            // The current processor formats AppStateError into anyhow at this boundary.
            assert_eq!(err.to_string(), "patch version mismatch: expected 3, got 2");
            let (mutations, advanced, _) = restarted
                .process_patch_list(list(vec![third.clone()]), true)
                .await
                .unwrap();
            assert_eq!(
                mutations.len(),
                1,
                "failed patch's two mutations were never returned"
            );
            assert!(
                advanced.mac_mismatch_fatal,
                "partial MAC persistence caused aggregate divergence"
            );
            let (_, consistent, _) = reference_proc
                .process_patch_list(list(vec![third]), true)
                .await
                .unwrap();
            assert!(!consistent.mac_mismatch_fatal);
            assert_ne!(advanced.hash, consistent.hash);
            eprintln!(
                "failed write {fail_at}: cursor={}, replay refused; valid next patch accepted with divergent aggregate hash",
                held.version
            );
        }
    }
}

#[tokio::test]
async fn legacy_commit_failure_preserves_earlier_result_but_persisted_cursor_can_disagree() {
    let p = seeded().await;
    let proc = processor(p.clone());
    let second = patch(&proc, vec![mutation(OLD, true, 2), mutation(NEW, false, 2)]).await;
    let reference = seeded().await;
    let reference_proc = processor(reference);
    reference_proc
        .process_patch_list(list(vec![second.clone()]), true)
        .await
        .unwrap();
    let third = patch(&reference_proc, vec![mutation(NEW, false, 3)]).await;
    // Second patch writes version/delete/add. Third writes version then fails adding MACs.
    p.fail_write.store(5, Ordering::SeqCst);
    let err = proc
        .process_patch_list(list(vec![second, third]), true)
        .await
        .unwrap_err();
    let committed = err.downcast::<CommittedMutationsError>().unwrap();
    let (mutations, state, collection, cause) = committed.into_parts();
    assert_eq!(mutations.len(), 2);
    assert_eq!(state.version, 2);
    assert_eq!(collection, WAPatchName::Regular);
    assert!(matches!(
        cause.downcast_ref::<StoreError>(),
        Some(StoreError::Validation(_))
    ));
    assert!(cause.to_string().contains("injected write 5"));
    assert_eq!(
        p.get_version("regular").await.unwrap().unwrap().version,
        3,
        "persisted cursor is ahead of the correctly reported committed result"
    );
}

#[tokio::test]
async fn atomic_token_merges_preserve_interleaved_updates() {
    for operation in 0..3 {
        let p = Probe::atomic();
        p.park_read.store(true, Ordering::SeqCst);
        let mut pending = match operation {
            0 => p.touch_tc_token_sender_timestamp("100001@lid", 5000),
            1 => p.store_received_tc_token("100001@lid", b"real", 4000),
            _ => p.store_received_tc_token("100001@lid", b"old", 3000),
        };
        assert!(futures::poll!(pending.as_mut()).is_pending());
        match operation {
            0 => p
                .backend
                .store_received_tc_token("100001@lid", b"real", 4000)
                .await
                .unwrap(),
            1 => p
                .backend
                .touch_tc_token_sender_timestamp("100001@lid", 5000)
                .await
                .unwrap(),
            _ => p
                .backend
                .store_received_tc_token("100001@lid", b"new", 6000)
                .await
                .unwrap(),
        }
        p.resume_tx.send(()).await.unwrap();
        pending.await.unwrap();
        let row = p.get_tc_token("100001@lid").await.unwrap().unwrap();
        if operation == 2 {
            assert_eq!(row.token, b"new");
            assert_eq!(row.token_timestamp, 6000);
            assert_eq!(row.sender_timestamp, None);
        } else {
            assert_eq!(row.token, b"real");
            assert_eq!(row.token_timestamp, 4000);
            assert_eq!(row.sender_timestamp, Some(5000));
        }
    }
}

#[tokio::test]
async fn atomic_commit_failure_keeps_cursor_and_macs_replayable_after_restart() {
    for fail_at in 1..=3 {
        let p = seed(Arc::new(Probe::atomic())).await;
        let proc = processor(p.clone());
        let before = p.get_version("regular").await.unwrap().unwrap();
        let old_mac = p
            .get_mutation_mac("regular", &index_mac(OLD))
            .await
            .unwrap();
        let second = patch(&proc, vec![mutation(OLD, true, 2), mutation(NEW, false, 2)]).await;
        p.fail_write.store(fail_at, Ordering::SeqCst);
        let error = proc
            .process_patch_list(list(vec![second.clone()]), true)
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<StoreError>(),
            Some(StoreError::Validation(_))
        ));
        let held = p.get_version("regular").await.unwrap().unwrap();
        assert_eq!(held.version, before.version);
        assert_eq!(held.hash, before.hash);
        assert_eq!(
            p.get_mutation_mac("regular", &index_mac(OLD))
                .await
                .unwrap(),
            old_mac
        );
        assert!(
            p.get_mutation_mac("regular", &index_mac(NEW))
                .await
                .unwrap()
                .is_none()
        );
        drop(proc);
        p.fail_write.store(0, Ordering::SeqCst);
        let restarted = processor(p.clone());
        let (mutations, state, _) = restarted
            .process_patch_list(list(vec![second]), true)
            .await
            .unwrap();
        assert_eq!(mutations.len(), 2);
        assert_eq!(state.version, 2);
        assert!(!state.mac_mismatch_fatal);
        assert!(
            p.get_mutation_mac("regular", &index_mac(OLD))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            p.get_mutation_mac("regular", &index_mac(NEW))
                .await
                .unwrap()
                .is_some()
        );
        let third = patch(&restarted, vec![mutation(NEW, false, 3)]).await;
        let (_, state, _) = restarted
            .process_patch_list(list(vec![third]), true)
            .await
            .unwrap();
        assert_eq!(state.version, 3);
        assert!(!state.mac_mismatch_fatal);
    }
}

#[tokio::test]
async fn atomic_commit_error_reports_exactly_the_persisted_partial_result() {
    let p = seed(Arc::new(Probe::atomic())).await;
    let proc = processor(p.clone());
    let second = patch(&proc, vec![mutation(OLD, true, 2), mutation(NEW, false, 2)]).await;
    let reference = processor(seeded().await);
    reference
        .process_patch_list(list(vec![second.clone()]), true)
        .await
        .unwrap();
    let third = patch(&reference, vec![mutation(NEW, false, 3)]).await;
    p.fail_write.store(5, Ordering::SeqCst);
    let error = proc
        .process_patch_list(list(vec![second, third.clone()]), true)
        .await
        .unwrap_err();
    let (mutations, committed, collection, cause) = error
        .downcast::<CommittedMutationsError>()
        .unwrap()
        .into_parts();
    assert_eq!(mutations.len(), 2);
    assert_eq!(committed.version, 2);
    assert_eq!(collection, WAPatchName::Regular);
    assert!(matches!(
        cause.downcast_ref::<StoreError>(),
        Some(StoreError::Validation(_))
    ));
    let held = p.get_version("regular").await.unwrap().unwrap();
    assert_eq!(held.version, committed.version);
    assert_eq!(held.hash, committed.hash);
    drop(proc);
    p.fail_write.store(0, Ordering::SeqCst);
    let (mutations, state, _) = processor(p)
        .process_patch_list(list(vec![third]), true)
        .await
        .unwrap();
    assert_eq!(mutations.len(), 1);
    assert_eq!(state.version, 3);
    assert!(!state.mac_mismatch_fatal);
}

/// Atomicity prevents torn MAC state; it cannot promise delivery after a later
/// durability error. Keep this limitation explicit while preserving the error.
#[tokio::test]
async fn atomic_post_commit_error_still_requires_delivery_recovery() {
    let p = seed(Arc::new(Probe::atomic())).await;
    let proc = processor(p.clone());
    let second = patch(&proc, vec![mutation(OLD, true, 2), mutation(NEW, false, 2)]).await;
    p.fail_after_commit.store(true, Ordering::SeqCst);
    let error = proc
        .process_patch_list(list(vec![second.clone()]), true)
        .await
        .unwrap_err();
    assert!(error.downcast_ref::<CommittedMutationsError>().is_none());
    assert!(matches!(
        error.downcast_ref::<StoreError>(),
        Some(StoreError::Validation(_))
    ));
    assert_eq!(p.get_version("regular").await.unwrap().unwrap().version, 2);
    assert!(
        p.get_mutation_mac("regular", &index_mac(OLD))
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        p.get_mutation_mac("regular", &index_mac(NEW))
            .await
            .unwrap()
            .is_some()
    );
    drop(proc);
    p.fail_after_commit.store(false, Ordering::SeqCst);
    let restarted = processor(p);
    let error = restarted
        .process_patch_list(list(vec![second]), true)
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "patch version mismatch: expected 3, got 2"
    );
    let third = patch(&restarted, vec![mutation(NEW, false, 3)]).await;
    let (_, state, _) = restarted
        .process_patch_list(list(vec![third]), true)
        .await
        .unwrap();
    assert!(
        !state.mac_mismatch_fatal,
        "the complete MAC set survived the error"
    );
}
