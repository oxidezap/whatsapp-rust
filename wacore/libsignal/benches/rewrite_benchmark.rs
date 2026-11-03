//! Synthetic, matched fixtures for the independent engine and legacy test oracle.
//! Run only through CodSpeed. Fixture construction is outside measured regions.
//! `load` measures cold codec allocation; `clone_live` measures warm retained state.
//! Allocation profiling describes native API allocations, not whole-process RSS.
//! Cold/warm describes decoded-state availability, not hardware cache flushing.
use divan::black_box;
use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};
use wacore_libsignal::protocol as old;
use wacore_signal_engine::{self as new, RandomSource, RustCrypto};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

const INCARNATION: [u8; 16] = [0x19; 16];
fn main() {
    divan::main();
}
struct Random(StdRng);
impl Random {
    fn seeded() -> Self {
        Self(StdRng::seed_from_u64(0x1828_bec4))
    }
}
impl RandomSource for Random {
    fn fill(&mut self, bytes: &mut [u8]) -> new::Result<()> {
        self.0.fill(bytes);
        Ok(())
    }
}

/// A valid synthetic v3 state with compact seed keys, shared by both readers.
fn session(backlog: usize, archives: usize) -> new::SessionRecord {
    let mut random = Random::seeded();
    let mut engine = new::Engine::new(&RustCrypto, &mut random);
    let local = engine.generate_key_pair().unwrap();
    let remote = engine.generate_key_pair().unwrap();
    let ratchet = engine.generate_key_pair().unwrap();
    let recv = engine.generate_key_pair().unwrap();
    let mut chain = new::Chain::new(recv.public, None, [0x37; 32]);
    chain.index = backlog as u32;
    chain.skipped = (0..backlog)
        .map(|index| new::MessageKey::Seed {
            index: index as u32,
            seed: [(index % 251) as u8; 32],
        })
        .collect();
    let state = new::SessionState {
        version: 3,
        local_identity: local.public,
        remote_identity: remote.public,
        root: [0x42; 32],
        previous_counter: 0,
        sender: Some(new::Chain::new(
            ratchet.public,
            Some(ratchet.private),
            [0x51; 32],
        )),
        receivers: vec![chain],
        pending_pre_key: None,
        local_registration: 11,
        remote_registration: 22,
        needs_refresh: false,
        alice_base_key: Some([0x61; 32]),
        alice_base_key_tagged: true,
        pending_key_exchange: None,
    };
    let mut record = new::SessionRecord::default();
    for _ in 0..=archives {
        record.install(state.clone());
    }
    record.reservation.prepare(0, 64, INCARNATION).unwrap();
    record.reservation.mark_committed();
    record
}
fn bytes(backlog: usize, archives: usize) -> Vec<u8> {
    new::codec::encode_session_record(&session(backlog, archives))
}
fn old_session(input: &[u8]) -> old::SessionRecord {
    old::SessionRecord::deserialize_for_store(input, &INCARNATION).unwrap()
}
fn new_session(input: &[u8]) -> new::SessionRecord {
    new::codec::decode_session_record_live(&RustCrypto, input, &INCARNATION).unwrap()
}

