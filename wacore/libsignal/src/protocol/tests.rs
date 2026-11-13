// SPDX-License-Identifier: MIT
use super::*;

#[test]
fn receiver_state_inheritance_never_copies_a_signing_secret() {
    let pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let mut previous = SenderKeyRecord::new_empty();
    previous
        .add_sender_key_state(3, 17, 5, &[7; 32], pair.public_key, Some(pair.private_key))
        .unwrap();
    let before = previous.serialize().unwrap();
    let mut destination = SenderKeyRecord::new_empty();
    assert!(!destination.inherit_receiver_state(&previous, 17, &pair.public_key));
    assert!(destination.is_empty());
    assert_eq!(previous.serialize().unwrap(), before);
}
use async_trait::async_trait;
use rand::SeedableRng;
use std::{
    collections::HashMap,
    future::Future,
    pin::pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};
fn run<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
#[derive(Clone, Default)]
struct Sessions(HashMap<ProtocolAddress, SessionRecord>);
#[async_trait]
impl SessionStore for Sessions {
    async fn load_session(
        &self,
        address: &ProtocolAddress,
    ) -> error::Result<Option<SessionRecord>> {
        Ok(self.0.get(address).cloned())
    }
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> error::Result<()> {
        self.0.insert(address.clone(), record);
        Ok(())
    }
}
#[derive(Clone)]
struct Identity {
    pair: IdentityKeyPair,
    known: HashMap<ProtocolAddress, IdentityKey>,
    fail_save: bool,
    block_save: bool,
    save_calls: usize,
    only_trusted: Option<IdentityKey>,
}
#[async_trait]
impl IdentityKeyStore for Identity {
    async fn get_identity_key_pair(&self) -> error::Result<IdentityKeyPair> {
        Ok(self.pair.clone())
    }
    async fn get_local_registration_id(&self) -> error::Result<u32> {
        Ok(9)
    }
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> error::Result<IdentityChange> {
        self.save_calls += 1;
        if self.fail_save {
            return Err(SignalProtocolError::InvalidState(
                "test",
                "save failed".into(),
            ));
        }
        let old = self.known.insert(address.clone(), *identity);
        if self.block_save {
            std::future::pending::<()>().await;
        }
        Ok(match old {
            Some(previous) if previous != *identity => IdentityChange::ReplacedExistingWithKeys {
                previous,
                current: *identity,
            },
            _ => IdentityChange::NewOrUnchanged,
        })
    }
    async fn is_trusted_identity(
        &self,
        _: &ProtocolAddress,
        identity: &IdentityKey,
        _: Direction,
    ) -> error::Result<bool> {
        Ok(self.only_trusted.is_none_or(|trusted| trusted == *identity))
    }
    async fn get_identity(&self, address: &ProtocolAddress) -> error::Result<Option<IdentityKey>> {
        Ok(self.known.get(address).copied())
    }
}
struct Keys {
    prekey: PreKeyRecord,
    signed: SignedPreKeyRecord,
}
#[async_trait]
impl PreKeyStore for Keys {
    async fn get_pre_key(&self, id: PreKeyId) -> error::Result<PreKeyRecord> {
        if id == self.prekey.id()? {
            Ok(self.prekey.clone())
        } else {
            Err(SignalProtocolError::InvalidPreKeyId)
        }
    }
    async fn save_pre_key(&mut self, _: PreKeyId, key: &PreKeyRecord) -> error::Result<()> {
        self.prekey = key.clone();
        Ok(())
    }
    async fn remove_pre_key(&mut self, _: PreKeyId) -> error::Result<()> {
        Ok(())
    }
}
#[async_trait]
impl SignedPreKeyStore for Keys {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> error::Result<SignedPreKeyRecord> {
        if id == self.signed.id()? {
            Ok(self.signed.clone())
        } else {
            Err(SignalProtocolError::InvalidSignedPreKeyId)
        }
    }
    async fn save_signed_pre_key(
        &mut self,
        _: SignedPreKeyId,
        key: &SignedPreKeyRecord,
    ) -> error::Result<()> {
        self.signed = key.clone();
        Ok(())
    }
}
fn setup(rng: &mut rand::rngs::StdRng) -> (Identity, Identity, PreKeyBundle, Keys) {
    let a = Identity {
        pair: IdentityKeyPair::generate(rng),
        known: HashMap::new(),
        fail_save: false,
        block_save: false,
        save_calls: 0,
        only_trusted: None,
    };
    let b = Identity {
        pair: IdentityKeyPair::generate(rng),
        known: HashMap::new(),
        fail_save: false,
        block_save: false,
        save_calls: 0,
        only_trusted: None,
    };
    let signed = KeyPair::generate(rng);
    let otp = KeyPair::generate(rng);
    let signature = b
        .pair
        .private_key()
        .calculate_signature(&signed.public_key.serialize(), rng)
        .unwrap();
    let bundle = PreKeyBundle::new(
        9,
        0.into(),
        Some((4.into(), otp.public_key)),
        3.into(),
        signed.public_key,
        signature,
        *b.pair.identity_key(),
    )
    .unwrap();
    let keys = Keys {
        prekey: PreKeyRecord::new(4.into(), &otp),
        signed: SignedPreKeyRecord::new(3.into(), 123u64, &signed, &signature).unwrap(),
    };
    (a, b, bundle, keys)
}
#[test]
fn serialized_and_typed_encrypt_preserve_bytes_state_and_message_kind() {
    run(async {
        for version in [3, 4] {
            for prekey in [false, true] {
                let mut rng = rand::rngs::StdRng::seed_from_u64(184275);
                let (mut identity, _, bundle, _) = setup(&mut rng);
                let address = ProtocolAddress::new("serialized.invalid", 1.into());
                let mut typed_sessions = Sessions::default();
                process_prekey_bundle(
                    &address,
                    &mut typed_sessions,
                    &mut identity,
                    &bundle,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .unwrap();
                let state = typed_sessions
                    .0
                    .get_mut(&address)
                    .unwrap()
                    .session_state_mut()
                    .unwrap();
                state.inner.version = version;
                if !prekey {
                    state.clear_unacknowledged_pre_key_message();
                }
                let mut serialized_sessions = typed_sessions.clone();
                let mut serialized_identity = identity.clone();
                for len in [0, 1, 15, 16, 17, 1024] {
                    let plaintext = vec![len as u8; len];
                    let typed =
                        message_encrypt(&plaintext, &address, &mut typed_sessions, &mut identity)
                            .await
                            .unwrap();
                    let serialized = message_encrypt_serialized(
                        &plaintext,
                        &address,
                        &mut serialized_sessions,
                        &mut serialized_identity,
                    )
                    .await
                    .unwrap();
                    assert_eq!(serialized.serialized(), typed.serialized());
                    assert_eq!(serialized.message_type(), typed.message_type());
                    assert_eq!(
                        serialized_sessions.0[&address].serialize().unwrap(),
                        typed_sessions.0[&address].serialize().unwrap()
                    );
                    let pointer = serialized.serialized().as_ptr();
                    let output = serialized.into_serialized();
                    assert_eq!(output.as_ptr(), pointer);
                }
            }
        }
    })
}
#[test]
fn host_roundtrip_leases_and_consumed_prekey_reporting() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(41);
        let (mut a, mut b, bundle, keys) = setup(&mut rng);
        let (mut sa, mut sb) = (Sessions::default(), Sessions::default());
        let aa = ProtocolAddress::new("alice.invalid", 1.into());
        let ba = ProtocolAddress::new("bob.invalid", 1.into());
        process_prekey_bundle(&ba, &mut sa, &mut a, &bundle, &mut rng, UsePQRatchet::No)
            .await
            .unwrap();
        let ciphertext = message_encrypt(b"first", &ba, &mut sa, &mut a)
            .await
            .unwrap();
        assert!(sa.0[&ba].has_pending_reservation());
        let result = message_decrypt(
            &ciphertext,
            &aa,
            &mut sb,
            &mut b,
            &mut keys.clone_for_prekey(),
            &keys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        assert_eq!(result.plaintext, b"first");
        assert_eq!(result.consumed_prekey_id, Some(4.into()));
        let response = message_encrypt(b"response", &aa, &mut sb, &mut b)
            .await
            .unwrap();
        let result = message_decrypt(
            &response,
            &ba,
            &mut sa,
            &mut a,
            &mut keys.clone_for_prekey(),
            &keys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        assert_eq!(result.plaintext, b"response");
        assert!(matches!(
            message_decrypt(
                &response,
                &ba,
                &mut sa,
                &mut a,
                &mut keys.clone_for_prekey(),
                &keys,
                &mut rng,
                UsePQRatchet::No
            )
            .await,
            Err(SignalProtocolError::DuplicatedMessage(..))
        ));
    })
}
impl Keys {
    fn clone_for_prekey(&self) -> Self {
        Self {
            prekey: self.prekey.clone(),
            signed: self.signed.clone(),
        }
    }
}
#[test]
fn failed_rotated_identity_ciphertext_keeps_current_record() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(50);
        let (mut a, mut b, bundle, keys) = setup(&mut rng);
        let (mut sa, mut sb) = (Sessions::default(), Sessions::default());
        let aa = ProtocolAddress::new("alice.invalid", 1.into());
        let ba = ProtocolAddress::new("bob.invalid", 1.into());
        process_prekey_bundle(&ba, &mut sa, &mut a, &bundle, &mut rng, UsePQRatchet::No)
            .await
            .unwrap();
        let first = message_encrypt(b"first", &ba, &mut sa, &mut a)
            .await
            .unwrap();
        message_decrypt(
            &first,
            &aa,
            &mut sb,
            &mut b,
            &mut keys.clone_for_prekey(),
            &keys,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        let before = sb.0[&aa].serialize().unwrap();
        let (mut replacement, _, _, _) = setup(&mut rng);
        let mut fresh = Sessions::default();
        process_prekey_bundle(
            &ba,
            &mut fresh,
            &mut replacement,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        let rotated = message_encrypt(b"rotated", &ba, &mut fresh, &mut replacement)
            .await
            .unwrap();
        let mut bytes = rotated.serialized().to_vec();
        *bytes.last_mut().unwrap() ^= 1;
        let forged = CiphertextMessage::PreKeySignalMessage(
            PreKeySignalMessage::try_from(bytes.as_slice()).unwrap(),
        );
        assert!(
            message_decrypt(
                &forged,
                &aa,
                &mut sb,
                &mut b,
                &mut keys.clone_for_prekey(),
                &keys,
                &mut rng,
                UsePQRatchet::No
            )
            .await
            .is_err()
        );
        assert_eq!(sb.0[&aa].serialize().unwrap(), before);
    })
}
struct CheckoutStore {
    record: Mutex<Option<SessionRecord>>,
    queued: Option<SessionRecord>,
    completion: Arc<portable_atomic::AtomicBool>,
    cancelled: usize,
    block_completion: bool,
}
#[async_trait]
impl SessionStore for CheckoutStore {
    async fn load_session(&self, _: &ProtocolAddress) -> error::Result<Option<SessionRecord>> {
        Ok(self.record.lock().unwrap().clone())
    }
    async fn store_session(
        &mut self,
        _: &ProtocolAddress,
        record: SessionRecord,
    ) -> error::Result<()> {
        *self.record.lock().unwrap() = Some(record);
        Ok(())
    }
    async fn load_session_for_update(
        &self,
        _: &ProtocolAddress,
    ) -> error::Result<(Option<SessionRecord>, Option<SessionCheckoutKey>)> {
        Ok((
            self.record.lock().unwrap().take(),
            Some(SessionCheckoutKey::new(
                1,
                std::num::NonZeroU64::new(2).unwrap(),
            )),
        ))
    }
    fn try_store_session_from_checkout(
        &mut self,
        _: &ProtocolAddress,
        record: SessionRecord,
        _: Option<SessionCheckoutKey>,
        _: bool,
    ) -> SessionCheckoutStoreResult {
        self.queued = Some(record);
        SessionCheckoutStoreResult::Pending(self.completion.clone())
    }
    fn cancel_session_checkout(&mut self, _: &ProtocolAddress, _: Option<SessionCheckoutKey>) {
        self.cancelled += 1;
    }
    async fn complete_session_checkout(&mut self) {
        if self.block_completion {
            std::future::pending::<()>().await;
        }
        *self.record.lock().unwrap() = self.queued.take();
        self.completion
            .store(true, portable_atomic::Ordering::Release);
    }
}
fn checkout_store() -> CheckoutStore {
    CheckoutStore {
        record: Mutex::new(Some(SessionRecord::new_fresh())),
        queued: None,
        completion: Arc::new(portable_atomic::AtomicBool::new(false)),
        cancelled: 0,
        block_completion: false,
    }
}
#[test]
fn pairwise_provider_panic_restores_checkout_and_survives_reload() {
    use crate::crypto::{CryptoProviderError, RustCryptoProvider, SymmetricCryptoProvider};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Panicking(bool);
    impl SymmetricCryptoProvider for Panicking {
        fn aes_256_cbc_encrypt(
            &self,
            _: &[u8; 32],
            _: &[u8; 16],
            _: &[u8],
            output: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            output.extend_from_slice(&[77; 17]);
            panic!("synthetic CBC provider panic");
        }
        fn hmac_sha256(
            &self,
            key: &[u8],
            parts: &[&[u8]],
        ) -> Result<[u8; 32], CryptoProviderError> {
            assert!(!self.0, "synthetic HMAC provider panic");
            RustCryptoProvider.hmac_sha256(key, parts)
        }
    }
    for hmac in [false, true] {
        for owned in [false, true] {
            let mut f = run(established_signal_fixture(184260));
            let late = run(message_encrypt(
                b"delayed",
                &f.aa,
                &mut f.b_sessions,
                &mut f.b,
            ))
            .unwrap();
            let later = run(message_encrypt(
                b"later",
                &f.aa,
                &mut f.b_sessions,
                &mut f.b,
            ))
            .unwrap();
            run(message_decrypt(
                &later,
                &f.ba,
                &mut f.a_sessions,
                &mut f.a,
                &mut f.keys.clone_for_prekey(),
                &f.keys,
                &mut f.rng,
                UsePQRatchet::No,
            ))
            .unwrap();
            let mut record = f.a_sessions.0.remove(&f.ba).unwrap();
            let current = record.session_state().unwrap().clone();
            record.archive_current_state().unwrap();
            record.set_session_state(current);
            let counter = record
                .session_state()
                .unwrap()
                .get_sender_chain_key()
                .unwrap()
                .index();
            record.reserve_sender_chain_counters(counter);
            record.inner.reservation.mark_committed();
            let before = record.serialize().unwrap();
            let mut store = checkout_store();
            *store.record.lock().unwrap() = Some(record);
            let panic = catch_unwind(AssertUnwindSafe(|| {
                if owned {
                    run(message_encrypt_serialized_with_provider(
                        &Panicking(hmac),
                        b"never published",
                        &f.ba,
                        &mut store,
                        &mut f.a,
                    ))
                    .map(|_| ())
                } else {
                    run(message_encrypt_with_provider(
                        &Panicking(hmac),
                        b"never published",
                        &f.ba,
                        &mut store,
                        &mut f.a,
                    ))
                    .map(|_| ())
                }
            }));
            assert!(panic.is_err());
            let returned = store
                .queued
                .as_ref()
                .expect("checkout restored during unwind");
            assert!(
                returned.session_state().is_some(),
                "panic erased current session"
            );
            assert_eq!(returned.previous_session_count(), 1);
            assert_eq!(returned.serialize().unwrap(), before);
            run(store.complete_session_checkout());
            assert_eq!(store.cancelled, 0);
            let persisted = store
                .record
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .serialize()
                .unwrap();
            *store.record.lock().unwrap() = Some(SessionRecord::deserialize(&persisted).unwrap());
            store
                .completion
                .store(false, portable_atomic::Ordering::Release);
            let retry = run(message_encrypt(b"after panic", &f.ba, &mut store, &mut f.a)).unwrap();
            let decrypted = run(message_decrypt(
                &retry,
                &f.aa,
                &mut f.b_sessions,
                &mut f.b,
                &mut f.keys.clone_for_prekey(),
                &f.keys,
                &mut f.rng,
                UsePQRatchet::No,
            ))
            .unwrap();
            assert_eq!(decrypted.plaintext, b"after panic");
            store
                .completion
                .store(false, portable_atomic::Ordering::Release);
            let delayed = run(message_decrypt(
                &late,
                &f.ba,
                &mut store,
                &mut f.a,
                &mut f.keys.clone_for_prekey(),
                &f.keys,
                &mut f.rng,
                UsePQRatchet::No,
            ))
            .unwrap();
            assert_eq!(delayed.plaintext, b"delayed");
        }
    }
}
#[test]
fn dropping_checkout_returns_owned_record() {
    let mut store = checkout_store();
    let address = ProtocolAddress::new("test.invalid", 1.into());
    let checkout = run(SessionCheckout::load(&mut store, &address))
        .unwrap()
        .unwrap();
    drop(checkout);
    assert!(store.queued.is_some());
    assert_eq!(store.cancelled, 0);
    run(store.complete_session_checkout());
    assert!(store.record.lock().unwrap().is_some());
}
#[test]
fn cancelling_pending_commit_does_not_cancel_queued_restoration() {
    let mut store = checkout_store();
    store.block_completion = true;
    let address = ProtocolAddress::new("test.invalid", 1.into());
    let checkout = run(SessionCheckout::load(&mut store, &address))
        .unwrap()
        .unwrap();
    let mut commit = Box::pin(checkout.commit());
    assert!(
        commit
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(commit);
    assert!(store.queued.is_some());
    assert_eq!(store.cancelled, 0);
    store.block_completion = false;
    run(store.complete_session_checkout());
    assert!(store.record.lock().unwrap().is_some());
}
#[test]
fn signed_key_helper_stores_seconds_and_clamps_pre_epoch() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(2);
    let key = KeyPair::generate(&mut rng);
    let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 123_000_000).unwrap();
    let record =
        crate::store::record_helpers::new_signed_pre_key_record(4, &key, [5; 64], timestamp)
            .unwrap();
    assert_eq!(record.timestamp, Some(1_700_000_000));
    let negative = chrono::DateTime::from_timestamp(-1, 0).unwrap();
    assert_eq!(
        crate::store::record_helpers::new_signed_pre_key_record(4, &key, [5; 64], negative)
            .unwrap()
            .timestamp,
        Some(0)
    );
}

