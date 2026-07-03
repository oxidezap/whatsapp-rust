//
// Copyright 2020-2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::collections::VecDeque;

use buffa::{Message, MessageField};

use hmac::{HmacReset, KeyInit, Mac};
use sha2::Sha256;

use crate::protocol::crypto::hmac_sha256;
use crate::protocol::stores::{
    SenderKeyRecordStructure, SenderKeyStateStructure, sender_key_state_structure,
};
use crate::protocol::{PrivateKey, PublicKey, SignalProtocolError, consts};

/// A distinct error type to keep from accidentally propagating deserialization errors.
#[derive(Debug)]
pub struct InvalidSenderKeySessionError(&'static str);

impl std::fmt::Display for InvalidSenderKeySessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct SenderMessageKey {
    iteration: u32,
    iv: [u8; 16],
    cipher_key: [u8; 32],
    seed: [u8; 32],
}

impl SenderMessageKey {
    pub fn new(iteration: u32, seed: [u8; 32]) -> Self {
        let mut derived = [0u8; 48];
        hkdf::Hkdf::<sha2::Sha256>::new(None, &seed)
            .expand(b"WhisperGroup", &mut derived)
            .expect("valid output length");
        Self {
            iteration,
            seed,
            iv: derived[0..16].try_into().expect("correct iv length"),
            cipher_key: derived[16..48]
                .try_into()
                .expect("correct cipher_key length"),
        }
    }

    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    pub fn iv(&self) -> &[u8] {
        &self.iv
    }

    pub fn cipher_key(&self) -> &[u8] {
        &self.cipher_key
    }
}

/// Backlog entry for a skipped message key: only the (iteration, seed) pair the
/// full [`SenderMessageKey`] is re-derived from on removal. `Copy`, so the
/// `Arc::make_mut` copy-on-write of the backlog is one flat memcpy — with the
/// protobuf element type, the first COW after a cold load promoted one `Bytes`
/// seed (a shared-control-block malloc) per cached key.
#[derive(Debug, Clone, Copy)]
struct StoredMessageKey {
    iteration: u32,
    seed: [u8; 32],
}

impl StoredMessageKey {
    fn from_protobuf(smk: &sender_key_state_structure::SenderMessageKey) -> Self {
        // Seed is validated at deserialization time; fall back to zeroes on corrupt in-memory data.
        Self {
            iteration: smk.iteration.unwrap_or_default(),
            seed: smk
                .seed
                .as_deref()
                .and_then(|b| b.try_into().ok())
                .unwrap_or_default(),
        }
    }

    fn as_protobuf(&self) -> sender_key_state_structure::SenderMessageKey {
        sender_key_state_structure::SenderMessageKey {
            iteration: Some(self.iteration),
            seed: Some(bytes::Bytes::copy_from_slice(&self.seed)),
        }
    }
}

fn seed_to_array(seed: Option<&bytes::Bytes>) -> Result<[u8; 32], SignalProtocolError> {
    let Some(seed) = seed else {
        return Err(SignalProtocolError::InvalidProtobufEncoding);
    };
    seed.as_ref()
        .try_into()
        .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)
}

#[derive(Debug, Clone, Copy)]
pub struct SenderChainKey {
    iteration: u32,
    chain_key: [u8; 32],
}

impl SenderChainKey {
    const MESSAGE_KEY_SEED: u8 = 0x01;
    const CHAIN_KEY_SEED: u8 = 0x02;

    pub(crate) fn new(iteration: u32, chain_key: [u8; 32]) -> Self {
        Self {
            iteration,
            chain_key,
        }
    }

    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    pub fn seed(&self) -> &[u8; 32] {
        &self.chain_key
    }

    pub fn next(&self) -> Result<SenderChainKey, SignalProtocolError> {
        let new_iteration = self.iteration.checked_add(1).ok_or_else(|| {
            SignalProtocolError::InvalidState(
                "sender_chain_key_next",
                "Sender chain is too long".into(),
            )
        })?;

        Ok(SenderChainKey::new(
            new_iteration,
            self.get_derivative(Self::CHAIN_KEY_SEED),
        ))
    }

    pub fn sender_message_key(&self) -> SenderMessageKey {
        SenderMessageKey::new(self.iteration, self.get_derivative(Self::MESSAGE_KEY_SEED))
    }

    /// Compute both sender message key and next chain key in one call, reusing HMAC key setup.
    #[inline]
    pub fn step_with_message_key(&self) -> Result<(SenderMessageKey, Self), SignalProtocolError> {
        let new_iteration = self.iteration.checked_add(1).ok_or_else(|| {
            SignalProtocolError::InvalidState(
                "sender_chain_key_step",
                "Sender chain is too long".into(),
            )
        })?;

        let mut hmac = HmacReset::<Sha256>::new_from_slice(&self.chain_key)
            .expect("HMAC-SHA256 should accept any size key");

        hmac.update(&[Self::MESSAGE_KEY_SEED]);
        let message_key_seed: [u8; 32] = hmac.finalize_reset().into_bytes().into();

        hmac.update(&[Self::CHAIN_KEY_SEED]);
        let next_chain_key: [u8; 32] = hmac.finalize().into_bytes().into();

        let message_key = SenderMessageKey::new(self.iteration, message_key_seed);
        let next_chain = Self {
            iteration: new_iteration,
            chain_key: next_chain_key,
        };

        Ok((message_key, next_chain))
    }

