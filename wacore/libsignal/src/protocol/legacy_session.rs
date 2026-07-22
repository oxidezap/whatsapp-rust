//! Interoperability with the decoded libsignal `SessionRecord` v1 model.
//!
//! Container decoding is intentionally out of scope. Callers provide owned
//! bytes and typed roles; this module owns protocol validation, chain-role
//! selection, counter translation, history ordering, and ratchet projection.

use std::cmp::Ordering;
use std::fmt;

use bytes::Bytes;
use thiserror::Error;

use crate::protocol::consts;
use crate::protocol::protocol::CIPHERTEXT_MESSAGE_CURRENT_VERSION;
use crate::protocol::{
    IdentityKey, PendingPreKeyComponents, PublicKey, SessionChainComponents,
    SessionChainKeyComponents, SessionComponents, SessionMessageKeyComponents,
    SessionMessageKeyMaterial, SessionRecord, SessionRecordComponents, SignalProtocolError,
};

const LEGACY_CHAIN_COUNTER_OFFSET: i64 = 1;
const LEGACY_KEY_MATERIAL_LEN: usize = 32;
const LEGACY_BASE_KEY_LOCAL: u32 = 1;
const LEGACY_BASE_KEY_REMOTE: u32 = 2;
const LEGACY_CHAIN_SENDING: u32 = 1;
const LEGACY_CHAIN_RECEIVING: u32 = 2;
const LEGACY_CURRENT_SESSION: i64 = -1;
const OPERATIONAL_CREATED_AT_MS: u64 = 0;

/// Whether the session base key was generated locally or remotely.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySessionBaseKeyRoleV1 {
    Local,
    Remote,
}

impl LegacySessionBaseKeyRoleV1 {
    pub const fn discriminant(self) -> u32 {
        match self {
            Self::Local => LEGACY_BASE_KEY_LOCAL,
            Self::Remote => LEGACY_BASE_KEY_REMOTE,
        }
    }
}

impl TryFrom<u32> for LegacySessionBaseKeyRoleV1 {
    type Error = LegacySessionInteropError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            LEGACY_BASE_KEY_LOCAL => Ok(Self::Local),
            LEGACY_BASE_KEY_REMOTE => Ok(Self::Remote),
            value => Err(LegacySessionInteropError::UnknownBaseKeyRole(value)),
        }
    }
}

/// Semantic role of a ratchet chain in `SessionRecord` v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySessionChainRoleV1 {
    Sending,
    Receiving,
}

impl LegacySessionChainRoleV1 {
    pub const fn discriminant(self) -> u32 {
        match self {
            Self::Sending => LEGACY_CHAIN_SENDING,
            Self::Receiving => LEGACY_CHAIN_RECEIVING,
        }
    }
}

impl TryFrom<u32> for LegacySessionChainRoleV1 {
    type Error = LegacySessionInteropError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            LEGACY_CHAIN_SENDING => Ok(Self::Sending),
            LEGACY_CHAIN_RECEIVING => Ok(Self::Receiving),
            value => Err(LegacySessionInteropError::UnknownChainRole(value)),
        }
    }
}

/// Current/archived state decoded from the v1 `closed` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySessionDispositionV1 {
    Current,
    Archived { closed_at_ms: i64 },
}

impl LegacySessionDispositionV1 {
    pub fn from_closed_timestamp(value: i64) -> Result<Self, LegacySessionInteropError> {
        match value {
            LEGACY_CURRENT_SESSION => Ok(Self::Current),
            value if value >= 0 => Ok(Self::Archived {
                closed_at_ms: value,
            }),
            value => Err(LegacySessionInteropError::InvalidClosedTimestamp(value)),
        }
    }

    pub fn closed_timestamp(self) -> i64 {
        match self {
            Self::Current => LEGACY_CURRENT_SESSION,
            Self::Archived { closed_at_ms } => closed_at_ms,
        }
    }
}

/// The v1 counter points at the last derived message; the canonical counter
/// points at the next message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacySessionChainCounterV1(i64);

impl LegacySessionChainCounterV1 {
    pub fn new(value: i64) -> Result<Self, LegacySessionInteropError> {
        let native = value
            .checked_add(LEGACY_CHAIN_COUNTER_OFFSET)
            .ok_or(LegacySessionInteropError::InvalidChainCounter(value))?;
        u32::try_from(native)
            .map(|_| Self(value))
            .map_err(|_| LegacySessionInteropError::InvalidChainCounter(value))
    }

    pub fn from_native_index(index: u32) -> Self {
        Self(i64::from(index) - LEGACY_CHAIN_COUNTER_OFFSET)
    }

    pub const fn value(self) -> i64 {
        self.0
    }

    fn native_index(self) -> u32 {
        u32::try_from(self.0 + LEGACY_CHAIN_COUNTER_OFFSET).expect("validated legacy chain counter")
    }
}

/// Local values absent from a persisted v1 session.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct LegacySessionLocalContext {
    pub identity_key: IdentityKey,
    pub registration_id: u32,
}

/// One indexed entry decoded from the outer v1 session map.
pub struct LegacyIndexedSessionV1 {
    pub index_key: Bytes,
    pub session: LegacySessionV1,
}

/// A decoded `SessionRecord` v1, independent of its transport encoding.
pub struct LegacySessionRecordV1 {
    sessions: Vec<LegacyIndexedSessionV1>,
}

/// One current or archived v1 session.
pub struct LegacySessionV1 {
    pub registration_id: u32,
    pub ratchet: LegacySessionRatchetV1,
    pub index: LegacySessionIndexV1,
    pub chains: Vec<LegacySessionChainV1>,
    pub pending_pre_key: Option<LegacySessionPendingPreKeyV1>,
}

/// Current double-ratchet state in the v1 record.
pub struct LegacySessionRatchetV1 {
    pub key_pair: LegacySessionKeyPairV1,
    pub last_remote_ephemeral_key: Bytes,
    pub previous_counter: u32,
    pub root_key: Bytes,
}

/// Public/private ratchet material.
pub struct LegacySessionKeyPairV1 {
    pub public: Bytes,
    pub private: Bytes,
}

/// Session identity, lifecycle, and ordering metadata.
pub struct LegacySessionIndexV1 {
    pub base_key: Bytes,
    pub base_key_role: LegacySessionBaseKeyRoleV1,
    pub disposition: LegacySessionDispositionV1,
    pub used_at_ms: u64,
    pub created_at_ms: u64,
    pub remote_identity_key: Bytes,
}

/// One sending or receiving chain.
pub struct LegacySessionChainV1 {
    pub ratchet_key: Bytes,
    pub role: LegacySessionChainRoleV1,
    pub chain_key: LegacySessionChainKeyV1,
    pub message_keys: Vec<LegacySessionMessageKeyV1>,
}

/// Chain position and optional seed. A missing seed denotes a closed receiver
/// chain that can only consume already-derived skipped keys.
pub struct LegacySessionChainKeyV1 {
    pub counter: LegacySessionChainCounterV1,
    pub key: Option<Bytes>,
}

/// A skipped message-key seed in the v1 model.
pub struct LegacySessionMessageKeyV1 {
    pub index: u32,
    pub seed: Bytes,
}

/// Pending classic pre-key metadata.
pub struct LegacySessionPendingPreKeyV1 {
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: u32,
    pub base_key: Bytes,
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl fmt::Debug for LegacyIndexedSessionV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacyIndexedSessionV1")
            .field("index_key", &Redacted)
            .field("session", &self.session)
            .finish()
    }
}

impl fmt::Debug for LegacySessionLocalContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionLocalContext")
            .field("identity_key", &Redacted)
            .field("registration_id", &self.registration_id)
            .finish()
    }
}

impl fmt::Debug for LegacySessionRecordV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionRecordV1")
            .field("session_count", &self.sessions.len())
            .finish()
    }
}

impl fmt::Debug for LegacySessionV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionV1")
            .field("registration_id", &self.registration_id)
            .field("ratchet", &self.ratchet)
            .field("index", &self.index)
            .field("chains", &self.chains)
            .field("pending_pre_key", &self.pending_pre_key)
            .finish()
    }
}

impl fmt::Debug for LegacySessionRatchetV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionRatchetV1")
            .field("key_pair", &Redacted)
            .field("last_remote_ephemeral_key", &Redacted)
            .field("previous_counter", &self.previous_counter)
            .field("root_key", &Redacted)
            .finish()
    }
}

