//! Cross-implementation fixtures use fictitious keys and no account data.
//! The legacy engine is an oracle only in this test target, never a dependency
//! of the independent engine.
use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};
use wacore_libsignal::protocol as old;
use wacore_signal_engine::{self as new, CryptoProvider as _, RandomSource, RustCrypto};

struct Random(StdRng);
impl Random {
    fn seeded() -> Self {
        Self(StdRng::seed_from_u64(1828))
    }
}
impl RandomSource for Random {
    fn fill(&mut self, bytes: &mut [u8]) -> new::Result<()> {
        self.0.fill(bytes);
        Ok(())
    }
}

fn old_key(key: &new::KeyPair) -> old::KeyPair {
    old::KeyPair::new(
        old::PublicKey::from_djb_public_key_bytes(&key.public).unwrap(),
        old::PrivateKey::deserialize(&key.private).unwrap(),
    )
}

#[test]
fn independent_signatures_cross_verify_with_legacy_signatures() {
    let mut random = Random::seeded();
    let key = new::Engine::new(&RustCrypto, &mut random)
        .generate_key_pair()
        .unwrap();
    let legacy = old_key(&key);
    for message in [b"".as_slice(), b"synthetic cross-implementation message"] {
        let signature = RustCrypto.sign(&key.private, message, &mut random).unwrap();
        assert!(legacy.public_key.verify_signature(message, &signature));
        let signature = legacy
            .private_key
            .calculate_signature(message, &mut random.0)
            .unwrap();
        RustCrypto.verify(&key.public, message, &signature).unwrap();
    }
}

#[test]
fn prekeys_cross_read_raw_and_historical_tagged_public_keys() {
    let mut random = Random::seeded();
    let key = new::Engine::new(&RustCrypto, &mut random)
        .generate_key_pair()
        .unwrap();
    let legacy = old::PreKeyRecord::new(73.into(), &old_key(&key));
    let raw = legacy.serialize().unwrap();
    let decoded = new::codec::decode_pre_key(&raw).unwrap();
    assert_eq!(decoded.id, 73);
    assert_eq!(decoded.key_pair, key);
    let encoded = new::codec::encode_pre_key(&decoded);
    let reloaded = old::PreKeyRecord::deserialize(&encoded).unwrap();
    assert_eq!(
        reloaded.key_pair().unwrap().public_key,
        old_key(&key).public_key
    );
    let mut proto = waproto::codec::pre_key_record_decode(&raw).unwrap();
    proto.public_key = Some(old_key(&key).public_key.serialize().to_vec());
    let historical = waproto::codec::pre_key_record_to_vec(&proto);
    let historical = new::codec::decode_pre_key(&historical).unwrap();
    assert_eq!(historical.key_pair, key);
    assert_eq!(new::codec::encode_pre_key(&historical), encoded);
}

#[test]
fn signed_prekey_timestamps_are_opaque_across_codecs() {
    use old::GenericSignedPreKey as _;
    let mut random = Random::seeded();
    let key = new::Engine::new(&RustCrypto, &mut random)
        .generate_key_pair()
        .unwrap();
    for timestamp in [1_700_000_000, 1_700_000_000_000, u64::MAX] {
        let value = new::SignedPreKeyRecord {
            id: 42,
            key_pair: key.clone(),
            signature: [7; 64],
            timestamp,
        };
        let bytes = new::codec::encode_signed_pre_key(&value);
        let legacy = old::SignedPreKeyRecord::deserialize(&bytes).unwrap();
        assert_eq!(legacy.get_storage().timestamp, Some(timestamp));
        let decoded = new::codec::decode_signed_pre_key(&legacy.serialize().unwrap()).unwrap();
        assert_eq!(decoded.timestamp, timestamp);
        assert_eq!(decoded.key_pair, key);
        assert_eq!(decoded.signature, [7; 64]);
    }
}

const INCARNATION: [u8; 16] = [0x19; 16];

