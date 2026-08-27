//! Proof that the X25519 agreement reaches the installed crypto provider.
//! Its own integration binary because `set_crypto_provider` writes a
//! process-wide global the rest of the suite must not observe.
//! Async I/O uses `futures::executor::block_on` (no tokio in this crate).

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};
use wacore_libsignal::crypto::{
    CryptoProviderError, RustCryptoProvider, SignalCryptoProvider, set_crypto_provider,
};
use wacore_libsignal::protocol::{
    CiphertextMessage, CurveError, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, PreKeyBundle, PreKeyId, PreKeyRecord, PreKeyStore,
    ProtocolAddress, PublicKey, SessionRecord, SessionStore, SignalProtocolError, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet, message_decrypt,
    message_encrypt, process_prekey_bundle,
};

// ---- the provider under test ------------------------------------------------

static AGREEMENTS: AtomicUsize = AtomicUsize::new(0);

/// Answers agreements with a secret that depends on the argument order, so the
/// two sides of a session derive different roots: the same shape of damage a
/// mismatched external implementation would cause.
fn substituted_agreement(private_key: &[u8; 32], their_public_key: &[u8; 32]) -> [u8; 32] {
    RustCryptoProvider.hmac_sha256(private_key, their_public_key)
}

/// Peer key that makes the provider below report a backend failure, standing in
/// for the moment an external module refuses the operation.
const UNAVAILABLE_PEER_KEY: [u8; 32] = [0xee; 32];

struct SubstitutedAgreementProvider;

impl SignalCryptoProvider for SubstitutedAgreementProvider {
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
        AGREEMENTS.fetch_add(1, Ordering::Relaxed);
        if their_public_key == &UNAVAILABLE_PEER_KEY {
            return Err(CryptoProviderError::BackendFailed);
        }
        Ok(substituted_agreement(private_key, their_public_key))
    }
}

/// The global takes one writer, and both tests share this binary when the
/// runner keeps them in a single process.
fn install_provider() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        set_crypto_provider(SubstitutedAgreementProvider).expect("provider installs first");
    });
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

// ---- scenarios --------------------------------------------------------------

/// The installed provider answers the agreement, and its answer is what the
/// key types hand back.
#[test]
fn agreement_is_routed_to_the_installed_provider() {
    install_provider();

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let alice = KeyPair::generate(&mut rng);
    let bob = KeyPair::generate(&mut rng);

    let before = AGREEMENTS.load(Ordering::Relaxed);
    let agreement = alice
        .calculate_agreement(&bob.public_key)
        .expect("agreement");

    let bob_public: [u8; 32] = bob
        .public_key
        .public_key_bytes()
        .try_into()
        .expect("djb public key");
    assert!(AGREEMENTS.load(Ordering::Relaxed) > before);
    assert_eq!(
        agreement,
        substituted_agreement(alice.private_key.serialize(), &bob_public)
    );
    assert_ne!(
        agreement,
        bob.calculate_agreement(&alice.public_key)
            .expect("agreement")
    );
}

/// A provider whose agreement is inconsistent between the two sides breaks the
/// session at the MAC, as a typed error rather than a panic.
#[test]
fn inconsistent_agreement_fails_the_session_with_a_typed_error() {
    install_provider();

    let mut alice = Peer::new("alice-provider", 1);
    let mut bob = Peer::new("bob-provider", 1);

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

    let ct = send(&mut alice, &bob.address, b"unreadable");
    let err = receive(&mut bob, &alice.address, &ct).expect_err("roots cannot match");
    assert!(
        matches!(
            err,
            SignalProtocolError::InvalidMessage(..) | SignalProtocolError::BadMac(..)
        ),
        "expected a typed decrypt failure, got {err:?}"
    );
}

/// A backend that refuses the operation surfaces as an error the caller can
/// match on, all the way up to the protocol error type, and never as bytes the
/// session would then build keys from.
#[test]
fn provider_backend_failure_surfaces_as_a_typed_error() {
    install_provider();

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let ours = KeyPair::generate(&mut rng);
    let unavailable =
        PublicKey::from_djb_public_key_bytes(&UNAVAILABLE_PEER_KEY).expect("peer key");

    let err = ours
        .calculate_agreement(&unavailable)
        .expect_err("the backend refused");
    assert!(
        matches!(
            err,
            CurveError::AgreementFailed(CryptoProviderError::BackendFailed)
        ),
        "expected a typed agreement failure, got {err:?}"
    );
    assert!(matches!(
        SignalProtocolError::from(err),
        SignalProtocolError::KeyAgreementFailed(CryptoProviderError::BackendFailed)
    ));
}
