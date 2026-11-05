// SPDX-License-Identifier: MIT
use super::error::Result;
use super::*;
use crate::engine::{self as engine, CryptoProvider};
use bytes::Bytes;
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
    if version > if group { 3 } else { 4 } {
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
/// Owned outbound bytes, without shared-buffer bookkeeping or parsed message views.
/// The host must satisfy the same durable reservation gate as for typed messages.
pub struct SerializedCiphertext {
    kind: CiphertextMessageType,
    bytes: Vec<u8>,
}
impl SerializedCiphertext {
    pub fn message_type(&self) -> CiphertextMessageType {
        self.kind
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_serialized(self) -> Vec<u8> {
        self.bytes
    }
    pub(crate) fn from_sender_key(bytes: Vec<u8>) -> Result<Self> {
        validate_version(&bytes, true, 64)?;
        engine::wire::SenderKeyMessageView::decode(&bytes)
            .map_err(|e| error::message_error(e, CiphertextMessageType::SenderKey, 0, 0))?;
        Ok(Self {
            kind: CiphertextMessageType::SenderKey,
            bytes,
        })
    }
    pub(crate) fn from_pairwise(bytes: Vec<u8>, prekey: bool) -> Result<Self> {
        // Validate before the adapter commits, exactly as the typed output path
        // does, while keeping ownership of the original allocation.
        let inner = if prekey {
            validate_version(&bytes, false, 0)?;
            let view = engine::wire::PreKeySignalMessageView::decode(&bytes)
                .map_err(|e| error::message_error(e, CiphertextMessageType::PreKey, 0, 0))?;
            public(&view.identity)?;
            public(&view.base_key)?;
            view.message
        } else {
            bytes.as_slice()
        };
        validate_version(inner, false, 8)?;
        let view = engine::wire::SignalMessageView::decode(inner)
            .map_err(|e| error::message_error(e, CiphertextMessageType::Whisper, 0, 0))?;
        public(&view.ratchet)?;
        Ok(Self {
            kind: if prekey {
                CiphertextMessageType::PreKey
            } else {
                CiphertextMessageType::Whisper
            },
            bytes,
        })
    }
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
        if !matches!(version, 3 | 4) {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(version));
        }
        let parsed = engine::wire::SignalMessage {
            ratchet: *ratchet.public_key_bytes(),
            counter,
            previous_counter,
            ciphertext: ciphertext.to_vec(),
            mac: [0; 8],
        };
        let mut unsigned = parsed.unsigned_with_version(version);
        let mac = engine::RustCrypto.hmac_sha256_parts_array(
            mac_key,
            &[&sender.serialize(), &receiver.serialize(), &unsigned],
        );
        unsigned.extend_from_slice(&mac[..8]);
        Self::try_from(Bytes::from(unsigned))
    }
    fn decode<T: AsRef<[u8]>>(input: T, take_bytes: impl FnOnce(T) -> Bytes) -> Result<Self> {
        let bytes = input.as_ref();
        validate_version(bytes, false, 8)?;
        let parsed = engine::wire::SignalMessageView::decode(bytes)
            .map_err(|e| error::message_error(e, CiphertextMessageType::Whisper, 0, 0))?;
        let ratchet = public(&parsed.ratchet)?;
        let counter = parsed.counter;
        let previous_counter = parsed.previous_counter;
        let ciphertext = range_of(bytes, parsed.ciphertext);
        Ok(Self {
            bytes: take_bytes(input),
            ratchet,
            counter,
            previous_counter,
            ciphertext,
        })
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        Vec::from(self.bytes).into_boxed_slice()
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
        Self::decode(bytes, std::convert::identity)
    }
}
impl TryFrom<&[u8]> for SignalMessage {
    type Error = SignalProtocolError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes, Bytes::copy_from_slice)
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
        if !matches!(version, 3 | 4) {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(version));
        }
        if version != inner.message_version() {
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
        Self::try_from(Bytes::from(parsed.encode_with_version(version)))
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        let Self { bytes, inner, .. } = self;
        // Release the nested view before recovering the outer allocation.
        drop(inner);
        Vec::from(bytes).into_boxed_slice()
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
        if (bytes[0] >> 4) != inner.message_version() {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(
                bytes[0] >> 4,
            ));
        }
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
#[derive(Clone)]
pub struct SenderKeyDistributionMessage {
    pub(crate) bytes: Bytes,
    pub(crate) parsed: engine::wire::SenderKeyDistribution,
    signing_public: PublicKey,
}
impl core::fmt::Debug for SenderKeyDistributionMessage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SenderKeyDistributionMessage")
            .field("id", &self.parsed.id)
            .field("iteration", &self.parsed.iteration)
            .finish_non_exhaustive()
    }
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
        Vec::from(self.bytes).into_boxed_slice()
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
        let parsed = engine::wire::SenderKeyMessage {
            id,
            iteration,
            ciphertext: ciphertext.into_vec(),
            signature: [0; 64],
        };
        let mut random = RandomAdapter(rng);
        let mut encoded = parsed.unsigned();
        let signature = engine::RustCrypto.sign(private.serialize(), &encoded, &mut random)?;
        encoded.extend_from_slice(&signature);
        Self::try_from(Bytes::from(encoded))
    }
    pub fn serialized(&self) -> &[u8] {
        &self.bytes
    }
    pub fn serialize(&self) -> Vec<u8> {
        self.bytes.to_vec()
    }
    pub fn into_serialized(self) -> Box<[u8]> {
        Vec::from(self.bytes).into_boxed_slice()
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
    #[test]
    fn owned_group_output_matches_typed_validation_for_mutated_envelopes() {
        use rand::SeedableRng as _;
        let mut random = rand::rngs::StdRng::seed_from_u64(1842);
        let key = KeyPair::generate(&mut random);
        for counter in [0, 127, 128, 16384, u32::MAX] {
            let encoded = SenderKeyMessage::new(
                3,
                17,
                counter,
                vec![67; 16].into_boxed_slice(),
                &mut random,
                &key.private_key,
            )
            .unwrap()
            .serialize();
            let mut variants = vec![encoded.clone()];
            variants.extend((0..encoded.len()).map(|end| encoded[..end].to_vec()));
            variants.extend((0..encoded.len()).map(|index| {
                let mut changed = encoded.clone();
                changed[index] ^= 0x80;
                changed
            }));
            for bytes in variants {
                let typed = SenderKeyMessage::try_from(bytes.as_slice()).map(|_| ());
                let owned = SerializedCiphertext::from_sender_key(bytes).map(|_| ());
                assert_eq!(
                    owned.map_err(|e| e.to_string()),
                    typed.map_err(|e| e.to_string())
                );
            }
        }
    }
    #[test]
    fn serialized_output_validation_matches_typed_decoders_for_mutated_envelopes() {
        let key = PublicKey::from_djb_public_key_bytes(&[9; 32]).unwrap();
        let identity = IdentityKey::new(key);
        for version in [3, 4] {
            let signal =
                SignalMessage::new(version, &[7; 32], key, 1, 0, &[2; 16], &identity, &identity)
                    .unwrap();
            let prekey = PreKeySignalMessage::new(
                version,
                5,
                Some(4.into()),
                7.into(),
                key,
                identity,
                signal.clone(),
            )
            .unwrap();
            for (encoded, is_prekey) in [(signal.serialize(), false), (prekey.serialize(), true)] {
                let mut variants = vec![encoded.clone()];
                variants.extend((0..encoded.len()).map(|end| encoded[..end].to_vec()));
                variants.extend((0..encoded.len()).map(|index| {
                    let mut changed = encoded.clone();
                    changed[index] ^= 0x80;
                    changed
                }));
                for bytes in variants {
                    let typed = if is_prekey {
                        PreKeySignalMessage::try_from(bytes.as_slice()).map(|_| ())
                    } else {
                        SignalMessage::try_from(bytes.as_slice()).map(|_| ())
                    };
                    let serialized =
                        SerializedCiphertext::from_pairwise(bytes, is_prekey).map(|_| ());
                    assert_eq!(
                        serialized.map_err(|e| e.to_string()),
                        typed.map_err(|e| e.to_string())
                    );
                }
            }
        }
    }
    #[test]
    fn sender_distribution_debug_hides_encoded_and_parsed_chain_secret() {
        let distribution = SenderKeyDistributionMessage::new(
            3,
            11,
            23,
            [167; 32],
            PublicKey::from_djb_public_key_bytes(&[9; 32]).unwrap(),
        )
        .unwrap();
        for debug in [
            format!("{distribution:?}"),
            format!("{:#?}", Some(&distribution)),
        ] {
            assert!(debug.contains("SenderKeyDistributionMessage"));
            assert!(debug.contains("11"));
            assert!(debug.contains("23"));
            assert!(!debug.contains("167"));
            assert!(!debug.contains("\\xa7"));
        }
    }

    #[test]
    fn pairwise_constructors_support_classic_version_four() {
        let public = PublicKey::from_djb_public_key_bytes(&[9; 32]).unwrap();
        let identity = IdentityKey::new(public);
        for version in [3, 4] {
            let signal = SignalMessage::new(
                version, &[7; 32], public, 1, 0, &[2; 16], &identity, &identity,
            )
            .unwrap();
            assert_eq!(signal.message_version(), version);
            assert_eq!(signal.serialized()[0], (version << 4) | 3);
            let mut authenticated = Vec::new();
            authenticated.extend_from_slice(&identity.serialize());
            authenticated.extend_from_slice(&identity.serialize());
            authenticated.extend_from_slice(&signal.serialized()[..signal.serialized().len() - 8]);
            assert_eq!(
                &signal.serialized()[signal.serialized().len() - 8..],
                &engine::RustCrypto.hmac_sha256_array(&[7; 32], &authenticated)[..8]
            );
            let prekey =
                PreKeySignalMessage::new(version, 5, None, 7.into(), public, identity, signal)
                    .unwrap();
            assert_eq!(prekey.message_version(), version);
            assert_eq!(prekey.message().message_version(), version);
        }
        assert!(
            SignalMessage::new(5, &[7; 32], public, 1, 0, &[2; 16], &identity, &identity).is_err()
        );
        assert!(SenderKeyDistributionMessage::new(4, 1, 0, [7; 32], public).is_err());
        let group = engine::wire::SenderKeyMessage {
            id: 3,
            iteration: 4,
            ciphertext: vec![6; 16],
            signature: [7; 64],
        };
        let mut encoded = group.encode();
        encoded[0] = 0x43;
        assert!(SenderKeyMessage::try_from(encoded.as_slice()).is_err());
    }

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
    fn borrowed_signal_input_preserves_owned_validation_and_offsets() {
        let encoded = signal_bytes();
        let mut variants = vec![encoded.clone()];
        variants.extend((0..encoded.len()).map(|end| encoded[..end].to_vec()));
        variants.extend((0..encoded.len()).map(|index| {
            let mut changed = encoded.clone();
            changed[index] ^= 0xff;
            changed
        }));
        for version in [0x23, 0x33, 0x43, 0x53] {
            let mut changed = encoded.clone();
            changed[0] = version;
            variants.push(changed);
        }
        for input in variants {
            let borrowed = SignalMessage::try_from(input.as_slice());
            let mut backing = vec![0xa5; 17];
            backing.extend_from_slice(&input);
            backing.push(0x5a);
            let owned = SignalMessage::try_from(Bytes::from(backing).slice(17..17 + input.len()));
            match (borrowed, owned) {
                (Ok(left), Ok(right)) => {
                    assert_eq!(left.serialized(), input);
                    assert_eq!(left.serialized(), right.serialized());
                    assert_eq!(left.sender_ratchet_key(), right.sender_ratchet_key());
                    assert_eq!(left.counter(), right.counter());
                    assert_eq!(left.previous_counter(), right.previous_counter());
                    assert_eq!(left.message_version(), right.message_version());
                    assert_eq!(left.body(), right.body());
                }
                (Err(left), Err(right)) => assert_eq!(format!("{left:?}"), format!("{right:?}")),
                _ => panic!("borrowed and owned input disagree"),
            }
        }
    }
    #[test]
    fn rejected_signal_input_never_requests_owned_storage() {
        let prekey = engine::wire::PreKeySignalMessage {
            registration: 1,
            pre_key_id: Some(2),
            signed_pre_key_id: 3,
            base_key: [4; 32],
            identity: [5; 32],
            message: signal_bytes(),
        }
        .encode();
        for input in [&[][..], &[0x33][..], prekey.as_slice()] {
            assert!(
                SignalMessage::decode(input, |_| panic!("rejected input requested ownership"))
                    .is_err()
            );
        }
    }
    #[test]
    fn consumed_messages_reuse_unique_storage_and_isolate_shared_views() {
        let encoded = signal_bytes();
        let input = encoded.clone().into_boxed_slice();
        let pointer = input.as_ptr();
        let message = SignalMessage::try_from(Bytes::from(input)).unwrap();
        let consumed = message.into_serialized();
        assert_eq!(&*consumed, encoded);
        assert_eq!(consumed.as_ptr(), pointer);

        let mut backing = vec![0xa5; 17];
        backing.extend_from_slice(&encoded);
        backing.push(0x5a);
        let backing = Bytes::from(backing);
        let message = SignalMessage::try_from(backing.slice(17..17 + encoded.len())).unwrap();
        let snapshot = message.clone();
        let mut consumed = message.into_serialized();
        assert_eq!(&*consumed, encoded);
        consumed.fill(0);
        assert_eq!(snapshot.serialized(), encoded);
        assert_eq!(&backing[17..17 + encoded.len()], encoded);

        let outer = engine::wire::PreKeySignalMessage {
            registration: 5,
            pre_key_id: Some(6),
            signed_pre_key_id: 7,
            base_key: [8; 32],
            identity: [9; 32],
            message: encoded.clone(),
        }
        .encode();
        let input = outer.clone().into_boxed_slice();
        let pointer = input.as_ptr();
        let message = PreKeySignalMessage::try_from(Bytes::from(input)).unwrap();
        let consumed = message.into_serialized();
        assert_eq!(&*consumed, outer);
        assert_eq!(consumed.as_ptr(), pointer);

        let message = PreKeySignalMessage::try_from(Bytes::from(outer.clone())).unwrap();
        let nested = message.message().clone();
        let mut consumed = message.into_serialized();
        assert_eq!(&*consumed, outer);
        consumed.fill(0);
        assert_eq!(nested.serialized(), encoded);
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
    fn prekey_constructor_rejects_mixed_inner_and_outer_versions() {
        for outer in [3, 4] {
            for inner_version in [3, 4] {
                let mut encoded = signal_bytes();
                encoded[0] = (inner_version << 4) | 3;
                let inner = SignalMessage::try_from(encoded.as_slice()).unwrap();
                let result = PreKeySignalMessage::new(
                    outer,
                    1,
                    Some(2.into()),
                    3.into(),
                    PublicKey::from_djb_public_key_bytes(&[4; 32]).unwrap(),
                    IdentityKey::new(PublicKey::from_djb_public_key_bytes(&[5; 32]).unwrap()),
                    inner,
                );
                assert_eq!(result.is_ok(), outer == inner_version);
            }
        }
    }
    #[test]
    fn prekey_parsers_reject_mixed_inner_and_outer_versions() {
        for outer in [3, 4] {
            for inner_version in [3, 4] {
                let mut inner = signal_bytes();
                inner[0] = (inner_version << 4) | 3;
                let mut encoded = engine::wire::PreKeySignalMessage {
                    registration: 5,
                    pre_key_id: Some(6),
                    signed_pre_key_id: 7,
                    base_key: [8; 32],
                    identity: [9; 32],
                    message: inner,
                }
                .encode();
                encoded[0] = (outer << 4) | 3;
                for result in [
                    PreKeySignalMessage::try_from(encoded.as_slice()),
                    PreKeySignalMessage::try_from(Bytes::from(encoded.clone())),
                ] {
                    if outer == inner_version {
                        let parsed = result.unwrap();
                        assert_eq!(parsed.message_version(), outer);
                        assert_eq!(parsed.message().message_version(), inner_version);
                    } else {
                        assert!(
                            matches!(result, Err(SignalProtocolError::UnrecognizedCiphertextVersion(version)) if version == outer)
                        );
                    }
                }
            }
        }
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