fn pair() -> (
    new::SessionRecord,
    new::SessionRecord,
    new::KeyPair,
    new::KeyPair,
) {
    let mut random = Random(StdRng::seed_from_u64(0x1828_aaaa));
    let mut engine = new::Engine::new(&RustCrypto, &mut random);
    let alice = engine.generate_key_pair().unwrap();
    let bob = engine.generate_key_pair().unwrap();
    let signed = engine.generate_key_pair().unwrap();
    let base = engine.generate_key_pair().unwrap();
    let signature = RustCrypto
        .sign(
            &bob.private,
            &new::wire::tagged_key(&signed.public),
            engine.random,
        )
        .unwrap();
    let mut alice_state = engine
        .initialize_initiator(&new::InitiatorParameters {
            local_identity: alice.clone(),
            local_registration: 11,
            remote_identity: bob.public,
            remote_registration: 22,
            remote_signed_pre_key: signed.public,
            remote_signed_pre_key_id: 5,
            remote_signed_pre_key_signature: signature,
            remote_one_time_pre_key: None,
            base_key: base.clone(),
        })
        .unwrap();
    alice_state.pending_pre_key = None;
    let bob_state = engine
        .initialize_responder(&new::ResponderParameters {
            local_identity: bob.clone(),
            local_registration: 22,
            local_signed_pre_key: signed,
            local_one_time_pre_key: None,
            remote_identity: alice.public,
            remote_base_key: base.public,
            remote_registration: 11,
        })
        .unwrap();
    let mut a = new::SessionRecord::default();
    a.install(alice_state);
    let mut b = new::SessionRecord::default();
    b.install(bob_state);
    grant(&mut a);
    grant(&mut b);
    (a, b, alice, bob)
}

fn grant(record: &mut new::SessionRecord) {
    let index = record
        .current
        .as_ref()
        .unwrap()
        .sender
        .as_ref()
        .unwrap()
        .index;
    record.reservation.prepare(index, 64, INCARNATION).unwrap();
    record.reservation.mark_committed();
}

struct SessionStore(Option<old::SessionRecord>);
#[async_trait::async_trait]
impl old::SessionStore for SessionStore {
    async fn load_session(
        &self,
        _: &old::ProtocolAddress,
    ) -> old::error::Result<Option<old::SessionRecord>> {
        Ok(self.0.clone())
    }
    async fn has_session(&self, _: &old::ProtocolAddress) -> old::error::Result<bool> {
        Ok(self.0.is_some())
    }
    async fn store_session(
        &mut self,
        _: &old::ProtocolAddress,
        record: old::SessionRecord,
    ) -> old::error::Result<()> {
        self.0 = Some(record);
        Ok(())
    }
}
struct IdentityStore {
    local: old::IdentityKeyPair,
    remote: old::IdentityKey,
}
#[async_trait::async_trait]
impl old::IdentityKeyStore for IdentityStore {
    async fn get_identity_key_pair(&self) -> old::error::Result<old::IdentityKeyPair> {
        Ok(self.local.clone())
    }
    async fn get_local_registration_id(&self) -> old::error::Result<u32> {
        Ok(11)
    }
    async fn save_identity(
        &mut self,
        _: &old::ProtocolAddress,
        key: &old::IdentityKey,
    ) -> old::error::Result<old::IdentityChange> {
        assert_eq!(*key, self.remote);
        Ok(old::IdentityChange::NewOrUnchanged)
    }
    async fn is_trusted_identity(
        &self,
        _: &old::ProtocolAddress,
        key: &old::IdentityKey,
        _: old::Direction,
    ) -> old::error::Result<bool> {
        Ok(*key == self.remote)
    }
    async fn get_identity(
        &self,
        _: &old::ProtocolAddress,
    ) -> old::error::Result<Option<old::IdentityKey>> {
        Ok(Some(self.remote))
    }
}
fn identities(local: &new::KeyPair, remote: &new::KeyPair) -> IdentityStore {
    IdentityStore {
        local: old_key(local).into(),
        remote: old_key(remote).public_key.into(),
    }
}
fn legacy(record: &new::SessionRecord) -> old::SessionRecord {
    old::SessionRecord::deserialize_for_store(
        &new::codec::encode_session_record(record),
        &INCARNATION,
    )
    .unwrap()
}

