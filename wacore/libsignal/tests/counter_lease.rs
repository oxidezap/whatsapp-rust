//! Sender-chain counter lease: outbound message keys/IVs derive
//! deterministically from the chain counter, so a counter must never be
//! re-derivable after a crash. Instead of persisting every advance before the
//! wire, the record durably reserves counters in batches
//! (`SENDER_CHAIN_RESERVATION_BATCH`) and a reloaded snapshot fast-forwards
//! past the whole lease. These tests drive the crash/reload interleavings
//! end-to-end against a real peer.
//! Async I/O uses `futures::executor::block_on` (no tokio in this crate).

use async_trait::async_trait;
use std::collections::HashMap;
use wacore_libsignal::protocol::consts::{
    MAX_RESERVATION_FAST_FORWARD, SENDER_CHAIN_RESERVATION_BATCH,
};
use wacore_libsignal::protocol::{
    CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, PreKeyBundle, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet, message_decrypt,
    message_encrypt, process_prekey_bundle,
};

// ---- in-memory store impls (clones of the session_divergence fixtures,
// kept local so this test file is self-contained) -----------------------------

#[derive(Clone)]
struct InMemoryIdentityKeyStore {
    identity_key_pair: IdentityKeyPair,
    registration_id: u32,
    identities: HashMap<ProtocolAddress, IdentityKey>,
}

#[async_trait]
impl IdentityKeyStore for InMemoryIdentityKeyStore {
    async fn get_identity_key_pair(
        &self,
    ) -> wacore_libsignal::protocol::error::Result<IdentityKeyPair> {
        Ok(self.identity_key_pair.clone())
    }
    async fn get_local_registration_id(&self) -> wacore_libsignal::protocol::error::Result<u32> {
        Ok(self.registration_id)
    }
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> wacore_libsignal::protocol::error::Result<IdentityChange> {
        let changed = self
            .identities
            .get(address)
            .is_some_and(|prev| prev != identity);
        self.identities.insert(address.clone(), *identity);
        Ok(IdentityChange::from_changed(changed))
    }
    async fn is_trusted_identity(
        &self,
        _: &ProtocolAddress,
        _: &IdentityKey,
        _: Direction,
    ) -> wacore_libsignal::protocol::error::Result<bool> {
        Ok(true)
    }
    async fn get_identity(
        &self,
        address: &ProtocolAddress,
    ) -> wacore_libsignal::protocol::error::Result<Option<IdentityKey>> {
        Ok(self.identities.get(address).cloned())
    }
}

#[derive(Default, Clone)]
struct InMemoryPreKeyStore(HashMap<PreKeyId, PreKeyRecord>);

#[async_trait]
impl PreKeyStore for InMemoryPreKeyStore {
    async fn get_pre_key(
        &self,
        id: PreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<PreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidPreKeyId)
    }
    async fn save_pre_key(
        &mut self,
        id: PreKeyId,
        record: &PreKeyRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.0.insert(id, record.clone());
        Ok(())
    }
    async fn remove_pre_key(
        &mut self,
        id: PreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.0.remove(&id);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct InMemorySignedPreKeyStore(HashMap<SignedPreKeyId, SignedPreKeyRecord>);

#[async_trait]
impl SignedPreKeyStore for InMemorySignedPreKeyStore {
    async fn get_signed_pre_key(
        &self,
        id: SignedPreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<SignedPreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidSignedPreKeyId)
    }
    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.0.insert(id, record.clone());
        Ok(())
    }
}

#[derive(Default, Clone)]
struct InMemorySessionStore(HashMap<ProtocolAddress, SessionRecord>);

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> wacore_libsignal::protocol::error::Result<Option<SessionRecord>> {
        Ok(self.0.get(address).cloned())
    }
    async fn has_session(
        &self,
        address: &ProtocolAddress,
    ) -> wacore_libsignal::protocol::error::Result<bool> {
        Ok(self.0.contains_key(address))
    }
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.0.insert(address.clone(), record);
        Ok(())
    }
}

