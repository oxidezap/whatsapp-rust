// SPDX-License-Identifier: MIT
use super::error::Result;
use super::*;
use bytes::Bytes;
use wacore_signal_engine::{self as engine, CryptoProvider};
pub const SENDERKEY_MESSAGE_CURRENT_VERSION: u8 = 3;
pub const CIPHERTEXT_MESSAGE_CURRENT_VERSION: u8 = 3;
fn validate_version(bytes: &[u8], group: bool, trailer: usize) -> Result<()> {
    let first = *bytes
        .first()
        .ok_or(SignalProtocolError::CiphertextMessageTooShort(bytes.len()))?;
    let version = first >> 4;
    if version < 3 {
        return Err(SignalProtocolError::LegacyCiphertextVersion(version));
    }
    if version > 3 {
        return Err(if group {
            SignalProtocolError::UnrecognizedMessageVersion(version.into())
        } else {
            SignalProtocolError::UnrecognizedCiphertextVersion(version)
        });
    }
    if bytes.len() <= trailer {
        return Err(SignalProtocolError::CiphertextMessageTooShort(bytes.len()));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CiphertextMessageType {
    Whisper,
    PreKey,
    SenderKey,
    Plaintext,
}
#[derive(Clone, Debug)]
pub struct SignalMessage {
    pub(crate) bytes: Bytes,
    ratchet: PublicKey,
    counter: u32,
    previous_counter: u32,
    ciphertext: std::ops::Range<usize>,
}
fn range_of(input: &[u8], part: &[u8]) -> std::ops::Range<usize> {
    let start = part.as_ptr() as usize - input.as_ptr() as usize;
    start..start + part.len()
}
impl SignalMessage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: u8,
        mac_key: &[u8],
        ratchet: PublicKey,
        counter: u32,
        previous_counter: u32,
        ciphertext: &[u8],
        sender: &IdentityKey,
        receiver: &IdentityKey,
    ) -> Result<Self> {
        if version != 3 {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(version));
        }
        let parsed = engine::wire::SignalMessage {
            ratchet: *ratchet.public_key_bytes(),
            counter,
            previous_counter,
            ciphertext: ciphertext.to_vec(),
            mac: [0; 8],
        };
        let mut unsigned = parsed.unsigned();
        let mut authenticated = Vec::with_capacity(66 + unsigned.len());
        authenticated.extend_from_slice(&sender.serialize());
        authenticated.extend_from_slice(&receiver.serialize());
        authenticated.extend_from_slice(&unsigned);
        unsigned.extend_from_slice(&engine::RustCrypto.hmac_sha256(mac_key, &authenticated)[..8]);
        Self::try_from(Bytes::from(unsigned))
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        self.bytes.to_vec().into_boxed_slice()
    }
    pub fn counter(&self) -> u32 {
        self.counter
    }
    pub fn previous_counter(&self) -> u32 {
        self.previous_counter
    }
    pub fn message_version(&self) -> u8 {
        self.bytes[0] >> 4
    }
    pub fn sender_ratchet_key(&self) -> &PublicKey {
        &self.ratchet
    }
    pub fn body(&self) -> &[u8] {
        &self.bytes[self.ciphertext.clone()]
    }
}
impl TryFrom<Bytes> for SignalMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: Bytes) -> Result<Self> {
        validate_version(&bytes, false, 8)?;
        let parsed = engine::wire::SignalMessageView::decode(&bytes)
            .map_err(|e| error::message_error(e, CiphertextMessageType::Whisper, 0, 0))?;
        let ratchet = public(&parsed.ratchet)?;
        let counter = parsed.counter;
        let previous_counter = parsed.previous_counter;
        let ciphertext = range_of(&bytes, parsed.ciphertext);
        Ok(Self {
            bytes,
            ratchet,
            counter,
            previous_counter,
            ciphertext,
        })
    }
}
impl TryFrom<&[u8]> for SignalMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::try_from(Bytes::copy_from_slice(bytes))
    }
}
#[derive(Clone, Debug)]
pub(crate) struct PreKeyHeaders {
    pub registration: u32,
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: u32,
    pub base_key: [u8; 32],
    pub identity: [u8; 32],
}
#[derive(Clone, Debug)]
pub struct PreKeySignalMessage {
    pub(crate) bytes: Bytes,
    pub(crate) parsed: PreKeyHeaders,
    identity: IdentityKey,
    base: PublicKey,
    inner: SignalMessage,
}
impl PreKeySignalMessage {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: u8,
        registration: u32,
        pre_key_id: Option<PreKeyId>,
        signed: SignedPreKeyId,
        base: PublicKey,
        identity: IdentityKey,
        inner: SignalMessage,
    ) -> Result<Self> {
        if version != 3 {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(version));
        }
        let parsed = engine::wire::PreKeySignalMessage {
            registration,
            pre_key_id: pre_key_id.map(Into::into),
            signed_pre_key_id: signed.into(),
            base_key: *base.public_key_bytes(),
            identity: *identity.public_key().public_key_bytes(),
            message: inner.serialized().to_vec(),
        };
        Self::try_from(Bytes::from(parsed.encode()))
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        self.bytes.to_vec().into_boxed_slice()
    }
    pub fn identity_key(&self) -> &IdentityKey {
        &self.identity
    }
    pub fn pre_key_id(&self) -> Option<PreKeyId> {
        self.parsed.pre_key_id.map(Into::into)
    }
    pub fn signed_pre_key_id(&self) -> SignedPreKeyId {
        self.parsed.signed_pre_key_id.into()
    }
    pub fn registration_id(&self) -> u32 {
        self.parsed.registration
    }
    pub fn base_key(&self) -> &PublicKey {
        &self.base
    }
    pub fn message_version(&self) -> u8 {
        self.bytes[0] >> 4
    }
    pub fn message(&self) -> &SignalMessage {
        &self.inner
    }
}
impl TryFrom<Bytes> for PreKeySignalMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: Bytes) -> Result<Self> {
        validate_version(&bytes, false, 0)?;
        let view = engine::wire::PreKeySignalMessageView::decode(&bytes)
            .map_err(|e| error::message_error(e, CiphertextMessageType::PreKey, 0, 0))?;
        let parsed = PreKeyHeaders {
            registration: view.registration,
            pre_key_id: view.pre_key_id,
            signed_pre_key_id: view.signed_pre_key_id,
            base_key: view.base_key,
            identity: view.identity,
        };
        let identity = IdentityKey::new(public(&view.identity)?);
        let base = public(&view.base_key)?;
        let range = range_of(&bytes, view.message);
        let inner = SignalMessage::try_from(bytes.slice(range))?;
        Ok(Self {
            bytes,
            parsed,
            identity,
            base,
            inner,
        })
    }
}
impl TryFrom<&[u8]> for PreKeySignalMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::try_from(Bytes::copy_from_slice(bytes))
    }
}
#[derive(Clone, Debug)]
pub struct SenderKeyDistributionMessage {
    pub(crate) bytes: Bytes,
    pub(crate) parsed: engine::wire::SenderKeyDistribution,
    signing_public: PublicKey,
}
impl SenderKeyDistributionMessage {
    pub fn new(
        version: u8,
        id: u32,
        iteration: u32,
        chain_key: [u8; 32],
        signing_key: PublicKey,
    ) -> Result<Self> {
        if version != 3 {
            return Err(SignalProtocolError::UnrecognizedMessageVersion(
                version.into(),
            ));
        }
        let parsed = engine::wire::SenderKeyDistribution {
            id,
            iteration,
            chain_key,
            signing_key: *signing_key.public_key_bytes(),
        };
        Ok(Self {
            bytes: parsed.encode().into(),
            parsed,
            signing_public: signing_key,
        })
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        self.bytes.to_vec().into_boxed_slice()
    }
    pub fn id(&self) -> u32 {
        self.parsed.id
    }
    pub fn iteration(&self) -> u32 {
        self.parsed.iteration
    }
    pub fn chain_key(&self) -> &[u8; 32] {
        &self.parsed.chain_key
    }
    pub fn signing_key(&self) -> &PublicKey {
        &self.signing_public
    }
    pub fn chain_id(&self) -> u32 {
        self.parsed.id
    }
}
impl TryFrom<Bytes> for SenderKeyDistributionMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: Bytes) -> Result<Self> {
        validate_version(&bytes, true, 0)?;
        let parsed = engine::wire::SenderKeyDistribution::decode(&bytes)?;
        let signing_public = public(&parsed.signing_key)?;
        Ok(Self {
            bytes,
            parsed,
            signing_public,
        })
    }
}
impl TryFrom<&[u8]> for SenderKeyDistributionMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::try_from(Bytes::copy_from_slice(bytes))
    }
}
#[derive(Clone, Debug)]
pub struct SenderKeyMessage {
    pub(crate) bytes: Bytes,
    id: u32,
    iteration: u32,
    ciphertext: std::ops::Range<usize>,
}
impl SenderKeyMessage {
    pub fn new<R: rand::Rng + rand::CryptoRng>(
        version: u8,
        id: u32,
        iteration: u32,
        ciphertext: Box<[u8]>,
        rng: &mut R,
        private: &PrivateKey,
    ) -> Result<Self> {
        if version != 3 {
            return Err(SignalProtocolError::UnrecognizedMessageVersion(
                version.into(),
            ));
        }
        let mut parsed = engine::wire::SenderKeyMessage {
            id,
            iteration,
            ciphertext: ciphertext.into_vec(),
            signature: [0; 64],
        };
        let mut random = RandomAdapter(rng);
        parsed.signature =
            engine::RustCrypto.sign(private.serialize(), &parsed.unsigned(), &mut random)?;
        Self::try_from(Bytes::from(parsed.encode()))
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        self.bytes.to_vec().into_boxed_slice()
    }
    pub fn chain_id(&self) -> u32 {
        self.id
    }
    pub fn key_id(&self) -> u32 {
        self.id
    }
    pub fn iteration(&self) -> u32 {
        self.iteration
    }
    pub fn ciphertext(&self) -> &[u8] {
        &self.bytes[self.ciphertext.clone()]
    }
}
impl TryFrom<Bytes> for SenderKeyMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: Bytes) -> Result<Self> {
        validate_version(&bytes, true, 64)?;
        let parsed = engine::wire::SenderKeyMessageView::decode(&bytes)
            .map_err(|e| error::message_error(e, CiphertextMessageType::SenderKey, 0, 0))?;
        let id = parsed.id;
        let iteration = parsed.iteration;
        let ciphertext = range_of(&bytes, parsed.ciphertext);
        Ok(Self {
            bytes,
            id,
            iteration,
            ciphertext,
        })
    }
}
impl TryFrom<&[u8]> for SenderKeyMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::try_from(Bytes::copy_from_slice(bytes))
    }
}
#[derive(Clone, Debug)]
pub enum CiphertextMessage {
    SignalMessage(SignalMessage),
    PreKeySignalMessage(PreKeySignalMessage),
    SenderKeyMessage(SenderKeyMessage),
}
impl CiphertextMessage {
    pub fn serialize(&self) -> &[u8] {
        self.serialized()
    }
    pub fn message_type(&self) -> CiphertextMessageType {
        match self {
            Self::SignalMessage(_) => CiphertextMessageType::Whisper,
            Self::PreKeySignalMessage(_) => CiphertextMessageType::PreKey,
            Self::SenderKeyMessage(_) => CiphertextMessageType::SenderKey,
        }
    }
    pub fn serialized(&self) -> &[u8] {
        match self {
            Self::SignalMessage(m) => m.serialized(),
            Self::PreKeySignalMessage(m) => m.serialized(),
            Self::SenderKeyMessage(m) => m.serialized(),
        }
    }
}
pub struct OwnedCiphertextMessage {
    pub(crate) message: Option<CiphertextMessage>,
}
impl From<CiphertextMessage> for OwnedCiphertextMessage {
    fn from(message: CiphertextMessage) -> Self {
        Self {
            message: Some(message),
        }
    }
}
impl OwnedCiphertextMessage {
    pub fn is_available(&self) -> bool {
        self.message.is_some()
    }
}

