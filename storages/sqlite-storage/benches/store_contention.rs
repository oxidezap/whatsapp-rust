//! Reads alongside a write burst, with a reader pool configured.
//! `get_group_metadata` routes through `read_query` (reader connection);
//! `get_devices` and `get_msg_secret_with_ts` are on the `ON_THE_WRITE_QUEUE`
//! allowlist and take the write permit instead. Each contended sample is one
//! burst, holding the permit before the read is issued, and one read, both
//! awaited, so the work per sample is fixed; see `read_under_write` for why a
//! background writer is not measurable here and how the two are ordered.
//!
//! The runtime is built and fully populated in `harness`, before anything is
//! measured, so the one iteration CodSpeed times cannot contain a thread the
//! store happened to need; see [`BLOCKING_THREADS`].

use divan::black_box;
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::Poll;
use std::time::Duration;
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

/// The blocking threads the pool may have, and the number it is given before
/// anything is measured.
///
/// A sample overlaps at most two database jobs: the burst holding the write
/// permit, and the read beside it on a reader connection. Everything else the
/// store does is one `spawn_blocking` at a time, and the async side is a
/// permit acquire and a dispatch, so a current-thread runtime is the whole
/// runtime this needs.
///
/// The cap and [`warm_blocking_pool`] exist because tokio creates a blocking
/// thread on demand and reaps it after ten seconds idle, while CodSpeed
/// measures **one** iteration per benchmark: a `pthread_create` — stack
/// `mmap`, `mprotect`, TLS allocation — landing in that iteration is charged
/// to the store and reads as a regression of several percent that no change
/// to the code caused. Two threads, created up front and never reaped, keep
/// every iteration measuring the same work.
const BLOCKING_THREADS: usize = 2;

/// How long an idle blocking thread is kept: longer than any benchmark run,
/// so the pool warmed below is the pool every measurement sees.
const THREAD_KEEP_ALIVE: Duration = Duration::from_secs(24 * 60 * 60);

/// Bring every blocking thread into existence before the first measurement.
///
/// The barrier is what forces distinct threads: tokio spawns one only when a
/// job arrives and no thread is idle, so the jobs have to overlap. Each job
/// here is dispatched while the previous one is parked on the barrier, so the
/// pool ends up with exactly [`BLOCKING_THREADS`] threads.
fn warm_blocking_pool(runtime: &tokio::runtime::Runtime) {
    let barrier = Arc::new(std::sync::Barrier::new(BLOCKING_THREADS));
    runtime.block_on(async {
        let jobs: Vec<_> = (0..BLOCKING_THREADS)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                tokio::task::spawn_blocking(move || {
                    barrier.wait();
                })
            })
            .collect();
        for job in jobs {
            job.await.expect("warm a blocking thread");
        }
    });
}

fn harness() -> &'static Harness {
    HARNESS.get_or_init(|| {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(BLOCKING_THREADS)
            .thread_keep_alive(THREAD_KEEP_ALIVE)
            .enable_all()
            .build()
            .expect("current-thread runtime");
        warm_blocking_pool(&runtime);
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

/// One read issued while one write burst holds the write permit, both
/// awaited.
///
/// A free-running writer thread is what this used to be, and CodSpeed's
/// deterministic instruments cannot measure it: they count instructions, not
/// time, so what a sample contained was however much of the writer's loop
/// happened to overlap the read, anywhere from none to a whole burst (a 100x
/// spread between two runs of the same code). Issuing exactly one burst per
/// sample makes the work per sample constant; the read's own cost, and any
/// serialization the store puts between the two, is the delta over the burst
/// alone (`write_burst_alone`) and over the idle read.
///
/// The burst is polled once before the read is issued, and that poll is what
/// orders them: the store's write path takes the write permit on its first
/// poll (it is free between samples, the previous burst having been awaited)
/// and only then parks on its blocking job. A read on the write queue thus
/// queues behind the burst, and a read on a reader connection runs beside a
/// write in progress, whatever the scheduler does with either task.
fn read_under_write<R>(
    h: &'static Harness,
    rows: &[AppStateMutationMAC],
    read: impl Future<Output = R>,
) -> R {
    h.runtime.block_on(async {
        let mut burst = pin!(h.store.put_mutation_macs("regular", 1, rows));
        let finished = poll_fn(|cx| Poll::Ready(burst.as_mut().poll(cx).is_ready())).await;
        assert!(!finished, "the burst finished before the read was issued");
        let out = read.await;
        burst.await.expect("write burst");
        out
    })
}

/// The row sets the bursts alternate between; see `BURST_SEEDS`.
fn burst_rows() -> impl Fn() -> Vec<AppStateMutationMAC> {
    let turn = AtomicUsize::new(0);
    move || {
        macs(
            BURST_ROWS,
            BURST_SEEDS[turn.fetch_add(1, Ordering::Relaxed) % 2],
        )
    }
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
fn write_burst_alone(bencher: divan::Bencher) {
    let h = harness();
    bencher.with_inputs(burst_rows()).bench_refs(|rows| {
        h.runtime
            .block_on(h.store.put_mutation_macs("regular", 1, rows))
            .expect("write burst")
    });
}

#[divan::bench]
fn read_query_read_under_write(bencher: divan::Bencher) {
    let h = harness();
    bencher.with_inputs(burst_rows()).bench_refs(|rows| {
        read_under_write(h, rows, async {
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
    bencher.with_inputs(burst_rows()).bench_refs(|rows| {
        read_under_write(h, rows, async {
            h.store.get_devices(black_box(USER)).await.expect("read")
        })
    });
}