// ---- peer fixture -----------------------------------------------------------

struct Peer {
    address: ProtocolAddress,
    identity_store: InMemoryIdentityKeyStore,
    prekey_store: InMemoryPreKeyStore,
    signed_prekey_store: InMemorySignedPreKeyStore,
    session_store: InMemorySessionStore,
}

impl Peer {
    fn new(name: &str) -> Self {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let identity_key_pair = IdentityKeyPair::generate(&mut rng);
        let registration_id = rand::random::<u32>() & 0x3FFF;

        let prekey_id: PreKeyId = 1u32.into();
        let prekey_pair = KeyPair::generate(&mut rng);

        let signed_prekey_id: SignedPreKeyId = 1u32.into();
        let signed_prekey_pair = KeyPair::generate(&mut rng);
        let signed_prekey_signature = identity_key_pair
            .private_key()
            .calculate_signature(&signed_prekey_pair.public_key.serialize(), &mut rng)
            .expect("sign");

        let mut prekey_store = InMemoryPreKeyStore::default();
        let mut signed_prekey_store = InMemorySignedPreKeyStore::default();
        futures::executor::block_on(async {
            prekey_store
                .save_pre_key(prekey_id, &PreKeyRecord::new(prekey_id, &prekey_pair))
                .await
                .unwrap();
            signed_prekey_store
                .save_signed_pre_key(
                    signed_prekey_id,
                    &SignedPreKeyRecord::new(
                        signed_prekey_id,
                        Timestamp::from_epoch_millis(0),
                        &signed_prekey_pair,
                        &signed_prekey_signature,
                    ),
                )
                .await
                .unwrap();
        });

        let bundle = PreKeyBundle::new(
            registration_id,
            1u32.into(),
            Some((prekey_id, prekey_pair.public_key)),
            signed_prekey_id,
            signed_prekey_pair.public_key,
            signed_prekey_signature.to_vec(),
            *identity_key_pair.identity_key(),
        )
        .expect("valid bundle");

        let peer = Self {
            address: ProtocolAddress::new(name, 1u32.into()),
            identity_store: InMemoryIdentityKeyStore {
                identity_key_pair,
                registration_id,
                identities: HashMap::new(),
            },
            prekey_store,
            signed_prekey_store,
            session_store: InMemorySessionStore::default(),
        };
        BUNDLES.with(|b| b.borrow_mut().insert(peer.address.clone(), bundle));
        peer
    }
}

thread_local! {
    /// Bundle published by each peer at construction, keyed by address.
    static BUNDLES: std::cell::RefCell<HashMap<ProtocolAddress, PreKeyBundle>> =
        std::cell::RefCell::new(HashMap::new());
}

// ---- helpers ----------------------------------------------------------------

