//! Signal-store write cost on a file-backed SQLite database.
//!
//! The client-level flush benchmark stores through `InMemoryBackend`, so the
//! storage engine's own cost per flushed row is excluded there by
//! construction. This is the other half: what one `SignalStore` write costs
//! against a real database file, with WAL and the crate's pragmas, for the
//! shapes the flush actually issues.
//!
//! - **Insert.** `put_sessions_batch` of fresh addresses: a first contact or
//!   an offline drain establishing sessions. An upsert that only changes the
//!   record touches no index, so fresh rows are what make an index's write
//!   cost visible.
//! - **Delete, per row vs. batched.** The flush used to delete one address
//!   per backend call, each a `spawn_blocking`, a pool checkout and a WAL
//!   commit; the batched form is one transaction.
//!
//! One database per benchmark, opened once for the process and grown by every
//! iteration: rows are never reused, so an insert is always an insert.

use bytes::Bytes;
use divan::black_box;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use wacore::store::traits::SignalStore;
use whatsapp_rust_sqlite_storage::SqliteStore;

fn main() {
    divan::main();
    for db in DBS.iter().filter_map(OnceLock::get) {
        db.remove_files();
    }
}

const BATCH_SIZES: &[usize] = &[1, 8, 64];

/// A flushed session record is a few hundred bytes; the size only has to be
/// realistic enough that the row write is not dominated by the key.
const RECORD_LEN: usize = 256;

struct Db {
    runtime: tokio::runtime::Runtime,
    store: SqliteStore,
    path: std::path::PathBuf,
    next: portable_atomic::AtomicU64,
}

impl Db {
    fn open(tag: &str) -> Self {
        // Every store call is one `spawn_blocking`. Tokio reaps an idle
        // blocking thread after ten seconds and creates a fresh one on the
        // next call, and each database sits idle while the other benchmarks
        // in this binary run — so a `pthread_create` (stack `mmap`, TLS
        // setup) can land inside a measured iteration and read as a
        // regression that has nothing to do with the store. One thread, kept
        // for the life of the process, keeps every iteration measuring the
        // same work.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .max_blocking_threads(1)
            .thread_keep_alive(Duration::from_secs(24 * 60 * 60))
            .enable_all()
            .build()
            .expect("current-thread runtime");
        let path =
            std::env::temp_dir().join(format!("wa-signal-flush-{}-{tag}.db", std::process::id()));
        remove_db_files(&path);
        let url = path.to_str().expect("utf-8 temp path").to_owned();
        let store = runtime
            .block_on(SqliteStore::new(&url))
            .expect("open file-backed store");
        Self {
            runtime,
            store,
            path,
            next: portable_atomic::AtomicU64::new(0),
        }
    }

    fn remove_files(&self) {
        remove_db_files(&self.path);
    }

    /// `n` addresses this database has never seen, with a record each.
    fn fresh_sessions(&self, n: usize) -> Vec<(Arc<str>, Bytes)> {
        let record = Bytes::from(vec![0x5A; RECORD_LEN]);
        (0..n)
            .map(|_| {
                let id = self.next.fetch_add(1, portable_atomic::Ordering::Relaxed);
                (
                    Arc::from(format!("1000000{id:08}@lid").as_str()),
                    record.clone(),
                )
            })
            .collect()
    }

    fn fresh_prekeys(&self, n: usize) -> Vec<(u32, Bytes)> {
        let record = Bytes::from(vec![0x3C; 64]);
        (0..n)
            .map(|_| {
                let id = self.next.fetch_add(1, portable_atomic::Ordering::Relaxed);
                (
                    u32::try_from(id + 1).expect("prekey id fits"),
                    record.clone(),
                )
            })
            .collect()
    }

    fn inserted_sessions(&self, n: usize) -> Vec<Arc<str>> {
        let batch = self.fresh_sessions(n);
        self.runtime
            .block_on(self.store.put_sessions_batch(&batch))
            .expect("seed sessions");
        batch.into_iter().map(|(address, _)| address).collect()
    }

