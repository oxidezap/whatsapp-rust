// SPDX-License-Identifier: MIT
//! Wire field numbers follow the public protobuf data declarations.
//! Pairwise versions 3 and 4 use the classical fields here; post-quantum fields
//! remain unsupported.
use crate::engine::{Error, Result};
pub(crate) const MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
#[derive(Clone, Copy)]
pub(crate) enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed64(u64),
    Fixed32,
}
pub(crate) struct Fields<'a> {
    bytes: &'a [u8],
}
impl<'a> Fields<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() > MAX_MESSAGE_SIZE {
            return Err(Error::Encoding("protobuf size limit"));
        }
        Ok(Self { bytes })
    }
    /// Stored records aggregate archived sessions and expanded historical keys.
    /// They have a separate finite budget; network envelopes keep the 16 MiB gate.
    pub(crate) fn new_storage(bytes: &'a [u8]) -> Result<Self> {
        const STORAGE_PROTOBUF_LIMIT: usize = 128 * 1024 * 1024;
        if bytes.len() > STORAGE_PROTOBUF_LIMIT {
            return Err(Error::Encoding("stored protobuf size limit"));
        }
        Ok(Self { bytes })
    }
    fn varint(&mut self) -> Result<u64> {
        let mut value = 0;
        for i in 0..10 {
            let (&byte, rest) = self
                .bytes
                .split_first()
                .ok_or(Error::Encoding("truncated varint"))?;
            self.bytes = rest;
            if i == 9 && byte > 1 {
                return Err(Error::Encoding("varint overflow"));
            }
            value |= u64::from(byte & 127) << (7 * i);
            if byte & 128 == 0 {
                return Ok(value);
            }
        }
        Err(Error::Encoding("varint overflow"))
    }
    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        if size > self.bytes.len() {
            return Err(Error::Encoding("truncated field"));
        }
        let (value, rest) = self.bytes.split_at(size);
        self.bytes = rest;
        Ok(value)
    }
    pub fn next(&mut self) -> Result<Option<(u32, Value<'a>)>> {
        if self.bytes.is_empty() {
            return Ok(None);
        }
        let tag = self.varint()?;
        let number =
            u32::try_from(tag >> 3).map_err(|_| Error::Encoding("field number overflow"))?;
        if number == 0 || number > 0x1fff_ffff {
            return Err(Error::Encoding("invalid field number"));
        }
        let value = match tag & 7 {
            0 => Value::Varint(self.varint()?),
            1 => Value::Fixed64(u64::from_le_bytes(
                self.take(8)?
                    .try_into()
                    .map_err(|_| Error::Encoding("fixed64"))?,
            )),
            2 => {
                let size = usize::try_from(self.varint()?)
                    .map_err(|_| Error::Encoding("length overflow"))?;
                Value::Bytes(self.take(size)?)
            }
            5 => {
                self.take(4)?;
                Value::Fixed32
            }
            _ => return Err(Error::Encoding("unsupported wire type")),
        };
        Ok(Some((number, value)))
    }
}
pub(crate) fn varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn varint_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 128 {
        len += 1;
        value >>= 7;
    }
    len
}
fn uint_len(number: u32, value: u32) -> usize {
    varint_len(u64::from(number) << 3) + varint_len(u64::from(value))
}
fn bytes_len(number: u32, len: usize) -> usize {
    varint_len((u64::from(number) << 3) | 2) + varint_len(len as u64) + len
}
pub(crate) fn uint(out: &mut Vec<u8>, number: u32, value: u64) {
    varint(out, u64::from(number) << 3);
    varint(out, value);
}
pub(crate) fn bytes(out: &mut Vec<u8>, number: u32, value: &[u8]) {
    varint(out, (u64::from(number) << 3) | 2);
    varint(out, value.len() as u64);
    out.extend_from_slice(value);
}
pub(crate) fn fixed64(out: &mut Vec<u8>, number: u32, value: u64) {
    varint(out, (u64::from(number) << 3) | 1);
    out.extend_from_slice(&value.to_le_bytes());
}
pub(crate) fn as_bytes(value: Value<'_>) -> Result<&[u8]> {
    if let Value::Bytes(bytes) = value {
        Ok(bytes)
    } else {
        Err(Error::Encoding("expected bytes"))
    }
}
pub(crate) fn as_u32(value: Value<'_>) -> Result<u32> {
    if let Value::Varint(number) = value {
        u32::try_from(number).map_err(|_| Error::Encoding("u32 overflow"))
    } else {
        Err(Error::Encoding("expected varint"))
    }
}
pub(crate) fn array<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| Error::Encoding("invalid byte length"))
}
pub fn tagged_key(public: &[u8; 32]) -> [u8; 33] {
    let mut result = [5; 33];
    result[1..].copy_from_slice(public);
    result
}
pub fn decode_public(bytes: &[u8]) -> Result<[u8; 32]> {
    match bytes {
        [5, public @ ..] if public.len() == 32 => array(public),
        public if public.len() == 32 => array(public),
        _ => Err(Error::Encoding("invalid public key")),
    }
}
pub(crate) fn versioned(bytes: &[u8], trailer: usize) -> Result<(&[u8], &[u8])> {
    versioned_profile(bytes, trailer, false)
}
fn pairwise_versioned(bytes: &[u8], trailer: usize) -> Result<(&[u8], &[u8])> {
    versioned_profile(bytes, trailer, true)
}
fn versioned_profile(bytes: &[u8], trailer: usize, pairwise: bool) -> Result<(&[u8], &[u8])> {
    let (&version, rest) = bytes
        .split_first()
        .ok_or(Error::Encoding("missing version"))?;
    let supported = if pairwise {
        matches!(version >> 4, 3 | 4)
    } else {
        version >> 4 == 3
    };
    if !supported {
        return Err(Error::Version);
    }
    if rest.len() < trailer {
        return Err(Error::Encoding("missing authentication trailer"));
    }
    Ok(rest.split_at(rest.len() - trailer))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignalMessage {
    pub ratchet: [u8; 32],
    pub counter: u32,
    pub previous_counter: u32,
    pub ciphertext: Vec<u8>,
    pub mac: [u8; 8],
}
impl SignalMessage {
    pub fn unsigned(&self) -> Vec<u8> {
        self.unsigned_with_version(3)
    }
    pub fn unsigned_with_version(&self, version: u8) -> Vec<u8> {
        let mut out = Self::prefix(
            version,
            &self.ratchet,
            self.counter,
            self.previous_counter,
            self.ciphertext.len(),
        );
        out.extend_from_slice(&self.ciphertext);
        out
    }
    pub(crate) fn prefix(
        version: u8,
        ratchet: &[u8; 32],
        counter: u32,
        previous_counter: u32,
        ciphertext_len: usize,
    ) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            1 + bytes_len(1, 33)
                + uint_len(2, counter)
                + uint_len(3, previous_counter)
                + bytes_len(4, ciphertext_len)
                + 8,
        );
        out.push((version << 4) | 3);
        bytes(&mut out, 1, &tagged_key(ratchet));
        uint(&mut out, 2, counter.into());
        uint(&mut out, 3, previous_counter.into());
        varint(&mut out, (4 << 3) | 2);
        varint(&mut out, ciphertext_len as u64);
        out
    }
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_version(3)
    }
    pub fn encode_with_version(&self, version: u8) -> Vec<u8> {
        let mut out = self.unsigned_with_version(version);
        out.extend_from_slice(&self.mac);
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        Ok(SignalMessageView::decode(input)?.to_owned())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreKeySignalMessage {
    pub registration: u32,
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: u32,
    pub base_key: [u8; 32],
    pub identity: [u8; 32],
    pub message: Vec<u8>,
}
impl PreKeySignalMessage {
    pub fn encode(&self) -> Vec<u8> {
        self.encode_with_version(3)
    }
    pub fn encode_with_version(&self, version: u8) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            1 + self.pre_key_id.map_or(0, |id| uint_len(1, id))
                + bytes_len(2, 33)
                + bytes_len(3, 33)
                + bytes_len(4, self.message.len())
                + uint_len(5, self.registration)
                + uint_len(6, self.signed_pre_key_id),
        );
        out.push((version << 4) | 3);
        if let Some(id) = self.pre_key_id {
            uint(&mut out, 1, id.into());
        }
        bytes(&mut out, 2, &tagged_key(&self.base_key));
        bytes(&mut out, 3, &tagged_key(&self.identity));
        bytes(&mut out, 4, &self.message);
        uint(&mut out, 5, self.registration.into());
        uint(&mut out, 6, self.signed_pre_key_id.into());
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        Ok(PreKeySignalMessageView::decode(input)?.to_owned())
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct SenderKeyDistribution {
    pub id: u32,
    pub iteration: u32,
    pub chain_key: [u8; 32],
    pub signing_key: [u8; 32],
}
impl core::fmt::Debug for SenderKeyDistribution {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SenderKeyDistribution")
            .field("id", &self.id)
            .field("iteration", &self.iteration)
            .finish_non_exhaustive()
    }
}
impl SenderKeyDistribution {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            1 + uint_len(1, self.id)
                + uint_len(2, self.iteration)
                + bytes_len(3, self.chain_key.len())
                + bytes_len(4, 33),
        );
        out.push(0x33);
        uint(&mut out, 1, self.id.into());
        uint(&mut out, 2, self.iteration.into());
        bytes(&mut out, 3, &self.chain_key);
        bytes(&mut out, 4, &tagged_key(&self.signing_key));
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        let (body, _) = versioned(input, 0)?;
        let mut f = Fields::new(body)?;
        let (mut id, mut iteration, mut chain, mut signing) = (None, None, None, None);
        while let Some((n, v)) = f.next()? {
            match n {
                1 => id = Some(as_u32(v)?),
                2 => iteration = Some(as_u32(v)?),
                3 => chain = Some(array(as_bytes(v)?)?),
                4 => signing = Some(decode_public(as_bytes(v)?)?),
                _ => {}
            }
        }
        Ok(Self {
            id: id.ok_or(Error::MissingKey)?,
            iteration: iteration.ok_or(Error::MissingKey)?,
            chain_key: chain.ok_or(Error::MissingKey)?,
            signing_key: signing.ok_or(Error::MissingKey)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SenderKeyMessage {
    pub id: u32,
    pub iteration: u32,
    pub ciphertext: Vec<u8>,
    pub signature: [u8; 64],
}
impl SenderKeyMessage {
    pub fn unsigned(&self) -> Vec<u8> {
        let mut out = Self::prefix(self.id, self.iteration, self.ciphertext.len());
        out.extend_from_slice(&self.ciphertext);
        out
    }
    pub(crate) fn prefix(id: u32, iteration: u32, ciphertext_len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(
            1 + uint_len(1, id) + uint_len(2, iteration) + bytes_len(3, ciphertext_len) + 64,
        );
        out.push(0x33);
        uint(&mut out, 1, id.into());
        uint(&mut out, 2, iteration.into());
        varint(&mut out, (3 << 3) | 2);
        varint(&mut out, ciphertext_len as u64);
        out
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.unsigned();
        out.extend_from_slice(&self.signature);
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        Ok(SenderKeyMessageView::decode(input)?.to_owned())
    }
}

/// Validated fields borrowing the exact original wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalMessageView<'a> {
    pub bytes: &'a [u8],
    pub ratchet: [u8; 32],
    pub counter: u32,
    pub previous_counter: u32,
    pub ciphertext: &'a [u8],
    pub mac: [u8; 8],
}
impl<'a> SignalMessageView<'a> {
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        let (body, mac) = pairwise_versioned(input, 8)?;
        let mut fields = Fields::new(body)?;
        let (mut ratchet, mut counter, mut previous, mut ciphertext) = (None, None, 0, None);
        while let Some((n, v)) = fields.next()? {
            match n {
                1 => ratchet = Some(decode_public(as_bytes(v)?)?),
                2 => counter = Some(as_u32(v)?),
                3 => previous = as_u32(v)?,
                4 => ciphertext = Some(as_bytes(v)?),
                _ => {}
            }
        }
        Ok(Self {
            bytes: input,
            ratchet: ratchet.ok_or(Error::MissingKey)?,
            counter: counter.ok_or(Error::Encoding("missing counter"))?,
            previous_counter: previous,
            ciphertext: ciphertext.ok_or(Error::Encoding("missing ciphertext"))?,
            mac: array(mac)?,
        })
    }
    pub fn serialized(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn to_owned(&self) -> SignalMessage {
        SignalMessage {
            ratchet: self.ratchet,
            counter: self.counter,
            previous_counter: self.previous_counter,
            ciphertext: self.ciphertext.to_vec(),
            mac: self.mac,
        }
    }
    pub fn unsigned(&self) -> &'a [u8] {
        &self.bytes[..self.bytes.len() - 8]
    }
}