fn process_bundle(initiator: &mut Peer, target: &ProtocolAddress) {
    let bundle = BUNDLES.with(|b| b.borrow().get(target).cloned().expect("bundle published"));
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(async {
        process_prekey_bundle(
            target,
            &mut initiator.session_store,
            &mut initiator.identity_store,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("prekey bundle accepted");
    });
}

fn send(from: &mut Peer, to: &ProtocolAddress, plaintext: &[u8]) -> CiphertextMessage {
    futures::executor::block_on(async {
        message_encrypt(
            plaintext,
            to,
            &mut from.session_store,
            &mut from.identity_store,
        )
        .await
        .expect("encrypt")
    })
}

fn receive(
    to: &mut Peer,
    from: &ProtocolAddress,
    ct: &CiphertextMessage,
) -> Result<Vec<u8>, SignalProtocolError> {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(async {
        message_decrypt(
            ct,
            from,
            &mut to.session_store,
            &mut to.identity_store,
            &mut to.prekey_store,
            &to.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .map(|d| d.plaintext)
    })
}

fn establish(alice: &mut Peer, bob: &mut Peer) {
    let bob_address = bob.address.clone();
    process_bundle(alice, &bob_address);
    let ct = send(alice, &bob_address, b"hello bob");
    let plaintext = receive(bob, &alice.address.clone(), &ct).expect("first pkmsg decrypts");
    assert_eq!(&plaintext[..], b"hello bob");
}

/// The wire counter of an outbound message (pkmsg or msg).
fn wire_counter(ct: &CiphertextMessage) -> u32 {
    match ct {
        CiphertextMessage::SignalMessage(m) => m.counter(),
        CiphertextMessage::PreKeySignalMessage(m) => m.message().counter(),
        other => panic!("unexpected message type {:?}", other.message_type()),
    }
}

fn record_of(peer: &Peer, remote: &ProtocolAddress) -> SessionRecord {
    peer.session_store
        .0
        .get(remote)
        .expect("session exists")
        .clone()
}

/// Live index of the record's current sender chain.
fn sender_chain_index(record: &SessionRecord) -> u32 {
    record
        .session_state()
        .expect("current session")
        .get_sender_chain_key()
        .expect("sender chain")
        .index()
}

/// Send until the lease ceiling passes `target`, mimicking a bot that
/// monologues at one peer (its own primary device gets a copy of every
/// outgoing message) without that peer ever replying.
fn monologue_until_lease_exceeds(alice: &mut Peer, bob: &ProtocolAddress, target: u32) {
    let mut sends = 0u32;
    while record_of(alice, bob).reserved_sender_chain_index() <= target {
        send(alice, bob, b"monologue");
        sends += 1;
        assert!(
            sends < target + 2 * SENDER_CHAIN_RESERVATION_BATCH,
            "runaway"
        );
    }
}

/// The two incarnations a store hands to the record: the same one for a
/// reload inside a live cache, a different one after a restart or a lossy
/// cache reset.
const LIVE_INCARNATION: [u8; 16] = [0xA1; 16];
const RESTART_INCARNATION: [u8; 16] = [0xB2; 16];

fn store_roundtrip(
    record: &SessionRecord,
    reload_as: &[u8; 16],
) -> Result<SessionRecord, SignalProtocolError> {
    let mut bytes = Vec::new();
    record.serialize_into_for_store(&mut bytes, &LIVE_INCARNATION);
    SessionRecord::deserialize_for_store(&bytes, reload_as)
}

/// Simulate the store layer acknowledging a durable flush: take over the wire
/// gate and return the serialized snapshot that "reached storage".
fn ack_flush(peer: &mut Peer, remote: &ProtocolAddress) -> Vec<u8> {
    let record = peer.session_store.0.get_mut(remote).expect("session");
    record.clear_pending_reservation();
    record.serialize().expect("serialize")
}

/// Simulate a crash: whatever was in memory is gone, the last durable
/// snapshot is what comes back.
fn crash_reload(peer: &mut Peer, remote: &ProtocolAddress, snapshot: &[u8]) {
    let restored = SessionRecord::deserialize(snapshot).expect("snapshot deserializes");
    peer.session_store.0.insert(remote.clone(), restored);
}

// ---- scenarios --------------------------------------------------------------

/// Guard for the fixture itself: `make_rng` seeds a StdRng from the OS/thread
/// entropy source, so each peer must get independent key material. Were it
/// deterministic, both peers would agree on keys by accident and every
/// assertion below would pass for the wrong reason.
#[test]
fn peers_generate_independent_keys() {
    let alice = Peer::new("alice");
    let bob = Peer::new("bob");

    assert_ne!(
        alice
            .identity_store
            .identity_key_pair
            .identity_key()
            .serialize(),
        bob.identity_store
            .identity_key_pair
            .identity_key()
            .serialize(),
        "peers must not share an identity key"
    );
    let key_of = |p: &Peer| {
        BUNDLES.with(|b| {
            b.borrow()
                .get(&p.address)
                .expect("bundle")
                .pre_key_public()
                .expect("bundle read")
                .expect("one-time prekey")
                .serialize()
        })
    };
    assert_ne!(
        key_of(&alice),
        key_of(&bob),
        "peers must not share a one-time prekey"
    );
}

/// The very first send on a fresh session must raise a lease and gate the
/// wire on its durability.
#[test]
fn first_send_raises_the_lease() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);

    let record = record_of(&alice, &bob.address);
    assert!(
        record.has_pending_reservation(),
        "first send must gate the wire on the raised lease"
    );
    assert_eq!(
        record.reserved_sender_chain_index(),
        SENDER_CHAIN_RESERVATION_BATCH,
        "counter 0 leases one full batch"
    );
}

