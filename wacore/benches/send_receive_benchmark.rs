//! Full send/receive pipeline benchmarks using real `prepare_*_stanza` functions.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use async_trait::async_trait;
use buffa::Message as ProtoMessage;
use std::collections::HashMap;

/// SipHash with fixed keys: the default RandomState seeds per process, so
/// bucket layout (and thus cache behavior) differed between benchmark runs.
type DetState = std::hash::BuildHasherDefault<std::hash::DefaultHasher>;
type DetHashMap<K, V> = HashMap<K, V, DetState>;
use std::hint::black_box;
use wacore::client::context::{GroupInfo, SendContextResolver};
use wacore::messages::MessageUtils;
use wacore::runtime::{AbortHandle, Runtime};
use wacore::send::{
    DmStanzaRequest, GroupStanzaRequest, ResolvedDmDevices, SenderKeyDistributionPolicy,
    SignalStores, prepare_dm_stanza, prepare_group_stanza, prepare_peer_stanza,
};
use wacore::types::jid::{JidExt, make_sender_key_name};
use wacore::types::message::AddressingMode;
use wacore_binary::JidExt as _;
use wacore_binary::jid::Jid;
use wacore_binary::marshal::marshal;
use wacore_binary::node::{Node, NodeContent};
use wacore_libsignal::protocol::{
    CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, PreKeyBundle, PreKeyId, PreKeyRecord,
    PreKeySignalMessage, PreKeyStore, ProtocolAddress, SenderKeyRecord, SenderKeyStore,
    SessionRecord, SessionStore, SignalMessage, SignedPreKeyId, SignedPreKeyRecord,
    SignedPreKeyStore, Timestamp, UsePQRatchet, create_sender_key_distribution_message,
    group_decrypt, message_decrypt, message_encrypt, process_prekey_bundle,
    process_sender_key_distribution_message,
};
use wacore_libsignal::store::sender_key_name::SenderKeyName;
use waproto::whatsapp as wa;

type SigResult<T> = wacore_libsignal::protocol::error::Result<T>;

fn main() {
    divan::main();
}

/// Deterministic bench RNG (SplitMix64). A local algorithm, so fixtures are
/// stable across rand versions and platforms and baselines never shift on a
/// dependency bump. The `CryptoRng` marker is satisfied for API purposes
/// only: bench key material is synthetic by design.
struct BenchRng(u64);