impl fmt::Debug for LegacySessionIndexV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionIndexV1")
            .field("base_key", &Redacted)
            .field("base_key_role", &self.base_key_role)
            .field("disposition", &self.disposition)
            .field("used_at_ms", &self.used_at_ms)
            .field("created_at_ms", &self.created_at_ms)
            .field("remote_identity_key", &Redacted)
            .finish()
    }
}

impl fmt::Debug for LegacySessionChainV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionChainV1")
            .field("ratchet_key", &Redacted)
            .field("role", &self.role)
            .field("chain_key", &self.chain_key)
            .field("message_key_count", &self.message_keys.len())
            .finish()
    }
}

impl fmt::Debug for LegacySessionChainKeyV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionChainKeyV1")
            .field("counter", &self.counter)
            .field("key", &Redacted)
            .finish()
    }
}

impl fmt::Debug for LegacySessionMessageKeyV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionMessageKeyV1")
            .field("index", &self.index)
            .field("seed", &Redacted)
            .finish()
    }
}

impl fmt::Debug for LegacySessionPendingPreKeyV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LegacySessionPendingPreKeyV1")
            .field("pre_key_id", &self.pre_key_id)
            .field("signed_pre_key_id", &self.signed_pre_key_id)
            .field("base_key", &Redacted)
            .finish()
    }
}

/// A field that cannot be reconstructed by an operational v1 projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySessionUnrepresentableFieldV1 {
    SessionVersion,
    SenderChain,
    PendingKeyExchange,
    PostQuantumPreKey,
    RefreshState,
    DerivedMessageKey,
    LastRemoteEphemeralKey,
}

impl fmt::Display for LegacySessionUnrepresentableFieldV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::SessionVersion => "session version",
            Self::SenderChain => "sender chain",
            Self::PendingKeyExchange => "pending key exchange",
            Self::PostQuantumPreKey => "post-quantum pending pre-key state",
            Self::RefreshState => "session refresh state",
            Self::DerivedMessageKey => "derived skipped-message key",
            Self::LastRemoteEphemeralKey => "last remote ephemeral key",
        };
        f.write_str(name)
    }
}

/// Field identifiers used by validation errors without exposing key material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySessionFieldV1 {
    RatchetPublicKey,
    RatchetPrivateKey,
    LastRemoteEphemeralKey,
    RootKey,
    BaseKey,
    RemoteIdentityKey,
    LocalIdentityKey,
    PendingPreKeyBaseKey,
    SendingChainKey,
    ChainKey,
    ChainKeyIndex,
    ChainRatchetPublicKey,
    SkippedMessageSeed,
    SenderRatchetPublicKey,
    SenderRatchetPrivateKey,
    RemoteRegistrationId,
    LocalRegistrationId,
    PreviousCounter,
}

impl fmt::Display for LegacySessionFieldV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::RatchetPublicKey => "ratchet public key",
            Self::RatchetPrivateKey => "ratchet private key",
            Self::LastRemoteEphemeralKey => "last remote ephemeral key",
            Self::RootKey => "root key",
            Self::BaseKey => "base key",
            Self::RemoteIdentityKey => "remote identity key",
            Self::LocalIdentityKey => "local identity key",
            Self::PendingPreKeyBaseKey => "pending pre-key base key",
            Self::SendingChainKey => "sending chain key",
            Self::ChainKey => "chain key",
            Self::ChainKeyIndex => "chain-key index",
            Self::ChainRatchetPublicKey => "chain ratchet public key",
            Self::SkippedMessageSeed => "skipped-message seed",
            Self::SenderRatchetPublicKey => "sender ratchet public key",
            Self::SenderRatchetPrivateKey => "sender ratchet private key",
            Self::RemoteRegistrationId => "remote registration id",
            Self::LocalRegistrationId => "local registration id",
            Self::PreviousCounter => "previous counter",
        };
        f.write_str(name)
    }
}

/// Typed failures for v1 import and operational projection.
#[derive(Debug, Error)]
pub enum LegacySessionInteropError {
    #[error("unknown legacy base-key role {0}")]
    UnknownBaseKeyRole(u32),
    #[error("unknown legacy chain role {0}")]
    UnknownChainRole(u32),
    #[error("invalid legacy closed timestamp {0}")]
    InvalidClosedTimestamp(i64),
    #[error("legacy chain counter {0} cannot map to a canonical index")]
    InvalidChainCounter(i64),
    #[error("legacy session {session} is indexed by a different base key")]
    MismatchedIndexKey { session: usize },
    #[error("legacy session {session} duplicates another base key")]
    DuplicateBaseKey { session: usize },
    #[error("legacy record contains more than one current session")]
    MultipleCurrentSessions,
    #[error("legacy session {session} contains duplicate ratchet chain {chain}")]
    DuplicateRatchetChain { session: usize, chain: usize },
    #[error("legacy session {session} has no sending chain")]
    MissingSenderChain { session: usize },
    #[error("legacy session {session} has more than one sending chain")]
    MultipleSenderChains { session: usize },
    #[error("legacy session {session} sending chain does not match its current ratchet")]
    SenderChainRatchetMismatch { session: usize },
    #[error("legacy session {session} most recent remote ratchet has no receiving chain")]
    LastRemoteRatchetMismatch { session: usize },
    #[error("legacy session {session} remote-base state has a pending local pre-key")]
    InvalidPendingPreKeyRole { session: usize },
    #[error("legacy session {session} pending pre-key does not match its base key")]
    PendingPreKeyBaseMismatch { session: usize },
    #[error("legacy session {session} signed pre-key id is out of range")]
    InvalidSignedPreKeyId { session: usize },
    #[error("legacy session {session} chain {chain} has too many skipped message keys")]
    TooManyMessageKeys { session: usize, chain: usize },
    #[error("canonical session {session} exceeds the receiver-chain limit")]
    InvalidCanonicalReceiverChainCount { session: usize },
    #[error("legacy session {session} chain {chain} has duplicate skipped-message index {index}")]
    DuplicateMessageKey {
        session: usize,
        chain: usize,
        index: u32,
    },
    #[error(
        "legacy session {session} chain {chain} has a skipped-message index outside its derived range"
    )]
    InvalidMessageKeyIndex { session: usize, chain: usize },
    #[error("legacy session {session} field {field} is invalid")]
    InvalidSessionField {
        session: usize,
        field: LegacySessionFieldV1,
    },
    #[error("legacy session {session} chain {chain} field {field} is invalid")]
    InvalidChainField {
        session: usize,
        chain: usize,
        field: LegacySessionFieldV1,
    },
    #[error("legacy session {session} cannot represent {field}")]
    NotRepresentable {
        session: usize,
        field: LegacySessionUnrepresentableFieldV1,
    },
    #[error("legacy session {session} chain {chain} cannot represent {field}")]
    ChainNotRepresentable {
        session: usize,
        chain: usize,
        field: LegacySessionUnrepresentableFieldV1,
    },
    #[error("canonical session record could not be projected")]
    InvalidCanonicalRecord(#[source] SignalProtocolError),
}

impl LegacySessionRecordV1 {
    /// Validates outer map keys once and normalizes both fields to one shared
    /// base-key allocation. The input vector is reused in place.
    pub fn from_indexed_sessions(
        mut sessions: Vec<LegacyIndexedSessionV1>,
    ) -> Result<Self, LegacySessionInteropError> {
        for (session, indexed) in sessions.iter_mut().enumerate() {
            if indexed.index_key != indexed.session.index.base_key {
                return Err(LegacySessionInteropError::MismatchedIndexKey { session });
            }
            indexed.index_key = indexed.session.index.base_key.clone();
        }

        sessions.sort_unstable_by(|left, right| {
            left.session
                .index
                .base_key
                .cmp(&right.session.index.base_key)
        });
        for (session, pair) in sessions.windows(2).enumerate() {
            if pair[0].session.index.base_key == pair[1].session.index.base_key {
                return Err(LegacySessionInteropError::DuplicateBaseKey {
                    session: session + 1,
                });
            }
        }

        let current_count = sessions
            .iter()
            .filter(|entry| {
                matches!(
                    entry.session.index.disposition,
                    LegacySessionDispositionV1::Current
                )
            })
            .count();
        if current_count > 1 {
            return Err(LegacySessionInteropError::MultipleCurrentSessions);
        }

        Ok(Self { sessions })
    }

