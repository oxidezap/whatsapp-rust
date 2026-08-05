//! What the per-state XEdDSA memo in `SenderKeyState` is worth, and to whom.
//!
//! The memo caches a basepoint multiplication for the signing key and the
//! Edwards derivations for the verifier. It lives in the record, so it only
//! pays off for a caller that keeps the record between operations. These
//! benches contrast the two shapes: a store that hands back the cached record
//! (what this repository does) against one that rebuilds it from components on
//! every load. `component_round_trip` isolates the conversion a component-backed
//! store pays per operation, in both directions, so what is left of the
//! difference between the two group benches can be attributed to the cold memo.
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

/// Fixed seeds, distinct per call, so instruction counts depend on the code
/// rather than on which scalars the OS entropy happened to produce.
fn bench_rng() -> rand::rngs::StdRng {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CTR: AtomicU32 = AtomicU32::new(0);
    <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(
        0x5E4D_0000 + u64::from(CTR.fetch_add(1, Ordering::Relaxed)),
    )
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
    let mut rng = bench_rng();
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
    bencher.with_inputs(warm_pair).bench_refs(|(sender, _)| {
        let mut rng = bench_rng();
        black_box(
            futures::executor::block_on(group_encrypt(
                sender,
                &sender_key_name,
                b"payload",
                &mut rng,
            ))
            .expect("encrypt must succeed on every timed iteration"),
        )
    });
}

#[divan::bench]
fn group_encrypt_rebuilt_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(rebuilding_pair)
        .bench_refs(|(sender, _)| {
            let mut rng = bench_rng();
            black_box(
                futures::executor::block_on(group_encrypt(
                    sender,
                    &sender_key_name,
                    b"payload",
                    &mut rng,
                ))
                .expect("encrypt must succeed on every timed iteration"),
            )
        });
}

/// Ciphertext the decrypt benches consume, produced outside the timed region.
fn ciphertext(sender: &mut WarmStore, sender_key_name: &SenderKeyName) -> Vec<u8> {
    let mut rng = bench_rng();
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

/// Both conversions a component-backed store pays per operation: `from_components`
/// on load and `into_components` on store. Subtract this from the rebuilt-record
/// benches to attribute what is left to the cold memo. Measuring only the load
/// side would leave the export attributed to re-derivation.
#[divan::bench]
fn component_round_trip(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| rebuilding_pair().0)
        .bench_refs(|store| {
            futures::executor::block_on(async {
                let record = store
                    .load_sender_key(&sender_key_name)
                    .await
                    .expect("load")
                    .expect("record present");
                store
                    .store_sender_key(&sender_key_name, black_box(record))
                    .await
                    .expect("store");
            })
        });
}
