// SPDX-License-Identifier: MIT
use crate::{CryptoProvider, Error, Result};
use std::collections::VecDeque;
use zeroize::Zeroize;
pub const MAX_SKIPPED: usize = 2000;
pub const MAX_RECEIVER_CHAINS: usize = 5;
pub const MAX_ARCHIVED_SESSIONS: usize = 40;
pub const MAX_SENDER_STATES: usize = 5;
pub const PEER_FUTURE_LIMIT: u32 = 2000;
pub const SELF_FUTURE_LIMIT: u32 = 25000;

#[derive(Clone, PartialEq, Eq)]
pub struct KeyPair {
    pub public: [u8; 32],
    pub private: [u8; 32],
}
impl Drop for KeyPair {
    fn drop(&mut self) {
        self.private.zeroize();
    }
}
impl core::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("KeyPair")
            .field("public", &self.public)
            .finish_non_exhaustive()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum MessageKey {
    Seed {
        index: u32,
        seed: [u8; 32],
    },
    Derived {
        index: u32,
        cipher: [u8; 32],
        mac: [u8; 32],
        iv: [u8; 16],
    },
}
impl Drop for MessageKey {
    fn drop(&mut self) {
        match self {
            Self::Seed { seed, .. } => seed.zeroize(),
            Self::Derived {
                cipher, mac, iv, ..
            } => {
                cipher.zeroize();
                mac.zeroize();
                iv.zeroize();
            }
        }
    }
}
impl MessageKey {
    pub fn index(&self) -> u32 {
        match self {
            Self::Seed { index, .. } | Self::Derived { index, .. } => *index,
        }
    }
    pub(crate) fn expanded<C: CryptoProvider>(
        &self,
        crypto: &C,
    ) -> Result<([u8; 32], [u8; 32], [u8; 16])> {
        match self {
            Self::Derived {
                cipher, mac, iv, ..
            } => Ok((*cipher, *mac, *iv)),
            Self::Seed { seed, .. } => {
                let mut material = [0; 80];
                crypto.hkdf_sha256(&[0; 32], seed, b"WhisperMessageKeys", &mut material)?;
                let mut cipher = [0; 32];
                let mut mac = [0; 32];
                let mut iv = [0; 16];
                cipher.copy_from_slice(&material[..32]);
                mac.copy_from_slice(&material[32..64]);
                iv.copy_from_slice(&material[64..]);
                material.zeroize();
                Ok((cipher, mac, iv))
            }
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct Chain {
    pub ratchet: [u8; 32],
    pub private: Option<[u8; 32]>,
    pub index: u32,
    pub key: [u8; 32],
    pub exhausted: bool,
    pub skipped: VecDeque<MessageKey>,
    pub message_key_limit: Option<u32>,
}
impl Drop for Chain {
    fn drop(&mut self) {
        self.key.zeroize();
        self.private.zeroize();
    }
}
impl Chain {
    pub fn new(ratchet: [u8; 32], private: Option<[u8; 32]>, key: [u8; 32]) -> Self {
        Self {
            ratchet,
            private,
            index: 0,
            key,
            exhausted: false,
            skipped: VecDeque::new(),
            message_key_limit: None,
        }
    }
    pub(crate) fn step<C: CryptoProvider>(&mut self, crypto: &C) -> Result<MessageKey> {
        if self.exhausted {
            return Err(Error::MissingKey);
        }
        let next = self.index.checked_add(1).ok_or(Error::CounterExhausted)?;
        let seed = crypto.hmac_sha256(&self.key, &[1]);
        let message = MessageKey::Seed {
            index: self.index,
            seed,
        };
        self.key = crypto.hmac_sha256(&self.key, &[2]);
        self.index = next;
        Ok(message)
    }
    pub(crate) fn plan<C: CryptoProvider>(
        &self,
        crypto: &C,
        index: u32,
        future_limit: u32,
    ) -> Result<ChainAdvance> {
        if self.exhausted || index < self.index {
            let position = self
                .skipped
                .iter()
                .position(|key| key.index() == index)
                .ok_or(Error::Duplicate)?;
            return Ok(ChainAdvance {
                message: self.skipped[position].clone(),
                consumed: Some(position),
                advanced: None,
            });
        }
        if index - self.index > future_limit {
            return Err(Error::FutureMessage);
        }
        let next_index = index.checked_add(1).ok_or(Error::CounterExhausted)?;
        let mut next_key = self.key;
        let mut skipped = Vec::with_capacity((index - self.index) as usize);
        for counter in self.index..index {
            skipped.push(MessageKey::Seed {
                index: counter,
                seed: crypto.hmac_sha256(&next_key, &[1]),
            });
            next_key = crypto.hmac_sha256(&next_key, &[2]);
        }
        let message = MessageKey::Seed {
            index,
            seed: crypto.hmac_sha256(&next_key, &[1]),
        };
        next_key = crypto.hmac_sha256(&next_key, &[2]);
        Ok(ChainAdvance {
            message,
            consumed: None,
            advanced: Some((next_index, next_key, skipped)),
        })
    }
    fn push_skipped(&mut self, key: MessageKey) {
        let budget = self
            .message_key_limit
            .map(|n| n as usize)
            .unwrap_or(MAX_SKIPPED)
            .max(MAX_SKIPPED);
        if self.skipped.len().saturating_sub(budget) > 50 {
            let remove = self.skipped.len() - budget;
            let mut counters: Vec<_> = self.skipped.iter().map(MessageKey::index).collect();
            let (_, cutoff, _) = counters.select_nth_unstable(remove - 1);
            let cutoff = *cutoff;
            self.skipped.retain(|key| key.index() > cutoff);
        }
        self.skipped.push_back(key);
    }
}
pub(crate) struct ChainAdvance {
    pub message: MessageKey,
    consumed: Option<usize>,
    advanced: Option<(u32, [u8; 32], Vec<MessageKey>)>,
}
impl ChainAdvance {
    pub fn commit(mut self, chain: &mut Chain) {
        if let Some(position) = self.consumed {
            chain.skipped.remove(position);
        }
        if let Some((index, mut key, skipped)) = self.advanced.take() {
            chain.index = index;
            chain.key = key;
            key.zeroize();
            for message in skipped {
                chain.push_skipped(message);
            }
        }
    }
}
impl Drop for ChainAdvance {
    fn drop(&mut self) {
        if let Some((_, key, _)) = &mut self.advanced {
            key.zeroize();
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct PendingPreKey {
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: i32,
    pub base_key: [u8; 32],
    pub kyber_pre_key_id: Option<u32>,
    pub kyber_ciphertext: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SessionState {
    pub version: u32,
    pub local_identity: [u8; 32],
    pub remote_identity: [u8; 32],
    pub root: [u8; 32],
    pub previous_counter: u32,
    pub sender: Option<Chain>,
    pub receivers: Vec<Chain>,
    pub pending_pre_key: Option<PendingPreKey>,
    pub local_registration: u32,
    pub remote_registration: u32,
    pub needs_refresh: bool,
    pub alice_base_key: Option<[u8; 32]>,
    /// Retained opaque legacy key-exchange protobuf, unused by v3 sessions.
    pub pending_key_exchange: Option<Vec<u8>>,
}
impl Drop for SessionState {
    fn drop(&mut self) {
        self.root.zeroize();
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Reservation {
    pub ceiling: Option<u32>,
    pub incarnation: Option<[u8; 16]>,
    pub(crate) committed: bool,
}
impl Reservation {
    pub fn prepare(&mut self, next_counter: u32, count: u32, incarnation: [u8; 16]) -> Result<u32> {
        if count == 0 || count > 2000 {
            return Err(Error::Encoding("invalid reservation size"));
        }
        let ceiling = next_counter
            .checked_add(count)
            .ok_or(Error::CounterExhausted)?;
        if self.ceiling.is_some_and(|old| ceiling < old) {
            return Err(Error::Encoding("reservation cannot decrease"));
        }
        self.ceiling = Some(ceiling);
        self.incarnation = Some(incarnation);
        self.committed = false;
        Ok(ceiling)
    }
    /// Call only after the bytes containing this reservation have durably committed.
    pub fn mark_committed(&mut self) {
        self.committed = true;
    }
    pub fn permits(&self, index: u32) -> bool {
        self.committed && self.ceiling.is_some_and(|ceiling| index < ceiling)
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ArchivedSession {
    pub(crate) bytes: Vec<u8>,
}
impl ArchivedSession {
    pub fn encoded(&self) -> &[u8] {
        &self.bytes
    }
    pub fn decode<C: CryptoProvider>(&self, crypto: &C) -> Result<SessionState> {
        crate::codec::decode_session_state(crypto, &self.bytes)
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SessionRecord {
    pub current: Option<SessionState>,
    pub previous: Vec<ArchivedSession>,
    pub reservation: Reservation,
}
impl SessionRecord {
    pub fn install(&mut self, state: SessionState) {
        if let Some(old) = self.current.replace(state) {
            self.previous.insert(
                0,
                ArchivedSession {
                    bytes: crate::codec::encode_session_state(&old),
                },
            );
            self.previous.truncate(MAX_ARCHIVED_SESSIONS);
        }
        self.reservation = Reservation::default();
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct SenderMessageKey {
    pub iteration: u32,
    pub seed: [u8; 32],
}
impl Drop for SenderMessageKey {
    fn drop(&mut self) {
        self.seed.zeroize();
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct SenderKeyState {
    pub id: u32,
    pub iteration: u32,
    pub chain_key: [u8; 32],
    pub signing_public: [u8; 32],
    pub signing_private: Option<[u8; 32]>,
    pub skipped: VecDeque<SenderMessageKey>,
}
impl Drop for SenderKeyState {
    fn drop(&mut self) {
        self.chain_key.zeroize();
        self.signing_private.zeroize();
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SenderKeyRecord {
    pub states: Vec<SenderKeyState>,
    pub reservation: Reservation,
}
#[derive(Clone, PartialEq, Eq)]
pub struct PreKeyRecord {
    pub id: u32,
    pub key_pair: KeyPair,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SignedPreKeyRecord {
    pub id: u32,
    pub key_pair: KeyPair,
    pub signature: [u8; 64],
    pub timestamp: u64,
}