    /// Returns the validated outer index together with each decoded session.
    pub fn indexed_sessions(&self) -> &[LegacyIndexedSessionV1] {
        &self.sessions
    }

    /// Consumes the record without rebuilding its indexed entry list.
    pub fn into_indexed_sessions(self) -> Vec<LegacyIndexedSessionV1> {
        self.sessions
    }

    /// Converts decoded v1 state into the canonical record. Archived sessions
    /// are retained by close time, then ordered by last use to match the v1
    /// decrypt search. Both sorts are in-place and allocation-free.
    pub fn into_session_record(
        mut self,
        context: LegacySessionLocalContext,
    ) -> Result<SessionRecord, LegacySessionInteropError> {
        for (session_index, indexed) in self.sessions.iter_mut().enumerate() {
            validate_session(&mut indexed.session, session_index)?;
        }
        self.sessions
            .sort_unstable_by(|left, right| retention_order(&left.session, &right.session));

        let current_count = usize::from(self.sessions.first().is_some_and(|indexed| {
            matches!(
                indexed.session.index.disposition,
                LegacySessionDispositionV1::Current
            )
        }));
        self.sessions
            .truncate(current_count + consts::ARCHIVED_STATES_MAX_LENGTH);
        self.sessions[current_count..]
            .sort_unstable_by(|left, right| decrypt_order(&left.session, &right.session));

        let mut sessions = self.sessions.into_iter();
        let current_session = if current_count == 1 {
            Some(import_indexed_session(
                sessions.next().expect("current session was counted"),
                context,
                0,
            )?)
        } else {
            None
        };

        let previous_sessions = sessions
            .enumerate()
            .map(|(index, session)| import_indexed_session(session, context, current_count + index))
            .collect::<Result<Vec<_>, _>>()?;

        SessionRecord::from_components(SessionRecordComponents {
            current_session,
            previous_sessions,
        })
        .map_err(LegacySessionInteropError::InvalidCanonicalRecord)
    }
}

impl SessionRecord {
    /// Projects a canonical record into a v1 model with deterministic
    /// lifecycle ranks. This is operational, not byte-exact: v1 timestamps and
    /// acknowledged base-key ownership are not persisted canonically.
    ///
    /// Archive/use ranks preserve canonical search and eviction order. A
    /// pending pre-key identifies a local base key; otherwise the projection
    /// uses the remote role accepted by v1 lookup. The newest receiver chain
    /// supplies `last_remote_ephemeral_key`; without one, a non-chain base key
    /// is an inert placeholder. Local identity and registration values remain
    /// external because v1 does not persist them.
    ///
    /// Derived skipped-message keys have no inverse to their v1 seed, so their
    /// presence returns [`LegacySessionInteropError::NotRepresentable`].
    pub fn into_legacy_session_v1_operational(
        self,
    ) -> Result<LegacySessionRecordV1, LegacySessionInteropError> {
        let components = self
            .into_components()
            .map_err(LegacySessionInteropError::InvalidCanonicalRecord)?;
        let archived_count = components.previous_sessions.len();
        let mut sessions =
            Vec::with_capacity(archived_count + usize::from(components.current_session.is_some()));

        if let Some(current) = components.current_session {
            sessions.push(index_projected_session(project_session(
                current,
                LegacySessionDispositionV1::Current,
                archived_count.saturating_add(1) as u64,
                0,
            )?));
        }

        for (index, session) in components.previous_sessions.into_iter().enumerate() {
            let rank = archived_count.saturating_sub(index) as u64;
            sessions.push(index_projected_session(project_session(
                session,
                LegacySessionDispositionV1::Archived {
                    closed_at_ms: i64::try_from(rank).expect("archive limit fits i64"),
                },
                rank,
                index + 1,
            )?));
        }

        LegacySessionRecordV1::from_indexed_sessions(sessions)
    }
}

fn import_indexed_session(
    indexed: LegacyIndexedSessionV1,
    context: LegacySessionLocalContext,
    session_index: usize,
) -> Result<SessionComponents, LegacySessionInteropError> {
    let LegacyIndexedSessionV1 { index_key, session } = indexed;
    drop(index_key);
    import_session(session, context, session_index)
}

fn index_projected_session(session: LegacySessionV1) -> LegacyIndexedSessionV1 {
    LegacyIndexedSessionV1 {
        index_key: session.index.base_key.clone(),
        session,
    }
}

fn retention_order(left: &LegacySessionV1, right: &LegacySessionV1) -> Ordering {
    match (left.index.disposition, right.index.disposition) {
        (LegacySessionDispositionV1::Current, LegacySessionDispositionV1::Current) => {
            left.index.base_key.cmp(&right.index.base_key)
        }
        (LegacySessionDispositionV1::Current, _) => Ordering::Less,
        (_, LegacySessionDispositionV1::Current) => Ordering::Greater,
        (
            LegacySessionDispositionV1::Archived {
                closed_at_ms: left_closed,
            },
            LegacySessionDispositionV1::Archived {
                closed_at_ms: right_closed,
            },
        ) => right_closed
            .cmp(&left_closed)
            .then_with(|| left.index.base_key.cmp(&right.index.base_key)),
    }
}

fn decrypt_order(left: &LegacySessionV1, right: &LegacySessionV1) -> Ordering {
    right
        .index
        .used_at_ms
        .cmp(&left.index.used_at_ms)
        .then_with(|| retention_order(left, right))
}

fn import_session(
    session: LegacySessionV1,
    context: LegacySessionLocalContext,
    session_index: usize,
) -> Result<SessionComponents, LegacySessionInteropError> {
    let LegacySessionV1 {
        registration_id,
        ratchet,
        index,
        mut chains,
        pending_pre_key,
    } = session;
    let LegacySessionRatchetV1 {
        key_pair,
        last_remote_ephemeral_key,
        previous_counter,
        root_key,
    } = ratchet;
    let LegacySessionKeyPairV1 {
        public: sender_ratchet_public,
        private: sender_ratchet_private,
    } = key_pair;

    let sender_index = chains
        .iter()
        .position(|chain| chain.role == LegacySessionChainRoleV1::Sending)
        .ok_or(LegacySessionInteropError::MissingSenderChain {
            session: session_index,
        })?;
    let sender = chains.remove(sender_index);

    let last_remote = chains
        .iter()
        .position(|chain| chain.ratchet_key == last_remote_ephemeral_key);
    if !chains.is_empty() {
        let last_remote =
            last_remote.ok_or(LegacySessionInteropError::LastRemoteRatchetMismatch {
                session: session_index,
            })?;
        if last_remote + 1 != chains.len() {
            let newest = chains.remove(last_remote);
            chains.push(newest);
        }
    }

    if chains.len() > consts::MAX_RECEIVER_CHAINS {
        let excess = chains.len() - consts::MAX_RECEIVER_CHAINS;
        chains.drain(..excess);
    }

    // These v1 aliases are not persisted separately by the canonical model.
    // Drop them before moving their matching Bytes into Vec-backed components,
    // so uniquely owned buffers can transfer without a copy.
    drop(sender_ratchet_public);
    drop(last_remote_ephemeral_key);

    let pending_pre_key = pending_pre_key
        .map(|pending| {
            let signed_pre_key_id = i32::try_from(pending.signed_pre_key_id).map_err(|_| {
                LegacySessionInteropError::InvalidSignedPreKeyId {
                    session: session_index,
                }
            })?;
            Ok(PendingPreKeyComponents {
                pre_key_id: pending.pre_key_id,
                signed_pre_key_id: Some(signed_pre_key_id),
                base_key: Some(pending.base_key.into()),
                kyber_pre_key_id: None,
                kyber_ciphertext: None,
            })
        })
        .transpose()?;

    let sender_chain = chain_into_components(sender, Some(sender_ratchet_private));
    let receiver_chains = chains
        .into_iter()
        .map(|chain| chain_into_components(chain, None))
        .collect();

    Ok(SessionComponents {
        session_version: Some(u32::from(CIPHERTEXT_MESSAGE_CURRENT_VERSION)),
        local_identity_public: Some(context.identity_key.serialize().to_vec()),
        remote_identity_public: Some(index.remote_identity_key.into()),
        root_key: Some(root_key.into()),
        previous_counter: Some(previous_counter),
        sender_chain: Some(sender_chain),
        receiver_chains,
        pending_key_exchange: None,
        pending_pre_key,
        remote_registration_id: Some(registration_id),
        local_registration_id: Some(context.registration_id),
        needs_refresh: None,
        alice_base_key: Some(index.base_key.into()),
    })
}

