// SPDX-License-Identifier: MIT
use super::error::Result;
use super::*;
use crate::engine::{self, CryptoProvider};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, Zeroizing};
pub(crate) fn raw_pair(pair: &KeyPair) -> engine::KeyPair {
    engine::KeyPair {
        public: *pair.public_key.public_key_bytes(),
        private: *pair.private_key.serialize(),
    }
}
pub(crate) fn identity_pair(pair: &IdentityKeyPair) -> engine::KeyPair {
    engine::KeyPair {
        public: *pair.identity_key().public_key().public_key_bytes(),
        private: *pair.private_key().serialize(),
    }
}
pub(crate) fn public(raw: &[u8; 32]) -> Result<PublicKey> {
    Ok(PublicKey::from_djb_public_key_bytes(raw)?)
}
#[derive(Clone)]
pub struct RootKey([u8; 32]);
impl RootKey {
    pub fn new(key: [u8; 32]) -> Self {
        Self(key)
    }
    pub fn key(&self) -> &[u8; 32] {
        &self.0
    }
}
#[derive(Clone)]
pub struct ChainKey {
    key: [u8; 32],
    index: u32,
}
impl ChainKey {
    pub fn new(key: [u8; 32], index: u32) -> Self {
        Self { key, index }
    }
    pub fn key(&self) -> &[u8; 32] {
        &self.key
    }
    pub fn index(&self) -> u32 {
        self.index
    }
    pub fn next_chain_key(&self) -> Result<Self> {
        let index = self
            .index
            .checked_add(1)
            .ok_or(engine::Error::CounterExhausted)?;
        Ok(Self {
            key: engine::RustCrypto.hmac_sha256_array(&self.key, &[2]),
            index,
        })
    }
    pub fn message_keys(&self) -> MessageKeyGenerator {
        MessageKeyGenerator {
            seed: engine::RustCrypto.hmac_sha256_array(&self.key, &[1]),
            counter: self.index,
        }
    }
}

