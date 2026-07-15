//
// Copyright 2020-2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::cell::RefCell;

use rand::{CryptoRng, Rng, RngExt};

use crate::crypto::aes_256_cbc_decrypt_into;
use crate::crypto::{DecryptionError as DecryptionErrorCrypto, aes_256_cbc_encrypt_into};
use crate::protocol::SENDERKEY_MESSAGE_CURRENT_VERSION;
use crate::protocol::sender_keys::{SenderKeyState, SenderMessageKey};
use crate::protocol::{
    CiphertextMessageType, KeyPair, Result, SenderKeyDistributionMessage, SenderKeyMessage,
    SenderKeyRecord, SenderKeyStore, SignalProtocolError, consts,
};
use crate::store::sender_key_name::SenderKeyName;

/// Reusable buffer for cryptographic operations (encryption and decryption).
/// Named generically since it's used for both ENCRYPTION_BUFFER and DECRYPTION_BUFFER.
struct CryptoBuffer {
    buffer: Vec<u8>,
}

impl CryptoBuffer {
    const INITIAL_CAPACITY: usize = 1024;
    /// Reuse the buffer across sends, but do not let a one-off large message pin
    /// an oversized allocation on this thread for the rest of the process. Small
    /// messages (the common case) fit under this and reuse the buffer with no
    /// reallocation; anything larger is released back after its result is copied
    /// out. Sized well above a typical message so normal traffic never churns.
    const MAX_RETAINED_CAPACITY: usize = 16 * 1024;

    fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(Self::INITIAL_CAPACITY),
        }
    }

    /// Clears the buffer and returns a mutable reference for writing.
    fn get_buffer(&mut self) -> &mut Vec<u8> {
        self.buffer.clear();
        &mut self.buffer
    }

    /// Takes ownership of the buffer contents, replacing with a fresh pre-allocated buffer.
    /// More efficient than `mem::take` + `reserve` since we swap with an already-allocated buffer.
    fn take_buffer(&mut self) -> Vec<u8> {
        std::mem::replace(&mut self.buffer, Vec::with_capacity(Self::INITIAL_CAPACITY))
    }

    /// Copy the written bytes into a right-sized box, keeping the buffer for the
    /// next (small) message instead of handing away its capacity. A one-off
    /// large message's capacity is released here so it is not retained on this
    /// thread-local for the process lifetime.
    fn copy_out(&mut self) -> Box<[u8]> {
        let out: Box<[u8]> = self.buffer.as_slice().into();
        if self.buffer.capacity() > Self::MAX_RETAINED_CAPACITY {
            self.buffer = Vec::with_capacity(Self::INITIAL_CAPACITY);
        }
        out
    }
}

