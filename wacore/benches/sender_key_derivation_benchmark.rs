//! What the per-state XEdDSA memo in `SenderKeyState` is worth, and to whom.
//!
//! The memo caches a basepoint multiplication for the signing key and the
//! Edwards derivations for the verifier. It lives in the record, so it only
//! pays off for a caller that keeps the record between operations. These
//! benches contrast the two shapes: a store that hands back the cached record
//! (what this repository does) against one that rebuilds it from components on
//! every load. The two `store_round_trip_*` controls bound how much of that gap
//! is the store shape rather than the memo; what remains is not decomposed
//! further, because doing so credibly needs a control per side and per
//! direction, and the answer does not turn on it.
//!
//! The decrypt benches unwrap: a replayed ciphertext would return
//! `DuplicatedMessage` and time the error path instead, which no assertion on a
//! `black_box`ed `Result` would catch.
//!
//! Both stores waive the counter lease. Without that, only the rebuilding side
//! materializes a reservation on export, and the roughly 63 chain-KDF steps
//! that costs land in the delta as if they were re-derivation.
//!
//! Pin a core when reading these locally (`taskset -c <n> <bench binary>`) and
//! compare `fastest`. Unpinned, the absolute figures move by 2x between runs
//! while the ratios hold, which is enough to invent an environment difference
//! that is not there. CI reads them by instruction count instead, which is why
//! the RNG below is fixed-seed: scalar bits change instruction counts, so an
//! entropy-seeded key would move the numbers with no code change.

use async_trait::async_trait;
use divan::Bencher;
use std::collections::HashMap;
use std::hint::black_box;
use wacore_libsignal::protocol::{
    SenderKeyName, SenderKeyRecord, SenderKeyRecordComponents, SenderKeyStore,
    create_sender_key_distribution_message, group_decrypt, group_encrypt,
    process_sender_key_distribution_message,
};

type SigResult<T> = wacore_libsignal::protocol::error::Result<T>;

/// Fixed per purpose, never from a shared counter: the compared benchmarks have
/// to see the same key material and the same signature nonce, or the delta
/// carries the cost difference between two sets of scalars. A counter would
/// also make every stream depend on how many benchmarks ran before it.
const FIXTURE_SEED: u64 = 0x5E4D_0001;
const OPERATION_SEED: u64 = 0x5E4D_0002;
const CIPHERTEXT_SEED: u64 = 0x5E4D_0003;

fn seeded(seed: u64) -> rand::rngs::StdRng {
    <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(seed)
}

fn main() {
    divan::main();
}

/// Keeps the record, so the memo warms once and every later operation reuses
/// it. This is the shape a long-lived record cache produces.
#[derive(Default)]
struct WarmStore(HashMap<SenderKeyName, SenderKeyRecord>);

#[async_trait]
impl SenderKeyStore for WarmStore {
    async fn store_sender_key(&mut self, n: &SenderKeyName, r: SenderKeyRecord) -> SigResult<()> {
        self.0.insert(n.clone(), r);
        Ok(())
    }
    async fn load_sender_key(&self, n: &SenderKeyName) -> SigResult<Option<SenderKeyRecord>> {
        let mut record = match self.0.get(n) {
            Some(record) => record.clone(),
            None => return Ok(None),
        };
        record.waive_counter_lease()?;
        Ok(Some(record))
    }
}

/// Persists components instead of the record, so every load rebuilds a state
/// whose memo is cold and has to re-derive.
#[derive(Default)]
struct RebuildingStore(HashMap<SenderKeyName, SenderKeyRecordComponents>);

#[async_trait]
impl SenderKeyStore for RebuildingStore {
    async fn store_sender_key(&mut self, n: &SenderKeyName, r: SenderKeyRecord) -> SigResult<()> {
        self.0.insert(n.clone(), r.into_components()?);
        Ok(())
    }
    async fn load_sender_key(&self, n: &SenderKeyName) -> SigResult<Option<SenderKeyRecord>> {
        let Some(components) = self.0.get(n) else {
            return Ok(None);
        };
        let mut record = SenderKeyRecord::from_components(components.clone())?;
        record.waive_counter_lease()?;
        Ok(Some(record))
    }
}

fn name() -> SenderKeyName {
    SenderKeyName::new("group@g.us".to_string(), "sender.0".to_string())
}

