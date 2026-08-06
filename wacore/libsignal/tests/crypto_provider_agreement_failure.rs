//! A backend refusal during a DH ratchet must reach the caller as itself.
//! Its own integration binary because `set_crypto_provider` writes a
//! process-wide global the rest of the suite must not observe.
//! Async I/O uses `futures::executor::block_on` (no tokio in this crate).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore_libsignal::crypto::{
    CryptoProviderError, RustCryptoProvider, SignalCryptoProvider, set_crypto_provider,
};
use wacore_libsignal::protocol::{
    CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, PreKeyBundle, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet, message_decrypt,
    message_encrypt, process_prekey_bundle,
};

// ---- the provider under test ------------------------------------------------

/// Trips the agreement into a backend failure. Off while the session is built,
/// so everything up to that point is the real primitive.
static REFUSING: AtomicBool = AtomicBool::new(false);

struct FusedAgreementProvider;

impl SignalCryptoProvider for FusedAgreementProvider {
    fn aes_256_cbc_encrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        RustCryptoProvider.aes_256_cbc_encrypt(key, iv, plaintext, out)
    }

    fn aes_256_cbc_decrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        RustCryptoProvider.aes_256_cbc_decrypt(key, iv, ciphertext, out)
    }

    fn aes_256_gcm_encrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        RustCryptoProvider.aes_256_gcm_encrypt(key, nonce, aad, plaintext, out)
    }

    fn aes_256_gcm_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext_with_tag: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        RustCryptoProvider.aes_256_gcm_decrypt(key, nonce, aad, ciphertext_with_tag, out)
    }

    fn hmac_sha256(&self, key: &[u8], input: &[u8]) -> [u8; 32] {
        RustCryptoProvider.hmac_sha256(key, input)
    }

    fn x25519_agreement(
        &self,
        private_key: &[u8; 32],
        their_public_key: &[u8; 32],
    ) -> Result<[u8; 32], CryptoProviderError> {
        if REFUSING.load(Ordering::SeqCst) {
            return Err(CryptoProviderError::BackendFailed);
        }
        RustCryptoProvider.x25519_agreement(private_key, their_public_key)
    }
}

// ---- in-memory stores (same fixtures the other test binaries keep local) -----

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
                .expect("store prekey");
            signed_prekey_store
                .save_signed_pre_key(signed_prekey_id, &signed_prekey_record)
                .await
                .expect("store signed prekey");
        });

        Self {
            address: ProtocolAddress::new(name, device_id.into()),
            identity_store: InMemoryIdentityKeyStore {
                identity_key_pair,
                registration_id: 1234,
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
            self.address.device_id(),
            Some((self.prekey_id, self.prekey_pair.public_key)),
            self.signed_prekey_id,
            self.signed_prekey_pair.public_key,
            self.signed_prekey_signature.clone(),
            *self.identity_store.identity_key_pair.identity_key(),
        )
        .expect("valid bundle")
    }
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

// ---- scenario ---------------------------------------------------------------

