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
    /// The current sender chain key, held as a `Copy` value instead of in the
    /// protobuf. The chain seed is a `Bytes` in the generated structure, so
    /// keeping it there made every record clone (and the copy-on-write on every
    /// encrypt/decrypt advance) promote that `Bytes` to a shared allocation.
    /// Same source-of-truth-outside-the-protobuf trick as `message_keys`:
    /// `state.sender_chain_key` stays empty in memory, reassembled only at
    /// `as_protobuf`. `None` only for a structurally invalid state.
    sender_chain: Option<SenderChainKey>,
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
        let sender_chain = Some(SenderChainKey::new(iteration, chain_key_arr));
        let state = SenderKeyStateStructure {
            sender_key_id: Some(chain_id),
            // Source of truth is `sender_chain`; the protobuf field stays empty
            // in memory and is reassembled at as_protobuf.
            sender_chain_key: MessageField::none(),
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
            sender_chain,
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
        // Likewise move the chain key out into the Copy field; the seed was
        // validated at deserialize before this runs.
        let sender_chain = state.sender_chain_key.take().and_then(|sc| {
            let seed: [u8; 32] = sc.seed.as_deref()?.try_into().ok()?;
            Some(SenderChainKey::new(sc.iteration.unwrap_or_default(), seed))
        });
        Self {
            state,
            message_keys,
            sender_chain,
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
        self.sender_chain
    }

    pub fn set_sender_chain_key(&mut self, chain_key: SenderChainKey) {
        self.sender_chain = Some(chain_key);
    }

    /// Advance the sender chain up to a reload's reserved iteration ceiling so no
    /// possibly-spent iteration below it stays derivable. Bounded by
    /// `MAX_RESERVATION_FAST_FORWARD`; a target past that is a corrupt
    /// reservation and errors rather than looping the KDF unboundedly. Mirrors
    /// `SessionRecord::fast_forward_sender_chain`.
    pub(crate) fn fast_forward_sender_chain(
        &mut self,
        target: u32,
    ) -> Result<(), SignalProtocolError> {
        let Some(mut chain_key) = self.sender_chain_key() else {
            return Ok(());
        };
        if target.saturating_sub(chain_key.iteration()) > consts::MAX_RESERVATION_FAST_FORWARD {
            return Err(SignalProtocolError::InvalidState(
                "fast_forward_sender_chain",
                "reserved sender-key iteration implausibly far ahead".into(),
            ));
        }
        if chain_key.iteration() >= target {
            return Ok(());
        }
        while chain_key.iteration() < target {
            chain_key = chain_key.next()?;
        }
        self.set_sender_chain_key(chain_key);
        Ok(())
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
            self.state.sender_message_keys.is_empty()
                && self.state.sender_chain_key.as_option().is_none(),
            "backlog and chain key must live only in their Copy/Arc fields; the protobuf copies stay empty"
        );
        let mut state = self.state.clone();
        state.sender_message_keys = self
            .message_keys
            .iter()
            .map(StoredMessageKey::as_protobuf)
            .collect();
        state.sender_chain_key = self
            .sender_chain
            .as_ref()
            .map_or_else(MessageField::none, |c| MessageField::some(c.as_protobuf()));
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
    /// An outbound chain advance not yet known durable. Sender-key message
    /// keys/IVs derive deterministically from the iteration, so the advance
    /// must reach storage before its ciphertext reaches the wire (unlike
    /// decrypt advances, which re-derive forward). Transient — never
    /// serialized; the store layer converts it into flush gating.
    wire_gated: bool,
    /// Durably-reserved iteration ceiling for the current state's sender chain,
    /// mirroring `SessionRecord::reserved_sender_chain_index` for DM. Iterations
    /// below this ceiling are covered by a persisted reservation, so their sends
    /// skip the synchronous pre-wire flush and ride the coalesced write-behind;
    /// only the send that raises the ceiling gates. A reload fast-forwards the
    /// current chain past this ceiling so no possibly-spent iteration is
    /// re-derivable. Reset to 0 on any state change (rotation/promotion), which
    /// forces the next send to re-reserve and gate; never reuses an iteration.
    reserved_iteration: u32,
}

/// Local-only field appended to the serialized record for `reserved_iteration`.
/// The vendored `SenderKeyRecordStructure` proto is untouched; the generated
/// decoder skips this unknown top-level field and `deserialize` scans it out.
/// Matches the field-number scheme `SessionRecord` uses for its DM counterpart.
const RESERVED_ITERATION_FIELD: u32 = 100;

impl SenderKeyRecord {
    /// Replaces the states wholesale, so the wire gate — which belongs to the
    /// advance being replaced — resets with them.
    pub fn set_states_for_testing(&mut self, states: std::collections::VecDeque<SenderKeyState>) {
        self.states = states;
        self.wire_gated = false;
        self.reserved_iteration = 0;
    }

