// SPDX-License-Identifier: MIT
//! Adapted from João Lucas's project-authored MIT record projections,
//! introduced in 87fbcb55589180dd76ea8c3d02166a358a79b1f8, with later
//! project revisions. This is an attributed port, not an independent rewrite.
//! Owned, validated projections of persisted Signal records.
//!
//! These types keep codec-generated structures private while providing a
//! stable representation for record handoff. Conversion validates fixed-width
//! key material and emits public keys in their canonical serialized form. The
//! explicit field mappings are intentional: a generated schema change must
//! fail to compile here instead of silently dropping protocol state.

use std::fmt;

use bytes::Bytes;

use super::{SenderKeyRecord, SessionRecord, SessionState};
use crate::core::PublicKey;
use crate::engine::{self as engine, CryptoProvider};
use crate::protocol::error::{Result, SignalProtocolError};
use waproto::protobuf::Message as _;
use waproto::whatsapp::{SessionStructure, session_structure};

const PRIVATE_KEY_BYTES: usize = 32;
const SYMMETRIC_KEY_BYTES: usize = 32;
const MESSAGE_IV_BYTES: usize = 16;

/// Complete protocol state carried by a session record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionRecordComponents {
    pub current_session: Option<SessionComponents>,
    pub previous_sessions: Vec<SessionComponents>,
}

/// One current or archived session state.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SessionComponents {
    pub session_version: Option<u32>,
    pub local_identity_public: Option<Vec<u8>>,
    pub remote_identity_public: Option<Vec<u8>>,
    pub root_key: Option<Vec<u8>>,
    pub previous_counter: Option<u32>,
    pub sender_chain: Option<SessionChainComponents>,
    pub receiver_chains: Vec<SessionChainComponents>,
    pub pending_key_exchange: Option<PendingKeyExchangeComponents>,
    pub pending_pre_key: Option<PendingPreKeyComponents>,
    pub remote_registration_id: Option<u32>,
    pub local_registration_id: Option<u32>,
    pub needs_refresh: Option<bool>,
    pub alice_base_key: Option<Vec<u8>>,
}

/// A sender or receiver ratchet chain and its skipped message keys.
///
/// Imported sender chains must be structurally complete. Imported receiver
/// chains must not carry private material; persisted receiver private fields
/// are ignored for compatibility with the canonical reader.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SessionChainComponents {
    pub sender_ratchet_key: Option<Vec<u8>>,
    pub sender_ratchet_key_private: Option<Vec<u8>>,
    pub chain_key: Option<SessionChainKeyComponents>,
    pub message_keys: Vec<SessionMessageKeyComponents>,
}

/// Position and secret material for a session chain.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct SessionChainKeyComponents {
    pub index: Option<u32>,
    pub key: Option<Vec<u8>>,
}

/// A skipped message key identified by its chain index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionMessageKeyComponents {
    pub index: u32,
    pub material: SessionMessageKeyMaterial,
}

/// Secret material used by a skipped session message key.
///
/// `Seed` is the compact form: the engine derives its keys when it consumes
/// the message key, and exporting a record that retained the seed returns it. `Derived` is what remains when there is no seed to return,
/// which is the case for keys persisted before the seed was retained.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionMessageKeyMaterial {
    Seed([u8; SYMMETRIC_KEY_BYTES]),
    Derived {
        cipher_key: [u8; SYMMETRIC_KEY_BYTES],
        mac_key: [u8; SYMMETRIC_KEY_BYTES],
        iv: [u8; MESSAGE_IV_BYTES],
    },
}

/// Pending key-exchange state.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct PendingKeyExchangeComponents {
    pub sequence: Option<u32>,
    pub local_base_key: Option<Vec<u8>>,
    pub local_base_key_private: Option<Vec<u8>>,
    pub local_ratchet_key: Option<Vec<u8>>,
    pub local_ratchet_key_private: Option<Vec<u8>>,
    pub local_identity_key: Option<Vec<u8>>,
    pub local_identity_key_private: Option<Vec<u8>>,
}

/// Pending pre-key state.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct PendingPreKeyComponents {
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: Option<i32>,
    pub base_key: Option<Vec<u8>>,
    pub kyber_pre_key_id: Option<u32>,
    pub kyber_ciphertext: Option<Vec<u8>>,
}

/// Complete protocol state carried by a sender-key record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SenderKeyRecordComponents {
    pub states: Vec<SenderKeyStateComponents>,
}

/// One sender-key state, including skipped message keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderKeyStateComponents {
    pub key_id: u32,
    pub chain_key: SenderChainKeyComponents,
    pub signing_key: SenderSigningKeyComponents,
    pub message_keys: Vec<SenderMessageKeyComponents>,
}

/// Position and secret material for a sender chain.
#[derive(Clone, PartialEq, Eq)]
pub struct SenderChainKeyComponents {
    pub iteration: u32,
    pub seed: Vec<u8>,
}

/// Public and optional private signing-key material.
#[derive(Clone, PartialEq, Eq)]
pub struct SenderSigningKeyComponents {
    pub public: Vec<u8>,
    pub private: Option<Vec<u8>>,
}

/// A skipped sender message key identified by its iteration.
#[derive(Clone, PartialEq, Eq)]
pub struct SenderMessageKeyComponents {
    pub iteration: u32,
    pub seed: Vec<u8>,
}

struct Redacted;

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

macro_rules! impl_redacted_debug {
    ($type:ident { visible: [$($visible:ident),* $(,)?], secret: [$($secret:ident),* $(,)?] }) => {
        impl fmt::Debug for $type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut debug = f.debug_struct(stringify!($type));
                $(debug.field(stringify!($visible), &self.$visible);)*
                $(debug.field(stringify!($secret), &Redacted);)*
                debug.finish()
            }
        }
    };
}