fn validate_session(
    session: &mut LegacySessionV1,
    session_index: usize,
) -> Result<(), LegacySessionInteropError> {
    if let LegacySessionDispositionV1::Archived { closed_at_ms } = session.index.disposition
        && closed_at_ms < 0
    {
        return Err(LegacySessionInteropError::InvalidClosedTimestamp(
            closed_at_ms,
        ));
    }
    validate_public_key(
        &session.ratchet.key_pair.public,
        session_index,
        LegacySessionFieldV1::RatchetPublicKey,
    )?;
    validate_exact(
        &session.ratchet.key_pair.private,
        LEGACY_KEY_MATERIAL_LEN,
        session_index,
        LegacySessionFieldV1::RatchetPrivateKey,
    )?;
    validate_public_key(
        &session.ratchet.last_remote_ephemeral_key,
        session_index,
        LegacySessionFieldV1::LastRemoteEphemeralKey,
    )?;
    validate_exact(
        &session.ratchet.root_key,
        LEGACY_KEY_MATERIAL_LEN,
        session_index,
        LegacySessionFieldV1::RootKey,
    )?;
    validate_public_key(
        &session.index.base_key,
        session_index,
        LegacySessionFieldV1::BaseKey,
    )?;
    validate_public_key(
        &session.index.remote_identity_key,
        session_index,
        LegacySessionFieldV1::RemoteIdentityKey,
    )?;

    if session.index.base_key_role == LegacySessionBaseKeyRoleV1::Remote
        && session.pending_pre_key.is_some()
    {
        return Err(LegacySessionInteropError::InvalidPendingPreKeyRole {
            session: session_index,
        });
    }
    if let Some(pending) = session.pending_pre_key.as_ref() {
        if pending.base_key != session.index.base_key {
            validate_public_key(
                &pending.base_key,
                session_index,
                LegacySessionFieldV1::PendingPreKeyBaseKey,
            )?;
            return Err(LegacySessionInteropError::PendingPreKeyBaseMismatch {
                session: session_index,
            });
        }
        if i32::try_from(pending.signed_pre_key_id).is_err() {
            return Err(LegacySessionInteropError::InvalidSignedPreKeyId {
                session: session_index,
            });
        }
    }

    let sender_count = session
        .chains
        .iter()
        .filter(|chain| chain.role == LegacySessionChainRoleV1::Sending)
        .count();
    match sender_count {
        0 => {
            return Err(LegacySessionInteropError::MissingSenderChain {
                session: session_index,
            });
        }
        1 => {}
        _ => {
            return Err(LegacySessionInteropError::MultipleSenderChains {
                session: session_index,
            });
        }
    }

    for chain_index in 0..session.chains.len() {
        for previous in 0..chain_index {
            if session.chains[previous].ratchet_key == session.chains[chain_index].ratchet_key {
                return Err(LegacySessionInteropError::DuplicateRatchetChain {
                    session: session_index,
                    chain: chain_index,
                });
            }
        }

        let chain = &mut session.chains[chain_index];
        if chain.role == LegacySessionChainRoleV1::Sending {
            if chain.ratchet_key != session.ratchet.key_pair.public {
                return Err(LegacySessionInteropError::SenderChainRatchetMismatch {
                    session: session_index,
                });
            }
            if chain.chain_key.key.is_none() {
                return Err(LegacySessionInteropError::InvalidChainField {
                    session: session_index,
                    chain: chain_index,
                    field: LegacySessionFieldV1::SendingChainKey,
                });
            }
        } else if chain.ratchet_key != session.ratchet.last_remote_ephemeral_key {
            validate_public_key_for_chain(&chain.ratchet_key, session_index, chain_index)?;
        }

        if let Some(key) = chain.chain_key.key.as_ref()
            && key.len() != LEGACY_KEY_MATERIAL_LEN
        {
            return Err(LegacySessionInteropError::InvalidChainField {
                session: session_index,
                chain: chain_index,
                field: LegacySessionFieldV1::ChainKey,
            });
        }
        if chain.message_keys.len() > consts::MAX_MESSAGE_KEYS {
            return Err(LegacySessionInteropError::TooManyMessageKeys {
                session: session_index,
                chain: chain_index,
            });
        }

        chain.message_keys.sort_unstable_by_key(|key| key.index);
        for pair in chain.message_keys.windows(2) {
            if pair[0].index == pair[1].index {
                return Err(LegacySessionInteropError::DuplicateMessageKey {
                    session: session_index,
                    chain: chain_index,
                    index: pair[0].index,
                });
            }
        }
        let next_index = chain.chain_key.counter.native_index();
        for message_key in &chain.message_keys {
            if message_key.index >= next_index {
                return Err(LegacySessionInteropError::InvalidMessageKeyIndex {
                    session: session_index,
                    chain: chain_index,
                });
            }
            if message_key.seed.len() != LEGACY_KEY_MATERIAL_LEN {
                return Err(LegacySessionInteropError::InvalidChainField {
                    session: session_index,
                    chain: chain_index,
                    field: LegacySessionFieldV1::SkippedMessageSeed,
                });
            }
        }
    }

    Ok(())
}

fn validate_public_key(
    value: &[u8],
    session: usize,
    field: LegacySessionFieldV1,
) -> Result<(), LegacySessionInteropError> {
    if value.len() != PublicKey::SERIALIZED_KEY_LEN || PublicKey::deserialize(value).is_err() {
        return Err(LegacySessionInteropError::InvalidSessionField { session, field });
    }
    Ok(())
}

fn validate_public_key_for_chain(
    value: &[u8],
    session: usize,
    chain: usize,
) -> Result<(), LegacySessionInteropError> {
    if value.len() != PublicKey::SERIALIZED_KEY_LEN || PublicKey::deserialize(value).is_err() {
        return Err(LegacySessionInteropError::InvalidChainField {
            session,
            chain,
            field: LegacySessionFieldV1::ChainRatchetPublicKey,
        });
    }
    Ok(())
}

fn validate_exact(
    value: &[u8],
    expected: usize,
    session: usize,
    field: LegacySessionFieldV1,
) -> Result<(), LegacySessionInteropError> {
    if value.len() != expected {
        return Err(LegacySessionInteropError::InvalidSessionField { session, field });
    }
    Ok(())
}

fn chain_into_components(
    chain: LegacySessionChainV1,
    private_key: Option<Bytes>,
) -> SessionChainComponents {
    SessionChainComponents {
        sender_ratchet_key: Some(chain.ratchet_key.into()),
        sender_ratchet_key_private: private_key.map(Into::into),
        chain_key: Some(SessionChainKeyComponents {
            index: Some(chain.chain_key.counter.native_index()),
            key: chain.chain_key.key.map(Into::into),
        }),
        message_keys: chain
            .message_keys
            .into_iter()
            .map(|key| SessionMessageKeyComponents {
                index: key.index,
                material: SessionMessageKeyMaterial::Seed(key.seed.into()),
            })
            .collect(),
    }
}