#[test]
fn pairwise_ciphertext_and_session_storage_alternate_between_engines() {
    let (mut a, mut b, alice, bob) = pair();
    let address = old::ProtocolAddress::new("synthetic-peer", 1.into());
    let mut random = Random::seeded();
    for turn in 0..12 {
        let plaintext = format!("synthetic message {turn}");
        if turn % 2 == 0 {
            let mut store = SessionStore(Some(legacy(&a)));
            let wire = futures::executor::block_on(old::message_encrypt(
                plaintext.as_bytes(),
                &address,
                &mut store,
                &mut identities(&alice, &bob),
            ))
            .unwrap();
            let mut bytes = Vec::new();
            store
                .0
                .unwrap()
                .serialize_into_for_store(&mut bytes, &INCARNATION);
            a = new::codec::decode_session_record_live(&RustCrypto, &bytes, &INCARNATION).unwrap();
            let clear = new::Engine::new(&RustCrypto, &mut random)
                .decrypt(&mut b, wire.serialize())
                .unwrap();
            assert_eq!(clear, plaintext.as_bytes());
        } else {
            grant(&mut b);
            let wire = new::Engine::new(&RustCrypto, &mut random)
                .encrypt(&mut b, plaintext.as_bytes())
                .unwrap();
            let mut store = SessionStore(Some(legacy(&a)));
            let message = old::SignalMessage::try_from(wire.as_slice()).unwrap();
            let clear = futures::executor::block_on(old::message_decrypt_signal(
                &message,
                &address,
                &mut store,
                &mut identities(&alice, &bob),
                &mut random.0,
            ))
            .unwrap_or_else(|error| panic!("turn={turn}: {error}"));
            assert_eq!(clear.plaintext, plaintext.as_bytes());
            let mut bytes = Vec::new();
            store
                .0
                .unwrap()
                .serialize_into_for_store(&mut bytes, &INCARNATION);
            a = new::codec::decode_session_record_live(&RustCrypto, &bytes, &INCARNATION).unwrap();
        }
    }
}

#[test]
fn restart_burns_identical_reserved_counters_in_both_readers() {
    let (mut a, _, _, _) = pair();
    let mut random = Random::seeded();
    new::Engine::new(&RustCrypto, &mut random)
        .encrypt(&mut a, b"before crash")
        .unwrap();
    let bytes = new::codec::encode_session_record(&a);
    let recovered = new::codec::decode_session_record(&RustCrypto, &bytes).unwrap();
    let mut old_recovered = old::SessionRecord::deserialize(&bytes).unwrap();
    old_recovered.waive_counter_lease();
    let expected = old_recovered
        .session_state()
        .unwrap()
        .get_sender_chain_key()
        .unwrap();
    let actual = recovered.current.as_ref().unwrap().sender.as_ref().unwrap();
    assert_eq!(actual.index, expected.index());
    assert_eq!(&actual.key, expected.key());
    assert_eq!(actual.index, 64);
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
        self.0 = Some(record.clone());
        Ok(())
    }
}
fn old_sender(record: &new::SenderKeyRecord) -> old::SenderKeyRecord {
    old::SenderKeyRecord::deserialize_for_store(
        &new::codec::encode_sender_key_record(record),
        &INCARNATION,
    )
    .unwrap()
}
#[test]
fn sender_key_messages_and_storage_alternate_both_directions() {
    let mut random = Random::seeded();
    let mut sender = new::SenderKeyRecord::default();
    let distribution = new::Engine::new(&RustCrypto, &mut random)
        .create_sender_distribution(&mut sender)
        .unwrap();
    let mut receiver = new::SenderKeyRecord::default();
    new::Engine::new(&RustCrypto, &mut random)
        .process_sender_distribution(&mut receiver, &distribution)
        .unwrap();
    let name =
        old::SenderKeyName::from_parts("synthetic-group@g.us", "synthetic-sender:1@s.whatsapp.net");
    for index in 0..12 {
        sender
            .reservation
            .prepare(sender.states[0].iteration, 64, INCARNATION)
            .unwrap();
        sender.reservation.mark_committed();
        let plaintext = format!("synthetic group message {index}");
        let wire = if index % 2 == 0 {
            let mut store = SenderStore(Some(old_sender(&sender)));
            let wire = futures::executor::block_on(old::group_encrypt(
                &mut store,
                &name,
                plaintext.as_bytes(),
                &mut random.0,
            ))
            .unwrap();
            sender = new::codec::decode_sender_key_record_live(
                &RustCrypto,
                &store.0.unwrap().serialize_for_store(&INCARNATION).unwrap(),
                &INCARNATION,
            )
            .unwrap();
            wire.serialized().to_vec()
        } else {
            new::Engine::new(&RustCrypto, &mut random)
                .encrypt_sender_key(&mut sender, plaintext.as_bytes())
                .unwrap()
        };
        let clear = if index % 2 == 0 {
            new::Engine::new(&RustCrypto, &mut random)
                .decrypt_sender_key(&mut receiver, &wire)
                .unwrap()
        } else {
            let mut store = SenderStore(Some(old_sender(&receiver)));
            let clear =
                futures::executor::block_on(old::group_decrypt(&wire, &mut store, &name)).unwrap();
            receiver = new::codec::decode_sender_key_record_live(
                &RustCrypto,
                &store.0.unwrap().serialize_for_store(&INCARNATION).unwrap(),
                &INCARNATION,
            )
            .unwrap();
            clear
        };
        assert_eq!(clear, plaintext.as_bytes());
    }
}