/// Validated fields borrowing the exact original wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreKeySignalMessageView<'a> {
    pub bytes: &'a [u8],
    pub registration: u32,
    pub pre_key_id: Option<u32>,
    pub signed_pre_key_id: u32,
    pub base_key: [u8; 32],
    pub identity: [u8; 32],
    pub message: &'a [u8],
}
impl<'a> PreKeySignalMessageView<'a> {
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        let (body, _) = pairwise_versioned(input, 0)?;
        let mut f = Fields::new(body)?;
        let (mut registration, mut pre_key_id, mut signed, mut base, mut identity, mut message) =
            (0, None, None, None, None, None);
        while let Some((n, v)) = f.next()? {
            match n {
                1 => pre_key_id = Some(as_u32(v)?),
                2 => base = Some(decode_public(as_bytes(v)?)?),
                3 => identity = Some(decode_public(as_bytes(v)?)?),
                4 => message = Some(as_bytes(v)?),
                5 => registration = as_u32(v)?,
                6 => signed = Some(as_u32(v)?),
                7 | 8 => return Err(Error::Version),
                _ => {}
            }
        }
        let message = message.ok_or(Error::MissingKey)?;
        SignalMessageView::decode(message)?;
        Ok(Self {
            bytes: input,
            registration,
            pre_key_id,
            signed_pre_key_id: signed.ok_or(Error::MissingKey)?,
            base_key: base.ok_or(Error::MissingKey)?,
            identity: identity.ok_or(Error::MissingKey)?,
            message,
        })
    }
    pub fn serialized(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn to_owned(&self) -> PreKeySignalMessage {
        PreKeySignalMessage {
            registration: self.registration,
            pre_key_id: self.pre_key_id,
            signed_pre_key_id: self.signed_pre_key_id,
            base_key: self.base_key,
            identity: self.identity,
            message: self.message.to_vec(),
        }
    }
}

