//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::cell::RefCell;

use rand::{CryptoRng, Rng};

use crate::crypto::DecryptionError as DecryptionErrorCrypto;
use crate::crypto::{
    aes_256_cbc_decrypt_in_place, aes_256_cbc_decrypt_into, aes_256_cbc_encrypt_into,
};

// Thread-local buffers for AES operations to reduce allocations and memory fragmentation
thread_local! {
    static ENCRYPTION_BUFFER: RefCell<EncryptionBuffer> = RefCell::new(EncryptionBuffer::new());
    static DECRYPTION_BUFFER: RefCell<EncryptionBuffer> = RefCell::new(EncryptionBuffer::new());
}

// Wrapper for the encryption buffer with intelligent size management
struct EncryptionBuffer {
    buffer: Vec<u8>,
    usage_count: usize,
}

impl EncryptionBuffer {
    const INITIAL_CAPACITY: usize = 1024;
    const MAX_CAPACITY: usize = 16 * 1024; // 16KB max
    const SHRINK_THRESHOLD: usize = 100; // Shrink every 100 uses if oversized

    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(Self::INITIAL_CAPACITY),
            usage_count: 0,
        }
    }

    fn get_buffer(&mut self) -> &mut Vec<u8> {
        self.usage_count += 1;

        // Periodically manage buffer size to prevent unbounded growth
        if self.usage_count.is_multiple_of(Self::SHRINK_THRESHOLD) {
            // If buffer has grown beyond max capacity, shrink it back
            if self.buffer.capacity() > Self::MAX_CAPACITY {
                self.buffer = Vec::with_capacity(Self::INITIAL_CAPACITY);
            } else if self.buffer.is_empty() && self.buffer.capacity() > Self::INITIAL_CAPACITY * 2
            {
                // If buffer is empty but has grown significantly, shrink it
                self.buffer.shrink_to(Self::INITIAL_CAPACITY);
            }
        }

        &mut self.buffer
    }
}

/// Current capacity of this thread's encrypt buffer. Test-only: lets the
/// oversized-buffer-release regression test observe the thread-local.
#[cfg(test)]
fn encryption_buffer_capacity() -> usize {
    ENCRYPTION_BUFFER.with(|b| b.borrow().buffer.capacity())
}
use crate::protocol::consts::MAX_FORWARD_JUMPS;

/// Max chain steps a single decrypt may derive before rejecting a
/// too-far-ahead counter. Peer sessions match WA Web's `signalFutureMessagesMax`
/// (2000); a session with our own devices gets the wider self ceiling.
const fn forward_jump_limit(is_self: bool) -> usize {
    if is_self {
        crate::protocol::consts::MAX_FORWARD_JUMPS_SELF
    } else {
        MAX_FORWARD_JUMPS
    }
}
use crate::protocol::ratchet::keys::MessageKeyGenerator;
use crate::protocol::ratchet::{ChainKey, RootKey, UsePQRatchet};
use crate::protocol::state::{DecryptSnapshot, PreKeyId, ReceiverChainState, SessionState};
use crate::protocol::{
    CiphertextMessage, CiphertextMessageType, Direction, IdentityChange, IdentityKeyStore, KeyPair,
    PreKeySignalMessage, PreKeyStore, ProtocolAddress, PublicKey, Result, SessionCheckout,
    SessionRecord, SessionStore, SignalMessage, SignalProtocolError, SignedPreKeyStore, session,
};

/// Plaintext plus whether decrypting this message replaced a previously-stored
/// identity key for the sender. A [`IdentityChange::ReplacedExisting`] is the
/// local signal that the peer's identity changed (e.g. reinstall), letting the
/// caller react without waiting for the server's `<identity/>` push.
#[derive(Debug)]
pub struct DecryptionResult {
    pub plaintext: Vec<u8>,
    pub identity_change: IdentityChange,
    /// The one-time pre-key a pkmsg consumed, if any. The decrypt does NOT delete
    /// it: removing the prekey is the caller's responsibility, and only once the
    /// promoted session is itself durable. A crash with the prekey already gone
    /// but the session still volatile makes a redelivered pkmsg undecryptable, so
    /// the caller buffers this id and deletes it alongside the session flush.
    /// `None` for a SignalMessage decrypt or a pkmsg that reused an existing session.
    pub consumed_prekey_id: Option<PreKeyId>,
}

/// A parsed Signal envelope whose wire storage may be consumed by decryption.
///
/// Borrowed APIs remain available for callers that need to retain ciphertext.
/// This owner lets receive pipelines reuse a uniquely-owned ciphertext
/// allocation for plaintext after authentication, avoiding a full-size peak
/// copy for large inline messages. Errors before authenticated decryption leave
/// the envelope available for identity/session migration retries.
#[derive(Debug)]
pub struct OwnedCiphertextMessage {
    inner: Option<CiphertextMessage>,
}

impl OwnedCiphertextMessage {
    pub fn new(message: CiphertextMessage) -> Self {
        Self {
            inner: Some(message),
        }
    }

    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    fn message_type(&self) -> Option<CiphertextMessageType> {
        self.inner.as_ref().map(CiphertextMessage::message_type)
    }

    fn pre_key_message(&self) -> Option<&PreKeySignalMessage> {
        match self.inner.as_ref()? {
            CiphertextMessage::PreKeySignalMessage(message) => Some(message),
            _ => None,
        }
    }

    fn signal_message(&self) -> Option<&SignalMessage> {
        match self.inner.as_ref()? {
            CiphertextMessage::SignalMessage(message) => Some(message),
            CiphertextMessage::PreKeySignalMessage(message) => Some(message.message()),
            _ => None,
        }
    }

    fn take_signal_message(&mut self) -> Option<SignalMessage> {
        match self.inner.take()? {
            CiphertextMessage::SignalMessage(message) => Some(message),
            CiphertextMessage::PreKeySignalMessage(message) => Some(message.into_message()),
            other => {
                self.inner = Some(other);
                None
            }
        }
    }
}

impl From<CiphertextMessage> for OwnedCiphertextMessage {
    fn from(message: CiphertextMessage) -> Self {
        Self::new(message)
    }
}

// Keeping both ownership modes in one concrete type avoids duplicating the
// decrypt state machine while deferring owned-envelope consumption until MAC
// authentication succeeds.
enum SignalDecryptInput<'a> {
    Borrowed(&'a SignalMessage),
    Owned(&'a mut OwnedCiphertextMessage),
}

impl SignalDecryptInput<'_> {
    #[inline(always)]
    fn signal_message(&self) -> &SignalMessage {
        match self {
            Self::Borrowed(message) => message,
            Self::Owned(message) => message
                .signal_message()
                .expect("owned Signal decrypt input has a Signal envelope"),
        }
    }

    #[inline(always)]
    fn is_available(&self) -> bool {
        match self {
            Self::Borrowed(_) => true,
            Self::Owned(message) => message.is_available(),
        }
    }

    fn decrypt(
        &mut self,
        key: &[u8],
        iv: &[u8],
    ) -> std::result::Result<Vec<u8>, DecryptionErrorCrypto> {
        match self {
            Self::Borrowed(message) => decrypt_borrowed_signal(message, key, iv),
            Self::Owned(message) => {
                let message = message
                    .take_signal_message()
                    .expect("authenticated owned input remains available");
                match message.try_into_ciphertext_vec() {
                    Ok(mut ciphertext) => {
                        aes_256_cbc_decrypt_in_place(&mut ciphertext, key, iv)?;
                        Ok(ciphertext)
                    }
                    Err(shared) => decrypt_borrowed_signal(&shared, key, iv),
                }
            }
        }
    }
}

#[inline(always)]
fn decrypt_borrowed_signal(
    message: &SignalMessage,
    key: &[u8],
    iv: &[u8],
) -> std::result::Result<Vec<u8>, DecryptionErrorCrypto> {
    DECRYPTION_BUFFER.with(|buffer| {
        let mut buf_wrapper = buffer.borrow_mut();
        let buf = buf_wrapper.get_buffer();
        let ciphertext = message
            .body()
            .map_err(|_| DecryptionErrorCrypto::BadCiphertext("invalid Signal ciphertext body"))?;
        aes_256_cbc_decrypt_into(ciphertext, key, iv, buf)?;
        let result = std::mem::take(buf);
        // Keep ordinary messages allocation-free without pinning an occasional
        // oversized plaintext in thread-local storage.
        buf.reserve(EncryptionBuffer::INITIAL_CAPACITY);
        Ok(result)
    })
}

enum PreKeyDecryptInput<'a> {
    Borrowed(&'a PreKeySignalMessage),
    Owned(&'a mut OwnedCiphertextMessage),
}

impl<'a> PreKeyDecryptInput<'a> {
    #[inline]
    fn pre_key_message(&self) -> &PreKeySignalMessage {
        match self {
            Self::Borrowed(message) => message,
            Self::Owned(message) => message
                .pre_key_message()
                .expect("owned PreKey decrypt input has a PreKey envelope"),
        }
    }

    #[inline]
    fn into_signal_input(self) -> SignalDecryptInput<'a> {
        match self {
            Self::Borrowed(message) => SignalDecryptInput::Borrowed(message.message()),
            Self::Owned(message) => SignalDecryptInput::Owned(message),
        }
    }
}

pub async fn message_encrypt(
    ptext: &[u8],
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
) -> Result<CiphertextMessage> {
    let mut session = SessionCheckout::load(session_store, remote_address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(remote_address.clone()))?;

    let result =
        message_encrypt_inner(ptext, remote_address, session.record_mut(), identity_store).await;

    // Always restore — chain key is only advanced inside the inner
    // function after identity checks pass, so no counters are burned.
    session.commit().await?;

    result
}