#[test]
fn failed_authentication_and_replay_leave_new_session_unchanged() {
    let (mut sender, mut receiver, _, _) = pair();
    let mut random = Random::seeded();
    let wire = new::Engine::new(&RustCrypto, &mut random)
        .encrypt(&mut sender, b"synthetic authenticated text")
        .unwrap();
    let original = new::codec::encode_session_record(&receiver);
    let mut tampered = wire.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(
        new::Engine::new(&RustCrypto, &mut random)
            .decrypt(&mut receiver, &tampered)
            .is_err()
    );
    assert_eq!(new::codec::encode_session_record(&receiver), original);
    assert_eq!(
        new::Engine::new(&RustCrypto, &mut random)
            .decrypt(&mut receiver, &wire)
            .unwrap(),
        b"synthetic authenticated text"
    );
    let committed = new::codec::encode_session_record(&receiver);
    assert!(
        new::Engine::new(&RustCrypto, &mut random)
            .decrypt(&mut receiver, &wire)
            .is_err()
    );
    assert_eq!(new::codec::encode_session_record(&receiver), committed);
}

#[test]
fn unknown_cache_incarnation_burns_reservation_but_live_cache_preserves_it() {
    let (record, _, _, _) = pair();
    let bytes = new::codec::encode_session_record(&record);
    let live = new::codec::decode_session_record_live(&RustCrypto, &bytes, &INCARNATION).unwrap();
    assert_eq!(live.current.unwrap().sender.as_ref().unwrap().index, 0);
    let restored =
        new::codec::decode_session_record_live(&RustCrypto, &bytes, &[0xff; 16]).unwrap();
    assert_eq!(restored.current.unwrap().sender.as_ref().unwrap().index, 64);
}

#[test]
fn duplicate_or_overflowing_counter_reservations_are_rejected_by_both_readers() {
    let (record, _, _, _) = pair();
    let bytes = new::codec::encode_session_record(&record);
    for malformed in [
        vec![0xa0, 0x06, 1],
        vec![0xa0, 0x06, 0xff, 0xff, 0xff, 0xff, 0x1f],
        vec![0xa2, 0x06, 0],
    ] {
        let mut input = bytes.clone();
        input.extend_from_slice(&malformed);
        assert!(new::codec::decode_session_record(&RustCrypto, &input).is_err());
        assert!(old::SessionRecord::deserialize(&input).is_err());
    }
}

#[test]
fn historical_large_skipped_backlog_and_closed_chain_remain_usable() {
    let (mut record, _, _, _) = pair();
    let chain = &mut record.current.as_mut().unwrap().receivers[0];
    chain.index = 3000;
    chain.message_key_limit = Some(4000);
    chain.skipped = (0..3000)
        .map(|index| new::MessageKey::Seed {
            index,
            seed: [(index % 251) as u8; 32],
        })
        .collect();
    let ratchet = chain.ratchet;
    chain.exhausted = true;
    chain.key = [0; 32];
    let input = new::codec::encode_session_record(&record);
    let mut old_record = old::SessionRecord::deserialize_for_store(&input, &INCARNATION).unwrap();
    let state = old_record.session_state_mut().unwrap();
    let public = old::PublicKey::from_djb_public_key_bytes(&ratchet).unwrap();
    assert!(state.get_receiver_chain_key(&public).is_err());
    let consumed = state.get_message_keys(&public, 2999).unwrap().unwrap();
    assert_eq!(consumed.counter(), 2999);
    let mut old_bytes = Vec::new();
    old_record.serialize_into_for_store(&mut old_bytes, &INCARNATION);
    let decoded =
        new::codec::decode_session_record_live(&RustCrypto, &old_bytes, &INCARNATION).unwrap();
    let chain = &decoded.current.as_ref().unwrap().receivers[0];
    assert!(chain.exhausted);
    assert_eq!(chain.index, 3000);
    assert_eq!(chain.message_key_limit, Some(4000));
    assert_eq!(chain.skipped.len(), 2999);
    assert!(!chain.skipped.iter().any(|key| key.index() == 2999));
}

