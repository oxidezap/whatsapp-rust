//! Hypotheses about how Alice's local session could end up unable to
//! decrypt despite Bob's encryption being deterministic from a shared
//! root key. Used to chase a deadlock where failed-MAC attempts kept
//! advancing the receiver chain past the peer's actual position.
//! Async I/O uses `futures::executor::block_on` (no tokio in this crate).
#![allow(clippy::too_many_lines)]

use async_trait::async_trait;
use std::collections::HashMap;
use wacore_libsignal::protocol::{
    CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, PreKeyBundle, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet, message_decrypt,
    message_encrypt, process_prekey_bundle,
};

// ---- in-memory store impls (clones of the bench fixtures, kept local
// so this test file is self-contained) ---------------------------------------

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

#[derive(Clone)]
struct Peer {
    address: ProtocolAddress,
    identity_store: InMemoryIdentityKeyStore,
    prekey_store: InMemoryPreKeyStore,
    signed_prekey_store: InMemorySignedPreKeyStore,
    session_store: InMemorySessionStore,
    /// Most recently issued prekey id — bumped each time the peer generates
    /// a fresh bundle so the receiver doesn't reuse a one-time-prekey.
    next_prekey_id: u32,
    /// Most recently published one-time prekey pair, mirrored alongside
    /// `next_prekey_id` so callers can build a bundle without re-walking
    /// the prekey store.
    prekey_pair: KeyPair,
    /// Current signed prekey id + pair. Always device-stable; rotated only
    /// when the test explicitly simulates a server-side rotation.
    signed_prekey_id: SignedPreKeyId,
    signed_prekey_pair: KeyPair,
    signed_prekey_signature: Vec<u8>,
}

impl Peer {
    fn new(name: &str, device_id: u32) -> Self {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let identity_key_pair = IdentityKeyPair::generate(&mut rng);
        let registration_id = rand::random::<u32>() & 0x3FFF;

        let prekey_id_int = 1u32;
        let prekey_id: PreKeyId = prekey_id_int.into();
        let prekey_pair = KeyPair::generate(&mut rng);
        let prekey_record = PreKeyRecord::new(prekey_id, &prekey_pair);

        let signed_prekey_id: SignedPreKeyId = 1u32.into();
        let signed_prekey_pair = KeyPair::generate(&mut rng);
        let signed_prekey_signature = identity_key_pair
            .private_key()
            .calculate_signature(&signed_prekey_pair.public_key.serialize(), &mut rng)
            .expect("sign");
        let signed_prekey_record = SignedPreKeyRecord::new(
            signed_prekey_id,
            Timestamp::from_epoch_millis(0),
            &signed_prekey_pair,
            &signed_prekey_signature,
        );

        let identity_store = InMemoryIdentityKeyStore {
            identity_key_pair,
            registration_id,
            identities: HashMap::new(),
        };
        let mut prekey_store = InMemoryPreKeyStore::default();
        let mut signed_prekey_store = InMemorySignedPreKeyStore::default();

        futures::executor::block_on(async {
            prekey_store
                .save_pre_key(prekey_id, &prekey_record)
                .await
                .unwrap();
            signed_prekey_store
                .save_signed_pre_key(signed_prekey_id, &signed_prekey_record)
                .await
                .unwrap();
        });

        Self {
            address: ProtocolAddress::new(name.to_string(), device_id.into()),
            identity_store,
            prekey_store,
            signed_prekey_store,
            session_store: InMemorySessionStore::default(),
            next_prekey_id: prekey_id_int,
            prekey_pair,
            signed_prekey_id,
            signed_prekey_pair,
            signed_prekey_signature: signed_prekey_signature.to_vec(),
        }
    }

    fn bundle(&self) -> PreKeyBundle {
        PreKeyBundle::new(
            self.identity_store.registration_id,
            1u32.into(),
            Some((self.next_prekey_id.into(), self.prekey_pair.public_key)),
            self.signed_prekey_id,
            self.signed_prekey_pair.public_key,
            self.signed_prekey_signature.clone(),
            *self.identity_store.identity_key_pair.identity_key(),
        )
        .expect("valid bundle")
    }

