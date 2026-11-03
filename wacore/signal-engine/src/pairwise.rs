// SPDX-License-Identifier: MIT
//! X3DH and the v3 Double Ratchet profile. Parameters are explicit; the caller
//! resolves prekey IDs and applies its identity trust policy before initialization.
use crate::{
    CryptoProvider, Engine, Error, RandomSource, Result,
    crypto::{authenticate, root_step},
    state::*,
    wire::{self, PreKeySignalMessage, SignalMessage},
};
use zeroize::Zeroize;
#[derive(Clone)]
pub struct InitiatorParameters {
    pub local_identity: KeyPair,
    pub local_registration: u32,
    pub remote_identity: [u8; 32],
    pub remote_registration: u32,
    pub remote_signed_pre_key: [u8; 32],
    pub remote_signed_pre_key_id: u32,
    pub remote_signed_pre_key_signature: [u8; 64],
    pub remote_one_time_pre_key: Option<(u32, [u8; 32])>,
    pub base_key: KeyPair,
}
#[derive(Clone)]
pub struct ResponderParameters {
    pub local_identity: KeyPair,
    pub local_registration: u32,
    pub local_signed_pre_key: KeyPair,
    pub local_one_time_pre_key: Option<KeyPair>,
    pub remote_identity: [u8; 32],
    pub remote_base_key: [u8; 32],
    pub remote_registration: u32,
}
fn initial_secrets<C: CryptoProvider>(
    crypto: &C,
    pairs: &[(&[u8; 32], &[u8; 32])],
) -> Result<([u8; 32], [u8; 32])> {
    let mut input = vec![255; 32];
    for (private, public) in pairs {
        let mut shared = crypto.agreement(private, public)?;
        authenticate_nonzero(&shared)?;
        input.extend_from_slice(&shared);
        shared.zeroize();
    }
    let mut expanded = [0; 64];
    let result = crypto.hkdf_sha256(&[0; 32], &input, b"WhisperText", &mut expanded);
    input.zeroize();
    result?;
    let mut root = [0; 32];
    let mut chain = [0; 32];
    root.copy_from_slice(&expanded[..32]);
    chain.copy_from_slice(&expanded[32..]);
    expanded.zeroize();
    Ok((root, chain))
}
fn authenticate_nonzero(value: &[u8; 32]) -> Result<()> {
    use subtle::ConstantTimeEq;
    if bool::from(value.ct_eq(&[0; 32])) {
        Err(Error::Crypto)
    } else {
        Ok(())
    }
}
fn state(
    local: &KeyPair,
    remote: [u8; 32],
    local_registration: u32,
    remote_registration: u32,
    root: [u8; 32],
) -> SessionState {
    SessionState {
        version: 3,
        local_identity: local.public,
        remote_identity: remote,
        root,
        previous_counter: 0,
        sender: None,
        receivers: Vec::new(),
        pending_pre_key: None,
        local_registration,
        remote_registration,
        needs_refresh: false,
        alice_base_key: None,
        pending_key_exchange: None,
    }
}
impl<C: CryptoProvider, R: RandomSource> Engine<'_, C, R> {
    pub fn initialize_initiator(&mut self, p: &InitiatorParameters) -> Result<SessionState> {
        self.crypto.verify(
            &p.remote_identity,
            &wire::tagged_key(&p.remote_signed_pre_key),
            &p.remote_signed_pre_key_signature,
        )?;
        let mut pairs = vec![
            (&p.local_identity.private, &p.remote_signed_pre_key),
            (&p.base_key.private, &p.remote_identity),
            (&p.base_key.private, &p.remote_signed_pre_key),
        ];
        if let Some((_, public)) = &p.remote_one_time_pre_key {
            pairs.push((&p.base_key.private, public));
        }
        let (root, initial_chain) = initial_secrets(self.crypto, &pairs)?;
        let ratchet = self.generate_key_pair()?;
        let (next_root, sending) = root_step(
            self.crypto,
            &root,
            &ratchet.private,
            &p.remote_signed_pre_key,
        )?;
        let mut session = state(
            &p.local_identity,
            p.remote_identity,
            p.local_registration,
            p.remote_registration,
            next_root,
        );
        session
            .receivers
            .push(Chain::new(p.remote_signed_pre_key, None, initial_chain));
        session.sender = Some(Chain::new(ratchet.public, Some(ratchet.private), sending));
        session.alice_base_key = Some(p.base_key.public);
        session.pending_pre_key = Some(PendingPreKey {
            pre_key_id: p.remote_one_time_pre_key.as_ref().map(|(id, _)| *id),
            signed_pre_key_id: p.remote_signed_pre_key_id as i32,
            base_key: p.base_key.public,
            kyber_pre_key_id: None,
            kyber_ciphertext: None,
        });
        Ok(session)
    }
    pub fn initialize_responder(&mut self, p: &ResponderParameters) -> Result<SessionState> {
        let mut pairs = vec![
            (&p.local_signed_pre_key.private, &p.remote_identity),
            (&p.local_identity.private, &p.remote_base_key),
            (&p.local_signed_pre_key.private, &p.remote_base_key),
        ];
        if let Some(one_time) = &p.local_one_time_pre_key {
            pairs.push((&one_time.private, &p.remote_base_key));
        }
        let (root, chain) = initial_secrets(self.crypto, &pairs)?;
        let mut session = state(
            &p.local_identity,
            p.remote_identity,
            p.local_registration,
            p.remote_registration,
            root,
        );
        session.sender = Some(Chain::new(
            p.local_signed_pre_key.public,
            Some(p.local_signed_pre_key.private),
            chain,
        ));
        session.alice_base_key = Some(p.remote_base_key);
        Ok(session)
    }
    /// Returns prepared ciphertext. The caller must persist this transition before
    /// publishing it. A committed counter reservation must already cover its index.
    pub fn encrypt(&mut self, record: &mut SessionRecord, plaintext: &[u8]) -> Result<Vec<u8>> {
        let session = record.current.as_ref().ok_or(Error::MissingKey)?;
        let mut sender = session.sender.as_ref().ok_or(Error::MissingKey)?.clone();
        if !record.reservation.permits(sender.index) {
            return Err(Error::ReservationRequired);
        }
        let key = sender.step(self.crypto)?;
        let (cipher, mac, iv) = key.expanded(self.crypto)?;
        let ciphertext = self.crypto.encrypt_cbc(&cipher, &iv, plaintext)?;
        let mut message = SignalMessage {
            ratchet: sender.ratchet,
            counter: key.index(),
            previous_counter: session.previous_counter,
            ciphertext,
            mac: [0; 8],
        };
        let mut authenticated = Vec::new();
        authenticated.extend_from_slice(&wire::tagged_key(&session.local_identity));
        authenticated.extend_from_slice(&wire::tagged_key(&session.remote_identity));
        authenticated.extend_from_slice(&message.unsigned());
        message
            .mac
            .copy_from_slice(&self.crypto.hmac_sha256(&mac, &authenticated)[..8]);
        let mut output = message.encode();
        if let Some(pending) = &session.pending_pre_key {
            if pending.kyber_pre_key_id.is_some() || pending.kyber_ciphertext.is_some() {
                return Err(Error::Version);
            }
            output = PreKeySignalMessage {
                registration: session.local_registration,
                pre_key_id: pending.pre_key_id,
                signed_pre_key_id: pending.signed_pre_key_id as u32,
                base_key: pending.base_key,
                identity: session.local_identity,
                message: output,
            }
            .encode();
        }
        record.current.as_mut().ok_or(Error::MissingKey)?.sender = Some(sender);
        Ok(output)
    }
    pub fn decrypt(&mut self, record: &mut SessionRecord, input: &[u8]) -> Result<Vec<u8>> {
        let message = SignalMessage::decode(input)?;
        let mut failure = Error::MissingKey;
        if let Some(current) = record.current.as_mut() {
            if let Some(index) = current
                .receivers
                .iter()
                .position(|chain| chain.ratchet == message.ratchet)
            {
                match self.decrypt_known_chain(current, index, &message, input) {
                    Ok(plaintext) => return Ok(plaintext),
                    Err(error) => failure = error,
                }
            } else {
                let mut candidate = current.clone();
                match self.decrypt_state(&mut candidate, &message, input) {
                    Ok((plaintext, ratcheted)) => {
                        record.current = Some(candidate);
                        if ratcheted {
                            record.reservation = Reservation::default();
                        }
                        return Ok(plaintext);
                    }
                    Err(error) => failure = error,
                }
            }
        }
        for index in 0..record.previous.len() {
            let mut candidate = record.previous[index].decode(self.crypto)?;
            match self.decrypt_state(&mut candidate, &message, input) {
                Ok((plaintext, _)) => {
                    record.previous.remove(index);
                    record.install(candidate);
                    return Ok(plaintext);
                }
                Err(error) => {
                    if failure == Error::MissingKey {
                        failure = error;
                    }
                }
            }
        }
        Err(failure)
    }
    fn decrypt_known_chain(
        &self,
        session: &mut SessionState,
        index: usize,
        message: &SignalMessage,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let limit = if session.local_identity == session.remote_identity {
            SELF_FUTURE_LIMIT
        } else {
            PEER_FUTURE_LIMIT
        };
        let plan = session.receivers[index].plan(self.crypto, message.counter, limit)?;
        let (cipher, mac, iv) = plan.message.expanded(self.crypto)?;
        let mut authenticated = Vec::with_capacity(input.len() + 66);
        authenticated.extend_from_slice(&wire::tagged_key(&session.remote_identity));
        authenticated.extend_from_slice(&wire::tagged_key(&session.local_identity));
        authenticated.extend_from_slice(&input[..input.len() - 8]);
        authenticate(
            &self.crypto.hmac_sha256(&mac, &authenticated)[..8],
            &message.mac,
        )?;
        let plaintext = self.crypto.decrypt_cbc(&cipher, &iv, &message.ciphertext)?;
        plan.commit(&mut session.receivers[index]);
        session.pending_pre_key = None;
        Ok(plaintext)
    }
    /// The caller must select the local prekeys by the wrapper's IDs, and delete
    /// the one-time prekey only after persisting a successful returned transition.
    pub fn decrypt_pre_key(
        &mut self,
        record: &mut SessionRecord,
        input: &[u8],
        p: &ResponderParameters,
    ) -> Result<Vec<u8>> {
        let wrapper = PreKeySignalMessage::decode(input)?;
        if wrapper.identity != p.remote_identity
            || wrapper.base_key != p.remote_base_key
            || wrapper.registration != p.remote_registration
        {
            return Err(Error::IdentityChanged);
        }
        let mut existing = record.current.as_ref().is_some_and(|s| {
            s.alice_base_key == Some(wrapper.base_key) && s.remote_identity == wrapper.identity
        });
        if !existing {
            for archive in &record.previous {
                let s = archive.decode(self.crypto)?;
                if s.alice_base_key == Some(wrapper.base_key)
                    && s.remote_identity == wrapper.identity
                {
                    existing = true;
                    break;
                }
            }
        }
        if existing {
            return self.decrypt(record, &wrapper.message);
        }
        if wrapper.pre_key_id.is_some() != p.local_one_time_pre_key.is_some() {
            return Err(Error::MissingKey);
        }
        if record
            .current
            .as_ref()
            .is_some_and(|s| s.remote_identity != wrapper.identity)
        {
            return Err(Error::IdentityChanged);
        }
        let mut candidate = record.clone();
        candidate.install(self.initialize_responder(p)?);
        let plaintext = self.decrypt(&mut candidate, &wrapper.message)?;
        *record = candidate;
        Ok(plaintext)
    }
    fn decrypt_state(
        &mut self,
        session: &mut SessionState,
        message: &SignalMessage,
        input: &[u8],
    ) -> Result<(Vec<u8>, bool)> {
        if session.version != 3 {
            return Err(Error::Version);
        }
        let mut ratcheted = false;
        let index = if let Some(index) = session
            .receivers
            .iter()
            .position(|c| c.ratchet == message.ratchet)
        {
            index
        } else {
            let old = session.sender.as_ref().ok_or(Error::MissingKey)?;
            let private = old.private.ok_or(Error::MissingKey)?;
            let (root, receiving) =
                root_step(self.crypto, &session.root, &private, &message.ratchet)?;
            let fresh = self.generate_key_pair()?;
            let (next_root, sending) =
                root_step(self.crypto, &root, &fresh.private, &message.ratchet)?;
            session.previous_counter = old.index.saturating_sub(1);
            session.root = next_root;
            session.sender = Some(Chain::new(fresh.public, Some(fresh.private), sending));
            session
                .receivers
                .push(Chain::new(message.ratchet, None, receiving));
            if session.receivers.len() > MAX_RECEIVER_CHAINS {
                session.receivers.remove(0);
            }
            ratcheted = true;
            session.receivers.len() - 1
        };
        let limit = if session.local_identity == session.remote_identity {
            SELF_FUTURE_LIMIT
        } else {
            PEER_FUTURE_LIMIT
        };
        let advance = session.receivers[index].plan(self.crypto, message.counter, limit)?;
        let (cipher, mac, iv) = advance.message.expanded(self.crypto)?;
        let mut authenticated = Vec::with_capacity(input.len() + 66);
        authenticated.extend_from_slice(&wire::tagged_key(&session.remote_identity));
        authenticated.extend_from_slice(&wire::tagged_key(&session.local_identity));
        authenticated.extend_from_slice(&input[..input.len() - 8]);
        authenticate(
            &self.crypto.hmac_sha256(&mac, &authenticated)[..8],
            &message.mac,
        )?;
        let plaintext = self.crypto.decrypt_cbc(&cipher, &iv, &message.ciphertext)?;
        advance.commit(&mut session.receivers[index]);
        session.pending_pre_key = None;
        Ok((plaintext, ratcheted))
    }
}