async fn message_encrypt_inner(
    ptext: &[u8],
    remote_address: &ProtocolAddress,
    session_record: &mut SessionRecord,
    identity_store: &mut dyn IdentityKeyStore,
) -> Result<CiphertextMessage> {
    let session_state = session_record
        .session_state_mut()
        .ok_or_else(|| SignalProtocolError::SessionNotFound(remote_address.clone()))?;

    let chain_key = session_state.get_sender_chain_key()?;

    let (message_keys_gen, next_chain_key) = chain_key.step_with_message_keys()?;
    let message_keys = message_keys_gen.generate_keys();

    let sender_ephemeral = session_state.sender_ratchet_key()?;
    let previous_counter = session_state.previous_counter();
    let session_version = session_state
        .session_version()?
        .try_into()
        .map_err(|_| SignalProtocolError::InvalidSessionStructure("version does not fit in u8"))?;

    let local_identity_key = session_state.local_identity_key()?;
    let their_identity_key = session_state.remote_identity_key()?.ok_or_else(|| {
        SignalProtocolError::InvalidState(
            "message_encrypt",
            format!("no remote identity key for {remote_address}"),
        )
    })?;

    // Check trust before doing any crypto work
    if !identity_store
        .is_trusted_identity(remote_address, &their_identity_key, Direction::Sending)
        .await?
    {
        log::warn!(
            "Identity key {} is not trusted for remote address {}",
            hex::encode(their_identity_key.public_key().public_key_bytes()),
            remote_address,
        );
        return Err(SignalProtocolError::UntrustedIdentity(
            remote_address.clone(),
        ));
    }

    // Encrypt into the thread-local buffer and build the message while still
    // borrowing it: SignalMessage::new copies the ciphertext into its protobuf
    // body, so no owned intermediate Vec is needed. The buffer is reused across
    // calls (cleared, capacity kept) instead of being taken out and reallocated.
    // aes_256_cbc_encrypt appends from buf.len(), so clear it first; this also
    // drops any ciphertext left by a prior call that errored.
    let message = ENCRYPTION_BUFFER.with(|buffer| {
        let mut buf_wrapper = buffer.borrow_mut();
        let buf = buf_wrapper.get_buffer();
        buf.clear();
        aes_256_cbc_encrypt_into(ptext, message_keys.cipher_key(), message_keys.iv(), buf)
            .map_err(|_| {
                log::error!("session state corrupt for {remote_address}");
                SignalProtocolError::InvalidSessionStructure("invalid sender chain message keys")
            })?;
        let ctext = buf.as_slice();

        let message = if let Some(items) = session_state.unacknowledged_pre_key_message_items()? {
            let local_registration_id = session_state.local_registration_id();

            log::debug!(
                "Building PreKeyWhisperMessage for: {} with preKeyId: {}",
                remote_address,
                items
                    .pre_key_id()
                    .map_or_else(|| "<none>".to_string(), |id| id.to_string()),
            );

            let message = SignalMessage::new(
                session_version,
                message_keys.mac_key(),
                sender_ephemeral,
                chain_key.index(),
                previous_counter,
                ctext,
                &local_identity_key,
                &their_identity_key,
            )?;

            CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::new(
                session_version,
                local_registration_id,
                items.pre_key_id(),
                items.signed_pre_key_id(),
                *items.base_key(),
                local_identity_key,
                message,
            )?)
        } else {
            CiphertextMessage::SignalMessage(SignalMessage::new(
                session_version,
                message_keys.mac_key(),
                sender_ephemeral,
                chain_key.index(),
                previous_counter,
                ctext,
                &local_identity_key,
                &their_identity_key,
            )?)
        };
        // A plaintext whose ciphertext exceeds MAX_CAPACITY leaves an oversized
        // buffer in thread-local storage; release it now (the old take+realloc
        // path did this implicitly) so a single large send doesn't pin memory
        // per worker thread until get_buffer's periodic shrink fires.
        if buf.capacity() > EncryptionBuffer::MAX_CAPACITY {
            *buf = Vec::with_capacity(EncryptionBuffer::INITIAL_CAPACITY);
        }
        Ok::<CiphertextMessage, SignalProtocolError>(message)
    })?;

    identity_store
        .save_identity(remote_address, &their_identity_key)
        .await?;

    session_state.set_sender_chain_key(&next_chain_key)?;

    // Counters are leased in batches so the send path only needs a durable
    // flush when the lease runs out; a reload fast-forwards past the whole
    // lease, so this counter can never be re-derived after a crash.
    if chain_key.index() >= session_record.reserved_sender_chain_index() {
        session_record.reserve_sender_chain_counters(chain_key.index());
    }

    Ok(message)
}

#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt<R: Rng + CryptoRng>(
    ciphertext: &CiphertextMessage,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    pre_key_store: &mut dyn PreKeyStore,
    signed_pre_key_store: &dyn SignedPreKeyStore,
    csprng: &mut R,
    use_pq_ratchet: UsePQRatchet,
) -> Result<DecryptionResult> {
    match ciphertext {
        CiphertextMessage::SignalMessage(m) => {
            message_decrypt_signal(m, remote_address, session_store, identity_store, csprng).await
        }
        CiphertextMessage::PreKeySignalMessage(m) => {
            message_decrypt_prekey(
                m,
                remote_address,
                session_store,
                identity_store,
                pre_key_store,
                signed_pre_key_store,
                csprng,
                use_pq_ratchet,
            )
            .await
        }
        _ => Err(SignalProtocolError::InvalidArgument(format!(
            "message_decrypt cannot be used to decrypt {:?} messages",
            ciphertext.message_type()
        ))),
    }
}