impl_redacted_debug!(SessionComponents {
    visible: [
        session_version,
        previous_counter,
        sender_chain,
        receiver_chains,
        pending_key_exchange,
        pending_pre_key,
        remote_registration_id,
        local_registration_id,
        needs_refresh,
    ],
    secret: [
        local_identity_public,
        remote_identity_public,
        root_key,
        alice_base_key,
    ]
});
impl_redacted_debug!(SessionChainComponents {
    visible: [chain_key, message_keys],
    secret: [sender_ratchet_key, sender_ratchet_key_private]
});
impl_redacted_debug!(SessionChainKeyComponents {
    visible: [index],
    secret: [key]
});
impl_redacted_debug!(PendingKeyExchangeComponents {
    visible: [sequence],
    secret: [
        local_base_key,
        local_base_key_private,
        local_ratchet_key,
        local_ratchet_key_private,
        local_identity_key,
        local_identity_key_private,
    ]
});
impl_redacted_debug!(PendingPreKeyComponents {
    visible: [pre_key_id, signed_pre_key_id, kyber_pre_key_id],
    secret: [base_key, kyber_ciphertext]
});
impl_redacted_debug!(SenderChainKeyComponents {
    visible: [iteration],
    secret: [seed]
});
impl_redacted_debug!(SenderSigningKeyComponents {
    visible: [],
    secret: [public, private]
});
impl_redacted_debug!(SenderMessageKeyComponents {
    visible: [iteration],
    secret: [seed]
});

impl fmt::Debug for SessionMessageKeyMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Seed(_) => f.debug_tuple("Seed").field(&Redacted).finish(),
            Self::Derived { .. } => f
                .debug_struct("Derived")
                .field("cipher_key", &Redacted)
                .field("mac_key", &Redacted)
                .field("iv", &Redacted)
                .finish(),
        }
    }
}

fn invalid(field: &'static str, expectation: &'static str) -> SignalProtocolError {
    SignalProtocolError::InvalidArgument(format!("{field} must be {expectation}"))
}

fn component_public_key(value: &[u8], field: &'static str) -> Result<PublicKey> {
    match value.len() {
        PublicKey::RAW_KEY_LEN => PublicKey::from_djb_public_key_bytes(value),
        PublicKey::SERIALIZED_KEY_LEN => PublicKey::deserialize(value),
        _ => {
            return Err(invalid(field, "a raw or canonically serialized public key"));
        }
    }
    .map_err(|_| invalid(field, "a valid public key"))
}

fn normalize_public_key(mut value: Vec<u8>, field: &'static str) -> Result<Vec<u8>> {
    let key = component_public_key(&value, field)?;

    value.clear();
    value.extend_from_slice(&key.serialize());
    Ok(value)
}

fn optional_public_key(value: Option<Vec<u8>>, field: &'static str) -> Result<Option<Vec<u8>>> {
    value
        .map(|value| normalize_public_key(value, field))
        .transpose()
}

fn required_public_key(value: Option<Vec<u8>>, field: &'static str) -> Result<Vec<u8>> {
    normalize_public_key(value.ok_or_else(|| invalid(field, "present"))?, field)
}

fn exact_bytes(value: Vec<u8>, length: usize, field: &'static str) -> Result<Vec<u8>> {
    if value.len() == length {
        Ok(value)
    } else {
        Err(SignalProtocolError::InvalidArgument(format!(
            "{field} must be {length} bytes"
        )))
    }
}

fn optional_exact_bytes(
    value: Option<Vec<u8>>,
    length: usize,
    field: &'static str,
) -> Result<Option<Vec<u8>>> {
    value
        .map(|value| exact_bytes(value, length, field))
        .transpose()
}

fn required_exact_bytes(
    value: Option<Vec<u8>>,
    length: usize,
    field: &'static str,
) -> Result<Vec<u8>> {
    exact_bytes(
        value.ok_or_else(|| invalid(field, "present"))?,
        length,
        field,
    )
}

/// Copy fixed-width key material from owned input without allocating.
fn key_material<const N: usize>(value: impl AsRef<[u8]>, field: &'static str) -> Result<[u8; N]> {
    <[u8; N]>::try_from(value.as_ref())
        .map_err(|_| SignalProtocolError::InvalidArgument(format!("{field} must be {N} bytes")))
}

fn required_key_material<const N: usize>(
    value: Option<Bytes>,
    field: &'static str,
) -> Result<[u8; N]> {
    key_material(value.ok_or_else(|| invalid(field, "present"))?, field)
}

impl SessionMessageKeyComponents {
    fn from_structure(mut value: session_structure::chain::MessageKey) -> Result<Self> {
        let index = value
            .index
            .ok_or_else(|| invalid("session message-key index", "present"))?;
        // A persisted seed supersedes the derived triple on export rather than
        // complementing it, so one that does not reproduce that triple would
        // hand a consumer a key decrypting nothing, undetectably. A record that
        // carries both wrote them from the same material; disagreeing means it
        // is corrupt. A record written since the triple stopped being persisted
        // carries the seed alone, and there the seed is simply the material.
        if let Some(seed) = value.seed.take() {
            let seed = key_material(seed, "session message-key seed")?;
            if value.cipher_key.is_some() || value.mac_key.is_some() || value.iv.is_some() {
                let derived = derived_keys(&seed)?;
                if value.cipher_key.as_deref() != Some(derived.0.as_slice())
                    || value.mac_key.as_deref() != Some(derived.1.as_slice())
                    || value.iv.as_deref() != Some(derived.2.as_slice())
                {
                    return Err(invalid(
                        "session message-key seed",
                        "consistent with the derived keys stored beside it",
                    ));
                }
            }
            return Ok(Self {
                index,
                material: SessionMessageKeyMaterial::Seed(seed),
            });
        }
        Ok(Self {
            index,
            material: SessionMessageKeyMaterial::Derived {
                cipher_key: required_key_material(value.cipher_key, "session message cipher key")?,
                mac_key: required_key_material(value.mac_key, "session message MAC key")?,
                iv: required_key_material(value.iv, "session message IV")?,
            },
        })
    }
}

impl SessionChainKeyComponents {
    fn from_structure(value: session_structure::chain::ChainKey) -> Result<Self> {
        Ok(Self {
            index: value.index,
            key: optional_exact_bytes(
                value.key.map(|value| value.to_vec()),
                SYMMETRIC_KEY_BYTES,
                "session chain key",
            )?,
        })
    }

    fn from_required_structure(value: session_structure::chain::ChainKey) -> Result<Self> {
        Ok(Self {
            index: Some(
                value
                    .index
                    .ok_or_else(|| invalid("sender chain-key index", "present"))?,
            ),
            key: Some(required_exact_bytes(
                value.key.map(|value| value.to_vec()),
                SYMMETRIC_KEY_BYTES,
                "sender chain key",
            )?),
        })
    }
}