fn project_session(
    session: SessionComponents,
    disposition: LegacySessionDispositionV1,
    used_at_ms: u64,
    session_index: usize,
) -> Result<LegacySessionV1, LegacySessionInteropError> {
    if session.receiver_chains.len() > consts::MAX_RECEIVER_CHAINS {
        return Err(
            LegacySessionInteropError::InvalidCanonicalReceiverChainCount {
                session: session_index,
            },
        );
    }
    if session.session_version != Some(u32::from(CIPHERTEXT_MESSAGE_CURRENT_VERSION)) {
        return Err(LegacySessionInteropError::NotRepresentable {
            session: session_index,
            field: LegacySessionUnrepresentableFieldV1::SessionVersion,
        });
    }
    if session.pending_key_exchange.is_some() {
        return Err(LegacySessionInteropError::NotRepresentable {
            session: session_index,
            field: LegacySessionUnrepresentableFieldV1::PendingKeyExchange,
        });
    }
    if session.needs_refresh == Some(true) {
        return Err(LegacySessionInteropError::NotRepresentable {
            session: session_index,
            field: LegacySessionUnrepresentableFieldV1::RefreshState,
        });
    }

    let sender = session
        .sender_chain
        .ok_or(LegacySessionInteropError::NotRepresentable {
            session: session_index,
            field: LegacySessionUnrepresentableFieldV1::SenderChain,
        })?;
    let sender_ratchet_key: Bytes = required_component(
        sender.sender_ratchet_key,
        session_index,
        LegacySessionFieldV1::SenderRatchetPublicKey,
    )?
    .into();
    let sender_ratchet_private: Bytes = required_component(
        sender.sender_ratchet_key_private,
        session_index,
        LegacySessionFieldV1::SenderRatchetPrivateKey,
    )?
    .into();
    let base_key: Bytes = required_component(
        session.alice_base_key,
        session_index,
        LegacySessionFieldV1::BaseKey,
    )?
    .into();
    let remote_identity_key: Bytes = required_component(
        session.remote_identity_public,
        session_index,
        LegacySessionFieldV1::RemoteIdentityKey,
    )?
    .into();
    drop(required_component(
        session.local_identity_public,
        session_index,
        LegacySessionFieldV1::LocalIdentityKey,
    )?);
    let root_key: Bytes = required_component(
        session.root_key,
        session_index,
        LegacySessionFieldV1::RootKey,
    )?
    .into();
    let registration_id =
        session
            .remote_registration_id
            .ok_or(LegacySessionInteropError::InvalidSessionField {
                session: session_index,
                field: LegacySessionFieldV1::RemoteRegistrationId,
            })?;
    session
        .local_registration_id
        .ok_or(LegacySessionInteropError::InvalidSessionField {
            session: session_index,
            field: LegacySessionFieldV1::LocalRegistrationId,
        })?;
    let previous_counter =
        session
            .previous_counter
            .ok_or(LegacySessionInteropError::InvalidSessionField {
                session: session_index,
                field: LegacySessionFieldV1::PreviousCounter,
            })?;

    let pending_pre_key = session
        .pending_pre_key
        .map(|pending| project_pending_pre_key(pending, session_index))
        .transpose()?;
    let base_key_role = if pending_pre_key.is_some() {
        LegacySessionBaseKeyRoleV1::Local
    } else {
        // The canonical matcher treats every stored alice base key as eligible
        // for a remote pre-key lookup. Remote mirrors that operational policy.
        LegacySessionBaseKeyRoleV1::Remote
    };

    let mut chains = Vec::with_capacity(session.receiver_chains.len() + 1);
    // v1 references the current public ratchet from both the key pair and the
    // sending chain; Bytes keeps that duplication allocation-free.
    chains.push(project_chain_parts(
        sender_ratchet_key.clone(),
        sender.chain_key,
        sender.message_keys,
        LegacySessionChainRoleV1::Sending,
        session_index,
        0,
    )?);
    for (index, receiver) in session.receiver_chains.into_iter().enumerate() {
        chains.push(project_chain(
            receiver,
            LegacySessionChainRoleV1::Receiving,
            session_index,
            index + 1,
        )?);
    }

    let last_receiver = chains
        .iter()
        .rev()
        .find(|chain| chain.role == LegacySessionChainRoleV1::Receiving);
    let last_remote_ephemeral_key = if let Some(receiver) = last_receiver {
        receiver.ratchet_key.clone()
    } else if base_key == sender_ratchet_key {
        return Err(LegacySessionInteropError::NotRepresentable {
            session: session_index,
            field: LegacySessionUnrepresentableFieldV1::LastRemoteEphemeralKey,
        });
    } else {
        base_key.clone()
    };

    Ok(LegacySessionV1 {
        registration_id,
        ratchet: LegacySessionRatchetV1 {
            key_pair: LegacySessionKeyPairV1 {
                public: sender_ratchet_key,
                private: sender_ratchet_private,
            },
            last_remote_ephemeral_key,
            previous_counter,
            root_key,
        },
        index: LegacySessionIndexV1 {
            base_key,
            base_key_role,
            disposition,
            used_at_ms,
            created_at_ms: OPERATIONAL_CREATED_AT_MS,
            remote_identity_key,
        },
        chains,
        pending_pre_key,
    })
}

fn project_pending_pre_key(
    pending: PendingPreKeyComponents,
    session: usize,
) -> Result<LegacySessionPendingPreKeyV1, LegacySessionInteropError> {
    if pending.kyber_pre_key_id.is_some() || pending.kyber_ciphertext.is_some() {
        return Err(LegacySessionInteropError::NotRepresentable {
            session,
            field: LegacySessionUnrepresentableFieldV1::PostQuantumPreKey,
        });
    }
    let signed_pre_key_id = pending
        .signed_pre_key_id
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(LegacySessionInteropError::InvalidSignedPreKeyId { session })?;
    Ok(LegacySessionPendingPreKeyV1 {
        pre_key_id: pending.pre_key_id,
        signed_pre_key_id,
        base_key: Bytes::from(required_component(
            pending.base_key,
            session,
            LegacySessionFieldV1::PendingPreKeyBaseKey,
        )?),
    })
}

fn project_chain(
    chain: SessionChainComponents,
    role: LegacySessionChainRoleV1,
    session: usize,
    chain_index: usize,
) -> Result<LegacySessionChainV1, LegacySessionInteropError> {
    let ratchet_key: Bytes = required_component(
        chain.sender_ratchet_key,
        session,
        LegacySessionFieldV1::ChainRatchetPublicKey,
    )?
    .into();
    project_chain_parts(
        ratchet_key,
        chain.chain_key,
        chain.message_keys,
        role,
        session,
        chain_index,
    )
}

fn project_chain_parts(
    ratchet_key: Bytes,
    chain_key: Option<SessionChainKeyComponents>,
    message_keys: Vec<SessionMessageKeyComponents>,
    role: LegacySessionChainRoleV1,
    session: usize,
    chain_index: usize,
) -> Result<LegacySessionChainV1, LegacySessionInteropError> {
    let chain_key = chain_key.ok_or(LegacySessionInteropError::InvalidChainField {
        session,
        chain: chain_index,
        field: LegacySessionFieldV1::ChainKey,
    })?;
    let native_index = chain_key
        .index
        .ok_or(LegacySessionInteropError::InvalidChainField {
            session,
            chain: chain_index,
            field: LegacySessionFieldV1::ChainKeyIndex,
        })?;
    if message_keys
        .iter()
        .any(|key| matches!(key.material, SessionMessageKeyMaterial::Derived { .. }))
    {
        return Err(LegacySessionInteropError::ChainNotRepresentable {
            session,
            chain: chain_index,
            field: LegacySessionUnrepresentableFieldV1::DerivedMessageKey,
        });
    }
    let message_keys = message_keys
        .into_iter()
        .map(|key| {
            let SessionMessageKeyMaterial::Seed(seed) = key.material else {
                unreachable!("derived message-key material was rejected before allocation")
            };
            LegacySessionMessageKeyV1 {
                index: key.index,
                seed: seed.into(),
            }
        })
        .collect();

    Ok(LegacySessionChainV1 {
        ratchet_key,
        role,
        chain_key: LegacySessionChainKeyV1 {
            counter: LegacySessionChainCounterV1::from_native_index(native_index),
            key: chain_key.key.map(Into::into),
        },
        message_keys,
    })
}