/// Decrypt a caller-owned Signal envelope, reusing its wire allocation when it
/// is uniquely owned. See [`OwnedCiphertextMessage`] for retry semantics.
#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_owned<R: Rng + CryptoRng>(
    ciphertext: &mut OwnedCiphertextMessage,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    pre_key_store: &mut dyn PreKeyStore,
    signed_pre_key_store: &dyn SignedPreKeyStore,
    csprng: &mut R,
    use_pq_ratchet: UsePQRatchet,
) -> Result<DecryptionResult> {
    match ciphertext.message_type() {
        Some(CiphertextMessageType::Whisper) => {
            message_decrypt_signal_input(
                SignalDecryptInput::Owned(ciphertext),
                remote_address,
                session_store,
                identity_store,
                csprng,
            )
            .await
        }
        Some(CiphertextMessageType::PreKey) => {
            message_decrypt_prekey_input(
                PreKeyDecryptInput::Owned(ciphertext),
                remote_address,
                session_store,
                identity_store,
                pre_key_store,
                signed_pre_key_store,
                csprng,
                use_pq_ratchet,
            )
            .await
        }
        Some(kind) => Err(SignalProtocolError::InvalidArgument(format!(
            "message_decrypt_owned cannot decrypt {kind:?} messages"
        ))),
        None => Err(SignalProtocolError::InvalidArgument(
            "owned ciphertext was already consumed".to_owned(),
        )),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn message_decrypt_prekey<R: Rng + CryptoRng>(
    ciphertext: &PreKeySignalMessage,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    pre_key_store: &mut dyn PreKeyStore,
    signed_pre_key_store: &dyn SignedPreKeyStore,
    csprng: &mut R,
    use_pq_ratchet: UsePQRatchet,
) -> Result<DecryptionResult> {
    message_decrypt_prekey_input(
        PreKeyDecryptInput::Borrowed(ciphertext),
        remote_address,
        session_store,
        identity_store,
        pre_key_store,
        signed_pre_key_store,
        csprng,
        use_pq_ratchet,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn message_decrypt_prekey_input<R: Rng + CryptoRng>(
    ciphertext: PreKeyDecryptInput<'_>,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    pre_key_store: &mut dyn PreKeyStore,
    signed_pre_key_store: &dyn SignedPreKeyStore,
    csprng: &mut R,
    use_pq_ratchet: UsePQRatchet,
) -> Result<DecryptionResult> {
    let mut session = SessionCheckout::load_or_create(session_store, remote_address).await?;
    session.snapshot_for_rollback();

    let result = message_decrypt_prekey_inner(
        ciphertext,
        remote_address,
        session.record_mut(),
        identity_store,
        pre_key_store,
        signed_pre_key_store,
        csprng,
        use_pq_ratchet,
    )
    .await;

    if result.is_ok() {
        session.commit().await?;
    } else {
        session.rollback().await?;
    }

    let (plaintext, pre_key_used, identity_change) = result?;

    // The consumed prekey is reported up, not deleted here: the promoted session
    // is still volatile in the caller's cache, so the prekey must only be removed
    // once that session is durable (see DecryptionResult::consumed_prekey_id).
    Ok(DecryptionResult {
        plaintext,
        identity_change,
        consumed_prekey_id: pre_key_used,
    })
}

#[allow(clippy::too_many_arguments)]
async fn message_decrypt_prekey_inner<R: Rng + CryptoRng>(
    ciphertext: PreKeyDecryptInput<'_>,
    remote_address: &ProtocolAddress,
    session_record: &mut SessionRecord,
    identity_store: &mut dyn IdentityKeyStore,
    pre_key_store: &mut dyn PreKeyStore,
    signed_pre_key_store: &dyn SignedPreKeyStore,
    csprng: &mut R,
    use_pq_ratchet: UsePQRatchet,
) -> Result<(Vec<u8>, Option<PreKeyId>, IdentityChange)> {
    let process_prekey_result = session::process_prekey(
        ciphertext.pre_key_message(),
        remote_address,
        session_record,
        identity_store,
        pre_key_store,
        signed_pre_key_store,
        use_pq_ratchet,
    )
    .await;

    let (pre_key_used, identity_to_save, reused_existing_session) = match process_prekey_result {
        Ok(result) => result,
        Err(e) => {
            let errs = [e];
            log::error!(
                "{}",
                create_decryption_failure_log(
                    remote_address,
                    &errs,
                    session_record,
                    ciphertext.pre_key_message().message()
                )?
            );
            let [e] = errs;
            return Err(e);
        }
    };
    let their_identity_key = *identity_to_save.their_identity_key;
    let mut ciphertext = ciphertext.into_signal_input();

    let decrypt = decrypt_message_with_record(
        remote_address,
        session_record,
        &mut ciphertext,
        CiphertextMessageType::PreKey,
        csprng,
    )?;

    let saved = identity_store
        .save_identity(remote_address, &their_identity_key)
        .await?;
    let plaintext = decrypt.commit();

    // A duplicate/out-of-order pkmsg that matched an existing session carries the
    // identity from when that session was built, not a fresh rotation. Reporting
    // it as a change would fire a spurious local identity-change reaction (mirrors
    // the previous-session SignalMessage path).
    let identity_change = if reused_existing_session {
        IdentityChange::NewOrUnchanged
    } else {
        saved
    };

    Ok((plaintext, pre_key_used.pre_key_id, identity_change))
}

pub async fn message_decrypt_signal<R: Rng + CryptoRng>(
    ciphertext: &SignalMessage,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    csprng: &mut R,
) -> Result<DecryptionResult> {
    message_decrypt_signal_input(
        SignalDecryptInput::Borrowed(ciphertext),
        remote_address,
        session_store,
        identity_store,
        csprng,
    )
    .await
}

async fn message_decrypt_signal_input<R: Rng + CryptoRng>(
    mut ciphertext: SignalDecryptInput<'_>,
    remote_address: &ProtocolAddress,
    session_store: &mut dyn SessionStore,
    identity_store: &mut dyn IdentityKeyStore,
    csprng: &mut R,
) -> Result<DecryptionResult> {
    let mut session = SessionCheckout::load(session_store, remote_address)
        .await?
        .ok_or_else(|| SignalProtocolError::SessionNotFound(remote_address.clone()))?;

    let result = message_decrypt_signal_inner(
        &mut ciphertext,
        remote_address,
        session.record_mut(),
        identity_store,
        csprng,
    )
    .await;

    session.commit().await?;

    let (plaintext, identity_change) = result?;
    Ok(DecryptionResult {
        plaintext,
        identity_change,
        consumed_prekey_id: None,
    })
}

async fn message_decrypt_signal_inner<R: Rng + CryptoRng>(
    ciphertext: &mut SignalDecryptInput<'_>,
    remote_address: &ProtocolAddress,
    session_record: &mut SessionRecord,
    identity_store: &mut dyn IdentityKeyStore,
    csprng: &mut R,
) -> Result<(Vec<u8>, IdentityChange)> {
    // A record with no current state and no previous states is degenerate — treat
    // it as missing so the caller gets SessionNotFound and sends a proper retry
    // receipt (with error code 1) instead of attempting decryption that will always
    // fail with an unhelpful InvalidMessage error.
    if session_record.session_state().is_none() && session_record.previous_session_count() == 0 {
        log::warn!(
            "Session record for {} exists but has no usable state (no current, 0 previous). \
             Treating as SessionNotFound.",
            remote_address
        );
        return Err(SignalProtocolError::SessionNotFound(remote_address.clone()));
    }

    let decrypt = decrypt_message_with_record(
        remote_address,
        session_record,
        ciphertext,
        CiphertextMessageType::Whisper,
        csprng,
    )?;

    let their_identity_key = decrypt
        .state()
        .remote_identity_key()
        .expect("successfully decrypted; must have a remote identity key")
        .expect("successfully decrypted; must have a remote identity key");

    // Handle identity trust based on whether we used the current or a previous session.
    //
    // For current session: Check if the identity is trusted, and save it if so.
    // For previous session: Skip the trust check and save the identity directly.
    //
    // When we successfully decrypt with a previous (archived) session, we already had
    // a valid session with that identity - it was trusted when the session was established.
    // The previous session gets promoted to current via `promote_old_session`, so we need
    // to save its identity to avoid UntrustedIdentity errors on subsequent messages.
    // This handles out-of-order message delivery after an identity change gracefully.
    let identity_change = if decrypt.used_previous_session() {
        log::debug!(
            "Saving identity for {} from previous session (skipping trust check)",
            remote_address,
        );
        // Re-saving an archived session's (older) identity for out-of-order
        // delivery is not the peer's current identity changing, so never report
        // it as a change. Doing so would fire a spurious local identity-change
        // reaction and clobber the current identity.
        identity_store
            .save_identity(remote_address, &their_identity_key)
            .await?;
        IdentityChange::NewOrUnchanged
    } else {
        if !identity_store
            .is_trusted_identity(remote_address, &their_identity_key, Direction::Receiving)
            .await?
        {
            log::warn!(
                "Identity key {} is not trusted for remote address {}",
                hex::encode(their_identity_key.public_key().public_key_bytes()),
                remote_address,
            );
            return Err(SignalProtocolError::UntrustedIdentity(
                remote_address.clone(),
            ));
        }

        identity_store
            .save_identity(remote_address, &their_identity_key)
            .await?
    };

    Ok((decrypt.commit(), identity_change))
}

fn create_decryption_failure_log(
    remote_address: &ProtocolAddress,
    mut errs: &[SignalProtocolError],
    record: &SessionRecord,
    ciphertext: &SignalMessage,
) -> Result<String> {
    fn append_session_summary(
        lines: &mut Vec<String>,
        idx: usize,
        state: std::result::Result<&SessionState, crate::protocol::state::InvalidSessionError>,
        err: Option<&SignalProtocolError>,
    ) {
        let chains = state.map(|state| state.all_receiver_chain_logging_info());
        match (err, &chains) {
            (Some(err), Ok(chains)) => {
                lines.push(format!(
                    "Candidate session {} failed with '{}', had {} receiver chains",
                    idx,
                    err,
                    chains.len()
                ));
            }
            (Some(err), Err(state_err)) => {
                lines.push(format!(
                    "Candidate session {idx} failed with '{err}'; cannot get receiver chain info ({state_err})",
                ));
            }
            (None, Ok(chains)) => {
                lines.push(format!(
                    "Candidate session {} had {} receiver chains",
                    idx,
                    chains.len()
                ));
            }
            (None, Err(state_err)) => {
                lines.push(format!(
                    "Candidate session {idx}: cannot get receiver chain info ({state_err})",
                ));
            }
        }

        if let Ok(chains) = chains {
            for chain in chains {
                let chain_idx = match chain.1 {
                    Some(i) => i.to_string(),
                    None => "missing in protobuf".to_string(),
                };

                lines.push(format!(
                    "Receiver chain with sender ratchet public key {} chain key index {}",
                    hex::encode(chain.0),
                    chain_idx
                ));
            }
        }
    }

    let mut lines = vec![];

    lines.push(format!(
        "Message from {} failed to decrypt; sender ratchet public key {} message counter {}",
        remote_address,
        hex::encode(ciphertext.sender_ratchet_key().public_key_bytes()),
        ciphertext.counter()
    ));

    if let Some(current_session) = record.session_state() {
        let err = errs.first();
        if err.is_some() {
            errs = &errs[1..];
        }
        append_session_summary(&mut lines, 0, Ok(current_session), err);
    } else {
        lines.push("No current session".to_string());
    }

    for (idx, (state, err)) in record
        .previous_session_states()
        .zip(errs.iter().map(Some).chain(std::iter::repeat(None)))
        .enumerate()
    {
        let state = match state {
            Ok(ref state) => Ok(state),
            Err(err) => Err(err),
        };
        append_session_summary(&mut lines, idx + 1, state, err);
    }

    Ok(lines.join("\n"))
}

enum RecordDecryptState {
    Current(DecryptSnapshot),
    Previous {
        state: Box<SessionState>,
        effect: StateDecryptEffect,
        index: usize,
    },
}

struct DeferredCurrentDecrypt<'a> {
    record: &'a mut SessionRecord,
    ratchet_key: PublicKey,
    chain_key: ChainKey,
    plaintext: Vec<u8>,
}

enum RecordDecrypt<'a> {
    Deferred(DeferredCurrentDecrypt<'a>),
    Transaction(RecordDecryptTransaction<'a>),
}

impl RecordDecrypt<'_> {
    fn state(&self) -> &SessionState {
        match self {
            Self::Deferred(decrypt) => decrypt
                .record
                .session_state()
                .expect("a deferred decrypt keeps the current state installed"),
            Self::Transaction(decrypt) => decrypt.state(),
        }
    }

    fn used_previous_session(&self) -> bool {
        match self {
            Self::Deferred(_) => false,
            Self::Transaction(decrypt) => decrypt.used_previous_session(),
        }
    }

    fn commit(self) -> Vec<u8> {
        match self {
            Self::Deferred(mut decrypt) => {
                let state = decrypt
                    .record
                    .session_state_mut()
                    .expect("a deferred decrypt keeps the current state installed");
                state
                    .set_receiver_chain_key(&decrypt.ratchet_key, &decrypt.chain_key)
                    .expect("the deferred in-order receiver chain remains installed");
                state.clear_unacknowledged_pre_key_message();
                std::mem::take(&mut decrypt.plaintext)
            }
            Self::Transaction(decrypt) => decrypt.commit(),
        }
    }
}

struct RecordDecryptTransaction<'a> {
    record: &'a mut SessionRecord,
    state: Option<RecordDecryptState>,
    plaintext: Vec<u8>,
}

impl RecordDecryptTransaction<'_> {
    fn state(&self) -> &SessionState {
        match self
            .state
            .as_ref()
            .expect("a live decrypt transaction owns its rollback")
        {
            RecordDecryptState::Current(_) => self
                .record
                .session_state()
                .expect("a current decrypt transaction keeps the current state installed"),
            RecordDecryptState::Previous { state, .. } => state,
        }
    }

    fn used_previous_session(&self) -> bool {
        matches!(self.state, Some(RecordDecryptState::Previous { .. }))
    }

    fn commit(mut self) -> Vec<u8> {
        match self
            .state
            .take()
            .expect("a live decrypt transaction owns its rollback")
        {
            RecordDecryptState::Current(_) => {
                let state = self
                    .record
                    .session_state_mut()
                    .expect("a current decrypt transaction keeps the current state installed");
                state.clear_unacknowledged_pre_key_message();
            }
            RecordDecryptState::Previous {
                mut state, effect, ..
            } => {
                effect.commit(&mut state);
                state.clear_unacknowledged_pre_key_message();
                self.record.promote_state(*state);
            }
        }
        std::mem::take(&mut self.plaintext)
    }
}

impl Drop for RecordDecryptTransaction<'_> {
    fn drop(&mut self) {
        let Some(state) = self.state.take() else {
            return;
        };
        match state {
            RecordDecryptState::Current(snapshot) => self
                .record
                .session_state_mut()
                .expect("a current decrypt transaction keeps the current state installed")
                .restore_decrypt_snapshot(snapshot),
            RecordDecryptState::Previous {
                mut state,
                effect,
                index,
            } => {
                effect.rollback(&mut state);
                self.record.restore_previous_session(index, *state);
            }
        }
    }
}

