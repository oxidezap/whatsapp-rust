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
use wacore_libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH;
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
            address: ProtocolAddress::new(name.to_string(), 1u32.into()),
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