#[derive(Default)]
struct Groups {
    records: HashMap<SenderKeyName, SenderKeyRecord>,
    lock: Arc<async_lock::Mutex<()>>,
    writes: usize,
    fail_store: bool,
}
#[async_trait]
impl SenderKeyStore for Groups {
    async fn load_sender_key(
        &self,
        name: &SenderKeyName,
    ) -> error::Result<Option<SenderKeyRecord>> {
        Ok(self.records.get(name).cloned())
    }
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> error::Result<()> {
        self.writes += 1;
        if self.fail_store {
            return Err(SignalProtocolError::InvalidState(
                "test",
                "sender-key persistence failed".into(),
            ));
        }
        self.records.insert(name.clone(), record);
        Ok(())
    }
    async fn sender_key_lock(&self, _: &SenderKeyName) -> Arc<async_lock::Mutex<()>> {
        self.lock.clone()
    }
}
#[test]
fn prepared_group_context_preserves_state_and_rejects_unrelated_keys() {
    run(async {
        let name = SenderKeyName::from_parts("prepared.invalid@g.us", "sender.invalid:1");
        let mut random = rand::rngs::StdRng::seed_from_u64(184259);
        let mut first = Groups::default();
        create_sender_key_distribution_message(&name, &mut first, &mut random)
            .await
            .unwrap();
        let record = first.records.get_mut(&name).unwrap();
        let before = record.serialize().unwrap();
        let components = record.clone().into_components().unwrap();
        let context = record.prepare_crypto_context();
        assert_eq!(record.serialize().unwrap(), before);
        let mut rebuilt = SenderKeyRecord::from_components(components).unwrap();
        let rebuilt_before = rebuilt.serialize().unwrap();
        rebuilt.restore_crypto_context(&context);
        assert_eq!(rebuilt.serialize().unwrap(), rebuilt_before);
        assert_eq!(record.reserved_iteration(), rebuilt.reserved_iteration());
        let mut prepared = Groups::default();
        prepared.records.insert(name.clone(), rebuilt);
        let mut cold = Groups::default();
        cold.records.insert(
            name.clone(),
            SenderKeyRecord::from_components(record.clone().into_components().unwrap()).unwrap(),
        );
        for _ in 0..3 {
            let left = group_encrypt(
                &mut cold,
                &name,
                b"matching",
                &mut rand::rngs::StdRng::seed_from_u64(11),
            )
            .await
            .unwrap();
            let right = group_encrypt(
                &mut prepared,
                &name,
                b"matching",
                &mut rand::rngs::StdRng::seed_from_u64(11),
            )
            .await
            .unwrap();
            assert_eq!(left.serialized(), right.serialized());
            assert_eq!(
                cold.records[&name].serialize().unwrap(),
                prepared.records[&name].serialize().unwrap()
            );
        }
        let mut second = Groups::default();
        let distribution = create_sender_key_distribution_message(&name, &mut second, &mut random)
            .await
            .unwrap();
        let mut wrong = Groups {
            records: second.records.clone(),
            ..Default::default()
        };
        wrong
            .records
            .get_mut(&name)
            .unwrap()
            .restore_crypto_context(&context);
        let mut receiver = Groups::default();
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let normal = group_encrypt(
            &mut second,
            &name,
            b"different key",
            &mut rand::rngs::StdRng::seed_from_u64(12),
        )
        .await
        .unwrap();
        let restored = group_encrypt(
            &mut wrong,
            &name,
            b"different key",
            &mut rand::rngs::StdRng::seed_from_u64(12),
        )
        .await
        .unwrap();
        assert_eq!(normal.serialized(), restored.serialized());
        assert_eq!(
            second.records[&name].serialize().unwrap(),
            wrong.records[&name].serialize().unwrap()
        );
        receiver
            .records
            .get_mut(&name)
            .unwrap()
            .restore_crypto_context(&context);
        assert_eq!(
            group_decrypt(restored.serialized(), &mut receiver, &name)
                .await
                .unwrap(),
            b"different key"
        );
        let mut changed = first.records[&name].clone().into_components().unwrap();
        changed.states[0].signing_key.private = second.records[&name]
            .clone()
            .into_components()
            .unwrap()
            .states[0]
            .signing_key
            .private
            .clone();
        let mut cold = Groups::default();
        cold.records.insert(
            name.clone(),
            SenderKeyRecord::from_components(changed.clone()).unwrap(),
        );
        let mut restored = SenderKeyRecord::from_components(changed).unwrap();
        restored.restore_crypto_context(&context);
        let mut warm = Groups::default();
        warm.records.insert(name.clone(), restored);
        let left = group_encrypt(
            &mut cold,
            &name,
            b"changed private",
            &mut rand::rngs::StdRng::seed_from_u64(13),
        )
        .await
        .unwrap();
        let right = group_encrypt(
            &mut warm,
            &name,
            b"changed private",
            &mut rand::rngs::StdRng::seed_from_u64(13),
        )
        .await
        .unwrap();
        assert_eq!(left.serialized(), right.serialized());
        assert_eq!(
            cold.records[&name].serialize().unwrap(),
            warm.records[&name].serialize().unwrap()
        );
    });
}
#[test]
fn owned_group_output_preserves_bytes_state_provider_and_store_failure() {
    use crate::crypto::{CryptoProviderError, RustCryptoProvider, SymmetricCryptoProvider};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Selected(AtomicUsize);
    impl SymmetricCryptoProvider for Selected {
        fn aes_256_cbc_encrypt(
            &self,
            key: &[u8; 32],
            iv: &[u8; 16],
            input: &[u8],
            output: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            RustCryptoProvider.aes_256_cbc_encrypt(key, iv, input, output)
        }
    }
    run(async {
        let name = SenderKeyName::from_parts("owned-output.invalid@g.us", "sender.invalid:1");
        let mut random = rand::rngs::StdRng::seed_from_u64(184257);
        let fresh_random = || rand::rngs::StdRng::seed_from_u64(184258);
        let mut initial = Groups::default();
        let distribution = create_sender_key_distribution_message(&name, &mut initial, &mut random)
            .await
            .unwrap();
        for selected in [false, true] {
            for length in [0, 1, 15, 16, 17, 127, 128, 1024] {
                let mut typed_store = Groups {
                    records: initial.records.clone(),
                    ..Default::default()
                };
                let mut owned_store = Groups {
                    records: initial.records.clone(),
                    ..Default::default()
                };
                let snapshot = owned_store.records[&name].serialize().unwrap();
                let payload = vec![93; length];
                let provider = Selected(AtomicUsize::new(0));
                owned_store.fail_store = true;
                assert!(
                    group_encrypt_serialized(
                        &mut owned_store,
                        &name,
                        &payload,
                        &mut fresh_random()
                    )
                    .await
                    .is_err()
                );
                assert_eq!(owned_store.records[&name].serialize().unwrap(), snapshot);
                owned_store.fail_store = false;
                let typed = if selected {
                    group_encrypt_with_provider(
                        &provider,
                        &mut typed_store,
                        &name,
                        &payload,
                        &mut fresh_random(),
                    )
                    .await
                } else {
                    group_encrypt(&mut typed_store, &name, &payload, &mut fresh_random()).await
                }
                .unwrap();
                let owned = if selected {
                    group_encrypt_serialized_with_provider(
                        &provider,
                        &mut owned_store,
                        &name,
                        &payload,
                        &mut fresh_random(),
                    )
                    .await
                } else {
                    group_encrypt_serialized(&mut owned_store, &name, &payload, &mut fresh_random())
                        .await
                }
                .unwrap();
                assert_eq!(owned.message_type(), CiphertextMessageType::SenderKey);
                assert_eq!(owned.serialized(), typed.serialized());
                assert_eq!(
                    owned_store.records[&name].serialize().unwrap(),
                    typed_store.records[&name].serialize().unwrap()
                );
                assert_eq!(
                    provider.0.load(Ordering::Relaxed),
                    if selected { 2 } else { 0 }
                );
                let mut receiver = Groups::default();
                process_sender_key_distribution_message(&name, &distribution, &mut receiver)
                    .await
                    .unwrap();
                assert_eq!(
                    group_decrypt(owned.serialized(), &mut receiver, &name)
                        .await
                        .unwrap(),
                    payload
                );
            }
        }
    });
}
#[test]
fn owned_group_output_provider_faults_do_not_persist_state() {
    use crate::crypto::{CryptoProviderError, SymmetricCryptoProvider};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    struct Broken(u8);
    impl SymmetricCryptoProvider for Broken {
        fn aes_256_cbc_encrypt(
            &self,
            _: &[u8; 32],
            _: &[u8; 16],
            _: &[u8],
            output: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            output.extend_from_slice(&[0xad; 3]);
            match self.0 {
                0 => Err(CryptoProviderError::BackendFailed),
                1 => Ok(()),
                _ => panic!("synthetic provider unwind"),
            }
        }
    }
    let name = SenderKeyName::from_parts("provider-fault.invalid@g.us", "sender.invalid:1");
    for mode in 0..3 {
        let mut random = rand::rngs::StdRng::seed_from_u64(184259);
        let mut sender = Groups::default();
        run(create_sender_key_distribution_message(
            &name,
            &mut sender,
            &mut random,
        ))
        .unwrap();
        let before = sender.records[&name].serialize().unwrap();
        let writes = sender.writes;
        let result = catch_unwind(AssertUnwindSafe(|| {
            run(group_encrypt_serialized_with_provider(
                &Broken(mode),
                &mut sender,
                &name,
                &[84; 128],
                &mut random,
            ))
        }));
        if mode == 2 {
            assert!(result.is_err());
        } else {
            assert!(result.is_ok_and(|output| output.is_err()));
        }
        assert_eq!(sender.writes, writes);
        assert_eq!(sender.records[&name].serialize().unwrap(), before);
    }
}
#[test]
fn existing_sender_distribution_is_read_only_even_from_cold_storage() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(137);
        let mut sender = Groups::default();
        let mut receiver = Groups::default();
        let name = SenderKeyName::from_parts("test.invalid@g.us", "sender.invalid:1");
        let lock = sender.sender_key_lock(&name).await;
        let _guard = lock.lock().await;
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        assert_eq!(sender.writes, 1);
        sender.records.get_mut(&name).unwrap().clear_wire_gated();
        let before = sender.records[&name].serialize().unwrap();
        sender.fail_store = true;
        for cold in [false, true] {
            if cold {
                sender
                    .records
                    .insert(name.clone(), SenderKeyRecord::deserialize(&before).unwrap());
            }
            let repeated = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
                .await
                .expect("an unchanged, durable distribution needs no write");
            assert_eq!(repeated.serialized(), distribution.serialized());
            assert_eq!(sender.records[&name].serialize().unwrap(), before);
            assert!(!sender.records[&name].is_wire_gated());
            assert_eq!(sender.writes, 1);
        }
        // Deferring cold signing preparation must retain interoperability.
        sender.fail_store = false;
        let receiver_lock = receiver.sender_key_lock(&name).await;
        let _receiver_guard = receiver_lock.lock().await;
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let message = group_encrypt(&mut sender, &name, b"after cold distribution", &mut rng)
            .await
            .unwrap();
        assert_eq!(
            group_decrypt(message.serialized(), &mut receiver, &name)
                .await
                .unwrap(),
            b"after cold distribution"
        );
    })
}
#[test]
fn gated_sender_distribution_retries_persistence_before_returning() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(139);
        let mut sender = Groups::default();
        let name = SenderKeyName::from_parts("test.invalid@g.us", "sender.invalid:1");
        let lock = sender.sender_key_lock(&name).await;
        let _guard = lock.lock().await;
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        let before = sender.records[&name].serialize().unwrap();
        assert!(sender.records[&name].is_wire_gated());
        sender.fail_store = true;
        assert!(
            create_sender_key_distribution_message(&name, &mut sender, &mut rng)
                .await
                .is_err()
        );
        assert_eq!(sender.writes, 2);
        assert_eq!(sender.records[&name].serialize().unwrap(), before);
        assert!(sender.records[&name].is_wire_gated());
        sender.fail_store = false;
        let retried = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        assert_eq!(sender.writes, 3);
        assert_eq!(retried.serialized(), distribution.serialized());
        assert_eq!(sender.records[&name].serialize().unwrap(), before);
        assert!(sender.records[&name].is_wire_gated());
    })
}
#[test]
fn sender_distribution_and_counter_reservations_raise_host_gate() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(73);
        let mut sender = Groups::default();
        let mut receiver = Groups::default();
        let name = SenderKeyName::from_parts("test.invalid@g.us", "sender.invalid:1");
        let lock = sender.sender_key_lock(&name).await;
        let _guard = lock.lock().await;
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        assert!(sender.records[&name].is_wire_gated());
        sender.records.get_mut(&name).unwrap().clear_wire_gated();
        let receiver_lock = receiver.sender_key_lock(&name).await;
        let _receiver_guard = receiver_lock.lock().await;
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let message = group_encrypt(&mut sender, &name, b"group", &mut rng)
            .await
            .unwrap();
        assert!(sender.records[&name].is_wire_gated());
        let before = receiver.records[&name].serialize().unwrap();
        let mut forged = message.serialized().to_vec();
        *forged.last_mut().unwrap() ^= 1;
        assert!(matches!(
            group_decrypt(&forged, &mut receiver, &name).await,
            Err(SignalProtocolError::SignatureValidationFailed)
        ));
        assert_eq!(receiver.records[&name].serialize().unwrap(), before);
        assert_eq!(
            group_decrypt(message.serialized(), &mut receiver, &name)
                .await
                .unwrap(),
            b"group"
        );
        assert!(matches!(
            group_decrypt(message.serialized(), &mut receiver, &name).await,
            Err(SignalProtocolError::DuplicatedMessage(..))
        ));
    })
}
#[test]
fn raw_base_key_storage_encoding_is_preserved_for_retry_side_table() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(21);
    let local = IdentityKeyPair::generate(&mut rng);
    let remote = IdentityKeyPair::generate(&mut rng);
    let base = KeyPair::generate(&mut rng);
    let mut state = SessionState::new(
        3,
        local.identity_key(),
        remote.identity_key(),
        &RootKey::new([4; 32]),
        &base.public_key,
    );
    state.inner.alice_base_key_tagged = false;
    let record = SessionRecord::new(state);
    let encoded = record.serialize().unwrap();
    let loaded = SessionRecord::deserialize(&encoded).unwrap();
    assert_eq!(
        loaded.alice_base_key().unwrap(),
        base.public_key.public_key_bytes()
    );
    assert_eq!(loaded.serialize().unwrap(), encoded);
}

