// SPDX-License-Identifier: MIT
//! Wire field numbers follow the public protobuf data declarations.
use crate::{Error, Result};
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
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(Error::Encoding("protobuf size limit"));
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
    let (&version, rest) = bytes
        .split_first()
        .ok_or(Error::Encoding("missing version"))?;
    if version >> 4 != 3 || version & 15 < 3 {
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
        let mut out = vec![0x33];
        bytes(&mut out, 1, &tagged_key(&self.ratchet));
        uint(&mut out, 2, self.counter.into());
        uint(&mut out, 3, self.previous_counter.into());
        bytes(&mut out, 4, &self.ciphertext);
        out
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.unsigned();
        out.extend_from_slice(&self.mac);
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        let (body, mac) = versioned(input, 8)?;
        let mut fields = Fields::new(body)?;
        let (mut ratchet, mut counter, mut previous, mut ciphertext) = (None, None, 0, None);
        while let Some((n, v)) = fields.next()? {
            match n {
                1 => ratchet = Some(decode_public(as_bytes(v)?)?),
                2 => counter = Some(as_u32(v)?),
                3 => previous = as_u32(v)?,
                4 => ciphertext = Some(as_bytes(v)?.to_vec()),
                _ => {}
            }
        }
        Ok(Self {
            ratchet: ratchet.ok_or(Error::MissingKey)?,
            counter: counter.ok_or(Error::Encoding("missing counter"))?,
            previous_counter: previous,
            ciphertext: ciphertext.ok_or(Error::Encoding("missing ciphertext"))?,
            mac: array(mac)?,
        })
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
        let mut out = vec![0x33];
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
        let (body, _) = versioned(input, 0)?;
        let mut f = Fields::new(body)?;
        let (mut registration, mut pre_key_id, mut signed, mut base, mut identity, mut message) =
            (0, None, None, None, None, None);
        while let Some((n, v)) = f.next()? {
            match n {
                1 => pre_key_id = Some(as_u32(v)?),
                2 => base = Some(decode_public(as_bytes(v)?)?),
                3 => identity = Some(decode_public(as_bytes(v)?)?),
                4 => message = Some(as_bytes(v)?.to_vec()),
                5 => registration = as_u32(v)?,
                6 => signed = Some(as_u32(v)?),
                7 | 8 => return Err(Error::Version),
                _ => {}
            }
        }
        Ok(Self {
            registration,
            pre_key_id,
            signed_pre_key_id: signed.ok_or(Error::MissingKey)?,
            base_key: base.ok_or(Error::MissingKey)?,
            identity: identity.ok_or(Error::MissingKey)?,
            message: message.ok_or(Error::MissingKey)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SenderKeyDistribution {
    pub id: u32,
    pub iteration: u32,
    pub chain_key: [u8; 32],
    pub signing_key: [u8; 32],
}
impl SenderKeyDistribution {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![0x33];
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
        let mut out = vec![0x33];
        uint(&mut out, 1, self.id.into());
        uint(&mut out, 2, self.iteration.into());
        bytes(&mut out, 3, &self.ciphertext);
        out
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.unsigned();
        out.extend_from_slice(&self.signature);
        out
    }
    pub fn decode(input: &[u8]) -> Result<Self> {
        let (body, sig) = versioned(input, 64)?;
        let mut f = Fields::new(body)?;
        let (mut id, mut iteration, mut ciphertext) = (None, None, None);
        while let Some((n, v)) = f.next()? {
            match n {
                1 => id = Some(as_u32(v)?),
                2 => iteration = Some(as_u32(v)?),
                3 => ciphertext = Some(as_bytes(v)?.to_vec()),
                _ => {}
            }
        }
        Ok(Self {
            id: id.ok_or(Error::MissingKey)?,
            iteration: iteration.ok_or(Error::MissingKey)?,
            ciphertext: ciphertext.ok_or(Error::MissingKey)?,
            signature: array(sig)?,
        })
    }
}
