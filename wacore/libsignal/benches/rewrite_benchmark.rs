//! Standalone synthetic benchmarks for the protocol engine and public adapter.
//! Run only through CodSpeed. Fixture construction is outside measured regions.
//! `load` measures cold codec allocation; `clone_live` measures warm retained state.
//! Allocation profiling describes native API allocations, not whole-process RSS.
//! Cold/warm describes decoded-state availability, not hardware cache flushing.
use divan::black_box;
use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};
use wacore_libsignal::engine::{self as new, RandomSource, RustCrypto};
use wacore_libsignal::protocol as sdk;

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
        metadata: std::sync::Arc::new(new::SessionMetadata {
            version: 3,
            local_identity: local.public,
            remote_identity: remote.public,
            root: [0x42; 32],
            previous_counter: 0,
            local_registration: 11,
            remote_registration: 22,
            needs_refresh: false,
            alice_base_key: Some([0x61; 32]),
            alice_base_key_tagged: true,
            pending_key_exchange: None,
        }),
        sender: Some(new::Chain::new(
            ratchet.public,
            Some(ratchet.private),
            [0x51; 32],
        )),
        receivers: vec![chain],
        pending_pre_key: None,
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
fn sdk_session(input: &[u8]) -> sdk::SessionRecord {
    sdk::SessionRecord::deserialize_for_store(input, &INCARNATION).unwrap()
}
fn new_session(input: &[u8]) -> new::SessionRecord {
    new::codec::decode_session_record_live(&RustCrypto, input, &INCARNATION).unwrap()
}