/// Validated fields borrowing the exact original wire encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SenderKeyMessageView<'a> {
    pub bytes: &'a [u8],
    pub id: u32,
    pub iteration: u32,
    pub ciphertext: &'a [u8],
    pub signature: [u8; 64],
}
impl<'a> SenderKeyMessageView<'a> {
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        let (body, sig) = versioned(input, 64)?;
        let mut f = Fields::new(body)?;
        let (mut id, mut iteration, mut ciphertext) = (None, None, None);
        while let Some((n, v)) = f.next()? {
            match n {
                1 => id = Some(as_u32(v)?),
                2 => iteration = Some(as_u32(v)?),
                3 => ciphertext = Some(as_bytes(v)?),
                _ => {}
            }
        }
        Ok(Self {
            bytes: input,
            id: id.ok_or(Error::MissingKey)?,
            iteration: iteration.ok_or(Error::MissingKey)?,
            ciphertext: ciphertext.ok_or(Error::MissingKey)?,
            signature: array(sig)?,
        })
    }
    pub fn serialized(&self) -> &'a [u8] {
        self.bytes
    }
    pub fn to_owned(&self) -> SenderKeyMessage {
        SenderKeyMessage {
            id: self.id,
            iteration: self.iteration,
            ciphertext: self.ciphertext.to_vec(),
            signature: self.signature,
        }
    }
    pub fn unsigned(&self) -> &'a [u8] {
        &self.bytes[..self.bytes.len() - 64]
    }
}

