// SPDX-License-Identifier: MIT
use crate::engine::{codec::*, wire::*, *};
#[test]
fn direct_cbc_envelopes_match_selected_provider_bytes_and_persisted_state() {
    let provider = crate::crypto::RustCryptoProvider;
    let fallback = crate::crypto::ProviderCrypto::new(&provider);
    let mut random = Deterministic(184276);
    let (session, _, _) = peers(&mut Engine::new(&RustCrypto, &mut random), true);
    let mut group = SenderKeyRecord::default();
    Engine::new(&RustCrypto, &mut random)
        .create_sender_distribution(&mut group)
        .unwrap();
    reserve_group(&mut group);
    for len in [0, 1, 15, 16, 17, 31, 32, 33, 127, 128, 129, 1024] {
        let plaintext = vec![len as u8; len];
        let (mut native, mut selected) = (session.clone(), session.clone());
        assert_eq!(
            Engine::new(&RustCrypto, &mut Deterministic(71))
                .encrypt(&mut native, &plaintext)
                .unwrap(),
            Engine::new(&fallback, &mut Deterministic(71))
                .encrypt(&mut selected, &plaintext)
                .unwrap()
        );
        assert_eq!(
            encode_session_record(&native),
            encode_session_record(&selected)
        );
        let (mut native, mut selected) = (group.clone(), group.clone());
        assert_eq!(
            Engine::new(&RustCrypto, &mut Deterministic(72))
                .encrypt_sender_key(&mut native, &plaintext)
                .unwrap(),
            Engine::new(&fallback, &mut Deterministic(72))
                .encrypt_sender_key(&mut selected, &plaintext)
                .unwrap()
        );
        assert_eq!(
            encode_sender_key_record(&native),
            encode_sender_key_record(&selected)
        );
    }
}

#[test]
fn malformed_selected_cbc_lengths_do_not_advance_pairwise_or_group_state() {
    struct BadCbc(usize);
    impl crate::crypto::SymmetricCryptoProvider for BadCbc {
        fn aes_256_cbc_encrypt(
            &self,
            _: &[u8; 32],
            _: &[u8; 16],
            _: &[u8],
            output: &mut Vec<u8>,
        ) -> std::result::Result<(), crate::crypto::CryptoProviderError> {
            output.extend(std::iter::repeat_n(0, self.0));
            Ok(())
        }
    }
    let mut random = Deterministic(184277);
    let (session, _, _) = peers(&mut Engine::new(&RustCrypto, &mut random), true);
    let mut group = SenderKeyRecord::default();
    Engine::new(&RustCrypto, &mut random)
        .create_sender_distribution(&mut group)
        .unwrap();
    reserve_group(&mut group);
    for len in [0, 1, 15, 17, 31, 33] {
        let provider = BadCbc(len);
        let crypto = crate::crypto::ProviderCrypto::new(&provider);
        let mut record = session.clone();
        let before = encode_session_record(&record);
        assert_eq!(
            Engine::new(&crypto, &mut random).encrypt(&mut record, b"synthetic"),
            Err(Error::Crypto)
        );
        assert_eq!(encode_session_record(&record), before);
        let mut record = group.clone();
        let before = encode_sender_key_record(&record);
        assert_eq!(
            Engine::new(&crypto, &mut random).encrypt_sender_key(&mut record, b"synthetic"),
            Err(Error::Crypto)
        );
        assert_eq!(encode_sender_key_record(&record), before);
    }
}
#[test]
fn prekey_wrapper_errors_follow_provider_errors_and_never_publish_state() {
    let mut random = Deterministic(1859);
    let (session, _, _) = peers(&mut Engine::new(&RustCrypto, &mut random), true);
    for unsupported in [false, true] {
        let mut invalid = session.clone();
        let pending = invalid
            .current
            .as_mut()
            .unwrap()
            .pending_pre_key
            .as_mut()
            .unwrap();
        let pending = std::sync::Arc::make_mut(pending);
        pending.signed_pre_key_id = None;
        if unsupported {
            pending.kyber_pre_key_id = Some(1);
        }
        let before = encode_session_record(&invalid);
        let provider = FailingMac::new(3);
        let crypto = crate::crypto::ProviderCrypto::new(&provider);
        assert_eq!(
            Engine::new(&crypto, &mut random).encrypt(&mut invalid, b"synthetic"),
            Err(Error::Crypto)
        );
        assert_eq!(encode_session_record(&invalid), before);
        assert_eq!(
            Engine::new(&RustCrypto, &mut random).encrypt(&mut invalid, b"synthetic"),
            Err(if unsupported {
                Error::Version
            } else {
                Error::MissingKey
            })
        );
        assert_eq!(encode_session_record(&invalid), before);
    }
}