fn decrypt_message_with_record<'a, R: Rng + CryptoRng>(
    remote_address: &ProtocolAddress,
    record: &'a mut SessionRecord,
    ciphertext: &mut SignalDecryptInput<'_>,
    original_message_type: CiphertextMessageType,
    csprng: &mut R,
) -> Result<RecordDecrypt<'a>> {
    debug_assert!(matches!(
        original_message_type,
        CiphertextMessageType::Whisper | CiphertextMessageType::PreKey
    ));
    let log_decryption_failure =
        |message: &SignalMessage, state: &SessionState, error: &SignalProtocolError| {
            // A warning rather than an error because we try multiple sessions.
            log::warn!(
                "Failed to decrypt {:?} message with ratchet key: {} and counter: {}. \
             Session loaded for {}. Local session has base key: {} and counter: {}. {}",
                original_message_type,
                hex::encode(message.sender_ratchet_key().public_key_bytes()),
                message.counter(),
                remote_address,
                state
                    .sender_ratchet_key_for_logging()
                    .unwrap_or_else(|e| format!("<error: {e}>")),
                state.previous_counter(),
                error
            );
        };

    let mut errs = vec![];

    // Take ownership of current state instead of cloning - avoids allocation
    if let Some(mut current_state) = record.take_session_state() {
        let result = decrypt_message_with_state(
            CurrentOrPrevious::Current,
            &mut current_state,
            ciphertext,
            original_message_type,
            remote_address,
            csprng,
        );

        match result {
            Ok(result) => {
                log::debug!(
                    "decrypted {:?} message from {} with current session state (base key {})",
                    original_message_type,
                    remote_address,
                    current_state
                        .sender_ratchet_key_for_logging()
                        .expect("successful decrypt always has a valid base key"),
                );
                record.set_session_state(current_state);
                return match result.effect {
                    StateDecryptEffect::DeferredChainKey {
                        ratchet_key,
                        chain_key,
                    } => Ok(RecordDecrypt::Deferred(DeferredCurrentDecrypt {
                        record,
                        ratchet_key,
                        chain_key,
                        plaintext: result.plaintext,
                    })),
                    StateDecryptEffect::Applied(snapshot) => {
                        Ok(RecordDecrypt::Transaction(RecordDecryptTransaction {
                            record,
                            state: Some(RecordDecryptState::Current(snapshot)),
                            plaintext: result.plaintext,
                        }))
                    }
                };
            }
            Err(SignalProtocolError::DuplicatedMessage(chain, counter)) => {
                // Restore state before returning error
                record.set_session_state(current_state);
                return Err(SignalProtocolError::DuplicatedMessage(chain, counter));
            }
            Err(e) if !ciphertext.is_available() => {
                // Authentication succeeded, but the provider rejected the
                // caller-owned body after it had been consumed for in-place
                // decryption. No other session can validly authenticate it.
                record.set_session_state(current_state);
                return Err(e);
            }
            Err(e) => {
                log_decryption_failure(ciphertext.signal_message(), &current_state, &e);
                errs.push(e);
                match original_message_type {
                    CiphertextMessageType::PreKey => {
                        // A PreKey message creates a session and then decrypts a Whisper message
                        // using that session. No need to check older sessions.
                        // Log at warn level since this error may be recoverable at higher layers
                        // (e.g., UntrustedIdentity can be handled by clearing old identity and retrying)
                        // Restore state before returning error
                        record.set_session_state(current_state);
                        log::warn!(
                            "{}",
                            create_decryption_failure_log(
                                remote_address,
                                &errs,
                                record,
                                ciphertext.signal_message()
                            )?
                        );
                        // Preserve BadMac so it maps to WA Web error code 7 in retry receipts.
                        if errs
                            .iter()
                            .any(|e| matches!(e, SignalProtocolError::BadMac(_)))
                        {
                            return Err(SignalProtocolError::BadMac(original_message_type));
                        }
                        return Err(SignalProtocolError::InvalidMessage(
                            original_message_type,
                            "decryption failed",
                        ));
                    }
                    CiphertextMessageType::Whisper => {
                        // Restore state before trying previous sessions
                        record.set_session_state(current_state);
                    }
                    CiphertextMessageType::SenderKey | CiphertextMessageType::Plaintext => {
                        unreachable!("should not be using Double Ratchet for these")
                    }
                }
            }
        }
    }

    // Try some old sessions using take/restore pattern to avoid cloning all sessions.
    // We take ownership of one session at a time, try to decrypt, and either:
    // - Promote it if successful (session already removed by take)
    // - Restore it if failed (put it back at the same index)
    let previous_count = record.previous_session_count();
    let mut idx = 0;

    while idx < previous_count {
        // Take ownership of this session (removes from list)
        let Some(mut previous) = record.take_previous_session(idx) else {
            // Should not happen since we checked count, but handle gracefully
            break;
        };

        let result = decrypt_message_with_state(
            CurrentOrPrevious::Previous,
            &mut previous,
            ciphertext,
            original_message_type,
            remote_address,
            csprng,
        );

        match result {
            Ok(result) => {
                log::debug!(
                    "decrypted {:?} message from {} with PREVIOUS session state (base key {})",
                    original_message_type,
                    remote_address,
                    previous
                        .sender_ratchet_key_for_logging()
                        .expect("successful decrypt always has a valid base key"),
                );
                return Ok(RecordDecrypt::Transaction(RecordDecryptTransaction {
                    record,
                    state: Some(RecordDecryptState::Previous {
                        state: Box::new(previous),
                        effect: result.effect,
                        index: idx,
                    }),
                    plaintext: result.plaintext,
                }));
            }
            Err(SignalProtocolError::DuplicatedMessage(chain, counter)) => {
                // Restore the session before returning error
                record.restore_previous_session(idx, previous);
                return Err(SignalProtocolError::DuplicatedMessage(chain, counter));
            }
            Err(e) if !ciphertext.is_available() => {
                record.restore_previous_session(idx, previous);
                return Err(e);
            }
            Err(e) => {
                log_decryption_failure(ciphertext.signal_message(), &previous, &e);
                errs.push(e);
                // Restore the session at the same index and move to next
                record.restore_previous_session(idx, previous);
                idx += 1;
            }
        }
    }

    // No session worked - log error and return failure
    let previous_state_count = record.previous_session_count();

    if let Some(current_state) = record.session_state() {
        log::error!(
            "No valid session for recipient: {}, current session base key {}, number of previous states: {}",
            remote_address,
            current_state
                .sender_ratchet_key_for_logging()
                .unwrap_or_else(|e| format!("<error: {e}>")),
            previous_state_count,
        );
    } else {
        log::error!(
            "No valid session for recipient: {}, (no current session state), number of previous states: {}",
            remote_address,
            previous_state_count,
        );
    }
    log::error!(
        "{}",
        create_decryption_failure_log(remote_address, &errs, record, ciphertext.signal_message())?
    );

    // If any session state produced a BadMac error, propagate it rather than the
    // generic InvalidMessage. BadMac means at least one state derived a message key
    // and verified the MAC — it specifically failed, which maps to WA Web error
    // code 7 (SignalErrorBadMac) vs 4 (SignalErrorInvalidMessage).
    if errs
        .iter()
        .any(|e| matches!(e, SignalProtocolError::BadMac(_)))
    {
        return Err(SignalProtocolError::BadMac(original_message_type));
    }

    Err(SignalProtocolError::InvalidMessage(
        original_message_type,
        "decryption failed",
    ))
}

#[derive(Clone, Copy)]
enum CurrentOrPrevious {
    Current,
    Previous,
}

impl std::fmt::Display for CurrentOrPrevious {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Current => write!(f, "current"),
            Self::Previous => write!(f, "previous"),
        }
    }
}

struct StateDecryptResult {
    plaintext: Vec<u8>,
    effect: StateDecryptEffect,
}

enum StateDecryptEffect {
    DeferredChainKey {
        ratchet_key: PublicKey,
        chain_key: ChainKey,
    },
    Applied(DecryptSnapshot),
}

impl StateDecryptEffect {
    fn commit(self, state: &mut SessionState) {
        match self {
            Self::DeferredChainKey {
                ratchet_key,
                chain_key,
            } => {
                state
                    .set_receiver_chain_key(&ratchet_key, &chain_key)
                    .expect("the deferred in-order receiver chain remains installed");
            }
            Self::Applied(_) => {}
        }
    }

    fn rollback(self, state: &mut SessionState) {
        match self {
            Self::DeferredChainKey { .. } => {}
            Self::Applied(snapshot) => state.restore_decrypt_snapshot(snapshot),
        }
    }
}