impl SessionChainComponents {
    fn from_sender_structure(mut value: session_structure::Chain) -> Result<Self> {
        Ok(Self {
            sender_ratchet_key: Some(required_public_key(
                value.sender_ratchet_key,
                "sender ratchet public key",
            )?),
            sender_ratchet_key_private: Some(required_exact_bytes(
                value.sender_ratchet_key_private,
                PRIVATE_KEY_BYTES,
                "sender ratchet private key",
            )?),
            chain_key: Some(SessionChainKeyComponents::from_required_structure(
                value
                    .chain_key
                    .take()
                    .ok_or_else(|| invalid("sender chain key", "present"))?,
            )?),
            message_keys: value
                .message_keys
                .into_iter()
                .map(SessionMessageKeyComponents::from_structure)
                .collect::<Result<_>>()?,
        })
    }

    fn from_receiver_structure(mut value: session_structure::Chain) -> Result<Self> {
        // Local message_key_limit is retention policy, not a protocol component.
        // Record serialization preserves it; rebuilding components uses defaults.
        // The official reader ignores this field for receiver chains, including
        // historical non-canonical values.
        value.sender_ratchet_key_private = None;
        Ok(Self {
            sender_ratchet_key: optional_public_key(
                value.sender_ratchet_key,
                "receiver ratchet public key",
            )?,
            sender_ratchet_key_private: None,
            chain_key: value
                .chain_key
                .take()
                .map(SessionChainKeyComponents::from_structure)
                .transpose()?,
            message_keys: value
                .message_keys
                .into_iter()
                .map(SessionMessageKeyComponents::from_structure)
                .collect::<Result<_>>()?,
        })
    }
}

impl PendingKeyExchangeComponents {
    fn into_structure(self) -> Result<session_structure::PendingKeyExchange> {
        Ok({
            let mut proto = session_structure::PendingKeyExchange::default();
            proto.sequence = self.sequence;
            proto.local_base_key =
                optional_public_key(self.local_base_key, "local base public key")?;
            proto.local_base_key_private = optional_exact_bytes(
                self.local_base_key_private,
                PRIVATE_KEY_BYTES,
                "local base private key",
            )?;
            proto.local_ratchet_key =
                optional_public_key(self.local_ratchet_key, "local ratchet public key")?;
            proto.local_ratchet_key_private = optional_exact_bytes(
                self.local_ratchet_key_private,
                PRIVATE_KEY_BYTES,
                "local ratchet private key",
            )?;
            proto.local_identity_key =
                optional_public_key(self.local_identity_key, "local identity public key")?;
            proto.local_identity_key_private = optional_exact_bytes(
                self.local_identity_key_private,
                PRIVATE_KEY_BYTES,
                "local identity private key",
            )?;
            proto
        })
    }

    fn from_structure(value: session_structure::PendingKeyExchange) -> Result<Self> {
        Ok(Self {
            sequence: value.sequence,
            local_base_key: optional_public_key(value.local_base_key, "local base public key")?,
            local_base_key_private: optional_exact_bytes(
                value.local_base_key_private,
                PRIVATE_KEY_BYTES,
                "local base private key",
            )?,
            local_ratchet_key: optional_public_key(
                value.local_ratchet_key,
                "local ratchet public key",
            )?,
            local_ratchet_key_private: optional_exact_bytes(
                value.local_ratchet_key_private,
                PRIVATE_KEY_BYTES,
                "local ratchet private key",
            )?,
            local_identity_key: optional_public_key(
                value.local_identity_key,
                "local identity public key",
            )?,
            local_identity_key_private: optional_exact_bytes(
                value.local_identity_key_private,
                PRIVATE_KEY_BYTES,
                "local identity private key",
            )?,
        })
    }
}

impl PendingPreKeyComponents {
    fn from_structure(value: session_structure::PendingPreKey) -> Result<Self> {
        Ok(Self {
            pre_key_id: value.pre_key_id,
            signed_pre_key_id: value.signed_pre_key_id,
            base_key: optional_public_key(value.base_key, "pending pre-key base key")?,
            kyber_pre_key_id: value.kyber_pre_key_id,
            kyber_ciphertext: value.kyber_ciphertext,
        })
    }
}

pub(crate) fn session_components_from_structure(
    mut value: SessionStructure,
) -> Result<SessionComponents> {
    Ok(SessionComponents {
        session_version: value.session_version,
        local_identity_public: optional_public_key(
            value.local_identity_public,
            "local identity public key",
        )?,
        remote_identity_public: optional_public_key(
            value.remote_identity_public,
            "remote identity public key",
        )?,
        root_key: optional_exact_bytes(value.root_key, SYMMETRIC_KEY_BYTES, "session root key")?,
        previous_counter: value.previous_counter,
        sender_chain: value
            .sender_chain
            .take()
            .map(SessionChainComponents::from_sender_structure)
            .transpose()?,
        receiver_chains: value
            .receiver_chains
            .into_iter()
            .map(SessionChainComponents::from_receiver_structure)
            .collect::<Result<_>>()?,
        pending_key_exchange: value
            .pending_key_exchange
            .take()
            .map(PendingKeyExchangeComponents::from_structure)
            .transpose()?,
        pending_pre_key: value
            .pending_pre_key
            .take()
            .map(PendingPreKeyComponents::from_structure)
            .transpose()?,
        remote_registration_id: value.remote_registration_id,
        local_registration_id: value.local_registration_id,
        needs_refresh: value.needs_refresh,
        alice_base_key: optional_public_key(value.alice_base_key, "session base public key")?,
    })
}

fn sender_state_from_components(value: SenderKeyStateComponents) -> Result<engine::SenderKeyState> {
    // Keep the persisted reader's per-state budget without constructing its
    // protobuf field table. The three fixed fields precede the skipped keys.
    if value.message_keys.len() > engine::codec::MAX_STORED_FIELDS - 3 {
        return Err(engine::Error::Encoding("stored protobuf field budget exceeded").into());
    }
    Ok(engine::SenderKeyState {
        id: value.key_id,
        iteration: value.chain_key.iteration,
        chain_key: key_material(value.chain_key.seed, "sender chain seed")?,
        signing_public: *component_public_key(
            &value.signing_key.public,
            "sender signing public key",
        )?
        .public_key_bytes(),
        signing_private: value
            .signing_key
            .private
            .map(|private| key_material(private, "sender signing private key"))
            .transpose()?,
        skipped: value
            .message_keys
            .into_iter()
            .map(|key| {
                Ok(engine::SenderMessageKey {
                    iteration: key.iteration,
                    seed: key_material(key.seed, "sender message-key seed")?,
                })
            })
            .collect::<Result<_>>()?,
    })
}