    pub fn new_empty() -> Self {
        Self {
            states: VecDeque::with_capacity(consts::MAX_SENDER_KEY_STATES),
            wire_gated: false,
            reserved_iteration: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Iterations strictly below this ceiling are covered by a durable
    /// reservation and their sends need no synchronous flush.
    pub fn reserved_iteration(&self) -> u32 {
        self.reserved_iteration
    }

    /// Lease a fresh batch of iterations after `spent_iteration` reached the
    /// current ceiling. Marks the record wire-gated: the caller's ciphertext
    /// must not hit the wire until a flush persists the raised ceiling. Mirrors
    /// `SessionRecord::reserve_sender_chain_counters`.
    pub fn reserve_iterations(&mut self, spent_iteration: u32) {
        self.reserved_iteration =
            spent_iteration.saturating_add(consts::SENDER_CHAIN_RESERVATION_BATCH);
        self.wire_gated = true;
    }

    pub fn deserialize(buf: &[u8]) -> Result<SenderKeyRecord, SignalProtocolError> {
        Self::deserialize_inner(buf, None)
    }

    /// A matching live cache proves this snapshot was not recovered after a crash.
    #[doc(hidden)]
    pub fn deserialize_for_store(
        buf: &[u8],
        incarnation: &[u8; 16],
    ) -> Result<SenderKeyRecord, SignalProtocolError> {
        Self::deserialize_inner(buf, Some(incarnation))
    }

    fn deserialize_inner(
        buf: &[u8],
        incarnation: Option<&[u8; 16]>,
    ) -> Result<SenderKeyRecord, SignalProtocolError> {
        let skr = waproto::codec::sender_key_record_decode(buf)
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

        let local_fields =
            super::local_field::decode_local_record_fields(buf, RESERVED_ITERATION_FIELD, || {
                SignalProtocolError::InvalidProtobufEncoding
            })?;
        let reserved_iteration = local_fields.reservation;
        let trusted_reload =
            incarnation.is_some_and(|current| local_fields.incarnation == Some(*current));

        // Only an untrusted snapshot may have spent its still-reserved range.
        if !trusted_reload
            && reserved_iteration > 0
            && let Some(state) = states.front_mut()
        {
            state.fast_forward_sender_chain(reserved_iteration)?;
        }

        Ok(Self {
            states,
            wire_gated: false,
            reserved_iteration,
        })
    }

    /// Flag an outbound chain advance; cleared by the store layer once it
    /// owns the durability gate.
    pub fn mark_wire_gated(&mut self) {
        self.wire_gated = true;
    }

    pub fn is_wire_gated(&self) -> bool {
        self.wire_gated
    }

    pub fn clear_wire_gated(&mut self) {
        self.wire_gated = false;
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
        // Reset the reservation unconditionally. It is a record-level ceiling for
        // whatever chain is current, and this call may have replaced or reordered
        // the current chain. Resetting is always safe (the next send re-reserves
        // and re-gates); keeping a stale ceiling across a chain change is not,
        // since a lower-iteration chain would treat already-covered iterations as
        // durable and re-derive a spent (key, IV). In practice this is a no-op:
        // the sending record only reaches here on first creation (reservation
        // already 0, warm sends reuse the record without re-adding), and receiver
        // records never carry a reservation.
        self.reserved_iteration = 0;
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
        self.serialize_inner(None)
    }

    /// The incarnation prevents clean reloads from looking like crashes.
    #[doc(hidden)]
    pub fn serialize_for_store(
        &self,
        incarnation: &[u8; 16],
    ) -> Result<Vec<u8>, SignalProtocolError> {
        self.serialize_inner(Some(incarnation))
    }

    fn serialize_inner(
        &self,
        incarnation: Option<&[u8; 16]>,
    ) -> Result<Vec<u8>, SignalProtocolError> {
        use buffa::encoding::{Tag, WireType, encode_varint, varint_len};

        let mut buf = waproto::codec::sender_key_record_to_vec(&self.as_protobuf());
        let incarnation = incarnation.filter(|_| self.reserved_iteration > 0);
        let reservation_len = if self.reserved_iteration > 0 {
            2 + varint_len(self.reserved_iteration as u64)
        } else {
            0
        };
        let incarnation_len = incarnation
            .map(|_| super::local_field::STORE_INCARNATION_ENCODED_LEN)
            .unwrap_or(0);
        buf.reserve(reservation_len + incarnation_len);
        // Append the local-only reservation as a top-level field the generated
        // decoder skips. Emitted only when non-zero, so legacy/unreserved records
        // stay byte-identical. Mirrors SessionRecord::serialize_into.
        if self.reserved_iteration > 0 {
            Tag::new(RESERVED_ITERATION_FIELD, WireType::Varint).encode(&mut buf);
            encode_varint(self.reserved_iteration as u64, &mut buf);
        }
        if let Some(incarnation) = incarnation {
            super::local_field::encode_store_incarnation(&mut buf, incarnation);
        }
        Ok(buf)
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
                    + s.sender_chain
                        .map_or(0, |_| std::mem::size_of::<SenderChainKey>())
            })
            .sum()
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
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

    fn record_with_state(chain_id: u32, seed: u8) -> SenderKeyRecord {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let keypair = KeyPair::generate(&mut rng);
        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                chain_id,
                0,
                &[seed; 32],
                keypair.public_key,
                Some(keypair.private_key),
            )
            .expect("state should be valid");
        record
    }

