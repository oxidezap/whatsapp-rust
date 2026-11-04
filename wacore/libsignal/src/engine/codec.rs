// SPDX-License-Identifier: MIT
//! Persisted protobuf data codecs. Reloads burn reserved counters unless the
//! caller supplies the live cache incarnation. No storage operation occurs here.
use crate::engine::{CryptoProvider, Error, Result, state::*, wire::*};
use std::collections::VecDeque;
type Data<'a> = Vec<(u32, Value<'a>)>;
// The wire byte bound alone does not bound a table of two-byte scalar fields.
// This independent per-message bound caps the table at 4 MiB on 64-bit hosts,
// while allowing historical skipped-key backlogs well above the default 2000.
pub(crate) const MAX_STORED_FIELDS: usize = 131_072;
fn fields(bytes: &[u8]) -> Result<Data<'_>> {
    let mut f = Fields::new_storage(bytes)?;
    let mut out = Vec::new();
    while let Some(field) = f.next()? {
        if out.len() == MAX_STORED_FIELDS {
            return Err(Error::Encoding("stored protobuf field budget exceeded"));
        }
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
        // A redundant seed is useful for operational export only when it
        // reproduces the authoritative expanded fields exactly.
        if let Some(Value::Bytes(seed)) = one(&f, 100)
            && seed.len() == 32
        {
            use subtle::ConstantTimeEq;
            let candidate = MessageKey::Seed {
                index,
                seed: array(seed)?,
            };
            let expanded = zeroize::Zeroizing::new(candidate.expanded(&crate::engine::RustCrypto)?);
            if bool::from(
                expanded.0.as_slice().ct_eq(cipher)
                    & expanded.1.as_slice().ct_eq(mac)
                    & expanded.2.as_slice().ct_eq(iv),
            ) {
                return Ok(candidate);
            }
        }
        return Ok(MessageKey::Derived {
            index,
            material: Box::new(DerivedMessageKey {
                cipher: array(cipher)?,
                mac: array(mac)?,
                iv: array(iv)?,
            }),
        });
    }
    Ok(MessageKey::Seed {
        index,
        seed: raw(&f, 100)?,
    })
}
enum StoredChainRole {
    Sender,
    Receiver,
}
fn decode_chain(input: &[u8], role: StoredChainRole) -> Result<Chain> {
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
        // Historical readers ignore this unused field on receiving chains.
        private: match role {
            StoredChainRole::Sender => data(&f, 2)?
                .filter(|b| !b.is_empty())
                .map(array)
                .transpose()?,
            StoredChainRole::Receiver => None,
        },
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
        skipped: skipped.into(),
        message_key_limit: one(&f, 100).map(as_u32).transpose()?,
    })
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
pub fn encode_session_state(state: &SessionState) -> Vec<u8> {
    let mut out = Vec::new();
    crate::engine::encode::append_session_state(state, &mut out);
    out
}
pub fn decode_session_state<C: CryptoProvider>(_crypto: &C, input: &[u8]) -> Result<SessionState> {
    let f = fields(input)?;
    let version = number(&f, 1)?;
    let mut receivers = Vec::new();
    for (n, v) in &f {
        if *n == 7 {
            if receivers.len() == MAX_RECEIVER_CHAINS {
                return Err(Error::Encoding("too many receiver chains"));
            }
            receivers.push(decode_chain(as_bytes(*v)?, StoredChainRole::Receiver)?);
        }
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
        sender: data(&f, 6)?
            .map(|bytes| decode_chain(bytes, StoredChainRole::Sender))
            .transpose()?,
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
    crate::engine::encode::append_session_parts(current, previous, reservation, incarnation, out);
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
        if *n == 2 && previous.len() < MAX_ARCHIVED_SESSIONS {
            let b = as_bytes(*v)?;
            decode_session_state(crypto, b)?;
            previous.push(ArchivedSession { bytes: b.into() });
        }
    }
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
        skipped: skipped.into(),
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
    crate::engine::encode::append_sender_record(record, incarnation, &mut out);
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
        if *n == 1 && states.len() < MAX_SENDER_STATES {
            states.push(decode_sender_state(as_bytes(*v)?)?);
        }
    }
    if !trusted && let (Some(ceiling), Some(state)) = (reservation.ceiling, states.first_mut()) {
        if ceiling.saturating_sub(state.iteration) > 2000 {
            return Err(Error::Encoding("sender reservation burn exceeds limit"));
        }
        while state.iteration < ceiling {
            state.chain_key = crypto.hmac_sha256(&state.chain_key, &[&[2]])?;
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

#[cfg(test)]
mod parser_budget_tests {
    use super::*;
    use crate::engine::RustCrypto;

    fn session(registration: u32) -> Vec<u8> {
        let mut out = Vec::new();
        uint(&mut out, 1, 3);
        bytes(&mut out, 2, &tagged_key(&[1; 32]));
        bytes(&mut out, 3, &tagged_key(&[2; 32]));
        bytes(&mut out, 4, &[3; 32]);
        uint(&mut out, 10, registration.into());
        out
    }
    fn receiver() -> Vec<u8> {
        let mut out = Vec::new();
        bytes(&mut out, 1, &tagged_key(&[4; 32]));
        let mut position = Vec::new();
        uint(&mut position, 1, 7);
        bytes(&mut position, 2, &[5; 32]);
        bytes(&mut out, 3, &position);
        out
    }
    fn sender(id: u32) -> Vec<u8> {
        let mut out = Vec::new();
        uint(&mut out, 1, id.into());
        let mut position = Vec::new();
        uint(&mut position, 1, 9);
        bytes(&mut position, 2, &[6; 32]);
        bytes(&mut out, 2, &position);
        let mut signing = Vec::new();
        bytes(&mut signing, 1, &tagged_key(&[7; 32]));
        bytes(&mut out, 3, &signing);
        out
    }

    #[test]
    fn tiny_unknown_fields_reject_at_a_finite_table_budget() {
        assert!(MAX_STORED_FIELDS * size_of::<(u32, Value<'_>)>() <= 8 * 1024 * 1024);
        let mut input = [0x78, 0].repeat(MAX_STORED_FIELDS);
        let at_limit = fields(&input).unwrap();
        assert_eq!(at_limit.len(), MAX_STORED_FIELDS);
        assert!(at_limit.capacity() <= MAX_STORED_FIELDS);
        drop(at_limit);
        input.extend_from_slice(&[0x78, 0]);
        assert!(matches!(
            fields(&input),
            Err(Error::Encoding("stored protobuf field budget exceeded"))
        ));
        assert!(matches!(
            decode_session_record(&RustCrypto, &input),
            Err(Error::Encoding("stored protobuf field budget exceeded"))
        ));
    }

    #[test]
    fn archives_outside_retained_prefix_are_not_decoded() {
        let mut input = Vec::new();
        for registration in 0..MAX_ARCHIVED_SESSIONS as u32 {
            bytes(&mut input, 2, &session(registration));
        }
        bytes(&mut input, 2, &[0]);
        let decoded = decode_session_record(&RustCrypto, &input).unwrap();
        assert_eq!(decoded.previous.len(), MAX_ARCHIVED_SESSIONS);
        for (index, archive) in decoded.previous.iter().enumerate() {
            assert_eq!(
                archive.decode(&RustCrypto).unwrap().remote_registration,
                index as u32
            );
        }
        let mut retained_invalid = Vec::new();
        bytes(&mut retained_invalid, 2, &[0]);
        assert!(decode_session_record(&RustCrypto, &retained_invalid).is_err());
    }

    #[test]
    fn sender_states_outside_retained_prefix_are_not_decoded() {
        let mut input = Vec::new();
        for id in 0..MAX_SENDER_STATES as u32 {
            bytes(&mut input, 1, &sender(id));
        }
        bytes(&mut input, 1, &[0]);
        let decoded = decode_sender_key_record(&RustCrypto, &input).unwrap();
        assert_eq!(decoded.states.len(), MAX_SENDER_STATES);
        assert_eq!(
            decoded
                .states
                .iter()
                .map(|state| state.id)
                .collect::<Vec<_>>(),
            (0..MAX_SENDER_STATES as u32).collect::<Vec<_>>()
        );
        let mut retained_invalid = Vec::new();
        bytes(&mut retained_invalid, 1, &[0]);
        assert!(decode_sender_key_record(&RustCrypto, &retained_invalid).is_err());
    }

    #[test]
    fn sixth_receiver_rejects_before_its_nested_payload_is_decoded() {
        let mut input = session(0);
        for _ in 0..MAX_RECEIVER_CHAINS {
            bytes(&mut input, 7, &receiver());
        }
        assert_eq!(
            decode_session_state(&RustCrypto, &input)
                .unwrap()
                .receivers
                .len(),
            MAX_RECEIVER_CHAINS
        );
        bytes(&mut input, 7, &[0]);
        assert!(matches!(
            decode_session_state(&RustCrypto, &input),
            Err(Error::Encoding("too many receiver chains"))
        ));
    }

    #[test]
    fn field_budget_does_not_reduce_historical_skipped_key_retention_to_default() {
        let mut chain = receiver();
        uint(&mut chain, 100, 10_000);
        for index in 0..4096u32 {
            let mut key = Vec::new();
            uint(&mut key, 1, index.into());
            bytes(&mut key, 100, &[8; 32]);
            bytes(&mut chain, 4, &key);
        }
        let mut input = session(0);
        bytes(&mut input, 7, &chain);
        let decoded = decode_session_state(&RustCrypto, &input).unwrap();
        assert_eq!(decoded.receivers[0].skipped.len(), 4096);
        assert_eq!(decoded.receivers[0].message_key_limit, Some(10_000));
        assert_eq!(decoded.receivers[0].skipped[4095].index(), 4095);
    }
}

#[cfg(test)]
mod message_key_tests {
    use super::*;
    #[test]
    fn redundant_seed_requires_all_expanded_fields_to_match() {
        let seed = [17; 32];
        for variant in 0..8 {
            let (mut cipher, mut mac, mut iv) = MessageKey::Seed { index: 5, seed }
                .expanded(&crate::engine::RustCrypto)
                .unwrap();
            let mut stored_seed = seed.to_vec();
            match variant {
                1 => cipher[0] ^= 1,
                2 => mac[0] ^= 1,
                3 => iv[0] ^= 1,
                4 => stored_seed[0] ^= 1,
                5 => {
                    stored_seed.pop();
                }
                _ => {}
            }
            let mut wire = Vec::new();
            uint(&mut wire, 1, 5);
            bytes(&mut wire, 2, &cipher);
            bytes(&mut wire, 3, &mac);
            bytes(&mut wire, 4, &iv);
            match variant {
                6 => {}
                7 => uint(&mut wire, 100, 1),
                _ => bytes(&mut wire, 100, &stored_seed),
            }
            let key = message_key(&wire).unwrap();
            assert_eq!(matches!(key, MessageKey::Seed { .. }), variant == 0);
            assert_eq!(
                key.expanded(&crate::engine::RustCrypto).unwrap(),
                (cipher, mac, iv)
            );
        }
    }
}
