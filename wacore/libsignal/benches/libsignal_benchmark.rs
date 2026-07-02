use std::collections::HashMap;

/// SipHash with fixed keys: the default RandomState seeds per process, so
/// bucket layout (and thus cache behavior) differed between benchmark runs.
type DetState = std::hash::BuildHasherDefault<std::hash::DefaultHasher>;
type DetHashMap<K, V> = HashMap<K, V, DetState>;

/// Deterministic per-call-site RNG: entropy-seeded keys made every run measure
/// different instruction counts (vartime signature paths depend on scalar
/// bits), which CodSpeed reads as noise. A counter keeps distinct call sites
/// on distinct streams so parties never share key material.
fn bench_rng() -> rand::rngs::StdRng {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CTR: AtomicU32 = AtomicU32::new(0);
    <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(
        0xB3AC_0000 + u64::from(CTR.fetch_add(1, Ordering::Relaxed)),
    )
}

use async_trait::async_trait;
use divan::black_box;

fn main() {
    divan::main();
}
use wacore_libsignal::protocol::{
    ChainKey, CiphertextMessage, Direction, GenericSignedPreKey, IdentityChange, IdentityKey,
    IdentityKeyPair, IdentityKeyStore, KeyPair, MessageKeyGenerator, PreKeyBundle, PreKeyId,
    PreKeyRecord, PreKeyStore, ProtocolAddress, RootKey, SenderKeyDistributionMessage,
    SenderKeyRecord, SenderKeyStore, SessionRecord, SessionState, SessionStore, SignedPreKeyId,
    SignedPreKeyRecord, SignedPreKeyStore, Timestamp, UsePQRatchet, consts,
    create_sender_key_distribution_message, group_decrypt, group_encrypt, message_decrypt,
    message_encrypt, process_prekey_bundle, process_sender_key_distribution_message,
};
use wacore_libsignal::store::sender_key_name::SenderKeyName;

struct InMemoryIdentityKeyStore {
    identity_key_pair: IdentityKeyPair,
    registration_id: u32,
    identities: DetHashMap<ProtocolAddress, IdentityKey>,
}