mod backlog {
    use super::*;
    #[divan::bench(args = [0, 128, 2000])]
    fn runtime_load(b: divan::Bencher, keys: usize) {
        let input = bytes(keys, 0);
        b.bench(|| black_box(sdk_session(black_box(&input))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn runtime_save_fresh(b: divan::Bencher, keys: usize) {
        let record = sdk_session(&bytes(keys, 0));
        b.bench(|| {
            let mut output = Vec::new();
            record.serialize_into_for_store(&mut output, &INCARNATION);
            black_box(output)
        });
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn runtime_clone_live(b: divan::Bencher, keys: usize) {
        let record = sdk_session(&bytes(keys, 0));
        b.bench(|| black_box(black_box(&record).clone()));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_load(b: divan::Bencher, keys: usize) {
        let input = bytes(keys, 0);
        b.bench(|| black_box(new_session(black_box(&input))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_save_fresh(b: divan::Bencher, keys: usize) {
        let record = new_session(&bytes(keys, 0));
        b.bench(|| black_box(new::codec::encode_session_record(black_box(&record))));
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn independent_clone_live(b: divan::Bencher, keys: usize) {
        let record = new_session(&bytes(keys, 0));
        b.bench(|| black_box(black_box(&record).clone()));
    }
}
mod archive {
    use super::*;
    #[divan::bench(args = [1, 10, 40])]
    fn runtime_load(b: divan::Bencher, count: usize) {
        let input = bytes(128, count);
        b.bench(|| black_box(sdk_session(black_box(&input))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn runtime_save_fresh(b: divan::Bencher, count: usize) {
        let record = sdk_session(&bytes(128, count));
        b.bench(|| {
            let mut output = Vec::new();
            record.serialize_into_for_store(&mut output, &INCARNATION);
            black_box(output)
        });
    }
    #[divan::bench(args = [1, 10, 40])]
    fn runtime_clone_live(b: divan::Bencher, count: usize) {
        let record = sdk_session(&bytes(128, count));
        b.bench(|| black_box(black_box(&record).clone()));
    }
    // Each archived state has 128 compact skipped keys. The current state does too.
    #[divan::bench(args = [1, 10, 40])]
    fn independent_load(b: divan::Bencher, count: usize) {
        let input = bytes(128, count);
        b.bench(|| black_box(new_session(black_box(&input))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn independent_save_fresh(b: divan::Bencher, count: usize) {
        let record = new_session(&bytes(128, count));
        b.bench(|| black_box(new::codec::encode_session_record(black_box(&record))));
    }
    #[divan::bench(args = [1, 10, 40])]
    fn independent_clone_live(b: divan::Bencher, count: usize) {
        let record = new_session(&bytes(128, count));
        b.bench(|| black_box(black_box(&record).clone()));
    }
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
mod sender_key {
    use super::*;
    // Legacy measurements include its async in-memory store interface and clone.
    // Independent measurements use the engine's direct mutable-state API.
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

#[derive(Clone)]
struct RuntimeSenderStore(Option<sdk::SenderKeyRecord>);
#[async_trait::async_trait]
impl sdk::SenderKeyStore for RuntimeSenderStore {
    async fn load_sender_key(
        &self,
        _: &sdk::SenderKeyName,
    ) -> sdk::error::Result<Option<sdk::SenderKeyRecord>> {
        Ok(self.0.clone())
    }
    async fn store_sender_key(
        &mut self,
        _: &sdk::SenderKeyName,
        record: sdk::SenderKeyRecord,
    ) -> sdk::error::Result<()> {
        self.0 = Some(record);
        Ok(())
    }
}
fn runtime_sender(record: &new::SenderKeyRecord) -> RuntimeSenderStore {
    RuntimeSenderStore(Some(
        sdk::SenderKeyRecord::deserialize_for_store(
            &new::codec::encode_sender_key_record(record),
            &INCARNATION,
        )
        .unwrap(),
    ))
}
fn runtime_name() -> sdk::SenderKeyName {
    sdk::SenderKeyName::from_parts("synthetic-group@g.us", "synthetic-sender:1@s.whatsapp.net")
}

/// Two consecutive messages let each reader warm its verifier with the first
/// and measure the second, without replaying a consumed message or key.
fn receive_fixture(
    size: usize,
    backlog: usize,
) -> (new::SenderKeyRecord, bytes::Bytes, bytes::Bytes) {
    use new::CryptoProvider as _;
    let (mut sender, mut receiver) = sender_pair();
    let state = &mut sender.states[0];
    let receiver_state = &mut receiver.states[0];
    for index in 0..backlog as u32 {
        receiver_state.skipped.push_back(new::SenderMessageKey {
            iteration: index,
            seed: RustCrypto.hmac_sha256(&state.chain_key, &[&[1]]).unwrap(),
        });
        state.chain_key = RustCrypto.hmac_sha256(&state.chain_key, &[&[2]]).unwrap();
    }
    state.iteration = backlog as u32;
    receiver_state.iteration = state.iteration;
    receiver_state.chain_key = state.chain_key;
    sender.reservation = new::Reservation::default();
    sender
        .reservation
        .prepare(state.iteration, 64, INCARNATION)
        .unwrap();
    sender.reservation.mark_committed();
    let payload = vec![0x75; size];
    let mut random = Random::seeded();
    let mut engine = new::Engine::new(&RustCrypto, &mut random);
    let first = engine
        .encrypt_sender_key(&mut sender, &payload)
        .unwrap()
        .into();
    let second = engine
        .encrypt_sender_key(&mut sender, &payload)
        .unwrap()
        .into();
    (receiver, first, second)
}
mod sender_sdk {
    use super::*;
    // Both SDKs use the same async clone-on-load store and shared ciphertext
    // API. Host session locks, disk flushes, network, and fixture decoding are
    // excluded. Internal prepared-cache synchronization remains measured.
    #[divan::bench(args = [32, 1024])]
    fn runtime_encrypt_cold(b: divan::Bencher, size: usize) {
        let (sender, _) = sender_pair();
        let record = runtime_sender(&sender).0.unwrap();
        let payload = vec![0x75; size];
        let name = runtime_name();
        b.with_inputs(|| (RuntimeSenderStore(Some(record.clone())), Random::seeded()))
            .bench_refs(|(store, random)| {
                black_box(
                    futures::executor::block_on(sdk::group_encrypt(
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
    fn runtime_decrypt_cold(b: divan::Bencher, size: usize) {
        let (receiver, wire, _) = receive_fixture(size, 0);
        let record = runtime_sender(&receiver).0.unwrap();
        let name = runtime_name();
        b.with_inputs(|| RuntimeSenderStore(Some(record.clone())))
            .bench_refs(|store| {
                black_box(
                    futures::executor::block_on(sdk::group_decrypt_shared(
                        wire.clone(),
                        store,
                        &name,
                    ))
                    .unwrap(),
                );
            });
    }
    #[divan::bench(args = [32, 1024])]
    fn runtime_encrypt_warm(b: divan::Bencher, size: usize) {
        let (sender, _) = sender_pair();
        let mut store = runtime_sender(&sender);
        let name = runtime_name();
        let payload = vec![0x75; size];
        futures::executor::block_on(sdk::group_encrypt(
            &mut store,
            &name,
            &payload,
            &mut Random::seeded().0,
        ))
        .unwrap();
        let record = store.0.unwrap();
        b.with_inputs(|| (RuntimeSenderStore(Some(record.clone())), Random::seeded()))
            .bench_refs(|(store, random)| {
                black_box(
                    futures::executor::block_on(sdk::group_encrypt(
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
    fn runtime_decrypt_warm(b: divan::Bencher, size: usize) {
        let (receiver, first, second) = receive_fixture(size, 0);
        let mut store = runtime_sender(&receiver);
        let name = runtime_name();
        futures::executor::block_on(sdk::group_decrypt_shared(first, &mut store, &name)).unwrap();
        let record = store.0.unwrap();
        b.with_inputs(|| RuntimeSenderStore(Some(record.clone())))
            .bench_refs(|store| {
                black_box(
                    futures::executor::block_on(sdk::group_decrypt_shared(
                        second.clone(),
                        store,
                        &name,
                    ))
                    .unwrap(),
                );
            });
    }
    #[divan::bench(args = [0, 128, 2000])]
    fn runtime_clone_warm_backlog(b: divan::Bencher, keys: usize) {
        let (receiver, first, _) = receive_fixture(32, keys);
        let mut store = runtime_sender(&receiver);
        futures::executor::block_on(sdk::group_decrypt_shared(
            first,
            &mut store,
            &runtime_name(),
        ))
        .unwrap();
        let record = store.0.unwrap();
        b.bench(|| black_box(black_box(&record).clone()));
    }
}