    /// Generate a brand-new one-time prekey and publish it locally,
    /// rotating `next_prekey_id`. Used to model the bot uploading a
    /// fresh one-time prekey alongside a retry-receipt-with-keys.
    fn rotate_one_time_prekey(&mut self) {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let new_id = self.next_prekey_id + 1;
        let new_pair = KeyPair::generate(&mut rng);
        let id: PreKeyId = new_id.into();
        let record = PreKeyRecord::new(id, &new_pair);
        futures::executor::block_on(async {
            self.prekey_store.save_pre_key(id, &record).await.unwrap();
        });
        self.next_prekey_id = new_id;
        self.prekey_pair = new_pair;
    }
}

// ---- helpers ----------------------------------------------------------------

/// Hand `bob` Alice's bundle so he can speak to her. Mirrors the bot
/// pulling a fresh prekey bundle for its primary phone via
/// `ensure_e2e_sessions` and calling `process_prekey_bundle`.
fn process_bundle(initiator: &mut Peer, target_address: &ProtocolAddress, bundle: &PreKeyBundle) {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(async {
        process_prekey_bundle(
            target_address,
            &mut initiator.session_store,
            &mut initiator.identity_store,
            bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("prekey bundle accepted");
    });
}

/// `from` encrypts `plaintext` for `to`, returns the wire bytes + the
/// kind of stanza it produced (pkmsg on a fresh session, msg afterwards).
/// Mirrors `message_encrypt` on the bot.
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

/// Inverse: `to` decrypts. Returns the plaintext or the SignalProtocolError
/// that fired, so tests can assert on the specific failure mode (BadMac vs
/// SessionNotFound vs DuplicatedMessage) the way the bot's message.rs does.
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

/// Establish a working session: Alice has Bob's bundle and sends one
/// `pkmsg` so Bob has a session record on his side too. After this both
/// sides hold a current session and can exchange msgs in either direction.
fn establish(alice: &mut Peer, bob: &mut Peer) {
    let bundle = bob.bundle();
    process_bundle(alice, &bob.address, &bundle);

    let ct = send(alice, &bob.address, b"hello bob");
    let plaintext = receive(bob, &alice.address, &ct).expect("first pkmsg decrypts");
    assert_eq!(&plaintext[..], b"hello bob");
}

// ---- scenarios --------------------------------------------------------------

/// Sanity. Baseline ping-pong over a single session. If this regresses
/// nothing else in the file means anything.
#[test]
fn baseline_dm_ping_pong() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    for i in 0..10 {
        let msg = format!("a→b #{i}");
        let ct = send(&mut alice, &bob.address, msg.as_bytes());
        let pt = receive(&mut bob, &alice.address, &ct).expect("decrypt");
        assert_eq!(&pt[..], msg.as_bytes());

        let reply = format!("b→a #{i}");
        let ct = send(&mut bob, &alice.address, reply.as_bytes());
        let pt = receive(&mut alice, &bob.address, &ct).expect("decrypt");
        assert_eq!(&pt[..], reply.as_bytes());
    }
}

/// Prod-like long-running chain. Bob sends N msgs straight at Alice
/// (sender-chain advance without DH-rotating intermissions, the way
/// the user's Android phone does on a streak of self-DMs). Alice must
/// keep decrypting; if she falls behind once the chain is past
/// ~100 counters we'd be reproducing prod.
#[test]
fn long_sender_chain_alice_keeps_up() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    for i in 0..1000 {
        let payload = format!("b→a #{i}");
        let ct = send(&mut bob, &alice.address, payload.as_bytes());
        let pt = receive(&mut alice, &bob.address, &ct)
            .unwrap_or_else(|e| panic!("counter {i} failed: {e:?}"));
        assert_eq!(&pt[..], payload.as_bytes());
    }
}

