//! Offline historical-writer qualification. Normal CI has no external snapshot.
//! Run explicitly with A12_FIXTURE_DIR pointing to a synthetic v0.7.0 fixture.
//! The same test source can produce that fixture in the historical checkout;
//! adapt the SQLite and Signal adapter constructors to their historical signatures.
#![cfg(feature = "sqlite-storage")]

use std::{path::Path, sync::Arc};
use wacore::appstate::hash::HashState;
use wacore::libsignal::protocol::{
    CiphertextMessage, IdentityKey, PreKeyBundle, ProtocolAddress, SignalMessage, UsePQRatchet,
    create_sender_key_distribution_message, group_decrypt, group_encrypt, message_decrypt,
    message_encrypt, process_prekey_bundle, process_sender_key_distribution_message,
};
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use wacore::store::traits::{AppStateSyncKey, AppSyncStore, DeviceStore, ProtocolStore};
use whatsapp_rust::store::{
    SqliteStore, commands::DeviceCommand, persistence_manager::PersistenceManager,
    signal_adapter::SignalProtocolStoreAdapter, signal_cache::SignalStoreCache,
};

const BASELINE: &str = "f8165f282008935732e0b26d6f5cca5038ecd222";
const FIXTURE_FILES: &[&str] = &[
    "historical.db",
    "device-1.json",
    "device-2.json",
    "payload-1.bin",
    "payload-2.bin",
    "dm-skipped.bin",
    "group-skipped.bin",
];