struct FailingMac {
    calls: std::sync::atomic::AtomicUsize,
    fail_at: usize,
}
impl crate::crypto::SymmetricCryptoProvider for FailingMac {
    fn hmac_sha256(
        &self,
        key: &[u8],
        parts: &[&[u8]],
    ) -> std::result::Result<[u8; 32], crate::crypto::CryptoProviderError> {
        let call = self
            .calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        if call == self.fail_at {
            return Err(crate::crypto::CryptoProviderError::BackendFailed);
        }
        Ok(RustCrypto.hmac_sha256_parts_array(key, parts))
    }
}
impl FailingMac {
    fn new(fail_at: usize) -> Self {
        Self {
            calls: Default::default(),
            fail_at,
        }
    }
}
#[test]
fn mac_failures_leave_ciphertext_preparation_and_reservation_burn_unpublished() {
    let mut random = Deterministic(501);
    let (session, _, _) = peers(&mut Engine::new(&RustCrypto, &mut random), true);
    for fail_at in 1..=3 {
        let mut record = session.clone();
        let before = encode_session_record(&record);
        let provider = FailingMac::new(fail_at);
        let crypto = crate::crypto::ProviderCrypto::new(&provider);
        assert_eq!(
            Engine::new(&crypto, &mut random).encrypt(&mut record, b"synthetic"),
            Err(Error::Crypto)
        );
        assert_eq!(encode_session_record(&record), before, "MAC call {fail_at}");
    }
    for fail_at in [1, 17, 64] {
        let mut record = session.clone();
        let before = encode_session_record(&record);
        let provider = FailingMac::new(fail_at);
        let crypto = crate::crypto::ProviderCrypto::new(&provider);
        assert_eq!(record.materialize_reservation(&crypto), Err(Error::Crypto));
        assert_eq!(
            encode_session_record(&record),
            before,
            "burn MAC call {fail_at}"
        );
        record.materialize_reservation(&RustCrypto).unwrap();
        assert_eq!(
            record
                .current
                .as_ref()
                .unwrap()
                .sender
                .as_ref()
                .unwrap()
                .index,
            64
        );
    }
    let mut group = SenderKeyRecord::default();
    Engine::new(&RustCrypto, &mut random)
        .create_sender_distribution(&mut group)
        .unwrap();
    reserve_group(&mut group);
    for fail_at in 1..=2 {
        let mut record = group.clone();
        let before = encode_sender_key_record(&record);
        let provider = FailingMac::new(fail_at);
        let crypto = crate::crypto::ProviderCrypto::new(&provider);
        assert_eq!(
            Engine::new(&crypto, &mut random).encrypt_sender_key(&mut record, b"synthetic"),
            Err(Error::Crypto)
        );
        assert_eq!(encode_sender_key_record(&record), before);
    }
}
#[test]
fn oversized_ciphertexts_do_not_publish_pairwise_or_group_state() {
    let mut random = Deterministic(502);
    let (mut session, _, _) = peers(&mut Engine::new(&RustCrypto, &mut random), true);
    let plaintext = vec![0; MAX_MESSAGE_SIZE];
    let before = encode_session_record(&session);
    assert_eq!(
        Engine::new(&RustCrypto, &mut random).encrypt(&mut session, &plaintext),
        Err(Error::Encoding("protobuf size limit"))
    );
    assert_eq!(encode_session_record(&session), before);
    let mut group = SenderKeyRecord::default();
    Engine::new(&RustCrypto, &mut random)
        .create_sender_distribution(&mut group)
        .unwrap();
    reserve_group(&mut group);
    let before = encode_sender_key_record(&group);
    assert_eq!(
        Engine::new(&RustCrypto, &mut random).encrypt_sender_key(&mut group, &plaintext),
        Err(Error::Encoding("protobuf size limit"))
    );
    assert_eq!(encode_sender_key_record(&group), before);
}
#[test]
fn closed_receiver_keeps_skipped_keys_and_distinguishes_unreceived_counters() {
    let mut chain = Chain::new([3; 32], None, [4; 32]);
    chain.index = 5;
    chain.exhausted = true;
    chain.skipped.push_back(MessageKey::Seed {
        index: 3,
        seed: [5; 32],
    });
    let pending = chain.plan(&RustCrypto, 3, PEER_FUTURE_LIMIT).unwrap();
    assert_eq!(pending.message.index(), 3);
    pending.commit(&mut chain);
    for index in [0, 3, 4] {
        assert!(matches!(
            chain.plan(&RustCrypto, index, PEER_FUTURE_LIMIT),
            Err(Error::Duplicate)
        ));
    }
    for index in [5, 6, u32::MAX] {
        assert!(matches!(
            chain.plan(&RustCrypto, index, PEER_FUTURE_LIMIT),
            Err(Error::ClosedChain)
        ));
    }
    assert_eq!(chain.index, 5);
}
#[test]
fn delayed_key_consumption_preserves_duplicate_selection_and_sibling_state() {
    let mut chain = Chain::new([3; 32], None, [4; 32]);
    chain.index = 4;
    for (index, seed) in [(1, [11; 32]), (0, [10; 32]), (3, [13; 32]), (0, [12; 32])] {
        chain.skipped.push_back(MessageKey::Seed { index, seed });
    }
    let sibling = chain.clone();
    let remaining: &[&[u32]] = &[&[0, 0, 3], &[3, 0], &[3], &[]];
    for ((index, seed), remaining) in [(1, [11; 32]), (0, [12; 32]), (0, [10; 32]), (3, [13; 32])]
        .into_iter()
        .zip(remaining)
    {
        let plan = chain.plan(&RustCrypto, index, PEER_FUTURE_LIMIT).unwrap();
        assert!(
            plan.message.expanded(&RustCrypto).unwrap()
                == MessageKey::Seed { index, seed }
                    .expanded(&RustCrypto)
                    .unwrap()
        );
        plan.commit(&mut chain);
        assert_eq!(
            chain
                .skipped
                .iter()
                .map(MessageKey::index)
                .collect::<Vec<_>>(),
            *remaining
        );
        assert_eq!(
            sibling
                .skipped
                .iter()
                .map(MessageKey::index)
                .collect::<Vec<_>>(),
            [1, 0, 3, 0]
        );
    }
    assert!(matches!(
        chain.plan(&RustCrypto, 0, PEER_FUTURE_LIMIT),
        Err(Error::Duplicate)
    ));
}
struct Deterministic(u64);
impl RandomSource for Deterministic {
    fn fill(&mut self, out: &mut [u8]) -> Result<()> {
        for byte in out {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *byte = (self.0 >> 32) as u8;
        }
        Ok(())
    }
}
fn reserve(record: &mut SessionRecord) {
    let index = record
        .current
        .as_ref()
        .unwrap()
        .sender
        .as_ref()
        .unwrap()
        .index;
    record.reservation.prepare(index, 64, [7; 16]).unwrap();
    record.reservation.mark_committed();
}
fn reserve_group(record: &mut SenderKeyRecord) {
    let index = record.states[0].iteration;
    record.reservation.prepare(index, 64, [8; 16]).unwrap();
    record.reservation.mark_committed();
}
fn peers(
    engine: &mut Engine<'_, RustCrypto, Deterministic>,
    one_time: bool,
) -> (SessionRecord, SessionRecord, ResponderParameters) {
    let alice = engine.generate_key_pair().unwrap();
    let bob = engine.generate_key_pair().unwrap();
    let signed = engine.generate_key_pair().unwrap();
    let ephemeral = engine.generate_key_pair().unwrap();
    let otp = one_time.then(|| engine.generate_key_pair().unwrap());
    let signature = engine
        .crypto
        .sign(&bob.private, &tagged_key(&signed.public), engine.random)
        .unwrap();
    let initiator = InitiatorParameters {
        local_identity: alice.clone(),
        local_registration: 11,
        remote_identity: bob.public,
        remote_registration: 22,
        remote_signed_pre_key: signed.public,
        remote_signed_pre_key_id: 3,
        remote_signed_pre_key_signature: signature,
        remote_one_time_pre_key: otp.as_ref().map(|key| (4, key.public)),
        base_key: ephemeral.clone(),
    };
    let responder = ResponderParameters {
        local_identity: bob,
        local_registration: 22,
        local_signed_pre_key: signed,
        local_one_time_pre_key: otp,
        remote_identity: alice.public,
        remote_base_key: ephemeral.public,
        remote_registration: 11,
    };
    let mut outgoing = SessionRecord::default();
    outgoing.install(engine.initialize_initiator(&initiator).unwrap());
    reserve(&mut outgoing);
    (outgoing, SessionRecord::default(), responder)
}
#[test]
fn x3dh_double_ratchet_roundtrip_and_reordering() {
    for one_time in [false, true] {
        let mut rng = Deterministic(42);
        let mut engine = Engine::new(&RustCrypto, &mut rng);
        let (mut alice, mut bob, p) = peers(&mut engine, one_time);
        let first = engine.encrypt(&mut alice, b"first").unwrap();
        assert_eq!(
            engine.decrypt_pre_key(&mut bob, &first, &p).unwrap(),
            b"first"
        );
        reserve(&mut bob);
        let reply = engine.encrypt(&mut bob, b"reply").unwrap();
        assert_eq!(engine.decrypt(&mut alice, &reply).unwrap(), b"reply");
        reserve(&mut alice);
        reserve(&mut alice);
        let a = engine.encrypt(&mut alice, b"a").unwrap();
        let b = engine.encrypt(&mut alice, b"b").unwrap();
        let c = engine.encrypt(&mut alice, b"c").unwrap();
        assert_eq!(engine.decrypt(&mut bob, &c).unwrap(), b"c");
        assert_eq!(engine.decrypt(&mut bob, &a).unwrap(), b"a");
        assert_eq!(engine.decrypt(&mut bob, &b).unwrap(), b"b");
        assert_eq!(engine.decrypt(&mut bob, &a), Err(Error::Duplicate));
        let encoded = encode_session_record(&bob);
        let loaded = decode_session_record(&RustCrypto, &encoded).unwrap();
        assert_eq!(
            loaded.current.as_ref().unwrap().remote_identity,
            bob.current.as_ref().unwrap().remote_identity
        );
    }
}
#[test]
fn forged_pairwise_message_never_consumes_state() {
    let mut rng = Deterministic(123);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let (mut a, mut b, p) = peers(&mut e, true);
    let first = e.encrypt(&mut a, b"init").unwrap();
    e.decrypt_pre_key(&mut b, &first, &p).unwrap();
    let wrapper = PreKeySignalMessage::decode(&e.encrypt(&mut a, b"next").unwrap()).unwrap();
    let before = encode_session_record(&b);
    let mut forged = wrapper.message.clone();
    *forged.last_mut().unwrap() ^= 1;
    assert_eq!(e.decrypt(&mut b, &forged), Err(Error::Authentication));
    assert_eq!(encode_session_record(&b), before);
    assert_eq!(e.decrypt(&mut b, &wrapper.message).unwrap(), b"next");
}
#[test]
fn unauthenticated_ratchet_does_not_require_or_consume_randomness() {
    struct UnavailableRandom;
    impl RandomSource for UnavailableRandom {
        fn fill(&mut self, _: &mut [u8]) -> Result<()> {
            Err(Error::Crypto)
        }
    }

    let mut random = Deterministic(0x1828_d003);
    let mut engine = Engine::new(&RustCrypto, &mut random);
    let (mut alice, mut bob, parameters) = peers(&mut engine, true);
    let first = engine.encrypt(&mut alice, b"initial").unwrap();
    engine
        .decrypt_pre_key(&mut bob, &first, &parameters)
        .unwrap();
    reserve(&mut bob);
    let reply = engine.encrypt(&mut bob, b"authenticated ratchet").unwrap();
    let before = encode_session_record(&alice);
    let mut forged = reply.clone();
    *forged.last_mut().unwrap() ^= 1;
    let mut unavailable_random = UnavailableRandom;
    let mut unavailable = Engine::new(&RustCrypto, &mut unavailable_random);
    assert_eq!(
        unavailable.decrypt(&mut alice, &forged),
        Err(Error::Authentication)
    );
    assert_eq!(encode_session_record(&alice), before);
    assert_eq!(unavailable.decrypt(&mut alice, &reply), Err(Error::Crypto));
    assert_eq!(encode_session_record(&alice), before);
    assert_eq!(
        engine.decrypt(&mut alice, &reply).unwrap(),
        b"authenticated ratchet"
    );
    assert!(alice.reservation.requires_durable_commit());
}
#[test]
fn reservations_gate_and_burn_without_reusing_counters() {
    let mut rng = Deterministic(7);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let (mut a, _, _) = peers(&mut e, false);
    a.reservation = Reservation::default();
    assert_eq!(
        e.encrypt(&mut a, b"blocked"),
        Err(Error::ReservationRequired)
    );
    a.reservation.prepare(0, 64, [9; 16]).unwrap();
    assert_eq!(
        e.encrypt(&mut a, b"pending"),
        Err(Error::ReservationRequired)
    );
    let stored = encode_session_record(&a);
    a.reservation.mark_committed();
    e.encrypt(&mut a, b"ok").unwrap();
    let live =
        decode_session_record_live(&RustCrypto, &encode_session_record(&a), &[9; 16]).unwrap();
    assert_eq!(
        live.current
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .unwrap()
            .index,
        1
    );
    let reload = decode_session_record(&RustCrypto, &stored).unwrap();
    assert_eq!(
        reload
            .current
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .unwrap()
            .index,
        64
    );
    assert!(!reload.reservation.permits(64));
}
#[test]
fn reservation_parser_rejects_bad_ceilings_and_distrusts_bad_tokens() {
    let mut duplicate = Vec::new();
    uint(&mut duplicate, 100, 1);
    uint(&mut duplicate, 100, 2);
    assert!(decode_session_record(&RustCrypto, &duplicate).is_err());
    let mut overflow = Vec::new();
    uint(&mut overflow, 100, u64::MAX);
    assert!(decode_session_record(&RustCrypto, &overflow).is_err());
    let mut malformed = Vec::new();
    bytes(&mut malformed, 100, &[1]);
    assert!(decode_session_record(&RustCrypto, &malformed).is_err());
    let mut token = Vec::new();
    uint(&mut token, 100, 2);
    bytes(&mut token, 101, &[9; 16]);
    bytes(&mut token, 101, &[9; 16]);
    let reload = decode_session_record_live(&RustCrypto, &token, &[9; 16]).unwrap();
    assert!(!reload.reservation.permits(0));
    assert!(
        decode_session_record(&RustCrypto, &[])
            .unwrap()
            .current
            .is_none()
    );
}
#[test]
fn sender_key_reordering_signature_rejection_and_replay() {
    let mut rng = Deterministic(55);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let mut sending = SenderKeyRecord::default();
    let mut receiving = SenderKeyRecord::default();
    let distribution = e.create_sender_distribution(&mut sending).unwrap();
    e.process_sender_distribution(&mut receiving, &distribution)
        .unwrap();
    reserve_group(&mut sending);
    let a = e.encrypt_sender_key(&mut sending, b"a").unwrap();
    let b = e.encrypt_sender_key(&mut sending, b"b").unwrap();
    let c = e.encrypt_sender_key(&mut sending, b"c").unwrap();
    let before = encode_sender_key_record(&receiving);
    let mut forged = c.clone();
    *forged.last_mut().unwrap() ^= 1;
    assert!(e.decrypt_sender_key(&mut receiving, &forged).is_err());
    assert_eq!(encode_sender_key_record(&receiving), before);
    assert_eq!(e.decrypt_sender_key(&mut receiving, &c).unwrap(), b"c");
    let encoded = encode_sender_key_record(&receiving);
    receiving = decode_sender_key_record(&RustCrypto, &encoded).unwrap();
    assert_eq!(e.decrypt_sender_key(&mut receiving, &a).unwrap(), b"a");
    e.process_sender_distribution(&mut receiving, &distribution)
        .unwrap();
    assert_eq!(e.decrypt_sender_key(&mut receiving, &b).unwrap(), b"b");
    assert_eq!(
        e.decrypt_sender_key(&mut receiving, &b),
        Err(Error::Duplicate)
    );
}
#[test]
fn prekey_formats_and_timestamp_are_preserved() {
    let key = KeyPair {
        public: [1; 32],
        private: [2; 32],
    };
    let record = SignedPreKeyRecord {
        id: 17,
        key_pair: key.clone(),
        signature: [3; 64],
        timestamp: u64::MAX - 5,
    };
    let decoded = decode_signed_pre_key(&encode_signed_pre_key(&record)).unwrap();
    assert!(decoded == record);
    let mut tagged = Vec::new();
    uint(&mut tagged, 1, 17);
    bytes(&mut tagged, 2, &tagged_key(&key.public));
    bytes(&mut tagged, 3, &key.private);
    let decoded = decode_pre_key(&tagged).unwrap();
    assert_eq!(decoded.key_pair.public, key.public);
    let canonical = encode_pre_key(&decoded);
    let mut f = Fields::new(&canonical).unwrap();
    while let Some((n, v)) = f.next().unwrap() {
        if n == 2 {
            assert_eq!(as_bytes(v).unwrap().len(), 32);
        }
    }
}
#[test]
fn compact_skipped_keys_survive_persistence() {
    let mut rng = Deterministic(82);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let (mut a, mut b, p) = peers(&mut e, false);
    let first = e.encrypt(&mut a, b"init").unwrap();
    e.decrypt_pre_key(&mut b, &first, &p).unwrap();
    let delayed = e.encrypt(&mut a, b"delayed").unwrap();
    for _ in 0..10 {
        e.encrypt(&mut a, b"gap").unwrap();
    }
    let last = e.encrypt(&mut a, b"last").unwrap();
    e.decrypt_pre_key(&mut b, &last, &p).unwrap();
    assert!(
        b.current
            .as_ref()
            .unwrap()
            .receivers
            .iter()
            .flat_map(|c| &c.skipped)
            .all(|k| matches!(k, MessageKey::Seed { .. }))
    );
    let encoded = encode_session_record(&b);
    let mut decoded = decode_session_record(&RustCrypto, &encoded).unwrap();
    assert_eq!(
        e.decrypt_pre_key(&mut decoded, &delayed, &p).unwrap(),
        b"delayed"
    );
}
#[test]
fn alternating_ratchets_survive_every_turn_reload() {
    let mut rng = Deterministic(143);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let (mut a, mut b, p) = peers(&mut e, true);
    let first = e.encrypt(&mut a, b"init").unwrap();
    e.decrypt_pre_key(&mut b, &first, &p).unwrap();
    for turn in 0u8..12 {
        if turn % 2 == 0 {
            reserve(&mut b);
            let message = e.encrypt(&mut b, &[turn]).unwrap();
            assert_eq!(e.decrypt(&mut a, &message).unwrap(), [turn]);
        } else {
            reserve(&mut a);
            let message = e.encrypt(&mut a, &[turn]).unwrap();
            assert_eq!(e.decrypt(&mut b, &message).unwrap(), [turn]);
        }
        a = decode_session_record_live(&RustCrypto, &encode_session_record(&a), &[7; 16]).unwrap();
        b = decode_session_record_live(&RustCrypto, &encode_session_record(&b), &[7; 16]).unwrap();
    }
}
#[test]
fn existing_prekey_session_does_not_need_consumed_one_time_key() {
    let mut rng = Deterministic(43);
    let mut e = Engine::new(&RustCrypto, &mut rng);
    let (mut a, mut b, mut p) = peers(&mut e, true);
    let first = e.encrypt(&mut a, b"first").unwrap();
    e.decrypt_pre_key(&mut b, &first, &p).unwrap();
    p.local_one_time_pre_key = None;
    let second = e.encrypt(&mut a, b"second").unwrap();
    assert_eq!(e.decrypt_pre_key(&mut b, &second, &p).unwrap(), b"second");
}

#[test]
fn fresh_dh_cannot_publish_with_the_previous_chain_commitment() {
    let mut rng = Deterministic(0x1828_d002);
    let mut engine = Engine::new(&RustCrypto, &mut rng);
    let (mut alice, mut bob, parameters) = peers(&mut engine, false);
    let first = engine.encrypt(&mut alice, b"init").unwrap();
    engine
        .decrypt_pre_key(&mut bob, &first, &parameters)
        .unwrap();
    reserve(&mut bob);
    let reply = engine.encrypt(&mut bob, b"fresh peer ratchet").unwrap();
    engine.decrypt(&mut alice, &reply).unwrap();
    assert_eq!(alice.reservation.ceiling, Some(64));
    assert!(alice.reservation.requires_durable_commit());
    assert_eq!(alice.reservation.incarnation, None);
    let snapshot = encode_session_record(&alice);
    assert_eq!(
        engine.encrypt(&mut alice, b"not durable"),
        Err(Error::ReservationRequired)
    );
    assert_eq!(encode_session_record(&alice), snapshot);
    alice.reservation.mark_committed();
    assert!(engine.encrypt(&mut alice, b"durable private key").is_ok());
}
