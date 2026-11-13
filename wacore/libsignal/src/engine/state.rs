// SPDX-License-Identifier: MIT
use crate::engine::{CryptoProvider, Error, Result};
use std::{
    collections::{VecDeque, vec_deque},
    fmt,
    iter::FusedIterator,
    ops::Index,
    sync::{Arc, OnceLock},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroize;
pub const MAX_SKIPPED: usize = 2000;
pub const MAX_RECEIVER_CHAINS: usize = 5;
pub const MAX_ARCHIVED_SESSIONS: usize = 40;
pub const MAX_SENDER_STATES: usize = 5;
pub const PEER_FUTURE_LIMIT: u32 = 2000;
pub const SELF_FUTURE_LIMIT: u32 = 25000;

/// Copy-on-write skipped-key storage. The default empty value owns no allocation.
pub struct SharedDeque<T> {
    inner: Option<Arc<VecDeque<T>>>,
}
impl<T> Default for SharedDeque<T> {
    fn default() -> Self {
        Self { inner: None }
    }
}
impl<T> Clone for SharedDeque<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}
impl<T: PartialEq> PartialEq for SharedDeque<T> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl<T: Eq> Eq for SharedDeque<T> {}
impl<T: fmt::Debug> fmt::Debug for SharedDeque<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}
impl<T> SharedDeque<T> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.inner.as_ref().map_or(0, |items| items.len())
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn capacity(&self) -> usize {
        self.inner.as_ref().map_or(0, |items| items.capacity())
    }
    pub fn iter(&self) -> SharedDequeIter<'_, T> {
        SharedDequeIter {
            inner: self.inner.as_ref().map(|items| items.iter()),
        }
    }
    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.as_ref().and_then(|items| items.get(index))
    }
    /// The shared allocation's deque header and reference counters, excluding items.
    pub fn allocation_overhead(&self) -> usize {
        if self.inner.is_some() {
            size_of::<VecDeque<T>>() + 2 * size_of::<usize>()
        } else {
            0
        }
    }
}
impl<T: Clone> SharedDeque<T> {
    fn make_mut(&mut self) -> &mut VecDeque<T> {
        Arc::make_mut(self.inner.get_or_insert_with(|| Arc::new(VecDeque::new())))
    }
    pub fn push_back(&mut self, value: T) {
        self.make_mut().push_back(value);
    }
    pub fn remove(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        Arc::make_mut(self.inner.as_mut()?).remove(index)
    }
    pub(crate) fn swap_remove_back(&mut self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        Arc::make_mut(self.inner.as_mut()?).swap_remove_back(index)
    }
    pub fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        Arc::make_mut(self.inner.as_mut()?).pop_front()
    }
    pub fn retain<F: FnMut(&T) -> bool>(&mut self, predicate: F) {
        if !self.is_empty() {
            self.make_mut().retain(predicate);
        }
    }
}
impl<T> From<VecDeque<T>> for SharedDeque<T> {
    fn from(items: VecDeque<T>) -> Self {
        if items.capacity() == 0 {
            Self::default()
        } else {
            Self {
                inner: Some(Arc::new(items)),
            }
        }
    }
}
impl<T> FromIterator<T> for SharedDeque<T> {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        VecDeque::from_iter(items).into()
    }
}
impl<T: Clone> Extend<T> for SharedDeque<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        for item in items {
            self.push_back(item);
        }
    }
}
impl<T> Index<usize> for SharedDeque<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index).expect("deque index out of bounds")
    }
}
pub struct SharedDequeIter<'a, T> {
    inner: Option<vec_deque::Iter<'a, T>>,
}
impl<'a, T> Iterator for SharedDequeIter<'a, T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.as_mut()?.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let length = self.len();
        (length, Some(length))
    }
}
impl<T> DoubleEndedIterator for SharedDequeIter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.inner.as_mut()?.next_back()
    }
}
impl<T> ExactSizeIterator for SharedDequeIter<'_, T> {
    fn len(&self) -> usize {
        self.inner.as_ref().map_or(0, ExactSizeIterator::len)
    }
}
impl<T> FusedIterator for SharedDequeIter<'_, T> {}
impl<'a, T> IntoIterator for &'a SharedDeque<T> {
    type Item = &'a T;
    type IntoIter = SharedDequeIter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

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
pub struct DerivedMessageKey {
    pub cipher: [u8; 32],
    pub mac: [u8; 32],
    pub iv: [u8; 16],
}
impl Drop for DerivedMessageKey {
    fn drop(&mut self) {
        self.cipher.zeroize();
        self.mac.zeroize();
        self.iv.zeroize();
    }
}
/// Expanded storage material remains authoritative. Its redundant seed is
/// checked only when a caller requests a seed-based projection.
pub struct StoredMessageKey {
    pub(crate) material: DerivedMessageKey,
    pub(crate) seed: [u8; 32],
    valid_seed: OnceLock<bool>,
}
impl StoredMessageKey {
    pub(crate) fn new(material: DerivedMessageKey, seed: [u8; 32]) -> Self {
        Self {
            material,
            seed,
            valid_seed: OnceLock::new(),
        }
    }
    fn retained_seed(&self) -> Option<&[u8; 32]> {
        self.valid_seed
            .get_or_init(|| {
                let candidate = MessageKey::Seed {
                    index: 0,
                    seed: self.seed,
                };
                let expanded = zeroize::Zeroizing::new(
                    candidate
                        .expanded(&crate::engine::RustCrypto)
                        .expect("80 bytes are within the SHA-256 HKDF output limit"),
                );
                bool::from(
                    expanded.0.ct_eq(&self.material.cipher)
                        & expanded.1.ct_eq(&self.material.mac)
                        & expanded.2.ct_eq(&self.material.iv),
                )
            })
            .then_some(&self.seed)
    }
}
impl Drop for StoredMessageKey {
    fn drop(&mut self) {
        self.seed.zeroize();
    }
}
#[derive(PartialEq, Eq)]
enum MessageMaterialRef<'a> {
    Seed(&'a [u8; 32]),
    Derived(&'a DerivedMessageKey),
}
#[derive(Clone)]
pub enum MessageKey {
    Seed {
        index: u32,
        seed: [u8; 32],
    },
    Derived {
        index: u32,
        material: Box<DerivedMessageKey>,
    },
    Stored {
        index: u32,
        material: Arc<StoredMessageKey>,
    },
}
impl PartialEq for MessageKey {
    fn eq(&self, other: &Self) -> bool {
        self.index() == other.index()
            && (matches!(
                (self, other),
                (Self::Stored { material: a, .. }, Self::Stored { material: b, .. })
                    if Arc::ptr_eq(a, b)
            ) || self.canonical_material() == other.canonical_material())
    }
}
impl Eq for MessageKey {}
impl Drop for MessageKey {
    fn drop(&mut self) {
        if let Self::Seed { seed, .. } = self {
            seed.zeroize();
        }
    }
}
impl MessageKey {
    pub fn index(&self) -> u32 {
        match self {
            Self::Seed { index, .. } | Self::Derived { index, .. } | Self::Stored { index, .. } => {
                *index
            }
        }
    }
    pub fn extra_heap_bytes(&self) -> usize {
        match self {
            Self::Seed { .. } => 0,
            Self::Derived { .. } => size_of::<DerivedMessageKey>(),
            Self::Stored { .. } => size_of::<StoredMessageKey>() + 2 * size_of::<usize>(),
        }
    }
    fn canonical_material(&self) -> MessageMaterialRef<'_> {
        match self {
            Self::Seed { seed, .. } => MessageMaterialRef::Seed(seed),
            Self::Derived { material, .. } => MessageMaterialRef::Derived(material),
            Self::Stored { material, .. } => match material.retained_seed() {
                Some(seed) => MessageMaterialRef::Seed(seed),
                None => MessageMaterialRef::Derived(&material.material),
            },
        }
    }
    pub(crate) fn retained_seed(&self) -> Option<&[u8; 32]> {
        match self.canonical_material() {
            MessageMaterialRef::Seed(seed) => Some(seed),
            MessageMaterialRef::Derived(_) => None,
        }
    }
    pub(crate) fn expanded<C: CryptoProvider>(
        &self,
        crypto: &C,
    ) -> Result<([u8; 32], [u8; 32], [u8; 16])> {
        match self {
            Self::Derived { material, .. } => Ok((material.cipher, material.mac, material.iv)),
            Self::Stored { material, .. } => Ok((
                material.material.cipher,
                material.material.mac,
                material.material.iv,
            )),
            Self::Seed { seed, .. } => {
                let mut material = [0; 80];
                crypto.hkdf_sha256_zero_salt(seed, b"WhisperMessageKeys", &mut material)?;
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
    pub skipped: SharedDeque<MessageKey>,
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
            skipped: SharedDeque::new(),
            message_key_limit: None,
        }
    }
    pub(crate) fn step<C: CryptoProvider>(&mut self, crypto: &C) -> Result<MessageKey> {
        if self.exhausted {
            return Err(Error::MissingKey);
        }
        let next = self.index.checked_add(1).ok_or(Error::CounterExhausted)?;
        let (seed, next_key) = crypto.chain_step(&self.key)?;
        let message = MessageKey::Seed {
            index: self.index,
            seed,
        };
        self.key = next_key;
        self.index = next;
        Ok(message)
    }
    pub(crate) fn plan<C: CryptoProvider>(
        &self,
        crypto: &C,
        index: u32,
        future_limit: u32,
    ) -> Result<ChainAdvance> {
        if self.exhausted && index >= self.index {
            return Err(Error::ClosedChain);
        }
        if index < self.index {
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
        let mut next_key = zeroize::Zeroizing::new(self.key);
        let mut skipped = Vec::with_capacity((index - self.index) as usize);
        for counter in self.index..index {
            let (seed, advanced) = crypto.chain_step(&next_key)?;
            skipped.push(MessageKey::Seed {
                index: counter,
                seed,
            });
            *next_key = advanced;
        }
        let (seed, advanced) = crypto.chain_step(&next_key)?;
        let message = MessageKey::Seed { index, seed };
        *next_key = advanced;
        Ok(ChainAdvance {
            message,
            consumed: None,
            advanced: Some((next_index, *next_key, skipped)),
        })
    }
    pub(crate) fn push_skipped(&mut self, key: MessageKey) {
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
            chain.skipped.swap_remove_back(position);
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
    pub signed_pre_key_id: Option<i32>,
    pub base_key: [u8; 32],
    pub kyber_pre_key_id: Option<u32>,
    pub kyber_ciphertext: Option<Vec<u8>>,
}
#[derive(Clone, PartialEq, Eq)]
pub struct SessionState {
    pub metadata: Arc<SessionMetadata>,
    pub sender: Option<Chain>,
    pub receivers: Vec<Chain>,
    pub pending_pre_key: Option<Arc<PendingPreKey>>,
}
/// Session identity and ratchet metadata change at setup or a DH transition,
/// while message-counter updates keep sharing the same immutable snapshot.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionMetadata {
    pub version: u32,
    pub local_identity: [u8; 32],
    pub remote_identity: [u8; 32],
    pub root: [u8; 32],
    pub previous_counter: u32,
    pub local_registration: u32,
    pub remote_registration: u32,
    pub needs_refresh: bool,
    pub alice_base_key: Option<[u8; 32]>,
    /// Retry collision records compare these bytes with a separately persisted key.
    pub alice_base_key_tagged: bool,
    /// Retained opaque legacy key-exchange protobuf, unused by v3 sessions.
    pub pending_key_exchange: Option<Vec<u8>>,
}
impl std::ops::Deref for SessionState {
    type Target = SessionMetadata;
    fn deref(&self) -> &Self::Target {
        &self.metadata
    }
}
impl std::ops::DerefMut for SessionState {
    fn deref_mut(&mut self) -> &mut Self::Target {
        Arc::make_mut(&mut self.metadata)
    }
}
impl Drop for SessionMetadata {
    fn drop(&mut self) {
        self.root.zeroize();
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Reservation {
    pub ceiling: Option<u32>,
    pub incarnation: Option<[u8; 16]>,
    pub(crate) committed: bool,
    pub(crate) waived: bool,
    /// The host still owes persistence before publishing group output.
    pub(crate) publication_pending: bool,
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
        self.waived = false;
        Ok(ceiling)
    }
    /// Call only after the bytes containing this reservation have durably committed.
    pub fn mark_committed(&mut self) {
        self.committed = true;
    }
    pub fn permits(&self, index: u32) -> bool {
        self.waived || self.committed && self.ceiling.is_some_and(|ceiling| index < ceiling)
    }
    /// Opt out only when the host supplies its own persistence-before-publication
    /// guarantee. Materialize any existing ceiling before calling this method.
    pub fn waive(&mut self) {
        self.ceiling = None;
        self.incarnation = None;
        self.committed = false;
        self.waived = true;
    }
    pub fn is_waived(&self) -> bool {
        self.waived
    }
    pub fn reset_chain(&mut self) {
        self.ceiling = None;
        self.incarnation = None;
        self.committed = false;
    }
    /// The numerical horizon can be rebased, but a fresh private key has no
    /// durability proof until its own state snapshot commits.
    pub fn rebase(&mut self) {
        self.ceiling = self.ceiling.map(|ceiling| ceiling.min(64));
        self.committed = false;
        self.incarnation = None;
    }
    pub fn requires_durable_commit(&self) -> bool {
        !self.waived && self.ceiling.is_some() && !self.committed
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct ArchivedSession {
    pub(crate) bytes: Arc<[u8]>,
}
impl ArchivedSession {
    pub fn encoded(&self) -> &[u8] {
        &self.bytes
    }
    pub fn decode(&self) -> Result<SessionState> {
        crate::engine::codec::decode_session_state(&self.bytes)
    }
}
/// Archive snapshots share their immutable handle list until a session is promoted.
/// A fresh session does not allocate an empty list.
#[derive(Clone, Default)]
pub struct ArchivedSessions(Option<Arc<Vec<ArchivedSession>>>);
impl ArchivedSessions {
    pub fn capacity(&self) -> usize {
        self.0.as_ref().map_or(0, |items| items.capacity())
    }
    pub fn allocation_overhead(&self) -> usize {
        self.0.as_ref().map_or(0, |_| {
            size_of::<Vec<ArchivedSession>>() + 2 * size_of::<usize>()
        })
    }
    pub fn insert(&mut self, index: usize, value: ArchivedSession) {
        Arc::make_mut(self.0.get_or_insert_with(|| Arc::new(Vec::new()))).insert(index, value);
    }
    pub fn remove(&mut self, index: usize) -> ArchivedSession {
        let value = Arc::make_mut(self.0.as_mut().expect("archive index must exist")).remove(index);
        if self.is_empty() {
            self.0 = None;
        }
        value
    }
    pub fn truncate(&mut self, length: usize) {
        if length == 0 {
            self.0 = None;
        } else if self.len() > length {
            Arc::make_mut(self.0.as_mut().expect("nonempty archive list")).truncate(length);
        }
    }
}
impl std::ops::Deref for ArchivedSessions {
    type Target = [ArchivedSession];
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().map_or(&[], |items| items.as_slice())
    }
}
impl PartialEq for ArchivedSessions {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}
impl Eq for ArchivedSessions {}
impl From<Vec<ArchivedSession>> for ArchivedSessions {
    fn from(items: Vec<ArchivedSession>) -> Self {
        if items.is_empty() {
            Self::default()
        } else {
            Self(Some(Arc::new(items)))
        }
    }
}
impl<'a> IntoIterator for &'a ArchivedSessions {
    type Item = &'a ArchivedSession;
    type IntoIter = std::slice::Iter<'a, ArchivedSession>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
#[derive(Clone, Default, PartialEq, Eq)]
pub struct SessionRecord {
    pub current: Option<SessionState>,
    pub previous: ArchivedSessions,
    pub reservation: Reservation,
}
impl SessionRecord {
    /// Burn the current chain's unspent reservation before it becomes an archive.
    pub fn materialize_reservation<C: CryptoProvider>(&mut self, crypto: &C) -> Result<()> {
        let Some(chain) = self
            .current
            .as_mut()
            .and_then(|state| state.sender.as_mut())
        else {
            return Ok(());
        };
        let target = self.reservation.ceiling.unwrap_or(0);
        if chain.exhausted || chain.index >= target {
            return Ok(());
        }
        if target - chain.index > PEER_FUTURE_LIMIT {
            return Err(Error::Encoding("reservation burn exceeds limit"));
        }
        // A backend can fail partway through a reservation burn. Publish the
        // chain position only after every derivation succeeds.
        let mut key = zeroize::Zeroizing::new(chain.key);
        for _ in chain.index..target {
            *key = crypto.hmac_sha256(&*key, &[&[2]])?;
        }
        chain.key = *key;
        chain.index = target;
        Ok(())
    }
    pub fn install(&mut self, state: SessionState) {
        if let Some(old) = self.current.replace(state) {
            self.previous.insert(
                0,
                ArchivedSession {
                    bytes: crate::engine::codec::encode_session_state(&old).into(),
                },
            );
            self.previous.truncate(MAX_ARCHIVED_SESSIONS);
        }
        self.reservation.reset_chain();
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
    pub skipped: SharedDeque<SenderMessageKey>,
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
    pub legacy_receiver_recovery_disabled: bool,
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

#[cfg(test)]
mod shared_storage_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CloneProbe {
        value: u8,
        clones: Arc<AtomicUsize>,
    }
    impl Clone for CloneProbe {
        fn clone(&self) -> Self {
            self.clones.fetch_add(1, Ordering::Relaxed);
            Self {
                value: self.value,
                clones: self.clones.clone(),
            }
        }
    }

    #[test]
    fn shared_deque_clone_does_not_clone_items_and_write_detaches() {
        let clones = Arc::new(AtomicUsize::new(0));
        let original: SharedDeque<_> = (0..3)
            .map(|value| CloneProbe {
                value,
                clones: clones.clone(),
            })
            .collect();
        let mut changed = original.clone();
        assert!(Arc::ptr_eq(
            original.inner.as_ref().unwrap(),
            changed.inner.as_ref().unwrap()
        ));
        assert_eq!(clones.load(Ordering::Relaxed), 0);
        assert_eq!(changed.remove(99).map(|item| item.value), None);
        assert_eq!(clones.load(Ordering::Relaxed), 0);
        changed.push_back(CloneProbe {
            value: 3,
            clones: clones.clone(),
        });
        assert_eq!(clones.load(Ordering::Relaxed), 3);
        assert!(!Arc::ptr_eq(
            original.inner.as_ref().unwrap(),
            changed.inner.as_ref().unwrap()
        ));
        assert_eq!(
            original.iter().map(|item| item.value).collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            changed.iter().map(|item| item.value).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        changed.retain(|item| item.value % 2 == 0);
        assert_eq!(changed.pop_front().map(|item| item.value), Some(0));
        assert_eq!(changed.get(0).map(|item| item.value), Some(2));
        assert_eq!(original.iter().len(), 3);
        assert_eq!(
            changed
                .iter()
                .rev()
                .map(|item| item.value)
                .collect::<Vec<_>>(),
            vec![2]
        );
    }

    #[test]
    fn empty_deque_has_no_allocation_and_equality_ignores_backing_storage() {
        let empty = SharedDeque::<u8>::default();
        let allocated = SharedDeque::from(VecDeque::<u8>::with_capacity(8));
        assert!(empty.inner.is_none());
        assert_eq!(empty.capacity(), 0);
        assert_eq!(empty.allocation_overhead(), 0);
        assert!(allocated.inner.is_some());
        assert!(allocated.allocation_overhead() >= size_of::<VecDeque<u8>>());
        assert_eq!(empty, allocated);
        let mut consumed: SharedDeque<_> = [1u8].into_iter().collect();
        assert_eq!(consumed.pop_front(), Some(1));
        assert_eq!(consumed, empty);
        assert_eq!(empty.iter().size_hint(), (0, Some(0)));
    }

    #[test]
    fn compact_seed_slots_and_legacy_heap_material_are_accounted_separately() {
        assert!(
            size_of::<MessageKey>() <= 48,
            "a compact seed must not reserve an expanded key triple inline"
        );
        assert_eq!(size_of::<SharedDeque<MessageKey>>(), size_of::<usize>());
        let seed = MessageKey::Seed {
            index: 1,
            seed: [2; 32],
        };
        assert_eq!(seed.extra_heap_bytes(), 0);
        let derived = MessageKey::Derived {
            index: 1,
            material: Box::new(DerivedMessageKey {
                cipher: [3; 32],
                mac: [4; 32],
                iv: [5; 16],
            }),
        };
        assert_eq!(derived.extra_heap_bytes(), size_of::<DerivedMessageKey>());
        assert_eq!(
            derived.expanded(&crate::engine::RustCrypto).unwrap(),
            ([3; 32], [4; 32], [5; 16])
        );
    }

    #[test]
    fn stored_key_equality_preserves_canonical_material_and_counter() {
        let seed = MessageKey::Seed {
            index: 7,
            seed: [2; 32],
        };
        let (cipher, mac, iv) = seed.expanded(&crate::engine::RustCrypto).unwrap();
        let material = DerivedMessageKey { cipher, mac, iv };
        let stored = MessageKey::Stored {
            index: 7,
            material: Arc::new(StoredMessageKey::new(material.clone(), [2; 32])),
        };
        assert!(stored == seed);
        assert!(stored.clone() == stored);
        assert!(stored.extra_heap_bytes() >= size_of::<StoredMessageKey>());
        assert!(
            stored
                != MessageKey::Seed {
                    index: 8,
                    seed: [2; 32]
                }
        );
        let inconsistent = MessageKey::Stored {
            index: 7,
            material: Arc::new(StoredMessageKey::new(material.clone(), [3; 32])),
        };
        assert!(inconsistent != seed);
        assert!(
            inconsistent
                == MessageKey::Derived {
                    index: 7,
                    material: Box::new(material),
                }
        );
    }

    #[test]
    fn archived_clone_shares_immutable_encoded_storage() {
        let archive = ArchivedSession {
            bytes: Arc::from([8u8, 3].as_slice()),
        };
        let cloned = archive.clone();
        assert!(Arc::ptr_eq(&archive.bytes, &cloned.bytes));
        assert_eq!(archive.encoded(), cloned.encoded());
    }

    #[test]
    fn archive_snapshots_detach_only_when_the_ordered_history_changes() {
        let make = |tag| ArchivedSession {
            bytes: Arc::from([8u8, tag].as_slice()),
        };
        let empty = ArchivedSessions::default();
        assert_eq!(empty.capacity(), 0);
        assert_eq!(empty.allocation_overhead(), 0);
        let original = ArchivedSessions::from(vec![make(1), make(2), make(3)]);
        let mut sibling = original.clone();
        let shared = Arc::downgrade(original.0.as_ref().unwrap());
        sibling.truncate(3);
        assert!(Arc::ptr_eq(
            original.0.as_ref().unwrap(),
            sibling.0.as_ref().unwrap()
        ));
        let removed = sibling.remove(1);
        assert_eq!(removed.encoded(), &[8, 2]);
        assert!(!Arc::ptr_eq(
            original.0.as_ref().unwrap(),
            sibling.0.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(&original[0].bytes, &sibling[0].bytes));
        sibling.insert(0, make(4));
        sibling.truncate(2);
        assert_eq!(
            original
                .iter()
                .map(|item| item.encoded()[1])
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        assert_eq!(
            sibling
                .iter()
                .map(|item| item.encoded()[1])
                .collect::<Vec<_>>(),
            [4, 1]
        );
        drop(original);
        assert!(shared.upgrade().is_none());
        sibling.truncate(0);
        assert!(sibling == empty);
        assert_eq!(sibling.allocation_overhead(), 0);
        sibling.insert(0, make(5));
        assert_eq!(sibling.remove(0).encoded(), &[8, 5]);
        assert!(sibling == empty);
        assert_eq!(sibling.capacity(), 0);
    }
}