#[cfg(test)]
mod borrowed_tests {
    use super::*;
    #[test]
    fn wire_serialization_contract_across_varint_boundaries() {
        use crate::core::{IdentityKey, PublicKey};
        use sha2::{Digest, Sha256};

        let sender = IdentityKey::new(PublicKey::from_djb_public_key_bytes(&[1; 32]).unwrap());
        let receiver = IdentityKey::new(PublicKey::from_djb_public_key_bytes(&[2; 32]).unwrap());
        let mut hashes = Vec::new();
        for version in [3, 4] {
            for (counter, size) in [
                (0, 0),
                (127, 16),
                (128, 127),
                (16383, 128),
                (16384, 129),
                (u32::MAX, 1024),
            ] {
                let signal = SignalMessage {
                    ratchet: [9; 32],
                    counter,
                    previous_counter: counter,
                    ciphertext: vec![3; size],
                    mac: [4; 8],
                };
                let authenticated = crate::protocol::SignalMessage::new(
                    version,
                    &[5; 32],
                    PublicKey::from_djb_public_key_bytes(&signal.ratchet).unwrap(),
                    counter,
                    counter,
                    &signal.ciphertext,
                    &sender,
                    &receiver,
                )
                .unwrap();
                let prekey = PreKeySignalMessage {
                    registration: counter,
                    pre_key_id: (!counter.is_multiple_of(2)).then_some(counter),
                    signed_pre_key_id: counter,
                    base_key: [8; 32],
                    identity: [9; 32],
                    message: signal.encode_with_version(version),
                };
                let group = SenderKeyMessage {
                    id: counter,
                    iteration: counter,
                    ciphertext: vec![6; size],
                    signature: [7; 64],
                };
                let distribution = SenderKeyDistribution {
                    id: counter,
                    iteration: counter,
                    chain_key: [6; 32],
                    signing_key: [7; 32],
                };
                let mut digest = Sha256::new();
                for encoded in [
                    signal.encode_with_version(version),
                    authenticated.serialized().to_vec(),
                    prekey.encode_with_version(version),
                    group.encode(),
                    distribution.encode(),
                ] {
                    digest.update(u64::try_from(encoded.len()).unwrap().to_le_bytes());
                    digest.update(encoded);
                }
                hashes.push(hex::encode(digest.finalize()));
            }
        }
        // Pin complete encodings and MAC bytes independently of buffer sizing.
        assert_eq!(
            hashes,
            [
                "6770df5338c9c8c81af1c6e6113de8b3700a46eb4b3221f25aec6dd09a717cd2",
                "8702d7e40ac2c35e2296557694df1c78d50256c07f967d104ab067deea7df184",
                "32466210c4041e5863eae19b3b65309407c89a080121bb3e4849c8b271f4873d",
                "f7a082107fe8aa914cdfa76afd03a9fb4795ca97d429cc26090b9d42021f4d84",
                "ae209121a9782dee8535fe7fc417eab957d187edc4e1fba1354dcd5e2dcca7ce",
                "02f66851cc91cc20f8591d4f4301677da21ded2421485bc33e1d7341d43c2f43",
                "21baeec75662e976c7b97060fca8819e96b1d7f48b0660abebc7dfe2a5461a78",
                "4b3965a38402a84731ff2a4c54ca708012385e6bb35ec2890ae110dbc07f94a0",
                "b454fdaac61ca88b62cd81425689e5f9ea79bf84272729cd84e4df85e65e5358",
                "44e34687d8c0bdb855c574f783003479930d63cac284c690c7e82e57e438cecd",
                "3fbe64b45a1d279de4a5837a2c8f76af1cc2f2c2508b80e8feec7533fa8e31a7",
                "bfc4888b9768cf69bf52c712f4161262202dd521677a92dbe99d40fede0bb7a9",
            ]
        );
    }
    #[test]
    fn sender_distribution_debug_hides_chain_secret() {
        let distribution = SenderKeyDistribution {
            id: 11,
            iteration: 23,
            chain_key: [167; 32],
            signing_key: [9; 32],
        };
        for debug in [
            format!("{distribution:?}"),
            format!("{:#?}", Some(&distribution)),
        ] {
            assert!(debug.contains("SenderKeyDistribution"));
            assert!(debug.contains("11"));
            assert!(debug.contains("23"));
            assert!(!debug.contains("167"));
            assert!(!debug.contains("\\xa7"));
        }
    }