    #[inline]
    fn get_derivative(&self, label: u8) -> [u8; 32] {
        let label = [label];
        hmac_sha256(&self.chain_key, &label)
    }

    pub(crate) fn as_protobuf(&self) -> sender_key_state_structure::SenderChainKey {
        use bytes::Bytes;
        sender_key_state_structure::SenderChainKey {
            iteration: Some(self.iteration),
            seed: Some(Bytes::copy_from_slice(&self.chain_key)),
        }
    }
}

#[derive(Clone)]
pub struct SenderKeyState {
    state: SenderKeyStateStructure,
    /// The cached out-of-order message keys, held behind an `Arc` so cloning the
    /// state (and thus the whole `SenderKeyRecord` on every group load) is a
    /// refcount bump instead of a deep copy of up to `MAX_MESSAGE_KEYS` keys.
    /// The in-order decrypt path never touches it, so a load there clones nothing
    /// even when a prior out-of-order burst left a large backlog; a mutation
    /// (skip-ahead caching or an out-of-order removal) pays one copy-on-write via
    /// `Arc::make_mut`, leaving any sharing clone (the cache's copy) intact.
    /// `state.sender_message_keys` is kept empty in memory; this is the source of
    /// truth, reassembled into the protobuf only at `as_protobuf` (serialization).
    message_keys: std::sync::Arc<Vec<StoredMessageKey>>,
    /// Parsed signing key with its XEdDSA cache pre-derived, memoized so the
    /// per-send signature skips a basepoint multiplication (~18% of a warm
    /// group send when re-derived from bytes every message). Clones carry the
    /// warm value, and the record cache stores this object back after every
    /// send, so the memo persists for the cache lifetime. Never persisted;
    /// rebuilt lazily after a cold load. If a signing-key setter is ever
    /// added, it must reset this memo.
    signing_key_memo: std::sync::OnceLock<PrivateKey>,
    /// Receive-side mirror of `signing_key_memo`: cached verifier whose
    /// Edwards derivations are reused across every incoming message under
    /// this sender key. Same lifecycle rules as above.
    verifying_key_memo: std::sync::OnceLock<crate::core::curve::PreparedVerifyingKey>,
}