fn sender_state_components(value: &engine::SenderKeyState) -> SenderKeyStateComponents {
    SenderKeyStateComponents {
        key_id: value.id,
        chain_key: SenderChainKeyComponents {
            iteration: value.iteration,
            seed: value.chain_key.to_vec(),
        },
        signing_key: SenderSigningKeyComponents {
            public: engine::wire::tagged_key(&value.signing_public).to_vec(),
            private: value
                .signing_private
                .as_ref()
                .map(|private| private.to_vec()),
        },
        message_keys: value
            .skipped
            .iter()
            .map(|key| SenderMessageKeyComponents {
                iteration: key.iteration,
                seed: key.seed.to_vec(),
            })
            .collect(),
    }
}

/// Expand a compact skipped-key seed through the independent crypto provider.
pub(crate) fn derived_keys(seed: &[u8; 32]) -> Result<([u8; 32], [u8; 32], [u8; 16])> {
    let mut material = [0; 80];
    engine::RustCrypto.hkdf_sha256_zero_salt(seed, b"WhisperMessageKeys", &mut material)?;
    let mut cipher = [0; 32];
    let mut mac = [0; 32];
    let mut iv = [0; 16];
    cipher.copy_from_slice(&material[..32]);
    mac.copy_from_slice(&material[32..64]);
    iv.copy_from_slice(&material[64..]);
    zeroize::Zeroize::zeroize(&mut material);
    Ok((cipher, mac, iv))
}

#[allow(clippy::disallowed_methods)]
fn decode_session_proto(
    bytes: &[u8],
) -> core::result::Result<SessionStructure, waproto::protobuf::DecodeError> {
    SessionStructure::decode_from_slice(bytes)
}

fn import_public(value: Option<Vec<u8>>, field: &'static str) -> Result<[u8; 32]> {
    let value = value.ok_or(engine::Error::Encoding("missing required field"))?;
    Ok(*component_public_key(&value, field)?.public_key_bytes())
}

fn chain_from_components(
    value: SessionChainComponents,
    sender: bool,
    limit: Option<u32>,
) -> Result<engine::Chain> {
    let fixed_fields = usize::from(value.sender_ratchet_key.is_some())
        + usize::from(value.sender_ratchet_key_private.is_some())
        + 1 // ChainPosition is emitted even for a closed chain without a key.
        + usize::from(!sender && limit.is_some());
    if value.message_keys.len() > engine::codec::MAX_STORED_FIELDS - fixed_fields {
        return Err(engine::Error::Encoding("stored protobuf field budget exceeded").into());
    }
    let private = if sender {
        Some(key_material(
            value
                .sender_ratchet_key_private
                .ok_or_else(|| invalid("sender ratchet private key", "present"))?,
            "sender ratchet private key",
        )?)
    } else {
        if value.sender_ratchet_key_private.is_some() {
            return Err(invalid(
                "receiver ratchet private key",
                "absent from receiver chains",
            ));
        }
        None
    };
    let position = if sender {
        Some(
            value
                .chain_key
                .ok_or_else(|| invalid("sender chain key", "present"))?,
        )
    } else {
        value.chain_key
    };
    let (index, key, exhausted) = match position {
        Some(position) if sender => (
            position
                .index
                .ok_or_else(|| invalid("sender chain-key index", "present"))?,
            key_material(
                position
                    .key
                    .ok_or_else(|| invalid("sender chain key", "present"))?,
                "sender chain key",
            )?,
            false,
        ),
        Some(position) => {
            let exhausted = position.key.is_none();
            (
                position.index.unwrap_or(0),
                position
                    .key
                    .map(|key| key_material(key, "session chain key"))
                    .transpose()?
                    .unwrap_or([0; 32]),
                exhausted,
            )
        }
        None => (0, [0; 32], true),
    };
    Ok(engine::Chain {
        ratchet: import_public(
            value.sender_ratchet_key,
            if sender {
                "sender ratchet public key"
            } else {
                "receiver ratchet public key"
            },
        )?,
        private,
        index,
        key,
        exhausted,
        skipped: value
            .message_keys
            .into_iter()
            .map(|key| match key.material {
                SessionMessageKeyMaterial::Seed(seed) => engine::MessageKey::Seed {
                    index: key.index,
                    seed,
                },
                SessionMessageKeyMaterial::Derived {
                    cipher_key,
                    mac_key,
                    iv,
                } => engine::MessageKey::Derived {
                    index: key.index,
                    material: Box::new(engine::DerivedMessageKey {
                        cipher: cipher_key,
                        mac: mac_key,
                        iv,
                    }),
                },
            })
            .collect(),
        message_key_limit: if sender { None } else { limit },
    })
}

#[allow(clippy::disallowed_methods)]
fn import_exchange(value: PendingKeyExchangeComponents) -> Result<Vec<u8>> {
    Ok(value.into_structure()?.encode_to_vec())
}

// Count absent default-valued fields separately to retain the typed handoff
// wire budget, including closed receiver chains.
fn omitted_component_bytes(value: &SessionComponents) -> usize {
    let absent = usize::from(value.previous_counter.is_none())
        + usize::from(value.remote_registration_id.is_none())
        + usize::from(value.local_registration_id.is_none())
        + usize::from(value.needs_refresh.is_none());
    let version_bytes = (u32::BITS - value.session_version.unwrap_or(3).leading_zeros())
        .div_ceil(7)
        .max(1) as usize;
    absent * 2 + version_bytes - 1
}

fn omitted_chain_bytes(value: &SessionChainComponents) -> usize {
    match &value.chain_key {
        None => 4,
        Some(position) => usize::from(position.index.is_none()) * 2,
    }
}

