//! What the per-state XEdDSA memo in `SenderKeyState` is worth, and to whom.
//!
//! The memo caches a basepoint multiplication for the signing key and the
//! Edwards derivations for the verifier. It lives in the record, so it only
//! pays off for a caller that keeps the record between operations. These
//! benches contrast the two shapes: a store that hands back the cached record
//! (what this repository does) against one that rebuilds it from components on
//! every load. `rebuild_only` isolates the conversion cost so the difference
//! between the two group benches can be attributed.

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
        Ok(self.0.get(n).cloned())
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
        self.0
            .get(n)
            .map(|components| SenderKeyRecord::from_components(components.clone()))
            .transpose()
    }
}

fn name() -> SenderKeyName {
    SenderKeyName::new("group@g.us".to_string(), "sender.0".to_string())
}

/// A sender that has already sent once (memo warm, chain past its first
/// advance) and a receiver that has already received once.
fn warm_pair() -> (WarmStore, WarmStore) {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
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
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        black_box(futures::executor::block_on(group_encrypt(
            sender,
            &sender_key_name,
            b"payload",
            &mut rng,
        )))
    });
}

#[divan::bench]
fn group_encrypt_rebuilt_record(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(rebuilding_pair)
        .bench_refs(|(sender, _)| {
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            black_box(futures::executor::block_on(group_encrypt(
                sender,
                &sender_key_name,
                b"payload",
                &mut rng,
            )))
        });
}

/// Ciphertext the decrypt benches consume, produced outside the timed region.
fn ciphertext(sender: &mut WarmStore, sender_key_name: &SenderKeyName) -> Vec<u8> {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
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
            black_box(futures::executor::block_on(group_decrypt(
                message,
                receiver,
                &sender_key_name,
            )))
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
            black_box(futures::executor::block_on(group_decrypt(
                message,
                receiver,
                &sender_key_name,
            )))
        });
}

/// The component round trip on its own. Subtract this from the rebuilt-record
/// benches to attribute what is left to the cold memo.
#[divan::bench]
fn rebuild_only(bencher: Bencher) {
    let sender_key_name = name();
    bencher
        .with_inputs(|| rebuilding_pair().0)
        .bench_refs(|store| {
            black_box(futures::executor::block_on(
                store.load_sender_key(&sender_key_name),
            ))
        });
}