// Manual impl with the signing key REDACTED: the protobuf state embeds the
// serialized private signing key, and the previous derive printed it raw
// into any `{:?}` log or panic message.
impl std::fmt::Debug for SenderKeyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SenderKeyState")
            .field("chain_id", &self.chain_id())
            .field(
                "chain_iteration",
                &self.sender_chain_key().map(|c| c.iteration()),
            )
            .field("message_keys", &self.message_keys.len())
            .field("signing_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl SenderKeyState {
    pub fn new(
        _message_version: u8,
        chain_id: u32,
        iteration: u32,
        chain_key: &[u8],
        signature_key: PublicKey,
        signature_private_key: Option<PrivateKey>,
    ) -> Result<SenderKeyState, SignalProtocolError> {
        use bytes::Bytes;
        let chain_key_arr: [u8; 32] = chain_key
            .try_into()
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        let state = SenderKeyStateStructure {
            sender_key_id: Some(chain_id),
            sender_chain_key: MessageField::some(
                SenderChainKey::new(iteration, chain_key_arr).as_protobuf(),
            ),
            sender_signing_key: MessageField::some(sender_key_state_structure::SenderSigningKey {
                public: Some(Bytes::copy_from_slice(&signature_key.serialize())),
                private: signature_private_key
                    .as_ref()
                    .map(|k| Bytes::copy_from_slice(k.serialize().as_ref())),
            }),
            sender_message_keys: vec![],
        };

        let signing_key_memo = std::sync::OnceLock::new();
        if let Some(key) = signature_private_key {
            key.precompute_signing_cache();
            let _ = signing_key_memo.set(key);
        }
        let verifying_key_memo = std::sync::OnceLock::new();
        if signing_key_memo.get().is_none() {
            // Receive-side state (no private key): build the verifier and derive its
            // Edwards entries here, at SKDM processing, once per sender rotation.
            // Send-side states skip the allocation; it builds lazily if ever asked.
            let verifier = crate::core::curve::PreparedVerifyingKey::new(&signature_key);
            verifier.precompute();
            let _ = verifying_key_memo.set(verifier);
        }
        Ok(Self {
            state,
            message_keys: std::sync::Arc::new(Vec::new()),
            signing_key_memo,
            verifying_key_memo,
        })
    }

    pub(crate) fn from_protobuf(mut state: SenderKeyStateStructure) -> Self {
        // Move the backlog out of the protobuf into the shared Arc so the
        // in-memory `state` stays empty; see `message_keys` field docs.
        let message_keys = std::sync::Arc::new(
            std::mem::take(&mut state.sender_message_keys)
                .iter()
                .map(StoredMessageKey::from_protobuf)
                .collect::<Vec<_>>(),
        );
        Self {
            state,
            message_keys,
            signing_key_memo: std::sync::OnceLock::new(),
            verifying_key_memo: std::sync::OnceLock::new(),
        }
    }

    pub fn message_version(&self) -> u32 {
        3
    }

    pub fn chain_id(&self) -> u32 {
        self.state.sender_key_id.unwrap_or_default()
    }

    pub fn sender_chain_key(&self) -> Option<SenderChainKey> {
        let sender_chain = self.state.sender_chain_key.as_option()?;
        let seed: [u8; 32] = sender_chain
            .seed
            .as_deref()
            .unwrap_or_default()
            .try_into()
            .ok()?;
        Some(SenderChainKey::new(
            sender_chain.iteration.unwrap_or_default(),
            seed,
        ))
    }

    pub fn set_sender_chain_key(&mut self, chain_key: SenderChainKey) {
        self.state.sender_chain_key = MessageField::some(chain_key.as_protobuf());
    }

    pub fn signing_key_public(&self) -> Result<PublicKey, InvalidSenderKeySessionError> {
        if let Some(signing_key) = self.state.sender_signing_key.as_option() {
            let public = signing_key
                .public
                .as_ref()
                .ok_or(InvalidSenderKeySessionError("missing public key bytes"))?;
            PublicKey::try_from(&public[..])
                .map_err(|_| InvalidSenderKeySessionError("invalid public signing key"))
        } else {
            Err(InvalidSenderKeySessionError("missing signing key"))
        }
    }

    /// Cached verifier for this sender's signing key; the Edwards
    /// derivations warm on first use and persist with the in-memory state.
    pub fn signing_key_verifier(
        &self,
    ) -> Result<&crate::core::curve::PreparedVerifyingKey, InvalidSenderKeySessionError> {
        if let Some(verifier) = self.verifying_key_memo.get() {
            return Ok(verifier);
        }
        let verifier = crate::core::curve::PreparedVerifyingKey::new(&self.signing_key_public()?);
        // Benign race: concurrent firsts compute the same value.
        let _ = self.verifying_key_memo.set(verifier);
        Ok(self
            .verifying_key_memo
            .get()
            .expect("set on the line above"))
    }

    pub fn signing_key_private(&self) -> Result<PrivateKey, InvalidSenderKeySessionError> {
        if let Some(key) = self.signing_key_memo.get() {
            return Ok(key.clone());
        }
        if let Some(signing_key) = self.state.sender_signing_key.as_option() {
            let private = signing_key
                .private
                .as_ref()
                .ok_or(InvalidSenderKeySessionError("missing private key bytes"))?;
            let key = PrivateKey::deserialize(private)
                .map_err(|_| InvalidSenderKeySessionError("invalid private signing key"))?;
            // Warm BEFORE memoizing: the caller gets a clone, and clones of a
            // cold key would each re-derive the cache; clones of a warm one
            // carry it. Benign race: concurrent firsts compute equal values.
            key.precompute_signing_cache();
            let _ = self.signing_key_memo.set(key.clone());
            Ok(key)
        } else {
            Err(InvalidSenderKeySessionError("missing signing key"))
        }
    }

    /// Test-only: whether the signing-key memo is populated.
    #[cfg(test)]
    pub(crate) fn signing_key_memo_initialized(&self) -> bool {
        self.signing_key_memo.get().is_some()
    }

    pub(crate) fn as_protobuf(&self) -> SenderKeyStateStructure {
        debug_assert!(
            self.state.sender_message_keys.is_empty(),
            "backlog must live only in `message_keys`; the protobuf copy stays empty"
        );
        let mut state = self.state.clone();
        state.sender_message_keys = self
            .message_keys
            .iter()
            .map(StoredMessageKey::as_protobuf)
            .collect();
        state
    }

    pub fn add_sender_message_key(&mut self, sender_message_key: &SenderMessageKey) {
        let keys = std::sync::Arc::make_mut(&mut self.message_keys);
        keys.push(StoredMessageKey {
            iteration: sender_message_key.iteration,
            seed: sender_message_key.seed,
        });
        // AMORTIZED EVICTION: Only prune when exceeding MAX + threshold.
        // This reduces O(n) drain() calls from every insert to once every PRUNE_THRESHOLD inserts.
        let len = keys.len();
        if len > consts::MAX_MESSAGE_KEYS + consts::MESSAGE_KEY_PRUNE_THRESHOLD {
            let excess = len - consts::MAX_MESSAGE_KEYS;
            keys.drain(..excess);
        }
    }

    pub(crate) fn remove_sender_message_key(&mut self, iteration: u32) -> Option<SenderMessageKey> {
        // Find first so a miss (e.g. a duplicate message) returns without the
        // copy-on-write clone that `make_mut` would force.
        let index = self
            .message_keys
            .iter()
            .position(|x| x.iteration == iteration)?;
        let smk = std::sync::Arc::make_mut(&mut self.message_keys).remove(index);
        Some(SenderMessageKey::new(smk.iteration, smk.seed))
    }
}

