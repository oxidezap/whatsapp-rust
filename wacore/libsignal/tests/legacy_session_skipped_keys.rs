//! A session that skipped a message key must still round-trip through the v1
//! model. The skipped key is produced the only way it can be produced: an
//! out-of-order delivery over the real encrypt/decrypt APIs.
//! Async I/O uses `futures::executor::block_on` (no tokio in this crate).
#![cfg(feature = "legacy-session-interop")]

use async_trait::async_trait;
use std::collections::HashMap;
use wacore_libsignal::protocol::{
    CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, LegacySessionLocalContext, PreKeyBundle, PreKeyId,
    PreKeyRecord, PreKeyStore, ProtocolAddress, SessionRecord, SessionStore, SignalProtocolError,
    SignedPreKeyId, SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet,
    message_decrypt, message_encrypt, process_prekey_bundle,
};

type Result<T> = wacore_libsignal::protocol::error::Result<T>;

// ---- in-memory stores, kept local so this test file is self-contained ------

#[derive(Clone)]
struct InMemoryIdentityKeyStore {
    identity_key_pair: IdentityKeyPair,
    registration_id: u32,
    identities: HashMap<ProtocolAddress, IdentityKey>,
}

#[async_trait]
impl IdentityKeyStore for InMemoryIdentityKeyStore {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair> {
        Ok(self.identity_key_pair.clone())
    }
    async fn get_local_registration_id(&self) -> Result<u32> {
        Ok(self.registration_id)
    }
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Result<IdentityChange> {
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
    ) -> Result<bool> {
        Ok(true)
    }
    async fn get_identity(&self, address: &ProtocolAddress) -> Result<Option<IdentityKey>> {
        Ok(self.identities.get(address).cloned())
    }
}

#[derive(Default, Clone)]
struct InMemoryPreKeyStore(HashMap<PreKeyId, PreKeyRecord>);

#[async_trait]
impl PreKeyStore for InMemoryPreKeyStore {
    async fn get_pre_key(&self, id: PreKeyId) -> Result<PreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidPreKeyId)
    }
    async fn save_pre_key(&mut self, id: PreKeyId, record: &PreKeyRecord) -> Result<()> {
        self.0.insert(id, record.clone());
        Ok(())
    }
    async fn remove_pre_key(&mut self, id: PreKeyId) -> Result<()> {
        self.0.remove(&id);
        Ok(())
    }
}

#[derive(Default, Clone)]
struct InMemorySignedPreKeyStore(HashMap<SignedPreKeyId, SignedPreKeyRecord>);

#[async_trait]
impl SignedPreKeyStore for InMemorySignedPreKeyStore {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> Result<SignedPreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidSignedPreKeyId)
    }
    async fn save_signed_pre_key(
        &mut self,
        id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> Result<()> {
        self.0.insert(id, record.clone());
        Ok(())
    }
}

#[derive(Default, Clone)]
struct InMemorySessionStore(HashMap<ProtocolAddress, SessionRecord>);

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn load_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>> {
        Ok(self.0.get(address).cloned())
    }
    async fn has_session(&self, address: &ProtocolAddress) -> Result<bool> {
        Ok(self.0.contains_key(address))
    }
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> Result<()> {
        self.0.insert(address.clone(), record);
        Ok(())
    }
}

// ---- peer fixture ----------------------------------------------------------

struct Peer {
    address: ProtocolAddress,
    identity_store: InMemoryIdentityKeyStore,
    prekey_store: InMemoryPreKeyStore,
    signed_prekey_store: InMemorySignedPreKeyStore,
    session_store: InMemorySessionStore,
    prekey_id: PreKeyId,
    prekey_pair: KeyPair,
    signed_prekey_id: SignedPreKeyId,
    signed_prekey_pair: KeyPair,
    signed_prekey_signature: Vec<u8>,
}