fn decrypt_message_with_state<R: Rng + CryptoRng>(
    current_or_previous: CurrentOrPrevious,
    state: &mut SessionState,
    ciphertext: &mut SignalDecryptInput<'_>,
    original_message_type: CiphertextMessageType,
    remote_address: &ProtocolAddress,
    csprng: &mut R,
) -> Result<StateDecryptResult> {
    // Check for a completely empty or invalid session state before we do anything else.
    let _ = state.root_key().map_err(|_| {
        SignalProtocolError::InvalidMessage(
            original_message_type,
            "No session available to decrypt",
        )
    })?;

    let message = ciphertext.signal_message();
    let ciphertext_version = message.message_version() as u32;
    if ciphertext_version != state.session_version()? {
        return Err(SignalProtocolError::UnrecognizedMessageVersion(
            ciphertext_version,
        ));
    }

    let their_ephemeral = *message.sender_ratchet_key();
    let counter = message.counter();

    // Deferring the common in-order advance keeps cancellation rollback free.
    let receiver_chain = state.receiver_chain_state(&their_ephemeral)?;
    if let Some(ReceiverChainState::Open(chain_key)) = receiver_chain
        && chain_key.index() == counter
    {
        let (message_key_gen, next_chain) = chain_key.step_with_message_keys()?;
        let plaintext = decrypt_with_message_keys(
            current_or_previous,
            state,
            ciphertext,
            original_message_type,
            remote_address,
            message_key_gen,
        )?;
        return Ok(StateDecryptResult {
            plaintext,
            effect: StateDecryptEffect::DeferredChainKey {
                ratchet_key: their_ephemeral,
                chain_key: next_chain,
            },
        });
    }

    let snapshot = state.decrypt_snapshot();

    let result = decrypt_with_pending_state(
        current_or_previous,
        state,
        ciphertext,
        original_message_type,
        remote_address,
        csprng,
        &their_ephemeral,
        receiver_chain,
        counter,
    );
    match result {
        Ok(plaintext) => Ok(StateDecryptResult {
            plaintext,
            effect: StateDecryptEffect::Applied(snapshot),
        }),
        Err(e) => {
            state.restore_decrypt_snapshot(snapshot);
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn decrypt_with_pending_state<R: Rng + CryptoRng>(
    current_or_previous: CurrentOrPrevious,
    state: &mut SessionState,
    ciphertext: &mut SignalDecryptInput<'_>,
    original_message_type: CiphertextMessageType,
    remote_address: &ProtocolAddress,
    csprng: &mut R,
    their_ephemeral: &PublicKey,
    receiver_chain: Option<ReceiverChainState>,
    counter: u32,
) -> Result<Vec<u8>> {
    if let Some(ReceiverChainState::Closed { next_index }) = receiver_chain {
        if counter >= next_index {
            return Err(SignalProtocolError::InvalidSessionStructure(
                "receiver chain is closed",
            ));
        }
        let message_key_gen = state
            .get_message_keys(their_ephemeral, counter)?
            .ok_or(SignalProtocolError::DuplicatedMessage(next_index, counter))?;
        return decrypt_with_message_keys(
            current_or_previous,
            state,
            ciphertext,
            original_message_type,
            remote_address,
            message_key_gen,
        );
    }

    let (chain_key, deferred_ratchet) =
        get_or_create_chain_key(state, their_ephemeral, remote_address, receiver_chain)?;

    let message_key_gen = get_or_create_message_key(
        state,
        their_ephemeral,
        remote_address,
        original_message_type,
        &chain_key,
        counter,
    )?;

    let plaintext = decrypt_with_message_keys(
        current_or_previous,
        state,
        ciphertext,
        original_message_type,
        remote_address,
        message_key_gen,
    )?;

    // Generating a new sender ratchet requires fresh entropy and a second DH.
    // Neither can affect inbound MAC verification, so perform them only after
    // the candidate session has authenticated the message.
    if let Some(deferred_ratchet) = deferred_ratchet {
        deferred_ratchet.apply(state, their_ephemeral, csprng)?;
    }

    Ok(plaintext)
}

fn decrypt_with_message_keys(
    current_or_previous: CurrentOrPrevious,
    state: &SessionState,
    ciphertext: &mut SignalDecryptInput<'_>,
    original_message_type: CiphertextMessageType,
    remote_address: &ProtocolAddress,
    message_key_gen: MessageKeyGenerator,
) -> Result<Vec<u8>> {
    let message_keys = message_key_gen.generate_keys();

    let their_identity_key =
        state
            .remote_identity_key()?
            .ok_or(SignalProtocolError::InvalidSessionStructure(
                "cannot decrypt without remote identity key",
            ))?;

    let local_identity_key = state.local_identity_key()?;

    let mac_valid = ciphertext.signal_message().verify_mac(
        &their_identity_key,
        &local_identity_key,
        message_keys.mac_key(),
    )?;

    if !mac_valid {
        let their_id_fingerprint = hex::encode(their_identity_key.public_key().public_key_bytes());
        let local_id_fingerprint = hex::encode(local_identity_key.public_key().public_key_bytes());

        let mac_key_bytes = message_keys.mac_key();
        let mac_key_fingerprint: String = hex::encode(mac_key_bytes).chars().take(8).collect();

        log::error!(
            "MAC verification failed for message from {}. \
             Remote Identity: {}, \
             Local Identity: {}, \
             MAC Key Fingerprint: {}...",
            remote_address,
            their_id_fingerprint,
            local_id_fingerprint,
            mac_key_fingerprint
        );
        return Err(SignalProtocolError::BadMac(original_message_type));
    }

    match ciphertext.decrypt(message_keys.cipher_key(), message_keys.iv()) {
        Ok(plaintext) => Ok(plaintext),
        Err(DecryptionErrorCrypto::BadKeyOrIv) => {
            log::warn!("{current_or_previous} session state corrupt for {remote_address}",);
            Err(SignalProtocolError::InvalidSessionStructure(
                "invalid receiver chain message keys",
            ))
        }
        Err(DecryptionErrorCrypto::BadCiphertext(msg)) => {
            log::warn!("failed to decrypt 1:1 message: {msg}");
            Err(SignalProtocolError::InvalidMessage(
                original_message_type,
                "failed to decrypt",
            ))
        }
    }
}

#[must_use = "the deferred sender ratchet must be applied after authenticated decryption succeeds"]
struct DeferredSenderRatchet {
    receiver_root_key: RootKey,
    previous_counter: u32,
}

impl DeferredSenderRatchet {
    fn apply<R: Rng + CryptoRng>(
        self,
        state: &mut SessionState,
        their_ephemeral: &PublicKey,
        csprng: &mut R,
    ) -> Result<()> {
        let our_new_ephemeral = KeyPair::generate(csprng);
        let sender_chain = self
            .receiver_root_key
            .create_chain(their_ephemeral, &our_new_ephemeral.private_key)?;

        state.set_root_key(&sender_chain.0);
        state.set_previous_counter(self.previous_counter);
        state.set_sender_chain(&our_new_ephemeral, &sender_chain.1);
        Ok(())
    }
}

fn get_or_create_chain_key(
    state: &mut SessionState,
    their_ephemeral: &PublicKey,
    remote_address: &ProtocolAddress,
    receiver_chain: Option<ReceiverChainState>,
) -> Result<(ChainKey, Option<DeferredSenderRatchet>)> {
    match receiver_chain {
        Some(ReceiverChainState::Open(chain)) => return Ok((chain, None)),
        Some(ReceiverChainState::Closed { .. }) => {
            return Err(SignalProtocolError::InvalidSessionStructure(
                "receiver chain is closed",
            ));
        }
        None => {}
    }

    log::debug!("{remote_address} creating new chains.");

    let root_key = state.root_key()?;
    let our_ephemeral = state.sender_ratchet_private_key()?;
    let receiver_chain = root_key.create_chain(their_ephemeral, &our_ephemeral)?;

    let current_index = state.get_sender_chain_key()?.index();
    let deferred_ratchet = DeferredSenderRatchet {
        receiver_root_key: receiver_chain.0,
        previous_counter: current_index.saturating_sub(1),
    };
    state.add_receiver_chain(their_ephemeral, &receiver_chain.1);

    Ok((receiver_chain.1, Some(deferred_ratchet)))
}

fn get_or_create_message_key(
    state: &mut SessionState,
    their_ephemeral: &PublicKey,
    remote_address: &ProtocolAddress,
    original_message_type: CiphertextMessageType,
    chain_key: &ChainKey,
    counter: u32,
) -> Result<MessageKeyGenerator> {
    let chain_index = chain_key.index();

    if chain_index > counter {
        return match state.get_message_keys(their_ephemeral, counter)? {
            Some(keys) => Ok(keys),
            None => Err(SignalProtocolError::DuplicatedMessage(chain_index, counter)),
        };
    }

    assert!(chain_index <= counter);

    let jump = (counter - chain_index) as usize;

    // Trusted self-sessions get a wider (but no longer unbounded) ceiling; peer
    // sessions match WA Web's `signalFutureMessagesMax`.
    let limit = forward_jump_limit(state.session_with_self()?);
    if jump > limit {
        log::error!(
            "{remote_address} Exceeded future message limit: {limit}, index: {chain_index}, counter: {counter})"
        );
        return Err(SignalProtocolError::InvalidMessage(
            original_message_type,
            "message from too far into the future",
        ));
    }

    let mut chain_key = *chain_key;

    while chain_key.index() < counter {
        let (message_keys, next_chain) = chain_key.step_with_message_keys()?;
        state.set_message_keys(their_ephemeral, message_keys)?;
        chain_key = next_chain;
    }

    let (result_message_keys, next_chain) = chain_key.step_with_message_keys()?;
    state.set_receiver_chain_key(their_ephemeral, &next_chain)?;
    Ok(result_message_keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::*;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::future::Future;
    use std::num::NonZeroU64;
    use std::sync::Mutex as SyncMutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::task::{Context, Poll};

    #[test]
    fn forward_jump_limit_matches_wa_web_signal_future_messages_max() {
        // Peer sessions cap at WA Web's `signalFutureMessagesMax` (2000); the
        // self ceiling stays wider but bounded (previously unbounded).
        assert_eq!(forward_jump_limit(false), 2_000);
        assert_eq!(
            forward_jump_limit(false),
            crate::protocol::consts::MAX_FORWARD_JUMPS
        );
        assert_eq!(forward_jump_limit(true), 25_000);
        assert_eq!(
            forward_jump_limit(true),
            crate::protocol::consts::MAX_FORWARD_JUMPS_SELF
        );
        assert!(forward_jump_limit(true) > forward_jump_limit(false));
    }

    // -- In-memory stores for test isolation --

    struct MemSessionStore(HashMap<String, SessionRecord>);
    impl MemSessionStore {
        fn new() -> Self {
            Self(HashMap::new())
        }
    }

    struct DestructiveSessionStore(SyncMutex<Option<SessionRecord>>);

    impl DestructiveSessionStore {
        const CHECKOUT: SessionCheckoutKey = SessionCheckoutKey::new(0, NonZeroU64::MIN);

        fn new(record: Option<SessionRecord>) -> Self {
            Self(SyncMutex::new(record))
        }

        fn take(&self) -> Option<SessionRecord> {
            self.0.lock().expect("test store lock poisoned").take()
        }

        fn replace(&self, record: SessionRecord) {
            *self.0.lock().expect("test store lock poisoned") = Some(record);
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl SessionStore for DestructiveSessionStore {
        async fn load_session(
            &self,
            _address: &ProtocolAddress,
        ) -> error::Result<Option<SessionRecord>> {
            Ok(self.0.lock().expect("test store lock poisoned").clone())
        }

        fn try_load_session_for_update(
            &self,
            _address: &ProtocolAddress,
        ) -> Option<error::Result<(Option<SessionRecord>, Option<SessionCheckoutKey>)>> {
            Some(Ok((self.take(), Some(Self::CHECKOUT))))
        }

        async fn has_session(&self, _address: &ProtocolAddress) -> error::Result<bool> {
            Ok(self.0.lock().expect("test store lock poisoned").is_some())
        }

        async fn store_session(
            &mut self,
            _address: &ProtocolAddress,
            record: SessionRecord,
        ) -> error::Result<()> {
            self.replace(record);
            Ok(())
        }

        fn try_store_session_from_checkout(
            &mut self,
            _address: &ProtocolAddress,
            record: SessionRecord,
            checkout: Option<SessionCheckoutKey>,
            _had_session: bool,
        ) -> SessionCheckoutStoreResult {
            if checkout != Some(Self::CHECKOUT)
                || self.0.lock().expect("test store lock poisoned").is_some()
            {
                return SessionCheckoutStoreResult::Rejected;
            }
            self.replace(record);
            SessionCheckoutStoreResult::Stored
        }
    }
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl SessionStore for MemSessionStore {
        async fn load_session(
            &self,
            address: &ProtocolAddress,
        ) -> error::Result<Option<SessionRecord>> {
            Ok(self.0.get(address.as_str()).cloned())
        }
        async fn has_session(&self, address: &ProtocolAddress) -> error::Result<bool> {
            Ok(self.0.contains_key(address.as_str()))
        }
        async fn store_session(
            &mut self,
            address: &ProtocolAddress,
            record: SessionRecord,
        ) -> error::Result<()> {
            self.0.insert(address.as_str().to_string(), record);
            Ok(())
        }
    }

    struct MemIdentityStore {
        pair: IdentityKeyPair,
        reg_id: u32,
        known: HashMap<String, IdentityKey>,
    }

    #[derive(Clone, Copy)]
    enum PendingIdentityCall {
        Trust,
        Save,
    }

    struct PendingIdentityStore {
        inner: MemIdentityStore,
        call: PendingIdentityCall,
        entered: AtomicBool,
    }

    impl PendingIdentityStore {
        fn new(inner: MemIdentityStore, call: PendingIdentityCall) -> Self {
            Self {
                inner,
                call,
                entered: AtomicBool::new(false),
            }
        }

        fn wait_forever<T>(&self) -> impl Future<Output = T> {
            self.entered.store(true, Ordering::Release);
            futures::future::pending()
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl IdentityKeyStore for PendingIdentityStore {
        async fn get_identity_key_pair(&self) -> error::Result<IdentityKeyPair> {
            self.inner.get_identity_key_pair().await
        }

        async fn get_local_registration_id(&self) -> error::Result<u32> {
            self.inner.get_local_registration_id().await
        }

        async fn save_identity(
            &mut self,
            address: &ProtocolAddress,
            identity: &IdentityKey,
        ) -> error::Result<IdentityChange> {
            if matches!(self.call, PendingIdentityCall::Save) {
                self.wait_forever().await
            } else {
                self.inner.save_identity(address, identity).await
            }
        }

        async fn is_trusted_identity(
            &self,
            address: &ProtocolAddress,
            identity: &IdentityKey,
            direction: Direction,
        ) -> error::Result<bool> {
            if matches!(self.call, PendingIdentityCall::Trust) {
                self.wait_forever().await
            } else {
                self.inner
                    .is_trusted_identity(address, identity, direction)
                    .await
            }
        }

        async fn get_identity(
            &self,
            address: &ProtocolAddress,
        ) -> error::Result<Option<IdentityKey>> {
            self.inner.get_identity(address).await
        }
    }
    impl MemIdentityStore {
        fn new(pair: IdentityKeyPair, reg_id: u32) -> Self {
            Self {
                pair,
                reg_id,
                known: HashMap::new(),
            }
        }
    }
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl IdentityKeyStore for MemIdentityStore {
        async fn get_identity_key_pair(&self) -> error::Result<IdentityKeyPair> {
            Ok(self.pair.clone())
        }
        async fn get_local_registration_id(&self) -> error::Result<u32> {
            Ok(self.reg_id)
        }
        async fn save_identity(
            &mut self,
            address: &ProtocolAddress,
            identity: &IdentityKey,
        ) -> error::Result<IdentityChange> {
            let changed = self
                .known
                .get(address.as_str())
                .is_some_and(|k| k != identity);
            self.known.insert(address.as_str().to_string(), *identity);
            Ok(IdentityChange::from_changed(changed))
        }
        async fn is_trusted_identity(
            &self,
            _address: &ProtocolAddress,
            _identity: &IdentityKey,
            _direction: Direction,
        ) -> error::Result<bool> {
            Ok(true)
        }
        async fn get_identity(
            &self,
            address: &ProtocolAddress,
        ) -> error::Result<Option<IdentityKey>> {
            Ok(self.known.get(address.as_str()).copied())
        }
    }

    struct MemPreKeyStore(HashMap<PreKeyId, PreKeyRecord>);
    impl MemPreKeyStore {
        fn new() -> Self {
            Self(HashMap::new())
        }
    }
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl PreKeyStore for MemPreKeyStore {
        async fn get_pre_key(&self, id: PreKeyId) -> error::Result<PreKeyRecord> {
            self.0
                .get(&id)
                .cloned()
                .ok_or(SignalProtocolError::InvalidPreKeyId)
        }
        async fn save_pre_key(&mut self, id: PreKeyId, record: &PreKeyRecord) -> error::Result<()> {
            self.0.insert(id, record.clone());
            Ok(())
        }
        async fn remove_pre_key(&mut self, id: PreKeyId) -> error::Result<()> {
            self.0.remove(&id);
            Ok(())
        }
    }

    struct MemSignedPreKeyStore(HashMap<SignedPreKeyId, SignedPreKeyRecord>);
    impl MemSignedPreKeyStore {
        fn new() -> Self {
            Self(HashMap::new())
        }
    }
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl SignedPreKeyStore for MemSignedPreKeyStore {
        async fn get_signed_pre_key(
            &self,
            id: SignedPreKeyId,
        ) -> error::Result<SignedPreKeyRecord> {
            self.0
                .get(&id)
                .cloned()
                .ok_or(SignalProtocolError::InvalidSignedPreKeyId)
        }
        async fn save_signed_pre_key(
            &mut self,
            id: SignedPreKeyId,
            record: &SignedPreKeyRecord,
        ) -> error::Result<()> {
            self.0.insert(id, record.clone());
            Ok(())
        }
    }

    struct TestPair {
        alice_addr: ProtocolAddress,
        alice_sessions: MemSessionStore,
        alice_identity: MemIdentityStore,
        bob_addr: ProtocolAddress,
        bob_sessions: MemSessionStore,
        bob_identity: MemIdentityStore,
        bob_prekeys: MemPreKeyStore,
        bob_signed: MemSignedPreKeyStore,
    }

    fn setup_established_session() -> TestPair {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let alice_addr = ProtocolAddress::new("alice".to_string(), 1.into());
        let alice_id = IdentityKeyPair::generate(&mut rng);
        let mut alice_sessions = MemSessionStore::new();
        let mut alice_identity = MemIdentityStore::new(alice_id, 1);

        let bob_addr = ProtocolAddress::new("bob".to_string(), 1.into());
        let bob_id = IdentityKeyPair::generate(&mut rng);
        let bob_identity_key = *bob_id.identity_key();

        let prekey_id: PreKeyId = 1.into();
        let prekey_pair = KeyPair::generate(&mut rng);
        let mut bob_prekeys = MemPreKeyStore::new();

        let signed_id: SignedPreKeyId = 1.into();
        let signed_pair = KeyPair::generate(&mut rng);
        let signed_sig = bob_id
            .private_key()
            .calculate_signature(&signed_pair.public_key.serialize(), &mut rng)
            .expect("signature");

        let mut bob_sessions = MemSessionStore::new();
        let mut bob_identity = MemIdentityStore::new(bob_id, 2);
        let mut bob_signed = MemSignedPreKeyStore::new();

        futures::executor::block_on(async {
            bob_prekeys
                .save_pre_key(prekey_id, &PreKeyRecord::new(prekey_id, &prekey_pair))
                .await
                .expect("save prekey");
            bob_signed
                .save_signed_pre_key(
                    signed_id,
                    &SignedPreKeyRecord::new(
                        signed_id,
                        Timestamp::from_epoch_millis(0),
                        &signed_pair,
                        &signed_sig,
                    ),
                )
                .await
                .expect("save signed prekey");
        });

        let bundle = PreKeyBundle::new(
            2,
            1.into(),
            Some((prekey_id, prekey_pair.public_key)),
            signed_id,
            signed_pair.public_key,
            signed_sig.to_vec(),
            bob_identity_key,
        )
        .expect("valid bundle");

        // Alice processes Bob's bundle → establishes session
        futures::executor::block_on(async {
            process_prekey_bundle(
                &bob_addr,
                &mut alice_sessions,
                &mut alice_identity,
                &bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("process prekey bundle");

            // Alice sends → Bob receives (completes handshake)
            let ct = message_encrypt(
                b"hello",
                &bob_addr,
                &mut alice_sessions,
                &mut alice_identity,
            )
            .await
            .expect("encrypt first message");

            let ct_msg = CiphertextMessage::PreKeySignalMessage(
                PreKeySignalMessage::try_from(ct.serialize())
                    .expect("parse as PreKeySignalMessage"),
            );
            message_decrypt(
                &ct_msg,
                &alice_addr,
                &mut bob_sessions,
                &mut bob_identity,
                &mut bob_prekeys,
                &bob_signed,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("decrypt first message");
        });

        TestPair {
            alice_addr,
            alice_sessions,
            alice_identity,
            bob_addr,
            bob_sessions,
            bob_identity,
            bob_prekeys,
            bob_signed,
        }
    }

    #[test]
    fn closed_receiver_chain_can_consume_a_persisted_skipped_key() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        futures::executor::block_on(async {
            let reply = message_encrypt(
                b"ratchet reply",
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
            )
            .await
            .expect("encrypt reply");
            let CiphertextMessage::SignalMessage(reply) = reply else {
                panic!("established responder sends a signal message");
            };
            message_decrypt_signal(
                &reply,
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
                &mut rng,
            )
            .await
            .expect("decrypt reply");

            let first = message_encrypt(
                b"first",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt first");
            let second = message_encrypt(
                b"second",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt second");
            let CiphertextMessage::SignalMessage(first) = first else {
                panic!("pending pre-key was acknowledged");
            };
            let CiphertextMessage::SignalMessage(second) = second else {
                panic!("pending pre-key was acknowledged");
            };

            message_decrypt_signal(
                &second,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut rng,
            )
            .await
            .expect("decrypt out of order");

            let stored = tp
                .bob_sessions
                .0
                .remove(tp.alice_addr.as_str())
                .expect("stored session");
            let mut components = stored.into_components().expect("components");
            let current = components.current_session.as_mut().expect("current");
            let receiver = current
                .receiver_chains
                .iter_mut()
                .find(|chain| {
                    chain.sender_ratchet_key.as_deref()
                        == Some(first.sender_ratchet_key().serialize().as_slice())
                })
                .expect("matching receiver chain");
            receiver.chain_key.as_mut().expect("chain key").key = None;
            tp.bob_sessions.0.insert(
                tp.alice_addr.as_str().to_owned(),
                SessionRecord::from_components(components).expect("closed-chain record"),
            );

            let decrypted = message_decrypt_signal(
                &first,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut rng,
            )
            .await
            .expect("consume skipped key from closed chain");
            assert_eq!(decrypted.plaintext, b"first");
        });
    }

    #[test]
    fn owned_prekey_decrypt_consumes_the_envelope_on_success() {
        const PLAINTEXT_BYTES: usize = 64 * 1024;

        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let plaintext = vec![0x7a; PLAINTEXT_BYTES];
        let ciphertext = futures::executor::block_on(message_encrypt(
            &plaintext,
            &tp.bob_addr,
            &mut tp.alice_sessions,
            &mut tp.alice_identity,
        ))
        .expect("encrypt owned prekey test message");
        assert!(matches!(
            &ciphertext,
            CiphertextMessage::PreKeySignalMessage(_)
        ));
        let mut ciphertext = OwnedCiphertextMessage::from(ciphertext);

        let decrypted = futures::executor::block_on(message_decrypt_owned(
            &mut ciphertext,
            &tp.alice_addr,
            &mut tp.bob_sessions,
            &mut tp.bob_identity,
            &mut tp.bob_prekeys,
            &tp.bob_signed,
            &mut rng,
            UsePQRatchet::No,
        ))
        .expect("decrypt caller-owned prekey message");

        assert_eq!(decrypted.plaintext, plaintext);
        assert!(!ciphertext.is_available());
    }

    #[test]
    fn owned_prekey_bad_mac_preserves_the_envelope_for_retry() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let ciphertext = futures::executor::block_on(message_encrypt(
            b"authenticated retry",
            &tp.bob_addr,
            &mut tp.alice_sessions,
            &mut tp.alice_identity,
        ))
        .expect("encrypt owned retry test message");
        let CiphertextMessage::PreKeySignalMessage(pre_key) = ciphertext else {
            panic!("pending session must emit a PreKey envelope");
        };
        let mut nested_wire = pre_key.message().serialized().to_vec();
        let mac_byte = nested_wire
            .len()
            .checked_sub(4)
            .expect("test Signal envelope contains a MAC");
        nested_wire[mac_byte] ^= 0xff;
        let nested = SignalMessage::try_from(nested_wire.as_slice())
            .expect("corrupted MAC keeps the nested envelope parseable");
        let corrupted = PreKeySignalMessage::new(
            pre_key.message_version(),
            pre_key.registration_id(),
            pre_key.pre_key_id(),
            pre_key.signed_pre_key_id(),
            *pre_key.base_key(),
            *pre_key.identity_key(),
            nested,
        )
        .expect("rebuild test PreKey envelope");
        let mut corrupted =
            OwnedCiphertextMessage::from(CiphertextMessage::PreKeySignalMessage(corrupted));

        let error = futures::executor::block_on(message_decrypt_owned(
            &mut corrupted,
            &tp.alice_addr,
            &mut tp.bob_sessions,
            &mut tp.bob_identity,
            &mut tp.bob_prekeys,
            &tp.bob_signed,
            &mut rng,
            UsePQRatchet::No,
        ))
        .expect_err("a corrupted nested MAC must fail authentication");

        assert!(matches!(error, SignalProtocolError::BadMac(_)));
        assert!(corrupted.is_available());
    }

    #[test]
    fn cancelled_signal_decrypt_restores_the_ratchet_for_retry() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let ciphertext = futures::executor::block_on(async {
            let reply = message_encrypt(
                b"ack",
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
            )
            .await
            .expect("encrypt reply");
            let CiphertextMessage::SignalMessage(reply) = reply else {
                panic!("established reply must be a SignalMessage");
            };
            message_decrypt_signal(
                &reply,
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
                &mut rng,
            )
            .await
            .expect("decrypt reply");

            let message = message_encrypt(
                b"retry-safe",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt test message");
            let CiphertextMessage::SignalMessage(message) = message else {
                panic!("acknowledged session must emit a SignalMessage");
            };
            message
        });

        let record = tp
            .bob_sessions
            .0
            .remove(tp.alice_addr.as_str())
            .expect("Bob session");
        let before = record.serialize().expect("serialize baseline");
        let mut sessions = DestructiveSessionStore::new(Some(record));
        let mut identity = PendingIdentityStore::new(tp.bob_identity, PendingIdentityCall::Trust);

        let mut decrypt = Box::pin(message_decrypt_signal(
            &ciphertext,
            &tp.alice_addr,
            &mut sessions,
            &mut identity,
            &mut rng,
        ));
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(matches!(decrypt.as_mut().poll(&mut context), Poll::Pending));
        drop(decrypt);
        assert!(identity.entered.load(Ordering::Acquire));

        let restored = sessions.take().expect("cancelled checkout restored");
        assert_eq!(restored.serialize().expect("serialize restored"), before);
        sessions.replace(restored);
        let retried = futures::executor::block_on(message_decrypt_signal(
            &ciphertext,
            &tp.alice_addr,
            &mut sessions,
            &mut identity.inner,
            &mut rng,
        ))
        .expect("redelivery must decrypt");
        assert_eq!(retried.plaintext, b"retry-safe");
    }

    #[test]
    fn cancelled_fresh_prekey_decrypt_discards_the_promoted_session() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let ciphertext = futures::executor::block_on(message_encrypt(
            b"fresh-retry",
            &tp.bob_addr,
            &mut tp.alice_sessions,
            &mut tp.alice_identity,
        ))
        .expect("encrypt prekey test message");
        let CiphertextMessage::PreKeySignalMessage(ciphertext) = ciphertext else {
            panic!("unacknowledged session must emit a PreKeySignalMessage");
        };

        let mut sessions = DestructiveSessionStore::new(None);
        let mut identity = PendingIdentityStore::new(tp.bob_identity, PendingIdentityCall::Save);
        let mut decrypt = Box::pin(message_decrypt_prekey(
            &ciphertext,
            &tp.alice_addr,
            &mut sessions,
            &mut identity,
            &mut tp.bob_prekeys,
            &tp.bob_signed,
            &mut rng,
            UsePQRatchet::No,
        ));
        let waker = futures::task::noop_waker();
        let mut context = Context::from_waker(&waker);
        assert!(matches!(decrypt.as_mut().poll(&mut context), Poll::Pending));
        drop(decrypt);
        assert!(identity.entered.load(Ordering::Acquire));
        assert!(sessions.take().is_none());

        let retried = futures::executor::block_on(message_decrypt_prekey(
            &ciphertext,
            &tp.alice_addr,
            &mut sessions,
            &mut identity.inner,
            &mut tp.bob_prekeys,
            &tp.bob_signed,
            &mut rng,
            UsePQRatchet::No,
        ))
        .expect("redelivery must establish the session");
        assert_eq!(retried.plaintext, b"fresh-retry");
    }

    /// Builds Bob's prekey stores plus a self-signed `PreKeyBundle`, without
    /// establishing any session. Each call uses a fresh identity, so two bundles
    /// for the same address model a peer reinstall.
    #[allow(clippy::type_complexity)]
    fn fresh_bob(
        rng: &mut rand::rngs::StdRng,
    ) -> (
        ProtocolAddress,
        MemSessionStore,
        MemIdentityStore,
        MemPreKeyStore,
        MemSignedPreKeyStore,
        PreKeyBundle,
    ) {
        let bob_addr = ProtocolAddress::new("bob".to_string(), 1.into());
        let bob_id = IdentityKeyPair::generate(rng);
        let bob_identity_key = *bob_id.identity_key();

        let prekey_id: PreKeyId = 1.into();
        let prekey_pair = KeyPair::generate(rng);
        let signed_id: SignedPreKeyId = 1.into();
        let signed_pair = KeyPair::generate(rng);
        let signed_sig = bob_id
            .private_key()
            .calculate_signature(&signed_pair.public_key.serialize(), rng)
            .expect("signature");

        let mut bob_prekeys = MemPreKeyStore::new();
        let mut bob_signed = MemSignedPreKeyStore::new();
        futures::executor::block_on(async {
            bob_prekeys
                .save_pre_key(prekey_id, &PreKeyRecord::new(prekey_id, &prekey_pair))
                .await
                .expect("save prekey");
            bob_signed
                .save_signed_pre_key(
                    signed_id,
                    &SignedPreKeyRecord::new(
                        signed_id,
                        Timestamp::from_epoch_millis(0),
                        &signed_pair,
                        &signed_sig,
                    ),
                )
                .await
                .expect("save signed prekey");
        });

        let bundle = PreKeyBundle::new(
            2,
            1.into(),
            Some((prekey_id, prekey_pair.public_key)),
            signed_id,
            signed_pair.public_key,
            signed_sig.to_vec(),
            bob_identity_key,
        )
        .expect("valid bundle");

        (
            bob_addr,
            MemSessionStore::new(),
            MemIdentityStore::new(bob_id, 2),
            bob_prekeys,
            bob_signed,
            bundle,
        )
    }

    /// `process_prekey_bundle` must report `NewOrUnchanged` on first contact and
    /// `ReplacedExisting` when a later bundle carries a different identity for the
    /// same address (peer reinstall). This is the signal the high-level client
    /// threads up to mirror WA Web `saveIdentity` -> `handleNewIdentity`.
    #[test]
    fn process_prekey_bundle_reports_identity_change() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let (bob_addr, _bs, _bi, _bp, _bsp, bundle1) = fresh_bob(&mut rng);
        let (_bob_addr2, _bs2, _bi2, _bp2, _bsp2, bundle2) = fresh_bob(&mut rng);

        let alice_id = IdentityKeyPair::generate(&mut rng);
        let mut alice_sessions = MemSessionStore::new();
        let mut alice_identity = MemIdentityStore::new(alice_id, 1);

        futures::executor::block_on(async {
            let first = process_prekey_bundle(
                &bob_addr,
                &mut alice_sessions,
                &mut alice_identity,
                &bundle1,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("first bundle");
            assert_eq!(first, IdentityChange::NewOrUnchanged, "first contact");

            let second = process_prekey_bundle(
                &bob_addr,
                &mut alice_sessions,
                &mut alice_identity,
                &bundle2,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("second bundle");
            assert_eq!(
                second,
                IdentityChange::ReplacedExisting,
                "different identity for same address"
            );
        });
    }

    /// `message_decrypt` of a pkmsg reports `NewOrUnchanged` on first contact and
    /// `ReplacedExisting` when the sender's stored identity differs (reinstall),
    /// while still returning the correct plaintext.
    #[test]
    fn decrypt_prekey_reports_identity_change() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        for (preseed_stale, expected) in [
            (false, IdentityChange::NewOrUnchanged),
            (true, IdentityChange::ReplacedExisting),
        ] {
            let alice_addr = ProtocolAddress::new("alice".to_string(), 1.into());
            let alice_id = IdentityKeyPair::generate(&mut rng);
            let mut alice_sessions = MemSessionStore::new();
            let mut alice_identity = MemIdentityStore::new(alice_id, 1);

            let (bob_addr, mut bob_sessions, mut bob_identity, mut bob_prekeys, bob_signed, bundle) =
                fresh_bob(&mut rng);

            futures::executor::block_on(async {
                if preseed_stale {
                    // Bob already knows a different identity for Alice → reinstall.
                    let stale = *IdentityKeyPair::generate(&mut rng).identity_key();
                    bob_identity
                        .save_identity(&alice_addr, &stale)
                        .await
                        .expect("seed stale identity");
                }

                process_prekey_bundle(
                    &bob_addr,
                    &mut alice_sessions,
                    &mut alice_identity,
                    &bundle,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("process bundle");

                let ct =
                    message_encrypt(b"hi", &bob_addr, &mut alice_sessions, &mut alice_identity)
                        .await
                        .expect("encrypt");
                let pkmsg = CiphertextMessage::PreKeySignalMessage(
                    PreKeySignalMessage::try_from(ct.serialize()).expect("parse pkmsg"),
                );

                let res = message_decrypt(
                    &pkmsg,
                    &alice_addr,
                    &mut bob_sessions,
                    &mut bob_identity,
                    &mut bob_prekeys,
                    &bob_signed,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                .expect("decrypt pkmsg");

                assert_eq!(res.plaintext, b"hi".to_vec());
                assert_eq!(res.identity_change, expected);
            });
        }
    }

    /// `process_prekey` must signal session reuse when a pkmsg matches an
    /// already-established session. The caller relies on this to avoid treating
    /// a duplicate/out-of-order pkmsg's (possibly stale) identity as a fresh
    /// rotation and firing a spurious local identity-change reaction.
    #[test]
    fn process_prekey_signals_reuse_for_established_session() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let alice_addr = ProtocolAddress::new("alice".to_string(), 1.into());
        let alice_id = IdentityKeyPair::generate(&mut rng);
        let mut alice_sessions = MemSessionStore::new();
        let mut alice_identity = MemIdentityStore::new(alice_id, 1);

        let (bob_addr, _bs, bob_identity, bob_prekeys, bob_signed, bundle) = fresh_bob(&mut rng);

        futures::executor::block_on(async {
            process_prekey_bundle(
                &bob_addr,
                &mut alice_sessions,
                &mut alice_identity,
                &bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("process bundle");
            let ct = message_encrypt(b"hi", &bob_addr, &mut alice_sessions, &mut alice_identity)
                .await
                .expect("encrypt");
            let pkmsg = PreKeySignalMessage::try_from(ct.serialize()).expect("parse pkmsg");

            let mut record = SessionRecord::new_fresh();
            let (_used, _save, reused_first) = process_prekey(
                &pkmsg,
                &alice_addr,
                &mut record,
                &bob_identity,
                &bob_prekeys,
                &bob_signed,
                UsePQRatchet::No,
            )
            .await
            .expect("first process_prekey");
            assert!(!reused_first, "first pkmsg establishes a new session");

            let (_used2, _save2, reused_again) = process_prekey(
                &pkmsg,
                &alice_addr,
                &mut record,
                &bob_identity,
                &bob_prekeys,
                &bob_signed,
                UsePQRatchet::No,
            )
            .await
            .expect("second process_prekey");
            assert!(
                reused_again,
                "re-processing the same pkmsg must signal session reuse"
            );
        });
    }

    /// P0: MAC verification failure must return `BadMac`, not `InvalidMessage`.
    ///
    /// Establishes a full Alice↔Bob session with a complete round-trip, then
    /// encrypts a Whisper message, corrupts the MAC, and verifies that decryption
    /// produces `BadMac` (not `InvalidMessage`).
    #[test]
    fn decrypt_corrupted_mac_returns_bad_mac() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        futures::executor::block_on(async {
            // Step 1: Bob replies → creates Bob's sending chain
            let bob_reply = message_encrypt(
                b"ack",
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
            )
            .await
            .expect("Bob encrypt reply");

            // Step 2: Alice decrypts Bob's reply.
            // Bob's reply is a SignalMessage (Bob has no pending prekey).
            let bob_msg = SignalMessage::try_from(bob_reply.serialize())
                .expect("Bob's reply should be a SignalMessage");
            message_decrypt_signal(
                &bob_msg,
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
                &mut rng,
            )
            .await
            .expect("Alice should decrypt Bob's reply");

            // Step 3: Alice sends a second message. After receiving Bob's reply,
            // Alice's pending prekey is cleared, so this is a plain SignalMessage.
            let ct = message_encrypt(
                b"secret payload",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("Alice encrypt second message");

            // Verify it's a SignalMessage, not PreKey
            assert!(
                matches!(ct, CiphertextMessage::SignalMessage(_)),
                "Expected SignalMessage after full round-trip, got {:?}",
                ct.message_type()
            );

            // Step 4: Corrupt the MAC (last 8 bytes) without disturbing protobuf
            let raw = ct.serialize();
            let mut corrupted_bytes = raw.to_vec();
            let mac_offset = corrupted_bytes.len() - 4;
            corrupted_bytes[mac_offset] ^= 0xFF;

            let corrupted = SignalMessage::try_from(corrupted_bytes.as_slice())
                .expect("protobuf is intact, only MAC region is modified");

            // This message carries a fresh inbound ratchet key. Authentication
            // failure must not generate the deferred outbound ratchet or consume
            // caller entropy.
            const DECRYPT_RNG_SEED: u64 = 0xD3FE_22ED;
            let mut decrypt_rng =
                <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(DECRYPT_RNG_SEED);
            let mut untouched_rng =
                <rand::rngs::StdRng as rand::SeedableRng>::seed_from_u64(DECRYPT_RNG_SEED);

            // Bob tries to decrypt the corrupted message
            let err = message_decrypt_signal(
                &corrupted,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut decrypt_rng,
            )
            .await
            .expect_err("decryption should fail due to corrupted MAC");

            // Must be BadMac (error code 7), not InvalidMessage (error code 4)
            assert!(
                matches!(err, SignalProtocolError::BadMac(_)),
                "expected BadMac, got: {err}"
            );
            use rand::RngExt as _;
            assert_eq!(decrypt_rng.random::<u64>(), untouched_rng.random::<u64>());
        });
    }

    /// Fast-path rollback: a MAC failure on an in-order message (the path that
    /// now rolls back via the single saved chain key instead of the full
    /// `decrypt_snapshot` clone) must leave the receiver chain untouched, so the
    /// next legitimate message on the same chain still decrypts. A regressed
    /// rollback would leave the chain advanced and surface `DuplicatedMessage`.
    #[test]
    fn inorder_macfail_rolls_back_chain_and_recovers() {
        let mut tp = setup_established_session();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        futures::executor::block_on(async {
            // Bob replies so Alice clears her pending prekey and sends plain
            // SignalMessages from here on.
            let bob_reply = message_encrypt(
                b"ack",
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
            )
            .await
            .expect("Bob encrypt reply");
            let bob_msg =
                SignalMessage::try_from(bob_reply.serialize()).expect("reply is a SignalMessage");
            message_decrypt_signal(
                &bob_msg,
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
                &mut rng,
            )
            .await
            .expect("Alice decrypts reply");

            // Alice sends two consecutive messages on the same sending chain.
            let m1 = message_encrypt(
                b"first",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt m1");
            let m2 = message_encrypt(
                b"second",
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt m2");
            assert!(
                matches!(m2, CiphertextMessage::SignalMessage(_)),
                "m2 should be a SignalMessage"
            );

            // Bob decrypts m1 → advances the receiver chain, so m2 is an in-order
            // message on an existing chain (the fast path).
            let m1_sig = SignalMessage::try_from(m1.serialize()).expect("m1 SignalMessage");
            let p1 = message_decrypt_signal(
                &m1_sig,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut rng,
            )
            .await
            .expect("Bob decrypts m1");
            assert_eq!(p1.plaintext, b"first");

            // Corrupt m2's MAC and decrypt → BadMac, exercising the fast-path rollback.
            let raw = m2.serialize();
            let mut corrupted = raw.to_vec();
            let off = corrupted.len() - 4;
            corrupted[off] ^= 0xFF;
            let corrupted_msg =
                SignalMessage::try_from(corrupted.as_slice()).expect("protobuf intact");
            let err = message_decrypt_signal(
                &corrupted_msg,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut rng,
            )
            .await
            .expect_err("corrupted m2 must fail");
            assert!(
                matches!(err, SignalProtocolError::BadMac(_)),
                "expected BadMac, got: {err}"
            );

            // The legit m2 must still decrypt — proving the chain key was rolled back.
            let m2_sig = SignalMessage::try_from(m2.serialize()).expect("m2 SignalMessage");
            let p2 = message_decrypt_signal(
                &m2_sig,
                &tp.alice_addr,
                &mut tp.bob_sessions,
                &mut tp.bob_identity,
                &mut rng,
            )
            .await
            .expect("Bob decrypts legit m2 after rollback");
            assert_eq!(p2.plaintext, b"second");
        });
    }

    /// Reusing the encrypt buffer must not pin an oversized allocation: a
    /// plaintext larger than MAX_CAPACITY grows the thread-local buffer, which
    /// has to be released after the message is built (the old take+realloc path
    /// did this implicitly).
    #[test]
    fn encrypt_releases_oversized_buffer() {
        let mut tp = setup_established_session();
        futures::executor::block_on(async {
            let big = vec![7u8; 32 * 1024]; // ciphertext far exceeds MAX_CAPACITY (16 KiB)
            message_encrypt(
                &big,
                &tp.bob_addr,
                &mut tp.alice_sessions,
                &mut tp.alice_identity,
            )
            .await
            .expect("encrypt large message");

            // The fix's contract is "buffer must not exceed MAX_CAPACITY after a
            // send". Asserting against MAX (not INITIAL) keeps the test robust:
            // Vec::with_capacity only guarantees a lower bound, so an allocator
            // that rounds up must not flake this. A 32 KiB plaintext without the
            // release leaves the buffer well above MAX (~32 KiB).
            let cap = encryption_buffer_capacity();
            assert!(
                cap <= EncryptionBuffer::MAX_CAPACITY,
                "encrypt buffer should be released after an oversized send, got capacity {cap}"
            );
        });
    }

    /// P1: A session record that exists but has no usable state (no current session,
    /// 0 previous sessions) must return `SessionNotFound`, not `InvalidMessage`.
    #[test]
    fn decrypt_with_empty_session_returns_session_not_found() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let alice_addr = ProtocolAddress::new("alice".to_string(), 1.into());
        let alice_id = IdentityKeyPair::generate(&mut rng);
        let bob_id = IdentityKeyPair::generate(&mut rng);
        let alice_identity_key = *alice_id.identity_key();
        let bob_identity_key = *bob_id.identity_key();

        // Store an empty (degenerate) session record for Alice
        let mut bob_sessions = MemSessionStore::new();
        let mut bob_identity = MemIdentityStore::new(bob_id, 2);

        futures::executor::block_on(async {
            // Store empty session — record exists but has no ratchet state
            bob_sessions
                .store_session(&alice_addr, SessionRecord::new_fresh())
                .await
                .expect("store empty session");

            // Verify the session "exists" in the store
            let loaded = bob_sessions
                .load_session(&alice_addr)
                .await
                .expect("load session");
            assert!(loaded.is_some(), "record should exist");

            // Craft a plausible SignalMessage (it won't actually decrypt, but we
            // need it to reach the session-check code path)
            let ratchet_key = KeyPair::generate(&mut rng).public_key;
            let msg = SignalMessage::new(
                4,
                &[0u8; 32],
                ratchet_key,
                0,
                0,
                b"dummy",
                &alice_identity_key,
                &bob_identity_key,
            )
            .expect("craft SignalMessage");

            let err = message_decrypt_signal(
                &msg,
                &alice_addr,
                &mut bob_sessions,
                &mut bob_identity,
                &mut rng,
            )
            .await
            .expect_err("should fail on empty session");

            assert!(
                matches!(err, SignalProtocolError::SessionNotFound(_)),
                "expected SessionNotFound for degenerate session, got: {err}"
            );
        });
    }
}