#[derive(Debug, Clone)]
pub struct SenderKeyRecord {
    states: VecDeque<SenderKeyState>,
}

impl SenderKeyRecord {
    pub fn set_states_for_testing(&mut self, states: std::collections::VecDeque<SenderKeyState>) {
        self.states = states;
    }

    pub fn new_empty() -> Self {
        Self {
            states: VecDeque::with_capacity(consts::MAX_SENDER_KEY_STATES),
        }
    }

    pub fn deserialize(buf: &[u8]) -> Result<SenderKeyRecord, SignalProtocolError> {
        let skr = SenderKeyRecordStructure::decode_from_slice(buf)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;

        let mut states = VecDeque::with_capacity(skr.sender_key_states.len());
        for state in skr.sender_key_states {
            // Validate seeds eagerly so callers get a clear error on corrupt data.
            if let Some(sender_chain) = state.sender_chain_key.as_option() {
                let _ = seed_to_array(sender_chain.seed.as_ref())?;
            }
            for smk in &state.sender_message_keys {
                let _ = seed_to_array(smk.seed.as_ref())?;
            }
            states.push_back(SenderKeyState::from_protobuf(state));
        }
        Ok(Self { states })
    }

    pub fn sender_key_state(&self) -> Result<&SenderKeyState, InvalidSenderKeySessionError> {
        if !self.states.is_empty() {
            return Ok(&self.states[0]);
        }
        Err(InvalidSenderKeySessionError("empty sender key state"))
    }

    pub fn sender_key_state_mut(
        &mut self,
    ) -> Result<&mut SenderKeyState, InvalidSenderKeySessionError> {
        if !self.states.is_empty() {
            return Ok(&mut self.states[0]);
        }
        Err(InvalidSenderKeySessionError("empty sender key state"))
    }

    pub(crate) fn sender_key_state_for_chain_id(
        &mut self,
        chain_id: u32,
    ) -> Option<&mut SenderKeyState> {
        for i in 0..self.states.len() {
            if self.states[i].chain_id() == chain_id {
                return Some(&mut self.states[i]);
            }
        }
        None
    }

    pub(crate) fn chain_ids_for_logging(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        self.states.iter().map(|state| state.chain_id())
    }

    pub fn add_sender_key_state(
        &mut self,
        message_version: u8,
        chain_id: u32,
        iteration: u32,
        chain_key: &[u8],
        signature_key: PublicKey,
        signature_private_key: Option<PrivateKey>,
    ) -> Result<(), SignalProtocolError> {
        let existing_state = self.remove_state(chain_id, signature_key);

        if self.remove_states_with_chain_id(chain_id) > 0 {
            log::warn!(
                "Removed a matching chain_id ({chain_id}) found with a different public key"
            );
        }

        let state = match existing_state {
            None => SenderKeyState::new(
                message_version,
                chain_id,
                iteration,
                chain_key,
                signature_key,
                signature_private_key,
            )?,
            Some(state) => state,
        };

        while self.states.len() >= consts::MAX_SENDER_KEY_STATES {
            self.states.pop_back();
        }

        self.states.push_front(state);
        Ok(())
    }

    /// Remove the state with the matching `chain_id` and `signature_key`.
    ///
    /// Skips any bad protobufs.
    fn remove_state(&mut self, chain_id: u32, signature_key: PublicKey) -> Option<SenderKeyState> {
        let (index, _state) = self.states.iter().enumerate().find(|(_, state)| {
            state.chain_id() == chain_id && state.signing_key_public().ok() == Some(signature_key)
        })?;

        self.states.remove(index)
    }

    /// Returns the number of removed states.
    ///
    /// Skips any bad protobufs.
    fn remove_states_with_chain_id(&mut self, chain_id: u32) -> usize {
        let initial_length = self.states.len();
        self.states.retain(|state| state.chain_id() != chain_id);
        initial_length - self.states.len()
    }

