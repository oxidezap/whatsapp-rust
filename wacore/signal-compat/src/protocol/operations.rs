// SPDX-License-Identifier: MIT
use super::error::{Result, message_error};
use super::*;
use wacore_signal_engine::{self as engine, RandomSource};
pub(crate) struct RandomAdapter<'a, R>(pub &'a mut R);
impl<R: rand::Rng + rand::CryptoRng> RandomSource for RandomAdapter<'_, R> {
    fn fill(&mut self, out: &mut [u8]) -> engine::Result<()> {
        self.0.fill_bytes(out);
        Ok(())
    }
}
struct NoRandomSource;
impl RandomSource for NoRandomSource {
    fn fill(&mut self, _out: &mut [u8]) -> engine::Result<()> {
        Err(engine::Error::Crypto)
    }
}
pub struct DecryptionResult {
    pub plaintext: Vec<u8>,
    pub consumed_prekey_id: Option<PreKeyId>,
    pub identity_change: IdentityChange,
}
pub async fn process_prekey_bundle<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    bundle: &PreKeyBundle,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<IdentityChange> {
    if pq == UsePQRatchet::Yes {
        return Err(SignalProtocolError::InvalidArgument(
            "post-quantum ratchet unsupported".into(),
        ));
    }
    if !is_trusted_identity(identities, address, &bundle.identity, Direction::Sending).await? {
        return Err(SignalProtocolError::UntrustedIdentity(address.clone()));
    }
    let local = identities.get_identity_key_pair().await?;
    let registration = identities.get_local_registration_id().await?;
    let base = KeyPair::generate(rng);
    let parameters = engine::InitiatorParameters {
        local_identity: identity_pair(&local),
        local_registration: registration,
        remote_identity: *bundle.identity.public_key().public_key_bytes(),
        remote_registration: bundle.registration,
        remote_signed_pre_key: *bundle.signed_public.public_key_bytes(),
        remote_signed_pre_key_id: bundle.signed_id.0,
        remote_signed_pre_key_signature: bundle
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| SignalProtocolError::SignatureValidationFailed)?,
        remote_one_time_pre_key: bundle
            .pre_key
            .as_ref()
            .map(|(id, key)| (id.0, *key.public_key_bytes())),
        base_key: raw_pair(&base),
    };
    let mut random = RandomAdapter(rng);
    let state = engine::Engine::new(&engine::RustCrypto, &mut random)
        .initialize_initiator(&parameters)
        .map_err(|error| {
            if error == engine::Error::Authentication {
                SignalProtocolError::SignatureValidationFailed
            } else {
                error.into()
            }
        })?;
    let mut checkout = SessionCheckout::load_or_fresh(sessions, address).await?;
    let change = save_identity(identities, address, &bundle.identity).await?;
    checkout.record_mut().operate(|record| {
        record.install(state);
        Ok(())
    })?;
    checkout.commit().await?;
    Ok(change)
}
pub async fn message_encrypt<S: SessionStore + ?Sized, I: IdentityKeyStore + ?Sized>(
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
) -> Result<CiphertextMessage> {
    let mut checkout = SessionCheckout::load(sessions, address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?;
    let remote = checkout
        .record()
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?
        .remote_identity_key()?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?;
    if !is_trusted_identity(identities, address, &remote, Direction::Sending).await? {
        return Err(SignalProtocolError::UntrustedIdentity(address.clone()));
    }
    let record = checkout.record_mut();
    let index = record
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?
        .get_sender_chain_key()?
        .index();
    if index >= record.reserved_sender_chain_index() {
        record.reserve_sender_chain_counters(index);
    }
    // Authorizes preparation only. The record's pending flag transfers to the
    // host cache wire gate, which must flush successfully before publication.
    record.inner.reservation.mark_committed();
    let prekey = record
        .session_state()
        .is_some_and(|state| state.inner.pending_pre_key.is_some());
    let bytes = record.operate(|record| {
        engine::Engine::new(&engine::RustCrypto, &mut NoRandomSource).encrypt(record, plaintext)
    })?;
    let message = if prekey {
        CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::try_from(bytes.as_slice())?)
    } else {
        CiphertextMessage::SignalMessage(SignalMessage::try_from(bytes.as_slice())?)
    };
    checkout.commit().await?;
    Ok(message)
}
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    message: &CiphertextMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    if pq == UsePQRatchet::Yes {
        return Err(SignalProtocolError::InvalidArgument(
            "post-quantum ratchet unsupported".into(),
        ));
    }
    let mut checkout = SessionCheckout::load_or_fresh(sessions, address).await?;
    let mut consumed = None;
    let mut change = IdentityChange::NewOrUnchanged;
    let mut random = RandomAdapter(rng);
    let mut engine = engine::Engine::new(&engine::RustCrypto, &mut random);
    let plaintext = match message {
        CiphertextMessage::SignalMessage(message) => {
            let remote = checkout
                .record()
                .session_state()
                .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?
                .remote_identity_key()?
                .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?;
            if !is_trusted_identity(identities, address, &remote, Direction::Receiving).await? {
                return Err(SignalProtocolError::UntrustedIdentity(address.clone()));
            }
            checkout
                .record_mut()
                .operate(|record| engine.decrypt(record, &message.bytes))
                .map_err(|error| {
                    message_error(error, CiphertextMessageType::Whisper, 0, message.counter())
                })?
        }
        CiphertextMessage::PreKeySignalMessage(message) => {
            if !is_trusted_identity(
                identities,
                address,
                message.identity_key(),
                Direction::Receiving,
            )
            .await?
            {
                return Err(SignalProtocolError::UntrustedIdentity(address.clone()));
            }
            let base = message.base_key();
            let mut candidate = checkout.record().clone();
            let existing = candidate.has_session_state(3, &base)?;
            let plaintext = if existing {
                candidate
                    .operate(|record| engine.decrypt(record, message.message().serialized()))
                    .map_err(|error| message_error(error, CiphertextMessageType::PreKey, 0, 0))?
            } else {
                let local = identities.get_identity_key_pair().await?;
                let registration = identities.get_local_registration_id().await?;
                let signed = signed
                    .get_signed_pre_key(message.signed_pre_key_id())
                    .await?;
                let otp = if let Some(id) = message.pre_key_id() {
                    Some(prekeys.get_pre_key(id).await?)
                } else {
                    None
                };
                let parameters = engine::ResponderParameters {
                    local_identity: identity_pair(&local),
                    local_registration: registration,
                    local_signed_pre_key: signed.inner.key_pair.clone(),
                    local_one_time_pre_key: otp.as_ref().map(|key| key.inner.key_pair.clone()),
                    remote_identity: message.parsed.identity,
                    remote_base_key: message.parsed.base_key,
                    remote_registration: message.parsed.registration,
                };
                // Identity trust was checked above. The host permits verified
                // rotations, so archive a different current identity first.
                if candidate
                    .session_state()
                    .is_some_and(|state| state.inner.remote_identity != message.parsed.identity)
                {
                    candidate.archive_current_state()?;
                }
                let plaintext = candidate
                    .operate(|record| engine.decrypt_pre_key(record, &message.bytes, &parameters))
                    .map_err(|error| message_error(error, CiphertextMessageType::PreKey, 0, 0))?;
                consumed = message.pre_key_id();
                plaintext
            };
            change = save_identity(identities, address, message.identity_key()).await?;
            *checkout.record_mut() = candidate;
            plaintext
        }
        CiphertextMessage::SenderKeyMessage(_) => {
            return Err(SignalProtocolError::InvalidArgument(
                "sender-key message requires group decrypt".into(),
            ));
        }
    };
    checkout.commit().await?;
    Ok(DecryptionResult {
        plaintext,
        consumed_prekey_id: consumed,
        identity_change: change,
    })
}
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_owned<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    message: &mut OwnedCiphertextMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    let retained = message.message.as_ref().ok_or_else(|| {
        SignalProtocolError::InvalidState("decrypt", "ciphertext consumed".into())
    })?;
    let result = message_decrypt(
        retained, address, sessions, identities, prekeys, signed, rng, pq,
    )
    .await;
    if result.is_ok() {
        message.message = None;
    }
    result
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn create_sender_key_distribution_message<
    S: SenderKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    name: &SenderKeyName,
    store: &mut S,
    rng: &mut R,
) -> Result<SenderKeyDistributionMessage> {
    let mut record = store
        .load_sender_key(name)
        .await?
        .unwrap_or_else(SenderKeyRecord::new_empty);
    let already_sending = record
        .inner
        .states
        .first()
        .is_some_and(|state| state.signing_private.is_some());
    let mut random = RandomAdapter(rng);
    let bytes = engine::Engine::new(&engine::RustCrypto, &mut random)
        .create_sender_distribution(&mut record.inner)?;
    if !already_sending {
        record.mark_wire_gated();
    }
    store.store_sender_key(name, record).await?;
    SenderKeyDistributionMessage::try_from(bytes.as_slice())
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn process_sender_key_distribution_message<S: SenderKeyStore + ?Sized>(
    name: &SenderKeyName,
    message: &SenderKeyDistributionMessage,
    store: &mut S,
) -> Result<()> {
    let mut record = store
        .load_sender_key(name)
        .await?
        .unwrap_or_else(SenderKeyRecord::new_empty);
    engine::Engine::new(&engine::RustCrypto, &mut NoRandomSource)
        .process_sender_distribution(&mut record.inner, &message.bytes)?;
    store.store_sender_key(name, record).await
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn group_encrypt<S: SenderKeyStore + ?Sized, R: rand::Rng + rand::CryptoRng>(
    store: &mut S,
    name: &SenderKeyName,
    plaintext: &[u8],
    rng: &mut R,
) -> Result<SenderKeyMessage> {
    let mut record = store
        .load_sender_key(name)
        .await?
        .ok_or_else(|| SignalProtocolError::NoSenderKeyState(name.to_string()))?;
    let iteration = record
        .inner
        .states
        .first()
        .ok_or(SignalProtocolError::InvalidSenderKeySession)?
        .iteration;
    if record
        .inner
        .reservation
        .ceiling
        .is_none_or(|ceiling| iteration >= ceiling)
    {
        let ceiling = iteration
            .checked_add(64)
            .ok_or(engine::Error::CounterExhausted)?;
        record.inner.reservation.ceiling = Some(ceiling);
        record.inner.reservation.incarnation = None;
        record.mark_wire_gated();
    }
    record.inner.reservation.mark_committed();
    let mut random = RandomAdapter(rng);
    let bytes = engine::Engine::new(&engine::RustCrypto, &mut random)
        .encrypt_sender_key(&mut record.inner, plaintext)?;
    store.store_sender_key(name, record).await?;
    SenderKeyMessage::try_from(bytes.as_slice())
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn group_decrypt_shared<S: SenderKeyStore + ?Sized>(
    ciphertext: bytes::Bytes,
    store: &mut S,
    name: &SenderKeyName,
) -> Result<Vec<u8>> {
    let message = SenderKeyMessage::try_from(ciphertext)?;
    let mut record = store
        .load_sender_key(name)
        .await?
        .ok_or_else(|| SignalProtocolError::NoSenderKeyState(name.to_string()))?;
    let plaintext = engine::Engine::new(&engine::RustCrypto, &mut NoRandomSource)
        .decrypt_sender_key(&mut record.inner, &message.bytes)
        .map_err(|error| match error {
            engine::Error::Authentication => SignalProtocolError::SignatureValidationFailed,
            engine::Error::MissingKey => SignalProtocolError::NoSenderKeyState(name.to_string()),
            engine::Error::Crypto => SignalProtocolError::InvalidSenderKeySession,
            error => message_error(
                error,
                CiphertextMessageType::SenderKey,
                0,
                message.iteration(),
            ),
        })?;
    store.store_sender_key(name, record).await?;
    Ok(plaintext)
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn group_decrypt<S: SenderKeyStore + ?Sized>(
    ciphertext: &[u8],
    store: &mut S,
    name: &SenderKeyName,
) -> Result<Vec<u8>> {
    group_decrypt_shared(bytes::Bytes::copy_from_slice(ciphertext), store, name).await
}

pub async fn message_decrypt_signal<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    message: &SignalMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    rng: &mut R,
) -> Result<DecryptionResult> {
    let mut checkout = SessionCheckout::load(sessions, address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?;
    let remote = checkout
        .record()
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?
        .remote_identity_key()?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone()))?;
    if !is_trusted_identity(identities, address, &remote, Direction::Receiving).await? {
        return Err(SignalProtocolError::UntrustedIdentity(address.clone()));
    }
    let mut random = RandomAdapter(rng);
    let plaintext = checkout
        .record_mut()
        .operate(|record| {
            engine::Engine::new(&engine::RustCrypto, &mut random).decrypt(record, &message.bytes)
        })
        .map_err(|error| {
            message_error(error, CiphertextMessageType::Whisper, 0, message.counter())
        })?;
    checkout.commit().await?;
    Ok(DecryptionResult {
        plaintext,
        consumed_prekey_id: None,
        identity_change: IdentityChange::NewOrUnchanged,
    })
}
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_prekey<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    message: &PreKeySignalMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    message_decrypt(
        &CiphertextMessage::PreKeySignalMessage(message.clone()),
        address,
        sessions,
        identities,
        prekeys,
        signed,
        rng,
        pq,
    )
    .await
}