    fn inserted_prekeys(&self, n: usize) -> Vec<u32> {
        let batch = self.fresh_prekeys(n);
        self.runtime
            .block_on(self.store.store_prekeys_batch(&batch, true))
            .expect("seed prekeys");
        batch.into_iter().map(|(id, _)| id).collect()
    }
}

fn remove_db_files(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(name);
    }
}

const DB_SLOTS: usize = 7;
static DBS: [OnceLock<Db>; DB_SLOTS] = [const { OnceLock::new() }; DB_SLOTS];

fn db(slot: usize, tag: &str) -> &'static Db {
    DBS[slot].get_or_init(|| Db::open(tag))
}

/// Fresh rows through the batched upsert, the shape a flush writes new
/// sessions in.
#[divan::bench(args = BATCH_SIZES)]
fn insert_sessions_batch(bencher: divan::Bencher, n: usize) {
    let db = db(0, "insert");
    bencher
        .with_inputs(|| db.fresh_sessions(n))
        .bench_values(|batch| {
            db.runtime
                .block_on(db.store.put_sessions_batch(black_box(&batch)))
                .expect("insert batch");
        });
}

/// One backend call per address: what the flush issued before batching.
#[divan::bench(args = BATCH_SIZES)]
fn delete_sessions_each(bencher: divan::Bencher, n: usize) {
    let db = db(1, "delete-each");
    bencher
        .with_inputs(|| db.inserted_sessions(n))
        .bench_values(|addresses| {
            db.runtime.block_on(async {
                for address in &addresses {
                    db.store
                        .delete_session(black_box(address))
                        .await
                        .expect("delete session");
                }
            });
        });
}

/// The same rows through one transaction.
#[divan::bench(args = BATCH_SIZES)]
fn delete_sessions_batch(bencher: divan::Bencher, n: usize) {
    let db = db(2, "delete-batch");
    bencher
        .with_inputs(|| db.inserted_sessions(n))
        .bench_values(|addresses| {
            db.runtime
                .block_on(db.store.delete_sessions_batch(black_box(&addresses)))
                .expect("delete batch");
        });
}

/// Consumed one-time pre-keys, one call each: an offline drain of `n` `pkmsg`s.
#[divan::bench(args = BATCH_SIZES)]
fn remove_prekeys_each(bencher: divan::Bencher, n: usize) {
    let db = db(3, "prekeys-each");
    bencher
        .with_inputs(|| db.inserted_prekeys(n))
        .bench_values(|ids| {
            db.runtime.block_on(async {
                for id in &ids {
                    db.store
                        .remove_prekey(black_box(*id))
                        .await
                        .expect("remove prekey");
                }
            });
        });
}

/// The same pre-keys through one transaction.
#[divan::bench(args = BATCH_SIZES)]
fn remove_prekeys_batch(bencher: divan::Bencher, n: usize) {
    let db = db(4, "prekeys-batch");
    bencher
        .with_inputs(|| db.inserted_prekeys(n))
        .bench_values(|ids| {
            db.runtime
                .block_on(db.store.remove_prekeys_batch(black_box(&ids)))
                .expect("remove batch");
        });
}

/// One `get_session` per device: the N+1 a group send pays on its session
/// checkout, with no batch API to fold it into. The number a future
/// `load_sessions_batch` has to beat.
#[divan::bench(args = BATCH_SIZES)]
fn get_session_hit(bencher: divan::Bencher, n: usize) {
    let db = db(5, "get-hit");
    bencher
        .with_inputs(|| db.inserted_sessions(n))
        .bench_values(|addresses| {
            db.runtime.block_on(async {
                for address in &addresses {
                    black_box(
                        db.store
                            .get_session(black_box(address))
                            .await
                            .expect("load session"),
                    );
                }
            });
        });
}

/// Same, for addresses never written: the miss a first-contact send pays.
#[divan::bench(args = BATCH_SIZES)]
fn get_session_miss(bencher: divan::Bencher, n: usize) {
    let db = db(6, "get-miss");
    bencher
        .with_inputs(|| db.fresh_sessions(n))
        .bench_values(|batch| {
            db.runtime.block_on(async {
                for (address, _) in &batch {
                    black_box(
                        db.store
                            .get_session(black_box(address))
                            .await
                            .expect("load miss"),
                    );
                }
            });
        });
}
