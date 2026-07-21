//! Owned, validated projections of persisted Signal records.
//!
//! These types keep codec-generated structures private while providing a
//! stable representation for record handoff. Conversion validates fixed-width
//! key material and emits public keys in their canonical serialized form. The
//! explicit field mappings are intentional: a generated schema change must
//! fail to compile here instead of silently dropping protocol state.

use std::fmt;

use buffa::MessageField;
use bytes::Bytes;

use crate::core::curve::PublicKey;
use crate::protocol::error::{Result, SignalProtocolError};
use crate::protocol::ratchet::MessageKeyGenerator;
use crate::protocol::stores::{
    SenderKeyStateStructure, SessionStructure, sender_key_state_structure, session_structure,
};

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
/// `Seed` is accepted as a compact import form and is expanded with the
/// protocol's canonical derivation. Exported records always use `Derived`.
#[derive(Clone, PartialEq, Eq)]
pub enum SessionMessageKeyMaterial {
    Seed(Vec<u8>),
    Derived {
        cipher_key: Vec<u8>,
        mac_key: Vec<u8>,
        iv: Vec<u8>,
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

fn normalize_public_key(mut value: Vec<u8>, field: &'static str) -> Result<Vec<u8>> {
    let key = match value.len() {
        PublicKey::RAW_KEY_LEN => PublicKey::from_djb_public_key_bytes(&value),
        PublicKey::SERIALIZED_KEY_LEN => PublicKey::deserialize(&value),
        _ => {
            return Err(invalid(field, "a raw or canonically serialized public key"));
        }
    }
    .map_err(|_| invalid(field, "a valid public key"))?;

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

impl SessionMessageKeyComponents {
    fn into_structure(self) -> Result<session_structure::chain::MessageKey> {
        match self.material {
            SessionMessageKeyMaterial::Seed(seed) => {
                let seed: [u8; SYMMETRIC_KEY_BYTES] = seed
                    .try_into()
                    .map_err(|_| invalid("session message-key seed", "32 bytes"))?;
                Ok(MessageKeyGenerator::new_from_seed(&seed, self.index).into_pb())
            }
            SessionMessageKeyMaterial::Derived {
                cipher_key,
                mac_key,
                iv,
            } => Ok(session_structure::chain::MessageKey {
                index: Some(self.index),
                cipher_key: Some(Bytes::from(exact_bytes(
                    cipher_key,
                    SYMMETRIC_KEY_BYTES,
                    "session message cipher key",
                )?)),
                mac_key: Some(Bytes::from(exact_bytes(
                    mac_key,
                    SYMMETRIC_KEY_BYTES,
                    "session message MAC key",
                )?)),
                iv: Some(Bytes::from(exact_bytes(
                    iv,
                    MESSAGE_IV_BYTES,
                    "session message IV",
                )?)),
            }),
        }
    }

    fn from_structure(value: session_structure::chain::MessageKey) -> Result<Self> {
        Ok(Self {
            index: value
                .index
                .ok_or_else(|| invalid("session message-key index", "present"))?,
            material: SessionMessageKeyMaterial::Derived {
                cipher_key: exact_bytes(
                    value
                        .cipher_key
                        .ok_or_else(|| invalid("session message cipher key", "present"))?
                        .to_vec(),
                    SYMMETRIC_KEY_BYTES,
                    "session message cipher key",
                )?,
                mac_key: exact_bytes(
                    value
                        .mac_key
                        .ok_or_else(|| invalid("session message MAC key", "present"))?
                        .to_vec(),
                    SYMMETRIC_KEY_BYTES,
                    "session message MAC key",
                )?,
                iv: exact_bytes(
                    value
                        .iv
                        .ok_or_else(|| invalid("session message IV", "present"))?
                        .to_vec(),
                    MESSAGE_IV_BYTES,
                    "session message IV",
                )?,
            },
        })
    }
}

impl SessionChainKeyComponents {
    fn into_structure(self) -> Result<session_structure::chain::ChainKey> {
        Ok(session_structure::chain::ChainKey {
            index: self.index,
            key: optional_exact_bytes(self.key, SYMMETRIC_KEY_BYTES, "session chain key")?
                .map(Bytes::from),
        })
    }

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

    fn into_required_structure(self) -> Result<session_structure::chain::ChainKey> {
        Ok(session_structure::chain::ChainKey {
            index: Some(
                self.index
                    .ok_or_else(|| invalid("sender chain-key index", "present"))?,
            ),
            key: Some(Bytes::from(required_exact_bytes(
                self.key,
                SYMMETRIC_KEY_BYTES,
                "sender chain key",
            )?)),
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
    fn into_sender_structure(self) -> Result<session_structure::Chain> {
        Ok(session_structure::Chain {
            sender_ratchet_key: Some(required_public_key(
                self.sender_ratchet_key,
                "sender ratchet public key",
            )?),
            sender_ratchet_key_private: Some(required_exact_bytes(
                self.sender_ratchet_key_private,
                PRIVATE_KEY_BYTES,
                "sender ratchet private key",
            )?),
            chain_key: MessageField::some(
                self.chain_key
                    .ok_or_else(|| invalid("sender chain key", "present"))?
                    .into_required_structure()?,
            ),
            message_keys: self
                .message_keys
                .into_iter()
                .map(SessionMessageKeyComponents::into_structure)
                .collect::<Result<_>>()?,
        })
    }

    fn into_receiver_structure(self) -> Result<session_structure::Chain> {
        if self.sender_ratchet_key_private.is_some() {
            return Err(invalid(
                "receiver ratchet private key",
                "absent from receiver chains",
            ));
        }

        Ok(session_structure::Chain {
            sender_ratchet_key: optional_public_key(
                self.sender_ratchet_key,
                "receiver ratchet public key",
            )?,
            sender_ratchet_key_private: None,
            chain_key: self
                .chain_key
                .map(SessionChainKeyComponents::into_structure)
                .transpose()?
                .into(),
            message_keys: self
                .message_keys
                .into_iter()
                .map(SessionMessageKeyComponents::into_structure)
                .collect::<Result<_>>()?,
        })
    }

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
        Ok(session_structure::PendingKeyExchange {
            sequence: self.sequence,
            local_base_key: optional_public_key(self.local_base_key, "local base public key")?,
            local_base_key_private: optional_exact_bytes(
                self.local_base_key_private,
                PRIVATE_KEY_BYTES,
                "local base private key",
            )?,
            local_ratchet_key: optional_public_key(
                self.local_ratchet_key,
                "local ratchet public key",
            )?,
            local_ratchet_key_private: optional_exact_bytes(
                self.local_ratchet_key_private,
                PRIVATE_KEY_BYTES,
                "local ratchet private key",
            )?,
            local_identity_key: optional_public_key(
                self.local_identity_key,
                "local identity public key",
            )?,
            local_identity_key_private: optional_exact_bytes(
                self.local_identity_key_private,
                PRIVATE_KEY_BYTES,
                "local identity private key",
            )?,
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
    fn into_structure(self) -> Result<session_structure::PendingPreKey> {
        Ok(session_structure::PendingPreKey {
            pre_key_id: self.pre_key_id,
            signed_pre_key_id: self.signed_pre_key_id,
            base_key: optional_public_key(self.base_key, "pending pre-key base key")?,
            kyber_pre_key_id: self.kyber_pre_key_id,
            kyber_ciphertext: self.kyber_ciphertext,
        })
    }

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

pub(crate) fn session_structure_from_components(
    value: SessionComponents,
) -> Result<SessionStructure> {
    Ok(SessionStructure {
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
            .map(SessionChainComponents::into_sender_structure)
            .transpose()?
            .into(),
        receiver_chains: value
            .receiver_chains
            .into_iter()
            .map(SessionChainComponents::into_receiver_structure)
            .collect::<Result<_>>()?,
        pending_key_exchange: value
            .pending_key_exchange
            .map(PendingKeyExchangeComponents::into_structure)
            .transpose()?
            .into(),
        pending_pre_key: value
            .pending_pre_key
            .map(PendingPreKeyComponents::into_structure)
            .transpose()?
            .into(),
        remote_registration_id: value.remote_registration_id,
        local_registration_id: value.local_registration_id,
        needs_refresh: value.needs_refresh,
        alice_base_key: optional_public_key(value.alice_base_key, "session base public key")?,
    })
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

pub(crate) fn sender_state_structure_from_components(
    value: SenderKeyStateComponents,
) -> Result<SenderKeyStateStructure> {
    let chain_seed = exact_bytes(
        value.chain_key.seed,
        SYMMETRIC_KEY_BYTES,
        "sender chain seed",
    )?;
    let signing_public =
        normalize_public_key(value.signing_key.public, "sender signing public key")?;
    let signing_private = optional_exact_bytes(
        value.signing_key.private,
        PRIVATE_KEY_BYTES,
        "sender signing private key",
    )?;
    Ok(SenderKeyStateStructure {
        sender_key_id: Some(value.key_id),
        sender_chain_key: MessageField::some(sender_key_state_structure::SenderChainKey {
            iteration: Some(value.chain_key.iteration),
            seed: Some(Bytes::from(chain_seed)),
        }),
        sender_signing_key: MessageField::some(sender_key_state_structure::SenderSigningKey {
            public: Some(Bytes::from(signing_public)),
            private: signing_private.map(Bytes::from),
        }),
        sender_message_keys: value
            .message_keys
            .into_iter()
            .map(|key| {
                Ok(sender_key_state_structure::SenderMessageKey {
                    iteration: Some(key.iteration),
                    seed: Some(Bytes::from(exact_bytes(
                        key.seed,
                        SYMMETRIC_KEY_BYTES,
                        "sender message-key seed",
                    )?)),
                })
            })
            .collect::<Result<_>>()?,
    })
}

pub(crate) fn sender_state_components_from_structure(
    mut value: SenderKeyStateStructure,
) -> Result<SenderKeyStateComponents> {
    let chain = value
        .sender_chain_key
        .take()
        .ok_or_else(|| invalid("sender chain key", "present"))?;
    let signing = value
        .sender_signing_key
        .take()
        .ok_or_else(|| invalid("sender signing key", "present"))?;
    Ok(SenderKeyStateComponents {
        key_id: value
            .sender_key_id
            .ok_or_else(|| invalid("sender key id", "present"))?,
        chain_key: SenderChainKeyComponents {
            iteration: chain
                .iteration
                .ok_or_else(|| invalid("sender chain iteration", "present"))?,
            seed: exact_bytes(
                chain
                    .seed
                    .ok_or_else(|| invalid("sender chain seed", "present"))?
                    .to_vec(),
                SYMMETRIC_KEY_BYTES,
                "sender chain seed",
            )?,
        },
        signing_key: SenderSigningKeyComponents {
            public: normalize_public_key(
                signing
                    .public
                    .ok_or_else(|| invalid("sender signing public key", "present"))?
                    .to_vec(),
                "sender signing public key",
            )?,
            private: optional_exact_bytes(
                signing.private.map(|value| value.to_vec()),
                PRIVATE_KEY_BYTES,
                "sender signing private key",
            )?,
        },
        message_keys: value
            .sender_message_keys
            .into_iter()
            .map(|key| {
                Ok(SenderMessageKeyComponents {
                    iteration: key
                        .iteration
                        .ok_or_else(|| invalid("sender message-key iteration", "present"))?,
                    seed: exact_bytes(
                        key.seed
                            .ok_or_else(|| invalid("sender message-key seed", "present"))?
                            .to_vec(),
                        SYMMETRIC_KEY_BYTES,
                        "sender message-key seed",
                    )?,
                })
            })
            .collect::<Result<_>>()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{
        ChainKey, IdentityKey, KeyPair, RootKey, SenderKeyRecord, SessionRecord, SessionState,
    };

    #[derive(Debug, Clone, Copy)]
    enum SenderChainFault {
        MissingPrivateKey,
        PrivateKeyLength(usize),
        MissingPublicKey,
        MissingChainKey,
        MissingChainKeyIndex,
        MissingChainKeySecret,
    }

    fn public_key(seed: u8) -> Vec<u8> {
        PublicKey::from_djb_public_key_bytes(&[seed; PublicKey::RAW_KEY_LEN])
            .expect("valid test key")
            .serialize()
            .to_vec()
    }

    fn canonical_session_state() -> SessionState {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let local_identity = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let remote_identity = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let base_key = KeyPair::generate(&mut rng).public_key;
        let mut state = SessionState::new(
            3,
            &local_identity,
            &remote_identity,
            &RootKey::new([3; SYMMETRIC_KEY_BYTES]),
            &base_key,
        );
        state.set_sender_chain(
            &KeyPair::generate(&mut rng),
            &ChainKey::new([4; SYMMETRIC_KEY_BYTES], 5),
        );
        state.add_receiver_chain(
            &KeyPair::generate(&mut rng).public_key,
            &ChainKey::new([6; SYMMETRIC_KEY_BYTES], 7),
        );
        state
    }

    fn canonical_session_components() -> SessionRecordComponents {
        SessionRecord::new(canonical_session_state())
            .into_components()
            .expect("canonical session projects")
    }

    fn sender_chain_faults() -> [SenderChainFault; 8] {
        [
            SenderChainFault::MissingPrivateKey,
            SenderChainFault::PrivateKeyLength(0),
            SenderChainFault::PrivateKeyLength(PRIVATE_KEY_BYTES - 1),
            SenderChainFault::PrivateKeyLength(PRIVATE_KEY_BYTES + 1),
            SenderChainFault::MissingPublicKey,
            SenderChainFault::MissingChainKey,
            SenderChainFault::MissingChainKeyIndex,
            SenderChainFault::MissingChainKeySecret,
        ]
    }

    fn corrupt_sender_components(
        components: &mut SessionRecordComponents,
        fault: SenderChainFault,
    ) {
        let sender = components
            .current_session
            .as_mut()
            .and_then(|session| session.sender_chain.as_mut())
            .expect("canonical sender chain");
        match fault {
            SenderChainFault::MissingPrivateKey => sender.sender_ratchet_key_private = None,
            SenderChainFault::PrivateKeyLength(length) => {
                sender.sender_ratchet_key_private = Some(vec![0; length]);
            }
            SenderChainFault::MissingPublicKey => sender.sender_ratchet_key = None,
            SenderChainFault::MissingChainKey => sender.chain_key = None,
            SenderChainFault::MissingChainKeyIndex => {
                sender
                    .chain_key
                    .as_mut()
                    .expect("canonical chain key")
                    .index = None;
            }
            SenderChainFault::MissingChainKeySecret => {
                sender.chain_key.as_mut().expect("canonical chain key").key = None;
            }
        }
    }

    fn corrupt_persisted_sender(session: &mut SessionStructure, fault: SenderChainFault) {
        let sender = session
            .sender_chain
            .as_option_mut()
            .expect("canonical sender chain");
        match fault {
            SenderChainFault::MissingPrivateKey => sender.sender_ratchet_key_private = None,
            SenderChainFault::PrivateKeyLength(length) => {
                sender.sender_ratchet_key_private = Some(vec![0; length]);
            }
            SenderChainFault::MissingPublicKey => sender.sender_ratchet_key = None,
            SenderChainFault::MissingChainKey => sender.chain_key = MessageField::none(),
            SenderChainFault::MissingChainKeyIndex => {
                sender
                    .chain_key
                    .as_option_mut()
                    .expect("canonical chain key")
                    .index = None;
            }
            SenderChainFault::MissingChainKeySecret => {
                sender
                    .chain_key
                    .as_option_mut()
                    .expect("canonical chain key")
                    .key = None;
            }
        }
    }

    fn session_record() -> SessionRecordComponents {
        let current_session = SessionComponents {
            session_version: Some(3),
            local_identity_public: Some(public_key(1)),
            remote_identity_public: Some(public_key(2)),
            root_key: Some(vec![3; 32]),
            previous_counter: Some(0),
            sender_chain: Some(SessionChainComponents {
                sender_ratchet_key: Some(public_key(4)),
                sender_ratchet_key_private: Some(vec![5; 32]),
                chain_key: Some(SessionChainKeyComponents {
                    index: Some(0),
                    key: Some(vec![6; 32]),
                }),
                message_keys: vec![SessionMessageKeyComponents {
                    index: 1,
                    material: SessionMessageKeyMaterial::Derived {
                        cipher_key: vec![16; 32],
                        mac_key: vec![17; 32],
                        iv: vec![18; 16],
                    },
                }],
            }),
            receiver_chains: vec![SessionChainComponents {
                sender_ratchet_key: Some(public_key(7)),
                sender_ratchet_key_private: None,
                chain_key: Some(SessionChainKeyComponents {
                    index: Some(2),
                    key: Some(vec![8; 32]),
                }),
                message_keys: vec![SessionMessageKeyComponents {
                    index: 3,
                    material: SessionMessageKeyMaterial::Derived {
                        cipher_key: vec![19; 32],
                        mac_key: vec![20; 32],
                        iv: vec![21; 16],
                    },
                }],
            }],
            pending_key_exchange: Some(PendingKeyExchangeComponents {
                sequence: Some(22),
                local_base_key: Some(public_key(23)),
                local_base_key_private: Some(vec![24; 32]),
                local_ratchet_key: Some(public_key(25)),
                local_ratchet_key_private: Some(vec![26; 32]),
                local_identity_key: Some(public_key(27)),
                local_identity_key_private: Some(vec![28; 32]),
            }),
            pending_pre_key: Some(PendingPreKeyComponents {
                pre_key_id: Some(29),
                signed_pre_key_id: Some(30),
                base_key: Some(public_key(31)),
                kyber_pre_key_id: Some(32),
                kyber_ciphertext: Some(vec![33; 48]),
            }),
            remote_registration_id: Some(9),
            local_registration_id: Some(10),
            needs_refresh: Some(false),
            alice_base_key: Some(public_key(11)),
        };
        SessionRecordComponents {
            current_session: Some(current_session.clone()),
            previous_sessions: vec![current_session],
        }
    }

    fn sender_key_record() -> SenderKeyRecordComponents {
        SenderKeyRecordComponents {
            states: vec![
                SenderKeyStateComponents {
                    key_id: 17,
                    chain_key: SenderChainKeyComponents {
                        iteration: 0,
                        seed: vec![12; 32],
                    },
                    signing_key: SenderSigningKeyComponents {
                        public: public_key(13),
                        private: Some(vec![14; 32]),
                    },
                    message_keys: vec![SenderMessageKeyComponents {
                        iteration: 3,
                        seed: vec![15; 32],
                    }],
                },
                SenderKeyStateComponents {
                    key_id: 18,
                    chain_key: SenderChainKeyComponents {
                        iteration: 4,
                        seed: vec![16; 32],
                    },
                    signing_key: SenderSigningKeyComponents {
                        public: public_key(17),
                        private: None,
                    },
                    message_keys: Vec::new(),
                },
            ],
        }
    }

    #[test]
    fn raw_public_keys_are_normalized_once() {
        let normalized = normalize_public_key(vec![7; 32], "test key").expect("valid key");
        assert_eq!(normalized.len(), PublicKey::SERIALIZED_KEY_LEN);
        assert_eq!(normalized[0], crate::core::curve::KeyType::Djb.value());
        assert_eq!(&normalized[1..], &[7; 32]);
    }

    #[test]
    fn invalid_serialized_public_key_type_is_rejected() {
        let mut key = public_key(7);
        key[0] = u8::MAX;

        assert!(normalize_public_key(key, "test key").is_err());
    }

    #[test]
    fn debug_output_redacts_key_material() {
        let chain = SessionChainKeyComponents {
            index: Some(7),
            key: Some(vec![42; 32]),
        };
        let material = SessionMessageKeyMaterial::Derived {
            cipher_key: vec![1; 32],
            mac_key: vec![2; 32],
            iv: vec![3; 16],
        };

        assert_eq!(
            format!("{chain:?}"),
            "SessionChainKeyComponents { index: Some(7), key: <redacted> }"
        );
        assert_eq!(
            format!("{material:?}"),
            "Derived { cipher_key: <redacted>, mac_key: <redacted>, iv: <redacted> }"
        );
    }

    #[test]
    fn message_key_seed_uses_the_existing_derivation() {
        let seed = [9; 32];
        let expected = MessageKeyGenerator::new_from_seed(&seed, 17).into_pb();
        let actual = SessionMessageKeyComponents {
            index: 17,
            material: SessionMessageKeyMaterial::Seed(seed.to_vec()),
        }
        .into_structure()
        .expect("valid seed");

        assert_eq!(actual, expected);
    }

    #[test]
    fn invalid_seed_length_is_rejected() {
        let error = SessionMessageKeyComponents {
            index: 1,
            material: SessionMessageKeyMaterial::Seed(vec![0; 31]),
        }
        .into_structure()
        .expect_err("short seed must fail");

        assert!(matches!(error, SignalProtocolError::InvalidArgument(_)));
    }

    #[test]
    fn canonical_session_api_round_trips_through_components() {
        let state = canonical_session_state();
        let structure = SessionStructure::from(&state);
        assert_eq!(
            structure.receiver_chains[0].sender_ratchet_key_private,
            Some(Vec::new())
        );
        assert_eq!(
            structure
                .sender_chain
                .as_option()
                .and_then(|chain| chain.sender_ratchet_key_private.as_ref())
                .map(Vec::len),
            Some(PRIVATE_KEY_BYTES)
        );

        let persisted = SessionRecord::new(state)
            .serialize()
            .expect("serialize canonical record");
        let projected = SessionRecord::deserialize(&persisted)
            .expect("deserialize canonical record")
            .into_components()
            .expect("project canonical record");
        let session = projected
            .current_session
            .as_ref()
            .expect("test session is present");
        assert_eq!(session.receiver_chains[0].sender_ratchet_key_private, None);
        assert_eq!(
            session
                .sender_chain
                .as_ref()
                .and_then(|chain| chain.sender_ratchet_key_private.as_ref())
                .map(Vec::len),
            Some(PRIVATE_KEY_BYTES)
        );

        let rebuilt = SessionRecord::from_components(projected.clone())
            .expect("rebuild canonical record")
            .serialize()
            .expect("serialize rebuilt record");
        let reprojected = SessionRecord::deserialize(&rebuilt)
            .expect("deserialize rebuilt record")
            .into_components()
            .expect("project rebuilt record");
        assert_eq!(reprojected, projected);
    }

    #[test]
    fn persisted_receiver_private_material_is_ignored() {
        for (case, private_key) in [
            ("absent", None),
            ("empty", Some(Vec::new())),
            ("one byte", Some(vec![0; 1])),
            ("short key", Some(vec![0; PRIVATE_KEY_BYTES - 1])),
            ("key-sized", Some(vec![0; PRIVATE_KEY_BYTES])),
            ("long key", Some(vec![0; PRIVATE_KEY_BYTES + 1])),
        ] {
            let mut persisted = SessionStructure::from(canonical_session_state());
            persisted.receiver_chains[0].sender_ratchet_key_private = private_key;

            let projected = session_components_from_structure(persisted).unwrap_or_else(|error| {
                panic!("{case} receiver material must be ignored: {error}")
            });
            assert_eq!(
                projected.receiver_chains[0].sender_ratchet_key_private, None,
                "{case}"
            );
        }
    }

    #[test]
    fn receiver_component_import_rejects_private_material() {
        for private_key in [Vec::new(), vec![0; PRIVATE_KEY_BYTES]] {
            let mut components = canonical_session_components();
            components
                .current_session
                .as_mut()
                .expect("canonical session")
                .receiver_chains[0]
                .sender_ratchet_key_private = Some(private_key);

            let error = SessionRecord::from_components(components)
                .err()
                .expect("receiver components must not carry private material");
            assert!(matches!(error, SignalProtocolError::InvalidArgument(_)));
        }
    }

    #[test]
    fn sender_component_import_requires_a_complete_chain() {
        for fault in sender_chain_faults() {
            let mut components = canonical_session_components();
            corrupt_sender_components(&mut components, fault);

            let error = SessionRecord::from_components(components)
                .err()
                .unwrap_or_else(|| panic!("sender fault {fault:?} must fail"));
            assert!(
                matches!(error, SignalProtocolError::InvalidArgument(_)),
                "{fault:?}: {error}"
            );
        }
    }

    #[test]
    fn persisted_sender_projection_requires_a_complete_chain() {
        for fault in sender_chain_faults() {
            let mut persisted = SessionStructure::from(canonical_session_state());
            corrupt_persisted_sender(&mut persisted, fault);

            let error = session_components_from_structure(persisted)
                .err()
                .unwrap_or_else(|| panic!("persisted sender fault {fault:?} must fail"));
            assert!(
                matches!(error, SignalProtocolError::InvalidArgument(_)),
                "{fault:?}: {error}"
            );
        }
    }

    #[test]
    fn component_import_allows_an_absent_sender_chain() {
        let mut components = canonical_session_components();
        components
            .current_session
            .as_mut()
            .expect("canonical session")
            .sender_chain = None;

        SessionRecord::from_components(components)
            .expect("intermediate state without sender chain");
    }

    #[test]
    fn session_components_use_the_canonical_record_codec() {
        let expected = session_record();
        let bytes = SessionRecord::from_components(expected.clone())
            .expect("valid components")
            .serialize()
            .expect("serialize record");
        let actual = SessionRecord::deserialize(&bytes)
            .expect("deserialize record")
            .into_components()
            .expect("project record");

        assert_eq!(actual, expected);
    }

    #[test]
    fn session_components_bound_archived_state_count() {
        let mut components = session_record();
        let archived = components
            .current_session
            .clone()
            .expect("test session is present");
        components.previous_sessions =
            vec![archived; crate::protocol::consts::ARCHIVED_STATES_MAX_LENGTH + 1];

        let actual = SessionRecord::from_components(components)
            .expect("valid components")
            .into_components()
            .expect("project record");

        assert_eq!(
            actual.previous_sessions.len(),
            crate::protocol::consts::ARCHIVED_STATES_MAX_LENGTH
        );
    }

    #[test]
    fn session_handoff_burns_the_reserved_sender_range_in_all_states() {
        let mut record = SessionRecord::from_components(session_record()).expect("valid record");
        record.reserve_sender_chain_counters(0);
        let ceiling = record.reserved_sender_chain_index();
        let components = record.into_components().expect("safe handoff");
        let index = components
            .current_session
            .as_ref()
            .and_then(|session| session.sender_chain.as_ref())
            .and_then(|chain| chain.chain_key.as_ref())
            .and_then(|chain| chain.index);
        let archived_indexes: Vec<_> = components
            .previous_sessions
            .iter()
            .map(|session| {
                session
                    .sender_chain
                    .as_ref()
                    .and_then(|chain| chain.chain_key.as_ref())
                    .and_then(|chain| chain.index)
            })
            .collect();

        assert_eq!(index, Some(ceiling));
        assert_eq!(archived_indexes, vec![Some(ceiling)]);
    }

    #[test]
    fn sender_key_components_use_the_canonical_record_codec() {
        let expected = sender_key_record();
        let bytes = SenderKeyRecord::from_components(expected.clone())
            .expect("valid components")
            .serialize()
            .expect("serialize record");
        let actual = SenderKeyRecord::deserialize(&bytes)
            .expect("deserialize record")
            .into_components()
            .expect("project record");

        assert_eq!(actual, expected);
    }

    #[test]
    fn sender_key_components_bound_state_history() {
        let mut components = sender_key_record();
        let state = components.states[0].clone();
        components.states = vec![state; crate::protocol::consts::MAX_SENDER_KEY_STATES + 1];

        let actual = SenderKeyRecord::from_components(components)
            .expect("valid components")
            .into_components()
            .expect("project record");

        assert_eq!(
            actual.states.len(),
            crate::protocol::consts::MAX_SENDER_KEY_STATES
        );
    }

    #[test]
    fn sender_key_handoff_burns_the_reserved_iteration_range() {
        let mut record =
            SenderKeyRecord::from_components(sender_key_record()).expect("valid record");
        record.reserve_iterations(0);
        let ceiling = record.reserved_iteration();
        let components = record.into_components().expect("safe handoff");

        assert_eq!(components.states[0].chain_key.iteration, ceiling);
    }
}
