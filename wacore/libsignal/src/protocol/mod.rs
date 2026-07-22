//
// Copyright 2020-2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

//! Rust implementation of the **[Signal Protocol]** for asynchronous
//! forward-secret public-key cryptography.
//!
//! In particular, this library implements operations conforming to the following specifications:
//! - the **[X3DH]** key agreement protocol,
//! - the **[Double Ratchet]** *(Axolotl)* messaging protocol,
//!
//! [Signal Protocol]: https://signal.org/
//! [X3DH]: https://signal.org/docs/specifications/x3dh/
//! [Double Ratchet]: https://signal.org/docs/specifications/doubleratchet/

#![warn(clippy::unwrap_used)]
#![deny(unsafe_code)]

pub mod consts;
mod crypto;
pub mod error;
mod group_cipher;
mod identity_key;
mod legacy_session;
mod local_field;
#[allow(clippy::module_inception)]
mod protocol;
mod ratchet;
mod record_components;
mod sender_keys;
pub mod session;
mod session_cipher;
mod state;
mod storage;
mod stores;
mod timestamp;
pub use crate::core::curve::{CurveError, KeyPair, PreparedVerifyingKey, PrivateKey, PublicKey};
pub use crate::core::{
    Aci, DeviceId, Pni, ProtocolAddress, ServiceId, ServiceIdFixedWidthBinaryBytes, ServiceIdKind,
};
pub use crate::protocol::protocol::SENDERKEY_MESSAGE_CURRENT_VERSION;
pub use crate::protocol::sender_keys::InvalidSenderKeySessionError;
pub use crate::store::sender_key_name::SenderKeyName;
use error::Result;
pub use error::SignalProtocolError;
pub use group_cipher::{
    create_sender_key_distribution_message, group_decrypt, group_encrypt,
    process_sender_key_distribution_message,
};
pub use identity_key::{IdentityKey, IdentityKeyPair};
pub use legacy_session::{
    LegacyIndexedSessionV1, LegacySessionBaseKeyRoleV1, LegacySessionChainCounterV1,
    LegacySessionChainKeyV1, LegacySessionChainRoleV1, LegacySessionChainV1,
    LegacySessionDispositionV1, LegacySessionFieldV1, LegacySessionIndexV1,
    LegacySessionInteropError, LegacySessionKeyPairV1, LegacySessionLocalContext,
    LegacySessionMessageKeyV1, LegacySessionPendingPreKeyV1, LegacySessionRatchetV1,
    LegacySessionRecordV1, LegacySessionUnrepresentableFieldV1, LegacySessionV1,
};
pub use protocol::{
    CiphertextMessage, CiphertextMessageType, DecryptionErrorMessage, PlaintextContent,
    PreKeySignalMessage, SenderKeyDistributionMessage, SenderKeyMessage, SignalMessage,
};
pub use ratchet::{
    AliceSignalProtocolParameters, BobSignalProtocolParameters, ChainKey, MessageKeyGenerator,
    RootKey, UsePQRatchet, derive_keys, initialize_alice_session_record, initialize_bob_session,
    initialize_bob_session_record,
};
pub use record_components::{
    PendingKeyExchangeComponents, PendingPreKeyComponents, SenderChainKeyComponents,
    SenderKeyRecordComponents, SenderKeyStateComponents, SenderMessageKeyComponents,
    SenderSigningKeyComponents, SessionChainComponents, SessionChainKeyComponents,
    SessionComponents, SessionMessageKeyComponents, SessionMessageKeyMaterial,
    SessionRecordComponents,
};
pub use sender_keys::{SenderKeyRecord, SenderKeyState};
pub use session::{process_prekey, process_prekey_bundle};
pub use session_cipher::{
    DecryptionResult, OwnedCiphertextMessage, message_decrypt, message_decrypt_owned,
    message_decrypt_prekey, message_decrypt_signal, message_encrypt,
};
pub use state::{
    GenericSignedPreKey, PreKeyBundle, PreKeyBundleContent, PreKeyId, PreKeyRecord, SessionRecord,
    SessionState, SignedPreKeyId, SignedPreKeyRecord,
};
pub use storage::{
    Direction, IdentityChange, IdentityKeyStore, PreKeyStore, ProtocolStore, SenderKeyStore,
    SessionCheckout, SessionCheckoutKey, SessionCheckoutStoreResult, SessionStore,
    SignedPreKeyStore,
};
pub use timestamp::Timestamp;
