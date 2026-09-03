//! Non-Signal store write and read shapes on a file-backed SQLite database.
//!
//! `signal_flush` covers the Signal tables; this is the rest of what a client
//! persists, in the shapes the client issues them, so the cost of a round
//! trip is visible where it is paid:
//!
//! - **Device registry writes**, one row per call vs. one transaction (the
//!   usync response shape), which is also where the per-row index cost of the
//!   table shows.
//! - **App-state patch commit**: the version, removed MACs and added MACs as
//!   three calls (what the sync loop used to issue) vs. `commit_patch`.
//! - **App-state MAC reads**, one call each vs. one `IN (...)`.
//! - **LID-PN**: the startup warm-up scan, and one mapping per call vs. a
//!   batch.
//! - **Message secrets**, one per call vs. a batch: the floor the write
//!   buffer degenerates to when the backend keeps up with the producer.
//!
//! One database per benchmark, opened once for the process and grown by every
//! iteration.

use divan::black_box;
use std::sync::{Arc, OnceLock};
use wacore::appstate::hash::HashState;
use wacore::appstate::processor::AppStateMutationMAC;
use wacore::store::traits::{
    AppSyncStore, DeviceInfo, DeviceListRecord, LidPnMappingEntry, MsgSecretEntry, MsgSecretStore,
    ProtocolStore,
};
use whatsapp_rust_sqlite_storage::SqliteStore;

fn main() {
    divan::main();
    for db in DBS.iter().filter_map(OnceLock::get) {
        db.remove_files();
    }
}

const N: &[usize] = &[1, 32, 256];

struct Db {
    runtime: tokio::runtime::Runtime,
    store: SqliteStore,
    path: std::path::PathBuf,
    next: portable_atomic::AtomicU64,
}

impl Db {
    fn open(tag: &str) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        let path =
            std::env::temp_dir().join(format!("wa-store-shapes-{}-{tag}.db", std::process::id()));
        remove_db_files(&path);
        let url = path.to_str().expect("utf-8 temp path").to_owned();
        let store = runtime
            .block_on(SqliteStore::new(&url))
            .expect("open file-backed store");
        // `lid_pn_mapping` references the device row the store stamps.
        runtime
            .block_on(wacore::store::traits::DeviceStore::create(&store))
            .expect("create device row");
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

    /// A fresh identifier, so rows are never reused across iterations.
    fn id(&self) -> u64 {
        self.next.fetch_add(1, portable_atomic::Ordering::Relaxed)
    }
}

fn remove_db_files(path: &std::path::Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(name);
    }
}

/// One database slot per benchmark, so no benchmark inherits another's rows.
const DB_SLOTS: usize = 11;
static DBS: [OnceLock<Db>; DB_SLOTS] = [const { OnceLock::new() }; DB_SLOTS];

fn db(slot: usize, tag: &str) -> &'static Db {
    DBS[slot].get_or_init(|| Db::open(tag))
}

// ---------- device registry ----------

fn registry_record(id: u64) -> DeviceListRecord {
    DeviceListRecord {
        user: Arc::from(format!("1000000{id:08}").as_str()),
        devices: vec![DeviceInfo::new(0, None), DeviceInfo::new(1, None)].into_boxed_slice(),
        timestamp: 1_700_000_000,
        phash: None,
        raw_id: None,
    }
}

/// One `update_device_list` per row: a permit, a `spawn_blocking` and a
/// commit each.
#[divan::bench(args = N)]
fn registry_put_each(bencher: divan::Bencher, n: usize) {
    let d = db(0, "registry-put-each");
    bencher
        .with_inputs(|| (0..n).map(|_| registry_record(d.id())).collect::<Vec<_>>())
        .bench_values(|records| {
            d.runtime.block_on(async {
                for record in records {
                    d.store.update_device_list(record).await.expect("put");
                }
            })
        });
}