#[test]
#[allow(clippy::disallowed_methods)]
fn mixed_expanded_and_seed_message_keys_follow_historical_precedence() {
    use buffa::Message as _;
    use waproto::whatsapp::session_structure::chain::MessageKey;
    let (record, _, _, _) = pair();
    let mut proto = waproto::whatsapp::RecordStructure::decode_from_slice(
        &new::codec::encode_session_record(&record),
    )
    .unwrap();
    let chain = &mut proto
        .current_session
        .as_option_mut()
        .unwrap()
        .receiver_chains[0];
    let mut key = MessageKey::default();
    key.index = Some(9);
    key.cipher_key = Some(bytes::Bytes::from_static(&[1; 32]));
    key.mac_key = Some(bytes::Bytes::from_static(&[2; 32]));
    key.iv = Some(bytes::Bytes::from_static(&[3; 16]));
    key.seed = Some(bytes::Bytes::from_static(&[4; 32]));
    chain.message_keys.push(key);
    let bytes = proto.encode_to_vec();
    let decoded = new::codec::decode_session_record(&RustCrypto, &bytes).unwrap();
    assert!(
        matches!(&decoded.current.as_ref().unwrap().receivers[0].skipped[0],new::MessageKey::Derived{index:9,material}if material.cipher==[1;32])
    );
    let old = old::SessionRecord::deserialize(&bytes).unwrap();
    let mut normalized = Vec::new();
    old.serialize_into(&mut normalized);
    let decoded = new::codec::decode_session_record(&RustCrypto, &normalized).unwrap();
    assert!(
        matches!(&decoded.current.as_ref().unwrap().receivers[0].skipped[0],new::MessageKey::Derived{index:9,material}if material.mac==[2;32])
    );
}

#[test]
fn archive_encoding_and_local_pending_fields_survive_crossed_storage() {
    let (mut record, _, _, _) = pair();
    let mut archived = record.current.as_ref().unwrap().clone();
    archived.pending_key_exchange = Some(vec![8, 9]);
    archived.pending_pre_key = Some(new::PendingPreKey {
        pre_key_id: Some(73),
        signed_pre_key_id: Some(-1),
        base_key: [5; 32],
        kyber_pre_key_id: Some(8),
        kyber_ciphertext: Some(vec![7; 33]),
    });
    record.install(archived);
    let old = old::SessionRecord::deserialize(&new::codec::encode_session_record(&record)).unwrap();
    let decoded =
        new::codec::decode_session_record(&RustCrypto, &old.serialize().unwrap()).unwrap();
    assert_eq!(decoded.previous.len(), 1);
    let state = decoded.current.as_ref().unwrap();
    assert_eq!(state.pending_key_exchange, Some(vec![8, 9]));
    let pending = state.pending_pre_key.as_ref().unwrap();
    assert_eq!(pending.pre_key_id, Some(73));
    assert_eq!(pending.signed_pre_key_id, Some(-1));
    assert_eq!(pending.kyber_pre_key_id, Some(8));
    assert_eq!(pending.kyber_ciphertext, Some(vec![7; 33]));
    let bytes = new::codec::encode_session_record(&decoded);
    let reread = new::codec::decode_session_record(&RustCrypto, &bytes).unwrap();
    assert_eq!(reread.previous[0].encoded(), decoded.previous[0].encoded());
}