pub struct MessageKeyGenerator {
    seed: [u8; 32],
    counter: u32,
}
impl Drop for MessageKeyGenerator {
    fn drop(&mut self) {
        self.seed.zeroize();
    }
}
impl MessageKeyGenerator {
    pub fn generate_keys(&self) -> MessageKeys {
        let mut expanded = Zeroizing::new([0; 80]);
        engine::RustCrypto
            .hkdf_sha256_zero_salt(&self.seed, b"WhisperMessageKeys", expanded.as_mut())
            .expect("80 bytes are within the SHA-256 HKDF output limit");
        let mut cipher_key = [0; 32];
        let mut mac_key = [0; 32];
        let mut iv = [0; 16];
        cipher_key.copy_from_slice(&expanded[..32]);
        mac_key.copy_from_slice(&expanded[32..64]);
        iv.copy_from_slice(&expanded[64..]);
        MessageKeys {
            cipher_key,
            mac_key,
            iv,
            counter: self.counter,
        }
    }
}
pub struct MessageKeys {
    cipher_key: [u8; 32],
    mac_key: [u8; 32],
    iv: [u8; 16],
    counter: u32,
}
impl Drop for MessageKeys {
    fn drop(&mut self) {
        self.cipher_key.zeroize();
        self.mac_key.zeroize();
        self.iv.zeroize();
    }
}
impl MessageKeys {
    pub fn cipher_key(&self) -> &[u8; 32] {
        &self.cipher_key
    }
    pub fn mac_key(&self) -> &[u8; 32] {
        &self.mac_key
    }
    pub fn iv(&self) -> &[u8; 16] {
        &self.iv
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
}
#[derive(Clone)]
pub struct SessionState {
    pub(crate) inner: engine::SessionState,
}
impl SessionState {
    pub fn new(
        version: u32,
        local: &IdentityKey,
        remote: &IdentityKey,
        root: &RootKey,
        base: &PublicKey,
    ) -> Self {
        Self {
            inner: engine::SessionState {
                metadata: Arc::new(engine::SessionMetadata {
                    version,
                    local_identity: *local.public_key().public_key_bytes(),
                    remote_identity: *remote.public_key().public_key_bytes(),
                    root: root.0,
                    previous_counter: 0,
                    local_registration: 0,
                    remote_registration: 0,
                    needs_refresh: false,
                    alice_base_key: Some(*base.public_key_bytes()),
                    alice_base_key_tagged: true,
                    pending_key_exchange: None,
                }),
                sender: None,
                receivers: Vec::new(),
                pending_pre_key: None,
            },
        }
    }
    /// Public ratchet keys and available chain positions for session diagnostics.
    pub fn all_receiver_chain_logging_info(&self) -> Vec<(Vec<u8>, Option<u32>)> {
        self.inner
            .receivers
            .iter()
            .map(|chain| {
                (
                    engine::wire::tagged_key(&chain.ratchet).to_vec(),
                    Some(chain.index),
                )
            })
            .collect()
    }
    pub fn session_version(&self) -> Result<u32> {
        Ok(self.inner.version)
    }
    pub fn local_identity_key(&self) -> Result<IdentityKey> {
        Ok(IdentityKey::new(public(&self.inner.local_identity)?))
    }
    pub fn remote_identity_key(&self) -> Result<Option<IdentityKey>> {
        Ok(Some(IdentityKey::new(public(&self.inner.remote_identity)?)))
    }
    pub fn set_sender_chain(&mut self, key: &KeyPair, chain: &ChainKey) {
        let mut sender = engine::Chain::new(
            *key.public_key.public_key_bytes(),
            Some(*key.private_key.serialize()),
            chain.key,
        );
        sender.index = chain.index;
        self.inner.sender = Some(sender);
    }
    pub fn get_sender_chain_key(&self) -> Result<ChainKey> {
        let chain =
            self.inner.sender.as_ref().ok_or_else(|| {
                SignalProtocolError::InvalidSessionStructure("sender absent".into())
            })?;
        Ok(ChainKey::new(chain.key, chain.index))
    }
    pub fn set_sender_chain_key(&mut self, key: &ChainKey) -> Result<()> {
        let chain =
            self.inner.sender.as_mut().ok_or_else(|| {
                SignalProtocolError::InvalidSessionStructure("sender absent".into())
            })?;
        chain.key = key.key;
        chain.index = key.index;
        chain.exhausted = false;
        Ok(())
    }
    pub fn sender_ratchet_key(&self) -> Result<PublicKey> {
        public(
            &self
                .inner
                .sender
                .as_ref()
                .ok_or_else(|| {
                    SignalProtocolError::InvalidSessionStructure("sender absent".into())
                })?
                .ratchet,
        )
    }
    pub fn sender_ratchet_key_for_logging(&self) -> Result<String> {
        Ok(hex::encode(self.sender_ratchet_key()?.serialize()))
    }
    pub fn get_root_key(&self) -> Result<RootKey> {
        Ok(RootKey::new(self.inner.root))
    }
    pub fn set_root_key(&mut self, key: &RootKey) {
        self.inner.root = key.0;
    }
    pub fn local_registration_id(&self) -> Result<u32> {
        Ok(self.inner.local_registration)
    }
    pub fn remote_registration_id(&self) -> Result<u32> {
        Ok(self.inner.remote_registration)
    }
    pub fn set_local_registration_id(&mut self, id: u32) {
        self.inner.local_registration = id;
    }
    pub fn set_remote_registration_id(&mut self, id: u32) {
        self.inner.remote_registration = id;
    }
    pub fn alice_base_key(&self) -> Result<&[u8]> {
        self.inner
            .alice_base_key
            .as_ref()
            .map(|key| key.as_slice())
            .ok_or_else(|| SignalProtocolError::InvalidSessionStructure("base key absent".into()))
    }
    pub fn unacknowledged_pre_key_message_items(
        &self,
    ) -> Result<Option<UnacknowledgedPreKeyMessageItems>> {
        self.inner
            .pending_pre_key
            .as_ref()
            .map(|p| {
                Ok(UnacknowledgedPreKeyMessageItems {
                    pre_key_id: p.pre_key_id.map(Into::into),
                    signed_pre_key_id: (p.signed_pre_key_id.ok_or_else(|| {
                        SignalProtocolError::InvalidState(
                            "pending prekey",
                            "signed prekey id absent".into(),
                        )
                    })? as u32)
                        .into(),
                    base_key: public(&p.base_key)?,
                })
            })
            .transpose()
    }
    pub fn set_unacknowledged_pre_key_message(
        &mut self,
        pre_key_id: Option<PreKeyId>,
        signed_pre_key_id: SignedPreKeyId,
        base: &PublicKey,
    ) {
        self.inner.pending_pre_key = Some(Arc::new(engine::PendingPreKey {
            pre_key_id: pre_key_id.map(Into::into),
            signed_pre_key_id: Some(signed_pre_key_id.0 as i32),
            base_key: *base.public_key_bytes(),
            kyber_pre_key_id: None,
            kyber_ciphertext: None,
        }));
    }
    pub fn clear_unacknowledged_pre_key_message(&mut self) {
        self.inner.pending_pre_key = None;
    }
    pub fn has_sender_chain(&self) -> bool {
        self.inner.sender.is_some()
    }
}
#[derive(Clone, Debug)]
pub struct UnacknowledgedPreKeyMessageItems {
    pub pre_key_id: Option<PreKeyId>,
    pub signed_pre_key_id: SignedPreKeyId,
    pub base_key: PublicKey,
}
#[derive(Clone, Default)]
pub(crate) struct RecordMetadata {
    pub previous: engine::ArchivedSessions,
    pub reservation: engine::Reservation,
}
#[derive(Clone, Default)]
pub struct SessionRecord {
    pub(crate) inner: RecordMetadata,
    pub(crate) current: Option<SessionState>,
    pending_reservation: bool,
}
pub(crate) enum SessionMatch {
    Current,
    Archived {
        index: usize,
        state: engine::SessionState,
    },
}
// Provider callbacks can unwind. Return moved state before the outer checkout
// hands the record back to its store, without cloning caches on the hot path.
struct SessionOperation<'a> {
    target: &'a mut SessionRecord,
    record: engine::SessionRecord,
}
impl Drop for SessionOperation<'_> {
    fn drop(&mut self) {
        self.target.current = self
            .record
            .current
            .take()
            .map(|inner| SessionState { inner });
        self.target.inner.previous = core::mem::take(&mut self.record.previous);
        self.target.inner.reservation = core::mem::take(&mut self.record.reservation);
    }
}
impl core::fmt::Debug for SessionRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SessionRecord")
            .field("has_current", &self.current.is_some())
            .field("archived", &self.inner.previous.len())
            .finish()
    }
}
impl SessionRecord {
    pub fn new_fresh() -> Self {
        Self::default()
    }
    pub fn new(state: SessionState) -> Self {
        Self {
            current: Some(state),
            ..Self::default()
        }
    }
    pub(crate) fn from_engine(mut inner: engine::SessionRecord) -> Self {
        let current = inner.current.take().map(|inner| SessionState { inner });
        Self {
            inner: RecordMetadata {
                previous: core::mem::take(&mut inner.previous),
                reservation: core::mem::take(&mut inner.reservation),
            },
            current,
            pending_reservation: false,
        }
    }
    pub(crate) fn operate<T>(
        &mut self,
        f: impl FnOnce(&mut engine::SessionRecord) -> engine::Result<T>,
    ) -> engine::Result<T> {
        // Move ownership through the engine without cloning active caches or
        // retaining a second inline Option<SessionState> in every stored record.
        let previous_sender = self
            .current
            .as_ref()
            .and_then(|state| state.inner.sender.as_ref())
            .map(|chain| chain.ratchet);
        let previous_root = self
            .current
            .as_ref()
            .map(|state| Zeroizing::new(state.inner.root));
        let record = engine::SessionRecord {
            current: self.current.take().map(|s| s.inner),
            previous: core::mem::take(&mut self.inner.previous),
            reservation: core::mem::take(&mut self.inner.reservation),
        };
        let mut operation = SessionOperation {
            target: self,
            record,
        };
        let result = f(&mut operation.record);
        let selected_sender = operation
            .record
            .current
            .as_ref()
            .and_then(|state| state.sender.as_ref())
            .map(|chain| chain.ratchet);
        let root_changed = match (previous_root.as_deref(), operation.record.current.as_ref()) {
            (Some(previous), Some(current)) => !bool::from(previous.ct_eq(&current.root)),
            (None, None) => false,
            _ => true,
        };
        if result.is_ok()
            && (previous_sender != selected_sender || root_changed)
            && operation.record.reservation.requires_durable_commit()
        {
            operation.target.pending_reservation = true;
        }
        result
    }
    pub fn alice_base_key(&self) -> Result<Vec<u8>> {
        let state = self.current.as_ref().ok_or_else(|| {
            SignalProtocolError::InvalidSessionStructure("current state absent".into())
        })?;
        let key = state.inner.alice_base_key.as_ref().ok_or_else(|| {
            SignalProtocolError::InvalidSessionStructure("base key absent".into())
        })?;
        Ok(if state.inner.alice_base_key_tagged {
            engine::wire::tagged_key(key).to_vec()
        } else {
            key.to_vec()
        })
    }
    pub fn remote_registration_id(&self) -> Result<u32> {
        self.current
            .as_ref()
            .map(|s| s.inner.remote_registration)
            .ok_or_else(|| {
                SignalProtocolError::InvalidSessionStructure("current state absent".into())
            })
    }
    pub fn local_registration_id(&self) -> Result<u32> {
        self.current
            .as_ref()
            .map(|s| s.inner.local_registration)
            .ok_or_else(|| {
                SignalProtocolError::InvalidSessionStructure("current state absent".into())
            })
    }
    pub fn session_state(&self) -> Option<&SessionState> {
        self.current.as_ref()
    }
    pub fn session_state_mut(&mut self) -> Option<&mut SessionState> {
        self.current.as_mut()
    }
    pub fn previous_session_count(&self) -> usize {
        self.inner.previous.len()
    }
    pub fn previous_session_states(&self) -> impl Iterator<Item = Result<SessionState>> + '_ {
        self.inner.previous.iter().map(|archive| {
            archive
                .decode()
                .map(|inner| SessionState { inner })
                .map_err(Into::into)
        })
    }
    pub fn rebase_lease_after_sender_chain_reset(&mut self) {
        self.inner.reservation.rebase();
        self.pending_reservation |= self.inner.reservation.requires_durable_commit();
    }
    pub fn set_session_state(&mut self, state: SessionState) {
        self.current = Some(state);
        let waived = self.inner.reservation.is_waived();
        self.inner.reservation = engine::Reservation::default();
        if waived {
            self.inner.reservation.waive();
        }
        self.pending_reservation = false;
    }
    pub fn archive_current_state(&mut self) -> Result<()> {
        // The record-level ceiling belongs to this sending chain. Embed its
        // consumed position before the ceiling is reset for another session.
        self.materialize_sender_reservation();
        if let Some(state) = self.current.as_ref() {
            let mut bytes = Vec::new();
            put_bytes(
                &mut bytes,
                2,
                &engine::codec::encode_session_state(&state.inner),
            );
            let mut archive = engine::codec::decode_session_record(&engine::RustCrypto, &bytes)?;
            self.inner.previous.insert(0, archive.previous.remove(0));
            self.current = None;
            self.inner.previous.truncate(engine::MAX_ARCHIVED_SESSIONS);
        }
        let waived = self.inner.reservation.is_waived();
        self.inner.reservation = engine::Reservation::default();
        if waived {
            self.inner.reservation.waive();
        }
        Ok(())
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_engine(engine::codec::decode_session_record(
            &engine::RustCrypto,
            bytes,
        )?))
    }
    pub fn deserialize_for_store(bytes: &[u8], incarnation: &[u8; 16]) -> Result<Self> {
        Ok(Self::from_engine(
            engine::codec::decode_session_record_live(&engine::RustCrypto, bytes, incarnation)?,
        ))
    }
    fn encode_with_token(&self, token: Option<&[u8; 16]>) -> Vec<u8> {
        engine::codec::encode_session_record_parts(
            self.current.as_ref().map(|s| &s.inner),
            &self.inner.previous,
            &self.inner.reservation,
            token,
        )
    }
    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(self.encode_with_token(None))
    }
    pub fn serialize_into(&self, out: &mut Vec<u8>) {
        engine::codec::encode_session_record_parts_into(
            self.current.as_ref().map(|s| &s.inner),
            &self.inner.previous,
            &self.inner.reservation,
            None,
            out,
        );
    }
    pub fn serialize_into_for_store(&self, out: &mut Vec<u8>, incarnation: &[u8; 16]) {
        engine::codec::encode_session_record_parts_into(
            self.current.as_ref().map(|s| &s.inner),
            &self.inner.previous,
            &self.inner.reservation,
            Some(incarnation),
            out,
        );
    }
    /// Opt out of counter reservations for this in-memory object. Any existing
    /// reservation is consumed first, so this does not revive published counters.
    /// Serialization does not carry this policy into another object.
    pub fn waive_counter_lease(&mut self) {
        self.materialize_sender_reservation();
        self.inner.reservation.waive();
        self.pending_reservation = false;
    }
    fn materialize_sender_reservation(&mut self) {
        if let Some(ceiling) = self.inner.reservation.ceiling
            && let Some(state) = self.current.as_mut()
            && let Some(sender) = state.inner.sender.as_mut()
        {
            let gap = ceiling.saturating_sub(sender.index);
            if gap > consts::MAX_RESERVATION_FAST_FORWARD || gap > 0 && sender.exhausted {
                state.inner.sender = None;
            } else {
                while sender.index < ceiling {
                    sender.key = engine::RustCrypto.hmac_sha256_array(&sender.key, &[2]);
                    sender.index += 1;
                }
            }
        }
    }
    pub fn reserve_sender_chain_counters(&mut self, index: u32) {
        if self.inner.reservation.is_waived() {
            return;
        }
        let ceiling = index.saturating_add(consts::SENDER_CHAIN_RESERVATION_BATCH);
        if self
            .inner
            .reservation
            .ceiling
            .is_none_or(|old| old < ceiling)
        {
            self.inner.reservation.ceiling = Some(ceiling);
            self.pending_reservation = true;
        }
    }
    pub fn reserved_sender_chain_index(&self) -> u32 {
        self.inner.reservation.ceiling.unwrap_or(0)
    }
    pub fn has_pending_reservation(&self) -> bool {
        self.pending_reservation
    }
    pub fn clear_pending_reservation(&mut self) {
        self.pending_reservation = false;
    }
    pub fn estimated_heap_bytes(&self) -> usize {
        self.inner.previous.allocation_overhead()
            + self.inner.previous.capacity() * size_of::<engine::ArchivedSession>()
            + self
                .inner
                .previous
                .iter()
                .map(|s| s.encoded().len() + 2 * size_of::<usize>())
                .sum::<usize>()
            + self.current.as_ref().map_or(0, |s| {
                let state = &s.inner;
                size_of::<engine::SessionMetadata>()
                    + 2 * size_of::<usize>()
                    + state.receivers.capacity() * size_of::<engine::Chain>()
                    + state
                        .receivers
                        .iter()
                        .chain(state.sender.iter())
                        .map(|c| {
                            c.skipped.allocation_overhead()
                                + c.skipped.capacity() * size_of::<engine::MessageKey>()
                                + c.skipped
                                    .iter()
                                    .map(engine::MessageKey::extra_heap_bytes)
                                    .sum::<usize>()
                        })
                        .sum::<usize>()
                    + state.pending_key_exchange.as_ref().map_or(0, Vec::capacity)
                    + state.pending_pre_key.as_ref().map_or(0, |pending| {
                        size_of::<engine::PendingPreKey>()
                            + 2 * size_of::<usize>()
                            + pending.kyber_ciphertext.as_ref().map_or(0, Vec::capacity)
                    })
            })
    }
    pub fn estimated_size(&self) -> usize {
        size_of::<Self>() + self.estimated_heap_bytes()
    }
    pub fn has_session_state(&self, version: u32, base_key: &PublicKey) -> Result<bool> {
        Ok(self.matching_session(version, base_key)?.is_some())
    }
    pub(crate) fn matching_session(
        &self,
        version: u32,
        base_key: &PublicKey,
    ) -> Result<Option<SessionMatch>> {
        if self.current.as_ref().is_some_and(|s| {
            s.inner.version == version
                && s.inner.alice_base_key == Some(*base_key.public_key_bytes())
        }) {
            return Ok(Some(SessionMatch::Current));
        }
        for (index, archive) in self.inner.previous.iter().enumerate() {
            let state = archive.decode()?;
            if state.version == version
                && state.alice_base_key == Some(*base_key.public_key_bytes())
            {
                return Ok(Some(SessionMatch::Archived { index, state }));
            }
        }
        Ok(None)
    }
}
fn put_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push((value as u8) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn put_bytes(out: &mut Vec<u8>, field: u32, value: &[u8]) {
    put_varint(out, (u64::from(field) << 3) | 2);
    put_varint(out, value.len() as u64);
    out.extend_from_slice(value);
}
#[derive(Clone)]
pub struct PreKeyRecord {
    pub(crate) inner: engine::PreKeyRecord,
}
impl core::fmt::Debug for PreKeyRecord {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreKeyRecord")
            .field("id", &self.inner.id)
            .finish_non_exhaustive()
    }
}
impl PreKeyRecord {
    pub fn new(id: PreKeyId, key: &KeyPair) -> Self {
        Self {
            inner: engine::PreKeyRecord {
                id: id.0,
                key_pair: raw_pair(key),
            },
        }
    }
    pub fn id(&self) -> Result<PreKeyId> {
        Ok(self.inner.id.into())
    }
    pub fn key_pair(&self) -> Result<KeyPair> {
        Ok(KeyPair::new(
            public(&self.inner.key_pair.public)?,
            PrivateKey::deserialize(&self.inner.key_pair.private)?,
        ))
    }
    pub fn public_key(&self) -> Result<PublicKey> {
        public(&self.inner.key_pair.public)
    }
    pub fn private_key(&self) -> Result<PrivateKey> {
        Ok(PrivateKey::deserialize(&self.inner.key_pair.private)?)
    }
    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(engine::codec::encode_pre_key(&self.inner))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            inner: engine::codec::decode_pre_key(bytes)?,
        })
    }
}
#[derive(Clone)]
pub struct SignedPreKeyRecord {
    pub(crate) inner: engine::SignedPreKeyRecord,
}
pub trait GenericSignedPreKey {
    fn id(&self) -> Result<SignedPreKeyId>;
    fn key_pair(&self) -> Result<KeyPair>;
    fn public_key(&self) -> Result<PublicKey>;
    fn private_key(&self) -> Result<PrivateKey>;
    fn signature(&self) -> Result<Vec<u8>>;
    fn timestamp(&self) -> Result<u64>;
    fn serialize(&self) -> Result<Vec<u8>>;
}
impl SignedPreKeyRecord {
    pub fn new(
        id: SignedPreKeyId,
        timestamp: impl Into<u64>,
        key: &KeyPair,
        signature: &[u8],
    ) -> Result<Self> {
        Ok(Self {
            inner: engine::SignedPreKeyRecord {
                id: id.0,
                key_pair: raw_pair(key),
                signature: signature
                    .try_into()
                    .map_err(|_| SignalProtocolError::InvalidArgument("signature length".into()))?,
                timestamp: timestamp.into(),
            },
        })
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            inner: engine::codec::decode_signed_pre_key(bytes)?,
        })
    }
}
impl GenericSignedPreKey for SignedPreKeyRecord {
    fn id(&self) -> Result<SignedPreKeyId> {
        Ok(self.inner.id.into())
    }
    fn key_pair(&self) -> Result<KeyPair> {
        Ok(KeyPair::new(
            public(&self.inner.key_pair.public)?,
            PrivateKey::deserialize(&self.inner.key_pair.private)?,
        ))
    }
    fn public_key(&self) -> Result<PublicKey> {
        public(&self.inner.key_pair.public)
    }
    fn private_key(&self) -> Result<PrivateKey> {
        Ok(PrivateKey::deserialize(&self.inner.key_pair.private)?)
    }
    fn signature(&self) -> Result<Vec<u8>> {
        Ok(self.inner.signature.to_vec())
    }
    fn timestamp(&self) -> Result<u64> {
        Ok(self.inner.timestamp)
    }
    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(engine::codec::encode_signed_pre_key(&self.inner))
    }
}
/// Caller-owned prepared group crypto, independent of the stored record format.
/// Retain it with one logical sender-key cache entry and drop it when that entry
/// is evicted. It can retain private signing material, is bounded to the sender
/// history limit, and deliberately has no serialization or diagnostic formatter.
#[derive(Clone, Default)]
pub struct SenderKeyCryptoContext {
    inner: Option<Arc<group_crypto::GroupCryptoCache>>,
}
#[derive(Clone, Default)]
pub struct SenderKeyRecord {
    pub(crate) inner: engine::SenderKeyRecord,
    wire_gated: bool,
    group_crypto: Option<Arc<group_crypto::GroupCryptoCache>>,
}
impl SenderKeyRecord {
    pub fn new_empty() -> Self {
        Self::default()
    }
    /// Retain prepared crypto across component-backed store handoffs.
    pub fn crypto_context(&self) -> SenderKeyCryptoContext {
        SenderKeyCryptoContext {
            inner: self.group_crypto.clone(),
        }
    }
    /// Prepare the current state's roles without advancing any protocol state.
    pub fn prepare_crypto_context(&mut self) -> SenderKeyCryptoContext {
        self.warm_current_signer();
        self.warm_current_verifier();
        self.crypto_context()
    }
    /// Reuse prepared work for matching keys, discarding unrelated cached keys.
    /// No counters, message keys, wire gate or reservation state are restored.
    pub fn restore_crypto_context(&mut self, context: &SenderKeyCryptoContext) {
        self.group_crypto = context.inner.clone();
        self.prune_prepared_crypto();
    }
    /// Opt out of reservations for this in-memory record after consuming its
    /// current lease. The waiver is deliberately not serialized.
    pub fn waive_counter_lease(&mut self) -> Result<()> {
        if let (Some(ceiling), Some(state)) = (
            self.inner.reservation.ceiling,
            self.inner.states.first_mut(),
        ) {
            if ceiling.saturating_sub(state.iteration) > engine::PEER_FUTURE_LIMIT {
                return Err(
                    engine::Error::Encoding("sender reservation burn exceeds limit").into(),
                );
            }
            while state.iteration < ceiling {
                state.chain_key = engine::RustCrypto.hmac_sha256_array(&state.chain_key, &[2]);
                state.iteration += 1;
            }
        }
        self.inner.reservation.waive();
        self.clear_wire_gated();
        Ok(())
    }
    pub fn reserved_iteration(&self) -> u32 {
        self.inner.reservation.ceiling.unwrap_or(0)
    }
    pub fn reserve_iterations(&mut self, index: u32) {
        if self.inner.reservation.is_waived() {
            return;
        }
        let ceiling = index.saturating_add(consts::SENDER_CHAIN_RESERVATION_BATCH);
        if self
            .inner
            .reservation
            .ceiling
            .is_none_or(|old| old < ceiling)
        {
            self.inner.reservation.ceiling = Some(ceiling);
            self.mark_wire_gated();
        }
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self {
            inner: engine::codec::decode_sender_key_record(&engine::RustCrypto, bytes)?,
            wire_gated: false,
            group_crypto: None,
        })
    }
    pub fn deserialize_for_store(bytes: &[u8], token: &[u8; 16]) -> Result<Self> {
        Ok(Self {
            inner: engine::codec::decode_sender_key_record_live(&engine::RustCrypto, bytes, token)?,
            wire_gated: false,
            group_crypto: None,
        })
    }
    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(engine::codec::encode_sender_key_record(&self.inner))
    }
    pub(crate) fn prune_prepared_crypto(&mut self) {
        let states = &self.inner.states;
        group_crypto::GroupCryptoCache::prune(&mut self.group_crypto, |public, private| {
            states.iter().any(|state| {
                state.signing_public == *public
                    && (private.is_none()
                        || group_crypto::private_keys_match(
                            state.signing_private.as_ref(),
                            private,
                        ))
            })
        });
    }
    pub(crate) fn warm_current_signer(&mut self) {
        self.prune_prepared_crypto();
        if let Some(state) = self.inner.states.first()
            && let Some(private) = &state.signing_private
        {
            group_crypto::GroupCryptoCache::prepare(
                &mut self.group_crypto,
                &state.signing_public,
                Some(private),
            );
        }
    }
    pub(crate) fn warm_current_verifier(&mut self) {
        self.prune_prepared_crypto();
        if let Some(state) = self.inner.states.first() {
            group_crypto::GroupCryptoCache::prepare(
                &mut self.group_crypto,
                &state.signing_public,
                None,
            );
        }
    }
    pub(crate) fn prepared_signer(
        &mut self,
        index: usize,
    ) -> Result<group_crypto::PreparedGroupProvider> {
        self.prune_prepared_crypto();
        let state = self
            .inner
            .states
            .get(index)
            .ok_or(SignalProtocolError::InvalidSenderKeySession)?;
        let private = state
            .signing_private
            .as_ref()
            .ok_or(engine::Error::MissingKey)?;
        Ok(group_crypto::GroupCryptoCache::prepare(
            &mut self.group_crypto,
            &state.signing_public,
            Some(private),
        ))
    }
    pub(crate) fn prepared_verifier(
        &mut self,
        index: usize,
    ) -> Result<group_crypto::PreparedGroupProvider> {
        self.prune_prepared_crypto();
        let state = self
            .inner
            .states
            .get(index)
            .ok_or(SignalProtocolError::InvalidSenderKeySession)?;
        Ok(group_crypto::GroupCryptoCache::prepare(
            &mut self.group_crypto,
            &state.signing_public,
            None,
        ))
    }
    pub fn serialize_for_store(&self, token: &[u8; 16]) -> Result<Vec<u8>> {
        Ok(engine::codec::encode_sender_key_record_for_store(
            &self.inner,
            token,
        ))
    }
    pub fn is_empty(&self) -> bool {
        self.inner.states.is_empty()
    }
    pub fn estimated_size(&self) -> usize {
        size_of::<Self>() + self.estimated_heap_bytes()
    }
    pub fn mark_wire_gated(&mut self) {
        self.wire_gated = true;
    }
    pub fn clear_wire_gated(&mut self) {
        self.wire_gated = false;
    }
    pub fn is_wire_gated(&self) -> bool {
        self.wire_gated
    }
    pub fn estimated_heap_bytes(&self) -> usize {
        self.inner.states.capacity() * size_of::<engine::SenderKeyState>()
            + self
                .inner
                .states
                .iter()
                .map(|s| {
                    s.skipped.allocation_overhead()
                        + s.skipped.capacity() * size_of::<engine::SenderMessageKey>()
                })
                .sum::<usize>()
            + self
                .group_crypto
                .as_ref()
                .map_or(0, |cache| cache.estimated_heap_bytes())
    }
    pub fn add_sender_key_state(
        &mut self,
        version: u8,
        id: u32,
        iteration: u32,
        chain_key: &[u8],
        public: PublicKey,
        private: Option<PrivateKey>,
    ) -> Result<()> {
        if version != 3 {
            return Err(SignalProtocolError::UnrecognizedMessageVersion(
                version.into(),
            ));
        }
        let chain_key = Zeroizing::new(
            <[u8; 32]>::try_from(chain_key)
                .map_err(|_| SignalProtocolError::InvalidSenderKeySession)?,
        );
        let same_active = self.inner.states.first().is_some_and(|state| {
            state.id == id && state.signing_public == *public.public_key_bytes()
        });
        let retained = self
            .inner
            .states
            .iter()
            .position(|state| state.id == id && state.signing_public == *public.public_key_bytes())
            .map(|index| self.inner.states.remove(index));
        self.inner.states.retain(|state| state.id != id);
        let state = retained.unwrap_or_else(|| engine::SenderKeyState {
            id,
            iteration,
            chain_key: *chain_key,
            signing_public: *public.public_key_bytes(),
            signing_private: private.map(|p| *p.serialize()),
            skipped: engine::SharedDeque::new(),
        });
        self.inner.states.insert(0, state);
        self.inner.states.truncate(engine::MAX_SENDER_STATES);
        if !same_active {
            // Reservations and publication proof belong to the active chain.
            self.inner.reservation.reset_chain();
            if self.inner.states[0].signing_private.is_some() {
                self.mark_wire_gated();
            }
        }
        if self.inner.states[0].signing_private.is_some() {
            self.warm_current_signer();
        } else {
            self.warm_current_verifier();
        }
        Ok(())
    }
    pub fn sender_key_state(&self) -> Result<SenderKeyState<'_>> {
        self.inner
            .states
            .first()
            .map(SenderKeyState)
            .ok_or_else(|| SignalProtocolError::NoSenderKeyState("empty record".into()))
    }
}
pub struct SenderKeyState<'a>(&'a engine::SenderKeyState);
impl SenderKeyState<'_> {
    pub fn chain_id(&self) -> u32 {
        self.0.id
    }
    pub fn sender_key_id(&self) -> Result<u32> {
        Ok(self.0.id)
    }
    pub fn sender_chain_key(&self) -> Option<SenderChainKey> {
        Some(SenderChainKey {
            iteration: self.0.iteration,
            seed: self.0.chain_key,
        })
    }
    pub fn signing_key_public(&self) -> Result<PublicKey> {
        public(&self.0.signing_public)
    }
}
pub struct SenderChainKey {
    iteration: u32,
    seed: [u8; 32],
}
impl SenderChainKey {
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }
    pub fn sender_message_key(&self) -> SenderMessageKey {
        let seed = Zeroizing::new(engine::RustCrypto.hmac_sha256_array(&self.seed, &[1]));
        let mut expanded = Zeroizing::new([0; 48]);
        engine::RustCrypto
            .hkdf_sha256_zero_salt(seed.as_ref(), b"WhisperGroup", expanded.as_mut())
            .expect("48 bytes are within the SHA-256 HKDF output limit");
        let mut iv = [0; 16];
        let mut cipher_key = [0; 32];
        iv.copy_from_slice(&expanded[..16]);
        cipher_key.copy_from_slice(&expanded[16..]);
        SenderMessageKey {
            cipher_key,
            iv,
            iteration: self.iteration,
        }
    }
}
pub struct SenderMessageKey {
    cipher_key: [u8; 32],
    iv: [u8; 16],
    iteration: u32,
}
impl Drop for SenderMessageKey {
    fn drop(&mut self) {
        self.cipher_key.zeroize();
        self.iv.zeroize();
    }
}
impl SenderMessageKey {
    pub fn cipher_key(&self) -> &[u8] {
        &self.cipher_key
    }
    pub fn iv(&self) -> &[u8] {
        &self.iv
    }
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
}
#[derive(Clone, Debug)]
pub struct PreKeyBundle {
    pub(crate) registration: u32,
    pub(crate) device: DeviceId,
    pub(crate) pre_key: Option<(PreKeyId, PublicKey)>,
    pub(crate) signed_id: SignedPreKeyId,
    pub(crate) signed_public: PublicKey,
    pub(crate) signature: Vec<u8>,
    pub(crate) identity: IdentityKey,
}
impl PreKeyBundle {
    #[allow(clippy::too_many_arguments)]
    pub fn new<S: Into<Vec<u8>>>(
        registration: u32,
        device: DeviceId,
        pre_key: Option<(PreKeyId, PublicKey)>,
        signed_id: SignedPreKeyId,
        signed_public: PublicKey,
        signature: S,
        identity: IdentityKey,
    ) -> Result<Self> {
        let signature = signature.into();
        if signature.len() != 64 {
            return Err(SignalProtocolError::InvalidArgument(
                "signature length".into(),
            ));
        }
        Ok(Self {
            registration,
            device,
            pre_key,
            signed_id,
            signed_public,
            signature,
            identity,
        })
    }
    pub fn registration_id(&self) -> Result<u32> {
        Ok(self.registration)
    }
    pub fn device_id(&self) -> Result<DeviceId> {
        Ok(self.device)
    }
    pub fn identity_key(&self) -> Result<&IdentityKey> {
        Ok(&self.identity)
    }
    pub fn pre_key_id(&self) -> Result<Option<PreKeyId>> {
        Ok(self.pre_key.as_ref().map(|(id, _)| *id))
    }
    pub fn pre_key_public(&self) -> Result<Option<&PublicKey>> {
        Ok(self.pre_key.as_ref().map(|(_, key)| key))
    }
    pub fn signed_pre_key_id(&self) -> Result<SignedPreKeyId> {
        Ok(self.signed_id)
    }
    pub fn signed_pre_key_public(&self) -> Result<&PublicKey> {
        Ok(&self.signed_public)
    }
    pub fn signed_pre_key_signature(&self) -> Result<&[u8]> {
        Ok(&self.signature)
    }
}

impl UnacknowledgedPreKeyMessageItems {
    pub fn pre_key_id(&self) -> Option<PreKeyId> {
        self.pre_key_id
    }
    pub fn signed_pre_key_id(&self) -> SignedPreKeyId {
        self.signed_pre_key_id
    }
    pub fn base_key(&self) -> &PublicKey {
        &self.base_key
    }
}