impl InMemoryIdentityKeyStore {
    fn new(identity_key_pair: IdentityKeyPair, registration_id: u32) -> Self {
        Self {
            identity_key_pair,
            registration_id,
            identities: DetHashMap::default(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
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
            .is_some_and(|existing| existing != identity);
        self.identities.insert(address.clone(), *identity);
        Ok(IdentityChange::from_changed(changed))
    }

    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
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

struct InMemoryPreKeyStore {
    prekeys: DetHashMap<PreKeyId, PreKeyRecord>,
}

impl InMemoryPreKeyStore {
    fn new() -> Self {
        Self {
            prekeys: DetHashMap::default(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PreKeyStore for InMemoryPreKeyStore {
    async fn get_pre_key(
        &self,
        prekey_id: PreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<PreKeyRecord> {
        self.prekeys
            .get(&prekey_id)
            .cloned()
            .ok_or(wacore_libsignal::protocol::SignalProtocolError::InvalidPreKeyId)
    }

    async fn save_pre_key(
        &mut self,
        prekey_id: PreKeyId,
        record: &PreKeyRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.prekeys.insert(prekey_id, record.clone());
        Ok(())
    }

    async fn remove_pre_key(
        &mut self,
        prekey_id: PreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.prekeys.remove(&prekey_id);
        Ok(())
    }
}

struct InMemorySignedPreKeyStore {
    signed_prekeys: DetHashMap<SignedPreKeyId, SignedPreKeyRecord>,
}

impl InMemorySignedPreKeyStore {
    fn new() -> Self {
        Self {
            signed_prekeys: DetHashMap::default(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignedPreKeyStore for InMemorySignedPreKeyStore {
    async fn get_signed_pre_key(
        &self,
        signed_prekey_id: SignedPreKeyId,
    ) -> wacore_libsignal::protocol::error::Result<SignedPreKeyRecord> {
        self.signed_prekeys
            .get(&signed_prekey_id)
            .cloned()
            .ok_or(wacore_libsignal::protocol::SignalProtocolError::InvalidSignedPreKeyId)
    }

    async fn save_signed_pre_key(
        &mut self,
        signed_prekey_id: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.signed_prekeys.insert(signed_prekey_id, record.clone());
        Ok(())
    }
}

struct InMemorySessionStore {
    sessions: DetHashMap<ProtocolAddress, SessionRecord>,
}

impl InMemorySessionStore {
    fn new() -> Self {
        Self {
            sessions: DetHashMap::default(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SessionStore for InMemorySessionStore {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> wacore_libsignal::protocol::error::Result<Option<SessionRecord>> {
        Ok(self.sessions.get(address).cloned())
    }

    async fn has_session(
        &self,
        address: &ProtocolAddress,
    ) -> wacore_libsignal::protocol::error::Result<bool> {
        Ok(self.sessions.contains_key(address))
    }

    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.sessions.insert(address.clone(), record);
        Ok(())
    }
}

struct InMemorySenderKeyStore {
    sender_keys: DetHashMap<SenderKeyName, SenderKeyRecord>,
}

impl InMemorySenderKeyStore {
    fn new() -> Self {
        Self {
            sender_keys: DetHashMap::default(),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SenderKeyStore for InMemorySenderKeyStore {
    async fn store_sender_key(
        &mut self,
        sender_key_name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> wacore_libsignal::protocol::error::Result<()> {
        self.sender_keys.insert(sender_key_name.clone(), record);
        Ok(())
    }

    async fn load_sender_key(
        &self,
        sender_key_name: &SenderKeyName,
    ) -> wacore_libsignal::protocol::error::Result<Option<SenderKeyRecord>> {
        Ok(self.sender_keys.get(sender_key_name).cloned())
    }
}

struct User {
    address: ProtocolAddress,
    identity_store: InMemoryIdentityKeyStore,
    prekey_store: InMemoryPreKeyStore,
    signed_prekey_store: InMemorySignedPreKeyStore,
    session_store: InMemorySessionStore,
    sender_key_store: InMemorySenderKeyStore,
    prekey_id: PreKeyId,
    signed_prekey_id: SignedPreKeyId,
    prekey_pair: KeyPair,
    signed_prekey_pair: KeyPair,
    signed_prekey_signature: Vec<u8>,
}

impl User {
    fn new(name: &str, device_id: u32) -> Self {
        let mut rng = bench_rng();

        let identity_key_pair = IdentityKeyPair::generate(&mut rng);
        // Same deterministic stream: the id is varint-encoded into prekey
        // bundles, so an entropy draw here still shifted payload sizes.
        let registration_id = {
            use rand::RngExt as _;
            rng.random::<u32>() & 0x3FFF
        };

        let prekey_id: PreKeyId = 1.into();
        let prekey_pair = KeyPair::generate(&mut rng);
        let prekey_record = PreKeyRecord::new(prekey_id, &prekey_pair);

        let signed_prekey_id: SignedPreKeyId = 1.into();
        let signed_prekey_pair = KeyPair::generate(&mut rng);
        let signed_prekey_signature = identity_key_pair
            .private_key()
            .calculate_signature(&signed_prekey_pair.public_key.serialize(), &mut rng)
            .expect("signature");
        let signed_prekey_record = SignedPreKeyRecord::new(
            signed_prekey_id,
            Timestamp::from_epoch_millis(0),
            &signed_prekey_pair,
            &signed_prekey_signature,
        );

        let identity_store = InMemoryIdentityKeyStore::new(identity_key_pair, registration_id);
        let mut prekey_store = InMemoryPreKeyStore::new();
        let mut signed_prekey_store = InMemorySignedPreKeyStore::new();
        let session_store = InMemorySessionStore::new();
        let sender_key_store = InMemorySenderKeyStore::new();

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

        let address = ProtocolAddress::new(name.to_string(), device_id.into());

        Self {
            address,
            identity_store,
            prekey_store,
            signed_prekey_store,
            session_store,
            sender_key_store,
            prekey_id,
            signed_prekey_id,
            prekey_pair,
            signed_prekey_pair,
            signed_prekey_signature: signed_prekey_signature.to_vec(),
        }
    }

    fn get_prekey_bundle(&self) -> PreKeyBundle {
        PreKeyBundle::new(
            self.identity_store.registration_id,
            1.into(),
            Some((self.prekey_id, self.prekey_pair.public_key)),
            self.signed_prekey_id,
            self.signed_prekey_pair.public_key,
            self.signed_prekey_signature.clone(),
            *self.identity_store.identity_key_pair.identity_key(),
        )
        .expect("valid bundle")
    }
}

fn setup_dm_users() -> (User, User) {
    let alice = User::new("alice", 1);
    let bob = User::new("bob", 1);
    (alice, bob)
}

fn setup_dm_session() -> (User, User) {
    let (mut alice, bob) = setup_dm_users();

    let bob_bundle = bob.get_prekey_bundle();
    let mut rng = bench_rng();

    futures::executor::block_on(async {
        process_prekey_bundle(
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &bob_bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("session established");
    });

    (alice, bob)
}

fn setup_dm_with_first_message() -> (User, User, Vec<u8>) {
    let (mut alice, bob) = setup_dm_session();

    let plaintext = b"Hello Bob! This is Alice.";
    let ciphertext = futures::executor::block_on(async {
        message_encrypt(
            plaintext,
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encryption")
    });

    (alice, bob, ciphertext.serialize().to_vec())
}

fn setup_established_dm_session() -> (User, User) {
    let (mut alice, mut bob) = setup_dm_session();

    let plaintext = b"Hello Bob!";
    futures::executor::block_on(async {
        let ct = message_encrypt(
            plaintext,
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encryption");

        let ct_msg = CiphertextMessage::PreKeySignalMessage(
            wacore_libsignal::protocol::PreKeySignalMessage::try_from(ct.serialize()).unwrap(),
        );
        let mut rng = bench_rng();
        message_decrypt(
            &ct_msg,
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
            &mut bob.prekey_store,
            &bob.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("decryption");
    });

    (alice, bob)
}

fn setup_group_sender() -> (User, SenderKeyName) {
    let alice = User::new("alice", 1);
    let group_id = "group123@g.us".to_string();
    let sender_key_name = SenderKeyName::new(group_id, alice.address.name().to_string());
    (alice, sender_key_name)
}

fn setup_group_with_distribution() -> (User, User, SenderKeyName) {
    let (mut alice, sender_key_name) = setup_group_sender();
    let mut bob = User::new("bob", 1);

    futures::executor::block_on(async {
        let mut rng = bench_rng();
        let skdm = create_sender_key_distribution_message(
            &sender_key_name,
            &mut alice.sender_key_store,
            &mut rng,
        )
        .await
        .expect("skdm");

        let bob_sender_key_name = SenderKeyName::new(
            sender_key_name.group_id().to_string(),
            alice.address.name().to_string(),
        );
        process_sender_key_distribution_message(
            &bob_sender_key_name,
            &skdm,
            &mut bob.sender_key_store,
        )
        .await
        .expect("process skdm");
    });

    (alice, bob, sender_key_name)
}

#[divan::bench]
fn bench_dm_session_establishment(bencher: divan::Bencher) {
    bencher.with_inputs(setup_dm_users).bench_refs(|data| {
        let (alice, bob) = data;
        let bob_bundle = bob.get_prekey_bundle();
        let mut rng = bench_rng();

        futures::executor::block_on(async {
            process_prekey_bundle(
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
                &bob_bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("session");
        });

        black_box(alice);
    });
}

#[divan::bench]
fn bench_dm_encrypt_first_message(bencher: divan::Bencher) {
    bencher.with_inputs(setup_dm_session).bench_refs(|data| {
        let (alice, bob) = data;
        let plaintext = b"Hello Bob! This is the first message.";

        let ciphertext = futures::executor::block_on(async {
            message_encrypt(
                plaintext,
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
            )
            .await
            .expect("encryption")
        });

        black_box(ciphertext);
    });
}

#[divan::bench]
fn bench_dm_decrypt_first_message(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_dm_with_first_message)
        .bench_refs(|data| {
            let (alice, bob, ciphertext_bytes) = data;
            let mut rng = bench_rng();

            let plaintext = futures::executor::block_on(async {
                let ciphertext = CiphertextMessage::PreKeySignalMessage(
                    wacore_libsignal::protocol::PreKeySignalMessage::try_from(
                        ciphertext_bytes.as_slice(),
                    )
                    .unwrap(),
                );
                message_decrypt(
                    &ciphertext,
                    &alice.address,
                    &mut bob.session_store,
                    &mut bob.identity_store,
                    &mut bob.prekey_store,
                    &bob.signed_prekey_store,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decryption")
            });

            black_box(plaintext);
        });
}

#[divan::bench]
fn bench_dm_encrypt_subsequent_message(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_established_dm_session)
        .bench_refs(|data| {
            let (alice, bob) = data;
            let plaintext = b"This is a follow-up message after session is established.";

            let ciphertext = futures::executor::block_on(async {
                message_encrypt(
                    plaintext,
                    &bob.address,
                    &mut alice.session_store,
                    &mut alice.identity_store,
                )
                .await
                .expect("encryption")
            });

            black_box(ciphertext);
        });
}

// Establish a DM session and advance it to where Alice sends plain
// SignalMessages on an existing Bob receiver chain, then return the next
// strictly in-order ciphertext. Decrypting it exercises the steady-state path
// (existing chain, counter == chain index) — the common case for an active
// chat, distinct from the first-message (PreKeySignalMessage) path.
fn setup_dm_with_inorder_subsequent_message() -> (User, User, Vec<u8>) {
    let (mut alice, mut bob) = setup_established_dm_session();

    futures::executor::block_on(async {
        let mut rng = bench_rng();

        // Bob replies and Alice decrypts it, clearing Alice's pending prekey so
        // her subsequent messages are plain SignalMessages.
        let reply = message_encrypt(
            b"ack",
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
        )
        .await
        .expect("bob reply");
        let reply_bytes = reply.serialize().to_vec();
        let reply_msg = CiphertextMessage::SignalMessage(
            wacore_libsignal::protocol::SignalMessage::try_from(reply_bytes.as_slice()).unwrap(),
        );
        message_decrypt(
            &reply_msg,
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &mut alice.prekey_store,
            &alice.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("alice decrypts reply");

        // Alice sends a first SignalMessage; Bob decrypts it so his receiver
        // chain for Alice's current ephemeral exists and is advanced. The
        // benchmarked message below is then strictly in-order on that chain.
        let m1 = message_encrypt(
            b"first subsequent",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("alice m1");
        let m1_bytes = m1.serialize().to_vec();
        let m1_msg = CiphertextMessage::SignalMessage(
            wacore_libsignal::protocol::SignalMessage::try_from(m1_bytes.as_slice()).unwrap(),
        );
        message_decrypt(
            &m1_msg,
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
            &mut bob.prekey_store,
            &bob.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("bob decrypts m1");

        // The in-order message to benchmark (same sending chain as m1, next counter).
        let m2 = message_encrypt(
            b"in-order subsequent message",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("alice m2");

        (alice, bob, m2.serialize().to_vec())
    })
}

#[divan::bench]
fn bench_dm_decrypt_subsequent_message(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_dm_with_inorder_subsequent_message)
        .bench_refs(|data| {
            let (alice, bob, ciphertext_bytes) = data;
            let mut rng = bench_rng();

            let plaintext = futures::executor::block_on(async {
                let ciphertext = CiphertextMessage::SignalMessage(
                    wacore_libsignal::protocol::SignalMessage::try_from(
                        ciphertext_bytes.as_slice(),
                    )
                    .unwrap(),
                );
                message_decrypt(
                    &ciphertext,
                    &alice.address,
                    &mut bob.session_store,
                    &mut bob.identity_store,
                    &mut bob.prekey_store,
                    &bob.signed_prekey_store,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decryption")
            });

            black_box(plaintext);
        });
}

#[divan::bench]
fn bench_group_create_distribution_message(bencher: divan::Bencher) {
    bencher.with_inputs(setup_group_sender).bench_refs(|data| {
        let (alice, sender_key_name) = data;
        let mut rng = bench_rng();

        let skdm = futures::executor::block_on(async {
            create_sender_key_distribution_message(
                sender_key_name,
                &mut alice.sender_key_store,
                &mut rng,
            )
            .await
            .expect("skdm")
        });

        black_box(skdm);
    });
}

#[divan::bench]
fn bench_group_encrypt_message(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_with_distribution)
        .bench_refs(|data| {
            let (alice, _bob, sender_key_name) = data;
            let plaintext = b"Hello group! This is a group message from Alice.";
            let mut rng = bench_rng();

            let ciphertext = futures::executor::block_on(async {
                group_encrypt(
                    &mut alice.sender_key_store,
                    sender_key_name,
                    plaintext,
                    &mut rng,
                )
                .await
                .expect("group encrypt")
            });

            black_box(ciphertext);
        });
}

fn setup_group_with_encrypted_message() -> (User, User, SenderKeyName, Vec<u8>) {
    let (mut alice, bob, sender_key_name) = setup_group_with_distribution();

    let ciphertext = futures::executor::block_on(async {
        let mut rng = bench_rng();
        let skm = group_encrypt(
            &mut alice.sender_key_store,
            &sender_key_name,
            b"Group message content",
            &mut rng,
        )
        .await
        .expect("group encrypt");
        skm.serialized().to_vec()
    });

    (alice, bob, sender_key_name, ciphertext)
}

#[divan::bench]
fn bench_group_decrypt_message(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_with_encrypted_message)
        .bench_refs(|data| {
            let (alice, bob, sender_key_name, ciphertext) = data;

            let bob_sender_key_name = SenderKeyName::new(
                sender_key_name.group_id().to_string(),
                alice.address.name().to_string(),
            );

            let plaintext = futures::executor::block_on(async {
                group_decrypt(ciphertext, &mut bob.sender_key_store, &bob_sender_key_name)
                    .await
                    .expect("group decrypt")
            });

            black_box(plaintext);
        });
}

fn setup_conversation_data() -> (User, User) {
    setup_dm_users()
}

#[divan::bench]
fn bench_full_dm_conversation(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_conversation_data)
        .bench_refs(|data| {
            let (alice, bob) = data;
            let mut rng = bench_rng();

            futures::executor::block_on(async {
                let bob_bundle = bob.get_prekey_bundle();
                process_prekey_bundle(
                    &bob.address,
                    &mut alice.session_store,
                    &mut alice.identity_store,
                    &bob_bundle,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("session");

                let msg1 = message_encrypt(
                    b"Hello Bob!",
                    &bob.address,
                    &mut alice.session_store,
                    &mut alice.identity_store,
                )
                .await
                .expect("encrypt1");

                let ct1 = CiphertextMessage::PreKeySignalMessage(
                    wacore_libsignal::protocol::PreKeySignalMessage::try_from(msg1.serialize())
                        .unwrap(),
                );
                let _ = message_decrypt(
                    &ct1,
                    &alice.address,
                    &mut bob.session_store,
                    &mut bob.identity_store,
                    &mut bob.prekey_store,
                    &bob.signed_prekey_store,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decrypt1");

                let msg2 = message_encrypt(
                    b"Hi Alice!",
                    &alice.address,
                    &mut bob.session_store,
                    &mut bob.identity_store,
                )
                .await
                .expect("encrypt2");

                let ct2 = CiphertextMessage::SignalMessage(
                    wacore_libsignal::protocol::SignalMessage::try_from(msg2.serialize()).unwrap(),
                );
                let _ = message_decrypt(
                    &ct2,
                    &bob.address,
                    &mut alice.session_store,
                    &mut alice.identity_store,
                    &mut alice.prekey_store,
                    &alice.signed_prekey_store,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decrypt2");

                let msg3 = message_encrypt(
                    b"How are you?",
                    &bob.address,
                    &mut alice.session_store,
                    &mut alice.identity_store,
                )
                .await
                .expect("encrypt3");

                let ct3 = CiphertextMessage::SignalMessage(
                    wacore_libsignal::protocol::SignalMessage::try_from(msg3.serialize()).unwrap(),
                );
                let _ = message_decrypt(
                    &ct3,
                    &alice.address,
                    &mut bob.session_store,
                    &mut bob.identity_store,
                    &mut bob.prekey_store,
                    &bob.signed_prekey_store,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decrypt3");
            });

            black_box((alice, bob));
        });
}

// Signature-specific benchmarks to measure the XEdDSA optimization
fn setup_keypair_with_message() -> (KeyPair, [u8; 64]) {
    let mut rng = bench_rng();
    let keypair = KeyPair::generate(&mut rng);
    let message = [0x42u8; 64]; // Fixed message for consistent benchmarking
    (keypair, message)
}

// Benchmark raw signature creation (the main target of the caching optimization).
// This measures signing with a pre-created key, which is the common case in real usage.
#[divan::bench]
fn bench_signature_creation(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_keypair_with_message)
        .bench_refs(|data| {
            let (keypair, message) = data;
            let mut rng = bench_rng();

            // Sign multiple times to amortize any setup overhead
            for _ in 0..10 {
                let signature = keypair
                    .calculate_signature(&message[..], &mut rng)
                    .expect("signature");
                black_box(signature);
            }
        });
}

// Benchmark signature verification
#[divan::bench]
fn bench_signature_verification(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_keypair_with_message)
        .bench_refs(|data| {
            let (keypair, message) = data;
            let mut rng = bench_rng();
            let signature = keypair
                .calculate_signature(&message[..], &mut rng)
                .expect("signature");

            // Verify multiple times
            for _ in 0..10 {
                let valid = keypair
                    .public_key
                    .verify_signature(&message[..], &signature);
                black_box(valid);
            }
        });
}

// Benchmark key generation (shows the added cost of caching)
#[divan::bench]
fn bench_key_generation() {
    let mut rng = bench_rng();
    for _ in 0..10 {
        let keypair = KeyPair::generate(&mut rng);
        black_box(keypair);
    }
}

/// Creates a session with multiple archived previous sessions.
/// This simulates a scenario where Alice has re-keyed multiple times.
fn setup_with_archived_sessions() -> (User, User, Vec<Vec<u8>>) {
    let mut alice = User::new("alice", 1);
    let mut bob = User::new("bob", 1);
    let mut rng = bench_rng();

    // Store ciphertexts encrypted with each session version
    let mut old_ciphertexts = Vec::new();

    futures::executor::block_on(async {
        // Create initial session and send first message
        let bob_bundle = bob.get_prekey_bundle();
        process_prekey_bundle(
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &bob_bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("session 1");

        // Send a message with this session (to be used for previous session decryption)
        let msg = message_encrypt(
            b"Message from session 1",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encrypt");
        old_ciphertexts.push(msg.serialize().to_vec());

        // Bob processes Alice's first message to establish his side
        let ct = CiphertextMessage::PreKeySignalMessage(
            wacore_libsignal::protocol::PreKeySignalMessage::try_from(
                old_ciphertexts[0].as_slice(),
            )
            .unwrap(),
        );
        message_decrypt(
            &ct,
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
            &mut bob.prekey_store,
            &bob.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("decrypt");

        // Now create multiple new sessions to archive the old one
        // Each new PreKey message from Alice will archive Bob's current session
        for i in 2..=10 {
            // Alice creates a fresh session (simulating re-keying)
            alice.session_store = InMemorySessionStore::new();

            // Generate new prekeys for Bob
            bob.prekey_id = (i as u32).into();
            bob.prekey_pair = KeyPair::generate(&mut rng);
            let prekey_record = PreKeyRecord::new(bob.prekey_id, &bob.prekey_pair);
            bob.prekey_store
                .save_pre_key(bob.prekey_id, &prekey_record)
                .await
                .unwrap();

            let bob_bundle = bob.get_prekey_bundle();
            process_prekey_bundle(
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
                &bob_bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("new session");

            // Send PreKey message to establish new session on Bob's side
            let msg = message_encrypt(
                format!("Message from session {}", i).as_bytes(),
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
            )
            .await
            .expect("encrypt");

            let ct = CiphertextMessage::PreKeySignalMessage(
                wacore_libsignal::protocol::PreKeySignalMessage::try_from(msg.serialize()).unwrap(),
            );
            message_decrypt(
                &ct,
                &alice.address,
                &mut bob.session_store,
                &mut bob.identity_store,
                &mut bob.prekey_store,
                &bob.signed_prekey_store,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("decrypt");

            // Store the message encrypted with this session
            let msg2 = message_encrypt(
                format!("Another message from session {}", i).as_bytes(),
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
            )
            .await
            .expect("encrypt");
            old_ciphertexts.push(msg2.serialize().to_vec());
        }
    });

    (alice, bob, old_ciphertexts)
}

// Benchmark decryption that requires searching through previous sessions.
// This tests the take/restore optimization for previous session iteration.
#[divan::bench]
fn bench_decrypt_with_previous_session(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_with_archived_sessions)
        .bench_refs(|data| {
            let (alice, bob, ciphertexts) = data;
            let mut rng = bench_rng();

            // Try to decrypt an old message (encrypted with a previous session)
            // This forces the decryption to iterate through previous sessions
            futures::executor::block_on(async {
                for ciphertext in ciphertexts.iter().take(5) {
                    // Try to parse as SignalMessage (non-PreKey)
                    if let Ok(signal_msg) =
                        wacore_libsignal::protocol::SignalMessage::try_from(ciphertext.as_slice())
                    {
                        let ct = CiphertextMessage::SignalMessage(signal_msg);
                        let result = message_decrypt(
                            &ct,
                            &alice.address,
                            &mut bob.session_store,
                            &mut bob.identity_store,
                            &mut bob.prekey_store,
                            &bob.signed_prekey_store,
                            &mut rng,
                            UsePQRatchet::No,
                        )
                        .await;
                        let _ = black_box(result);
                    }
                }
            });
        });
}

// Setup for out-of-order message benchmark.
// Creates a fully established session and prepares multiple SignalMessages.
fn setup_out_of_order_messages() -> (User, User, Vec<Vec<u8>>) {
    let (mut alice, mut bob) = setup_dm_session();
    let mut messages = Vec::new();

    futures::executor::block_on(async {
        let mut rng = bench_rng();

        // Alice sends initial PreKey message to Bob
        let msg = message_encrypt(
            b"Initial message",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encrypt");

        let ct = CiphertextMessage::PreKeySignalMessage(
            wacore_libsignal::protocol::PreKeySignalMessage::try_from(msg.serialize()).unwrap(),
        );
        message_decrypt(
            &ct,
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
            &mut bob.prekey_store,
            &bob.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("decrypt initial");

        // Bob replies to Alice - this completes the session establishment
        let reply = message_encrypt(
            b"Reply from Bob",
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
        )
        .await
        .expect("encrypt reply");

        let ct_reply = CiphertextMessage::SignalMessage(
            wacore_libsignal::protocol::SignalMessage::try_from(reply.serialize()).unwrap(),
        );
        message_decrypt(
            &ct_reply,
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &mut alice.prekey_store,
            &alice.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("decrypt reply");

        // Now Alice can send SignalMessages (not PreKey)
        for i in 0..20 {
            let msg = message_encrypt(
                format!("Message {}", i).as_bytes(),
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
            )
            .await
            .expect("encrypt");
            messages.push(msg.serialize().to_vec());
        }
    });

    (alice, bob, messages)
}

// Benchmark out-of-order message decryption.
// This tests the set_message_keys optimization (push vs insert(0)).
// Messages are decrypted in reverse order, causing maximum message key storage.
#[divan::bench]
fn bench_out_of_order_decryption(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_out_of_order_messages)
        .bench_refs(|data| {
            let (alice, bob, messages) = data;
            let mut rng = bench_rng();

            futures::executor::block_on(async {
                // Decrypt messages in reverse order (worst case for message key storage)
                for ciphertext in messages.iter().rev() {
                    let signal_msg =
                        wacore_libsignal::protocol::SignalMessage::try_from(ciphertext.as_slice())
                            .expect("parse");
                    let ct = CiphertextMessage::SignalMessage(signal_msg);
                    let result = message_decrypt(
                        &ct,
                        &alice.address,
                        &mut bob.session_store,
                        &mut bob.identity_store,
                        &mut bob.prekey_store,
                        &bob.signed_prekey_store,
                        &mut rng,
                        UsePQRatchet::No,
                    )
                    .await
                    .expect("decrypt");
                    black_box(result);
                }
            });
        });
}

/// Setup for promote_matching_session benchmark.
/// Creates a session record with many archived sessions, then creates a PreKey
/// message that should match and promote one of the previous sessions.
fn setup_promote_matching_session() -> (User, User, Vec<u8>) {
    let mut alice = User::new("alice", 1);
    let mut bob = User::new("bob", 1);
    let mut rng = bench_rng();

    let prekey_message = futures::executor::block_on(async {
        // Create initial session
        let bob_bundle = bob.get_prekey_bundle();
        process_prekey_bundle(
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
            &bob_bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("session");

        // Send initial message to establish Bob's session
        let msg = message_encrypt(
            b"First contact",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encrypt");

        let ct = CiphertextMessage::PreKeySignalMessage(
            wacore_libsignal::protocol::PreKeySignalMessage::try_from(msg.serialize()).unwrap(),
        );
        message_decrypt(
            &ct,
            &alice.address,
            &mut bob.session_store,
            &mut bob.identity_store,
            &mut bob.prekey_store,
            &bob.signed_prekey_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("decrypt");

        // Create many new sessions to push the original to previous_sessions
        for i in 2..=20 {
            // Generate new prekeys for Bob
            bob.prekey_id = (i as u32).into();
            bob.prekey_pair = KeyPair::generate(&mut rng);
            let prekey_record = PreKeyRecord::new(bob.prekey_id, &bob.prekey_pair);
            bob.prekey_store
                .save_pre_key(bob.prekey_id, &prekey_record)
                .await
                .unwrap();

            // Alice starts fresh session
            alice.session_store = InMemorySessionStore::new();
            let bob_bundle = bob.get_prekey_bundle();
            process_prekey_bundle(
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
                &bob_bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("new session");

            let msg = message_encrypt(
                format!("Session {}", i).as_bytes(),
                &bob.address,
                &mut alice.session_store,
                &mut alice.identity_store,
            )
            .await
            .expect("encrypt");

            let ct = CiphertextMessage::PreKeySignalMessage(
                wacore_libsignal::protocol::PreKeySignalMessage::try_from(msg.serialize()).unwrap(),
            );
            message_decrypt(
                &ct,
                &alice.address,
                &mut bob.session_store,
                &mut bob.identity_store,
                &mut bob.prekey_store,
                &bob.signed_prekey_store,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("decrypt");
        }

        // Now Alice sends another PreKey message with the CURRENT session
        // This should trigger promote_matching_session to find and promote it
        let final_msg = message_encrypt(
            b"Final message with current session",
            &bob.address,
            &mut alice.session_store,
            &mut alice.identity_store,
        )
        .await
        .expect("encrypt");

        final_msg.serialize().to_vec()
    });

    (alice, bob, prekey_message)
}

// Benchmark promote_matching_session during PreKey processing.
// This tests the find_matching_previous_session_index optimization.
#[divan::bench]
fn bench_promote_matching_session(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_promote_matching_session)
        .bench_refs(|data| {
            let (alice, bob, prekey_message) = data;
            let mut rng = bench_rng();

            futures::executor::block_on(async {
                // Process multiple PreKey messages to exercise promote_matching_session
                for _ in 0..5 {
                    let ct = CiphertextMessage::PreKeySignalMessage(
                        wacore_libsignal::protocol::PreKeySignalMessage::try_from(
                            prekey_message.as_slice(),
                        )
                        .unwrap(),
                    );
                    let result = message_decrypt(
                        &ct,
                        &alice.address,
                        &mut bob.session_store,
                        &mut bob.identity_store,
                        &mut bob.prekey_store,
                        &bob.signed_prekey_store,
                        &mut rng,
                        UsePQRatchet::No,
                    )
                    .await;
                    let _ = black_box(result);
                }
            });
        });
}

/// Helper function to create a test message key generator.
fn create_test_message_key_generator(counter: u32) -> MessageKeyGenerator {
    let mut seed = [0u8; 32];
    seed[0] = counter as u8;
    seed[1] = (counter >> 8) as u8;
    seed[2] = (counter >> 16) as u8;
    seed[3] = (counter >> 24) as u8;
    MessageKeyGenerator::new_from_seed(&seed, counter)
}

/// Helper function to create a minimal valid SessionState for testing.
fn create_test_session_state(
    version: u8,
    base_key: &wacore_libsignal::protocol::PublicKey,
) -> SessionState {
    let mut csprng = bench_rng();
    let identity_keypair = KeyPair::generate(&mut csprng);
    let their_identity = IdentityKey::new(identity_keypair.public_key);
    let our_identity = IdentityKey::new(KeyPair::generate(&mut csprng).public_key);
    let root_key = RootKey::new([0u8; 32]);

    let mut state = SessionState::new(version, &our_identity, &their_identity, &root_key, base_key);

    // Add a sender chain to make it usable
    let sender_keypair = KeyPair::generate(&mut csprng);
    let chain_key = ChainKey::new([1u8; 32], 0);
    state.set_sender_chain(&sender_keypair, &chain_key);

    state
}

/// Setup for message key eviction benchmark.
/// Creates a session with a receiver chain pre-filled near capacity.
fn setup_message_key_eviction() -> (SessionState, wacore_libsignal::protocol::PublicKey) {
    let mut csprng = bench_rng();
    let base_key = KeyPair::generate(&mut csprng).public_key;
    let mut state = create_test_session_state(3, &base_key);

    // Add a receiver chain
    let sender_key = KeyPair::generate(&mut csprng).public_key;
    let chain_key = ChainKey::new([2u8; 32], 0);
    state.add_receiver_chain(&sender_key, &chain_key);

    // Pre-fill the chain to MAX_MESSAGE_KEYS - 1 (one less than capacity)
    // This simulates a session that has been receiving out-of-order messages
    // and is about to hit the eviction threshold
    for counter in 0..(consts::MAX_MESSAGE_KEYS - 1) as u32 {
        let keys = create_test_message_key_generator(counter);
        state.set_message_keys(&sender_key, keys).unwrap();
    }

    (state, sender_key)
}

// Benchmark message key insertion with amortized eviction.
// This tests the set_message_keys optimization with PRUNE_THRESHOLD.
// We insert 200 keys beyond MAX_MESSAGE_KEYS to measure multiple eviction cycles.
#[divan::bench]
fn bench_message_key_eviction(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_message_key_eviction)
        .bench_refs(|data| {
            let (state, sender_key) = data;
            let start_counter = (consts::MAX_MESSAGE_KEYS - 1) as u32;

            // Insert 200 keys beyond capacity - this will trigger eviction cycles
            // With PRUNE_THRESHOLD=50, we expect ~4 eviction events
            for i in 0..200u32 {
                let counter = start_counter + i;
                let keys = create_test_message_key_generator(counter);
                state.set_message_keys(sender_key, keys).unwrap();
            }

            black_box(state);
        });
}

/// Worst-case group out-of-order decrypt: a receiver that fell behind by ~2000
/// messages buffers that many skipped sender-message-keys, and each late
/// message consumes one via `SenderKeyState::remove_sender_message_key`, an
/// O(n) linear scan over the backlog (gap-analysis data-structures-22). The
/// expensive backlog fill runs once per iteration in `with_inputs` (untimed);
/// only the worst-case decrypt — the one whose key sits at the tail of the
/// backlog, forcing a full scan — is measured.
///
/// This is the full out-of-order decrypt, not a scan-isolating microbenchmark:
/// the backlog-sized `SenderKeyRecord` clone in `load_sender_key` and the
/// signature check are the bulk of it, with the linear scan a smaller slice. Use
/// it as the out-of-order decrypt baseline; the clone is itself backlog-proportional.
fn setup_group_out_of_order_worst_case() -> (User, SenderKeyName, Vec<u8>) {
    // A fill just under MAX_MESSAGE_KEYS: eviction only starts past
    // MAX_MESSAGE_KEYS + MESSAGE_KEY_PRUNE_THRESHOLD, so the whole backlog
    // survives intact for the scan.
    const N: u32 = (consts::MAX_MESSAGE_KEYS - 1) as u32;

    let (mut alice, mut bob, sender_key_name) = setup_group_with_distribution();

    let bob_sender_key_name = SenderKeyName::new(
        sender_key_name.group_id().to_string(),
        alice.address.name().to_string(),
    );

    let worst_case_ct = futures::executor::block_on(async {
        let mut rng = bench_rng();
        let mut ciphertexts: Vec<Vec<u8>> = Vec::with_capacity((N + 1) as usize);
        for i in 0..=N {
            let skm = group_encrypt(
                &mut alice.sender_key_store,
                &sender_key_name,
                format!("group msg {i}").as_bytes(),
                &mut rng,
            )
            .await
            .expect("group encrypt");
            ciphertexts.push(skm.serialized().to_vec());
        }

        // Decrypting the latest message first ratchets the chain forward over
        // iterations 0..N-1, buffering N skipped keys in arrival order.
        group_decrypt(
            &ciphertexts[N as usize],
            &mut bob.sender_key_store,
            &bob_sender_key_name,
        )
        .await
        .expect("group decrypt newest");

        // Iteration N-1 sits at the tail of the buffer: its lookup scans every
        // entry, the worst case for the linear `position()`.
        ciphertexts[(N - 1) as usize].clone()
    });

    (bob, bob_sender_key_name, worst_case_ct)
}

#[divan::bench(sample_count = 30)]
fn bench_group_out_of_order_decrypt_worst_case(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_out_of_order_worst_case)
        .bench_refs(|(bob, sender_key_name, ciphertext)| {
            let plaintext = futures::executor::block_on(async {
                group_decrypt(
                    black_box(ciphertext.as_slice()),
                    &mut bob.sender_key_store,
                    sender_key_name,
                )
                .await
                .expect("group decrypt worst case")
            });
            black_box(plaintext);
        });
}

/// In-order group decrypt while a large skipped-key backlog is still buffered.
/// Once a receiver falls ~2000 messages behind and catches up, the backlog
/// persists; every subsequent in-order message ratchets the chain forward
/// without ever reading the backlog, yet `load_sender_key` still clones the
/// whole backlog-sized record. The backlog fill and the catch-up run in
/// `with_inputs` (untimed); only the in-order decrypt — whose sole backlog cost
/// is that load clone — is measured.
fn setup_group_in_order_decrypt_with_backlog() -> (User, SenderKeyName, Vec<u8>) {
    // Fill just under MAX_MESSAGE_KEYS so the whole backlog survives intact
    // (eviction only starts past MAX + MESSAGE_KEY_PRUNE_THRESHOLD), then send
    // one more message that bob decrypts in order on top of that backlog.
    const N: u32 = (consts::MAX_MESSAGE_KEYS - 1) as u32;

    let (mut alice, mut bob, sender_key_name) = setup_group_with_distribution();

    let bob_sender_key_name = SenderKeyName::new(
        sender_key_name.group_id().to_string(),
        alice.address.name().to_string(),
    );

    let in_order_ct = futures::executor::block_on(async {
        let mut rng = bench_rng();
        let mut ciphertexts: Vec<Vec<u8>> = Vec::with_capacity((N + 2) as usize);
        for i in 0..=(N + 1) {
            let skm = group_encrypt(
                &mut alice.sender_key_store,
                &sender_key_name,
                format!("group msg {i}").as_bytes(),
                &mut rng,
            )
            .await
            .expect("group encrypt");
            ciphertexts.push(skm.serialized().to_vec());
        }

        // Decrypting message N first ratchets bob's chain to N+1 and buffers the
        // N skipped keys (iterations 0..N-1) in the backlog.
        group_decrypt(
            &ciphertexts[N as usize],
            &mut bob.sender_key_store,
            &bob_sender_key_name,
        )
        .await
        .expect("group decrypt newest");

        // Message N+1 is exactly in order (jump == 0): it advances the chain and
        // never touches the buffered backlog.
        ciphertexts[(N + 1) as usize].clone()
    });

    (bob, bob_sender_key_name, in_order_ct)
}

#[divan::bench(sample_count = 30)]
fn bench_group_in_order_decrypt_with_backlog(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_group_in_order_decrypt_with_backlog)
        .bench_refs(|(bob, sender_key_name, ciphertext)| {
            let plaintext = futures::executor::block_on(async {
                group_decrypt(
                    black_box(ciphertext.as_slice()),
                    &mut bob.sender_key_store,
                    sender_key_name,
                )
                .await
                .expect("group decrypt in order with backlog")
            });
            black_box(plaintext);
        });
}

/// SKDM ingest: installing a sender's distribution message into a fresh
/// receiver store, the work done on the first group message from each new
/// sender and on every sender-key rotation. The SKDM build and the fresh empty
/// store are prepared in `with_inputs`; only the ingest is measured.
fn setup_skdm_ingest() -> (
    SenderKeyDistributionMessage,
    InMemorySenderKeyStore,
    SenderKeyName,
) {
    let (mut alice, sender_key_name) = setup_group_sender();
    let receiver_key_name = SenderKeyName::new(
        sender_key_name.group_id().to_string(),
        alice.address.name().to_string(),
    );
    let skdm = futures::executor::block_on(async {
        let mut rng = bench_rng();
        create_sender_key_distribution_message(
            &sender_key_name,
            &mut alice.sender_key_store,
            &mut rng,
        )
        .await
        .expect("skdm")
    });
    (skdm, InMemorySenderKeyStore::new(), receiver_key_name)
}

#[divan::bench]
fn bench_process_sender_key_distribution_message(bencher: divan::Bencher) {
    bencher.with_inputs(setup_skdm_ingest).bench_refs(
        |(skdm, receiver_store, receiver_key_name)| {
            futures::executor::block_on(async {
                process_sender_key_distribution_message(
                    receiver_key_name,
                    black_box(skdm),
                    receiver_store,
                )
                .await
                .expect("process skdm")
            });
            // Observe the store so the store_sender_key write is not elided.
            black_box(&*receiver_store);
        },
    );
}