#[test]
fn historical_json_and_binary_serde_keys_cross_read_without_conversion() {
    let mut random = Random::seeded();
    let key = new::Engine::new(&RustCrypto, &mut random)
        .generate_key_pair()
        .unwrap();
    let legacy = old_key(&key);
    let replacement = replacement::core::KeyPair::new(
        replacement::core::PublicKey::from_djb_public_key_bytes(&key.public).unwrap(),
        replacement::core::PrivateKey::deserialize(&key.private).unwrap(),
    );
    let json = serde_json::to_vec(&legacy).unwrap();
    assert_eq!(json, serde_json::to_vec(&replacement).unwrap());
    let reloaded: replacement::core::KeyPair = serde_json::from_slice(&json).unwrap();
    assert_eq!(reloaded, replacement);
    let old_reloaded: old::KeyPair =
        serde_json::from_slice(&serde_json::to_vec(&replacement).unwrap()).unwrap();
    assert_eq!(old_reloaded.public_key, legacy.public_key);
    assert_eq!(
        old_reloaded.private_key.serialize(),
        legacy.private_key.serialize()
    );
    let mut binary = Vec::new();
    ciborium::into_writer(&legacy, &mut binary).unwrap();
    let reloaded: replacement::core::KeyPair = ciborium::from_reader(binary.as_slice()).unwrap();
    assert_eq!(reloaded, replacement);
    let mut replacement_binary = Vec::new();
    ciborium::into_writer(&replacement, &mut replacement_binary).unwrap();
    assert_eq!(binary, replacement_binary);
    let old_reloaded: old::KeyPair = ciborium::from_reader(replacement_binary.as_slice()).unwrap();
    assert_eq!(old_reloaded.public_key, legacy.public_key);
    let identity: old::IdentityKeyPair = legacy.into();
    let json = serde_json::to_vec(&identity).unwrap();
    let reloaded: replacement::core::IdentityKeyPair = serde_json::from_slice(&json).unwrap();
    assert_eq!(serde_json::to_vec(&reloaded).unwrap(), json);
}

#[test]
fn classical_version_four_sessions_and_ciphertext_cross_both_engines() {
    let (mut alice, mut bob, alice_key, bob_key) = pair();
    alice.current.as_mut().unwrap().version = 4;
    bob.current.as_mut().unwrap().version = 4;
    let address = old::ProtocolAddress::new("synthetic-classic-v4", 1.into());
    let mut random = Random::seeded();
    let wire = new::Engine::new(&RustCrypto, &mut random)
        .encrypt(&mut alice, b"classic-v4-new")
        .unwrap();
    assert_eq!(wire[0], 0x43);
    let mut store = SessionStore(Some(legacy(&bob)));
    let message = old::SignalMessage::try_from(wire.as_slice()).unwrap();
    assert_eq!(
        futures::executor::block_on(old::message_decrypt_signal(
            &message,
            &address,
            &mut store,
            &mut identities(&bob_key, &alice_key),
            &mut random.0
        ))
        .unwrap()
        .plaintext,
        b"classic-v4-new"
    );
    let wire = futures::executor::block_on(old::message_encrypt(
        b"classic-v4-old",
        &address,
        &mut store,
        &mut identities(&bob_key, &alice_key),
    ))
    .unwrap();
    assert_eq!(wire.serialize()[0], 0x43);
    assert_eq!(
        new::Engine::new(&RustCrypto, &mut random)
            .decrypt(&mut alice, wire.serialize())
            .unwrap(),
        b"classic-v4-old"
    );
    let stored = new::codec::encode_session_record(&alice);
    assert_eq!(
        new::codec::decode_session_record_live(&RustCrypto, &stored, &INCARNATION)
            .unwrap()
            .current
            .unwrap()
            .version,
        4
    );
}

#[test]
fn stored_history_has_a_bounded_budget_separate_from_network_envelopes() {
    let (mut record, _, _, _) = pair();
    record.current.as_mut().unwrap().pending_pre_key = Some(new::PendingPreKey {
        pre_key_id: Some(73),
        signed_pre_key_id: Some(5),
        base_key: [4; 32],
        kyber_pre_key_id: Some(8),
        kyber_ciphertext: Some(vec![6; 16 * 1024 * 1024]),
    });
    let bytes = new::codec::encode_session_record(&record);
    assert!(bytes.len() > 16 * 1024 * 1024);
    assert!(old::SessionRecord::deserialize(&bytes).is_ok());
    let decoded = new::codec::decode_session_record(&RustCrypto, &bytes).unwrap();
    assert_eq!(
        decoded
            .current
            .as_ref()
            .unwrap()
            .pending_pre_key
            .as_ref()
            .unwrap()
            .kyber_ciphertext
            .as_ref()
            .unwrap()
            .len(),
        16 * 1024 * 1024
    );
    let oversized_wire = new::wire::SignalMessage {
        ratchet: [1; 32],
        counter: 0,
        previous_counter: 0,
        ciphertext: vec![7; 16 * 1024 * 1024],
        mac: [0; 8],
    }
    .encode();
    assert!(new::wire::SignalMessageView::decode(&oversized_wire).is_err());
}