fn required_component<T>(
    value: Option<T>,
    session: usize,
    field: LegacySessionFieldV1,
) -> Result<T, LegacySessionInteropError> {
    value.ok_or(LegacySessionInteropError::InvalidSessionField { session, field })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        KeyPair, MessageKeyGenerator, PendingKeyExchangeComponents, SenderChainKeyComponents,
        SenderKeyRecord, SenderKeyRecordComponents, SenderKeyStateComponents,
        SenderMessageKeyComponents, SenderSigningKeyComponents,
    };

    fn public_key(seed: u8) -> Bytes {
        let mut key = vec![seed; PublicKey::SERIALIZED_KEY_LEN];
        key[0] = 0x05;
        key.into()
    }

    fn local_context() -> LegacySessionLocalContext {
        LegacySessionLocalContext {
            identity_key: IdentityKey::decode(&public_key(0x70)).expect("valid identity"),
            registration_id: 7,
        }
    }

    fn chain(seed: u8, role: LegacySessionChainRoleV1, counter: i64) -> LegacySessionChainV1 {
        LegacySessionChainV1 {
            ratchet_key: public_key(seed),
            role,
            chain_key: LegacySessionChainKeyV1 {
                counter: LegacySessionChainCounterV1::new(counter).expect("valid counter"),
                key: Some(vec![seed.wrapping_add(1); 32].into()),
            },
            message_keys: Vec::new(),
        }
    }

    // This is the already-decoded shape emitted by the reference
    // `libsignal/src/session_record.js` v1 serializer. Counter and role values
    // follow `session_builder.js` and `session_cipher.js`.
    fn reference_session(seed: u8, disposition: LegacySessionDispositionV1) -> LegacySessionV1 {
        let base_key = public_key(seed.wrapping_add(1));
        let sender_ratchet = seed.wrapping_add(2);
        let remote_ratchet = seed.wrapping_add(3);
        LegacySessionV1 {
            registration_id: 10_000 + u32::from(seed),
            ratchet: LegacySessionRatchetV1 {
                key_pair: LegacySessionKeyPairV1 {
                    public: public_key(sender_ratchet),
                    private: vec![seed.wrapping_add(4); 32].into(),
                },
                last_remote_ephemeral_key: public_key(remote_ratchet),
                previous_counter: u32::from(seed),
                root_key: vec![seed.wrapping_add(5); 32].into(),
            },
            index: LegacySessionIndexV1 {
                base_key,
                base_key_role: LegacySessionBaseKeyRoleV1::Remote,
                disposition,
                used_at_ms: 1_000 + u64::from(seed),
                created_at_ms: 500 + u64::from(seed),
                remote_identity_key: public_key(seed.wrapping_add(6)),
            },
            chains: vec![
                chain(sender_ratchet, LegacySessionChainRoleV1::Sending, -1),
                chain(remote_ratchet, LegacySessionChainRoleV1::Receiving, 1),
            ],
            pending_pre_key: None,
        }
    }

    fn indexed(session: LegacySessionV1) -> LegacyIndexedSessionV1 {
        LegacyIndexedSessionV1 {
            index_key: session.index.base_key.clone(),
            session,
        }
    }

    fn record(sessions: Vec<LegacySessionV1>) -> LegacySessionRecordV1 {
        LegacySessionRecordV1::from_indexed_sessions(sessions.into_iter().map(indexed).collect())
            .expect("valid indexed record")
    }

    fn canonical_components(seed: u8) -> SessionRecordComponents {
        record(vec![reference_session(
            seed,
            LegacySessionDispositionV1::Current,
        )])
        .into_session_record(local_context())
        .expect("import")
        .into_components()
        .expect("components")
    }

    #[test]
    fn roles_and_lifecycle_decode_only_known_values() {
        assert_eq!(
            LegacySessionBaseKeyRoleV1::try_from(1).expect("local"),
            LegacySessionBaseKeyRoleV1::Local
        );
        assert_eq!(
            LegacySessionChainRoleV1::try_from(2).expect("receiving"),
            LegacySessionChainRoleV1::Receiving
        );
        assert!(matches!(
            LegacySessionBaseKeyRoleV1::try_from(3),
            Err(LegacySessionInteropError::UnknownBaseKeyRole(3))
        ));
        assert!(matches!(
            LegacySessionChainRoleV1::try_from(0),
            Err(LegacySessionInteropError::UnknownChainRole(0))
        ));
        assert_eq!(
            LegacySessionDispositionV1::from_closed_timestamp(-1).expect("current"),
            LegacySessionDispositionV1::Current
        );
        assert_eq!(
            LegacySessionDispositionV1::from_closed_timestamp(42).expect("archived"),
            LegacySessionDispositionV1::Archived { closed_at_ms: 42 }
        );
        assert!(matches!(
            LegacySessionDispositionV1::from_closed_timestamp(-2),
            Err(LegacySessionInteropError::InvalidClosedTimestamp(-2))
        ));

        let invalid = reference_session(
            90,
            LegacySessionDispositionV1::Archived { closed_at_ms: -2 },
        );
        assert!(matches!(
            record(vec![invalid]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidClosedTimestamp(-2))
        ));
    }

    #[test]
    fn chain_counter_translation_is_checked_at_both_bounds() {
        assert_eq!(
            LegacySessionChainCounterV1::new(-1)
                .expect("initial counter")
                .native_index(),
            0
        );
        let maximum_legacy = i64::from(u32::MAX) - 1;
        assert_eq!(
            LegacySessionChainCounterV1::new(maximum_legacy)
                .expect("maximum counter")
                .native_index(),
            u32::MAX
        );
        assert!(LegacySessionChainCounterV1::new(-2).is_err());
        assert!(LegacySessionChainCounterV1::new(i64::from(u32::MAX)).is_err());
        assert_eq!(
            LegacySessionChainCounterV1::from_native_index(0).value(),
            -1
        );
    }

    #[test]
    fn outer_index_and_current_session_invariants_are_enforced() {
        let first = reference_session(1, LegacySessionDispositionV1::Current);
        let mut bad_index = indexed(first);
        bad_index.index_key = public_key(99);
        assert!(matches!(
            LegacySessionRecordV1::from_indexed_sessions(vec![bad_index]),
            Err(LegacySessionInteropError::MismatchedIndexKey { session: 0 })
        ));

        let first = reference_session(2, LegacySessionDispositionV1::Current);
        let mut second = reference_session(3, LegacySessionDispositionV1::Current);
        second.index.base_key = first.index.base_key.clone();
        let second = LegacyIndexedSessionV1 {
            index_key: second.index.base_key.clone(),
            session: second,
        };
        assert!(matches!(
            LegacySessionRecordV1::from_indexed_sessions(vec![indexed(first), second]),
            Err(LegacySessionInteropError::DuplicateBaseKey { .. })
        ));

        let first = reference_session(4, LegacySessionDispositionV1::Current);
        let second = reference_session(5, LegacySessionDispositionV1::Current);
        assert!(matches!(
            LegacySessionRecordV1::from_indexed_sessions(vec![indexed(first), indexed(second)]),
            Err(LegacySessionInteropError::MultipleCurrentSessions)
        ));
    }

    #[test]
    fn imports_reference_v1_shape_and_preserves_pending_pre_key() {
        let mut session = reference_session(10, LegacySessionDispositionV1::Current);
        session.index.base_key_role = LegacySessionBaseKeyRoleV1::Local;
        session.pending_pre_key = Some(LegacySessionPendingPreKeyV1 {
            pre_key_id: Some(11),
            signed_pre_key_id: 12,
            base_key: session.index.base_key.clone(),
        });

        let components = record(vec![session])
            .into_session_record(local_context())
            .expect("import")
            .into_components()
            .expect("projection");
        let current = components.current_session.expect("current session");
        assert_eq!(current.session_version, Some(3));
        assert_eq!(current.local_registration_id, Some(7));
        assert_eq!(current.remote_registration_id, Some(10_010));
        let pending = current.pending_pre_key.expect("pending pre-key");
        assert_eq!(pending.pre_key_id, Some(11));
        assert_eq!(pending.signed_pre_key_id, Some(12));
    }

    #[test]
    fn pending_pre_key_role_and_base_are_strict() {
        let mut remote = reference_session(11, LegacySessionDispositionV1::Current);
        remote.pending_pre_key = Some(LegacySessionPendingPreKeyV1 {
            pre_key_id: None,
            signed_pre_key_id: 1,
            base_key: remote.index.base_key.clone(),
        });
        assert!(matches!(
            record(vec![remote]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidPendingPreKeyRole { .. })
        ));

        let mut local = reference_session(12, LegacySessionDispositionV1::Current);
        local.index.base_key_role = LegacySessionBaseKeyRoleV1::Local;
        local.pending_pre_key = Some(LegacySessionPendingPreKeyV1 {
            pre_key_id: None,
            signed_pre_key_id: 1,
            base_key: public_key(99),
        });
        assert!(matches!(
            record(vec![local]).into_session_record(local_context()),
            Err(LegacySessionInteropError::PendingPreKeyBaseMismatch { .. })
        ));

        let mut overflow = reference_session(13, LegacySessionDispositionV1::Current);
        overflow.index.base_key_role = LegacySessionBaseKeyRoleV1::Local;
        overflow.pending_pre_key = Some(LegacySessionPendingPreKeyV1 {
            pre_key_id: None,
            signed_pre_key_id: i32::MAX as u32 + 1,
            base_key: overflow.index.base_key.clone(),
        });
        assert!(matches!(
            record(vec![overflow]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidSignedPreKeyId { .. })
        ));
    }

    #[test]
    fn sender_chain_role_and_ratchet_are_strict() {
        let mut missing = reference_session(20, LegacySessionDispositionV1::Current);
        missing.chains[0].role = LegacySessionChainRoleV1::Receiving;
        assert!(matches!(
            record(vec![missing]).into_session_record(local_context()),
            Err(LegacySessionInteropError::MissingSenderChain { .. })
        ));

        let mut duplicate = reference_session(21, LegacySessionDispositionV1::Current);
        duplicate
            .chains
            .push(chain(90, LegacySessionChainRoleV1::Sending, -1));
        assert!(matches!(
            record(vec![duplicate]).into_session_record(local_context()),
            Err(LegacySessionInteropError::MultipleSenderChains { .. })
        ));

        let mut mismatch = reference_session(22, LegacySessionDispositionV1::Current);
        mismatch.ratchet.key_pair.public = public_key(91);
        assert!(matches!(
            record(vec![mismatch]).into_session_record(local_context()),
            Err(LegacySessionInteropError::SenderChainRatchetMismatch { .. })
        ));

        let mut invalid_private = reference_session(23, LegacySessionDispositionV1::Current);
        invalid_private.ratchet.key_pair.private = Bytes::from_static(&[0x23; 31]);
        assert!(matches!(
            record(vec![invalid_private]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidSessionField {
                field: LegacySessionFieldV1::RatchetPrivateKey,
                ..
            })
        ));

        let mut missing_key = reference_session(24, LegacySessionDispositionV1::Current);
        missing_key.chains[0].chain_key.key = None;
        assert!(matches!(
            record(vec![missing_key]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidChainField {
                field: LegacySessionFieldV1::SendingChainKey,
                ..
            })
        ));
    }

    #[test]
    fn receiver_order_uses_last_remote_then_applies_canonical_bound() {
        let mut session = reference_session(30, LegacySessionDispositionV1::Current);
        session.chains.truncate(1);
        for seed in 40..47 {
            session
                .chains
                .push(chain(seed, LegacySessionChainRoleV1::Receiving, 0));
        }
        session.ratchet.last_remote_ephemeral_key = public_key(43);

        let components = record(vec![session])
            .into_session_record(local_context())
            .expect("import")
            .into_components()
            .expect("components")
            .current_session
            .expect("current");
        let ratchets: Vec<_> = components
            .receiver_chains
            .into_iter()
            .map(|chain| chain.sender_ratchet_key.expect("ratchet"))
            .collect();
        assert_eq!(
            ratchets,
            vec![
                public_key(42),
                public_key(44),
                public_key(45),
                public_key(46),
                public_key(43),
            ]
        );
    }

    #[test]
    fn archive_retention_and_decrypt_order_follow_reference_semantics() {
        let current = reference_session(100, LegacySessionDispositionV1::Current);
        let mut sessions = vec![current];
        for ordinal in 1..=42u8 {
            let mut archived = reference_session(
                ordinal,
                LegacySessionDispositionV1::Archived {
                    closed_at_ms: i64::from(ordinal),
                },
            );
            archived.index.used_at_ms = 1_000 - u64::from(ordinal);
            sessions.push(archived);
        }

        let components = record(sessions)
            .into_session_record(local_context())
            .expect("import")
            .into_components()
            .expect("components");
        assert!(components.current_session.is_some());
        assert_eq!(
            components.previous_sessions.len(),
            consts::ARCHIVED_STATES_MAX_LENGTH
        );
        let bases: Vec<_> = components
            .previous_sessions
            .into_iter()
            .map(|session| session.alice_base_key.expect("base"))
            .collect();
        assert_eq!(
            bases.first().expect("newest archived base key"),
            public_key(4).as_ref()
        );
        assert_eq!(
            bases.last().expect("oldest retained base key"),
            public_key(43).as_ref()
        );
    }

    #[test]
    fn skipped_seed_uses_the_canonical_message_key_derivation() {
        let seed = [0xA5; 32];
        let mut session = reference_session(50, LegacySessionDispositionV1::Current);
        session.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
            index: 0,
            seed: Bytes::copy_from_slice(&seed),
        }];

        let components = record(vec![session])
            .into_session_record(local_context())
            .expect("import")
            .into_components()
            .expect("components");
        let material = &components.current_session.expect("current").receiver_chains[0]
            .message_keys[0]
            .material;
        let expected = MessageKeyGenerator::new_from_seed(&seed, 0).generate_keys();
        match material {
            SessionMessageKeyMaterial::Derived {
                cipher_key,
                mac_key,
                iv,
            } => {
                assert_eq!(cipher_key, expected.cipher_key());
                assert_eq!(mac_key, expected.mac_key());
                assert_eq!(iv, expected.iv());
            }
            SessionMessageKeyMaterial::Seed(_) => panic!("seed must be derived on import"),
        }
    }

    #[test]
    fn malformed_or_duplicate_skipped_keys_are_never_filtered() {
        let mut duplicate = reference_session(51, LegacySessionDispositionV1::Current);
        duplicate.chains[1].message_keys = vec![
            LegacySessionMessageKeyV1 {
                index: 0,
                seed: vec![1; 32].into(),
            },
            LegacySessionMessageKeyV1 {
                index: 0,
                seed: vec![2; 32].into(),
            },
        ];
        assert!(matches!(
            record(vec![duplicate]).into_session_record(local_context()),
            Err(LegacySessionInteropError::DuplicateMessageKey { .. })
        ));

        let mut malformed = reference_session(52, LegacySessionDispositionV1::Current);
        malformed.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
            index: 0,
            seed: vec![3; 31].into(),
        }];
        assert!(matches!(
            record(vec![malformed]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidChainField {
                field: LegacySessionFieldV1::SkippedMessageSeed,
                ..
            })
        ));

        let mut future = reference_session(53, LegacySessionDispositionV1::Current);
        future.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
            index: 2,
            seed: vec![4; 32].into(),
        }];
        assert!(matches!(
            record(vec![future]).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidMessageKeyIndex { .. })
        ));

        let mut sessions = Vec::with_capacity(consts::ARCHIVED_STATES_MAX_LENGTH + 1);
        for ordinal in 0..=consts::ARCHIVED_STATES_MAX_LENGTH {
            let mut archived = reference_session(
                u8::try_from(ordinal + 90).expect("test seed"),
                LegacySessionDispositionV1::Archived {
                    closed_at_ms: i64::try_from(ordinal).expect("test timestamp"),
                },
            );
            if ordinal == 0 {
                archived.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
                    index: 0,
                    seed: vec![5; 31].into(),
                }];
            }
            sessions.push(archived);
        }
        assert!(matches!(
            record(sessions).into_session_record(local_context()),
            Err(LegacySessionInteropError::InvalidChainField {
                field: LegacySessionFieldV1::SkippedMessageSeed,
                ..
            })
        ));
    }

    #[test]
    fn skipped_message_key_limit_is_checked_before_derivation() {
        let mut session = reference_session(55, LegacySessionDispositionV1::Current);
        session.chains[1].chain_key.counter = LegacySessionChainCounterV1::from_native_index(
            u32::try_from(consts::MAX_MESSAGE_KEYS).expect("message-key limit fits u32"),
        );
        session.chains[1].message_keys = (0..consts::MAX_MESSAGE_KEYS)
            .map(|index| LegacySessionMessageKeyV1 {
                index: u32::try_from(index).expect("message-key limit fits u32"),
                seed: Bytes::from_static(&[0x55; LEGACY_KEY_MATERIAL_LEN]),
            })
            .collect();
        validate_session(&mut session, 0).expect("canonical limit is accepted");

        session.chains[1]
            .message_keys
            .push(LegacySessionMessageKeyV1 {
                index: u32::try_from(consts::MAX_MESSAGE_KEYS).expect("message-key limit fits u32"),
                seed: Bytes::from_static(&[0x56; LEGACY_KEY_MATERIAL_LEN]),
            });
        assert!(matches!(
            validate_session(&mut session, 0),
            Err(LegacySessionInteropError::TooManyMessageKeys { .. })
        ));
    }

    #[test]
    fn empty_records_and_closed_receiver_chains_are_preserved() {
        let empty = LegacySessionRecordV1::from_indexed_sessions(Vec::new())
            .expect("empty record")
            .into_session_record(local_context())
            .expect("empty import")
            .into_components()
            .expect("empty components");
        assert!(empty.current_session.is_none());
        assert!(empty.previous_sessions.is_empty());

        let mut session = reference_session(54, LegacySessionDispositionV1::Current);
        session.chains[1].chain_key.key = None;
        session.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
            index: 0,
            seed: vec![6; 32].into(),
        }];
        let components = record(vec![session])
            .into_session_record(local_context())
            .expect("closed-chain import")
            .into_components()
            .expect("components");
        assert!(
            components.current_session.expect("current").receiver_chains[0]
                .chain_key
                .as_ref()
                .expect("chain")
                .key
                .is_none()
        );
    }

    #[test]
    fn operational_projection_fails_closed_for_derived_skipped_keys() {
        let mut session = reference_session(60, LegacySessionDispositionV1::Current);
        session.chains[1].message_keys = vec![LegacySessionMessageKeyV1 {
            index: 0,
            seed: vec![0x44; 32].into(),
        }];
        let native = record(vec![session])
            .into_session_record(local_context())
            .expect("import");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::ChainNotRepresentable {
                chain: 1,
                field: LegacySessionUnrepresentableFieldV1::DerivedMessageKey,
                ..
            })
        ));
    }

    #[test]
    fn operational_projection_preserves_behavior_without_claiming_exact_metadata() {
        let current = reference_session(61, LegacySessionDispositionV1::Current);
        let archived = reference_session(
            62,
            LegacySessionDispositionV1::Archived { closed_at_ms: 900 },
        );
        let native = record(vec![archived, current])
            .into_session_record(local_context())
            .expect("import");
        let projection = native
            .into_legacy_session_v1_operational()
            .expect("operational projection");
        assert_eq!(projection.indexed_sessions().len(), 2);
        let current = projection
            .sessions
            .iter()
            .find(|indexed| {
                indexed.session.index.disposition == LegacySessionDispositionV1::Current
            })
            .expect("current projection");
        let archived = projection
            .sessions
            .iter()
            .find(|indexed| {
                matches!(
                    indexed.session.index.disposition,
                    LegacySessionDispositionV1::Archived { .. }
                )
            })
            .expect("archived projection");
        assert_eq!(
            current.session.index.disposition,
            LegacySessionDispositionV1::Current
        );
        assert_eq!(current.session.index.created_at_ms, 0);
        assert_eq!(
            current.session.index.base_key_role,
            LegacySessionBaseKeyRoleV1::Remote
        );
        assert_eq!(
            archived.session.index.disposition,
            LegacySessionDispositionV1::Archived { closed_at_ms: 1 }
        );
        assert!(current.session.index.used_at_ms > archived.session.index.used_at_ms);

        let mut without_receiver = reference_session(64, LegacySessionDispositionV1::Current);
        without_receiver.chains.truncate(1);
        let projected = record(vec![without_receiver])
            .into_session_record(local_context())
            .expect("import")
            .into_legacy_session_v1_operational()
            .expect("safe last-remote projection");
        let projected = &projected.sessions[0].session;
        assert_eq!(
            projected.ratchet.last_remote_ephemeral_key,
            projected.index.base_key
        );
        assert_ne!(
            projected.ratchet.last_remote_ephemeral_key,
            projected.ratchet.key_pair.public
        );

        let mut collision = reference_session(65, LegacySessionDispositionV1::Current);
        collision.chains.truncate(1);
        collision.index.base_key = collision.ratchet.key_pair.public.clone();
        let native = record(vec![collision])
            .into_session_record(local_context())
            .expect("import collision");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::LastRemoteEphemeralKey,
                ..
            })
        ));
    }

    #[test]
    fn operational_projection_rejects_active_refresh_state() {
        let mut components = canonical_components(63);
        components
            .current_session
            .as_mut()
            .expect("current")
            .needs_refresh = Some(true);
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::RefreshState,
                ..
            })
        ));
    }

    #[test]
    fn operational_projection_rejects_canonical_state_v1_cannot_preserve() {
        let mut components = canonical_components(66);
        components
            .current_session
            .as_mut()
            .expect("current")
            .session_version = Some(2);
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::SessionVersion,
                ..
            })
        ));

        let mut components = canonical_components(67);
        components
            .current_session
            .as_mut()
            .expect("current")
            .sender_chain = None;
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::SenderChain,
                ..
            })
        ));

        let mut components = canonical_components(68);
        components
            .current_session
            .as_mut()
            .expect("current")
            .pending_key_exchange = Some(PendingKeyExchangeComponents::default());
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::PendingKeyExchange,
                ..
            })
        ));

        let mut components = canonical_components(69);
        let current = components.current_session.as_mut().expect("current");
        current.pending_pre_key = Some(PendingPreKeyComponents {
            pre_key_id: None,
            signed_pre_key_id: Some(1),
            base_key: current.alice_base_key.clone(),
            kyber_pre_key_id: Some(2),
            kyber_ciphertext: Some(vec![3; LEGACY_KEY_MATERIAL_LEN]),
        });
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::NotRepresentable {
                field: LegacySessionUnrepresentableFieldV1::PostQuantumPreKey,
                ..
            })
        ));

        let mut components = canonical_components(70);
        components
            .previous_sessions
            .push(components.current_session.clone().expect("current"));
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::DuplicateBaseKey { .. })
        ));

        let mut components = canonical_components(71);
        let current = components.current_session.as_mut().expect("current");
        let receiver = current.receiver_chains[0].clone();
        current
            .receiver_chains
            .resize(consts::MAX_RECEIVER_CHAINS + 1, receiver);
        let native = SessionRecord::from_components(components).expect("canonical record");
        assert!(matches!(
            native.into_legacy_session_v1_operational(),
            Err(LegacySessionInteropError::InvalidCanonicalReceiverChainCount { .. })
        ));
    }

    #[test]
    fn imported_record_survives_wire_reload_and_derives_live_sender_keys() {
        let native = record(vec![reference_session(
            70,
            LegacySessionDispositionV1::Current,
        )])
        .into_session_record(local_context())
        .expect("import");
        let wire = native.serialize().expect("serialize");
        let reloaded = SessionRecord::deserialize(&wire).expect("reload");
        let chain = reloaded
            .session_state()
            .expect("current")
            .get_sender_chain_key()
            .expect("sender chain");
        let (message_key, next_chain) = chain.step_with_message_keys().expect("derive");
        let generated = message_key.generate_keys();
        assert_eq!(next_chain.index(), chain.index() + 1);
        assert_eq!(generated.cipher_key().len(), 32);
        assert_eq!(generated.mac_key().len(), 32);
        assert_eq!(generated.iv().len(), 16);
    }

    #[test]
    fn sender_key_components_remain_the_single_bidirectional_model() {
        let key_pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let expected = SenderKeyRecordComponents {
            states: vec![SenderKeyStateComponents {
                key_id: 9,
                chain_key: SenderChainKeyComponents {
                    iteration: 4,
                    seed: vec![0x11; 32],
                },
                signing_key: SenderSigningKeyComponents {
                    public: key_pair.public_key.serialize().to_vec(),
                    private: Some(key_pair.private_key.serialize().to_vec()),
                },
                message_keys: vec![SenderMessageKeyComponents {
                    iteration: 3,
                    seed: vec![0x22; 32],
                }],
            }],
        };
        let actual = SenderKeyRecord::from_components(expected.clone())
            .expect("import sender-key")
            .into_components()
            .expect("project sender-key");
        assert_eq!(actual, expected);
    }

    #[test]
    fn debug_output_never_contains_key_material() {
        let session = reference_session(80, LegacySessionDispositionV1::Current);
        let output = format!("{session:?}");
        assert!(output.contains("<redacted>"));
        assert!(!output.contains("80, 80, 80"));
    }
}