mod backlog {
    use super::*;
    #[divan::bench(args = [0, 128, 2000])]
    fn legacy_load(b: divan::Bencher, keys: usize) {
        let input = bytes(keys, 0);
        b.bench(|| black_box(old_session(black_box(&input))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_load(b: divan::Bencher, keys: usize) {
        let input = bytes(keys, 0);
        b.bench(|| black_box(new_session(black_box(&input))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn legacy_save_fresh(b: divan::Bencher, keys: usize) {
        let record = old_session(&bytes(keys, 0));
        b.bench(|| {
            let mut output = Vec::new();
            record.serialize_into_for_store(&mut output, &INCARNATION);
            black_box(output)
        });
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_save_fresh(b: divan::Bencher, keys: usize) {
        let record = new_session(&bytes(keys, 0));
        b.bench(|| black_box(new::codec::encode_session_record(black_box(&record))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn legacy_clone_live(b: divan::Bencher, keys: usize) {
        let record = old_session(&bytes(keys, 0));
        b.bench(|| black_box(black_box(&record).clone()));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_clone_live(b: divan::Bencher, keys: usize) {
        let record = new_session(&bytes(keys, 0));
        b.bench(|| black_box(black_box(&record).clone()));
    }
}
mod archive {
    use super::*;
    // Each archived state has 128 compact skipped keys. The current state does too.
    #[divan::bench(args = [1, 10, 40])]
    fn legacy_load(b: divan::Bencher, count: usize) {
        let input = bytes(128, count);
        b.bench(|| black_box(old_session(black_box(&input))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn independent_load(b: divan::Bencher, count: usize) {
        let input = bytes(128, count);
        b.bench(|| black_box(new_session(black_box(&input))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn legacy_save_fresh(b: divan::Bencher, count: usize) {
        let record = old_session(&bytes(128, count));
        b.bench(|| {
            let mut output = Vec::new();
            record.serialize_into_for_store(&mut output, &INCARNATION);
            black_box(output)
        });
    }
    #[divan::bench(args = [1, 10, 40])]
    fn independent_save_fresh(b: divan::Bencher, count: usize) {
        let record = new_session(&bytes(128, count));
        b.bench(|| black_box(new::codec::encode_session_record(black_box(&record))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn legacy_clone_live(b: divan::Bencher, count: usize) {
        let record = old_session(&bytes(128, count));
        b.bench(|| black_box(black_box(&record).clone()));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn independent_clone_live(b: divan::Bencher, count: usize) {
        let record = new_session(&bytes(128, count));
        b.bench(|| black_box(black_box(&record).clone()));
    }
}

struct SenderStore(Option<old::SenderKeyRecord>);
#[async_trait::async_trait]
impl old::SenderKeyStore for SenderStore {
    async fn load_sender_key(
        &self,
        _: &old::SenderKeyName,
    ) -> old::error::Result<Option<old::SenderKeyRecord>> {
        Ok(self.0.clone())
    }
    async fn store_sender_key(
        &mut self,
        _: &old::SenderKeyName,
        record: old::SenderKeyRecord,
    ) -> old::error::Result<()> {
        self.0 = Some(record);
        Ok(())
    }
}
fn sender_name() -> old::SenderKeyName {
    old::SenderKeyName::from_parts("synthetic-group@g.us", "synthetic-sender:1@s.whatsapp.net")
}
fn sender_pair() -> (new::SenderKeyRecord, new::SenderKeyRecord) {
    let mut random = Random::seeded();
    let mut engine = new::Engine::new(&RustCrypto, &mut random);
    let mut sender = new::SenderKeyRecord::default();
    let distribution = engine.create_sender_distribution(&mut sender).unwrap();
    let mut receiver = new::SenderKeyRecord::default();
    engine
        .process_sender_distribution(&mut receiver, &distribution)
        .unwrap();
    sender.reservation.prepare(0, 64, INCARNATION).unwrap();
    sender.reservation.mark_committed();
    (sender, receiver)
}
fn old_sender(record: &new::SenderKeyRecord) -> SenderStore {
    SenderStore(Some(
        old::SenderKeyRecord::deserialize_for_store(
            &new::codec::encode_sender_key_record(record),
            &INCARNATION,
        )
        .unwrap(),
    ))
}
mod sender_key {
    use super::*;
    // Legacy measurements include its async in-memory store interface and clone.
    // Independent measurements use the engine's direct mutable-state API.
    #[divan::bench(args = [32, 1024])]
    fn legacy_encrypt(b: divan::Bencher, size: usize) {
        let (sender, _) = sender_pair();
        let payload = vec![0x75; size];
        let name = sender_name();
        b.with_inputs(|| (old_sender(&sender), Random::seeded()))
            .bench_refs(|(store, random)| {
                black_box(
                    futures::executor::block_on(old::group_encrypt(
                        store,
                        &name,
                        &payload,
                        &mut random.0,
                    ))
                    .unwrap(),
                );
            });
    }
    #[divan::bench(args = [32, 1024])]
    fn independent_encrypt(b: divan::Bencher, size: usize) {
        let (sender, _) = sender_pair();
        let payload = vec![0x75; size];
        b.with_inputs(|| (sender.clone(), Random::seeded()))
            .bench_refs(|(record, random)| {
                black_box(
                    new::Engine::new(&RustCrypto, random)
                        .encrypt_sender_key(record, &payload)
                        .unwrap(),
                );
            });
    }
    #[divan::bench(args = [32, 1024])]
    fn legacy_decrypt(b: divan::Bencher, size: usize) {
        let (mut sender, receiver) = sender_pair();
        let wire = new::Engine::new(&RustCrypto, &mut Random::seeded())
            .encrypt_sender_key(&mut sender, &vec![0x75; size])
            .unwrap();
        let name = sender_name();
        b.with_inputs(|| old_sender(&receiver)).bench_refs(|store| {
            black_box(
                futures::executor::block_on(old::group_decrypt(&wire, store, &name)).unwrap(),
            );
        });
    }
    #[divan::bench(args = [32, 1024])]
    fn independent_decrypt(b: divan::Bencher, size: usize) {
        let (mut sender, receiver) = sender_pair();
        let wire = new::Engine::new(&RustCrypto, &mut Random::seeded())
            .encrypt_sender_key(&mut sender, &vec![0x75; size])
            .unwrap();
        b.with_inputs(|| (receiver.clone(), Random::seeded()))
            .bench_refs(|(record, random)| {
                black_box(
                    new::Engine::new(&RustCrypto, random)
                        .decrypt_sender_key(record, &wire)
                        .unwrap(),
                );
            });
    }
}