fn check_reused_session_buffer(for_store: bool, reserved: bool) {
    fn record(marker: u8) -> SessionRecord {
        let mut rng = rand::rngs::StdRng::seed_from_u64(u64::from(marker));
        let pair = KeyPair::generate(&mut rng);
        let identity = IdentityKey::new(pair.public_key);
        let mut state = SessionState::new(
            3,
            &identity,
            &identity,
            &RootKey::new([marker; 32]),
            &pair.public_key,
        );
        state.set_sender_chain(&pair, &ChainKey::new([marker; 32], u32::from(marker)));
        SessionRecord::new(state)
    }
    let token = [42; 16];
    let write = |record: &SessionRecord, bytes: &mut Vec<u8>| {
        if for_store {
            record.serialize_into_for_store(bytes, &token);
        } else {
            record.serialize_into(bytes);
        }
    };
    let mut obsolete = record(1);
    obsolete.archive_current_state().unwrap();
    obsolete.set_session_state(record(2).current.take().unwrap());
    let mut current = record(3);
    if reserved {
        obsolete.reserve_sender_chain_counters(2);
        current.reserve_sender_chain_counters(3);
    }
    let mut expected = Vec::new();
    write(&current, &mut expected);
    let mut reused = Vec::with_capacity(4096);
    let capacity = reused.capacity();
    write(&obsolete, &mut reused);
    for _ in 0..2 {
        write(&current, &mut reused);
        let loaded = if for_store {
            SessionRecord::deserialize_for_store(&reused, &token)
        } else {
            SessionRecord::deserialize(&reused)
        }
        .expect("a reused buffer must contain one loadable session record");
        assert_eq!(
            loaded.previous_session_count(),
            0,
            "old archives survived a flush"
        );
        assert_eq!(loaded.session_state().unwrap().inner.root, [3; 32]);
        assert_eq!(reused, expected, "a flush must replace the previous bytes");
        if reserved {
            let restarted = SessionRecord::deserialize_for_store(&reused, &[43; 16]).unwrap();
            assert_eq!(
                restarted
                    .session_state()
                    .unwrap()
                    .get_sender_chain_key()
                    .unwrap()
                    .index(),
                current.reserved_sender_chain_index()
            );
        }
    }
    let empty = SessionRecord::new_fresh();
    let mut expected_empty = Vec::new();
    write(&empty, &mut expected_empty);
    write(&empty, &mut reused);
    assert_eq!(reused, expected_empty);
    assert_eq!(
        reused.capacity(),
        capacity,
        "reuse must preserve the allocation"
    );
}