    pub(crate) fn as_protobuf(&self) -> SenderKeyRecordStructure {
        let mut states = Vec::with_capacity(self.states.len());
        for state in &self.states {
            states.push(state.as_protobuf());
        }

        SenderKeyRecordStructure {
            sender_key_states: states,
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, SignalProtocolError> {
        Ok(self.as_protobuf().encode_to_vec())
    }

    /// Estimated in-memory footprint proxy: encoded size of each state's
    /// structure plus the out-of-order message-key backlog (held outside the
    /// protobuf in memory). Size computation only — nothing is cloned or
    /// encoded. Used by per-session memory reports.
    pub fn estimated_size(&self) -> usize {
        let mut cache = buffa::SizeCache::new();
        self.states
            .iter()
            .map(|s| {
                s.state.compute_size(&mut cache) as usize
                    + s.message_keys.len() * std::mem::size_of::<StoredMessageKey>()
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::KeyPair;

    /// Test SenderMessageKey derivation is deterministic
    #[test]
    fn test_sender_message_key_derivation() {
        let seed = [0x42u8; 32];
        let iteration = 10;

        let smk1 = SenderMessageKey::new(iteration, seed);
        let smk2 = SenderMessageKey::new(iteration, seed);

        // Same seed and iteration should produce same keys
        assert_eq!(smk1.iteration(), smk2.iteration());
        assert_eq!(smk1.iv(), smk2.iv());
        assert_eq!(smk1.cipher_key(), smk2.cipher_key());
    }

    /// Test SenderMessageKey produces different keys for different seeds
    #[test]
    fn test_sender_message_key_different_seeds() {
        let seed1 = [0x42u8; 32];
        let seed2 = [0x43u8; 32];

        let smk1 = SenderMessageKey::new(0, seed1);
        let smk2 = SenderMessageKey::new(0, seed2);

        assert_ne!(smk1.iv(), smk2.iv());
        assert_ne!(smk1.cipher_key(), smk2.cipher_key());
    }

    /// Test SenderChainKey iteration and stepping
    #[test]
    fn test_sender_chain_key_stepping() {
        let initial_chain = [0x55u8; 32];
        let sck = SenderChainKey::new(0, initial_chain);

        let sck1 = sck
            .next()
            .expect("sender chain key iteration should succeed");
        let sck2 = sck1
            .next()
            .expect("sender chain key iteration should succeed");
        let sck3 = sck2
            .next()
            .expect("sender chain key iteration should succeed");

        // Verify iteration increments
        assert_eq!(sck.iteration(), 0);
        assert_eq!(sck1.iteration(), 1);
        assert_eq!(sck2.iteration(), 2);
        assert_eq!(sck3.iteration(), 3);

        // Verify seeds change at each step
        assert_ne!(sck.seed(), sck1.seed());
        assert_ne!(sck1.seed(), sck2.seed());
        assert_ne!(sck2.seed(), sck3.seed());
    }

    /// Test SenderChainKey produces correct message keys
    #[test]
    fn test_sender_chain_key_message_key() {
        let chain = [0x55u8; 32];
        let sck = SenderChainKey::new(5, chain);

        let smk = sck.sender_message_key();

        assert_eq!(smk.iteration(), 5);
        assert_eq!(smk.iv().len(), 16);
        assert_eq!(smk.cipher_key().len(), 32);
    }

    /// Test SenderChainKey stepping is deterministic
    #[test]
    fn test_sender_chain_key_determinism() {
        let chain = [0x77u8; 32];

        let sck1 = SenderChainKey::new(0, chain);
        let sck2 = SenderChainKey::new(0, chain);

        let next1 = sck1
            .next()
            .expect("sender chain key iteration should succeed");
        let next2 = sck2
            .next()
            .expect("sender chain key iteration should succeed");

        assert_eq!(next1.seed(), next2.seed());
        assert_eq!(next1.iteration(), next2.iteration());
    }

    /// Test SenderKeyState basic operations
    #[test]
    fn test_sender_key_state_basic() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let state = SenderKeyState::new(3, 12345, 0, &chain_key, keypair.public_key, None)
            .expect("sender key state should be valid");

        assert_eq!(state.chain_id(), 12345);
        assert_eq!(state.message_version(), 3);
        assert!(state.sender_chain_key().is_some());
        assert!(state.signing_key_public().is_ok());
        // Private key was not provided
        assert!(state.signing_key_private().is_err());
    }

    #[test]
    fn test_sender_key_state_rejects_invalid_chain_key_length() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);

        let err = SenderKeyState::new(3, 12345, 0, &[0x42u8; 31], keypair.public_key, None)
            .expect_err("invalid chain key length should fail");

        assert!(matches!(err, SignalProtocolError::InvalidProtobufEncoding));
    }

    /// Test SenderKeyState with private signing key
    #[test]
    fn test_sender_key_state_with_private_key() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let state = SenderKeyState::new(
            3,
            12345,
            0,
            &chain_key,
            keypair.public_key,
            Some(keypair.private_key),
        )
        .expect("sender key state should be valid");

        assert!(state.signing_key_public().is_ok());
        assert!(state.signing_key_private().is_ok());
    }

    #[test]
    fn signing_key_memo_warms_on_first_use_and_survives_clone() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let signing = crate::core::curve::KeyPair::generate(&mut rng);
        let chain_key = [7u8; 32];
        let state = SenderKeyState::new(
            3,
            1,
            0,
            &chain_key,
            signing.public_key,
            Some(signing.private_key),
        )
        .expect("valid inputs");

        // new() received the parsed key: memo pre-populated and pre-warmed.
        assert!(state.signing_key_memo_initialized());
        assert!(
            state
                .signing_key_private()
                .expect("memo key")
                .has_warm_signing_cache()
        );

        // A cold load (protobuf roundtrip) drops the memo; the first
        // signing_key_private() call rebuilds AND warms it, and the clone
        // handed back carries the warm cache.
        let reloaded = SenderKeyState::from_protobuf(state.as_protobuf());
        assert!(!reloaded.signing_key_memo_initialized());
        let key = reloaded.signing_key_private().expect("reloaded key");
        assert!(key.has_warm_signing_cache());
        assert!(reloaded.signing_key_memo_initialized());

        // Clones of the state (the per-send record clone) carry the memo.
        let cloned = reloaded.clone();
        assert!(cloned.signing_key_memo_initialized());
        assert!(
            cloned
                .signing_key_private()
                .expect("cloned key")
                .has_warm_signing_cache()
        );

        // Verifier memo: send-side states (private key present) skip even
        // the allocation; it builds lazily if asked, is seeded eagerly only
        // on receive-side creation, rebuilds after a cold load, and clones
        // carry it.
        assert!(state.verifying_key_memo.get().is_none());
        let _ = state.signing_key_verifier().expect("lazy build works");
        assert!(state.verifying_key_memo.get().is_some());
        let cold = SenderKeyState::from_protobuf(state.as_protobuf());
        assert!(cold.verifying_key_memo.get().is_none());
        let _ = cold.signing_key_verifier().expect("verifier");
        assert!(cold.verifying_key_memo.get().is_some());
        assert!(cold.clone().verifying_key_memo.get().is_some());

        // The memoized key still signs correctly.
        let msg = b"skmsg";
        let sig = key.calculate_signature(msg, &mut rng).expect("sign");
        let public = reloaded.signing_key_public().expect("public key");
        assert!(public.verify_signature(msg, &sig));
    }