/// Steady-state ping-pong (every send is counter 0 of a freshly ratcheted
/// chain) must never re-raise the lease: this is what removes the per-message
/// synchronous flush from the hot send path.
#[test]
fn ping_pong_sends_stay_covered_by_the_lease() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);
    ack_flush(&mut alice, &bob.address);

    for i in 0..20 {
        let reply = format!("b→a #{i}");
        let ct = send(&mut bob, &alice.address, reply.as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("decrypt reply");

        let msg = format!("a→b #{i}");
        let ct = send(&mut alice, &bob.address, msg.as_bytes());
        assert!(
            !record_of(&alice, &bob.address).has_pending_reservation(),
            "ping-pong send #{i} is covered by the durable lease and must not re-flush"
        );
        let pt = receive(&mut bob, &alice.address, &ct).expect("decrypt");
        assert_eq!(&pt[..], msg.as_bytes());
    }
}

/// A monologue re-raises the lease exactly when it runs out, one batch at a
/// time.
#[test]
fn monologue_re_raises_the_lease_at_the_batch_boundary() {
    let batch = SENDER_CHAIN_RESERVATION_BATCH;
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob); // counter 0, lease -> batch
    ack_flush(&mut alice, &bob.address);

    // Counters 1..batch-1 ride the existing lease.
    for i in 1..batch {
        let ct = send(&mut alice, &bob.address, b"streak");
        assert_eq!(wire_counter(&ct), i);
        assert!(
            !record_of(&alice, &bob.address).has_pending_reservation(),
            "counter {i} is inside the lease"
        );
    }

    // Counter `batch` exhausts it: the lease must be re-raised.
    let ct = send(&mut alice, &bob.address, b"boundary");
    assert_eq!(wire_counter(&ct), batch);
    let record = record_of(&alice, &bob.address);
    assert!(record.has_pending_reservation());
    assert_eq!(record.reserved_sender_chain_index(), batch * 2);
}

/// The core no-reuse guarantee: sends past the durable snapshot are covered
/// by its lease, so a crash/reload can never re-derive their counters — and
/// the peer keeps decrypting across the gap.
#[test]
fn crash_reload_never_reuses_a_counter_and_peer_decrypts_across_the_gap() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob); // counter 0
    let snapshot = ack_flush(&mut alice, &bob.address);

    // Five more sends after the snapshot; the durable state now trails.
    let mut spent = vec![0u32];
    for _ in 0..5 {
        let ct = send(&mut alice, &bob.address, b"unflushed");
        spent.push(wire_counter(&ct));
        receive(&mut bob, &alice.address, &ct).expect("decrypt");
    }

    crash_reload(&mut alice, &bob.address, &snapshot);

    // The reloaded chain resumes past the whole lease...
    let ct = send(&mut alice, &bob.address, b"after crash");
    let resumed = wire_counter(&ct);
    assert_eq!(
        resumed, SENDER_CHAIN_RESERVATION_BATCH,
        "reload must fast-forward to the leased ceiling"
    );
    assert!(
        !spent.contains(&resumed),
        "a wire counter must never repeat across a crash"
    );
    // ...the resumed counter exhausts the old lease, so it re-raises...
    assert!(record_of(&alice, &bob.address).has_pending_reservation());
    // ...and Bob decrypts across the gap (skipped keys for the burned range).
    let pt = receive(&mut bob, &alice.address, &ct).expect("decrypt across the gap");
    assert_eq!(&pt[..], b"after crash");
}