#[cfg(test)]
mod borrowed_tests {
    use super::*;
    fn signal_bytes() -> Vec<u8> {
        let signal = engine::wire::SignalMessage {
            ratchet: [9; 32],
            counter: 7,
            previous_counter: 2,
            ciphertext: vec![3; 128],
            mac: [4; 8],
        };
        let mut encoded = signal.unsigned();
        encoded.extend_from_slice(&[0x78, 0x07]);
        encoded.extend_from_slice(&signal.mac);
        encoded
    }
    #[test]
    fn signal_and_sender_parse_share_input_storage() {
        let input = Bytes::from(signal_bytes());
        let pointer = input.as_ptr();
        let message = SignalMessage::try_from(input).unwrap();
        assert_eq!(message.serialized().as_ptr(), pointer);
        assert!(message.body().as_ptr() as usize >= pointer as usize);
        assert!(
            (message.body().as_ptr() as usize + message.body().len())
                <= pointer as usize + message.serialized().len()
        );
        let sender = engine::wire::SenderKeyMessage {
            id: 3,
            iteration: 4,
            ciphertext: vec![6; 128],
            signature: [7; 64],
        };
        let mut encoded = sender.unsigned();
        encoded.extend_from_slice(&[0x78, 0x07]);
        encoded.extend_from_slice(&sender.signature);
        let input = Bytes::from(encoded);
        let pointer = input.as_ptr();
        let message = SenderKeyMessage::try_from(input).unwrap();
        assert_eq!(message.serialized().as_ptr(), pointer);
        assert!(message.ciphertext().as_ptr() as usize >= pointer as usize);
        assert!(
            (message.ciphertext().as_ptr() as usize + message.ciphertext().len())
                <= pointer as usize + message.serialized().len()
        );
    }
    #[test]
    fn prekey_inner_retains_unknown_fields_and_shares_outer_storage() {
        let outer = engine::wire::PreKeySignalMessage {
            registration: 5,
            pre_key_id: Some(6),
            signed_pre_key_id: 7,
            base_key: [8; 32],
            identity: [9; 32],
            message: signal_bytes(),
        };
        let mut encoded = outer.encode();
        encoded.extend_from_slice(&[0x78, 0x07]);
        let input = Bytes::from(encoded);
        let pointer = input.as_ptr();
        let message = PreKeySignalMessage::try_from(input).unwrap();
        assert_eq!(message.serialized().as_ptr(), pointer);
        assert_eq!(message.message().serialized(), outer.message);
        let inner_pointer = message.message().serialized().as_ptr() as usize;
        assert!(inner_pointer >= pointer as usize);
        assert!(
            inner_pointer + outer.message.len() <= pointer as usize + message.serialized().len()
        );
        assert_eq!(message.base_key().public_key_bytes(), &outer.base_key);
        let copy = message.clone();
        assert_eq!(
            copy.message().serialized().as_ptr(),
            message.message().serialized().as_ptr()
        );
    }
}