    /// Test SenderKeyState chain key operations
    #[test]
    fn test_sender_key_state_chain_key_update() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut state = SenderKeyState::new(
            3,
            12345,
            0,
            &chain_key,
            keypair.public_key,
            Some(keypair.private_key),
        )
        .expect("sender key state should be valid");

        let initial_sck = state
            .sender_chain_key()
            .expect("sender chain key should exist");
        let next_sck = initial_sck
            .next()
            .expect("sender chain key iteration should succeed");

        state.set_sender_chain_key(next_sck);

        let updated_sck = state
            .sender_chain_key()
            .expect("sender chain key should exist");
        assert_eq!(updated_sck.iteration(), 1);
    }

    /// Test SenderKeyState message key storage
    #[test]
    fn test_sender_key_state_message_key_storage() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut state = SenderKeyState::new(
            3,
            12345,
            0,
            &chain_key,
            keypair.public_key,
            Some(keypair.private_key),
        )
        .expect("sender key state should be valid");

        let smk = SenderMessageKey::new(5, [0xAA; 32]);
        state.add_sender_message_key(&smk);

        // Should be able to retrieve it
        let retrieved = state.remove_sender_message_key(5);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.expect("message key should exist").iteration(), 5);

        // Should not find it again
        let not_found = state.remove_sender_message_key(5);
        assert!(not_found.is_none());
    }

    /// Test SenderKeyState message key limit
    #[test]
    fn test_sender_key_state_message_key_limit() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut state = SenderKeyState::new(
            3,
            12345,
            0,
            &chain_key,
            keypair.public_key,
            Some(keypair.private_key),
        )
        .expect("sender key state should be valid");

        // Amortized eviction uses MESSAGE_KEY_PRUNE_THRESHOLD.
        // Eviction triggers when len > MAX_MESSAGE_KEYS + MESSAGE_KEY_PRUNE_THRESHOLD.
        // Add MAX_MESSAGE_KEYS + 100 keys to ensure eviction happens.
        let total_keys = consts::MAX_MESSAGE_KEYS + 100;
        for i in 0..total_keys {
            let smk = SenderMessageKey::new(i as u32, [0xBB; 32]);
            state.add_sender_message_key(&smk);
        }

        // After adding 2100 keys:
        // - At 2051: prune to 2000 (removes first 51 keys: 0-50)
        // - Continue adding keys 2051-2099 (49 more)
        // - Final len = 2049, no second prune since 2049 <= 2050
        // So keys 0-50 (51 keys) should be evicted.
        let evicted_count = consts::MESSAGE_KEY_PRUNE_THRESHOLD + 1; // 51
        for i in 0..evicted_count {
            let not_found = state.remove_sender_message_key(i as u32);
            assert!(
                not_found.is_none(),
                "Key at iteration {} should have been evicted",
                i
            );
        }
    }

    /// The backlog lives in a separate `Arc` while the protobuf copy stays
    /// empty; a serialize/deserialize roundtrip must still carry every key.
    #[test]
    fn serialize_roundtrip_preserves_message_key_backlog() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                12345,
                0,
                &chain_key,
                keypair.public_key,
                Some(keypair.private_key),
            )
            .expect("add_sender_key_state should succeed");

        {
            let state = record.sender_key_state_mut().expect("state exists");
            for i in 0..5u32 {
                state.add_sender_message_key(&SenderMessageKey::new(i, [i as u8; 32]));
            }
            // The protobuf copy must stay empty in memory.
            assert!(state.state.sender_message_keys.is_empty());
        }

        let serialized = record.serialize().expect("serialize");
        let mut deserialized = SenderKeyRecord::deserialize(&serialized).expect("deserialize");

        let state = deserialized.sender_key_state_mut().expect("state exists");
        // After a cold load the backlog lives in the Arc, the protobuf stays empty.
        assert!(state.state.sender_message_keys.is_empty());
        for i in 0..5u32 {
            let smk = state
                .remove_sender_message_key(i)
                .unwrap_or_else(|| panic!("key {i} should survive the roundtrip"));
            assert_eq!(smk.iteration(), i);
        }
    }

    /// Cloning a state is a refcount bump; a later mutation must copy-on-write so
    /// the clone (the in-cache record) is never touched through the loaded copy.
    /// This is the invariant the whole `Arc`-backed backlog relies on.
    #[test]
    fn backlog_mutation_after_clone_is_isolated() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut original =
            SenderKeyState::new(3, 1, 0, &chain_key, keypair.public_key, None).expect("valid");
        original.add_sender_message_key(&SenderMessageKey::new(7, [7u8; 32]));

        // Clone shares the backlog Arc (mirrors the cache keeping its copy while
        // the loaded record is handed out).
        let mut loaded = original.clone();

        // Adding through the loaded copy must not appear in the original.
        loaded.add_sender_message_key(&SenderMessageKey::new(8, [8u8; 32]));
        assert!(original.remove_sender_message_key(8).is_none());

        // Removing through the loaded copy must not drop it from the original.
        assert!(loaded.remove_sender_message_key(7).is_some());
        assert!(
            original.remove_sender_message_key(7).is_some(),
            "the cache's copy must keep its key after the loaded copy removed it"
        );
    }

    /// Test SenderKeyRecord basic operations
    #[test]
    fn test_sender_key_record_basic() {
        let record = SenderKeyRecord::new_empty();
        assert!(record.sender_key_state().is_err());
    }

    /// Test SenderKeyRecord add and retrieve state
    #[test]
    fn test_sender_key_record_add_state() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                12345,
                0,
                &chain_key,
                keypair.public_key,
                Some(keypair.private_key),
            )
            .expect("sender key state should be valid");

        let state = record
            .sender_key_state()
            .expect("sender key state should exist");
        assert_eq!(state.chain_id(), 12345);
    }

    /// Test SenderKeyRecord state limit
    #[test]
    fn test_sender_key_record_state_limit() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let chain_key = [0x42u8; 32];

        let mut record = SenderKeyRecord::new_empty();

        // Add more than MAX_SENDER_KEY_STATES
        for i in 0..(consts::MAX_SENDER_KEY_STATES + 5) {
            let keypair = KeyPair::generate(&mut rng);
            record
                .add_sender_key_state(
                    3,
                    i as u32,
                    0,
                    &chain_key,
                    keypair.public_key,
                    Some(keypair.private_key),
                )
                .expect("sender key state should be valid");
        }

        // Should not have more than MAX_SENDER_KEY_STATES
        let chain_ids: Vec<u32> = record.chain_ids_for_logging().collect();
        assert!(chain_ids.len() <= consts::MAX_SENDER_KEY_STATES);
    }

    /// Test SenderKeyRecord chain ID lookup
    #[test]
    fn test_sender_key_record_chain_id_lookup() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair1 = KeyPair::generate(&mut rng);
        let keypair2 = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                111,
                0,
                &chain_key,
                keypair1.public_key,
                Some(keypair1.private_key),
            )
            .expect("sender key state should be valid");
        record
            .add_sender_key_state(
                3,
                222,
                0,
                &chain_key,
                keypair2.public_key,
                Some(keypair2.private_key),
            )
            .expect("sender key state should be valid");

        // Should find chain 222 (most recent is at front)
        let state = record.sender_key_state_for_chain_id(222);
        assert!(state.is_some());
        assert_eq!(state.expect("state should exist").chain_id(), 222);

        // Should find chain 111
        let state = record.sender_key_state_for_chain_id(111);
        assert!(state.is_some());
        assert_eq!(state.expect("state should exist").chain_id(), 111);

        // Should not find non-existent chain
        let state = record.sender_key_state_for_chain_id(333);
        assert!(state.is_none());
    }

    /// Test SenderKeyRecord serialization roundtrip
    #[test]
    fn test_sender_key_record_serialization() {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let chain_key = [0x42u8; 32];

        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                12345,
                5,
                &chain_key,
                keypair.public_key,
                Some(keypair.private_key),
            )
            .expect("sender key state should be valid");

        let serialized = record.serialize().expect("serialization should succeed");
        let deserialized =
            SenderKeyRecord::deserialize(&serialized).expect("deserialization should succeed");

        let state = deserialized
            .sender_key_state()
            .expect("sender key state should exist");
        assert_eq!(state.chain_id(), 12345);
        assert!(state.sender_chain_key().is_some());
    }

    #[test]
    fn test_sender_key_record_deserialize_rejects_invalid_chain_seed() {
        let record = SenderKeyRecordStructure {
            sender_key_states: vec![SenderKeyStateStructure {
                sender_key_id: Some(12345),
                sender_chain_key: MessageField::some(sender_key_state_structure::SenderChainKey {
                    iteration: Some(0),
                    seed: Some(bytes::Bytes::copy_from_slice(&[0x42; 31])),
                }),
                ..Default::default()
            }],
        };

        let err = SenderKeyRecord::deserialize(&record.encode_to_vec())
            .expect_err("invalid sender chain seed should fail");

        assert!(matches!(err, SignalProtocolError::InvalidProtobufEncoding));
    }

    #[test]
    fn test_sender_key_record_deserialize_rejects_invalid_message_seed() {
        let record = SenderKeyRecordStructure {
            sender_key_states: vec![SenderKeyStateStructure {
                sender_key_id: Some(12345),
                sender_chain_key: MessageField::some(sender_key_state_structure::SenderChainKey {
                    iteration: Some(0),
                    seed: Some(bytes::Bytes::copy_from_slice(&[0x42; 32])),
                }),
                sender_message_keys: vec![sender_key_state_structure::SenderMessageKey {
                    iteration: Some(1),
                    seed: Some(bytes::Bytes::copy_from_slice(&[0x43; 31])),
                }],
                ..Default::default()
            }],
        };

        let err = SenderKeyRecord::deserialize(&record.encode_to_vec())
            .expect_err("invalid sender message seed should fail");

        assert!(matches!(err, SignalProtocolError::InvalidProtobufEncoding));
    }

    /// Test that step_with_message_key produces the same results as
    /// calling sender_message_key() and next() separately
    #[test]
    fn test_step_with_message_key_equivalence() {
        let chain = [0x99u8; 32];
        let sck = SenderChainKey::new(5, chain);

        // Get results using separate calls
        let msg_key_separate = sck.sender_message_key();
        let next_chain_separate = sck.next().expect("next should succeed");

        // Get results using optimized combined call
        let (msg_key_combined, next_chain_combined) = sck
            .step_with_message_key()
            .expect("step_with_message_key should succeed");

        // Verify message keys are identical
        assert_eq!(msg_key_separate.iteration(), msg_key_combined.iteration());
        assert_eq!(msg_key_separate.iv(), msg_key_combined.iv());
        assert_eq!(msg_key_separate.cipher_key(), msg_key_combined.cipher_key());

        // Verify next chain key is identical
        assert_eq!(next_chain_separate.seed(), next_chain_combined.seed());
        assert_eq!(
            next_chain_separate.iteration(),
            next_chain_combined.iteration()
        );
    }

    /// Test step_with_message_key over multiple iterations
    #[test]
    fn test_step_with_message_key_chain() {
        let initial_chain = [0xBBu8; 32];
        let mut chain_separate = SenderChainKey::new(0, initial_chain);
        let mut chain_combined = SenderChainKey::new(0, initial_chain);

        // Step both chains 10 times and verify they stay in sync
        for i in 0..10 {
            let msg_key_sep = chain_separate.sender_message_key();
            chain_separate = chain_separate.next().expect("next should succeed");

            let (msg_key_comb, next_chain) = chain_combined
                .step_with_message_key()
                .expect("step_with_message_key should succeed");
            chain_combined = next_chain;

            // Verify message keys match
            assert_eq!(
                msg_key_sep.cipher_key(),
                msg_key_comb.cipher_key(),
                "cipher_key mismatch at iteration {i}"
            );

            // Verify chain keys match
            assert_eq!(
                chain_separate.seed(),
                chain_combined.seed(),
                "chain key mismatch at iteration {i}"
            );
            assert_eq!(chain_separate.iteration(), chain_combined.iteration());
        }
    }
}