fn digest(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

async fn open_store(path: &str, id: i32) -> SqliteStore {
    SqliteStore::open(path).await.unwrap().database().store(id)
}

struct Peer {
    store: Arc<SqliteStore>,
    manager: Arc<PersistenceManager>,
    cache: Arc<SignalStoreCache>,
    signal: SignalProtocolStoreAdapter,
    address: ProtocolAddress,
}

impl Peer {
    async fn open(path: &str, id: i32) -> Self {
        let store = Arc::new(open_store(path, id).await);
        let manager = Arc::new(PersistenceManager::new(store.clone()).await.unwrap());
        let cache = Arc::new(SignalStoreCache::new());
        let signal = SignalProtocolStoreAdapter::new(manager.clone(), cache.clone());
        Self {
            store,
            manager,
            cache,
            signal,
            address: ProtocolAddress::new(&format!("1555000800{id}@c.us"), 1.into()),
        }
    }

    async fn flush(&self) {
        self.cache.flush(&*self.store).await.unwrap();
        self.manager.flush().await.unwrap();
    }

    fn bundle(&self) -> PreKeyBundle {
        let device = self.manager.get_device_snapshot();
        PreKeyBundle::new(
            device.registration_id,
            1.into(),
            None,
            device.signed_pre_key_id.into(),
            device.signed_pre_key.public_key,
            device.signed_pre_key_signature.to_vec(),
            IdentityKey::new(device.identity_key.public_key),
        )
        .unwrap()
    }

    async fn send(&mut self, target: &ProtocolAddress, bytes: &[u8]) -> CiphertextMessage {
        message_encrypt(
            bytes,
            target,
            &mut self.signal.session_store,
            &mut self.signal.identity_store,
        )
        .await
        .unwrap()
    }

    async fn receive(&mut self, source: &ProtocolAddress, message: &CiphertextMessage) -> Vec<u8> {
        message_decrypt(
            message,
            source,
            &mut self.signal.session_store,
            &mut self.signal.identity_store,
            &mut self.signal.pre_key_store,
            &self.signal.signed_pre_key_store,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
            UsePQRatchet::No,
        )
        .await
        .unwrap()
        .plaintext
    }
}

fn group_name() -> SenderKeyName {
    SenderKeyName::from_parts("120363000000080001@g.us", "15550008001@c.us.1")
}

// Keep construction valid when generated messages become non_exhaustive.
#[allow(clippy::field_reassign_with_default)]
async fn produce(dir: &Path) {
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head.status.success());
    assert_eq!(
        String::from_utf8(head.stdout).unwrap().trim(),
        BASELINE,
        "historical fixtures must be produced by the selected historical source"
    );
    assert!(
        std::process::Command::new("git")
            .args(["diff", "--quiet", "HEAD"])
            .status()
            .unwrap()
            .success(),
        "historical tracked sources and lock must remain unchanged"
    );
    let db = dir.join("historical.db");
    assert!(!db.exists(), "producer refuses to overwrite a snapshot");
    let path = db.to_str().unwrap();
    let mut alice = Peer::open(path, 1).await;
    let mut bob = Peer::open(path, 2).await;
    for (id, peer) in [(1, &alice), (2, &bob)] {
        peer.manager
            .process_command(DeviceCommand::SetId(Some(
                format!("1555000800{id}@c.us").parse().unwrap(),
            )))
            .await;
        peer.manager
            .process_command(DeviceCommand::SetPushName(format!("synthetic-{id}")))
            .await;
        let mut account = waproto::whatsapp::ADVSignedDeviceIdentity::default();
        account.details = Some(format!("synthetic-account-{id}").into_bytes());
        peer.manager
            .process_command(DeviceCommand::SetAccount(Some(account)))
            .await;
        peer.manager.flush().await.unwrap();
        let mut key = AppStateSyncKey::default();
        key.key_data = vec![id as u8; 32];
        key.fingerprint = vec![id as u8; 4];
        key.timestamp = 1700000000 + i64::from(id);
        peer.store.set_sync_key(&[id as u8], key).await.unwrap();
        let mut state = HashState::default();
        state.version = 9;
        state.hash = [id as u8; 128];
        state.mac_mismatch_fatal = true;
        state
            .index_value_map
            .insert("synthetic-index".into(), vec![id as u8; 32]);
        peer.store.set_version("regular", state).await.unwrap();
        let mut message = waproto::whatsapp::Message::default();
        message.conversation = Some(format!("synthetic-payload-{id}"));
        let mut payload = waproto::codec::message_to_vec(&message);
        payload.extend_from_slice(b"\x82\xb5\x18\x03new");
        peer.store
            .store_sent_message("15550008001@c.us", "retry", &payload)
            .await
            .unwrap();
        peer.store
            .store_pending_inbound("15550008001@c.us", "15550008002@c.us", "pending", &payload)
            .await
            .unwrap();
        std::fs::write(dir.join(format!("payload-{id}.bin")), payload).unwrap();
        std::fs::write(
            dir.join(format!("device-{id}.json")),
            serde_json::to_vec(&peer.store.load().await.unwrap().unwrap()).unwrap(),
        )
        .unwrap();
    }
    process_prekey_bundle(
        &bob.address,
        &mut alice.signal.session_store,
        &mut alice.signal.identity_store,
        &bob.bundle(),
        &mut rand::make_rng::<rand::rngs::StdRng>(),
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    let first = alice.send(&bob.address, b"hello").await;
    assert_eq!(bob.receive(&alice.address, &first).await, b"hello");
    let reply = bob.send(&alice.address, b"reply").await;
    assert_eq!(alice.receive(&bob.address, &reply).await, b"reply");
    let skipped = bob.send(&alice.address, b"old skipped DM").await;
    let later = bob.send(&alice.address, b"later DM").await;
    assert_eq!(alice.receive(&bob.address, &later).await, b"later DM");
    assert!(matches!(skipped, CiphertextMessage::SignalMessage(_)));
    std::fs::write(dir.join("dm-skipped.bin"), skipped.serialize()).unwrap();
    let name = group_name();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let distribution =
        create_sender_key_distribution_message(&name, &mut alice.signal.sender_key_store, &mut rng)
            .await
            .unwrap();
    process_sender_key_distribution_message(&name, &distribution, &mut bob.signal.sender_key_store)
        .await
        .unwrap();
    let skipped = group_encrypt(
        &mut alice.signal.sender_key_store,
        &name,
        b"old skipped group",
        &mut rng,
    )
    .await
    .unwrap();
    let later = group_encrypt(
        &mut alice.signal.sender_key_store,
        &name,
        b"later group",
        &mut rng,
    )
    .await
    .unwrap();
    assert_eq!(
        group_decrypt(later.serialized(), &mut bob.signal.sender_key_store, &name)
            .await
            .unwrap(),
        b"later group"
    );
    std::fs::write(dir.join("group-skipped.bin"), skipped.serialized()).unwrap();
    alice.flush().await;
    bob.flush().await;
    drop(alice);
    drop(bob);
    let hashes: std::collections::BTreeMap<_, _> = FIXTURE_FILES
        .iter()
        .map(|name| (*name, digest(&dir.join(name))))
        .collect();
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "producer_sha": BASELINE, "files": hashes,
        }))
        .unwrap(),
    )
    .unwrap();
}

struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("a12-historical-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scratch_is_removed_after_failed_qualification() {
    let scratch = Scratch::new();
    let path = scratch.0.clone();
    let retained_path = path.clone();
    let result = std::panic::catch_unwind(move || {
        let _scratch = scratch;
        for name in ["candidate.db", "candidate.db-wal", "candidate.db-shm"] {
            std::fs::write(path.join(name), b"synthetic state").unwrap();
        }
        panic!("injected qualification failure");
    });
    assert!(result.is_err());
    assert!(!retained_path.exists());
    // The guard also runs on ordinary return; the panic case catches regressions
    // where cleanup is accidentally moved back behind the final assertion.
}

fn copy_historical_database(dir: &Path) -> Scratch {
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["producer_sha"], BASELINE);
    for name in FIXTURE_FILES {
        assert_eq!(
            manifest["files"][name],
            digest(&dir.join(name)),
            "fixture digest: {name}"
        );
    }
    let scratch = Scratch::new();
    let database = scratch.0.join("candidate.db");
    std::fs::copy(dir.join("historical.db"), &database).unwrap();
    scratch
}

async fn consume(dir: &Path) {
    let scratch = copy_historical_database(dir);
    let database = scratch.0.join("candidate.db");
    let path = database.to_str().unwrap();
    {
        let mut alice = Peer::open(path, 1).await;
        let mut bob = Peer::open(path, 2).await;
        for (id, peer) in [(1, &alice), (2, &bob)] {
            let old: serde_json::Value = serde_json::from_slice(
                &std::fs::read(dir.join(format!("device-{id}.json"))).unwrap(),
            )
            .unwrap();
            let current = serde_json::to_value(peer.store.load().await.unwrap().unwrap()).unwrap();
            for field in [
                "noise_key",
                "identity_key",
                "signed_pre_key",
                "signed_pre_key_id",
                "signed_pre_key_signature",
                "registration_id",
                "adv_secret_key",
                "account",
                "push_name",
            ] {
                assert_eq!(current[field], old[field], "device {id}, field {field}");
            }
            let key = peer.store.get_sync_key(&[id as u8]).await.unwrap().unwrap();
            assert_eq!(key.key_data, vec![id as u8; 32]);
            assert_eq!(key.fingerprint, vec![id as u8; 4]);
            let state = peer.store.get_version("regular").await.unwrap().unwrap();
            assert_eq!(state.version, 9);
            assert_eq!(state.hash, [id as u8; 128]);
            assert!(state.mac_mismatch_fatal);
            assert_eq!(state.index_value_map["synthetic-index"], vec![id as u8; 32]);
            let payload = std::fs::read(dir.join(format!("payload-{id}.bin"))).unwrap();
            assert_eq!(
                peer.store
                    .get_sent_message("15550008001@s.whatsapp.net", "retry")
                    .await
                    .unwrap()
                    .unwrap(),
                payload
            );
            assert_eq!(
                peer.store
                    .get_pending_inbound(
                        "15550008001@s.whatsapp.net",
                        "15550008002@s.whatsapp.net",
                        "pending"
                    )
                    .await
                    .unwrap()
                    .unwrap(),
                payload
            );
        }
        let delayed = CiphertextMessage::SignalMessage(
            SignalMessage::try_from(
                std::fs::read(dir.join("dm-skipped.bin"))
                    .unwrap()
                    .as_slice(),
            )
            .unwrap(),
        );
        assert_eq!(
            alice.receive(&bob.address, &delayed).await,
            b"old skipped DM"
        );
        assert_eq!(
            group_decrypt(
                &std::fs::read(dir.join("group-skipped.bin")).unwrap(),
                &mut bob.signal.sender_key_store,
                &group_name()
            )
            .await
            .unwrap(),
            b"old skipped group"
        );
        let fresh = alice.send(&bob.address, b"new candidate DM").await;
        assert_eq!(
            bob.receive(&alice.address, &fresh).await,
            b"new candidate DM"
        );
        alice.flush().await;
        bob.flush().await;
    }
    {
        let mut alice = Peer::open(path, 1).await;
        let mut bob = Peer::open(path, 2).await;
        let reply = bob.send(&alice.address, b"after restart DM").await;
        assert_eq!(
            alice.receive(&bob.address, &reply).await,
            b"after restart DM"
        );
        let name = group_name();
        let fresh = group_encrypt(
            &mut alice.signal.sender_key_store,
            &name,
            b"after restart group",
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .unwrap();
        assert!(
            fresh.iteration()
                >= wacore::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH,
            "the historical sender lease must survive upgrade and restart"
        );
        assert_eq!(
            group_decrypt(fresh.serialized(), &mut bob.signal.sender_key_store, &name)
                .await
                .unwrap(),
            b"after restart group"
        );
        let payload = std::fs::read(dir.join("payload-1.bin")).unwrap();
        assert_eq!(
            alice
                .store
                .get_sent_message("15550008001@s.whatsapp.net", "retry")
                .await
                .unwrap()
                .unwrap(),
            payload
        );
        assert_eq!(
            alice
                .store
                .get_pending_inbound(
                    "15550008001@s.whatsapp.net",
                    "15550008002@s.whatsapp.net",
                    "pending"
                )
                .await
                .unwrap()
                .unwrap(),
            payload
        );
        alice.flush().await;
        bob.flush().await;
    }
    drop(scratch);
}

#[tokio::test]
#[ignore = "requires explicit synthetic historical-writer fixture"]
async fn historical_writer_upgrade_decrypt_send_restart() {
    let directory = std::env::var("A12_FIXTURE_DIR").expect("A12_FIXTURE_DIR is required");
    let dir = Path::new(&directory);
    if std::env::var_os("A12_PRODUCE").is_some() {
        produce(dir).await;
    } else {
        consume(dir).await;
    }
}

// Golden framing bytes are deliberately independent of the private A09 encoder.
// This checks storage compatibility; client replay is qualified separately by
// the runtime's restart/re-encryption tests, not by a duplicate replay decoder.
#[tokio::test]
#[ignore = "requires explicit synthetic historical-writer fixture"]
async fn historical_pending_and_multipart_survive_reopen() {
    let directory = std::env::var("A12_FIXTURE_DIR").expect("A12_FIXTURE_DIR is required");
    let dir = Path::new(&directory);
    let original_digest = digest(&dir.join("historical.db"));
    let scratch = copy_historical_database(dir);
    let database = scratch.0.join("candidate.db");
    let path = database.to_str().unwrap();
    let chat = "15550008001@s.whatsapp.net";
    let sender = "15550008002@s.whatsapp.net";
    let legacy: Vec<_> = (1..=2)
        .map(|id| std::fs::read(dir.join(format!("payload-{id}.bin"))).unwrap())
        .collect();
    let mut multipart = b"\0WAPI\x01".to_vec();
    for part in [&legacy[0], &legacy[1], &legacy[1]] {
        multipart.extend_from_slice(&(part.len() as u64).to_be_bytes());
        multipart.extend_from_slice(part);
    }
    // Future versions and torn writes must remain available for repair too.
    let future = b"\0WAPI\x02opaque".to_vec();
    let truncated = multipart[..multipart.len() - 1].to_vec();
    let records = [
        ("multi", multipart),
        ("future", future),
        ("torn", truncated),
    ];
    {
        let store = open_store(path, 1).await;
        for (id, bytes) in &records {
            store
                .store_pending_inbound(chat, sender, id, bytes)
                .await
                .unwrap();
        }
    }
    for cycle in 0..2 {
        let store = open_store(path, 1).await;
        let other = open_store(path, 2).await;
        for (id, bytes) in &records {
            assert_eq!(
                store
                    .get_pending_inbound(chat, sender, id)
                    .await
                    .unwrap()
                    .as_ref(),
                Some(bytes)
            );
            assert!(
                other
                    .get_pending_inbound(chat, sender, id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(
            other
                .get_pending_inbound(chat, sender, "pending")
                .await
                .unwrap()
                .as_ref(),
            Some(&legacy[1])
        );
        assert_eq!(
            store
                .get_pending_inbound(chat, sender, "pending")
                .await
                .unwrap(),
            (cycle == 0).then(|| legacy[0].clone())
        );
        assert_eq!(
            store
                .get_sent_message(chat, "retry")
                .await
                .unwrap()
                .as_ref(),
            Some(&legacy[0])
        );
        if cycle == 0 {
            store
                .delete_pending_inbound(chat, sender, "pending")
                .await
                .unwrap();
        }
    }
    // The original historical artifact is never migrated in place.
    assert_eq!(digest(&dir.join("historical.db")), original_digest);
}
