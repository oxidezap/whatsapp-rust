//! Read latency while the single write permit is busy, with a reader pool
//! configured. `get_group_metadata` routes through `read_query` (reader
//! connection); `get_devices` and `get_msg_secret_with_ts` are on the
//! `ON_THE_WRITE_QUEUE` allowlist and take the write permit instead.

use divan::black_box;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::traits::{AppSyncStore, DeviceInfo, DeviceListRecord, ProtocolStore};
use whatsapp_rust_sqlite_storage::{SqliteStore, SqliteStoreConfig};

fn main() {
    divan::main();
    if let Some(h) = HARNESS.get() {
        h.stop.store(true, Ordering::Relaxed);
        remove_db_files(&h.path);
    }
}

struct Harness {
    runtime: tokio::runtime::Runtime,
    store: SqliteStore,
    path: std::path::PathBuf,
    stop: AtomicBool,
}

static HARNESS: OnceLock<Harness> = OnceLock::new();

fn remove_db_files(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(name);
    }
}

fn macs(n: usize, seed: u8) -> Vec<AppStateMutationMAC> {
    (0..n)
        .map(|i| {
            let mut index = [0u8; 32];
            index[..8].copy_from_slice(&(i as u64).to_be_bytes());
            index[8] = seed;
            AppStateMutationMAC {
                index_mac: index.to_vec(),
                value_mac: vec![0xC5; 32],
            }
        })
        .collect()
}

const USER: &str = "190455501800";
const GROUP: &str = "120363000000000001@g.us";

fn harness() -> &'static Harness {
    HARNESS.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("runtime");
        let path =
            std::env::temp_dir().join(format!("wa-store-contention-{}.db", std::process::id()));
        remove_db_files(&path);
        let url = path.to_str().expect("utf-8").to_owned();
        let store = runtime
            .block_on(SqliteStore::with_config(
                &url,
                SqliteStoreConfig::default().with_read_pool_size(4),
            ))
            .expect("open store");
        runtime.block_on(async {
            wacore::store::traits::DeviceStore::create(&store)
                .await
                .expect("device row");
            store
                .update_device_list(DeviceListRecord {
                    user: Arc::from(USER),
                    devices: vec![DeviceInfo::new(0, None), DeviceInfo::new(1, None)]
                        .into_boxed_slice(),
                    timestamp: 1_700_000_000,
                    phash: None,
                    raw_id: None,
                })
                .await
                .expect("seed registry");
            store
                .put_group_metadata(GROUP, &vec![0x2A; 4096])
                .await
                .expect("seed group");
        });
        Harness {
            runtime,
            store,
            path,
            stop: AtomicBool::new(false),
        }
    })
}

/// A writer that keeps the single write permit busy: 2 000-row MAC upserts
/// back to back, the shape a snapshot apply or a flush has.
fn start_writer(h: &'static Harness) -> tokio::task::JoinHandle<()> {
    h.stop.store(false, Ordering::Relaxed);
    h.runtime.spawn(async move {
        let mut seed = 0u8;
        while !h.stop.load(Ordering::Relaxed) {
            seed = seed.wrapping_add(1);
            let _ = h
                .store
                .put_mutation_macs("regular", 1, &macs(2_000, seed))
                .await;
        }
    })
}

#[divan::bench]
fn read_query_read_idle(bencher: divan::Bencher) {
    let h = harness();
    bencher.bench(|| {
        h.runtime
            .block_on(h.store.get_group_metadata(black_box(GROUP)))
            .expect("read")
    });
}

#[divan::bench]
fn write_queue_read_idle(bencher: divan::Bencher) {
    let h = harness();
    bencher.bench(|| {
        h.runtime
            .block_on(h.store.get_devices(black_box(USER)))
            .expect("read")
    });
}

#[divan::bench]
fn read_query_read_under_write(bencher: divan::Bencher) {
    let h = harness();
    let writer = start_writer(h);
    bencher.bench(|| {
        h.runtime
            .block_on(h.store.get_group_metadata(black_box(GROUP)))
            .expect("read")
    });
    h.stop.store(true, Ordering::Relaxed);
    let _ = h.runtime.block_on(writer);
}

#[divan::bench]
fn write_queue_read_under_write(bencher: divan::Bencher) {
    let h = harness();
    let writer = start_writer(h);
    bencher.bench(|| {
        h.runtime
            .block_on(h.store.get_devices(black_box(USER)))
            .expect("read")
    });
    h.stop.store(true, Ordering::Relaxed);
    let _ = h.runtime.block_on(writer);
}