/// One test per binary: the fuse is process-wide state, and a second test
/// running beside it would see the flipped one.
///
/// A message that carries a fresh ratchet key makes the receiver agree keys
/// mid-decrypt. With the backend refusing, that has to arrive as
/// `KeyAgreementFailed` and not as the "no session could read it" verdict the
/// candidate search otherwise reports, which would have the caller ask the peer
/// to resend a message that was never corrupt.
#[test]
fn a_refused_agreement_survives_the_decrypt_candidate_search() {
    set_crypto_provider(FusedAgreementProvider).expect("provider installs first");

    let mut alice = Peer::new("alice-fused", 1);
    let mut bob = Peer::new("bob-fused", 1);

    let bundle = bob.bundle();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    futures::executor::block_on(async {
        process_prekey_bundle(
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("bundle accepted");
    });

    // Ping-pong until both sides hold a live session, all on the real primitive.
    let opening = send(&mut alice, &bob.address, b"hello bob");
    assert_eq!(
        receive(&mut bob, &alice.address, &opening).expect("pkmsg decrypts"),
        b"hello bob"
    );
    let reply = send(&mut bob, &alice.address, b"hello alice");
    assert_eq!(
        receive(&mut alice, &bob.address, &reply).expect("reply decrypts"),
        b"hello alice"
    );
    let follow_up = send(&mut alice, &bob.address, b"still here");
    assert_eq!(
        receive(&mut bob, &alice.address, &follow_up).expect("follow-up decrypts"),
        b"still here"
    );

    // Bob's next send opens a new sending ratchet, so Alice has to agree keys
    // to read it.
    let ratcheted = send(&mut bob, &alice.address, b"new ratchet");

    REFUSING.store(true, Ordering::SeqCst);
    let err = receive(&mut alice, &bob.address, &ratcheted).expect_err("the backend refused");
    REFUSING.store(false, Ordering::SeqCst);

    assert!(
        matches!(
            err,
            SignalProtocolError::KeyAgreementFailed(CryptoProviderError::BackendFailed)
        ),
        "expected the backend failure to survive, got {err:?}"
    );

    // With the backend back, the same message reads normally: the failure was
    // never a property of the message.
    assert_eq!(
        receive(&mut alice, &bob.address, &ratcheted).expect("decrypts once the backend returns"),
        b"new ratchet"
    );

    // Same refusal, reached through the archived-session half of the search:
    // with no current state the candidate loop is the only path, and the
    // session it borrowed has to be back in place afterwards.
    let after = send(&mut alice, &bob.address, b"after recovery");
    assert_eq!(
        receive(&mut bob, &alice.address, &after).expect("decrypts"),
        b"after recovery"
    );
    let for_the_archive = send(&mut bob, &alice.address, b"read me from the archive");

    {
        let record = alice
            .session_store
            .0
            .get_mut(&bob.address)
            .expect("alice has a session");
        record.archive_current_state().expect("archive");
        assert!(record.session_state().is_none());
        assert_eq!(record.previous_session_count(), 1);
    }

    REFUSING.store(true, Ordering::SeqCst);
    let err = receive(&mut alice, &bob.address, &for_the_archive).expect_err("the backend refused");
    REFUSING.store(false, Ordering::SeqCst);

    assert!(
        matches!(
            err,
            SignalProtocolError::KeyAgreementFailed(CryptoProviderError::BackendFailed)
        ),
        "expected the backend failure to survive the archived-session search, got {err:?}"
    );
    assert_eq!(
        alice
            .session_store
            .0
            .get(&bob.address)
            .expect("record")
            .previous_session_count(),
        1,
        "the archived session must still be there after the refusal"
    );
    assert_eq!(
        receive(&mut alice, &bob.address, &for_the_archive)
            .expect("the archived session still reads it"),
        b"read me from the archive"
    );

    // A refusal in one candidate must not end the search: the backend is asked
    // with that candidate's own ratchet key, and a sibling with an open
    // receiver chain reads the message without agreeing anything.
    let first = send(&mut bob, &alice.address, b"same chain, first");
    let second = send(&mut bob, &alice.address, b"same chain, second");
    assert_eq!(
        receive(&mut alice, &bob.address, &first).expect("opens the chain"),
        b"same chain, first"
    );

    // Rebuilding archives that session and installs a fresh one, which knows
    // nothing about Bob's ratchet key and has to agree keys to try.
    let fresh_bundle = bob.bundle();
    futures::executor::block_on(async {
        process_prekey_bundle(
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &fresh_bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("bundle accepted");
    });

    REFUSING.store(true, Ordering::SeqCst);
    let plaintext = receive(&mut alice, &bob.address, &second);
    REFUSING.store(false, Ordering::SeqCst);
    assert_eq!(
        plaintext.expect("the archived chain needs no agreement"),
        b"same chain, second"
    );
}
