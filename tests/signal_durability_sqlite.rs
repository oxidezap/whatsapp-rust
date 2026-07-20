#![cfg(feature = "sqlite-storage")]

use std::path::Path;
use std::process::Command;

use rand::SeedableRng;
use wacore::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH;
use wacore::libsignal::protocol::{
    ChainKey, IdentityKey, KeyPair, RootKey, SenderKeyRecord, SenderKeyStore, SessionRecord,
    SessionState, create_sender_key_distribution_message, group_encrypt,
};
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::store::signal_cache::SignalStoreCache;

const CHILD_MARKER: &str = "SIGNAL_DURABILITY_CRASH_CHILD";
const DATABASE_ENV: &str = "SIGNAL_DURABILITY_DATABASE";
#[cfg(not(unix))]
const CHILD_EXIT_CODE: i32 = 91;
const FIXTURE_SEED: u64 = 0x51A6_5A17_EC4A_5E01;

type DmFingerprint = ([u8; 32], [u8; 16]);

struct CachedSenderKeyStore<'a> {
    cache: &'a SignalStoreCache,
    backend: &'a SqliteStore,
}

#[async_trait::async_trait]
impl SenderKeyStore for CachedSenderKeyStore<'_> {
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> wacore::libsignal::protocol::error::Result<()> {
        self.cache.put_sender_key(name, record).await;
        Ok(())
    }

    async fn load_sender_key(
        &self,
        name: &SenderKeyName,
    ) -> wacore::libsignal::protocol::error::Result<Option<SenderKeyRecord>> {
        Ok(self
            .cache
            .get_sender_key(name, self.backend)
            .await
            .map_err(|error| {
                wacore::libsignal::protocol::SignalProtocolError::BackendError(
                    "SQLite durability test",
                    error.into(),
                )
            })?
            .map(|record| (*record).clone()))
    }
}

fn dm_address() -> wacore::libsignal::protocol::ProtocolAddress {
    wacore::libsignal::protocol::ProtocolAddress::new("15550008001".to_string(), 1.into())
}

fn group_name() -> SenderKeyName {
    SenderKeyName::from_parts("120363000000080001@g.us", "15550008002@s.whatsapp.net:0")
}

fn fresh_session(rng: &mut rand::rngs::StdRng) -> SessionRecord {
    let local = IdentityKey::new(KeyPair::generate(rng).public_key);
    let remote = IdentityKey::new(KeyPair::generate(rng).public_key);
    let base_key = KeyPair::generate(rng).public_key;
    let mut state = SessionState::new(3, &local, &remote, &RootKey::new([7; 32]), &base_key);
    state.set_sender_chain(&KeyPair::generate(rng), &ChainKey::new([11; 32], 0));
    SessionRecord::new(state)
}

fn spend_dm(record: &mut SessionRecord) -> (u32, DmFingerprint) {
    let chain = record
        .session_state()
        .expect("session state")
        .get_sender_chain_key()
        .expect("sender chain");
    let keys = chain.message_keys().generate_keys();
    let fingerprint = (*keys.cipher_key(), *keys.iv());
    let next = chain.next_chain_key().expect("next chain key");
    record
        .session_state_mut()
        .expect("session state")
        .set_sender_chain_key(&next)
        .expect("sender chain update");
    if chain.index() >= record.reserved_sender_chain_index() {
        record.reserve_sender_chain_counters(chain.index());
    }
    (chain.index(), fingerprint)
}

async fn crash_child(database: &str) -> ! {
    let store = SqliteStore::new(database).await.expect("SQLite store");
    let cache = SignalStoreCache::new();
    let address = dm_address();
    let name = group_name();
    let mut rng = rand::rngs::StdRng::seed_from_u64(FIXTURE_SEED);

    let mut dm = fresh_session(&mut rng);
    assert_eq!(spend_dm(&mut dm).0, 0);
    cache.put_session(&address, dm).await;

    let mut sender = CachedSenderKeyStore {
        cache: &cache,
        backend: &store,
    };
    create_sender_key_distribution_message(&name, &mut sender, &mut rng)
        .await
        .expect("sender-key setup");
    let first = group_encrypt(&mut sender, &name, b"first", &mut rng)
        .await
        .expect("first group encrypt");
    assert_eq!(first.iteration(), 0);

    cache.flush(&store).await.expect("durable lease flush");
    assert!(!cache.needs_pre_wire_flush().await);

    let mut dm = cache
        .get_session(&address, &store)
        .await
        .expect("session load")
        .expect("session");
    for expected in 1..=5 {
        assert_eq!(spend_dm(&mut dm).0, expected);
    }
    cache.put_session(&address, dm).await;
    for expected in 1..=5 {
        let message = group_encrypt(&mut sender, &name, b"unflushed", &mut rng)
            .await
            .expect("group encrypt");
        assert_eq!(message.iteration(), expected);
    }

    crash_now()
}

