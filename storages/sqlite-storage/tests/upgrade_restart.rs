//! Synthetic schema upgrades, separate from old-binary and live-account qualification.
//! v0.7.0 (f8165f282008935732e0b26d6f5cca5038ecd222) shipped the first
//! selected baseline with both protobuf app-state and pending inbound storage.
//! Its 23 migrations are unchanged. Apply that prefix to an empty file rather
//! than reverse-migrating a current database, then let the real opener upgrade it.
//! Signal records below use the current codec; legacy codec/skipped-key coverage
//! lives in libsignal's existing tests. This is not a downgrade guarantee.
#![cfg(not(target_family = "wasm"))]

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sql_types::{Binary, Integer, Text};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use rand::SeedableRng;
use std::sync::atomic::{AtomicUsize, Ordering};
use wacore::libsignal::protocol::{
    ChainKey, IdentityKey, KeyPair, RootKey, SenderKeyName, SenderKeyRecord, SenderKeyStore,
    SessionRecord, SessionState, group_decrypt, group_encrypt,
};
use wacore::store::traits::{
    AppSyncStore, DeviceStore, MsgSecretStore, ProtocolStore, SignalStore,
};
use whatsapp_rust_sqlite_storage::{SqliteDatabase, SqliteDatabaseConfig, SqliteStore};

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");
const BASELINE_LAST: &str = "20260703000001";
const SIGNAL_ADDRESS: &str = "15550008001@c.us.1";
const SENDER_ADDRESS: &str = "120363000000080001@g.us::15550008001@c.us.1";
const OLD_JID: &str = "15550008001@c.us";
const JID: &str = "15550008001@s.whatsapp.net";
// Message.conversation = "synthetic", followed by an unknown length-delimited
// field 50000. The opaque retry/buffer payload must survive byte-for-byte.
const PAYLOAD: &[u8] = b"\x0a\x09synthetic\x82\xb5\x18\x03new";

struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "wa_upgrade_restart_{}_{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        // Refuse to overwrite even an unlikely stale fixture.
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        Self(path)
    }
    fn url(&self) -> &str {
        self.0.to_str().unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.0.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn keypair_bytes(pair: &KeyPair) -> Vec<u8> {
    // The v0.7.0 device writer stored private || unprefixed public bytes.
    [
        pair.private_key.serialize(),
        pair.public_key.public_key_bytes(),
    ]
    .concat()
}

struct Seed {
    identity: Vec<u8>,
    noise: Vec<u8>,
    signed_pre_key: Vec<u8>,
    registration_id: u32,
    marker: u8,
    session: Vec<u8>,
}

fn seed_baseline(fixture: &Fixture) -> Vec<Seed> {
    let mut conn = SqliteConnection::establish(fixture.url()).unwrap();
    let mut migrations = conn.pending_migrations(MIGRATIONS).unwrap();
    migrations.retain(|m| m.name().version().to_string().as_str() <= BASELINE_LAST);
    assert_eq!(migrations.len(), 23, "v0.7.0 migration prefix changed");
    conn.run_migrations(&migrations).unwrap();
    assert_eq!(conn.applied_migrations().unwrap().len(), 23);

    let mut rng = rand::rngs::StdRng::seed_from_u64(0x00A1_2070);
    let signing = KeyPair::generate(&mut rng);
    let mut seeds = Vec::new();

    for device_id in [1, 2] {
        let marker = device_id as u8;
        let registration_id = 1234 + device_id as u32;
        let identity = KeyPair::generate(&mut rng);
        let noise = keypair_bytes(&KeyPair::generate(&mut rng));
        let signed_pre_key = keypair_bytes(&KeyPair::generate(&mut rng));
        let remote = KeyPair::generate(&mut rng);
        let mut state = SessionState::new(
            3,
            &IdentityKey::new(identity.public_key),
            &IdentityKey::new(remote.public_key),
            &RootKey::new([7; 32]),
            &KeyPair::generate(&mut rng).public_key,
        );
        state.set_sender_chain(&KeyPair::generate(&mut rng), &ChainKey::new([11; 32], 4));
        let session = SessionRecord::new(state).serialize().unwrap();
        let identity = keypair_bytes(&identity);
        let mut sender_key = SenderKeyRecord::new_empty();
        sender_key
            .add_sender_key_state(
                3,
                17,
                0,
                &[0x33; 32],
                signing.public_key,
                (device_id == 1).then(|| signing.private_key.clone()),
            )
            .unwrap();
        diesel::sql_query("INSERT INTO sender_keys (address, record, device_id) VALUES (?, ?, ?)")
            .bind::<Text, _>(SENDER_ADDRESS)
            .bind::<Binary, _>(sender_key.serialize().unwrap())
            .bind::<Integer, _>(device_id)
            .execute(&mut conn)
            .unwrap();
        diesel::sql_query("INSERT INTO device (id, lid, pn, registration_id, noise_key, identity_key, signed_pre_key, signed_pre_key_id, signed_pre_key_signature, adv_secret_key) VALUES (?, '10000000008001@lid', ?, ?, ?, ?, ?, 8, ?, ?)")
            .bind::<Integer, _>(device_id).bind::<Text, _>(OLD_JID)
            .bind::<Integer, _>(registration_id as i32)
            .bind::<Binary, _>(&noise).bind::<Binary, _>(&identity)
            .bind::<Binary, _>(&signed_pre_key).bind::<Binary, _>(vec![marker; 64])
            .bind::<Binary, _>(vec![marker + 10; 32]).execute(&mut conn).unwrap();
        diesel::sql_query("INSERT INTO identities (address, key, device_id) VALUES (?, ?, ?)")
            .bind::<Text, _>(SIGNAL_ADDRESS)
            .bind::<Binary, _>(remote.public_key.public_key_bytes())
            .bind::<Integer, _>(device_id)
            .execute(&mut conn)
            .unwrap();
        diesel::sql_query("INSERT INTO sessions (address, record, device_id) VALUES (?, ?, ?)")
            .bind::<Text, _>(SIGNAL_ADDRESS)
            .bind::<Binary, _>(&session)
            .bind::<Integer, _>(device_id)
            .execute(&mut conn)
            .unwrap();
        for (query, id) in [
            (
                "INSERT INTO sent_messages (chat_jid, message_id, payload, device_id, created_at) VALUES (?, ?, ?, ?, 1700000000)",
                "retry",
            ),
            (
                "INSERT INTO pending_inbound_messages (chat, id, message, device_id, sender, inserted_at) VALUES (?, ?, ?, ?, '15550008001@c.us', 1700000000)",
                "pending",
            ),
        ] {
            diesel::sql_query(query)
                .bind::<Text, _>(OLD_JID)
                .bind::<Text, _>(id)
                .bind::<Binary, _>(PAYLOAD)
                .bind::<Integer, _>(device_id)
                .execute(&mut conn)
                .unwrap();
        }
        // Frozen protobuf shape from v0.7.0 wire.proto: master key, fingerprint,
        // timestamp; version, 128-byte hash, fatal flag. No bootstrapped field.
        let sync_key = [b"\x0a\x20".as_slice(), &[0x11; 32], b"\x12\x02fp\x18\x07"].concat();
        let hash_state = [
            b"\x08\x07\x12\x80\x01".as_slice(),
            &[0x22; 128],
            b"\x20\x01",
        ]
        .concat();
        diesel::sql_query(
            "INSERT INTO app_state_keys (key_id, key_data, device_id) VALUES (x'01', ?, ?)",
        )
        .bind::<Binary, _>(sync_key)
        .bind::<Integer, _>(device_id)
        .execute(&mut conn)
        .unwrap();
        diesel::sql_query(
            "INSERT INTO app_state_versions (name, state_data, device_id) VALUES ('regular', ?, ?)",
        )
        .bind::<Binary, _>(hash_state)
        .bind::<Integer, _>(device_id)
        .execute(&mut conn)
        .unwrap();
        seeds.push(Seed {
            identity,
            noise,
            signed_pre_key,
            registration_id,
            marker,
            session,
        });
    }
    conn.batch_execute("INSERT INTO msg_secrets (chat, sender, msg_id, secret, device_id, created_at, expires_at, message_ts) VALUES ('15550008001@c.us', '15550008001@c.us', 'secret', zeroblob(32), 1, 123, 0, 1700000000)").unwrap();
    seeds
}

async fn verify(db: &SqliteDatabase, seeds: &[Seed]) {
    for (index, seed) in seeds.iter().enumerate() {
        let device_id = index as i32 + 1;
        let store = db.store(device_id);
        let device = store.load().await.unwrap().expect("existing account");
        assert_eq!(device.registration_id, seed.registration_id);
        assert_eq!(keypair_bytes(&device.identity_key), seed.identity);
        assert_eq!(keypair_bytes(&device.noise_key), seed.noise);
        assert_eq!(keypair_bytes(&device.signed_pre_key), seed.signed_pre_key);
        assert_eq!(device.signed_pre_key_id, 8);
        assert_eq!(device.signed_pre_key_signature, [seed.marker; 64]);
        assert_eq!(device.adv_secret_key, [seed.marker + 10; 32]);
        assert_eq!(device.pn.unwrap().to_string(), JID);
        assert_eq!(
            store
                .get_session(SIGNAL_ADDRESS)
                .await
                .unwrap()
                .unwrap()
                .as_ref(),
            seed.session
        );
        assert!(
            store
                .get_session("15550008001@s.whatsapp.net.1")
                .await
                .unwrap()
                .is_none()
        );
        let record = SessionRecord::deserialize(&seed.session).unwrap();
        let state = record.session_state().unwrap();
        assert_eq!(
            store
                .load_identity(SIGNAL_ADDRESS)
                .await
                .unwrap()
                .unwrap()
                .as_slice(),
            state
                .remote_identity_key()
                .unwrap()
                .unwrap()
                .public_key()
                .public_key_bytes()
        );
        assert_eq!(state.get_sender_chain_key().unwrap().index(), 4);
        let key = store.get_sync_key(&[1]).await.unwrap().unwrap();
        assert_eq!(key.key_data, [0x11; 32]);
        assert_eq!(key.fingerprint, b"fp");
        assert_eq!(key.timestamp, 7);
        let hash = store.get_version("regular").await.unwrap().unwrap();
        assert_eq!(hash.version, 7);
        assert_eq!(hash.hash, [0x22; 128]);
        assert!(hash.mac_mismatch_fatal);
        assert!(!hash.bootstrapped, "older rows must request bootstrap once");
        assert_eq!(
            store
                .get_sent_message(JID, "retry")
                .await
                .unwrap()
                .as_deref(),
            Some(PAYLOAD)
        );
        assert_eq!(
            store
                .get_pending_inbound(JID, JID, "pending")
                .await
                .unwrap()
                .as_deref(),
            Some(PAYLOAD)
        );
        assert!(
            store
                .get_sent_message(OLD_JID, "retry")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .get_pending_inbound(OLD_JID, OLD_JID, "pending")
                .await
                .unwrap()
                .is_none()
        );
    }
    let secret = db
        .store(1)
        .get_stored_msg_secret(JID, JID, "secret")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(secret.message_ts, Some(1_700_000_000));
}

struct SenderStore(SqliteStore);

#[async_trait::async_trait]
impl SenderKeyStore for SenderStore {
    async fn load_sender_key(
        &self,
        _: &SenderKeyName,
    ) -> wacore::libsignal::protocol::error::Result<Option<SenderKeyRecord>> {
        self.0
            .get_sender_key(SENDER_ADDRESS)
            .await
            .unwrap()
            .map(|bytes| SenderKeyRecord::deserialize(&bytes))
            .transpose()
    }

    async fn store_sender_key(
        &mut self,
        _: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> wacore::libsignal::protocol::error::Result<()> {
        self.0
            .put_sender_key(SENDER_ADDRESS, &record.serialize()?)
            .await
            .unwrap();
        Ok(())
    }
}

async fn send_and_decrypt(db: &SqliteDatabase) -> u32 {
    let name = SenderKeyName::from_parts("120363000000080001@g.us", SIGNAL_ADDRESS);
    let mut sender = SenderStore(db.store(1));
    let mut receiver = SenderStore(db.store(2));
    let mut rng = rand::rngs::StdRng::seed_from_u64(0x00A1_2071);
    let message = group_encrypt(&mut sender, &name, PAYLOAD, &mut rng)
        .await
        .unwrap();
    assert_eq!(
        group_decrypt(message.serialized(), &mut receiver, &name)
            .await
            .unwrap(),
        PAYLOAD
    );
    message.iteration()
}

#[tokio::test]
async fn v070_schema_upgrade_and_restart_preserve_accounts_and_pending_work() {
    let fixture = Fixture::new();
    let seed = seed_baseline(&fixture);
    let mut previous_iteration = None;
    for _ in 0..2 {
        let db = SqliteDatabase::open(fixture.url(), SqliteDatabaseConfig::default())
            .await
            .unwrap();
        verify(&db, &seed).await;
        let iteration = send_and_decrypt(&db).await;
        if let Some(previous) = previous_iteration {
            assert!(
                iteration > previous,
                "reopening cannot reuse a sender counter"
            );
        }
        previous_iteration = Some(iteration);
        // Handles and pools are dropped before the next independent open.
    }
    let db = SqliteDatabase::open(fixture.url(), SqliteDatabaseConfig::default())
        .await
        .unwrap();
    let first = db.store(1);
    first
        .delete_pending_inbound(JID, JID, "pending")
        .await
        .unwrap();
    assert_eq!(
        first
            .take_sent_message(JID, "retry")
            .await
            .unwrap()
            .as_deref(),
        Some(PAYLOAD)
    );
    drop(first);
    drop(db);
    let db = SqliteDatabase::open(fixture.url(), SqliteDatabaseConfig::default())
        .await
        .unwrap();
    let first = db.store(1);
    assert!(
        first
            .get_sent_message(JID, "retry")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        first
            .get_pending_inbound(JID, JID, "pending")
            .await
            .unwrap()
            .is_none()
    );
    let second = db.store(2);
    assert_eq!(
        second
            .get_sent_message(JID, "retry")
            .await
            .unwrap()
            .as_deref(),
        Some(PAYLOAD)
    );
    assert_eq!(
        second
            .get_pending_inbound(JID, JID, "pending")
            .await
            .unwrap()
            .as_deref(),
        Some(PAYLOAD)
    );
    let mut conn = SqliteConnection::establish(fixture.url()).unwrap();
    assert!(!conn.has_pending_migration(MIGRATIONS).unwrap());
}
