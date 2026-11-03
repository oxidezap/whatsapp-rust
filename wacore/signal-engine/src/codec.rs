// SPDX-License-Identifier: MIT
//! Persisted protobuf data codecs. Reloads burn reserved counters unless the
//! caller supplies the live cache incarnation. No storage operation occurs here.
use crate::{CryptoProvider, Error, Result, state::*, wire::*};
use std::collections::VecDeque;
type Data<'a> = Vec<(u32, Value<'a>)>;
fn fields(bytes: &[u8]) -> Result<Data<'_>> {
    let mut f = Fields::new(bytes)?;
    let mut out = Vec::new();
    while let Some(field) = f.next()? {
        out.push(field);
    }
    Ok(out)
}
fn one<'a>(f: &Data<'a>, n: u32) -> Option<Value<'a>> {
    f.iter()
        .rev()
        .find_map(|(number, value)| (*number == n).then_some(*value))
}
fn number(f: &Data<'_>, n: u32) -> Result<u32> {
    one(f, n).map(as_u32).transpose().map(|x| x.unwrap_or(0))
}
fn data<'a>(f: &Data<'a>, n: u32) -> Result<Option<&'a [u8]>> {
    one(f, n).map(as_bytes).transpose()
}
fn required<'a>(f: &Data<'a>, n: u32) -> Result<&'a [u8]> {
    data(f, n)?.ok_or(Error::Encoding("missing required field"))
}
fn public(f: &Data<'_>, n: u32) -> Result<[u8; 32]> {
    decode_public(required(f, n)?)
}
fn raw<const N: usize>(f: &Data<'_>, n: u32) -> Result<[u8; N]> {
    array(required(f, n)?)
}
fn message_key(input: &[u8]) -> Result<MessageKey> {
    let f = fields(input)?;
    let index = number(&f, 1)?;
    if let (Some(cipher), Some(mac), Some(iv)) = (data(&f, 2)?, data(&f, 3)?, data(&f, 4)?)
        && cipher.len() == 32
        && mac.len() == 32
        && iv.len() == 16
    {
        return Ok(MessageKey::Derived {
            index,
            cipher: array(cipher)?,
            mac: array(mac)?,
            iv: array(iv)?,
        });
    }
    Ok(MessageKey::Seed {
        index,
        seed: raw(&f, 100)?,
    })
}
fn encode_message_key(key: &MessageKey) -> Vec<u8> {
    let mut out = Vec::new();
    uint(&mut out, 1, key.index().into());
    match key {
        MessageKey::Seed { seed, .. } => bytes(&mut out, 100, seed),
        MessageKey::Derived {
            cipher, mac, iv, ..
        } => {
            bytes(&mut out, 2, cipher);
            bytes(&mut out, 3, mac);
            bytes(&mut out, 4, iv);
        }
    }
    out
}
fn decode_chain(input: &[u8]) -> Result<Chain> {
    let f = fields(input)?;
    let ck = data(&f, 3)?.map(fields).transpose()?;
    let mut skipped = VecDeque::new();
    for (n, v) in &f {
        if *n == 4 {
            let key = message_key(as_bytes(*v)?)?;
            skipped.push_back(key);
        }
    }
    Ok(Chain {
        ratchet: public(&f, 1)?,
        private: data(&f, 2)?
            .filter(|b| !b.is_empty())
            .map(array)
            .transpose()?,
        index: ck.as_ref().map(|f| number(f, 1)).transpose()?.unwrap_or(0),
        key: ck
            .as_ref()
            .map(|f| data(f, 2))
            .transpose()?
            .flatten()
            .map(array)
            .transpose()?
            .unwrap_or([0; 32]),
        exhausted: ck
            .as_ref()
            .map(|f| data(f, 2))
            .transpose()?
            .flatten()
            .is_none(),
        skipped,
        message_key_limit: one(&f, 100).map(as_u32).transpose()?,
    })
}
fn encode_chain(chain: &Chain) -> Vec<u8> {
    let mut out = Vec::new();
    bytes(&mut out, 1, &tagged_key(&chain.ratchet));
    if let Some(private) = chain.private {
        bytes(&mut out, 2, &private);
    }
    let mut ck = Vec::new();
    uint(&mut ck, 1, chain.index.into());
    if !chain.exhausted {
        bytes(&mut ck, 2, &chain.key);
    }
    bytes(&mut out, 3, &ck);
    for key in &chain.skipped {
        bytes(&mut out, 4, &encode_message_key(key));
    }
    if let Some(limit) = chain.message_key_limit {
        uint(&mut out, 100, limit.into());
    }
    out
}
fn decode_pending(input: &[u8]) -> Result<PendingPreKey> {
    let f = fields(input)?;
    let signed = match one(&f, 3) {
        Some(Value::Varint(n)) => Some(n as i32),
        None => None,
        _ => return Err(Error::Encoding("signed prekey id")),
    };
    Ok(PendingPreKey {
        pre_key_id: one(&f, 1).map(as_u32).transpose()?,
        base_key: public(&f, 2)?,
        signed_pre_key_id: signed,
        kyber_pre_key_id: one(&f, 4).map(as_u32).transpose()?,
        kyber_ciphertext: data(&f, 5)?.map(<[u8]>::to_vec),
    })
}
fn encode_pending(key: &PendingPreKey) -> Vec<u8> {
    let mut out = Vec::new();
    if let Some(id) = key.pre_key_id {
        uint(&mut out, 1, id.into());
    }
    bytes(&mut out, 2, &tagged_key(&key.base_key));
    if let Some(id) = key.signed_pre_key_id {
        uint(&mut out, 3, id as i64 as u64);
    }
    if let Some(id) = key.kyber_pre_key_id {
        uint(&mut out, 4, id.into());
    }
    if let Some(ciphertext) = &key.kyber_ciphertext {
        bytes(&mut out, 5, ciphertext);
    }
    out
}
pub fn encode_session_state(state: &SessionState) -> Vec<u8> {
    let mut out = Vec::new();
    uint(&mut out, 1, state.version.into());
    bytes(&mut out, 2, &tagged_key(&state.local_identity));
    bytes(&mut out, 3, &tagged_key(&state.remote_identity));
    bytes(&mut out, 4, &state.root);
    uint(&mut out, 5, state.previous_counter.into());
    if let Some(sender) = &state.sender {
        bytes(&mut out, 6, &encode_chain(sender));
    }
    for receiver in &state.receivers {
        bytes(&mut out, 7, &encode_chain(receiver));
    }
    if let Some(exchange) = &state.pending_key_exchange {
        bytes(&mut out, 8, exchange);
    }
    if let Some(pending) = &state.pending_pre_key {
        bytes(&mut out, 9, &encode_pending(pending));
    }
    uint(&mut out, 10, state.remote_registration.into());
    uint(&mut out, 11, state.local_registration.into());
    uint(&mut out, 12, u64::from(state.needs_refresh));
    if let Some(base) = state.alice_base_key {
        if state.alice_base_key_tagged {
            bytes(&mut out, 13, &tagged_key(&base));
        } else {
            bytes(&mut out, 13, &base);
        }
    }
    out
}
pub fn decode_session_state<C: CryptoProvider>(_crypto: &C, input: &[u8]) -> Result<SessionState> {
    let f = fields(input)?;
    let version = number(&f, 1)?;
    if version != 3 {
        return Err(Error::Version);
    }
    let mut receivers = Vec::new();
    for (n, v) in &f {
        if *n == 7 {
            receivers.push(decode_chain(as_bytes(*v)?)?);
        }
    }
    if receivers.len() > MAX_RECEIVER_CHAINS {
        return Err(Error::Encoding("too many receiver chains"));
    }
    let pending_key_exchange = data(&f, 8)?.map(<[u8]>::to_vec);
    if let Some(exchange) = &pending_key_exchange {
        fields(exchange)?;
    }
    Ok(SessionState {
        version,
        local_identity: public(&f, 2)?,
        remote_identity: public(&f, 3)?,
        root: raw(&f, 4)?,
        previous_counter: number(&f, 5)?,
        sender: data(&f, 6)?.map(decode_chain).transpose()?,
        receivers,
        pending_key_exchange,
        pending_pre_key: data(&f, 9)?.map(decode_pending).transpose()?,
        remote_registration: number(&f, 10)?,
        local_registration: number(&f, 11)?,
        needs_refresh: number(&f, 12)? != 0,
        alice_base_key: data(&f, 13)?.map(decode_public).transpose()?,
        alice_base_key_tagged: data(&f, 13)?.is_none_or(|bytes| bytes.len() == 33),
    })
}
fn encode_reservation_with_incarnation(
    out: &mut Vec<u8>,
    r: &Reservation,
    incarnation: Option<&[u8; 16]>,
) {
    if let Some(ceiling) = r.ceiling {
        uint(out, 100, ceiling.into());
    }
    if let Some(token) = incarnation.or(r.incarnation.as_ref()) {
        bytes(out, 101, token);
    }
}
fn decode_reservation(f: &Data<'_>, live: Option<&[u8; 16]>) -> Result<(Reservation, bool)> {
    let mut ceiling = None;
    let mut incarnation = None;
    let mut token_count = 0;
    for (n, v) in f {
        match n {
            100 => {
                if ceiling.is_some() {
                    return Err(Error::Encoding("duplicate reservation ceiling"));
                }
                ceiling = Some(as_u32(*v)?);
            }
            101 => {
                token_count += 1;
                if let Value::Bytes(b) = v {
                    incarnation = array(b).ok();
                }
            }
            _ => {}
        }
    }
    if token_count != 1 {
        incarnation = None;
    }
    let trusted = live.is_some_and(|expected| incarnation.as_ref() == Some(expected));
    Ok((
        Reservation {
            ceiling,
            incarnation,
            committed: trusted,
            waived: false,
        },
        trusted,
    ))
}
pub fn encode_session_record(record: &SessionRecord) -> Vec<u8> {
    encode_session_record_parts(
        record.current.as_ref(),
        &record.previous,
        &record.reservation,
        None,
    )
}