#[cfg(unix)]
fn crash_now() -> ! {
    // SIGKILL prevents process-exit hooks from making the fixture cleaner than a real crash.
    let result = unsafe { libc::raise(libc::SIGKILL) };
    panic!("SIGKILL failed with {result}")
}

#[cfg(not(unix))]
fn crash_now() -> ! {
    std::process::exit(CHILD_EXIT_CODE)
}

async fn verify_recovery(database: &str) {
    let store = SqliteStore::new(database)
        .await
        .expect("reopen SQLite store");
    let cache = SignalStoreCache::new();
    let address = dm_address();
    let name = group_name();
    let mut rng = rand::rngs::StdRng::seed_from_u64(FIXTURE_SEED ^ 0xFFFF);

    let mut child_record = fresh_session(&mut rand::rngs::StdRng::seed_from_u64(FIXTURE_SEED));
    let mut child_fingerprints = Vec::with_capacity(6);
    for _ in 0..6 {
        child_fingerprints.push(spend_dm(&mut child_record).1);
    }

    let mut recovered = cache
        .get_session(&address, &store)
        .await
        .expect("recovery load")
        .expect("durable session");
    let (counter, fingerprint) = spend_dm(&mut recovered);
    assert_eq!(counter, SENDER_CHAIN_RESERVATION_BATCH);
    assert!(!child_fingerprints.contains(&fingerprint));
    cache.put_session(&address, recovered).await;

    let mut sender = CachedSenderKeyStore {
        cache: &cache,
        backend: &store,
    };
    let recovered_group = group_encrypt(&mut sender, &name, b"recovered", &mut rng)
        .await
        .expect("group recovery encrypt");
    assert_eq!(recovered_group.iteration(), SENDER_CHAIN_RESERVATION_BATCH);
    assert!(cache.needs_pre_wire_flush().await);
    cache.flush(&store).await.expect("recovery lease flush");
    assert!(!cache.needs_pre_wire_flush().await);

    cache.clear_after_flush().await;
    let mut exact = cache
        .get_session(&address, &store)
        .await
        .expect("clean reload")
        .expect("durable session");
    assert_eq!(spend_dm(&mut exact).0, SENDER_CHAIN_RESERVATION_BATCH + 1);
    cache.put_session(&address, exact).await;
    let exact_group = group_encrypt(&mut sender, &name, b"exact", &mut rng)
        .await
        .expect("clean group reload");
    assert_eq!(exact_group.iteration(), SENDER_CHAIN_RESERVATION_BATCH + 1);
    assert!(!cache.needs_pre_wire_flush().await);
}

fn remove_database(path: &Path) {
    let _ = std::fs::remove_file(path);
    let base = path.as_os_str().to_string_lossy();
    let _ = std::fs::remove_file(format!("{base}-wal"));
    let _ = std::fs::remove_file(format!("{base}-shm"));
}

#[tokio::test]
#[ignore = "run as a subprocess by the SQLite durability test"]
async fn signal_durability_sqlite_crash_child() {
    if std::env::var_os(CHILD_MARKER).is_none() {
        return;
    }
    let database = std::env::var(DATABASE_ENV).expect("child database path");
    crash_child(&database).await;
}

#[tokio::test]
#[ignore = "run by signal-durability-nightly.yml"]
async fn signal_durability_sqlite_process_restart() {
    let database = std::env::temp_dir().join(format!(
        "whatsapp-rust-signal-durability-{}.db",
        uuid::Uuid::new_v4()
    ));
    let executable = std::env::current_exe().expect("current test executable");
    let child_database = database.clone();
    let status = tokio::task::spawn_blocking(move || {
        Command::new(executable)
            .arg("signal_durability_sqlite_crash_child")
            .arg("--exact")
            .arg("--ignored")
            .arg("--nocapture")
            .env(CHILD_MARKER, "1")
            .env(DATABASE_ENV, child_database)
            .status()
    })
    .await
    .expect("child task")
    .expect("start child process");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(libc::SIGKILL));
    }
    #[cfg(not(unix))]
    assert_eq!(status.code(), Some(CHILD_EXIT_CODE));

    verify_recovery(database.to_str().expect("UTF-8 database path")).await;
    tokio::task::spawn_blocking(move || remove_database(&database))
        .await
        .expect("database cleanup task");
}