/// The "PDO loop" hypothesis: Alice's bot keeps building fresh sessions
/// against Bob's prekey bundle (every retry receipt with keys / every
/// `ensure_e2e_sessions` for a peer message) while Bob's outbound
/// chain is unchanged. After several rebuilds Alice still has the
/// originally-working session inside `previous_sessions[N]`; libsignal
/// is supposed to iterate previous sessions on BadMac and find it.
#[test]
fn alice_rebuilds_session_repeatedly_old_chain_still_decrypts() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Bob advances his send chain a bit so the working session has
    // some history (mirrors the chain index ~846 we saw in prod).
    for i in 0..50 {
        let ct = send(&mut bob, &alice.address, format!("pre {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("pre-rotate decrypts");
    }

    // Alice repeatedly rebuilds her session with Bob from fresh prekey
    // bundles (one-time prekey rotated each time, matching the bot's
    // retry-receipt-with-keys + PDO pkmsg sends in prod).
    for _ in 0..6 {
        bob.rotate_one_time_prekey();
        let new_bundle = bob.bundle();
        process_bundle(&mut alice, &bob.address, &new_bundle);
    }

    // Bob hasn't seen any of those rebuilds — he keeps using his
    // original send chain. Alice's CURRENT session won't decrypt this
    // (different root key), so libsignal has to walk back through the
    // previous_sessions list and find the original.
    let ct = send(&mut bob, &alice.address, b"old-chain msg after rebuilds");
    let pt = receive(&mut alice, &bob.address, &ct)
        .expect("must decrypt via archived previous_sessions[N]");
    assert_eq!(&pt[..], b"old-chain msg after rebuilds");
}

/// Same as above but Bob's chain is much longer (closer to the prod
/// counter ~940). If chain-step costs or `MAX_MESSAGE_KEYS` eviction
/// breaks the lookup at deep chains we'd hit it here.
#[test]
fn deep_chain_survives_repeat_rebuilds() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Drive both chains to ~900 like prod (bot's receiver chain index
    // was at 846, Bob's counter at 940 in the latest log).
    for i in 0..900 {
        let ct = send(&mut bob, &alice.address, format!("warm {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("warm decrypts");
    }

    for _ in 0..6 {
        bob.rotate_one_time_prekey();
        let new_bundle = bob.bundle();
        process_bundle(&mut alice, &bob.address, &new_bundle);
    }

    let ct = send(&mut bob, &alice.address, b"deep chain after rebuilds");
    let pt =
        receive(&mut alice, &bob.address, &ct).expect("deep chain must decrypt via archived state");
    assert_eq!(&pt[..], b"deep chain after rebuilds");
}

/// The "DB delete" hypothesis: someone wipes Alice's session blob
/// entirely (matches the manual SQLite DELETE we did in prod). Bob's
/// next outbound is a `msg` (Whisper) — Alice has no record for the
/// address so this surfaces as SessionNotFound. Asserts the exact
/// error variant so the bot's retry-decision code keeps fanning out
/// keys correctly.
#[test]
fn alice_loses_session_entirely_yields_session_not_found() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    for i in 0..5 {
        let ct = send(&mut bob, &alice.address, format!("warm {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("warm decrypts");
    }

    // Drop Alice's record. Mirrors `DELETE FROM sessions WHERE
    // address = '<bob>.0'`.
    alice.session_store.0.remove(&bob.address);

    let ct = send(&mut bob, &alice.address, b"first after delete");
    let err = receive(&mut alice, &bob.address, &ct).unwrap_err();
    assert!(
        matches!(err, SignalProtocolError::SessionNotFound(_)),
        "expected SessionNotFound, got {err:?}"
    );
}

/// Wiping the session record then rebuilding via prekey bundle while the
/// peer still sends from the old chain is unrecoverable at the libsignal
/// layer — the new current_session can't decrypt the old-chain msg and
/// there's no archived previous. Motivates the LID-keeps-PN policy.
#[test]
fn alice_delete_then_rebuild_loses_old_chain() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    for i in 0..50 {
        let ct = send(&mut bob, &alice.address, format!("warm {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("warm decrypts");
    }

    alice.session_store.0.remove(&bob.address);

    bob.rotate_one_time_prekey();
    let bundle = bob.bundle();
    process_bundle(&mut alice, &bob.address, &bundle);

    let ct = send(&mut bob, &alice.address, b"old chain after delete");
    let err = receive(&mut alice, &bob.address, &ct).unwrap_err();
    assert!(
        matches!(
            err,
            SignalProtocolError::BadMac(_) | SignalProtocolError::InvalidMessage(..)
        ),
        "expected BadMac/InvalidMessage on old-chain msg after rebuild, got {err:?}"
    );
}

#[test]
fn pkmsg_reset_does_not_fix_peer_outbound_if_delivered_to_wrong_store_key() {
    let bob_lid = ProtocolAddress::new("100000000000001@lid".to_string(), 0.into());
    let bob_pn = ProtocolAddress::new("15550001000@c.us".to_string(), 0.into());

    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("100000000000001@lid", 0);
    establish(&mut alice, &mut bob);

    let warm = send(&mut bob, &alice.address, b"old chain warm");
    receive(&mut alice, &bob_lid, &warm).expect("old LID-keyed session decrypts");

    alice.session_store.0.remove(&bob_lid);

    bob.rotate_one_time_prekey();
    let bundle = bob.bundle();
    let mut wrong_route_bob = bob.clone();
    process_bundle(&mut alice, &bob_pn, &bundle);
    let wrong_reset = send(&mut alice, &bob_pn, b"reset over wrong key");
    assert!(matches!(
        wrong_reset,
        CiphertextMessage::PreKeySignalMessage(_)
    ));
    let reset_plaintext =
        receive(&mut wrong_route_bob, &alice.address, &wrong_reset).expect("reset decrypts");
    assert_eq!(&reset_plaintext[..], b"reset over wrong key");

    let old_chain_msg = send(&mut bob, &alice.address, b"old chain still active");
    let err = receive(&mut alice, &bob_lid, &old_chain_msg).unwrap_err();
    assert!(
        matches!(err, SignalProtocolError::SessionNotFound(_)),
        "wrong-key reset must not populate Alice's LID record, got {err:?}"
    );

    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("100000000000001@lid", 0);
    establish(&mut alice, &mut bob);

    alice.session_store.0.remove(&bob_lid);
    bob.rotate_one_time_prekey();
    let bundle = bob.bundle();
    process_bundle(&mut alice, &bob_lid, &bundle);
    let correct_reset = send(&mut alice, &bob_lid, b"reset over correct key");
    assert!(matches!(
        correct_reset,
        CiphertextMessage::PreKeySignalMessage(_)
    ));
    let reset_plaintext =
        receive(&mut bob, &alice.address, &correct_reset).expect("correct reset decrypts");
    assert_eq!(&reset_plaintext[..], b"reset over correct key");

    let promoted_msg = send(&mut bob, &alice.address, b"new chain active");
    let plaintext = receive(&mut alice, &bob_lid, &promoted_msg)
        .expect("correct-key reset promotes Bob's next outbound");
    assert_eq!(&plaintext[..], b"new chain active");
}

/// A failed-MAC decryption attempt must not advance Alice's receiver
/// chain — otherwise repeated junk ciphertexts walk the chain past
/// the peer's position and recovery becomes impossible. Bombards with
/// tampered ciphertexts and asserts the chain index is unchanged.
#[test]
fn failed_mac_must_not_advance_receiver_chain() {
    use wacore_libsignal::protocol::SignalMessage;

    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Warm Bob's send chain so Alice has the receiver chain set up
    // and we have a captured pristine state.
    for i in 0..3 {
        let ct = send(&mut bob, &alice.address, format!("warm {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("warm decrypts");
    }

    // Snapshot Alice's chain index for Bob's current sender ratchet.
    // Sum of receiver chain indices across all of Alice's chains for
    // Bob's address. We sum (rather than read one specific chain)
    // because after X3DH the session has the signed-prekey ratchet at
    // index 0 that stays unused, and after Bob's first send Alice
    // adds a second chain for Bob's actual sender ratchet. The
    // invariant the test wants is "no advance on MAC failure" — sum
    // captures it without depending on which chain is at which
    // vec position.
    fn alice_chain_total(alice: &Peer, bob: &Peer) -> u32 {
        let Some(rec) = alice.session_store.0.get(&bob.address) else {
            return 0;
        };
        let Some(current) = rec.session_state() else {
            return 0;
        };
        current
            .all_receiver_chain_logging_info()
            .into_iter()
            .filter_map(|(_pubkey, idx)| idx)
            .sum()
    }
    let index_before = alice_chain_total(&alice, &bob);
    assert!(
        index_before > 0,
        "post-warm Alice's receiver chain must have advanced"
    );

    // Full byte-level snapshot — a chain-index check alone would miss a
    // partial rollback that restores indices but corrupts message_keys,
    // root_key, or previous_counter.
    let bytes_before = alice
        .session_store
        .0
        .get(&bob.address)
        .expect("alice has session for bob")
        .serialize()
        .expect("serialize before tamper rounds");

    // Fabricate a real ciphertext from Bob then corrupt the trailing
    // MAC bytes. The header (version + ratchet pubkey + counter) stays
    // valid so Alice walks the same chain-derive path she would on a
    // real msg; only verify_mac fails.
    for tamper_round in 0..10u32 {
        let ct = send(&mut bob, &alice.address, b"clean");
        let bytes = ct.serialize().to_vec();
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x80;
        let parsed = SignalMessage::try_from(&tampered[..])
            .expect("tampered bytes still parse as SignalMessage");
        let bad = CiphertextMessage::SignalMessage(parsed);
        let err = receive(&mut alice, &bob.address, &bad).unwrap_err();
        assert!(
            matches!(err, SignalProtocolError::BadMac(_)),
            "round {tamper_round} expected BadMac, got {err:?}"
        );
        let now = alice_chain_total(&alice, &bob);
        assert_eq!(
            now, index_before,
            "round {tamper_round}: failed-MAC attempt advanced chain"
        );
        let bytes_now = alice
            .session_store
            .0
            .get(&bob.address)
            .unwrap()
            .serialize()
            .unwrap();
        assert_eq!(
            bytes_before, bytes_now,
            "round {tamper_round}: failed-MAC must leave the session record \
             byte-identical; a partial restore would let other fields drift"
        );
    }
}

/// Out-of-order delivery within a single chain. Tests the
/// `MAX_MESSAGE_KEYS = 2000` skipped-keys buffer — Alice must hold
/// msg_keys for indices N+1..N+K and use them when the late msg shows
/// up.
#[test]
fn out_of_order_within_chain() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Bob produces 10 ciphertexts without Alice consuming.
    let mut pending = Vec::new();
    for i in 0..10 {
        let ct = send(&mut bob, &alice.address, format!("ooo {i}").as_bytes());
        pending.push((i, ct));
    }

    // Alice consumes in reverse order; the chain index has to jump
    // forward (saving message_keys) then walk the saved keys for the
    // earlier indices.
    pending.reverse();
    for (i, ct) in pending {
        let pt =
            receive(&mut alice, &bob.address, &ct).unwrap_or_else(|e| panic!("ooo {i}: {e:?}"));
        assert_eq!(&pt[..], format!("ooo {i}").as_bytes());
    }
}

/// Mid-stream DH ratchet step: Bob sends a few msgs, Alice replies
/// (forcing Bob's send chain to ratchet), Bob sends more. Each msg
/// must still decrypt — this exercises `with_receiver_chain` paths and
/// confirms the chain switch isn't what hits prod.
#[test]
fn dh_ratchet_step_preserves_decryption() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    for i in 0..5 {
        let ct = send(&mut bob, &alice.address, format!("pre {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("pre");
    }
    // Alice's reply triggers a DH ratchet step on Bob's side for his
    // next send.
    let ct = send(&mut alice, &bob.address, b"ratchet me");
    receive(&mut bob, &alice.address, &ct).expect("bob");

    for i in 0..5 {
        let ct = send(&mut bob, &alice.address, format!("post {i}").as_bytes());
        let pt =
            receive(&mut alice, &bob.address, &ct).unwrap_or_else(|e| panic!("post {i}: {e:?}"));
        assert_eq!(&pt[..], format!("post {i}").as_bytes());
    }
}

/// `process_prekey_bundle` while Bob has unconsumed in-flight msgs.
/// In prod the bot rebuilds the session via PDO while Android still
/// has earlier msgs queued. Alice must serve those queued msgs from
/// the archived previous session even though she's promoted a new
/// current.
#[test]
fn in_flight_msgs_decrypt_through_archived_session_after_rebuild() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Warm the chain to a non-trivial index so the archived state is
    // doing real work, not the trivial counter=0 case.
    for i in 0..30 {
        let ct = send(&mut bob, &alice.address, format!("warm {i}").as_bytes());
        receive(&mut alice, &bob.address, &ct).expect("warm");
    }

    // Bob queues a handful while Alice doesn't decrypt them yet.
    let mut queue = Vec::new();
    for i in 0..5 {
        let ct = send(&mut bob, &alice.address, format!("queued {i}").as_bytes());
        queue.push((i, ct));
    }

    // Alice rebuilds. New current is fresh; the chain Bob is on now
    // lives in previous_sessions[0].
    bob.rotate_one_time_prekey();
    let bundle = bob.bundle();
    process_bundle(&mut alice, &bob.address, &bundle);

    for (i, ct) in queue {
        let pt =
            receive(&mut alice, &bob.address, &ct).unwrap_or_else(|e| panic!("queued {i}: {e:?}"));
        assert_eq!(&pt[..], format!("queued {i}").as_bytes());
    }
}

/// Tampered pkmsg must NOT persist the promoted-but-unusable session.
/// `process_prekey` runs successfully (the prekey header is well-formed),
/// then the inner decrypt fails on the tampered payload with BadMac. Pre-fix,
/// `message_decrypt_prekey` would still call `store_session` on the mutated
/// record, replacing the receiver's current_session with one only an attacker
/// could write to. The fix snapshots the record before `process_prekey` and
/// restores it on inner failure.
#[test]
fn pkmsg_decrypt_failure_does_not_persist_promoted_session() {
    use wacore_libsignal::protocol::{PreKeySignalMessage, SignalMessage};

    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);

    // Alice gets Bob's bundle, encrypts a pkmsg. Bob has no session yet.
    let bundle = bob.bundle();
    process_bundle(&mut alice, &bob.address, &bundle);
    let ct = send(&mut alice, &bob.address, b"hello bob");

    // Bob's store is empty for Alice — precondition for the bug.
    let bob_pre = futures::executor::block_on(async {
        bob.session_store
            .load_session(&alice.address)
            .await
            .unwrap()
    });
    assert!(
        bob_pre.is_none(),
        "precondition: Bob has no session for Alice before the tampered pkmsg arrives"
    );

    // Tamper the inner SignalMessage's MAC. Pkmsg is protobuf-encoded so
    // a byte-flip on the wire bytes breaks parsing; rebuild the pkmsg via
    // PreKeySignalMessage::new with a tampered inner. process_prekey only
    // validates the header (prekey refs + identity_key + base_key signature)
    // so the rebuilt pkmsg still passes that step; the inner verify_mac
    // then fires.
    let CiphertextMessage::PreKeySignalMessage(pkmsg) = &ct else {
        panic!("Alice's fresh-session encrypt must produce a pkmsg, got {ct:?}");
    };
    let inner = pkmsg.message();
    let mut inner_bytes = inner.serialized().to_vec();
    let last = inner_bytes.len() - 1;
    inner_bytes[last] ^= 0x80;
    let tampered_inner = SignalMessage::try_from(&inner_bytes[..])
        .expect("tampered inner bytes still parse as SignalMessage");
    let tampered_pkmsg = PreKeySignalMessage::new(
        pkmsg.message_version(),
        pkmsg.registration_id(),
        pkmsg.pre_key_id(),
        pkmsg.signed_pre_key_id(),
        *pkmsg.base_key(),
        *pkmsg.identity_key(),
        tampered_inner,
    )
    .expect("reconstructed pkmsg with tampered inner");
    let tampered = CiphertextMessage::PreKeySignalMessage(tampered_pkmsg);

    let err = receive(&mut bob, &alice.address, &tampered)
        .expect_err("tampered payload must fail decrypt");
    assert!(
        matches!(
            err,
            SignalProtocolError::BadMac(_) | SignalProtocolError::InvalidMessage(_, _)
        ),
        "expected BadMac/InvalidMessage on tampered pkmsg, got {err:?}"
    );

    let bob_post = futures::executor::block_on(async {
        bob.session_store
            .load_session(&alice.address)
            .await
            .unwrap()
    });
    assert!(
        bob_post.is_none(),
        "BadMac on pkmsg must NOT persist the promoted session — an attacker \
         who can craft pkmsg headers with valid prekeys could otherwise force \
         the receiver into a session only they can write to."
    );
}