#[test]
fn session_serialize_into_replaces_archives_in_reused_buffer() {
    check_reused_session_buffer(false, false);
}
#[test]
fn session_serialize_into_for_store_replaces_archives_in_reused_buffer() {
    check_reused_session_buffer(true, false);
}
#[test]
fn session_serialize_into_replaces_reservations_in_reused_buffer() {
    check_reused_session_buffer(false, true);
}
#[test]
fn session_serialize_into_for_store_replaces_reservations_in_reused_buffer() {
    check_reused_session_buffer(true, true);
}

struct SignalFixture {
    a: Identity,
    b: Identity,
    a_sessions: Sessions,
    b_sessions: Sessions,
    keys: Keys,
    bundle: PreKeyBundle,
    aa: ProtocolAddress,
    ba: ProtocolAddress,
    rng: rand::rngs::StdRng,
    late_prekey: CiphertextMessage,
}
async fn established_signal_fixture(seed: u64) -> SignalFixture {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let (mut a, mut b, bundle, keys) = setup(&mut rng);
    let (mut a_sessions, mut b_sessions) = (Sessions::default(), Sessions::default());
    let aa = ProtocolAddress::new("alice.invalid", 1.into());
    let ba = ProtocolAddress::new("bob.invalid", 1.into());
    process_prekey_bundle(
        &ba,
        &mut a_sessions,
        &mut a,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    let first = message_encrypt(b"first", &ba, &mut a_sessions, &mut a)
        .await
        .unwrap();
    message_decrypt(
        &first,
        &aa,
        &mut b_sessions,
        &mut b,
        &mut keys.clone_for_prekey(),
        &keys,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    let late_prekey = message_encrypt(b"delayed prekey", &ba, &mut a_sessions, &mut a)
        .await
        .unwrap();
    let response = message_encrypt(b"response", &aa, &mut b_sessions, &mut b)
        .await
        .unwrap();
    message_decrypt(
        &response,
        &ba,
        &mut a_sessions,
        &mut a,
        &mut keys.clone_for_prekey(),
        &keys,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    SignalFixture {
        a,
        b,
        a_sessions,
        b_sessions,
        keys,
        bundle,
        aa,
        ba,
        rng,
        late_prekey,
    }
}
async fn replace_sending_identity(f: &mut SignalFixture) -> IdentityKey {
    let (mut replacement, _, _, _) = setup(&mut f.rng);
    let mut sessions = Sessions::default();
    process_prekey_bundle(
        &f.ba,
        &mut sessions,
        &mut replacement,
        &f.bundle,
        &mut f.rng,
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    let message = message_encrypt(
        b"replacement identity",
        &f.ba,
        &mut sessions,
        &mut replacement,
    )
    .await
    .unwrap();
    let result = message_decrypt(
        &message,
        &f.aa,
        &mut f.b_sessions,
        &mut f.b,
        &mut f.keys.clone_for_prekey(),
        &f.keys,
        &mut f.rng,
        UsePQRatchet::No,
    )
    .await
    .unwrap();
    assert!(result.identity_change.is_changed());
    *replacement.pair.identity_key()
}
#[test]
fn replacing_prekey_bundle_materializes_archived_sender_reservation() {
    run(async {
        let mut f = established_signal_fixture(930).await;
        message_encrypt(b"reserved output", &f.ba, &mut f.a_sessions, &mut f.a)
            .await
            .unwrap();
        let before = &f.a_sessions.0[&f.ba];
        let ceiling = before.reserved_sender_chain_index();
        let sender = before
            .current
            .as_ref()
            .unwrap()
            .inner
            .sender
            .as_ref()
            .unwrap();
        let old_ratchet = sender.ratchet;
        assert!(sender.index < ceiling);
        process_prekey_bundle(
            &f.ba,
            &mut f.a_sessions,
            &mut f.a,
            &f.bundle,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        let persisted = f.a_sessions.0[&f.ba].serialize().unwrap();
        let loaded = SessionRecord::deserialize(&persisted).unwrap();
        let archived = loaded.previous_session_states().next().unwrap().unwrap();
        let sender = archived.inner.sender.as_ref().unwrap();
        assert_eq!(sender.ratchet, old_ratchet);
        assert_eq!(sender.index, ceiling);
    })
}
#[test]
fn invalid_prekey_bundle_reservation_burn_precedes_identity_save() {
    run(async {
        let mut f = established_signal_fixture(931).await;
        let (_, _, replacement_bundle, _) = setup(&mut f.rng);
        let record = f.a_sessions.0.get_mut(&f.ba).unwrap();
        let index = record
            .session_state()
            .unwrap()
            .get_sender_chain_key()
            .unwrap()
            .index();
        record.inner.reservation.ceiling = Some(index + 2001);
        let before = record.serialize().unwrap();
        let identity_before = f.a.known[&f.ba];
        let saves = f.a.save_calls;
        assert!(matches!(
            process_prekey_bundle(
                &f.ba,
                &mut f.a_sessions,
                &mut f.a,
                &replacement_bundle,
                &mut f.rng,
                UsePQRatchet::No,
            )
            .await,
            Err(SignalProtocolError::Engine(crate::engine::Error::Encoding(
                "reservation burn exceeds limit"
            )))
        ));
        assert_eq!(f.a.save_calls, saves);
        assert_eq!(f.a.known[&f.ba], identity_before);
        assert_eq!(f.a_sessions.0[&f.ba].serialize().unwrap(), before);
    })
}
#[test]
fn prekey_bundle_identity_save_error_keeps_original_record() {
    run(async {
        let mut f = established_signal_fixture(932).await;
        let before = f.a_sessions.0[&f.ba].serialize().unwrap();
        f.a.fail_save = true;
        assert!(matches!(
            process_prekey_bundle(
                &f.ba,
                &mut f.a_sessions,
                &mut f.a,
                &f.bundle,
                &mut f.rng,
                UsePQRatchet::No,
            )
            .await,
            Err(SignalProtocolError::InvalidState("test", _))
        ));
        assert_eq!(f.a_sessions.0[&f.ba].serialize().unwrap(), before);
    })
}
#[test]
fn cancelling_prekey_bundle_identity_save_returns_original_checkout() {
    run(async {
        let mut f = established_signal_fixture(933).await;
        let before = f.a_sessions.0[&f.ba].serialize().unwrap();
        let mut sessions = checkout_store();
        *sessions.record.lock().unwrap() = f.a_sessions.0.remove(&f.ba);
        f.a.block_save = true;
        let mut future = Box::pin(process_prekey_bundle(
            &f.ba,
            &mut sessions,
            &mut f.a,
            &f.bundle,
            &mut f.rng,
            UsePQRatchet::No,
        ));
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(future);
        assert_eq!(
            sessions.queued.as_ref().unwrap().serialize().unwrap(),
            before
        );
        assert_eq!(sessions.cancelled, 0);
        sessions.complete_session_checkout().await;
        assert_eq!(
            sessions
                .record
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .serialize()
                .unwrap(),
            before
        );
    })
}
#[test]
fn ordinary_signal_decrypt_promotes_persisted_archive_without_current() {
    run(async {
        let mut f = established_signal_fixture(910).await;
        let packet = message_encrypt(b"archive only", &f.ba, &mut f.a_sessions, &mut f.a)
            .await
            .unwrap();
        let record = f.b_sessions.0.get_mut(&f.aa).unwrap();
        record.archive_current_state().unwrap();
        let encoded = record.serialize().unwrap();
        *record = SessionRecord::deserialize(&encoded).unwrap();
        assert!(record.session_state().is_none());
        f.b.known.remove(&f.aa);
        let saves = f.b.save_calls;
        let result = message_decrypt(
            &packet,
            &f.aa,
            &mut f.b_sessions,
            &mut f.b,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        assert_eq!(result.plaintext, b"archive only");
        assert_eq!(result.identity_change, IdentityChange::NewOrUnchanged);
        assert!(f.b_sessions.0[&f.aa].session_state().is_some());
        assert_eq!(f.b.known.get(&f.aa), Some(f.a.pair.identity_key()));
        assert_eq!(f.b.save_calls, saves + 1);
    })
}
async fn decrypt_receiver_entrypoint(
    entrypoint: u8,
    packet: &CiphertextMessage,
    fixture: &mut SignalFixture,
    keys: &Keys,
) -> error::Result<DecryptionResult> {
    match entrypoint {
        0 => match packet {
            CiphertextMessage::SignalMessage(message) => {
                message_decrypt_signal(
                    message,
                    &fixture.aa,
                    &mut fixture.b_sessions,
                    &mut fixture.b,
                    &mut fixture.rng,
                )
                .await
            }
            CiphertextMessage::PreKeySignalMessage(message) => {
                message_decrypt_prekey(
                    message,
                    &fixture.aa,
                    &mut fixture.b_sessions,
                    &mut fixture.b,
                    &mut keys.clone_for_prekey(),
                    keys,
                    &mut fixture.rng,
                    UsePQRatchet::No,
                )
                .await
            }
            _ => panic!("expected pairwise message"),
        },
        1 => {
            message_decrypt(
                packet,
                &fixture.aa,
                &mut fixture.b_sessions,
                &mut fixture.b,
                &mut keys.clone_for_prekey(),
                keys,
                &mut fixture.rng,
                UsePQRatchet::No,
            )
            .await
        }
        _ => {
            let mut owned = OwnedCiphertextMessage::from(packet.clone());
            let result = message_decrypt_owned(
                &mut owned,
                &fixture.aa,
                &mut fixture.b_sessions,
                &mut fixture.b,
                &mut keys.clone_for_prekey(),
                keys,
                &mut fixture.rng,
                UsePQRatchet::No,
            )
            .await;
            assert_eq!(owned.is_available(), result.is_err());
            result
        }
    }
}
#[test]
fn ordinary_archived_identity_is_selected_before_trust_and_saved_without_rotation_event() {
    run(async {
        for entrypoint in 0..3 {
            let mut f = established_signal_fixture(911 + u64::from(entrypoint)).await;
            let packet = message_encrypt(
                b"old authenticated identity",
                &f.ba,
                &mut f.a_sessions,
                &mut f.a,
            )
            .await
            .unwrap();
            assert!(matches!(packet, CiphertextMessage::SignalMessage(_)));
            let replacement = replace_sending_identity(&mut f).await;
            assert_eq!(f.b.known.get(&f.aa), Some(&replacement));
            f.b.only_trusted = Some(*f.a.pair.identity_key());
            let keys = f.keys.clone_for_prekey();
            let result = decrypt_receiver_entrypoint(entrypoint, &packet, &mut f, &keys)
                .await
                .unwrap();
            assert_eq!(result.plaintext, b"old authenticated identity");
            assert_eq!(result.identity_change, IdentityChange::NewOrUnchanged);
            assert_eq!(f.b.known.get(&f.aa), Some(f.a.pair.identity_key()));
            assert_eq!(
                f.b_sessions.0[&f.aa]
                    .session_state()
                    .unwrap()
                    .remote_identity_key()
                    .unwrap(),
                Some(*f.a.pair.identity_key())
            );
            // After recovery, a policy trusting only the persisted row must
            // permit the promoted session to send a reply.
            f.b.only_trusted = f.b.known.get(&f.aa).copied();
            let reply = message_encrypt(b"trusted reply", &f.aa, &mut f.b_sessions, &mut f.b)
                .await
                .unwrap();
            let recovered = message_decrypt(
                &reply,
                &f.ba,
                &mut f.a_sessions,
                &mut f.a,
                &mut keys.clone_for_prekey(),
                &keys,
                &mut f.rng,
                UsePQRatchet::No,
            )
            .await
            .unwrap();
            assert_eq!(recovered.plaintext, b"trusted reply");
        }
    })
}
#[test]
fn ordinary_identity_save_error_keeps_full_record_and_packet_replayable() {
    run(async {
        let mut f = established_signal_fixture(912).await;
        let packet = message_encrypt(b"save retry", &f.ba, &mut f.a_sessions, &mut f.a)
            .await
            .unwrap();
        let before = f.b_sessions.0[&f.aa].serialize().unwrap();
        f.b.fail_save = true;
        let CiphertextMessage::SignalMessage(message) = &packet else {
            panic!("expected ordinary message");
        };
        assert!(matches!(
            message_decrypt_signal(message, &f.aa, &mut f.b_sessions, &mut f.b, &mut f.rng).await,
            Err(SignalProtocolError::InvalidState("test", _))
        ));
        assert_eq!(f.b_sessions.0[&f.aa].serialize().unwrap(), before);
        f.b.fail_save = false;
        let result = message_decrypt(
            &packet,
            &f.aa,
            &mut f.b_sessions,
            &mut f.b,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        assert_eq!(result.plaintext, b"save retry");
    })
}
#[test]
fn cancelling_ordinary_identity_save_returns_original_checkout_and_packet_is_replayable() {
    run(async {
        let mut f = established_signal_fixture(913).await;
        let packet = message_encrypt(b"cancel retry", &f.ba, &mut f.a_sessions, &mut f.a)
            .await
            .unwrap();
        let before = f.b_sessions.0[&f.aa].serialize().unwrap();
        let mut sessions = checkout_store();
        *sessions.record.lock().unwrap() = f.b_sessions.0.remove(&f.aa);
        f.b.block_save = true;
        let CiphertextMessage::SignalMessage(message) = &packet else {
            panic!("expected ordinary message");
        };
        let mut future = Box::pin(message_decrypt_signal(
            message,
            &f.aa,
            &mut sessions,
            &mut f.b,
            &mut f.rng,
        ));
        assert!(
            future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(future);
        assert_eq!(
            sessions.queued.as_ref().unwrap().serialize().unwrap(),
            before
        );
        assert_eq!(sessions.cancelled, 0);
        sessions.complete_session_checkout().await;
        assert_eq!(
            sessions
                .record
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .serialize()
                .unwrap(),
            before
        );
        f.b.block_save = false;
        let result = message_decrypt_signal(message, &f.aa, &mut sessions, &mut f.b, &mut f.rng)
            .await
            .unwrap();
        assert_eq!(result.plaintext, b"cancel retry");
    })
}
#[test]
fn delayed_archived_prekey_updates_identity_row_without_fresh_rotation_or_prekey_use() {
    run(async {
        for entrypoint in 0..3 {
            let mut f = established_signal_fixture(914 + u64::from(entrypoint)).await;
            let replacement = replace_sending_identity(&mut f).await;
            assert_eq!(f.b.known.get(&f.aa), Some(&replacement));
            f.b.only_trusted = Some(*f.a.pair.identity_key());
            let mut unavailable = f.keys.clone_for_prekey();
            unavailable.prekey.inner.id = 900;
            unavailable.signed.inner.id = 900;
            let packet = f.late_prekey.clone();
            let saves = f.b.save_calls;
            let result = decrypt_receiver_entrypoint(entrypoint, &packet, &mut f, &unavailable)
                .await
                .unwrap();
            assert_eq!(result.plaintext, b"delayed prekey");
            assert_eq!(result.identity_change, IdentityChange::NewOrUnchanged);
            assert_eq!(result.consumed_prekey_id, None);
            assert_eq!(f.b.save_calls, saves + 1);
            assert_eq!(f.b.known.get(&f.aa), Some(f.a.pair.identity_key()));
        }
    })
}

#[test]
fn selected_prekey_failure_never_retries_an_older_copy_of_the_same_session() {
    run(async {
        for entrypoint in 0..3 {
            for current in [false, true] {
                for duplicate in [false, true] {
                    let mut f = established_signal_fixture(0x1828_d004).await;
                    let older = f.b_sessions.0[&f.aa]
                        .clone()
                        .into_components()
                        .unwrap()
                        .current_session
                        .unwrap();
                    let packet = f.late_prekey.clone();
                    let keys = f.keys.clone_for_prekey();
                    let mut selected = if duplicate {
                        decrypt_receiver_entrypoint(entrypoint, &packet, &mut f, &keys)
                            .await
                            .unwrap();
                        f.b_sessions.0[&f.aa]
                            .clone()
                            .into_components()
                            .unwrap()
                            .current_session
                            .unwrap()
                    } else {
                        older.clone()
                    };
                    if !duplicate {
                        selected.receiver_chains[0].chain_key.as_mut().unwrap().key =
                            Some(vec![0x73; 32]);
                    }
                    let mut components = f.b_sessions.0[&f.aa].clone().into_components().unwrap();
                    if current {
                        components.current_session = Some(selected);
                    } else {
                        components
                            .current_session
                            .as_mut()
                            .unwrap()
                            .alice_base_key
                            .as_mut()
                            .unwrap()[1] ^= 1;
                        components.previous_sessions.insert(0, selected);
                    }
                    components.previous_sessions.push(older);
                    let encoded = SessionRecord::from_components(components)
                        .unwrap()
                        .serialize()
                        .unwrap();
                    f.b_sessions
                        .0
                        .insert(f.aa.clone(), SessionRecord::deserialize(&encoded).unwrap());
                    let before = f.b_sessions.0[&f.aa].serialize().unwrap();
                    let saves = f.b.save_calls;
                    let error = decrypt_receiver_entrypoint(entrypoint, &packet, &mut f, &keys)
                        .await
                        .err()
                        .expect("selected prekey candidate must reject the packet");
                    if duplicate {
                        assert!(matches!(
                            error,
                            SignalProtocolError::DuplicatedMessage(_, _)
                        ));
                    } else {
                        assert!(matches!(error, SignalProtocolError::BadMac(_)));
                    }
                    assert_eq!(f.b_sessions.0[&f.aa].serialize().unwrap(), before);
                    assert_eq!(f.b.save_calls, saves);
                }
            }
        }
    })
}

#[test]
fn prekey_archive_decode_failure_preserves_its_error_and_checkout() {
    run(async {
        for entrypoint in 0..3 {
            let mut f = established_signal_fixture(0x1828_d005).await;
            let record = f.b_sessions.0.get_mut(&f.aa).unwrap();
            record.current.as_mut().unwrap().inner.alice_base_key = None;
            record.inner.previous = vec![crate::engine::ArchivedSession {
                bytes: Arc::from([0u8].as_slice()),
            }]
            .into();
            let before = record.serialize().unwrap();
            let saves = f.b.save_calls;
            let packet = f.late_prekey.clone();
            let keys = f.keys.clone_for_prekey();
            let error = decrypt_receiver_entrypoint(entrypoint, &packet, &mut f, &keys)
                .await
                .err()
                .expect("an invalid archive must reject the lookup");
            assert!(matches!(
                error,
                SignalProtocolError::Engine(crate::engine::Error::Encoding("invalid field number"))
            ));
            assert_eq!(f.b_sessions.0[&f.aa].serialize().unwrap(), before);
            assert_eq!(f.b.save_calls, saves);
        }
    })
}

#[test]
fn authenticated_bad_group_ciphertext_is_not_stored_state_corruption() {
    run(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(1828);
        let mut sender = Groups::default();
        let mut receiver = Groups::default();
        let name = SenderKeyName::from_parts("synthetic-group@g.us", "synthetic-sender:1");
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let before = receiver.records[&name].serialize().unwrap();
        let state = &sender.records[&name].inner.states[0];
        let mut message = crate::engine::wire::SenderKeyMessage {
            id: state.id,
            iteration: 0,
            ciphertext: vec![7],
            signature: [0; 64],
        };
        let private = PrivateKey::deserialize(&state.signing_private.unwrap()).unwrap();
        message.signature = private
            .calculate_signature(&message.unsigned(), &mut rng)
            .unwrap();
        let error = group_decrypt(&message.encode(), &mut receiver, &name)
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            SignalProtocolError::InvalidMessage(CiphertextMessageType::SenderKey, _)
        ));
        assert!(!error.is_stored_session_corruption());
        assert_eq!(receiver.records[&name].serialize().unwrap(), before);
    });
}

#[test]
fn sender_lease_waiver_consumes_reserved_keys_and_remains_in_memory() {
    futures::executor::block_on(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x1828_1ea5);
        let name = SenderKeyName::new("synthetic-group".into(), "synthetic-sender".into());
        let mut sender = Groups::default();
        let mut receiver = Groups::default();
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .unwrap();
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let record = sender.records.get_mut(&name).unwrap();
        record.reserve_iterations(0);
        let ceiling = record.reserved_iteration();
        record.waive_counter_lease().unwrap();
        assert_eq!(
            record
                .sender_key_state()
                .unwrap()
                .sender_chain_key()
                .unwrap()
                .iteration(),
            ceiling
        );
        assert!(!record.is_wire_gated());
        let message = group_encrypt(&mut sender, &name, b"after waiver", &mut rng)
            .await
            .unwrap();
        assert_eq!(message.iteration(), ceiling);
        let record = &sender.records[&name];
        assert_eq!(record.reserved_iteration(), 0);
        assert!(!record.is_wire_gated());
        assert_eq!(
            group_decrypt(message.serialized(), &mut receiver, &name)
                .await
                .unwrap(),
            b"after waiver"
        );
        let reloaded = SenderKeyRecord::deserialize(&record.serialize().unwrap()).unwrap();
        assert!(!reloaded.inner.reservation.is_waived());
        assert_eq!(
            reloaded
                .sender_key_state()
                .unwrap()
                .sender_chain_key()
                .unwrap()
                .iteration(),
            ceiling + 1
        );
    });
}

#[test]
fn sender_lease_waiver_rejects_excessive_burn_without_mutation() {
    futures::executor::block_on(async {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x1828_1ea6);
        let name = SenderKeyName::new("synthetic-group".into(), "synthetic-sender".into());
        let mut store = Groups::default();
        create_sender_key_distribution_message(&name, &mut store, &mut rng)
            .await
            .unwrap();
        let record = store.records.get_mut(&name).unwrap();
        record.reserve_iterations(2001);
        let before = record.serialize().unwrap();
        assert!(record.waive_counter_lease().is_err());
        assert_eq!(record.serialize().unwrap(), before);
        assert!(record.is_wire_gated());
    });
}

#[test]
fn fresh_ratchet_reply_survives_restart_when_following_the_host_wire_gate() {
    fn persist(record: &mut SessionRecord) -> Vec<u8> {
        let mut bytes = Vec::new();
        record.serialize_into_for_store(&mut bytes, &[0x27; 16]);
        *record = SessionRecord::deserialize_for_store(&bytes, &[0x27; 16]).unwrap();
        bytes
    }
    run(async {
        let mut f = established_signal_fixture(0x1828_d001).await;
        let mut durable_alice = persist(f.a_sessions.0.get_mut(&f.ba).unwrap());
        let a2 = message_encrypt(b"durable sender chain", &f.ba, &mut f.a_sessions, &mut f.a)
            .await
            .unwrap();
        message_decrypt(
            &a2,
            &f.aa,
            &mut f.b_sessions,
            &mut f.b,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        persist(f.b_sessions.0.get_mut(&f.aa).unwrap());
        let b2 = message_encrypt(b"peer ratchet", &f.aa, &mut f.b_sessions, &mut f.b)
            .await
            .unwrap();
        message_decrypt(
            &b2,
            &f.ba,
            &mut f.a_sessions,
            &mut f.a,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        let requires_flush = f.a_sessions.0[&f.ba].has_pending_reservation();
        if requires_flush {
            durable_alice = persist(f.a_sessions.0.get_mut(&f.ba).unwrap());
        }
        let a3 = message_encrypt(
            b"published fresh ratchet",
            &f.ba,
            &mut f.a_sessions,
            &mut f.a,
        )
        .await
        .unwrap();
        message_decrypt(
            &a3,
            &f.aa,
            &mut f.b_sessions,
            &mut f.b,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        persist(f.b_sessions.0.get_mut(&f.aa).unwrap());
        let after_restart =
            message_encrypt(b"reply after crash", &f.aa, &mut f.b_sessions, &mut f.b)
                .await
                .unwrap();
        // A different cache incarnation burns the reserved counters on restart.
        f.a_sessions.0.insert(
            f.ba.clone(),
            SessionRecord::deserialize_for_store(&durable_alice, &[0x28; 16]).unwrap(),
        );
        let clear = message_decrypt(
            &after_restart,
            &f.ba,
            &mut f.a_sessions,
            &mut f.a,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .expect("published ratchet private key must survive the crash");
        assert_eq!(clear.plaintext, b"reply after crash");
        assert!(
            requires_flush,
            "new private key requires persistence before publication"
        );
    });
}

#[test]
fn committed_fresh_private_key_is_not_regated_by_known_chain_receives() {
    run(async {
        let mut f = established_signal_fixture(0x1828_d003).await;
        let record = f.a_sessions.0.get_mut(&f.ba).unwrap();
        assert!(record.has_pending_reservation());
        assert!(record.inner.reservation.requires_durable_commit());
        let mut durable = Vec::new();
        record.serialize_into_for_store(&mut durable, &[0x29; 16]);
        // The cache acknowledges the external gate without replacing its live
        // native record. No new sender key is generated by the next receive.
        record.clear_pending_reservation();
        let sender = record
            .session_state()
            .unwrap()
            .sender_ratchet_key()
            .unwrap();
        let packet = message_encrypt(b"same peer chain", &f.aa, &mut f.b_sessions, &mut f.b)
            .await
            .unwrap();
        let result = message_decrypt(
            &packet,
            &f.ba,
            &mut f.a_sessions,
            &mut f.a,
            &mut f.keys.clone_for_prekey(),
            &f.keys,
            &mut f.rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        assert_eq!(result.plaintext, b"same peer chain");
        let record = &f.a_sessions.0[&f.ba];
        assert_eq!(
            record
                .session_state()
                .unwrap()
                .sender_ratchet_key()
                .unwrap(),
            sender
        );
        assert!(!record.has_pending_reservation());
        assert!(!durable.is_empty());
    })
}

#[test]
fn a_rebased_root_change_requires_persistence_even_if_the_public_key_repeats() {
    run(async {
        let mut f = established_signal_fixture(0x1828_d004).await;
        let record = f.a_sessions.0.get_mut(&f.ba).unwrap();
        let mut durable = Vec::new();
        record.serialize_into_for_store(&mut durable, &[0x2a; 16]);
        record.clear_pending_reservation();
        let public = record
            .session_state()
            .unwrap()
            .sender_ratchet_key()
            .unwrap();
        record
            .operate(|state| {
                state.current.as_mut().unwrap().root[0] ^= 1;
                state.reservation.rebase();
                Ok(())
            })
            .unwrap();
        assert_eq!(
            record
                .session_state()
                .unwrap()
                .sender_ratchet_key()
                .unwrap(),
            public
        );
        assert!(record.has_pending_reservation());
    })
}

#[test]
fn sending_repairs_missing_and_stale_identity_rows() {
    run(async {
        for stale in [false, true] {
            let mut f = established_signal_fixture(1201).await;
            let remote = *f.b.pair.identity_key();
            f.a.known.remove(&f.ba);
            if stale {
                f.a.known.insert(f.ba.clone(), *f.a.pair.identity_key());
            }
            f.a.only_trusted = Some(remote);
            let calls = f.a.save_calls;
            message_encrypt(b"repair identity", &f.ba, &mut f.a_sessions, &mut f.a)
                .await
                .unwrap();
            assert_eq!(f.a.save_calls, calls + 1);
            assert_eq!(f.a.known.get(&f.ba), Some(&remote));
        }
    })
}
#[test]
fn sending_identity_save_failure_and_cancellation_restore_owned_checkout() {
    run(async {
        for (cancel, serialized) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut f = established_signal_fixture(1202).await;
            let before = f.a_sessions.0[&f.ba].serialize().unwrap();
            let mut sessions = checkout_store();
            *sessions.record.lock().unwrap() = f.a_sessions.0.remove(&f.ba);
            f.a.fail_save = !cancel;
            f.a.block_save = cancel;
            let mut future = Box::pin(async {
                if serialized {
                    message_encrypt_serialized(b"identity failure", &f.ba, &mut sessions, &mut f.a)
                        .await
                        .map(|_| ())
                } else {
                    message_encrypt(b"identity failure", &f.ba, &mut sessions, &mut f.a)
                        .await
                        .map(|_| ())
                }
            });
            let result = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()));
            if cancel {
                assert!(result.is_pending());
            } else {
                assert!(matches!(
                    result,
                    Poll::Ready(Err(SignalProtocolError::InvalidState("test", _)))
                ));
            }
            drop(future);
            assert_eq!(
                sessions.queued.as_ref().unwrap().serialize().unwrap(),
                before
            );
            sessions.complete_session_checkout().await;
            assert_eq!(
                sessions
                    .record
                    .lock()
                    .unwrap()
                    .as_ref()
                    .unwrap()
                    .serialize()
                    .unwrap(),
                before
            );
        }
    })
}
#[test]
fn persisted_closed_receiver_reports_unreceived_counter_as_retryable_failure() {
    run(async {
        let mut f = established_signal_fixture(1203).await;
        let packet = message_encrypt(b"unreceived", &f.aa, &mut f.b_sessions, &mut f.b)
            .await
            .unwrap();
        let CiphertextMessage::SignalMessage(message) = &packet else {
            panic!("expected ordinary message");
        };
        let record = f.a_sessions.0.get_mut(&f.ba).unwrap();
        for chain in &mut record.session_state_mut().unwrap().inner.receivers {
            chain.exhausted = true;
            chain.key = [0; 32];
        }
        let bytes = record.serialize().unwrap();
        *record = SessionRecord::deserialize(&bytes).unwrap();
        // Loading consumes the sender reservation before the receive attempt.
        let bytes = record.serialize().unwrap();
        let result =
            message_decrypt_signal(message, &f.ba, &mut f.a_sessions, &mut f.a, &mut f.rng).await;
        let Err(error) = result else {
            panic!("closed receiver accepted a new counter")
        };
        assert!(
            matches!(&error, SignalProtocolError::InvalidSessionStructure(reason) if reason == "receiver chain is closed"),
            "{error}"
        );
        assert!(!error.is_stored_session_corruption());
        assert_eq!(f.a_sessions.0[&f.ba].serialize().unwrap(), bytes);
    })
}

#[test]
fn readding_known_sender_state_preserves_consumed_and_delayed_messages() {
    run(async {
        for advertised in [0, 100] {
            let mut random = rand::rngs::StdRng::seed_from_u64(1204);
            let mut sender = Groups::default();
            let mut receiver = Groups::default();
            let name = SenderKeyName::from_parts("synthetic.invalid@g.us", "synthetic.invalid:1");
            let distribution =
                create_sender_key_distribution_message(&name, &mut sender, &mut random)
                    .await
                    .unwrap();
            process_sender_key_distribution_message(&name, &distribution, &mut receiver)
                .await
                .unwrap();
            let delayed = group_encrypt(&mut sender, &name, b"delayed", &mut random)
                .await
                .unwrap();
            let consumed = group_encrypt(&mut sender, &name, b"consumed", &mut random)
                .await
                .unwrap();
            assert_eq!(
                group_decrypt_shared(
                    bytes::Bytes::copy_from_slice(consumed.serialized()),
                    &mut receiver,
                    &name
                )
                .await
                .unwrap(),
                b"consumed"
            );
            receiver
                .records
                .get_mut(&name)
                .unwrap()
                .add_sender_key_state(
                    3,
                    distribution.id(),
                    advertised,
                    distribution.chain_key(),
                    *distribution.signing_key(),
                    None,
                )
                .unwrap();
            assert!(matches!(
                group_decrypt_shared(
                    bytes::Bytes::copy_from_slice(consumed.serialized()),
                    &mut receiver,
                    &name
                )
                .await,
                Err(SignalProtocolError::DuplicatedMessage(..))
            ));
            assert_eq!(
                group_decrypt_shared(
                    bytes::Bytes::copy_from_slice(delayed.serialized()),
                    &mut receiver,
                    &name
                )
                .await
                .unwrap(),
                b"delayed"
            );
        }
    })
}
#[test]
fn readding_own_sender_state_preserves_private_key_and_reservation() {
    run(async {
        let mut random = rand::rngs::StdRng::seed_from_u64(1205);
        let mut sender = Groups::default();
        let name = SenderKeyName::from_parts("synthetic.invalid@g.us", "synthetic.invalid:1");
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut random)
            .await
            .unwrap();
        let record = sender.records.get_mut(&name).unwrap();
        let before = record.serialize().unwrap();
        let gated = record.is_wire_gated();
        record
            .add_sender_key_state(
                3,
                distribution.id(),
                0,
                distribution.chain_key(),
                *distribution.signing_key(),
                None,
            )
            .unwrap();
        assert_eq!(record.serialize().unwrap(), before);
        assert_eq!(record.is_wire_gated(), gated);
        assert!(matches!(
            record.add_sender_key_state(
                3,
                distribution.id(),
                0,
                &[1; 31],
                *distribution.signing_key(),
                None
            ),
            Err(SignalProtocolError::InvalidSenderKeySession)
        ));
        assert_eq!(record.serialize().unwrap(), before);
        group_encrypt(&mut sender, &name, b"still owns sender key", &mut random)
            .await
            .unwrap();
    })
}

#[test]
fn activating_another_own_sender_state_rebinds_reservation_before_publication() {
    run(async {
        let mut random = rand::rngs::StdRng::seed_from_u64(1206);
        let mut sender = Groups::default();
        let name = SenderKeyName::from_parts("synthetic.invalid@g.us", "synthetic.invalid:1");
        let original = create_sender_key_distribution_message(&name, &mut sender, &mut random)
            .await
            .unwrap();
        group_encrypt(&mut sender, &name, b"original zero", &mut random)
            .await
            .unwrap();
        let other = KeyPair::generate(&mut random);
        let record = sender.records.get_mut(&name).unwrap();
        record.clear_wire_gated();
        record
            .add_sender_key_state(
                3,
                original.id() ^ 1,
                0,
                &[99; 32],
                other.public_key,
                Some(other.private_key),
            )
            .unwrap();
        assert_eq!(record.reserved_iteration(), 0);
        assert!(record.is_wire_gated());
        assert_eq!(
            group_encrypt(&mut sender, &name, b"other zero", &mut random)
                .await
                .unwrap()
                .iteration(),
            0
        );
        let record = sender.records.get_mut(&name).unwrap();
        record.clear_wire_gated();
        record
            .add_sender_key_state(
                3,
                original.id(),
                0,
                original.chain_key(),
                *original.signing_key(),
                None,
            )
            .unwrap();
        assert_eq!(record.reserved_iteration(), 0);
        assert!(record.is_wire_gated());
        assert_eq!(
            group_encrypt(&mut sender, &name, b"original one", &mut random)
                .await
                .unwrap()
                .iteration(),
            1
        );
        let reloaded =
            SenderKeyRecord::deserialize(&sender.records[&name].serialize().unwrap()).unwrap();
        assert_eq!(reloaded.inner.states[0].iteration, 65);
    })
}
