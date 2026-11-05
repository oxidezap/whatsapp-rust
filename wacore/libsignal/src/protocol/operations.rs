// SPDX-License-Identifier: MIT
use super::error::{Result, message_error};
use super::records::SessionMatch;
use super::*;
use crate::engine::{self as engine, RandomSource};
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
// Shared by ordinary and prekey receives, independently of the crypto provider.
// Keep this storage transition out of their otherwise specialized state machines.
#[inline(never)]
async fn commit_received_session<S: SessionStore + ?Sized, I: IdentityKeyStore + ?Sized>(
    mut checkout: SessionCheckout<'_, S>,
    candidate: SessionRecord,
    address: &ProtocolAddress,
    identities: &mut I,
    consumed_prekey: Option<PreKeyId>,
) -> Result<IdentityChange> {
    // Authentication selects the session, including an archive-only record.
    // Its identity governs trust and the identity row update.
    let remote = candidate
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?
        .remote_identity_key()?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?;
    if !is_trusted_identity(identities, address, &remote, Direction::Receiving).await? {
        return Err(SignalProtocolError::UntrustedIdentity(
            address.clone().into(),
        ));
    }
    let change = save_identity(identities, address, &remote).await?;
    // The original session remains recoverable until the identity step succeeds.
    *checkout.record_mut() = candidate;
    checkout.commit_received(consumed_prekey).await?;
    Ok(change)
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
        return Err(SignalProtocolError::UntrustedIdentity(
            address.clone().into(),
        ));
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
    let mut candidate = checkout.record().clone();
    candidate.operate(|record| {
        record.materialize_reservation(&engine::RustCrypto)?;
        record.install(state);
        Ok(())
    })?;
    let change = save_identity(identities, address, &bundle.identity).await?;
    *checkout.record_mut() = candidate;
    checkout.commit().await?;
    Ok(change)
}
pub async fn message_encrypt<S: SessionStore + ?Sized, I: IdentityKeyStore + ?Sized>(
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
) -> Result<CiphertextMessage> {
    message_encrypt_using(
        engine::RustCrypto,
        plaintext,
        address,
        sessions,
        identities,
        parse_encrypted_message,
    )
    .await
}
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn message_encrypt_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
) -> Result<CiphertextMessage> {
    message_encrypt_using(
        crate::crypto::ProviderCrypto::new(provider),
        plaintext,
        address,
        sessions,
        identities,
        parse_encrypted_message,
    )
    .await
}
fn parse_encrypted_message(bytes: Vec<u8>, prekey: bool) -> Result<CiphertextMessage> {
    let bytes = bytes::Bytes::from(bytes);
    if prekey {
        Ok(CiphertextMessage::PreKeySignalMessage(
            PreKeySignalMessage::try_from(bytes)?,
        ))
    } else {
        Ok(CiphertextMessage::SignalMessage(SignalMessage::try_from(
            bytes,
        )?))
    }
}
/// Encrypt directly into owned wire bytes, preserving checkout and durability semantics.
pub async fn message_encrypt_serialized<S: SessionStore + ?Sized, I: IdentityKeyStore + ?Sized>(
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
) -> Result<SerializedCiphertext> {
    message_encrypt_using(
        engine::RustCrypto,
        plaintext,
        address,
        sessions,
        identities,
        SerializedCiphertext::from_pairwise,
    )
    .await
}
/// Encrypt owned wire bytes with an explicit symmetric crypto provider.
pub async fn message_encrypt_serialized_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
) -> Result<SerializedCiphertext> {
    message_encrypt_using(
        crate::crypto::ProviderCrypto::new(provider),
        plaintext,
        address,
        sessions,
        identities,
        SerializedCiphertext::from_pairwise,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
async fn message_encrypt_using<
    C: engine::CryptoProvider,
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    T,
>(
    crypto: C,
    plaintext: &[u8],
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    finish: impl FnOnce(Vec<u8>, bool) -> Result<T>,
) -> Result<T> {
    let mut checkout = SessionCheckout::load(sessions, address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?;
    let remote = checkout
        .record()
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?
        .remote_identity_key()?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?;
    if !is_trusted_identity(identities, address, &remote, Direction::Sending).await? {
        return Err(SignalProtocolError::UntrustedIdentity(
            address.clone().into(),
        ));
    }
    // Identity callbacks must succeed before advancing the owned sender state.
    // A failed or cancelled save can then return the unchanged checkout.
    save_identity(identities, address, &remote).await?;
    let record = checkout.record_mut();
    let index = record
        .session_state()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?
        .get_sender_chain_key()?
        .index();
    let previous_reservation = record.inner.reservation.clone();
    let previous_pending_reservation = record.has_pending_reservation();
    if index >= record.reserved_sender_chain_index() {
        record.reserve_sender_chain_counters(index);
    }
    // Authorizes preparation only. The record's pending flag transfers to the
    // host cache wire gate, which must flush successfully before publication.
    record.inner.reservation.mark_committed();
    let prekey = record
        .session_state()
        .is_some_and(|state| state.inner.pending_pre_key.is_some());
    let bytes = match record.operate(|record| {
        engine::Engine::new(&crypto, &mut NoRandomSource).encrypt(record, plaintext)
    }) {
        Ok(bytes) => bytes,
        Err(error) => {
            record.inner.reservation = previous_reservation;
            if !previous_pending_reservation {
                record.clear_pending_reservation();
            }
            return Err(error.into());
        }
    };
    let message = finish(bytes, prekey)?;
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
    message_decrypt_using(
        engine::RustCrypto,
        message,
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
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    message: &CiphertextMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    message_decrypt_using(
        crate::crypto::ProviderCrypto::new(provider),
        message,
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
#[allow(clippy::too_many_arguments)]
async fn message_decrypt_using<
    C: engine::CryptoProvider,
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    crypto: C,
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
    match message {
        CiphertextMessage::SignalMessage(message) => {
            message_decrypt_signal_using(crypto, message, address, sessions, identities, rng).await
        }
        CiphertextMessage::PreKeySignalMessage(message) => {
            message_decrypt_prekey_using(
                crypto, message, address, sessions, identities, prekeys, signed, rng, pq,
            )
            .await
        }
        CiphertextMessage::SenderKeyMessage(_) => Err(SignalProtocolError::InvalidArgument(
            "sender-key message requires group decrypt".into(),
        )),
    }
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
    message_decrypt_owned_using(
        engine::RustCrypto,
        message,
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
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_owned_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    message: &mut OwnedCiphertextMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    message_decrypt_owned_using(
        crate::crypto::ProviderCrypto::new(provider),
        message,
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
#[allow(clippy::too_many_arguments)]
async fn message_decrypt_owned_using<
    C: engine::CryptoProvider,
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    crypto: C,
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
    let result = message_decrypt_using(
        crypto, retained, address, sessions, identities, prekeys, signed, rng, pq,
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
    // Re-exporting an existing sending chain does not change protocol state.
    // Keep fresh/gated records on the persistence path, including retries after
    // a failed durability gate. Cold signing preparation can wait for encryption.
    if !already_sending || record.is_wire_gated() {
        record.warm_current_signer();
        if !already_sending {
            record.mark_wire_gated();
        }
        store.store_sender_key(name, record).await?;
    }
    SenderKeyDistributionMessage::try_from(bytes::Bytes::from(bytes))
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
    record.warm_current_verifier();
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
    group_encrypt_using(
        std::convert::identity,
        store,
        name,
        plaintext,
        rng,
        parse_sender_key_message,
    )
    .await
}
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn group_encrypt_with_provider<
    S: SenderKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    store: &mut S,
    name: &SenderKeyName,
    plaintext: &[u8],
    rng: &mut R,
) -> Result<SenderKeyMessage> {
    group_encrypt_using(
        |crypto| crate::crypto::ProviderCrypto::with_crypto(crypto, provider),
        store,
        name,
        plaintext,
        rng,
        parse_sender_key_message,
    )
    .await
}
fn parse_sender_key_message(bytes: Vec<u8>) -> Result<SenderKeyMessage> {
    SenderKeyMessage::try_from(bytes::Bytes::from(bytes))
}
/// Encrypt group output into owned wire bytes.
/// The caller must hold the store's stable sender-key lock across this operation
/// and any related distribution, as with [`group_encrypt`].
pub async fn group_encrypt_serialized<
    S: SenderKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    store: &mut S,
    name: &SenderKeyName,
    plaintext: &[u8],
    rng: &mut R,
) -> Result<SerializedCiphertext> {
    group_encrypt_using(
        std::convert::identity,
        store,
        name,
        plaintext,
        rng,
        SerializedCiphertext::from_sender_key,
    )
    .await
}
/// Encrypt owned group wire bytes with a caller-owned symmetric crypto backend.
/// The locking contract is the same as [`group_encrypt_serialized`].
pub async fn group_encrypt_serialized_with_provider<
    S: SenderKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    store: &mut S,
    name: &SenderKeyName,
    plaintext: &[u8],
    rng: &mut R,
) -> Result<SerializedCiphertext> {
    group_encrypt_using(
        |crypto| crate::crypto::ProviderCrypto::with_crypto(crypto, provider),
        store,
        name,
        plaintext,
        rng,
        SerializedCiphertext::from_sender_key,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
async fn group_encrypt_using<
    C: engine::CryptoProvider,
    F: FnOnce(group_crypto::PreparedGroupProvider) -> C,
    S: SenderKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
    T,
>(
    prepare: F,
    store: &mut S,
    name: &SenderKeyName,
    plaintext: &[u8],
    rng: &mut R,
    output: impl FnOnce(Vec<u8>) -> Result<T>,
) -> Result<T> {
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
    if !record.inner.reservation.is_waived()
        && record
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
    let bytes = {
        let mut random = RandomAdapter(rng);
        let crypto = prepare(record.prepared_signer(0)?);
        engine::Engine::new(&crypto, &mut random)
            .encrypt_sender_key(&mut record.inner, plaintext)?
    };
    let message = output(bytes)?;
    store.store_sender_key(name, record).await?;
    Ok(message)
}
/// The caller must hold the store's stable sender-key lock across this operation,
/// including a related distribution and encryption when they share a chain.
pub async fn group_decrypt_shared<S: SenderKeyStore + ?Sized>(
    ciphertext: bytes::Bytes,
    store: &mut S,
    name: &SenderKeyName,
) -> Result<Vec<u8>> {
    group_decrypt_shared_using(std::convert::identity, ciphertext, store, name).await
}
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn group_decrypt_shared_with_provider<S: SenderKeyStore + ?Sized>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    ciphertext: bytes::Bytes,
    store: &mut S,
    name: &SenderKeyName,
) -> Result<Vec<u8>> {
    group_decrypt_shared_using(
        |crypto| crate::crypto::ProviderCrypto::with_crypto(crypto, provider),
        ciphertext,
        store,
        name,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
async fn group_decrypt_shared_using<
    C: engine::CryptoProvider,
    F: FnOnce(group_crypto::PreparedGroupProvider) -> C,
    S: SenderKeyStore + ?Sized,
>(
    prepare: F,
    ciphertext: bytes::Bytes,
    store: &mut S,
    name: &SenderKeyName,
) -> Result<Vec<u8>> {
    let message = SenderKeyMessage::try_from(ciphertext)?;
    let mut record = store
        .load_sender_key(name)
        .await?
        .ok_or_else(|| SignalProtocolError::NoSenderKeyState(name.to_string()))?;
    let index = record
        .inner
        .states
        .iter()
        .position(|state| state.id == message.chain_id())
        .ok_or_else(|| SignalProtocolError::NoSenderKeyState(name.to_string()))?;
    let plaintext = {
        let crypto = prepare(record.prepared_verifier(index)?);
        engine::Engine::new(&crypto, &mut NoRandomSource)
            .decrypt_sender_key(&mut record.inner, &message.bytes)
            .map_err(|error| match error {
                engine::Error::Authentication => SignalProtocolError::SignatureValidationFailed,
                engine::Error::MissingKey => {
                    SignalProtocolError::NoSenderKeyState(name.to_string())
                }
                error => message_error(
                    error,
                    CiphertextMessageType::SenderKey,
                    0,
                    message.iteration(),
                ),
            })?
    };
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
    message_decrypt_signal_using(
        engine::RustCrypto,
        message,
        address,
        sessions,
        identities,
        rng,
    )
    .await
}
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_signal_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    message: &SignalMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    rng: &mut R,
) -> Result<DecryptionResult> {
    message_decrypt_signal_using(
        crate::crypto::ProviderCrypto::new(provider),
        message,
        address,
        sessions,
        identities,
        rng,
    )
    .await
}
#[allow(clippy::too_many_arguments)]
async fn message_decrypt_signal_using<
    C: engine::CryptoProvider,
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    crypto: C,
    message: &SignalMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    rng: &mut R,
) -> Result<DecryptionResult> {
    let checkout = SessionCheckout::load(sessions, address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(address.clone().into()))?;
    let mut candidate = checkout.record().clone();
    if candidate.session_state().is_none() && candidate.previous_session_count() == 0 {
        return Err(SignalProtocolError::SessionNotFound(address.clone().into()));
    }
    let mut random = RandomAdapter(rng);
    let plaintext = candidate
        .operate(|record| engine::Engine::new(&crypto, &mut random).decrypt(record, &message.bytes))
        .map_err(|error| {
            message_error(error, CiphertextMessageType::Whisper, 0, message.counter())
        })?;
    commit_received_session(checkout, candidate, address, identities, None).await?;
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
    message_decrypt_prekey_using(
        engine::RustCrypto,
        message,
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
/// Run this operation with a caller-owned symmetric crypto backend.
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_prekey_with_provider<
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    provider: &dyn crate::crypto::SymmetricCryptoProvider,
    message: &PreKeySignalMessage,
    address: &ProtocolAddress,
    sessions: &mut S,
    identities: &mut I,
    prekeys: &mut P,
    signed: &K,
    rng: &mut R,
    pq: UsePQRatchet,
) -> Result<DecryptionResult> {
    message_decrypt_prekey_using(
        crate::crypto::ProviderCrypto::new(provider),
        message,
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
// Keep the decoded archive local to a synchronous operation, rather than
// retaining its full state across identity/prekey storage awaits.
fn decrypt_existing_prekey<C: engine::CryptoProvider, R: RandomSource>(
    candidate: &mut SessionRecord,
    engine: &mut engine::Engine<'_, C, R>,
    message: &PreKeySignalMessage,
) -> Result<Option<Vec<u8>>> {
    let version = u32::from(message.message_version());
    let Some(selected) = candidate.matching_session(version, message.base_key())? else {
        return Ok(None);
    };
    candidate
        .operate(|record| match selected {
            SessionMatch::Current => engine.decrypt_existing_pre_key(
                record,
                message.message().serialized(),
                version,
                message.parsed.base_key,
            ),
            SessionMatch::Archived { index, state } => engine.decrypt_archived_pre_key(
                record,
                message.message().serialized(),
                index,
                state,
            ),
        })
        .map(Some)
        .map_err(|error| {
            message_error(
                error,
                CiphertextMessageType::PreKey,
                0,
                message.message().counter(),
            )
        })
}

#[allow(clippy::too_many_arguments)]
async fn message_decrypt_prekey_using<
    C: engine::CryptoProvider,
    S: SessionStore + ?Sized,
    I: IdentityKeyStore + ?Sized,
    P: PreKeyStore + ?Sized,
    K: SignedPreKeyStore + ?Sized,
    R: rand::Rng + rand::CryptoRng,
>(
    crypto: C,
    message: &PreKeySignalMessage,
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
    let checkout = SessionCheckout::load_or_fresh(sessions, address).await?;
    let mut candidate = checkout.record().clone();
    let mut random = RandomAdapter(rng);
    let mut engine = engine::Engine::new(&crypto, &mut random);
    let decrypted = decrypt_existing_prekey(&mut candidate, &mut engine, message)?;
    let existing = decrypted.is_some();
    let plaintext = if let Some(plaintext) = decrypted {
        plaintext
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
        if candidate
            .session_state()
            .is_some_and(|state| state.inner.remote_identity != message.parsed.identity)
        {
            candidate.archive_current_state()?;
        }
        candidate
            .operate(|record| engine.decrypt_pre_key(record, &message.bytes, &parameters))
            .map_err(|error| {
                message_error(
                    error,
                    CiphertextMessageType::PreKey,
                    0,
                    message.message().counter(),
                )
            })?
    };
    let consumed_prekey_id = if existing { None } else { message.pre_key_id() };
    let stored_change =
        commit_received_session(checkout, candidate, address, identities, consumed_prekey_id)
            .await?;
    Ok(DecryptionResult {
        plaintext,
        consumed_prekey_id,
        identity_change: if existing {
            IdentityChange::NewOrUnchanged
        } else {
            stored_change
        },
    })
}