/// Crash after a DH ratchet whose new chain never reached storage: the
/// snapshot's OLD chain resumes past its lease, the lost chain's keys are
/// unrecoverable (fresh random ephemeral), and the peer still decrypts via
/// its retained old receiver chain.
#[test]
fn crash_reload_after_unflushed_ratchet_resumes_the_old_chain_safely() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);
    let snapshot = ack_flush(&mut alice, &bob.address);

    // Bob's reply DH-ratchets Alice onto a brand-new sender chain; her send
    // on it is lease-covered (no flush) and the chain never gets persisted.
    let ct = send(&mut bob, &alice.address, b"reply");
    receive(&mut alice, &bob.address, &ct).expect("decrypt reply");
    let ct = send(&mut alice, &bob.address, b"on the lost chain");
    assert_eq!(wire_counter(&ct), 0, "fresh chain starts at 0");
    assert!(
        !record_of(&alice, &bob.address).has_pending_reservation(),
        "the ratcheted chain send rides the record lease"
    );
    receive(&mut bob, &alice.address, &ct).expect("decrypt");

    crash_reload(&mut alice, &bob.address, &snapshot);

    // Alice resumes on the old chain, past its lease; Bob retained the old
    // receiver chain and decrypts.
    let ct = send(&mut alice, &bob.address, b"back on the old chain");
    assert_eq!(wire_counter(&ct), SENDER_CHAIN_RESERVATION_BATCH);
    let pt = receive(&mut bob, &alice.address, &ct).expect("old receiver chain still works");
    assert_eq!(&pt[..], b"back on the old chain");
}

/// Serialize/deserialize round-trip: the lease survives storage, and a
/// snapshot with no lease (legacy format) loads with a zero reservation and
/// an untouched chain.
#[test]
fn lease_round_trips_through_storage_and_legacy_records_load_untouched() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);

    let bytes = ack_flush(&mut alice, &bob.address);
    let reloaded = SessionRecord::deserialize(&bytes).expect("deserialize");
    assert_eq!(
        reloaded.reserved_sender_chain_index(),
        SENDER_CHAIN_RESERVATION_BATCH
    );
    assert!(!reloaded.has_pending_reservation(), "the gate is transient");

    // A legacy record (serialized before the lease existed) must load with a
    // zero reservation. `new_fresh` never leases, so its encoding matches the
    // legacy layout exactly.
    let legacy = SessionRecord::new_fresh().serialize().expect("serialize");
    let reloaded = SessionRecord::deserialize(&legacy).expect("legacy deserializes");
    assert_eq!(reloaded.reserved_sender_chain_index(), 0);
}

// ---- stranded lease after a DH ratchet (issue #1146) ------------------------
//
// The lease ceiling is a record-level counter; the sender chain it bounds is
// per-ratchet-epoch and restarts at zero every time the peer replies. A long
// monologue followed by one reply used to leave the ceiling stranded thousands
// of counters above the live index, which no send ever created — and which a
// recovery reload could neither burn nor accept.

/// The regression itself: after the ratchet the ceiling must describe the
/// chain that is actually installed, not the one that was retired.
#[test]
fn a_dh_ratchet_rebases_the_lease_onto_the_fresh_chain() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);

    // Alice monologues past the load-time fast-forward ceiling. A bot that
    // copies every outgoing message to its own primary device reaches this in
    // a couple of thousand sends.
    monologue_until_lease_exceeds(&mut alice, &bob.address, MAX_RESERVATION_FAST_FORWARD);
    let stranded_ceiling = record_of(&alice, &bob.address).reserved_sender_chain_index();
    assert!(stranded_ceiling > MAX_RESERVATION_FAST_FORWARD);

    // Bob finally replies: Alice DH-ratchets onto a chain that starts at zero.
    let ct = send(&mut bob, &alice.address, b"reply");
    receive(&mut alice, &bob.address, &ct).expect("decrypt reply");

    let record = record_of(&alice, &bob.address);
    assert_eq!(sender_chain_index(&record), 0, "fresh chain starts at 0");
    assert!(
        record.reserved_sender_chain_index() <= SENDER_CHAIN_RESERVATION_BATCH,
        "the retired chain's ceiling ({}) must not survive onto the fresh chain",
        record.reserved_sender_chain_index()
    );
}