/// The same rows in one transaction, the usync response shape.
#[divan::bench(args = N)]
fn registry_put_batch(bencher: divan::Bencher, n: usize) {
    let d = db(1, "registry-put-batch");
    bencher
        .with_inputs(|| (0..n).map(|_| registry_record(d.id())).collect::<Vec<_>>())
        .bench_values(|records| {
            d.runtime
                .block_on(d.store.update_device_lists(records))
                .expect("put batch")
        });
}

// ---------- app-state patch commit ----------

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

fn state(version: u64) -> HashState {
    HashState {
        version,
        hash: [0x11; 128],
        ..Default::default()
    }
}

/// A patch's inputs: its version and `n` removed plus `n` added MACs, with
/// a seed that keeps consecutive patches from touching the same rows.
fn patch_inputs(
    counter: &portable_atomic::AtomicU64,
    n: usize,
) -> (u64, Vec<Vec<u8>>, Vec<AppStateMutationMAC>) {
    let version = counter.fetch_add(1, portable_atomic::Ordering::Relaxed) + 1;
    let seed = (version % 250) as u8;
    let removed = macs(n, seed).into_iter().map(|m| m.index_mac).collect();
    (version, removed, macs(n, seed.wrapping_add(1)))
}

/// The per-patch persistence the sync loop used to issue: `set_version`,
/// `delete_mutation_macs`, `put_mutation_macs` — three permits, three
/// `spawn_blocking`, three commits.
#[divan::bench(args = N)]
fn appstate_patch_three_calls(bencher: divan::Bencher, n: usize) {
    let d = db(2, "patch-three");
    let counter = portable_atomic::AtomicU64::new(0);
    bencher
        .with_inputs(|| patch_inputs(&counter, n))
        .bench_values(|(version, removed, added)| {
            d.runtime.block_on(async {
                d.store
                    .set_version("regular", state(version))
                    .await
                    .expect("set_version");
                d.store
                    .delete_mutation_macs("regular", &removed)
                    .await
                    .expect("delete");
                d.store
                    .put_mutation_macs("regular", version, &added)
                    .await
                    .expect("put");
            })
        });
}

/// The same three writes as one `commit_patch`.
#[divan::bench(args = N)]
fn appstate_patch_commit(bencher: divan::Bencher, n: usize) {
    let d = db(3, "patch-commit");
    let counter = portable_atomic::AtomicU64::new(0);
    bencher
        .with_inputs(|| patch_inputs(&counter, n))
        .bench_values(|(version, removed, added)| {
            d.runtime
                .block_on(
                    d.store
                        .commit_patch("regular", state(version), &removed, &added),
                )
                .expect("commit_patch")
        });
}

// ---------- app-state MAC reads ----------

fn mac_keys(n: usize) -> Vec<[u8; 32]> {
    macs(n, 0)
        .into_iter()
        .map(|m| <[u8; 32]>::try_from(m.index_mac.as_slice()).expect("32-byte index mac"))
        .collect()
}

fn macs_read_db() -> &'static Db {
    static SEEDED: OnceLock<()> = OnceLock::new();
    let d = db(4, "macs-read");
    SEEDED.get_or_init(|| {
        d.runtime
            .block_on(d.store.put_mutation_macs("regular", 1, &macs(5_000, 0)))
            .expect("seed");
    });
    d
}

/// The read half of a patch: one `IN (...)` for its index MACs.
#[divan::bench(args = N)]
fn appstate_macs_read_batch(bencher: divan::Bencher, n: usize) {
    let d = macs_read_db();
    let keys = mac_keys(n);
    bencher.bench(|| {
        d.runtime
            .block_on(d.store.get_mutation_macs("regular", black_box(&keys)))
            .expect("read")
    });
}

/// The same MACs one call each, what the processor did before the batch.
#[divan::bench(args = N)]
fn appstate_macs_read_each(bencher: divan::Bencher, n: usize) {
    let d = macs_read_db();
    let keys = mac_keys(n);
    bencher.bench(|| {
        d.runtime.block_on(async {
            for key in &keys {
                black_box(
                    d.store
                        .get_mutation_mac("regular", key)
                        .await
                        .expect("read one"),
                );
            }
        })
    });
}