/// Encode a host-owned current state without cloning its skipped-key backlog.
pub fn encode_session_record_parts(
    current: Option<&SessionState>,
    previous: &[ArchivedSession],
    reservation: &Reservation,
    incarnation: Option<&[u8; 16]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    encode_session_record_parts_into(current, previous, reservation, incarnation, &mut out);
    out
}

/// Append a record to the host's reusable persistence buffer.
pub fn encode_session_record_parts_into(
    current: Option<&SessionState>,
    previous: &[ArchivedSession],
    reservation: &Reservation,
    incarnation: Option<&[u8; 16]>,
    out: &mut Vec<u8>,
) {
    if let Some(state) = current {
        bytes(out, 1, &encode_session_state(state));
    }
    for archive in previous {
        bytes(out, 2, &archive.bytes);
    }
    encode_reservation_with_incarnation(out, reservation, incarnation);
}
pub fn decode_session_record<C: CryptoProvider>(crypto: &C, input: &[u8]) -> Result<SessionRecord> {
    decode_session_record_inner(crypto, input, None)
}
pub fn decode_session_record_live<C: CryptoProvider>(
    crypto: &C,
    input: &[u8],
    incarnation: &[u8; 16],
) -> Result<SessionRecord> {
    decode_session_record_inner(crypto, input, Some(incarnation))
}
fn decode_session_record_inner<C: CryptoProvider>(
    crypto: &C,
    input: &[u8],
    live: Option<&[u8; 16]>,
) -> Result<SessionRecord> {
    let f = fields(input)?;
    let (mut reservation, trusted) = decode_reservation(&f, live)?;
    let mut current = data(&f, 1)?
        .map(|b| decode_session_state(crypto, b))
        .transpose()?;
    let mut previous = Vec::new();
    for (n, v) in &f {
        if *n == 2 {
            let b = as_bytes(*v)?;
            decode_session_state(crypto, b)?;
            previous.push(ArchivedSession { bytes: b.to_vec() });
        }
    }
    previous.truncate(MAX_ARCHIVED_SESSIONS);
    if !trusted
        && let (Some(ceiling), Some(chain)) = (
            reservation.ceiling,
            current.as_mut().and_then(|s| s.sender.as_mut()),
        )
    {
        if ceiling.saturating_sub(chain.index) > 2000 {
            return Err(Error::Encoding("reservation burn exceeds limit"));
        }
        while chain.index < ceiling {
            chain.step(crypto)?;
        }
        reservation.incarnation = None;
    }
    Ok(SessionRecord {
        current,
        previous,
        reservation,
    })
}
fn encode_sender_state(state: &SenderKeyState) -> Vec<u8> {
    let mut out = Vec::new();
    uint(&mut out, 1, state.id.into());
    let mut ck = Vec::new();
    uint(&mut ck, 1, state.iteration.into());
    bytes(&mut ck, 2, &state.chain_key);
    bytes(&mut out, 2, &ck);
    let mut signing = Vec::new();
    bytes(&mut signing, 1, &tagged_key(&state.signing_public));
    if let Some(private) = state.signing_private {
        bytes(&mut signing, 2, &private);
    }
    bytes(&mut out, 3, &signing);
    for key in &state.skipped {
        let mut k = Vec::new();
        uint(&mut k, 1, key.iteration.into());
        bytes(&mut k, 2, &key.seed);
        bytes(&mut out, 4, &k);
    }
    out
}
fn decode_sender_state(input: &[u8]) -> Result<SenderKeyState> {
    let f = fields(input)?;
    let ck = fields(required(&f, 2)?)?;
    let signing = fields(required(&f, 3)?)?;
    let mut skipped = VecDeque::new();
    for (n, v) in &f {
        if *n == 4 {
            let key = fields(as_bytes(*v)?)?;
            let iteration = number(&key, 1)?;
            skipped.push_back(SenderMessageKey {
                iteration,
                seed: raw(&key, 2)?,
            });
        }
    }
    Ok(SenderKeyState {
        id: number(&f, 1)?,
        iteration: number(&ck, 1)?,
        chain_key: raw(&ck, 2)?,
        signing_public: public(&signing, 1)?,
        signing_private: data(&signing, 2)?
            .filter(|b| !b.is_empty())
            .map(array)
            .transpose()?,
        skipped,
    })
}
pub fn encode_sender_key_record(record: &SenderKeyRecord) -> Vec<u8> {
    encode_sender_key_record_with_incarnation(record, None)
}