/// The operator-visible symptom: a stranded ceiling turned the stored row
/// into a hard load failure for every path that touches the address — decrypt,
/// encrypt, and retry repair alike — and only after a restart or a lossy cache
/// reset, since a live reload skips the fast-forward entirely.
#[test]
fn a_ratcheted_record_still_loads_after_a_restart() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);
    monologue_until_lease_exceeds(&mut alice, &bob.address, MAX_RESERVATION_FAST_FORWARD);

    let ct = send(&mut bob, &alice.address, b"reply");
    receive(&mut alice, &bob.address, &ct).expect("decrypt reply");
    let record = record_of(&alice, &bob.address);

    store_roundtrip(&record, &LIVE_INCARNATION).expect("a live reload never fast-forwards");
    let recovered =
        store_roundtrip(&record, &RESTART_INCARNATION).expect("a restart must not strand the row");
    assert!(
        recovered.reserved_sender_chain_index() - sender_chain_index(&recovered)
            <= SENDER_CHAIN_RESERVATION_BATCH,
        "recovery must burn at most one batch"
    );
}

/// Performance guard: rebasing must not cost the fresh chain its lease
/// coverage. Dropping the ceiling to zero instead of one batch would put a
/// synchronous durability flush in front of every reply in a ping-pong.
#[test]
fn the_rebased_lease_still_covers_the_fresh_chain_without_a_flush() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);
    monologue_until_lease_exceeds(&mut alice, &bob.address, MAX_RESERVATION_FAST_FORWARD);
    ack_flush(&mut alice, &bob.address);

    let ct = send(&mut bob, &alice.address, b"reply");
    receive(&mut alice, &bob.address, &ct).expect("decrypt reply");

    for counter in 0..SENDER_CHAIN_RESERVATION_BATCH {
        let ct = send(&mut alice, &bob.address, b"post-ratchet");
        assert_eq!(wire_counter(&ct), counter);
        assert!(
            !record_of(&alice, &bob.address).has_pending_reservation(),
            "counter {counter} of the fresh chain must ride the rebased lease"
        );
    }

    // ...and the batch boundary still re-raises and re-gates as before.
    let ct = send(&mut alice, &bob.address, b"boundary");
    assert_eq!(wire_counter(&ct), SENDER_CHAIN_RESERVATION_BATCH);
    assert!(record_of(&alice, &bob.address).has_pending_reservation());
}

/// Lowering a ceiling is only safe if it can never uncover a counter that
/// already reached the wire. Publish across the ratchet and both sides of a
/// crash, and assert the resumed chain repeats nothing.
#[test]
fn a_rebased_lease_never_republishes_a_counter_across_a_crash() {
    let mut alice = Peer::new("alice");
    let mut bob = Peer::new("bob");
    establish(&mut alice, &mut bob);
    monologue_until_lease_exceeds(&mut alice, &bob.address, MAX_RESERVATION_FAST_FORWARD);
    ack_flush(&mut alice, &bob.address);

    let ct = send(&mut bob, &alice.address, b"reply");
    receive(&mut alice, &bob.address, &ct).expect("decrypt reply");

    // Everything below is on the post-ratchet chain, so counters are
    // comparable across the crash.
    let mut published = Vec::new();
    for _ in 0..3 {
        published.push(wire_counter(&send(
            &mut alice,
            &bob.address,
            b"pre-snapshot",
        )));
    }
    let snapshot = ack_flush(&mut alice, &bob.address);
    for _ in 0..5 {
        published.push(wire_counter(&send(&mut alice, &bob.address, b"unflushed")));
    }

    crash_reload(&mut alice, &bob.address, &snapshot);

    let ct = send(&mut alice, &bob.address, b"after crash");
    let resumed = wire_counter(&ct);
    assert!(
        !published.contains(&resumed),
        "counter {resumed} was already published before the crash"
    );
    assert_eq!(
        resumed, SENDER_CHAIN_RESERVATION_BATCH,
        "recovery burns the rebased lease, not the retired chain's ceiling"
    );
    let pt = receive(&mut bob, &alice.address, &ct).expect("bob decrypts across the burned gap");
    assert_eq!(&pt[..], b"after crash");
}