thread_local! {
    static ENCRYPTION_BUFFER: RefCell<CryptoBuffer> = RefCell::new(CryptoBuffer::new());
    static DECRYPTION_BUFFER: RefCell<CryptoBuffer> = RefCell::new(CryptoBuffer::new());
}

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name`
/// across this call (and any paired SKDM creation) so the load/advance/store
/// of the chain is atomic against concurrent encrypts.
pub async fn group_encrypt<S: SenderKeyStore + ?Sized, R: Rng + CryptoRng>(
    sender_key_store: &mut S,
    sender_key_name: &SenderKeyName,
    plaintext: &[u8],
    csprng: &mut R,
) -> Result<SenderKeyMessage> {
    let mut record = sender_key_store
        .load_sender_key(sender_key_name)
        .await?
        .ok_or_else(|| {
            SignalProtocolError::NoSenderKeyState(format!(
                "no sender key record for group {} sender {}",
                sender_key_name.group_id(),
                sender_key_name.sender_id()
            ))
        })?;

    let sender_key_state = record
        .sender_key_state_mut()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;

    let message_version = sender_key_state
        .message_version()
        .try_into()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;

    let Some(sender_chain_key) = sender_key_state.sender_chain_key() else {
        return Err(SignalProtocolError::InvalidSenderKeySession);
    };

    let (message_keys, next_sender_chain_key) = sender_chain_key.step_with_message_key()?;

    let ciphertext = ENCRYPTION_BUFFER.with(|buffer| {
        let mut buf_wrapper = buffer.borrow_mut();
        {
            let buf = buf_wrapper.get_buffer();
            aes_256_cbc_encrypt_into(plaintext, message_keys.cipher_key(), message_keys.iv(), buf)
                .map_err(|_| {
                    log::error!("outgoing sender key state corrupt for distribution");
                    SignalProtocolError::InvalidSenderKeySession
                })?;
        }
        // Copy out a right-sized ciphertext and keep the reusable buffer, rather
        // than handing away its (1 KiB) capacity and re-allocating one per send.
        // A group skmsg is small, so `take_buffer` here otherwise cost a fresh
        // 1 KiB per send once the production path started sharing this primitive.
        // `copy_out` releases the capacity of a one-off large message so it is
        // not pinned on this thread-local afterward.
        Ok::<Box<[u8]>, SignalProtocolError>(buf_wrapper.copy_out())
    })?;

    let signing_key = sender_key_state
        .signing_key_private()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;

    let skm = SenderKeyMessage::new(
        message_version,
        sender_key_state.chain_id(),
        message_keys.iteration(),
        ciphertext,
        csprng,
        &signing_key,
    )?;

    sender_key_state.set_sender_chain_key(next_sender_chain_key);

    // Outbound advance: this iteration's (key, IV) must never be re-derivable.
    // Iterations below the durable reservation are already covered, so their
    // sends ride the coalesced write-behind; only the send that reaches the
    // ceiling re-reserves and gates the ciphertext on a synchronous flush (which
    // fast-forwards past the reservation after any reload). Decrypt-side advances
    // stay ungated (they re-derive forward). Mirrors the DM counter lease in
    // SessionRecord.
    let spent_iteration = message_keys.iteration();
    if spent_iteration >= record.reserved_iteration() {
        record.reserve_iterations(spent_iteration);
    }

    sender_key_store
        .store_sender_key(sender_key_name, record)
        .await?;

    Ok(skm)
}

fn get_sender_key(state: &mut SenderKeyState, iteration: u32) -> Result<SenderMessageKey> {
    let Some(sender_chain_key) = state.sender_chain_key() else {
        return Err(SignalProtocolError::InvalidSenderKeySession);
    };
    let current_iteration = sender_chain_key.iteration();

    if current_iteration > iteration {
        if let Some(smk) = state.remove_sender_message_key(iteration) {
            return Ok(smk);
        } else {
            log::debug!("SenderKey Duplicate message for iteration: {iteration}");
            return Err(SignalProtocolError::DuplicatedMessage(
                current_iteration,
                iteration,
            ));
        }
    }

    let jump = (iteration - current_iteration) as usize;
    if jump > consts::MAX_FORWARD_JUMPS {
        log::error!(
            "SenderKey Exceeded future message limit: {}, current iteration: {})",
            consts::MAX_FORWARD_JUMPS,
            current_iteration
        );
        return Err(SignalProtocolError::InvalidMessage(
            CiphertextMessageType::SenderKey,
            "message from too far into the future",
        ));
    }

    let mut sender_chain_key = sender_chain_key;

    while sender_chain_key.iteration() < iteration {
        let (message_key, next_chain) = sender_chain_key.step_with_message_key()?;
        state.add_sender_message_key(&message_key);
        sender_chain_key = next_chain;
    }

    let (result_message_key, next_chain) = sender_chain_key.step_with_message_key()?;
    state.set_sender_chain_key(next_chain);
    Ok(result_message_key)
}

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name` so
/// a concurrent SKDM cannot overwrite the ratchet advance and skipped keys.
pub async fn group_decrypt(
    skm_bytes: &[u8],
    sender_key_store: &mut dyn SenderKeyStore,
    sender_key_name: &SenderKeyName,
) -> Result<Vec<u8>> {
    let skm = SenderKeyMessage::try_from(skm_bytes)?;

    let chain_id = skm.chain_id();

    let mut record = sender_key_store
        .load_sender_key(sender_key_name)
        .await?
        .ok_or_else(|| {
            SignalProtocolError::NoSenderKeyState(format!(
                "no sender key record for group {} sender {}",
                sender_key_name.group_id(),
                sender_key_name.sender_id()
            ))
        })?;

    let sender_key_state = match record.sender_key_state_for_chain_id(chain_id) {
        Some(state) => state,
        None => {
            // Expected when the sender rotated their key (WA Web: rotateKey=true),
            // re-registered, or when we missed the SKDM for this chain. Caller
            // handles the typed error by triggering a retry receipt; WA Web
            // emits `SenderKeyExpired` telemetry instead of an error log.
            log::debug!(
                "SenderKey could not find chain ID {} (known chain IDs: {:?})",
                chain_id,
                record.chain_ids_for_logging().collect::<Vec<_>>(),
            );
            return Err(SignalProtocolError::NoSenderKeyState(format!(
                "no sender key state for chain id {} (known chain IDs: {:?})",
                chain_id,
                record.chain_ids_for_logging().collect::<Vec<_>>()
            )));
        }
    };

    let message_version = skm.message_version() as u32;
    if message_version != sender_key_state.message_version() {
        return Err(SignalProtocolError::UnrecognizedMessageVersion(
            message_version,
        ));
    }

    let signing_key = sender_key_state
        .signing_key_verifier()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;
    if !skm.verify_signature_prepared(signing_key)? {
        return Err(SignalProtocolError::SignatureValidationFailed);
    }

    let sender_key = get_sender_key(sender_key_state, skm.iteration())?;

    let plaintext = DECRYPTION_BUFFER.with(|buffer| {
        let mut buf_wrapper = buffer.borrow_mut();
        let buf = buf_wrapper.get_buffer();
        let ciphertext = skm.ciphertext()?;
        if let Err(e) = aes_256_cbc_decrypt_into(
            ciphertext,
            sender_key.cipher_key(),
            sender_key.iv(),
            buf,
        ) {
            match e {
                DecryptionErrorCrypto::BadKeyOrIv => {
                    log::error!(
                        "incoming sender key state corrupt for group {} sender {} (chain ID {chain_id})",
                        sender_key_name.group_id(),
                        sender_key_name.sender_id()
                    );
                    return Err(SignalProtocolError::InvalidSenderKeySession);
                }
                DecryptionErrorCrypto::BadCiphertext(msg) => {
                    log::error!("sender key decryption failed: {msg}");
                    return Err(SignalProtocolError::InvalidMessage(
                        CiphertextMessageType::SenderKey,
                        "decryption failed",
                    ));
                }
            }
        }
        Ok::<Vec<u8>, SignalProtocolError>(buf_wrapper.take_buffer())
    })?;

    sender_key_store
        .store_sender_key(sender_key_name, record)
        .await?;

    Ok(plaintext)
}

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name` so
/// the new distribution state cannot overwrite a concurrent chain advance.
pub async fn process_sender_key_distribution_message(
    sender_key_name: &SenderKeyName,
    skdm: &SenderKeyDistributionMessage,
    sender_key_store: &mut dyn SenderKeyStore,
) -> Result<()> {
    log::debug!(
        "Processing SenderKey distribution for group {} from sender {} with chain ID {}",
        sender_key_name.group_id(),
        sender_key_name.sender_id(),
        skdm.chain_id()
    );

    let mut sender_key_record = sender_key_store
        .load_sender_key(sender_key_name)
        .await?
        .unwrap_or_else(SenderKeyRecord::new_empty);

    sender_key_record.add_sender_key_state(
        skdm.message_version(),
        skdm.chain_id(),
        skdm.iteration(),
        skdm.chain_key(),
        *skdm.signing_key(),
        None,
    )?;
    sender_key_store
        .store_sender_key(sender_key_name, sender_key_record)
        .await?;
    Ok(())
}

/// Build a `SenderKeyDistributionMessage` from the current state of a record.
fn build_skdm_from_record(record: &SenderKeyRecord) -> Result<SenderKeyDistributionMessage> {
    let state = record
        .sender_key_state()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;
    let Some(sender_chain_key) = state.sender_chain_key() else {
        return Err(SignalProtocolError::InvalidSenderKeySession);
    };
    let message_version = state
        .message_version()
        .try_into()
        .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?;

    SenderKeyDistributionMessage::new(
        message_version,
        state.chain_id(),
        sender_chain_key.iteration(),
        *sender_chain_key.seed(),
        state
            .signing_key_public()
            .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?,
    )
}

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name`
/// across this call and the matching encrypt so both use the same key.
pub async fn create_sender_key_distribution_message<R: Rng + CryptoRng>(
    sender_key_name: &SenderKeyName,
    sender_key_store: &mut dyn SenderKeyStore,
    csprng: &mut R,
) -> Result<SenderKeyDistributionMessage> {
    let sender_key_record = sender_key_store.load_sender_key(sender_key_name).await?;

    match sender_key_record {
        Some(record) => build_skdm_from_record(&record),
        None => {
            // libsignal-protocol-java uses 31-bit integers for sender key chain IDs
            let chain_id = (csprng.random::<u32>()) >> 1;
            log::debug!("Creating SenderKey with chain ID {chain_id}");

            let iteration = 0;
            let sender_key: [u8; 32] = csprng.random();
            let signing_key = KeyPair::generate(csprng);
            let mut record = SenderKeyRecord::new_empty();
            record.add_sender_key_state(
                SENDERKEY_MESSAGE_CURRENT_VERSION,
                chain_id,
                iteration,
                &sender_key,
                signing_key.public_key,
                Some(signing_key.private_key),
            )?;
            // Build SKDM before store so we can move ownership
            let skdm = build_skdm_from_record(&record)?;
            sender_key_store
                .store_sender_key(sender_key_name, record)
                .await?;
            Ok(skdm)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::IdentityKeyPair;
    use async_trait::async_trait;
    use std::collections::HashMap;

    #[test]
    fn crypto_buffer_reuses_small_and_releases_oversized() {
        let mut b = CryptoBuffer::new();

        // A small message keeps its (reusable) capacity: no release, no churn.
        b.get_buffer().extend_from_slice(&[0u8; 512]);
        let _ = b.copy_out();
        assert!(b.buffer.capacity() <= CryptoBuffer::MAX_RETAINED_CAPACITY);
        assert!(b.buffer.capacity() >= CryptoBuffer::INITIAL_CAPACITY);

        // A one-off large message grows the buffer, but copy_out releases the
        // excess so it is not pinned on the thread-local afterward.
        b.get_buffer()
            .resize(CryptoBuffer::MAX_RETAINED_CAPACITY * 4, 0);
        assert!(b.buffer.capacity() > CryptoBuffer::MAX_RETAINED_CAPACITY);
        let _ = b.copy_out();
        assert!(
            b.buffer.capacity() <= CryptoBuffer::MAX_RETAINED_CAPACITY,
            "oversized capacity must be released after copy_out"
        );
    }

    struct InMemorySenderKeyStore {
        keys: HashMap<SenderKeyName, SenderKeyRecord>,
    }

    #[async_trait]
    impl SenderKeyStore for InMemorySenderKeyStore {
        async fn store_sender_key(
            &mut self,
            name: &SenderKeyName,
            record: SenderKeyRecord,
        ) -> Result<()> {
            self.keys.insert(name.clone(), record);
            Ok(())
        }

        async fn load_sender_key(&self, name: &SenderKeyName) -> Result<Option<SenderKeyRecord>> {
            Ok(self.keys.get(name).cloned())
        }
    }

    /// Unknown chain IDs must return `NoSenderKeyState` (typed error) so the
    /// caller can trigger a retry receipt. The log at this site is `debug!` —
    /// WA Web treats this as expected (emits `SenderKeyExpired` WAM event, not
    /// an error).
    #[test]
    fn unknown_chain_id_returns_no_sender_key_state() {
        let mut rng = rand::rng();
        let name = SenderKeyName::new("group@g.us".to_string(), "alice.0".to_string());

        let mut alice_store = InMemorySenderKeyStore {
            keys: HashMap::new(),
        };

        let skdm = futures::executor::block_on(create_sender_key_distribution_message(
            &name,
            &mut alice_store,
            &mut rng,
        ))
        .expect("create skdm");

        // Different chain ID — not in the record.
        let unknown_chain_id = skdm.chain_id().wrapping_add(1);
        let signing_key = IdentityKeyPair::generate(&mut rng);
        let skm = SenderKeyMessage::new(
            SENDERKEY_MESSAGE_CURRENT_VERSION,
            unknown_chain_id,
            0,
            vec![0u8; 32].into_boxed_slice(),
            &mut rng,
            signing_key.private_key(),
        )
        .expect("build skm");

        let result =
            futures::executor::block_on(group_decrypt(skm.serialized(), &mut alice_store, &name));

        match result {
            Err(SignalProtocolError::NoSenderKeyState(msg)) => {
                assert!(
                    msg.contains(&unknown_chain_id.to_string()),
                    "error message should reference the unknown chain id: {msg}"
                );
            }
            other => panic!("expected NoSenderKeyState, got {other:?}"),
        }
    }

    use crate::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH;
    use futures::executor::block_on;

    /// THE crash-safety invariant: a reload mid-lease must never re-derive an
    /// iteration that a lost send may already have put on the wire, and a peer
    /// must still decrypt across the resulting gap. Bob sends iteration 0
    /// (reserving a batch), his record is snapshotted, a further send is lost in
    /// the crash, and the reload fast-forwards past the whole reservation.
    #[test]
    fn crash_mid_lease_skips_spent_iterations_and_peer_decrypts() {
        let mut rng = rand::rng();
        let name = SenderKeyName::new("group@g.us".to_string(), "bob.0".to_string());
        let mut bob = InMemorySenderKeyStore {
            keys: HashMap::new(),
        };
        let skdm = block_on(create_sender_key_distribution_message(
            &name, &mut bob, &mut rng,
        ))
        .expect("bob creates his sender key distribution message");
        let mut alice = InMemorySenderKeyStore {
            keys: HashMap::new(),
        };
        block_on(process_sender_key_distribution_message(
            &name, &skdm, &mut alice,
        ))
        .expect("alice processes bob's distribution message");

        // Bob's first send: iteration 0, reserves the first batch.
        let m0 = block_on(group_encrypt(&mut bob, &name, b"m0", &mut rng))
            .expect("bob encrypts m0 at iteration 0");
        assert_eq!(m0.iteration(), 0);
        assert_eq!(
            block_on(group_decrypt(m0.serialized(), &mut alice, &name))
                .expect("alice decrypts m0 at iteration 0"),
            b"m0"
        );

        // Snapshot Bob's durable state (chain at iteration 1, reservation persisted).
        let snapshot = bob
            .keys
            .get(&name)
            .expect("bob's record is stored after his first send")
            .serialize()
            .expect("bob's record serializes for the durable snapshot");

        // A send whose advance never becomes durable (crash before flush).
        let lost = block_on(group_encrypt(&mut bob, &name, b"lost", &mut rng))
            .expect("bob encrypts the send that the crash loses");
        assert_eq!(lost.iteration(), 1);

        // Reload from the snapshot: the current chain fast-forwards past the lease.
        let reloaded = SenderKeyRecord::deserialize(&snapshot)
            .expect("the durable snapshot deserializes on reload");
        bob.keys.insert(name.clone(), reloaded);

        // The next send must NOT reuse iterations 1..batch-1 (the lost one included).
        let after = block_on(group_encrypt(&mut bob, &name, b"after", &mut rng))
            .expect("bob encrypts the first send after the reload");
        assert!(
            after.iteration() >= SENDER_CHAIN_RESERVATION_BATCH,
            "reload reused a possibly-spent iteration: {} < {}",
            after.iteration(),
            SENDER_CHAIN_RESERVATION_BATCH
        );

        // Alice, who last saw iteration 0, still decrypts across the gap
        // (a forward jump well under MAX_FORWARD_JUMPS).
        assert_eq!(
            block_on(group_decrypt(after.serialized(), &mut alice, &name))
                .expect("alice decrypts across the fast-forwarded iteration gap"),
            b"after"
        );
    }

    /// A store that emulates the real signal-cache gate: it records whether each
    /// stored advance was wire-gated and clears the transient flag, so a run of
    /// sends can be counted for gate frequency.
    struct GateCountingStore {
        keys: HashMap<SenderKeyName, SenderKeyRecord>,
        gated: usize,
    }
    #[async_trait]
    impl SenderKeyStore for GateCountingStore {
        async fn store_sender_key(
            &mut self,
            name: &SenderKeyName,
            mut record: SenderKeyRecord,
        ) -> Result<()> {
            if record.is_wire_gated() {
                self.gated += 1;
                record.clear_wire_gated();
            }
            self.keys.insert(name.clone(), record);
            Ok(())
        }
        async fn load_sender_key(&self, name: &SenderKeyName) -> Result<Option<SenderKeyRecord>> {
            Ok(self.keys.get(name).cloned())
        }
    }

    /// The lease must amortize the synchronous flush to one per batch: over
    /// `2*batch + 2` sends, exactly the sends at iterations 0, batch and 2*batch
    /// gate; the rest ride the coalesced write-behind.
    #[test]
    fn lease_amortizes_the_wire_gate() {
        let mut rng = rand::rng();
        let name = SenderKeyName::new("group@g.us".to_string(), "bob.0".to_string());
        let mut bob = GateCountingStore {
            keys: HashMap::new(),
            gated: 0,
        };
        block_on(create_sender_key_distribution_message(
            &name, &mut bob, &mut rng,
        ))
        .expect("bob creates his sender key distribution message");

        let sends = 2 * SENDER_CHAIN_RESERVATION_BATCH + 2;
        for i in 0..sends {
            block_on(group_encrypt(&mut bob, &name, b"x", &mut rng)).unwrap_or_else(|e| {
                panic!("bob's send {i} of {sends} under the lease failed: {e}")
            });
        }
        assert_eq!(
            bob.gated,
            3,
            "expected one gate per batch (iterations 0, {batch}, {two_batch}), got {got}",
            batch = SENDER_CHAIN_RESERVATION_BATCH,
            two_batch = 2 * SENDER_CHAIN_RESERVATION_BATCH,
            got = bob.gated
        );
    }
}