impl Peer {
    fn new(name: &str, device_id: u32) -> Self {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let identity_key_pair = IdentityKeyPair::generate(&mut rng);
        let registration_id = rand::random::<u32>() & 0x3FFF;

        let prekey_id: PreKeyId = 1u32.into();
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

        let mut prekey_store = InMemoryPreKeyStore::default();
        let mut signed_prekey_store = InMemorySignedPreKeyStore::default();
        futures::executor::block_on(async {
            prekey_store
                .save_pre_key(prekey_id, &prekey_record)
                .await
                .expect("save prekey");
            signed_prekey_store
                .save_signed_pre_key(signed_prekey_id, &signed_prekey_record)
                .await
                .expect("save signed prekey");
        });

        Self {
            address: ProtocolAddress::new(name, device_id.into()),
            identity_store: InMemoryIdentityKeyStore {
                identity_key_pair,
                registration_id,
                identities: HashMap::new(),
            },
            prekey_store,
            signed_prekey_store,
            session_store: InMemorySessionStore::default(),
            prekey_id,
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
            Some((self.prekey_id, self.prekey_pair.public_key)),
            self.signed_prekey_id,
            self.signed_prekey_pair.public_key,
            self.signed_prekey_signature.clone(),
            *self.identity_store.identity_key_pair.identity_key(),
        )
        .expect("valid bundle")
    }

    fn legacy_context(&self) -> LegacySessionLocalContext {
        LegacySessionLocalContext {
            identity_key: *self.identity_store.identity_key_pair.identity_key(),
            registration_id: self.identity_store.registration_id,
        }
    }
}

// ---- helpers ---------------------------------------------------------------

fn send(from: &mut Peer, to: &ProtocolAddress, plaintext: &[u8]) -> CiphertextMessage {
    futures::executor::block_on(message_encrypt(
        plaintext,
        to,
        &mut from.session_store,
        &mut from.identity_store,
    ))
    .expect("encrypt")
}

fn receive(
    to: &mut Peer,
    from: &ProtocolAddress,
    ct: &CiphertextMessage,
) -> std::result::Result<Vec<u8>, SignalProtocolError> {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(message_decrypt(
        ct,
        from,
        &mut to.session_store,
        &mut to.identity_store,
        &mut to.prekey_store,
        &to.signed_prekey_store,
        &mut rng,
        UsePQRatchet::No,
    ))
    .map(|decrypted| decrypted.plaintext)
}

fn establish(alice: &mut Peer, bob: &mut Peer) {
    let bundle = bob.bundle();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(process_prekey_bundle(
        &bob.address,
        &mut alice.session_store,
        &mut alice.identity_store,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    ))
    .expect("prekey bundle accepted");

    let ct = send(alice, &bob.address, b"hello bob");
    assert_eq!(
        receive(bob, &alice.address, &ct).expect("first pkmsg decrypts"),
        b"hello bob"
    );
}

/// Export `peer`'s session with `remote` to the v1 model and put the reimported
/// record back in its place, so everything after this runs on state that made
/// the full trip.
fn cycle_through_v1(peer: &mut Peer, remote: &ProtocolAddress) {
    let context = peer.legacy_context();
    futures::executor::block_on(async {
        let record = peer
            .session_store
            .load_session(remote)
            .await
            .expect("load")
            .expect("session exists");
        let reimported = record
            .into_legacy_session_v1_operational()
            .expect("v1 projection")
            .into_session_record(context)
            .expect("v1 import");
        peer.session_store
            .store_session(remote, reimported)
            .await
            .expect("store");
    });
}

// ---- scenario --------------------------------------------------------------

/// The full cycle. Before the seed was persisted, the second projection failed
/// with `ChainNotRepresentable`: the skipped key had only its derived material
/// left, and the v1 model has nowhere to put that.
#[test]
fn a_skipped_key_survives_a_full_v1_projection_cycle() {
    let mut alice = Peer::new("alice", 1);
    let mut bob = Peer::new("bob", 1);
    establish(&mut alice, &mut bob);

    // Bob answers, so Alice's session leaves its pending-prekey state and gains
    // a receiver chain to skip a key on.
    let opener = send(&mut bob, &alice.address, b"hi alice");
    assert_eq!(
        receive(&mut alice, &bob.address, &opener).expect("decrypt"),
        b"hi alice"
    );
    cycle_through_v1(&mut alice, &bob.address);

    // Out-of-order delivery: the second message arrives first, so decrypting it
    // derives and retains the key for the first one.
    let skipped = send(&mut bob, &alice.address, b"m0");
    let delivered = send(&mut bob, &alice.address, b"m1");
    assert_eq!(
        receive(&mut alice, &bob.address, &delivered).expect("decrypt out of order"),
        b"m1"
    );

    cycle_through_v1(&mut alice, &bob.address);

    // The seed made the round trip only if it still derives the key the skipped
    // message was encrypted with.
    assert_eq!(
        receive(&mut alice, &bob.address, &skipped).expect("skipped message decrypts"),
        b"m0"
    );
}