fn state_from_components(
    value: SessionComponents,
    limit: Option<u32>,
) -> Result<engine::SessionState> {
    let omitted_bytes = omitted_component_bytes(&value);
    let metadata = engine::SessionMetadata {
        version: value.session_version.unwrap_or(3),
        local_identity: import_public(value.local_identity_public, "local identity public key")?,
        remote_identity: import_public(value.remote_identity_public, "remote identity public key")?,
        root: key_material(
            value
                .root_key
                .ok_or(engine::Error::Encoding("missing required field"))?,
            "session root key",
        )?,
        previous_counter: value.previous_counter.unwrap_or(0),
        pending_key_exchange: value
            .pending_key_exchange
            .map(import_exchange)
            .transpose()?,
        remote_registration: value.remote_registration_id.unwrap_or(0),
        local_registration: value.local_registration_id.unwrap_or(0),
        needs_refresh: value.needs_refresh.unwrap_or(false),
        alice_base_key: value
            .alice_base_key
            .map(|key| {
                Ok::<_, SignalProtocolError>(
                    *component_public_key(&key, "session base public key")?.public_key_bytes(),
                )
            })
            .transpose()?,
        alice_base_key_tagged: true,
    };
    let pending_pre_key = value
        .pending_pre_key
        .map(|pending| {
            Ok::<_, SignalProtocolError>(engine::PendingPreKey {
                pre_key_id: pending.pre_key_id,
                base_key: import_public(pending.base_key, "pending pre-key base key")?,
                signed_pre_key_id: pending.signed_pre_key_id,
                kyber_pre_key_id: pending.kyber_pre_key_id,
                kyber_ciphertext: pending.kyber_ciphertext,
            })
        })
        .transpose()?;
    let mut state = engine::SessionState {
        metadata: std::sync::Arc::new(metadata),
        sender: value
            .sender_chain
            .map(|chain| chain_from_components(chain, true, None))
            .transpose()?,
        receivers: Vec::with_capacity(value.receiver_chains.len()),
        pending_pre_key: pending_pre_key.map(std::sync::Arc::new),
    };
    // Handoff preserves future versions and receiver histories beyond the engine's
    // operational limit. Retain the persisted reader's finite budget for the
    // metadata plus each receiver without materializing intermediate protobufs.
    let mut size = engine::codec::component_session_size(&state, omitted_bytes)?;
    for receiver in value.receiver_chains {
        let omitted_bytes = omitted_chain_bytes(&receiver);
        let chain = chain_from_components(receiver, false, limit)?;
        size = engine::codec::check_component_receiver_size(size, &chain, omitted_bytes)?;
        state.receivers.push(chain);
    }
    Ok(state)
}
fn archive(state: engine::SessionState) -> engine::ArchivedSession {
    engine::ArchivedSession {
        bytes: engine::codec::encode_session_state(&state).into(),
    }
}
fn project_state(state: &engine::SessionState) -> Result<SessionComponents> {
    let mut proto = decode_session_proto(&engine::codec::encode_session_state(state))
        .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
    if let (Some(source), Some(target)) = (&state.sender, proto.sender_chain.as_option_mut()) {
        project_stored_keys(source, target);
    }
    for (source, target) in state.receivers.iter().zip(&mut proto.receiver_chains) {
        project_stored_keys(source, target);
    }
    session_components_from_structure(proto)
}
fn project_stored_keys(source: &engine::Chain, target: &mut session_structure::Chain) {
    for (source, target) in source.skipped.iter().zip(&mut target.message_keys) {
        if !matches!(source, engine::MessageKey::Stored { .. }) {
            continue;
        }
        if source.retained_seed().is_some() {
            // Canonical projection retains a validated seed without deriving it twice.
            target.cipher_key = None;
            target.mac_key = None;
            target.iv = None;
        } else {
            target.seed = None;
        }
    }
}
fn handoff(state: &mut engine::SessionState, ceiling: u32) {
    let Some(chain) = state.sender.as_mut() else {
        return;
    };
    if ceiling.saturating_sub(chain.index) > 2000 {
        state.sender = None;
        return;
    }
    while chain.index < ceiling {
        chain.key = engine::RustCrypto.hmac_sha256_array(&chain.key, &[2]);
        chain.index += 1;
    }
}
impl SessionRecord {
    pub fn from_components(value: SessionRecordComponents) -> Result<Self> {
        Self::from_components_with_message_key_limit(value, None)
    }
    pub(crate) fn from_components_with_message_key_limit(
        value: SessionRecordComponents,
        limit: Option<u32>,
    ) -> Result<Self> {
        let current = value
            .current_session
            .map(|s| state_from_components(s, limit).map(|inner| SessionState { inner }))
            .transpose()?;
        let previous = value
            .previous_sessions
            .into_iter()
            .take(engine::MAX_ARCHIVED_SESSIONS)
            .map(|s| state_from_components(s, limit).map(archive))
            .collect::<Result<Vec<_>>>()?;
        let mut result = Self::new_fresh();
        result.current = current;
        result.inner.previous = previous.into();
        Ok(result)
    }
    /// Consume protocol components after burning all reserved sending counters.
    pub fn into_components(mut self) -> Result<SessionRecordComponents> {
        let ceiling = self.inner.reservation.ceiling.unwrap_or(0);
        let current_session = self
            .current
            .take()
            .map(|mut state| {
                handoff(&mut state.inner, ceiling);
                project_state(&state.inner)
            })
            .transpose()?;
        let previous_sessions = self
            .inner
            .previous
            .iter()
            .map(|archived| {
                let state = archived.decode()?;
                project_state(&state)
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SessionRecordComponents {
            current_session,
            previous_sessions,
        })
    }
}
#[cfg(test)]
impl From<&SessionState> for SessionStructure {
    fn from(value: &SessionState) -> Self {
        // Every engine encoder output is a valid instance of this protobuf.
        decode_session_proto(&engine::codec::encode_session_state(&value.inner))
            .expect("engine encodes valid session protobuf")
    }
}
impl SenderKeyRecord {
    pub fn from_components(value: SenderKeyRecordComponents) -> Result<Self> {
        let states = value
            .states
            .into_iter()
            .take(engine::MAX_SENDER_STATES)
            .map(sender_state_from_components)
            .collect::<Result<_>>()?;
        let mut record = Self::default();
        record.inner.states = states;
        Ok(record)
    }
    pub fn into_components(mut self) -> Result<SenderKeyRecordComponents> {
        // A reservation belongs only to the current sending chain. Historical
        // states must retain unread counters and any retained private key.
        if let Some(ceiling) = self.inner.reservation.ceiling
            && let Some(state) = self.inner.states.first_mut()
        {
            if ceiling.saturating_sub(state.iteration) > 2000 {
                state.signing_private = None;
            } else {
                while state.iteration < ceiling {
                    state.chain_key = engine::RustCrypto.hmac_sha256_array(&state.chain_key, &[2]);
                    state.iteration += 1;
                }
            }
        }
        Ok(SenderKeyRecordComponents {
            states: self
                .inner
                .states
                .iter()
                .map(sender_state_components)
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sender_components() -> SenderKeyRecordComponents {
        SenderKeyRecordComponents {
            states: [0, 127, 128, u32::MAX]
                .into_iter()
                .enumerate()
                .map(|(index, iteration)| SenderKeyStateComponents {
                    key_id: iteration,
                    chain_key: SenderChainKeyComponents {
                        iteration,
                        seed: vec![index as u8 + 1; 32],
                    },
                    signing_key: SenderSigningKeyComponents {
                        public: if index.is_multiple_of(2) {
                            vec![index as u8 + 2; 32]
                        } else {
                            key(index as u8 + 2)
                        },
                        private: index.is_multiple_of(2).then(|| vec![9; 32]),
                    },
                    message_keys: [128, 0, 128, u32::MAX]
                        .into_iter()
                        .map(|iteration| SenderMessageKeyComponents {
                            iteration,
                            seed: vec![7; 32],
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    #[test]
    fn sender_component_handoff_preserves_order_duplicates_and_canonical_keys() {
        let input = sender_components();
        let mut expected = input.clone();
        for state in &mut expected.states {
            if state.signing_key.public.len() == 32 {
                state.signing_key.public.insert(0, 5);
            }
        }
        let record = SenderKeyRecord::from_components(input).unwrap();
        let bytes = record.serialize().unwrap();
        let mut digest = sha2::Sha256::new();
        use sha2::Digest;
        digest.update(&bytes);
        assert_eq!(
            hex::encode(digest.finalize()),
            "cd58fc7d88e3de26e5767d02e6d54723e6feb9a76a4a5870b8dea6b08ee801da"
        );
        assert_eq!(record.clone().into_components().unwrap(), expected);
        let reloaded = SenderKeyRecord::deserialize(&bytes).unwrap();
        assert_eq!(reloaded.into_components().unwrap(), expected);
        assert_eq!(
            SenderKeyRecord::from_components(expected)
                .unwrap()
                .serialize()
                .unwrap(),
            bytes
        );
    }

    #[test]
    fn sender_component_import_validates_retained_states_before_handoff() {
        for length in [0, 31, 33] {
            let mut input = sender_components();
            input.states[0].chain_key.seed = vec![1; length];
            assert!(SenderKeyRecord::from_components(input).is_err());
            let mut input = sender_components();
            input.states[0].signing_key.private = Some(vec![1; length]);
            assert!(SenderKeyRecord::from_components(input).is_err());
            let mut input = sender_components();
            input.states[0].message_keys[0].seed = vec![1; length];
            assert!(SenderKeyRecord::from_components(input).is_err());
        }
        for public in [vec![], vec![1; 31], vec![4; 33], vec![5; 34]] {
            let mut input = sender_components();
            input.states[0].signing_key.public = public;
            assert!(SenderKeyRecord::from_components(input).is_err());
        }
        let mut input = sender_components();
        input.states.push(input.states[0].clone());
        let expected = SenderKeyRecord::from_components(input.clone())
            .unwrap()
            .into_components()
            .unwrap();
        let mut ignored = input.states[0].clone();
        ignored.chain_key.seed.clear();
        input.states.push(ignored);
        assert_eq!(
            SenderKeyRecord::from_components(input)
                .unwrap()
                .into_components()
                .unwrap(),
            expected
        );
        assert_eq!(expected.states.len(), 5);
        assert!(
            SenderKeyRecord::from_components(SenderKeyRecordComponents::default())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sender_component_import_keeps_the_persisted_field_budget() {
        let mut input = sender_components();
        input.states.truncate(1);
        input.states[0].message_keys = vec![
            SenderMessageKeyComponents {
                iteration: 0,
                seed: vec![7; 32],
            };
            131_069
        ];
        let record = SenderKeyRecord::from_components(input.clone()).unwrap();
        assert_eq!(
            record.into_components().unwrap().states[0]
                .message_keys
                .len(),
            131_069
        );
        input.states[0]
            .message_keys
            .push(SenderMessageKeyComponents {
                iteration: 1,
                seed: vec![8; 32],
            });
        assert!(SenderKeyRecord::from_components(input).is_err());
    }

    fn key(seed: u8) -> Vec<u8> {
        let mut key = vec![seed; 33];
        key[0] = 5;
        key
    }
    fn record() -> SessionRecordComponents {
        let state = SessionComponents {
            session_version: Some(3),
            local_identity_public: Some(key(1)),
            remote_identity_public: Some(key(2)),
            root_key: Some(vec![3; 32]),
            previous_counter: Some(0),
            sender_chain: Some(SessionChainComponents {
                sender_ratchet_key: Some(key(4)),
                sender_ratchet_key_private: Some(vec![5; 32]),
                chain_key: Some(SessionChainKeyComponents {
                    index: Some(0),
                    key: Some(vec![6; 32]),
                }),
                message_keys: Vec::new(),
            }),
            receiver_chains: vec![SessionChainComponents {
                sender_ratchet_key: Some(key(7)),
                sender_ratchet_key_private: None,
                chain_key: Some(SessionChainKeyComponents {
                    index: Some(1),
                    key: Some(vec![8; 32]),
                }),
                message_keys: vec![SessionMessageKeyComponents {
                    index: 0,
                    material: SessionMessageKeyMaterial::Seed([9; 32]),
                }],
            }],
            remote_registration_id: Some(10),
            local_registration_id: Some(11),
            needs_refresh: Some(false),
            alice_base_key: Some(key(12)),
            ..Default::default()
        };
        SessionRecordComponents {
            current_session: Some(state.clone()),
            previous_sessions: vec![state],
        }
    }
    #[test]
    #[allow(clippy::disallowed_methods)]
    fn typed_import_matches_the_stored_reader_for_synthetic_schema_values() {
        for counter in [0, 127, 128, 16_383, 16_384, u32::MAX] {
            for closed in [false, true] {
                let mut proto = SessionStructure::default();
                proto.session_version = Some(3);
                proto.local_identity_public = Some(key(1));
                proto.remote_identity_public = Some(vec![2; 32]);
                proto.root_key = Some(vec![3; 32]);
                proto.previous_counter = Some(counter);
                proto.remote_registration_id = Some(counter);
                proto.local_registration_id = Some(counter);
                proto.needs_refresh = Some(counter != 0);
                proto.alice_base_key = Some(key(4));
                let mut position = session_structure::chain::ChainKey::default();
                position.index = Some(counter);
                position.key = Some(Bytes::from_static(&[5; 32]));
                let mut sender = session_structure::Chain::default();
                sender.sender_ratchet_key = Some(vec![6; 32]);
                sender.sender_ratchet_key_private = Some(vec![7; 32]);
                sender.chain_key = Some(position.clone()).into();
                let mut seed = session_structure::chain::MessageKey::default();
                seed.index = Some(counter);
                seed.seed = Some(Bytes::from_static(&[8; 32]));
                let mut derived = session_structure::chain::MessageKey::default();
                derived.index = Some(counter);
                derived.cipher_key = Some(Bytes::from_static(&[9; 32]));
                derived.mac_key = Some(Bytes::from_static(&[10; 32]));
                derived.iv = Some(Bytes::from_static(&[11; 16]));
                // Duplicate indices and insertion order are part of handoff.
                sender.message_keys = vec![derived, seed];
                proto.sender_chain = Some(sender.clone()).into();
                sender.sender_ratchet_key = Some(key(12));
                sender.sender_ratchet_key_private = None;
                if closed {
                    position.key = None;
                    sender.chain_key = Some(position).into();
                }
                proto.receiver_chains = vec![sender];
                let mut pending = session_structure::PendingPreKey::default();
                pending.pre_key_id = Some(counter);
                pending.base_key = Some(vec![13; 32]);
                pending.signed_pre_key_id = Some(counter as i32);
                pending.kyber_pre_key_id = Some(counter);
                pending.kyber_ciphertext = Some(vec![14; 37]);
                proto.pending_pre_key = Some(pending).into();
                let mut exchange = session_structure::PendingKeyExchange::default();
                exchange.sequence = Some(counter);
                exchange.local_base_key = Some(key(15));
                exchange.local_base_key_private = Some(vec![16; 32]);
                exchange.local_ratchet_key = Some(key(17));
                exchange.local_ratchet_key_private = Some(vec![18; 32]);
                exchange.local_identity_key = Some(key(19));
                exchange.local_identity_key_private = Some(vec![20; 32]);
                proto.pending_key_exchange = Some(exchange).into();
                for limit in [None, Some(0), Some(8192)] {
                    proto.receiver_chains[0].message_key_limit = limit;
                    let expected =
                        engine::codec::decode_session_state(&proto.encode_to_vec()).unwrap();
                    let input = session_components_from_structure(proto.clone()).unwrap();
                    let actual = state_from_components(input, limit).unwrap();
                    assert!(
                        actual == expected,
                        "counter={counter}, closed={closed}, limit={limit:?}"
                    );
                    assert_eq!(
                        engine::codec::encode_session_state(&actual),
                        engine::codec::encode_session_state(&expected)
                    );
                }
            }
        }
    }

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn typed_import_size_accounts_for_absent_defaults_and_future_versions() {
        for mask in 0..16 {
            for version in [0, 3, 999, u32::MAX] {
                let mut proto = SessionStructure::default();
                proto.session_version = Some(version);
                proto.local_identity_public = Some(key(1));
                proto.remote_identity_public = Some(key(2));
                proto.root_key = Some(vec![3; 32]);
                proto.previous_counter = (mask & 1 != 0).then_some(u32::MAX);
                proto.remote_registration_id = (mask & 2 != 0).then_some(1 << 29);
                proto.local_registration_id = (mask & 4 != 0).then_some(1 << 21);
                proto.needs_refresh = (mask & 8 != 0).then_some(false);
                let components = session_components_from_structure(proto.clone()).unwrap();
                let omitted = omitted_component_bytes(&components);
                let imported = state_from_components(components, None).unwrap();
                // Handoff retains future version metadata but counts the
                // version-3 storage representation for its size budget.
                proto.session_version = Some(3);
                assert_eq!(
                    engine::codec::component_session_size(&imported, omitted).unwrap(),
                    proto.encode_to_vec().len()
                );
            }
        }
    }

    #[test]
    fn typed_import_preserves_closed_chains_future_metadata_and_extended_history() {
        let mut input = record().current_session.unwrap();
        input.session_version = Some(999);
        input.receiver_chains[0].chain_key = None;
        input
            .receiver_chains
            .resize(7, input.receiver_chains[0].clone());
        let imported = state_from_components(input, Some(4096)).unwrap();
        assert_eq!(imported.version, 999);
        assert_eq!(imported.receivers.len(), 7);
        for receiver in &imported.receivers {
            assert!(receiver.exhausted);
            assert_eq!(receiver.index, 0);
            assert_eq!(receiver.message_key_limit, Some(4096));
            assert_eq!(receiver.skipped.len(), 1);
        }
    }

    #[test]
    fn extended_receiver_history_survives_current_and_archived_storage_roundtrips() {
        for version in [3, 4] {
            let mut input = record();
            for state in input
                .current_session
                .iter_mut()
                .chain(&mut input.previous_sessions)
            {
                state.session_version = Some(version);
                state
                    .receiver_chains
                    .resize(7, state.receiver_chains[0].clone());
            }
            let imported = SessionRecord::from_components(input.clone()).unwrap();
            assert_eq!(imported.clone().into_components().unwrap(), input);
            let reloaded = SessionRecord::deserialize(&imported.serialize().unwrap()).unwrap();
            assert_eq!(reloaded.into_components().unwrap(), input);
        }
    }

    #[test]
    fn closed_component_chain_counts_its_always_encoded_position_field() {
        let mut input = record();
        let chain = &mut input.current_session.as_mut().unwrap().receiver_chains[0];
        chain.chain_key = None;
        chain.message_keys.resize(
            engine::codec::MAX_STORED_FIELDS - 1,
            chain.message_keys[0].clone(),
        );
        assert!(SessionRecord::from_components(input).is_err());
    }

    #[test]
    fn typed_import_counts_all_receivers_against_the_session_storage_budget() {
        let mut input = record().current_session.unwrap();
        input.pending_pre_key = Some(PendingPreKeyComponents {
            base_key: Some(key(21)),
            kyber_ciphertext: Some(vec![0; engine::wire::MAX_STORAGE_SIZE - 1024]),
            ..Default::default()
        });
        input
            .receiver_chains
            .resize(32, input.receiver_chains[0].clone());
        assert!(matches!(
            state_from_components(input, None),
            Err(SignalProtocolError::Engine(engine::Error::Encoding(
                "stored protobuf size limit"
            )))
        ));
    }

    #[test]
    fn typed_pairwise_import_keeps_the_stored_field_budget() {
        for sender in [false, true] {
            for limit in [None, Some(4096)] {
                let mut input = record().current_session.unwrap();
                let chain = if sender {
                    input.sender_chain.take().unwrap()
                } else {
                    input.receiver_chains.remove(0)
                };
                let fixed = if sender || limit.is_some() { 3 } else { 2 };
                let mut chain = SessionChainComponents {
                    message_keys: vec![
                        SessionMessageKeyComponents {
                            index: 0,
                            material: SessionMessageKeyMaterial::Seed([1; 32]),
                        };
                        engine::codec::MAX_STORED_FIELDS - fixed
                    ],
                    ..chain
                };
                assert!(chain_from_components(chain.clone(), sender, limit).is_ok());
                chain.message_keys.push(SessionMessageKeyComponents {
                    index: 1,
                    material: SessionMessageKeyMaterial::Seed([2; 32]),
                });
                assert!(chain_from_components(chain, sender, limit).is_err());
            }
        }
    }

    #[test]
    fn oversized_typed_prekey_payload_is_rejected_before_serialization() {
        let mut input = record().current_session.unwrap();
        input.pending_pre_key = Some(PendingPreKeyComponents {
            base_key: Some(key(21)),
            kyber_ciphertext: Some(vec![0; engine::wire::MAX_STORAGE_SIZE]),
            ..Default::default()
        });
        assert!(matches!(
            state_from_components(input, None),
            Err(SignalProtocolError::Engine(engine::Error::Encoding(
                "stored protobuf size limit"
            )))
        ));
    }

    #[test]
    fn component_storage_roundtrip_preserves_seed_and_archive() {
        let expected = record();
        let record = SessionRecord::from_components(expected.clone()).unwrap();
        let loaded = SessionRecord::deserialize(&record.serialize().unwrap()).unwrap();
        assert_eq!(loaded.into_components().unwrap(), expected);
    }
    #[test]
    #[allow(clippy::disallowed_methods)]
    fn expanded_storage_roundtrip_validates_seeds_only_for_canonical_projection() {
        let (cipher_key, mac_key, iv) = derived_keys(&[9; 32]).unwrap();
        for inconsistent in [false, true] {
            let mut expected = record();
            let native = SessionRecord::from_components(expected.clone()).unwrap();
            let mut proto =
                waproto::whatsapp::RecordStructure::decode_from_slice(&native.serialize().unwrap())
                    .unwrap();
            for session in proto
                .current_session
                .as_option_mut()
                .into_iter()
                .chain(&mut proto.previous_sessions)
            {
                let key = &mut session.receiver_chains[0].message_keys[0];
                key.cipher_key = Some(Bytes::copy_from_slice(&cipher_key));
                key.mac_key = Some(Bytes::copy_from_slice(&mac_key));
                key.iv = Some(Bytes::copy_from_slice(&iv));
                if inconsistent {
                    key.seed = Some(Bytes::from_static(&[44; 32]));
                }
            }
            if inconsistent {
                for session in expected
                    .current_session
                    .iter_mut()
                    .chain(&mut expected.previous_sessions)
                {
                    session.receiver_chains[0].message_keys[0].material =
                        SessionMessageKeyMaterial::Derived {
                            cipher_key,
                            mac_key,
                            iv,
                        };
                }
            }
            let stored = proto.encode_to_vec();
            let loaded = SessionRecord::deserialize(&stored).unwrap();
            assert_eq!(loaded.serialize().unwrap(), stored);
            assert_eq!(loaded.clone().into_components().unwrap(), expected);
            assert_eq!(loaded.serialize().unwrap(), stored);
        }
    }
    #[test]
    fn receiver_private_import_is_rejected_and_sender_must_be_complete() {
        let mut components = record();
        components.current_session.as_mut().unwrap().receiver_chains[0]
            .sender_ratchet_key_private = Some(Vec::new());
        assert!(SessionRecord::from_components(components).is_err());
        let mut components = record();
        components
            .current_session
            .as_mut()
            .unwrap()
            .sender_chain
            .as_mut()
            .unwrap()
            .sender_ratchet_key_private = None;
        assert!(SessionRecord::from_components(components).is_err());
    }
    #[test]
    fn handoff_burns_only_current_counters_and_preserves_archived_senders() {
        let mut native = SessionRecord::from_components(record()).unwrap();
        native.reserve_sender_chain_counters(0);
        let expected = native.reserved_sender_chain_index();
        let projected = native.into_components().unwrap();
        for session in projected.current_session.iter() {
            assert_eq!(
                session
                    .sender_chain
                    .as_ref()
                    .unwrap()
                    .chain_key
                    .as_ref()
                    .unwrap()
                    .index,
                Some(expected)
            );
        }
        assert_eq!(projected.previous_sessions, record().previous_sessions);
        let mut native = SessionRecord::from_components(record()).unwrap();
        native.reserve_sender_chain_counters(2001);
        let projected = native.into_components().unwrap();
        assert!(projected.current_session.unwrap().sender_chain.is_none());
        assert_eq!(projected.previous_sessions, record().previous_sessions);
    }
    #[test]
    fn conflicting_seed_and_expanded_keys_fail_projection() {
        let mut proto = session_structure::chain::MessageKey::default();
        proto.index = Some(3);
        proto.seed = Some(Bytes::from_static(&[1; 32]));
        proto.cipher_key = Some(Bytes::from_static(&[2; 32]));
        proto.mac_key = Some(Bytes::from_static(&[3; 32]));
        proto.iv = Some(Bytes::from_static(&[4; 16]));
        assert!(SessionMessageKeyComponents::from_structure(proto).is_err());
    }
    #[test]
    fn public_keys_normalize_and_debug_redacts_secrets() {
        assert_eq!(normalize_public_key(vec![7; 32], "test").unwrap(), key(7));
        let mut invalid = key(7);
        invalid[0] = 6;
        assert!(normalize_public_key(invalid, "test").is_err());
        let material = SessionMessageKeyMaterial::Seed([42; 32]);
        assert_eq!(format!("{material:?}"), "Seed(<redacted>)");
    }
}