/// A sender that has already sent once (memo warm, chain past its first
/// advance) and a receiver that has already received once.
fn warm_pair() -> (WarmStore, WarmStore) {
    let mut rng = seeded(FIXTURE_SEED);
    let sender_key_name = name();
    let mut sender = WarmStore::default();
    let mut receiver = WarmStore::default();

    futures::executor::block_on(async {
        let skdm = create_sender_key_distribution_message(&sender_key_name, &mut sender, &mut rng)
            .await
            .expect("distribution message");
        process_sender_key_distribution_message(&sender_key_name, &skdm, &mut receiver)
            .await
            .expect("receiver processes it");
        let first = group_encrypt(&mut sender, &sender_key_name, b"warmup", &mut rng)
            .await
            .expect("first send warms the sender memo");
        group_decrypt(first.serialized(), &mut receiver, &sender_key_name)
            .await
            .expect("first receive warms the verifier memo");
    });

    (sender, receiver)
}

/// The same pair, with both sides persisting components.
fn rebuilding_pair() -> (RebuildingStore, RebuildingStore) {
    let (sender, receiver) = warm_pair();
    let convert = |store: WarmStore| {
        RebuildingStore(
            store
                .0
                .into_iter()
                .map(|(n, r)| (n, r.into_components().expect("export")))
                .collect(),
        )
    };
    (convert(sender), convert(receiver))
}

#[divan::bench]
fn group_encrypt_warm_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| (warm_pair().0, seeded(OPERATION_SEED)))
        .bench_refs(|(sender, rng)| {
            black_box(
                futures::executor::block_on(group_encrypt(
                    sender,
                    &sender_key_name,
                    b"payload",
                    rng,
                ))
                .expect("encrypt must succeed on every timed iteration"),
            )
        });
}

#[divan::bench]
fn group_encrypt_rebuilt_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| (rebuilding_pair().0, seeded(OPERATION_SEED)))
        .bench_refs(|(sender, rng)| {
            black_box(
                futures::executor::block_on(group_encrypt(
                    sender,
                    &sender_key_name,
                    b"payload",
                    rng,
                ))
                .expect("encrypt must succeed on every timed iteration"),
            )
        });
}

/// Ciphertext the decrypt benches consume, produced outside the timed region.
fn ciphertext(sender: &mut WarmStore, sender_key_name: &SenderKeyName) -> Vec<u8> {
    let mut rng = seeded(CIPHERTEXT_SEED);
    futures::executor::block_on(group_encrypt(sender, sender_key_name, b"payload", &mut rng))
        .expect("encrypt")
        .serialized()
        .to_vec()
}

#[divan::bench]
fn group_decrypt_warm_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| {
            let (mut sender, receiver) = warm_pair();
            let message = ciphertext(&mut sender, &name());
            (receiver, message)
        })
        .bench_refs(|(receiver, message)| {
            black_box(
                futures::executor::block_on(group_decrypt(message, receiver, &sender_key_name))
                    .expect("decrypt must succeed on every timed iteration"),
            )
        });
}

#[divan::bench]
fn group_decrypt_rebuilt_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| {
            let (mut sender, receiver) = warm_pair();
            let message = ciphertext(&mut sender, &name());
            let rebuilt = RebuildingStore(
                receiver
                    .0
                    .into_iter()
                    .map(|(n, r)| (n, r.into_components().expect("export")))
                    .collect(),
            );
            (rebuilt, message)
        })
        .bench_refs(|(receiver, message)| {
            black_box(
                futures::executor::block_on(group_decrypt(message, receiver, &sender_key_name))
                    .expect("decrypt must succeed on every timed iteration"),
            )
        });
}

/// Store overhead on the sender side, one control per shape, so the pair can be
/// differenced. `RebuildingStore` pays `from_components` plus `into_components`
/// on top of what `WarmStore` pays; subtracting the rebuilding control alone
/// would also remove the map lookup and handoff the warm path pays too. These
/// bound the encrypt comparison only: the receiver record carries no private
/// signing key, so its conversions are cheaper than the sender's.
#[divan::bench]
fn store_round_trip_warm(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| warm_pair().0)
        .bench_refs(|store| round_trip(store, &sender_key_name));
}

#[divan::bench]
fn store_round_trip_rebuilt(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| rebuilding_pair().0)
        .bench_refs(|store| round_trip(store, &sender_key_name));
}

fn round_trip<S: SenderKeyStore>(store: &mut S, sender_key_name: &SenderKeyName) {
    futures::executor::block_on(async {
        let record = store
            .load_sender_key(sender_key_name)
            .await
            .expect("load")
            .expect("record present");
        store
            .store_sender_key(sender_key_name, black_box(record))
            .await
            .expect("store");
    })
}
