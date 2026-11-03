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
        if self.fail_save {
            return Err(SignalProtocolError::InvalidState(
                "test",
                "save failed".into(),
            ));
        }
        let old = self.known.insert(address.clone(), *identity);
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
        _: &IdentityKey,
        _: Direction,
    ) -> error::Result<bool> {
        Ok(true)
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
    };
    let b = Identity {
        pair: IdentityKeyPair::generate(rng),
        known: HashMap::new(),
        fail_save: false,
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