    #[test]
    fn classic_pairwise_version_four_preserves_original_version_byte() {
        let signal = SignalMessage {
            ratchet: [9; 32],
            counter: 7,
            previous_counter: 2,
            ciphertext: vec![3; 16],
            mac: [4; 8],
        };
        let mut encoded = signal.encode_with_version(4);
        assert_eq!(encoded[0], 0x43);
        assert_eq!(
            SignalMessageView::decode(&encoded).unwrap().to_owned(),
            signal
        );
        assert_eq!(
            signal.unsigned_with_version(4),
            &encoded[..encoded.len() - 8]
        );
        for byte in [0x40, 0x43, 0x4f] {
            encoded[0] = byte;
            let view = SignalMessageView::decode(&encoded).unwrap();
            assert_eq!(view.unsigned()[0], byte);
            let prekey = PreKeySignalMessage {
                registration: 5,
                pre_key_id: None,
                signed_pre_key_id: 7,
                base_key: [8; 32],
                identity: [9; 32],
                message: encoded.clone(),
            };
            let mut outer = prekey.encode_with_version(4);
            outer[0] = byte;
            let view = PreKeySignalMessageView::decode(&outer).unwrap();
            assert_eq!(view.bytes[0], byte);
            assert_eq!(view.message, encoded);
        }
        assert_eq!(signal.encode()[0], 0x33);
        let group = SenderKeyMessage {
            id: 3,
            iteration: 4,
            ciphertext: vec![6; 16],
            signature: [7; 64],
        };
        let mut encoded = group.encode();
        encoded[0] = 0x43;
        assert_eq!(SenderKeyMessageView::decode(&encoded), Err(Error::Version));
        let distribution = SenderKeyDistribution {
            id: 3,
            iteration: 4,
            chain_key: [6; 32],
            signing_key: [7; 32],
        };
        let mut encoded = distribution.encode();
        encoded[0] = 0x43;
        assert_eq!(SenderKeyDistribution::decode(&encoded), Err(Error::Version));
    }

