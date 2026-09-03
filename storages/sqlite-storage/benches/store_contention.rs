//! Reads alongside a write burst, with a reader pool configured.
//! `get_group_metadata` routes through `read_query` (reader connection);
//! `get_devices` and `get_msg_secret_with_ts` are on the `ON_THE_WRITE_QUEUE`
//! allowlist and take the write permit instead. Each contended sample is one
//! burst and one read, both awaited, so the work per sample is fixed; see
//! `read_under_write` for why a background writer is not measurable here.

use divan::black_box;
use std::future::Future;
use std::sync::Arc;
use std::sync::OnceLock;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::traits::{AppSyncStore, DeviceInfo, DeviceListRecord, ProtocolStore};
use whatsapp_rust_sqlite_storage::{SqliteStore, SqliteStoreConfig};

fn main() {
    divan::main();
    if let Some(h) = HARNESS.get() {
        remove_db_files(&h.path);
    }
}

struct Harness {
    runtime: tokio::runtime::Runtime,
    store: SqliteStore,
    path: std::path::PathBuf,
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
            for seed in BURST_SEEDS {
                store
                    .put_mutation_macs("regular", 1, &macs(BURST_ROWS, seed))
                    .await
                    .expect("seed burst rows");
            }
        });
        Harness {
            runtime,
            store,
            path,
        }
    })
}

/// The write burst a snapshot apply or a flush has: a 2 000-row MAC upsert.
/// Two fixed row sets, alternated, so every measured burst after the seeding
/// in `harness` is an upsert of rows that exist: the same work each time,
/// not a table that grows with the iteration count.
const BURST_ROWS: usize = 2_000;
const BURST_SEEDS: [u8; 2] = [1, 2];

fn write_burst(h: &'static Harness, seed: u8) -> tokio::task::JoinHandle<()> {
    h.runtime.spawn(async move {
        h.store
            .put_mutation_macs("regular", 1, &macs(BURST_ROWS, seed))
            .await
            .expect("write burst");
    })
}

/// One read issued while one write burst is in flight, both awaited.
///
/// A free-running writer thread is what this used to be, and CodSpeed's
/// deterministic instruments cannot measure it: they count instructions, not
/// time, so what a sample contained was however much of the writer's loop
/// happened to overlap the read, anywhere from none to a whole burst (a 100x
/// spread between two runs of the same code). Issuing exactly one burst per
/// sample makes the work per sample constant; the read's own cost, and any
/// serialization the store puts between the two, is the delta over the burst
/// alone (`write_burst_alone`) and over the idle read.
fn read_under_write<R: Send + 'static>(
    h: &'static Harness,
    seed: u8,
    read: impl Future<Output = R>,
) -> R {
    h.runtime.block_on(async {
        let burst = write_burst(h, seed);
        let out = read.await;
        burst.await.expect("burst task");
        out
    })
}

#[divan::bench]
fn write_burst_alone(bencher: divan::Bencher) {
    let h = harness();
    let mut turn = 0usize;
    bencher.bench_local(|| {
        turn += 1;
        h.runtime
            .block_on(write_burst(h, BURST_SEEDS[turn % 2]))
            .expect("burst task")
    });
}

#[divan::bench]
fn read_query_read_under_write(bencher: divan::Bencher) {
    let h = harness();
    let mut turn = 0usize;
    bencher.bench_local(|| {
        turn += 1;
        read_under_write(h, BURST_SEEDS[turn % 2], async {
            h.store
                .get_group_metadata(black_box(GROUP))
                .await
                .expect("read")
        })
    });
}

#[divan::bench]
fn write_queue_read_under_write(bencher: divan::Bencher) {
    let h = harness();
    let mut turn = 0usize;
    bencher.bench_local(|| {
        turn += 1;
        read_under_write(h, BURST_SEEDS[turn % 2], async {
            h.store.get_devices(black_box(USER)).await.expect("read")
        })
    });
}
