// SPDX-License-Identifier: MIT
use super::*;
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
#[derive(Default)]
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
fn signed_key_helper_stores_seconds_and_rejects_pre_epoch() {
    let mut rng = rand::rngs::StdRng::seed_from_u64(2);
    let key = KeyPair::generate(&mut rng);
    let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 123_000_000).unwrap();
    let record =
        crate::store::record_helpers::new_signed_pre_key_record(4, &key, [5; 64], timestamp)
            .unwrap();
    assert_eq!(record.timestamp, Some(1_700_000_000));
    let negative = chrono::DateTime::from_timestamp(-1, 0).unwrap();
    assert!(
        crate::store::record_helpers::new_signed_pre_key_record(4, &key, [5; 64], negative)
            .is_err()
    );
}

#[derive(Default)]
struct Groups {
    records: HashMap<SenderKeyName, SenderKeyRecord>,
    lock: Arc<async_lock::Mutex<()>>,
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
        self.records.insert(name.clone(), record);
        Ok(())
    }
    async fn sender_key_lock(&self, _: &SenderKeyName) -> Arc<async_lock::Mutex<()>> {
        self.lock.clone()
    }
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
            Err(SignalProtocolError::Engine(
                wacore_signal_engine::Error::Encoding("reservation burn exceeds limit")
            ))
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
        let mut message = wacore_signal_engine::wire::SenderKeyMessage {
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