/// Encode the same group state with the host cache's live incarnation token.
pub fn encode_sender_key_record_for_store(
    record: &SenderKeyRecord,
    incarnation: &[u8; 16],
) -> Vec<u8> {
    encode_sender_key_record_with_incarnation(record, Some(incarnation))
}

fn encode_sender_key_record_with_incarnation(
    record: &SenderKeyRecord,
    incarnation: Option<&[u8; 16]>,
) -> Vec<u8> {
    let mut out = Vec::new();
    for state in &record.states {
        bytes(&mut out, 1, &encode_sender_state(state));
    }
    encode_reservation_with_incarnation(&mut out, &record.reservation, incarnation);
    out
}
pub fn decode_sender_key_record<C: CryptoProvider>(
    crypto: &C,
    input: &[u8],
) -> Result<SenderKeyRecord> {
    decode_sender_key_record_inner(crypto, input, None)
}
pub fn decode_sender_key_record_live<C: CryptoProvider>(
    crypto: &C,
    input: &[u8],
    incarnation: &[u8; 16],
) -> Result<SenderKeyRecord> {
    decode_sender_key_record_inner(crypto, input, Some(incarnation))
}
fn decode_sender_key_record_inner<C: CryptoProvider>(
    crypto: &C,
    input: &[u8],
    live: Option<&[u8; 16]>,
) -> Result<SenderKeyRecord> {
    let f = fields(input)?;
    let (mut reservation, trusted) = decode_reservation(&f, live)?;
    let mut states = Vec::new();
    for (n, v) in &f {
        if *n == 1 {
            states.push(decode_sender_state(as_bytes(*v)?)?);
        }
    }
    states.truncate(MAX_SENDER_STATES);
    if !trusted && let (Some(ceiling), Some(state)) = (reservation.ceiling, states.first_mut()) {
        if ceiling.saturating_sub(state.iteration) > 2000 {
            return Err(Error::Encoding("sender reservation burn exceeds limit"));
        }
        while state.iteration < ceiling {
            state.chain_key = crypto.hmac_sha256(&state.chain_key, &[2]);
            state.iteration = state
                .iteration
                .checked_add(1)
                .ok_or(Error::CounterExhausted)?;
        }
        reservation.incarnation = None;
    }
    Ok(SenderKeyRecord {
        states,
        reservation,
    })
}
pub fn encode_pre_key(record: &PreKeyRecord) -> Vec<u8> {
    let mut out = Vec::new();
    uint(&mut out, 1, record.id.into());
    bytes(&mut out, 2, &record.key_pair.public);
    bytes(&mut out, 3, &record.key_pair.private);
    out
}
pub fn decode_pre_key(input: &[u8]) -> Result<PreKeyRecord> {
    let f = fields(input)?;
    Ok(PreKeyRecord {
        id: number(&f, 1)?,
        key_pair: KeyPair {
            public: public(&f, 2)?,
            private: raw(&f, 3)?,
        },
    })
}
pub fn encode_signed_pre_key(record: &SignedPreKeyRecord) -> Vec<u8> {
    let mut out = Vec::new();
    uint(&mut out, 1, record.id.into());
    bytes(&mut out, 2, &record.key_pair.public);
    bytes(&mut out, 3, &record.key_pair.private);
    bytes(&mut out, 4, &record.signature);
    fixed64(&mut out, 5, record.timestamp);
    out
}
pub fn decode_signed_pre_key(input: &[u8]) -> Result<SignedPreKeyRecord> {
    let f = fields(input)?;
    let timestamp = match one(&f, 5) {
        Some(Value::Fixed64(value)) => value,
        None => 0,
        _ => return Err(Error::Encoding("timestamp must be fixed64")),
    };
    Ok(SignedPreKeyRecord {
        id: number(&f, 1)?,
        key_pair: KeyPair {
            public: public(&f, 2)?,
            private: raw(&f, 3)?,
        },
        signature: raw(&f, 4)?,
        timestamp,
    })
}
pub fn encode_identity_key_pair(key: &KeyPair) -> Vec<u8> {
    let mut out = Vec::new();
    bytes(&mut out, 1, &tagged_key(&key.public));
    bytes(&mut out, 2, &key.private);
    out
}
pub fn decode_identity_key_pair(input: &[u8]) -> Result<KeyPair> {
    let f = fields(input)?;
    Ok(KeyPair {
        public: public(&f, 1)?,
        private: raw(&f, 2)?,
    })
}