// ---- waiver -----------------------------------------------------------------

/// Stand in for a consumer whose persistence is a component export: it rebuilds
/// the record from components on every load, and waives the lease because its
/// writes are durable before the ciphertext reaches the wire.
fn components_roundtrip(peer: &mut Peer, remote: &ProtocolAddress, waive: bool) {
    let record = peer.session_store.0.remove(remote).expect("session exists");
    let mut rebuilt =
        SessionRecord::from_components(record.into_components().expect("export")).expect("import");
    if waive {
        rebuilt.waive_counter_lease();
    }
    peer.session_store.0.insert(remote.clone(), rebuilt);
}

/// Skipped message keys the peer had to buffer, across every receiver chain.
fn skipped_keys(peer: &Peer, remote: &ProtocolAddress) -> usize {
    record_of(peer, remote)
        .into_components()
        .expect("components")
        .current_session
        .expect("current session")
        .receiver_chains
        .iter()
        .map(|chain| chain.message_keys.len())
        .sum()
}

/// Drive `count` sends, exporting and reimporting components between each, and
/// return the wire counters the peer saw.
fn exported_send_run(alice: &mut Peer, bob: &mut Peer, count: u32, waive: bool) -> Vec<u32> {
    let bob_address = bob.address.clone();
    let alice_address = alice.address.clone();
    process_bundle(alice, &bob_address);
    components_roundtrip(alice, &bob_address, waive);

    let mut counters = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let ct = send(alice, &bob_address, b"m");
        counters.push(wire_counter(&ct));
        receive(bob, &alice_address, &ct).expect("peer decrypts");
        components_roundtrip(alice, &bob_address, waive);
    }
    counters
}

/// The symptom: a consumer that exports components burns a whole batch per
/// export, so consecutive sends land 64 apart and the peer buffers 63 skipped
/// keys for each one. Under a waived lease the counters are consecutive and
/// nothing is skipped.
#[test]
fn a_waived_lease_keeps_exported_counters_consecutive() {
    let mut alice = Peer::new("alice-waived");
    let mut bob = Peer::new("bob-waived");

    let counters = exported_send_run(&mut alice, &mut bob, 8, true);

    assert_eq!(counters, (0..8).collect::<Vec<_>>());
    assert_eq!(skipped_keys(&bob, &alice.address.clone()), 0);
}

/// The default is untouched: the reservation is still created, so an export
/// still materializes it and the counters still stride by a batch.
#[test]
fn the_default_lease_still_burns_a_batch_per_export() {
    let mut alice = Peer::new("alice-leased");
    let mut bob = Peer::new("bob-leased");

    let counters = exported_send_run(&mut alice, &mut bob, 4, false);

    let batch = SENDER_CHAIN_RESERVATION_BATCH;
    assert_eq!(counters, vec![0, batch, batch * 2, batch * 3]);
    assert!(
        skipped_keys(&bob, &alice.address.clone()) > 0,
        "the leased run must leave the peer with skipped keys"
    );
}