    fn current_iteration(record: &SenderKeyRecord) -> u32 {
        record
            .sender_key_state()
            .expect("test")
            .sender_chain_key()
            .expect("test")
            .iteration()
    }

    /// A reservation survives a serialize/deserialize round-trip, and the reload
    /// fast-forwards the current chain past the reserved ceiling so no
    /// possibly-spent iteration below it stays derivable.
    #[test]
    fn reservation_survives_roundtrip_and_reload_fast_forwards() {
        let mut record = record_with_state(12345, 0x42);
        record.reserve_iterations(0);
        assert_eq!(
            record.reserved_iteration(),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );
        assert_eq!(
            current_iteration(&record),
            0,
            "reserving does not advance the chain"
        );

        let reloaded =
            SenderKeyRecord::deserialize(&record.serialize().expect("test")).expect("test");
        assert_eq!(
            reloaded.reserved_iteration(),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );
        assert_eq!(
            current_iteration(&reloaded),
            consts::SENDER_CHAIN_RESERVATION_BATCH,
            "reload must burn the reserved iterations"
        );
    }

    #[test]
    fn cache_incarnation_separates_clean_reload_from_recovery() {
        let mut record = record_with_state(12345, 0x42);
        record.reserve_iterations(0);
        let incarnation = [0xA1; 16];
        let replacement = [0xB2; 16];
        let bytes = record.serialize_for_store(&incarnation).expect("test");

        let clean = SenderKeyRecord::deserialize_for_store(&bytes, &incarnation).expect("test");
        assert_eq!(current_iteration(&clean), 0);

        let recovered = SenderKeyRecord::deserialize_for_store(&bytes, &replacement).expect("test");
        assert_eq!(
            current_iteration(&recovered),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        let conservative = SenderKeyRecord::deserialize(&bytes).expect("test");
        assert_eq!(
            current_iteration(&conservative),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        let legacy = record.serialize().expect("test");
        let migrated = SenderKeyRecord::deserialize_for_store(&legacy, &incarnation).expect("test");
        assert_eq!(
            current_iteration(&migrated),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        let mut duplicated = bytes;
        crate::protocol::local_field::encode_store_incarnation(&mut duplicated, &incarnation);
        let duplicate_recovery =
            SenderKeyRecord::deserialize_for_store(&duplicated, &incarnation).expect("test");
        assert_eq!(
            current_iteration(&duplicate_recovery),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );
    }

    /// Rotating the current sender-key state (a fresh chain) resets the lease so
    /// the next send re-reserves against the new chain instead of treating stale
    /// iterations as durable.
    #[test]
    fn rotation_resets_the_lease() {
        let mut record = record_with_state(111, 0x42);
        record.reserve_iterations(0);
        assert_eq!(
            record.reserved_iteration(),
            consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let kp = KeyPair::generate(&mut rng);
        record
            .add_sender_key_state(
                3,
                222,
                0,
                &[0x43u8; 32],
                kp.public_key,
                Some(kp.private_key),
            )
            .expect("test");
        assert_eq!(
            record.reserved_iteration(),
            0,
            "rotation must reset the lease"
        );
    }

    /// A record written without the local-only field (an older lib, or an
    /// unreserved record) is byte-identical to the plain generated encoding and
    /// loads with reservation 0; the first send re-reserves.
    #[test]
    fn legacy_record_without_field_loads_with_zero() {
        let record = record_with_state(12345, 0x42);
        let bytes = record.serialize().expect("test");
        assert_eq!(
            bytes,
            record.as_protobuf().encode_to_vec(),
            "an unreserved record appends no field"
        );
        let loaded = SenderKeyRecord::deserialize(&bytes).expect("test");
        assert_eq!(loaded.reserved_iteration(), 0);
        assert_eq!(
            current_iteration(&loaded),
            0,
            "no reservation, no fast-forward"
        );
    }

    /// A reservation implausibly far past the current chain is a corrupt record
    /// and must fail closed at load rather than looping the KDF unboundedly.
    #[test]
    fn corrupt_reservation_is_rejected() {
        use buffa::encoding::{Tag, WireType, encode_varint};

        let record = record_with_state(12345, 0x42);
        let mut bytes = record.serialize().expect("test");
        let corrupt = consts::MAX_RESERVATION_FAST_FORWARD + 1000;
        Tag::new(RESERVED_ITERATION_FIELD, WireType::Varint).encode(&mut bytes);
        encode_varint(corrupt as u64, &mut bytes);
        assert!(
            SenderKeyRecord::deserialize(&bytes).is_err(),
            "an implausibly-far reservation must fail closed"
        );
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