    fn inside(input: &[u8], part: &[u8]) -> bool {
        let start = input.as_ptr() as usize;
        let end = start + input.len();
        let part_start = part.as_ptr() as usize;
        part_start >= start && part_start + part.len() <= end
    }
    #[test]
    fn borrowed_authentication_bytes_include_unknown_fields() {
        let owned = SignalMessage {
            ratchet: [9; 32],
            counter: 7,
            previous_counter: 2,
            ciphertext: vec![3; 128],
            mac: [4; 8],
        };
        let mut encoded = owned.unsigned();
        encoded.extend_from_slice(&[0x78, 0x07]);
        encoded.extend_from_slice(&owned.mac);
        let view = SignalMessageView::decode(&encoded).unwrap();
        assert!(inside(&encoded, view.ciphertext));
        assert_eq!(view.ciphertext, owned.ciphertext);
        assert_eq!(view.serialized().as_ptr(), encoded.as_ptr());
        assert_eq!(view.unsigned(), &encoded[..encoded.len() - 8]);
        assert_eq!(&view.unsigned()[view.unsigned().len() - 2..], &[0x78, 0x07]);
        assert_eq!(view.to_owned(), owned);
        let sender = SenderKeyMessage {
            id: 3,
            iteration: 4,
            ciphertext: vec![6; 128],
            signature: [7; 64],
        };
        let mut encoded = sender.unsigned();
        encoded.extend_from_slice(&[0x78, 0x07]);
        encoded.extend_from_slice(&sender.signature);
        let view = SenderKeyMessageView::decode(&encoded).unwrap();
        assert!(inside(&encoded, view.ciphertext));
        assert_eq!(view.unsigned(), &encoded[..encoded.len() - 64]);
        assert_eq!(view.to_owned(), sender);
    }
    #[test]
    fn borrowed_prekey_validates_and_borrows_nested_message() {
        let signal = SignalMessage {
            ratchet: [9; 32],
            counter: 0,
            previous_counter: 0,
            ciphertext: vec![3; 128],
            mac: [4; 8],
        };
        let owned = PreKeySignalMessage {
            registration: 5,
            pre_key_id: Some(6),
            signed_pre_key_id: 7,
            base_key: [8; 32],
            identity: [9; 32],
            message: signal.encode(),
        };
        let mut encoded = owned.encode();
        encoded.extend_from_slice(&[0x78, 0x07]);
        let view = PreKeySignalMessageView::decode(&encoded).unwrap();
        assert!(inside(&encoded, view.message));
        assert_eq!(view.serialized(), encoded);
        assert_eq!(view.to_owned(), owned);
        assert!(inside(
            &encoded,
            SignalMessageView::decode(view.message).unwrap().ciphertext
        ));
        let mut invalid = owned;
        invalid.message = vec![0x33];
        assert!(PreKeySignalMessageView::decode(&invalid.encode()).is_err());
    }
}
