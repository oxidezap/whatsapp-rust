use bytes::Bytes;
use divan::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;
use wacore::store::traits::{
    MsgSecretEntry, MsgSecretStore, ProtocolStore, SignalStore, TcTokenEntry,
};
use whatsapp_rust_sqlite_storage::SqliteStore;

fn main() {
    divan::main();
}

struct Fixture {
    runtime: tokio::runtime::Runtime,
    store: Option<SqliteStore>,
    path: PathBuf,
}

impl Fixture {
    fn new(storage: &str) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .thread_keep_alive(Duration::from_secs(3600))
            .enable_all()
            .build()
            .expect("runtime");
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/sqlite-prepared-bench");
        std::fs::create_dir_all(&dir).expect("scratch directory");
        let path = dir.join(format!("{}.db", std::process::id()));
        let url = match storage {
            "memory" => format!(
                "file:prepared_{}?mode=memory&cache=shared",
                std::process::id()
            ),
            "file" => path.to_str().expect("database path").to_owned(),
            _ => unreachable!(),
        };
        let store = runtime.block_on(SqliteStore::open(&url)).expect("open");
        Self {
            runtime,
            store: Some(store),
            path,
        }
    }

    fn store(&self) -> &SqliteStore {
        self.store.as_ref().expect("live fixture")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        drop(self.store.take());
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.path.as_os_str().to_owned();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn token_lookup(bencher: divan::Bencher, storage: &str, hit: bool) {
    let fixture = Fixture::new(storage);
    let store = fixture.store();
    fixture
        .runtime
        .block_on(store.put_tc_token(
            "15550000001",
            &TcTokenEntry {
                token: vec![0, 255, 42],
                token_timestamp: 1_700_000_000,
                sender_timestamp: None,
            },
        ))
        .expect("seed token");
    let key = if hit { "15550000001" } else { "15550000002" };
    bencher.bench(|| {
        black_box(
            fixture
                .runtime
                .block_on(store.get_tc_token(black_box(key)))
                .expect("lookup"),
        )
    });
}

#[divan::bench(args = ["memory", "file"])]
fn token_hit(bencher: divan::Bencher, storage: &str) {
    token_lookup(bencher, storage, true);
}

#[divan::bench(args = ["memory", "file"])]
fn token_miss(bencher: divan::Bencher, storage: &str) {
    token_lookup(bencher, storage, false);
}

fn secrets(count: usize, generation: u64) -> Vec<MsgSecretEntry> {
    (0..count)
        .map(|i| MsgSecretEntry {
            chat: "15550000001@s.whatsapp.net".into(),
            sender: "15550000002@lid".into(),
            msg_id: format!("SYNTHETIC-{generation}-{i}").into(),
            secret: [42; 32],
            expires_at: 0,
            message_ts: 1_700_000_000,
        })
        .collect()
}

fn secret_write(bencher: divan::Bencher, storage: &str, count: usize, update: bool) {
    let fixture = Fixture::new(storage);
    let store = fixture.store();
    fixture
        .runtime
        .block_on(store.put_msg_secrets(secrets(count, 0)))
        .expect("seed secrets");
    let generation = AtomicU64::new(0);
    bencher
        .with_inputs(|| {
            let id = if update {
                0
            } else {
                generation.fetch_add(1, Ordering::Relaxed) + 1
            };
            let mut entries = secrets(count, id);
            for entry in &mut entries {
                entry.secret = [43; 32];
            }
            entries
        })
        .bench_values(|entries| {
            black_box(
                fixture
                    .runtime
                    .block_on(store.put_msg_secrets(entries))
                    .expect("write secrets"),
            )
        });
}

#[divan::bench(args = ["memory", "file"])]
fn secret_insert(bencher: divan::Bencher, storage: &str) {
    secret_write(bencher, storage, 1, false);
}
#[divan::bench(args = ["memory", "file"])]
fn secret_update(bencher: divan::Bencher, storage: &str) {
    secret_write(bencher, storage, 1, true);
}
#[divan::bench(args = ["memory", "file"])]
fn secret_bulk_32(bencher: divan::Bencher, storage: &str) {
    secret_write(bencher, storage, 32, false);
}

fn prekey_write(bencher: divan::Bencher, storage: &str, update: bool) {
    let fixture = Fixture::new(storage);
    let store = fixture.store();
    let next = AtomicU32::new(812);
    let seeded: Vec<_> = (1..=812)
        .map(|id| (id, Bytes::from_static(&[42; 64])))
        .collect();
    fixture
        .runtime
        .block_on(store.store_prekeys_batch(&seeded, false))
        .expect("seed prekeys");
    bencher
        .with_inputs(|| {
            let base = if update {
                0
            } else {
                next.fetch_add(812, Ordering::Relaxed)
            };
            (0..812)
                .map(|i| {
                    let id = base + i + 1;
                    (id, Bytes::from_static(&[43; 64]))
                })
                .collect::<Vec<_>>()
        })
        .bench_values(|entries| {
            fixture
                .runtime
                .block_on(store.store_prekeys_batch(black_box(&entries), true))
                .expect("write prekeys")
        });
}

#[divan::bench(args = ["memory", "file"])]
fn prekeys_insert_812(bencher: divan::Bencher, storage: &str) {
    prekey_write(bencher, storage, false);
}
#[divan::bench(args = ["memory", "file"])]
fn prekeys_update_812(bencher: divan::Bencher, storage: &str) {
    prekey_write(bencher, storage, true);
}
