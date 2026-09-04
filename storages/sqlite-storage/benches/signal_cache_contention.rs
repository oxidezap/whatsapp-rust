//! Cache checkout, chain advance, restore and persistence cost, not message throughput.
//! The native report overlaps a finite flush with independent warm cache operations.
//! Divan measures a sequential fixed batch; simulated CPU is not a lock-wait metric.

use divan::black_box;
use rand::SeedableRng;
use std::future::poll_fn;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;
use wacore::libsignal::protocol::{
    ChainKey, IdentityKey, KeyPair, ProtocolAddress, RootKey, SessionCheckoutStoreResult,
    SessionRecord, SessionState,
};
use wacore::store::signal_cache::SignalStoreCache;
use wacore::time::Instant;
use whatsapp_rust_sqlite_storage::SqliteStore;

const FIXTURE_SEED: u64 = 0x51A6_5A17_EC4A_5E01;
const DEADLINE: Duration = Duration::from_secs(30);

fn remove_db_files(path: &Path) {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let _ = std::fs::remove_file(name);
    }
}

fn create_session(rng: &mut rand::rngs::StdRng) -> SessionRecord {
    let local = IdentityKey::new(KeyPair::generate(rng).public_key);
    let remote = IdentityKey::new(KeyPair::generate(rng).public_key);
    let base_key = KeyPair::generate(rng).public_key;
    let mut state = SessionState::new(3, &local, &remote, &RootKey::new([7; 32]), &base_key);
    state.set_sender_chain(&KeyPair::generate(rng), &ChainKey::new([11; 32], 0));
    SessionRecord::new(state)
}

fn chain_index(record: &SessionRecord) -> u32 {
    record
        .session_state()
        .expect("state")
        .get_sender_chain_key()
        .expect("chain")
        .index()
}

fn spend_session(record: &mut SessionRecord) {
    let chain = record
        .session_state()
        .expect("state")
        .get_sender_chain_key()
        .expect("chain");
    black_box(chain.message_keys().generate_keys());
    let next = chain.next_chain_key().expect("next chain key");
    record
        .session_state_mut()
        .expect("state")
        .set_sender_chain_key(&next)
        .expect("advance");
    if chain.index() >= record.reserved_sender_chain_index() {
        record.reserve_sender_chain_counters(chain.index());
    }
}

async fn cycle(
    cache: &SignalStoreCache,
    store: &SqliteStore,
    addr: &ProtocolAddress,
) -> (Duration, Duration) {
    let start = Instant::now();
    let (record, token) = cache.checkout_session(addr, store).await.expect("checkout");
    let checkout = start.elapsed();
    let mut record = record.expect("warm session");
    spend_session(&mut record);
    let start = Instant::now();
    // Match SessionCheckout::commit, including the queued restore on contention.
    match cache.restore_session_from_checkout(addr, record, token, true) {
        SessionCheckoutStoreResult::Stored => {}
        SessionCheckoutStoreResult::Pending(done) => {
            cache.complete_session_checkout().await;
            assert!(done.load(portable_atomic::Ordering::Acquire));
        }
        _ => panic!("restore rejected or unhandled"),
    }
    (checkout, start.elapsed())
}

struct Harness {
    runtime: tokio::runtime::Runtime,
    store: SqliteStore,
    cache: Arc<SignalStoreCache>,
    addresses: Vec<ProtocolAddress>,
    path: PathBuf,
}

impl Harness {
    fn new(chats: usize) -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(1)
            .thread_keep_alive(Duration::from_secs(86400))
            .enable_all()
            .build()
            .expect("runtime");
        let path =
            std::env::temp_dir().join(format!("wa-signal-cache-{}-{chats}.db", std::process::id()));
        remove_db_files(&path);
        let store = runtime
            .block_on(SqliteStore::new(path.to_str().expect("path")))
            .expect("store");
        let cache = Arc::new(SignalStoreCache::new());
        let mut rng = rand::rngs::StdRng::seed_from_u64(FIXTURE_SEED);
        let addresses: Vec<_> = (0..chats)
            .map(|i| ProtocolAddress::new(&format!("1555000{i:05}"), 1.into()))
            .collect();
        runtime.block_on(async {
            for addr in &addresses {
                cache.put_session(addr, create_session(&mut rng)).await;
            }
            cache.flush(&store).await.expect("warmup flush");
            // Prime one dirty advance, so even the first measured flush writes rows.
            for addr in &addresses {
                cycle(&cache, &store, addr).await;
            }
        });
        Self {
            runtime,
            store,
            cache,
            addresses,
            path,
        }
    }

    fn verify_persisted(&self, expected: u32) {
        self.runtime.block_on(async {
            assert!(!self.cache.needs_pre_wire_flush().await);
            // Exact reload in the same incarnation verifies the final flush, not cache hits.
            self.cache.clear_after_flush().await;
            for addr in &self.addresses {
                let (record, token) = self
                    .cache
                    .checkout_session(addr, &self.store)
                    .await
                    .expect("reload");
                assert_eq!(chain_index(&record.expect("persisted session")), expected);
                self.cache.cancel_session_checkout(addr, token);
            }
        });
    }

    fn close(self) {
        let Self {
            runtime,
            store,
            cache,
            addresses: _,
            path,
        } = self;
        drop(cache);
        drop(store);
        drop(runtime);
        remove_db_files(&path);
    }
}