impl BenchRng {
    fn step(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

// rand 0.10: `Rng`/`CryptoRng` are blanket-implemented over the infallible
// Try* traits, so these two impls are the whole surface.
impl rand::TryRng for BenchRng {
    type Error = std::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok((self.step() >> 32) as u32)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.step())
    }
    fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in dst.chunks_mut(8) {
            let bytes = self.step().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

impl rand::rand_core::TryCryptoRng for BenchRng {}

/// Deterministically seeded RNG: fixtures must be identical across runs and
/// builds so CodSpeed comparisons measure code, not key material.
fn bench_rng(seed: u64) -> BenchRng {
    BenchRng(seed)
}

/// FNV-1a fold of a fixture label into an RNG seed.
fn seed_of(label: &str) -> u64 {
    label.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

// ---------------------------------------------------------------------------
// In-memory Signal stores
// ---------------------------------------------------------------------------

// Bench runtime: runs spawned futures INLINE. CodSpeed's simulation
// serializes threads, so a real pool would measure scheduler and
// cross-thread synchronization overhead with zero parallelism benefit;
// inline execution measures the encrypt work itself, deterministically.
// `sleep` / `spawn_blocking` are not exercised by the encrypt path.
#[derive(Default)]
struct BenchRuntime;

#[async_trait]
impl Runtime for BenchRuntime {
    fn spawn(
        &self,
        mut future: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> AbortHandle {
        // Nested `futures::executor::block_on` panics, so drive the task with
        // a noop-waker poll loop. Encrypt tasks are CPU-bound and complete
        // without ever truly pending; the guard catches misuse.
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        for _ in 0..1_000_000 {
            if future.as_mut().poll(&mut cx).is_ready() {
                return AbortHandle::noop();
            }
        }
        panic!(
            "BenchRuntime::spawn: task pended forever; inline runtime only suits CPU-bound tasks"
        );
    }

    fn sleep(
        &self,
        _duration: std::time::Duration,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        unimplemented!("BenchRuntime::sleep is not used by the bench")
    }

    fn spawn_blocking(
        &self,
        _f: Box<dyn FnOnce() + Send + 'static>,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        unimplemented!("BenchRuntime::spawn_blocking is not used by the bench")
    }

    fn yield_now(&self) -> Option<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}

/// Bench fixture wrapping shared identity state. `Clone` is an Arc bump,
/// so spawned tasks see the same backing map as production adapters do
/// (whose internal cache is Arc-shared). Without this, the parallel
/// encrypt fan-out would deep-copy the HashMap per task and the bench
/// would over-count clone work that doesn't happen in production.
#[derive(Clone)]
struct MemIdentityStore {
    key_pair: IdentityKeyPair,
    reg_id: u32,
    identities: std::sync::Arc<std::sync::Mutex<DetHashMap<ProtocolAddress, IdentityKey>>>,
}

impl MemIdentityStore {
    fn new(key_pair: IdentityKeyPair, reg_id: u32) -> Self {
        Self {
            key_pair,
            reg_id,
            identities: std::sync::Arc::new(std::sync::Mutex::new(DetHashMap::default())),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl IdentityKeyStore for MemIdentityStore {
    async fn get_identity_key_pair(&self) -> SigResult<IdentityKeyPair> {
        Ok(self.key_pair.clone())
    }
    async fn get_local_registration_id(&self) -> SigResult<u32> {
        Ok(self.reg_id)
    }
    async fn save_identity(
        &mut self,
        a: &ProtocolAddress,
        id: &IdentityKey,
    ) -> SigResult<IdentityChange> {
        let mut guard = self.identities.lock().unwrap();
        let changed = guard.get(a).is_some_and(|e| e != id);
        guard.insert(a.clone(), *id);
        Ok(IdentityChange::from_changed(changed))
    }
    async fn is_trusted_identity(
        &self,
        _: &ProtocolAddress,
        _: &IdentityKey,
        _: Direction,
    ) -> SigResult<bool> {
        Ok(true)
    }
    async fn get_identity(&self, a: &ProtocolAddress) -> SigResult<Option<IdentityKey>> {
        Ok(self.identities.lock().unwrap().get(a).cloned())
    }
}

struct MemPreKeyStore(DetHashMap<PreKeyId, PreKeyRecord>);

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PreKeyStore for MemPreKeyStore {
    async fn get_pre_key(&self, id: PreKeyId) -> SigResult<PreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(wacore_libsignal::protocol::SignalProtocolError::InvalidPreKeyId)
    }
    async fn save_pre_key(&mut self, id: PreKeyId, r: &PreKeyRecord) -> SigResult<()> {
        self.0.insert(id, r.clone());
        Ok(())
    }
    async fn remove_pre_key(&mut self, id: PreKeyId) -> SigResult<()> {
        self.0.remove(&id);
        Ok(())
    }
}

struct MemSignedPreKeyStore(DetHashMap<SignedPreKeyId, SignedPreKeyRecord>);

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignedPreKeyStore for MemSignedPreKeyStore {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> SigResult<SignedPreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(wacore_libsignal::protocol::SignalProtocolError::InvalidSignedPreKeyId)
    }
    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        r: &SignedPreKeyRecord,
    ) -> SigResult<()> {
        self.0.insert(id, r.clone());
        Ok(())
    }
}

/// Bench fixture wrapping shared session state — see `MemIdentityStore`
/// for the rationale.
#[derive(Clone, Default)]
struct MemSessionStore(
    std::sync::Arc<std::sync::Mutex<DetHashMap<ProtocolAddress, SessionRecord>>>,
);

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SessionStore for MemSessionStore {
    async fn load_session(&self, a: &ProtocolAddress) -> SigResult<Option<SessionRecord>> {
        Ok(self.0.lock().unwrap().get(a).cloned())
    }
    async fn has_session(&self, a: &ProtocolAddress) -> SigResult<bool> {
        Ok(self.0.lock().unwrap().contains_key(a))
    }
    async fn store_session(&mut self, a: &ProtocolAddress, r: SessionRecord) -> SigResult<()> {
        self.0.lock().unwrap().insert(a.clone(), r);
        Ok(())
    }
}

struct MemSenderKeyStore(DetHashMap<SenderKeyName, SenderKeyRecord>);

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SenderKeyStore for MemSenderKeyStore {
    async fn store_sender_key(&mut self, n: &SenderKeyName, r: SenderKeyRecord) -> SigResult<()> {
        self.0.insert(n.clone(), r);
        Ok(())
    }
    async fn load_sender_key(&self, n: &SenderKeyName) -> SigResult<Option<SenderKeyRecord>> {
        Ok(self.0.get(n).cloned())
    }
}

// ---------------------------------------------------------------------------
// User: bundles all Signal stores for one participant
// ---------------------------------------------------------------------------

struct User {
    jid: Jid,
    address: ProtocolAddress,
    identity: MemIdentityStore,
    prekeys: MemPreKeyStore,
    signed_prekeys: MemSignedPreKeyStore,
    sessions: MemSessionStore,
    sender_keys: MemSenderKeyStore,
    prekey_pair: KeyPair,
    signed_prekey_pair: KeyPair,
    signed_prekey_sig: Vec<u8>,
}

impl User {
    fn new(user: &str, server: &str) -> Self {
        let mut rng = bench_rng(seed_of(user));
        let identity_key_pair = IdentityKeyPair::generate(&mut rng);
        let reg_id = (seed_of(user) as u32) & 0x3FFF;

        let pk_id: PreKeyId = 1.into();
        let pk_pair = KeyPair::generate(&mut rng);
        let pk_record = PreKeyRecord::new(pk_id, &pk_pair);

        let spk_id: SignedPreKeyId = 1.into();
        let spk_pair = KeyPair::generate(&mut rng);
        let spk_sig = identity_key_pair
            .private_key()
            .calculate_signature(&spk_pair.public_key.serialize(), &mut rng)
            .unwrap();
        let spk_record =
            SignedPreKeyRecord::new(spk_id, Timestamp::from_epoch_millis(0), &spk_pair, &spk_sig);

        let mut prekeys = MemPreKeyStore(DetHashMap::default());
        let mut signed_prekeys = MemSignedPreKeyStore(DetHashMap::default());
        futures::executor::block_on(async {
            prekeys.save_pre_key(pk_id, &pk_record).await.unwrap();
            signed_prekeys
                .save_signed_pre_key(spk_id, &spk_record)
                .await
                .unwrap();
        });

        let jid = Jid::new(
            user,
            wacore_binary::jid::Server::try_from(server)
                .expect("invalid server in benchmark fixture"),
        );
        let address = jid.to_protocol_address();

        Self {
            jid,
            address,
            identity: MemIdentityStore::new(identity_key_pair, reg_id),
            prekeys,
            signed_prekeys,
            sessions: MemSessionStore::default(),
            sender_keys: MemSenderKeyStore(DetHashMap::default()),
            prekey_pair: pk_pair,
            signed_prekey_pair: spk_pair,
            signed_prekey_sig: spk_sig.to_vec(),
        }
    }

    /// The same account on another device: same keys, distinct address. Real
    /// DM fan-out targets several devices of one user, which `User::new` alone
    /// cannot express since it derives the address from user+server.
    fn with_device(mut self, device: u16) -> Self {
        self.jid.device = device;
        self.address = self.jid.to_protocol_address();
        self
    }

    fn prekey_bundle(&self) -> PreKeyBundle {
        PreKeyBundle::new(
            self.identity.reg_id,
            1.into(),
            Some((1.into(), self.prekey_pair.public_key)),
            1.into(),
            self.signed_prekey_pair.public_key,
            self.signed_prekey_sig.clone(),
            *self.identity.key_pair.identity_key(),
        )
        .unwrap()
    }
}

fn establish_session(sender: &mut User, receiver: &User) {
    let bundle = receiver.prekey_bundle();
    let mut rng = bench_rng(0xBE_5EED + 1);
    futures::executor::block_on(async {
        process_prekey_bundle(
            &receiver.address,
            &mut sender.sessions,
            &mut sender.identity,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
    });
}

/// Establish bidirectional session by sending one message in each direction.
/// The return trip from b→a is required to clear a's `pending_pre_key`,
/// otherwise a's next outbound is still pkmsg and `prepare_peer_stanza`
/// without an `AdvSignedDeviceIdentity` would fail the pre-flight check.
fn establish_bidirectional(a: &mut User, b: &mut User) {
    establish_session(a, b);
    futures::executor::block_on(async {
        let ct = message_encrypt(b"init", &b.address, &mut a.sessions, &mut a.identity)
            .await
            .unwrap();
        let ct_msg = CiphertextMessage::PreKeySignalMessage(
            PreKeySignalMessage::try_from(ct.serialize()).unwrap(),
        );
        let mut rng = bench_rng(0xBE_5EED + 2);
        message_decrypt(
            &ct_msg,
            &a.address,
            &mut b.sessions,
            &mut b.identity,
            &mut b.prekeys,
            &b.signed_prekeys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();

        // b→a round trip clears a's pending_pre_key so subsequent sends from
        // a are plain `msg`, not pkmsg.
        let ct_back = message_encrypt(b"ack", &a.address, &mut b.sessions, &mut b.identity)
            .await
            .unwrap();
        let ct_back_msg =
            CiphertextMessage::SignalMessage(SignalMessage::try_from(ct_back.serialize()).unwrap());
        message_decrypt(
            &ct_back_msg,
            &b.address,
            &mut a.sessions,
            &mut a.identity,
            &mut a.prekeys,
            &a.signed_prekeys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
    });
}

// ---------------------------------------------------------------------------
// Mock resolver (returns pre-configured devices, no server)
// ---------------------------------------------------------------------------

struct MockResolver(Vec<Jid>);

#[async_trait]
impl SendContextResolver for MockResolver {
    async fn resolve_devices(&self, _: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        Ok(self.0.clone())
    }
    async fn fetch_prekeys(&self, _: &[Jid]) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        Ok(HashMap::new())
    }
    async fn fetch_prekeys_for_identity_check(
        &self,
        _: &[Jid],
    ) -> Result<wacore::prekeys::PreKeyFetchOutcome, anyhow::Error> {
        Ok(wacore::prekeys::PreKeyFetchOutcome::default())
    }
    async fn resolve_group_info(
        &self,
        _: &Jid,
    ) -> Result<std::sync::Arc<GroupInfo>, anyhow::Error> {
        Ok(std::sync::Arc::new(GroupInfo::new(
            self.0.clone(),
            AddressingMode::Pn,
        )))
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn text_msg() -> wa::Message {
    wa::Message {
        conversation: Some("Hello, this is a benchmark message.".to_string()),
        ..Default::default()
    }
}

/// Extract the skmsg ciphertext from a group stanza Node (owned, no unsafe).
/// In production the server strips <participants> before forwarding to recipients,
/// so we extract just the skmsg <enc> bytes to simulate what the receiver sees.
fn extract_skmsg_bytes(stanza: &Node) -> Vec<u8> {
    let enc = stanza
        .children()
        .unwrap()
        .iter()
        .find(|n| {
            n.tag == "enc"
                && n.attrs()
                    .optional_string("type")
                    .is_some_and(|t| t.as_ref() == "skmsg")
        })
        .expect("skmsg enc node");

    match &enc.content {
        Some(NodeContent::Bytes(b)) => b.clone(),
        _ => panic!("expected bytes"),
    }
}

/// Encrypt one padded text message from `alice` to `to`, returning the wire
/// bytes and the enc type the stanza would carry. Shared by both receive
/// fixtures so the ratchet and steady-state benches cannot drift apart on
/// padding or enc-type mapping.
fn encrypt_one(alice: &mut User, to: &ProtocolAddress) -> (Vec<u8>, String) {
    futures::executor::block_on(async {
        let ct = message_encrypt(
            &MessageUtils::pad_message_v2(text_msg().encode_to_vec()),
            to,
            &mut alice.sessions,
            &mut alice.identity,
        )
        .await
        .unwrap();
        match ct {
            CiphertextMessage::SignalMessage(m) => (m.serialized().to_vec(), "msg".to_string()),
            CiphertextMessage::PreKeySignalMessage(m) => {
                (m.serialized().to_vec(), "pkmsg".to_string())
            }
            _ => panic!("unexpected ciphertext type in benchmark fixture"),
        }
    })
}

fn decrypt_dm(
    ciphertext: &[u8],
    enc_type: &str,
    sender_addr: &ProtocolAddress,
    receiver: &mut User,
) -> wa::Message {
    futures::executor::block_on(async {
        let parsed = if enc_type == "pkmsg" {
            CiphertextMessage::PreKeySignalMessage(
                PreKeySignalMessage::try_from(ciphertext).unwrap(),
            )
        } else {
            CiphertextMessage::SignalMessage(SignalMessage::try_from(ciphertext).unwrap())
        };
        let mut rng = bench_rng(0xBE_5EED + 3);
        let decrypted = message_decrypt(
            &parsed,
            sender_addr,
            &mut receiver.sessions,
            &mut receiver.identity,
            &mut receiver.prekeys,
            &receiver.signed_prekeys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();

        let unpadded = MessageUtils::unpad_message_ref(&decrypted.plaintext, 2).unwrap();
        wa::Message::decode_from_slice(unpadded).unwrap()
    })
}

fn decrypt_group(
    ciphertext: &[u8],
    sender_addr: &ProtocolAddress,
    group_jid: &Jid,
    receiver: &mut User,
) -> wa::Message {
    futures::executor::block_on(async {
        let sk_name = make_sender_key_name(group_jid, sender_addr);
        let plaintext = group_decrypt(ciphertext, &mut receiver.sender_keys, &sk_name)
            .await
            .unwrap();

        let unpadded = MessageUtils::unpad_message_ref(&plaintext, 2).unwrap();
        wa::Message::decode_from_slice(unpadded).unwrap()
    })
}

// ---------------------------------------------------------------------------
// DM setups
// ---------------------------------------------------------------------------

/// The `<device-identity>` blob a paired device carries.
///
/// Every send path takes `Option<&ADVSignedDeviceIdentity>`, and `None` changes
/// what they do — differently per path, so the distinction matters here:
///
/// - `prepare_peer_stanza` refuses, and pays a whole extra session checkout to
///   find out (`account.is_none() && pkmsg_would_be_emitted(..)`);
/// - `prepare_dm_stanza` and the `BestEffort` group policy are lenient: they
///   drop the `<device-identity>` child and send anyway;
/// - only `SenderKeyDistributionPolicy::Required` propagates the error.
///
/// A paired device always has one (`device_snapshot.account.as_deref()`), so a
/// bench passing `None` measures a shape production never sends — and on the
/// lenient paths it silently omits a stanza child rather than failing loudly.
///
/// Only the bytes matter here — it is serialised into the stanza, never
/// validated by the sender.
fn bench_account() -> wa::ADVSignedDeviceIdentity {
    wa::ADVSignedDeviceIdentity {
        details: Some(vec![0xAD; 32]),
        account_signature_key: Some(vec![0xAC; 32]),
        account_signature: Some(vec![0x51; 64]),
        device_signature: Some(vec![0xD5; 64]),
    }
}

struct DmSendData {
    alice: User,
    bob_jid: Jid,
    msg: wa::Message,
    account: wa::ADVSignedDeviceIdentity,
}

fn setup_dm_send() -> DmSendData {
    let mut alice = User::new("5511999000001", "s.whatsapp.net");
    let mut bob = User::new("5511999000002", "s.whatsapp.net");
    establish_bidirectional(&mut alice, &mut bob);
    DmSendData {
        alice,
        bob_jid: bob.jid,
        msg: text_msg(),
        account: bench_account(),
    }
}

struct DmRecvData {
    bob: User,
    alice_addr: ProtocolAddress,
    ciphertext: Vec<u8>,
    enc_type: String,
}

fn setup_dm_recv() -> DmRecvData {
    let mut alice = User::new("5511999000001", "s.whatsapp.net");
    let mut bob = User::new("5511999000002", "s.whatsapp.net");
    establish_bidirectional(&mut alice, &mut bob);

    // A `msg` rather than a `pkmsg`, but NOT steady state: Bob has not decrypted
    // anything on this sending chain yet, so this first one costs him a DH ratchet
    // step (two X25519 agreements plus a key generation). That is a real shape —
    // the first message after the peer replies — but it is ~40x a steady-state
    // decrypt, so the two need separate benches or the common case is invisible.
    let bob_addr = bob.address.clone();
    let (ciphertext, enc_type) = encrypt_one(&mut alice, &bob_addr);

    DmRecvData {
        bob,
        alice_addr: alice.address,
        ciphertext,
        enc_type,
    }
}

// ---------------------------------------------------------------------------
// Group setups
// ---------------------------------------------------------------------------

struct GrpSendData {
    alice: User,
    group_jid: Jid,
    /// Built in setup, not per iteration: production resolves the group once
    /// and holds the result behind an `Arc` across sends (`ensure_self_in_group`
    /// hands the same `Arc` straight back whenever we are already a member, the
    /// steady state), so a send never constructs or drops a participant list.
    /// Building it in the measured body charged every group send an
    /// N-participant construct + teardown that no send performs — 26.8K
    /// instructions at 512 members, and the entire reason this benchmark
    /// appeared to scale with group size while `prepare_group_stanza` itself is
    /// flat (334.0K at 8 members, 334.1K at 512).
    group_info: GroupInfo,
    /// Warm-send fixture: the resolved set with its phash memo pre-warmed in
    /// setup, like the per-group device memo serves production repeat sends.
    resolved_for_phash: Option<std::sync::Arc<wacore::send::ResolvedGroupDevices>>,
    force_skdm: bool,
    resolver: MockResolver,
    msg: wa::Message,
    /// Built in setup, not in the measured body: production borrows the cached
    /// account rather than allocating one per send, so building it inside the
    /// closure would charge every group baseline four `Vec` allocations that no
    /// send performs.
    account: wa::ADVSignedDeviceIdentity,
    // Built once in setup so the measured body excludes thread-pool startup
    // (building the pool inside the bench body would charge its syscalls to
    // the encrypt path).
    runtime: BenchRuntime,
}

fn setup_group_send(n: usize) -> GrpSendData {
    let mut alice = User::new("100000000000001", "lid");
    let group_jid: Jid = "120363000000000001@g.us".parse().unwrap();

    let mut participants = Vec::with_capacity(n);
    let mut devices = Vec::with_capacity(n);

    for i in 0..n {
        let member = User::new(&format!("{}", 100000000000100u64 + i as u64), "lid");
        establish_session(&mut alice, &member);
        participants.push(member.jid.clone());
        devices.push(member.jid);
    }

    let sk_name = make_sender_key_name(&group_jid, &alice.address);
    futures::executor::block_on(async {
        let mut rng = bench_rng(0xBE_5EED + 4);
        create_sender_key_distribution_message(&sk_name, &mut alice.sender_keys, &mut rng)
            .await
            .unwrap();
    });

    let resolved = std::sync::Arc::new(wacore::send::ResolvedGroupDevices::new(
        participants.clone(),
    ));
    // Warm steady state: production warms the memo on the first send after a
    // topology change and serves every later send from it. Assert it, so a
    // silent failure can't leave the bench measuring the cold path.
    resolved
        .phash(&alice.jid)
        .expect("phash must warm in setup");

    // Self-append happens once here for the same reason production does it once
    // per resolution: `prepare_group_stanza` expects the sender in the list.
    let own_base = alice.jid.to_non_ad();
    if !participants.iter().any(|p| p.is_same_user_as(&own_base)) {
        participants.push(own_base);
    }
    let group_info = GroupInfo::new(participants, AddressingMode::Pn);

    GrpSendData {
        alice,
        group_jid,
        group_info,
        resolved_for_phash: Some(resolved),
        force_skdm: false,
        resolver: MockResolver(devices),
        msg: text_msg(),
        account: bench_account(),
        runtime: BenchRuntime,
    }
}

fn setup_group_send_10() -> GrpSendData {
    setup_group_send(10)
}
fn setup_group_send_50() -> GrpSendData {
    setup_group_send(50)
}
fn setup_group_send_256() -> GrpSendData {
    setup_group_send(256)
}

// First-message path: force_skdm=true exercises N pairwise encryptions
fn setup_group_skdm_10() -> GrpSendData {
    let mut d = setup_group_send(10);
    d.force_skdm = true;
    d.resolved_for_phash = None;
    d
}
fn setup_group_skdm_50() -> GrpSendData {
    let mut d = setup_group_send(50);
    d.force_skdm = true;
    d.resolved_for_phash = None;
    d
}
fn setup_group_skdm_256() -> GrpSendData {
    let mut d = setup_group_send(256);
    d.force_skdm = true;
    d.resolved_for_phash = None;
    d
}

struct GrpRecvData {
    bob: User,
    alice_addr: ProtocolAddress,
    group_jid: Jid,
    skmsg_bytes: Vec<u8>,
}

fn setup_group_recv() -> GrpRecvData {
    let mut alice = User::new("100000000000001", "lid");
    let mut bob = User::new("100000000000002", "lid");
    let group_jid: Jid = "120363000000000001@g.us".parse().unwrap();

    establish_session(&mut alice, &bob);

    // Alice creates sender key and distributes SKDM to Bob
    let sk_name = make_sender_key_name(&group_jid, &alice.address);
    futures::executor::block_on(async {
        let mut rng = bench_rng(0xBE_5EED + 5);
        let skdm =
            create_sender_key_distribution_message(&sk_name, &mut alice.sender_keys, &mut rng)
                .await
                .unwrap();

        process_sender_key_distribution_message(&sk_name, &skdm, &mut bob.sender_keys)
            .await
            .unwrap();
    });

    // Build a full group stanza, then extract just the skmsg bytes
    // (server strips <participants> before forwarding to recipients)
    let resolver = MockResolver(vec![bob.jid.clone()]);
    let own_jid = alice.jid.clone();
    let group_info = GroupInfo::new(vec![bob.jid.clone(), alice.jid.clone()], AddressingMode::Pn);

    let mut stores = SignalStores {
        sender_key_store: &mut alice.sender_keys,
        session_store: &mut alice.sessions,
        identity_store: &mut alice.identity,
        prekey_store: &mut alice.prekeys,
        signed_prekey_store: &alice.signed_prekeys,
    };

    let runtime = BenchRuntime;
    let message = text_msg();
    let account = bench_account();
    let result = futures::executor::block_on(prepare_group_stanza(
        &runtime,
        &mut stores,
        &resolver,
        GroupStanzaRequest {
            group: &group_info,
            own_jid: &own_jid,
            own_lid: &own_jid,
            account: Some(&account),
            to: &group_jid,
            message: &message,
            message_id: "bench-grp-recv",
            force_distribution: false,
            distribution_targets: None,
            distribution_policy: SenderKeyDistributionPolicy::BestEffort,
            phash_devices: None,
            edit: None,
            extra_nodes: &[],
            pre_encoded: None,
        },
    ))
    .unwrap();

    let skmsg_bytes = extract_skmsg_bytes(&result.node);

    GrpRecvData {
        bob,
        alice_addr: alice.address,
        group_jid,
        skmsg_bytes,
    }
}

// ===========================================================================
// Benchmarks
// ===========================================================================

struct DmFanoutData {
    alice: User,
    to_jid: Jid,
    msg: wa::Message,
    account: wa::ADVSignedDeviceIdentity,
    devices: ResolvedDmDevices,
    resolver: MockResolver,
    runtime: BenchRuntime,
}

/// The real 1:1 send: `prepare_dm_stanza`, which fans out to the recipient's
/// devices and our own companions and builds the `<participants>` tree.
///
/// `n_recipient_devices` counts the contact's devices; one own companion is
/// always added on top, because a paired account has at least the phone and
/// this bench should not measure the degenerate zero-companion shape. The
/// total fan-out is therefore `n_recipient_devices + 1` pairwise encryptions.
///
/// Sessions are established **bidirectionally**. `establish_session` alone
/// leaves `pending_pre_key` set, so every outbound stays a `pkmsg` and the
/// bench would measure first-message prekey wrapping plus `<device-identity>`
/// serialisation forever — the opposite of the repeat-send shape intended here.
fn setup_dm_fanout(n_recipient_devices: usize) -> DmFanoutData {
    let mut alice = User::new("5511999000001", "s.whatsapp.net");
    let own_jid = alice.jid.clone();
    let to_base: Jid = "5511999000002@s.whatsapp.net".parse().unwrap();

    let mut all_devices = Vec::with_capacity(n_recipient_devices + 1);
    for i in 0..n_recipient_devices {
        let mut device = to_base.clone();
        device.device = i as u16;
        let mut peer = User::new("5511999000002", "s.whatsapp.net").with_device(i as u16);
        establish_bidirectional(&mut alice, &mut peer);
        all_devices.push(device);
    }
    // One own companion, so the own-device half of the partition is exercised.
    let mut companion = own_jid.clone();
    companion.device = 42;
    let mut own_peer = User::new("5511999000001", "s.whatsapp.net").with_device(42);
    establish_bidirectional(&mut alice, &mut own_peer);
    all_devices.push(companion);

    // Assert the sessions really are acknowledged, so a fixture regression cannot
    // quietly turn this back into a first-message benchmark: with `pending_pre_key`
    // still set every outbound is a pkmsg, which measures prekey wrapping and
    // `<device-identity>` serialisation instead of the repeat send.
    for device in &all_devices {
        let addr = device.to_protocol_address();
        let emits_pkmsg = futures::executor::block_on(wacore::send::pkmsg_would_be_emitted(
            &mut alice.sessions,
            &addr,
        ))
        .expect("session must be loadable in setup");
        assert!(
            !emits_pkmsg,
            "fixture must use acknowledged sessions; {addr} still emits pkmsg"
        );
    }

    let resolver = MockResolver(all_devices.clone());
    let devices = ResolvedDmDevices::new(all_devices, &own_jid, None);
    // Warm the phash memo in setup, as the per-recipient memo does for repeat
    // sends in production; otherwise every iteration measures the cold path.
    devices.phash();

    DmFanoutData {
        alice,
        to_jid: to_base,
        msg: text_msg(),
        account: bench_account(),
        devices,
        resolver,
        runtime: BenchRuntime,
    }
}

/// One recipient device plus our companion: two pairwise encryptions.
fn setup_dm_fanout_1() -> DmFanoutData {
    setup_dm_fanout(1)
}
/// Four recipient devices plus our companion: five pairwise encryptions.
fn setup_dm_fanout_4() -> DmFanoutData {
    setup_dm_fanout(4)
}

fn run_dm_fanout(d: &mut DmFanoutData) {
    let own_jid = d.alice.jid.clone();
    let mut stores = SignalStores {
        sender_key_store: &mut d.alice.sender_keys,
        session_store: &mut d.alice.sessions,
        identity_store: &mut d.alice.identity,
        prekey_store: &mut d.alice.prekeys,
        signed_prekey_store: &d.alice.signed_prekeys,
    };
    let prepared = futures::executor::block_on(prepare_dm_stanza(
        &d.runtime,
        &mut stores,
        &d.resolver,
        DmStanzaRequest {
            own_jid: &own_jid,
            account: Some(&d.account),
            to: &d.to_jid,
            message: &d.msg,
            message_id: "b-dm",
            edit: None,
            extra_nodes: &[],
            devices: &d.devices,
            pre_encoded: None,
        },
    ))
    .unwrap();
    black_box(marshal(&prepared.node).unwrap());
}

/// The 1:1 path an actual message takes. Nothing covered this before: the only
/// DM-shaped bench measured `prepare_peer_stanza`, which is the own-device peer
/// path and skips the fan-out, the participants tree and the reporting token.
#[divan::bench]
fn bench_dm_send(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_dm_fanout_1)
        .bench_refs(run_dm_fanout);
}

/// Multi-device recipient: the shape that actually spawns encrypt tasks.
/// Named for the total fan-out — four recipient devices plus one own companion,
/// so five pairwise encryptions.
#[divan::bench]
fn bench_dm_send_5_way_fanout(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_dm_fanout_4)
        .bench_refs(run_dm_fanout);
}

/// A peer stanza: what we send to our OWN other devices, not a DM to a contact.
/// See `bench_dm_send` for the contact path.
///
/// `account` is `Some`, matching a paired device. With `None` the `&&` in
/// `prepare_peer_stanza` does not short-circuit and every send pays an extra
/// `SessionCheckout::load` + `commit()` just to pre-flight the pkmsg case —
/// work production never does, and enough of it to move the baseline.
#[divan::bench]
fn bench_peer_send(bencher: divan::Bencher) {
    bencher.with_inputs(setup_dm_send).bench_refs(|d| {
        let signal_addr = d.bob_jid.to_protocol_address();
        let node = futures::executor::block_on(prepare_peer_stanza(
            &mut d.alice.sessions,
            &mut d.alice.identity,
            d.bob_jid.clone(),
            &signal_addr,
            &d.msg,
            "b-001",
            Some(&d.account),
        ))
        .unwrap();
        black_box(marshal(&node).unwrap());
    });
}

#[divan::bench]
fn bench_dm_recv(bencher: divan::Bencher) {
    bencher.with_inputs(setup_dm_recv).bench_refs(|d| {
        black_box(decrypt_dm(
            &d.ciphertext,
            &d.enc_type,
            &d.alice_addr,
            &mut d.bob,
        ));
    });
}

/// Steady state: Bob has already ratcheted into Alice's current sending chain,
/// so this decrypt is symmetric-only — no curve operation at all. This is what
/// an active conversation actually pays per message, and it is roughly an order
/// of magnitude cheaper than the ratchet step `bench_dm_recv` measures.
fn setup_dm_recv_steady() -> DmRecvData {
    let mut alice = User::new("5511999000001", "s.whatsapp.net");
    let mut bob = User::new("5511999000002", "s.whatsapp.net");
    establish_bidirectional(&mut alice, &mut bob);

    let bob_addr = bob.address.clone();

    // Burn the ratchet step on a throwaway message, so the one we hand back is
    // decrypted with the chain already advanced.
    let (first, first_type) = encrypt_one(&mut alice, &bob_addr);
    decrypt_dm(&first, &first_type, &alice.address, &mut bob);

    let (ciphertext, enc_type) = encrypt_one(&mut alice, &bob_addr);

    DmRecvData {
        bob,
        alice_addr: alice.address,
        ciphertext,
        enc_type,
    }
}

#[divan::bench]
fn bench_dm_recv_steady(bencher: divan::Bencher) {
    bencher.with_inputs(setup_dm_recv_steady).bench_refs(|d| {
        black_box(decrypt_dm(
            &d.ciphertext,
            &d.enc_type,
            &d.alice_addr,
            &mut d.bob,
        ));
    });
}

fn run_group_send(d: &mut GrpSendData) {
    let own_jid = d.alice.jid.clone();
    // Warm sends (force_skdm=false) distribute no SKDM, so prepare_group_stanza
    // only emits a phash if it gets the full device set. Mirror the real
    // warm-send caller by passing it; the cold/force_skdm path resolves the set
    // itself and keeps None.
    let group_info = &d.group_info;
    let mut stores = SignalStores {
        sender_key_store: &mut d.alice.sender_keys,
        session_store: &mut d.alice.sessions,
        identity_store: &mut d.alice.identity,
        prekey_store: &mut d.alice.prekeys,
        signed_prekey_store: &d.alice.signed_prekeys,
    };

    let result = futures::executor::block_on(prepare_group_stanza(
        &d.runtime,
        &mut stores,
        &d.resolver,
        GroupStanzaRequest {
            group: group_info,
            own_jid: &own_jid,
            own_lid: &own_jid,
            account: Some(&d.account),
            to: &d.group_jid,
            message: &d.msg,
            message_id: "b-grp",
            force_distribution: d.force_skdm,
            distribution_targets: None,
            distribution_policy: SenderKeyDistributionPolicy::BestEffort,
            phash_devices: d.resolved_for_phash.as_deref(),
            edit: None,
            extra_nodes: &[],
            pre_encoded: None,
        },
    ))
    .unwrap();

    black_box(marshal(&result.node).unwrap());
}

// Steady-state group send (skmsg only, no SKDM distribution)
#[divan::bench]
fn bench_group_send_10(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_send_10)
        .bench_refs(run_group_send);
}

#[divan::bench]
fn bench_group_send_50(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_send_50)
        .bench_refs(run_group_send);
}

#[divan::bench]
fn bench_group_send_256(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_send_256)
        .bench_refs(run_group_send);
}

// First-message group send: forces SKDM distribution with N pairwise encryptions
#[divan::bench]
fn bench_group_send_skdm_10(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_skdm_10)
        .bench_refs(run_group_send);
}

#[divan::bench]
fn bench_group_send_skdm_50(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_skdm_50)
        .bench_refs(run_group_send);
}

#[divan::bench]
fn bench_group_send_skdm_256(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_skdm_256)
        .bench_refs(run_group_send);
}

#[divan::bench]
fn bench_group_recv(bencher: divan::Bencher) {
    bencher.with_inputs(setup_group_recv).bench_refs(|d| {
        black_box(decrypt_group(
            &d.skmsg_bytes,
            &d.alice_addr,
            &d.group_jid,
            &mut d.bob,
        ));
    });
}