/// The waiver removes the reservation, not just its materialization: a send
/// under it must not gate the ciphertext on a flush that no longer protects
/// anything.
#[test]
fn a_waived_lease_never_gates_the_wire() {
    let mut alice = Peer::new("alice-ungated");
    let mut bob = Peer::new("bob-ungated");
    let bob_address = bob.address.clone();

    process_bundle(&mut alice, &bob_address);
    components_roundtrip(&mut alice, &bob_address, true);
    for _ in 0..3 {
        let ct = send(&mut alice, &bob_address, b"m");
        receive(&mut bob, &alice.address.clone(), &ct).expect("peer decrypts");
        let record = record_of(&alice, &bob_address);
        assert_eq!(record.reserved_sender_chain_index(), 0);
        assert!(!record.has_pending_reservation());
    }
}

/// Under the default, the same run keeps gating and reserving.
#[test]
fn the_default_lease_still_gates_the_wire() {
    let mut alice = Peer::new("alice-gated");
    let mut bob = Peer::new("bob-gated");
    let bob_address = bob.address.clone();

    process_bundle(&mut alice, &bob_address);
    let ct = send(&mut alice, &bob_address, b"m");
    receive(&mut bob, &alice.address.clone(), &ct).expect("peer decrypts");

    let record = record_of(&alice, &bob_address);
    assert_eq!(
        record.reserved_sender_chain_index(),
        SENDER_CHAIN_RESERVATION_BATCH
    );
    assert!(record.has_pending_reservation());
}

/// A record written while the lease was in force may already have published
/// counters below its ceiling, and waiving does not make that untrue. The
/// ceiling is materialized once, then counters run consecutively from there.
#[test]
fn waiving_materializes_a_previously_reserved_ceiling_once() {
    let mut alice = Peer::new("alice-preexisting");
    let mut bob = Peer::new("bob-preexisting");
    let bob_address = bob.address.clone();
    let alice_address = alice.address.clone();

    // Build up a real reservation under the default lease.
    establish(&mut alice, &mut bob);
    let ceiling = record_of(&alice, &bob_address).reserved_sender_chain_index();
    assert_eq!(ceiling, SENDER_CHAIN_RESERVATION_BATCH);

    // The consumer turns the waiver on and reloads the record it already had.
    let record = alice
        .session_store
        .0
        .get_mut(&bob_address)
        .expect("session");
    record.waive_counter_lease();
    assert_eq!(record.reserved_sender_chain_index(), 0);

    let first = send(&mut alice, &bob_address, b"after waiving");
    assert_eq!(wire_counter(&first), ceiling);
    receive(&mut bob, &alice_address, &first).expect("peer decrypts across the burn");

    let second = send(&mut alice, &bob_address, b"and the next");
    assert_eq!(wire_counter(&second), ceiling + 1);
    receive(&mut bob, &alice_address, &second).expect("peer decrypts");
}

/// Archived states were covered by the same ceiling, so waiving has to burn
/// them too: once the lease is gone, a promotion has nothing left telling it
/// those counters may already be on the wire.
#[test]
fn waiving_burns_the_ceiling_into_archived_states_too() {
    let mut alice = Peer::new("alice-archived");
    let mut bob = Peer::new("bob-archived");
    let bob_address = bob.address.clone();

    establish(&mut alice, &mut bob);
    let ceiling = record_of(&alice, &bob_address).reserved_sender_chain_index();
    assert_eq!(ceiling, SENDER_CHAIN_RESERVATION_BATCH);

    // Archive the leased state, then waive.
    let record = alice
        .session_store
        .0
        .get_mut(&bob_address)
        .expect("session");
    record
        .archive_current_state()
        .expect("archive the leased state");
    record.waive_counter_lease();

    let archived_index = record_of(&alice, &bob_address)
        .into_components()
        .expect("components")
        .previous_sessions
        .first()
        .expect("archived state")
        .sender_chain
        .as_ref()
        .expect("sender chain")
        .chain_key
        .as_ref()
        .expect("chain key")
        .index;
    assert_eq!(archived_index, Some(ceiling));
}