// ---------- lid-pn ----------

fn lid_entry(id: u64) -> LidPnMappingEntry {
    LidPnMappingEntry {
        lid: format!("1000000{id:08}"),
        phone_number: format!("1904555{:04}", id % 10_000),
        created_at: 1_700_000_000,
        updated_at: 1_700_000_000,
        learning_source: "usync".to_string(),
    }
}

/// The startup warm-up read: a scan of the whole `lid_pn_mapping` table, on
/// the write queue, for a store that knows this many contacts.
#[divan::bench(args = [500usize, 5_000])]
fn lid_pn_warm_up_scan(bencher: divan::Bencher, rows: usize) {
    let (slot, tag) = if rows == 500 {
        (5, "lidpn-500")
    } else {
        (6, "lidpn-5000")
    };
    let d = db(slot, tag);
    if d.next.load(portable_atomic::Ordering::Relaxed) == 0 {
        let entries: Vec<LidPnMappingEntry> = (0..rows).map(|_| lid_entry(d.id())).collect();
        d.runtime
            .block_on(d.store.put_lid_mappings(&entries))
            .expect("seed");
    }
    bencher.bench(|| {
        d.runtime
            .block_on(d.store.get_all_lid_mappings())
            .expect("scan")
    });
}

/// Learning one mapping per call vs. a batch: the live-learn write shape.
#[divan::bench(args = N)]
fn lid_pn_put_each(bencher: divan::Bencher, n: usize) {
    let d = db(7, "lidpn-put-each");
    bencher
        .with_inputs(|| (0..n).map(|_| lid_entry(d.id())).collect::<Vec<_>>())
        .bench_values(|entries| {
            d.runtime.block_on(async {
                for entry in &entries {
                    d.store.put_lid_mapping(entry).await.expect("put");
                }
            })
        });
}

#[divan::bench(args = N)]
fn lid_pn_put_batch(bencher: divan::Bencher, n: usize) {
    let d = db(8, "lidpn-put-batch");
    bencher
        .with_inputs(|| (0..n).map(|_| lid_entry(d.id())).collect::<Vec<_>>())
        .bench_values(|entries| {
            d.runtime
                .block_on(d.store.put_lid_mappings(black_box(&entries)))
                .expect("put batch")
        });
}

// ---------- message secrets ----------

fn secret_entry(id: u64) -> MsgSecretEntry {
    MsgSecretEntry {
        chat: Arc::from(format!("1904555{:04}@s.whatsapp.net", id % 10_000).as_str()),
        sender: Arc::from("100000000000002@lid"),
        msg_id: Arc::from(format!("MSG{id:016X}").as_str()),
        secret: [0x7A; wacore::reporting_token::MESSAGE_SECRET_SIZE],
        expires_at: 0,
        message_ts: 1_700_000_000,
    }
}

#[divan::bench(args = N)]
fn msg_secrets_put_batch(bencher: divan::Bencher, n: usize) {
    let d = db(9, "msg-secrets");
    bencher
        .with_inputs(|| (0..n).map(|_| secret_entry(d.id())).collect::<Vec<_>>())
        .bench_values(|entries| {
            d.runtime
                .block_on(d.store.put_msg_secrets(entries))
                .expect("put secrets")
        });
}

/// One backend call per secret: the floor the write-behind buffer degenerates
/// to when the backend keeps up with the producer.
#[divan::bench(args = N)]
fn msg_secrets_put_each(bencher: divan::Bencher, n: usize) {
    let d = db(10, "msg-secrets-each");
    bencher
        .with_inputs(|| (0..n).map(|_| secret_entry(d.id())).collect::<Vec<_>>())
        .bench_values(|entries| {
            d.runtime.block_on(async {
                for entry in entries {
                    d.store.put_msg_secrets(vec![entry]).await.expect("put one");
                }
            })
        });
}