fn percentile(samples: &mut [Duration], percent: usize) -> f64 {
    assert!(!samples.is_empty());
    samples.sort_unstable();
    let index = (samples.len() * percent).div_ceil(100).saturating_sub(1);
    samples[index].as_secs_f64() * 1_000_000.0
}

#[allow(clippy::print_stdout)]
fn report() {
    println!(
        "Cache operations only; timings include scheduling. Flush duration is not mutex hold time."
    );
    println!(
        "The no-overlap control persists once at the end; the overlap case persists each round plus final drain."
    );
    println!(
        "Early flushes completed before sibling operations were issued and did not overlap them."
    );
    println!(
        "| Round | Chats | Overlap attempted | Early flushes | Cache cycles/s | Checkout p50 us | Checkout p99 us | Restore p99 us | Flush-call p99 us |"
    );
    println!("|---|---|---|---|---|---|---|---|---|");
    for repetition in 0..3 {
        for chats in [1, 8, 32] {
            for overlap in [false, true] {
                let h = Harness::new(chats);
                let rounds = 100;
                let mut checkouts = Vec::with_capacity(chats * rounds);
                let mut restores = Vec::with_capacity(chats * rounds);
                let mut flushes = Vec::with_capacity(rounds + 1);
                let mut early_flushes = 0;
                let start = Instant::now();
                h.runtime.block_on(async {
                    tokio::time::timeout(DEADLINE, async {
                        for _ in 0..rounds {
                            let mut flush = pin!(h.cache.flush(&h.store));
                            let flush_start = Instant::now();
                            // A native blocking job can finish before its join handle is polled.
                            // Count that case instead of assuming every flush yields.
                            let early_duration = if overlap {
                                match poll_fn(|cx| Poll::Ready(flush.as_mut().poll(cx))).await {
                                    Poll::Ready(result) => {
                                        result.expect("early flush");
                                        early_flushes += 1;
                                        Some(flush_start.elapsed())
                                    }
                                    Poll::Pending => None,
                                }
                            } else {
                                None
                            };
                            let tasks: Vec<_> = h
                                .addresses
                                .iter()
                                .map(|addr| {
                                    let addr = addr.clone();
                                    let cache = Arc::clone(&h.cache);
                                    let store = h.store.clone();
                                    tokio::spawn(async move { cycle(&cache, &store, &addr).await })
                                })
                                .collect();
                            if overlap {
                                match early_duration {
                                    Some(duration) => flushes.push(duration),
                                    None => {
                                        flush.await.expect("round flush");
                                        flushes.push(flush_start.elapsed());
                                    }
                                }
                            }
                            for task in tasks {
                                let (checkout, restore) = task.await.expect("task");
                                checkouts.push(checkout);
                                restores.push(restore);
                            }
                        }
                        let final_start = Instant::now();
                        h.cache.flush(&h.store).await.expect("final drain");
                        flushes.push(final_start.elapsed());
                    })
                    .await
                    .expect("workload timeout");
                });
                let elapsed = start.elapsed();
                h.verify_persisted(u32::try_from(rounds + 1).expect("counter"));
                println!(
                    "| {} | {chats} | {overlap} | {early_flushes} | {:.0} | {:.2} | {:.2} | {:.2} | {:.2} |",
                    repetition + 1,
                    (chats * rounds) as f64 / elapsed.as_secs_f64(),
                    percentile(&mut checkouts, 50),
                    percentile(&mut checkouts, 99),
                    percentile(&mut restores, 99),
                    percentile(&mut flushes, 99)
                );
                h.close();
            }
        }
    }
}

/// Sequential cache cycles followed by a fixed flush; no concurrency claim.
#[divan::bench(args = [1, 8, 32])]
fn signal_cache_batch_flush(bencher: divan::Bencher, chats: usize) {
    let h = Harness::new(chats);
    let mut advances = 1u32;
    bencher.bench_local(|| {
        h.runtime.block_on(async {
            for addr in &h.addresses {
                cycle(&h.cache, &h.store, black_box(addr)).await;
            }
            h.cache.flush(&h.store).await.expect("flush");
        });
        advances += 1;
    });
    h.verify_persisted(advances);
    h.close();
}

fn main() {
    if std::env::args().any(|arg| arg == "--report") {
        report();
    } else {
        divan::main();
    }
}
