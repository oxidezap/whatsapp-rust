// SPDX-License-Identifier: MIT
//! X3DH and the v3 Double Ratchet profile. Parameters are explicit; the caller
//! resolves prekey IDs and applies its identity trust policy before initialization.
use crate::engine::{
    CryptoProvider, Engine, Error, RandomSource, Result,
    crypto::{authenticate, root_step},
    state::*,
    wire::{self, PreKeySignalMessage, PreKeySignalMessageView, SignalMessage, SignalMessageView},
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
    let result = crypto.hkdf_sha256_zero_salt(&input, b"WhisperText", &mut expanded);
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
        metadata: std::sync::Arc::new(SessionMetadata {
            version: 3,
            local_identity: local.public,
            remote_identity: remote,
            root,
            previous_counter: 0,
            local_registration,
            remote_registration,
            needs_refresh: false,
            alice_base_key: None,
            alice_base_key_tagged: true,
            pending_key_exchange: None,
        }),
        sender: None,
        receivers: Vec::new(),
        pending_pre_key: None,
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
        session.pending_pre_key = Some(std::sync::Arc::new(PendingPreKey {
            pre_key_id: p.remote_one_time_pre_key.as_ref().map(|(id, _)| *id),
            signed_pre_key_id: Some(p.remote_signed_pre_key_id as i32),
            base_key: p.base_key.public,
            kyber_pre_key_id: None,
            kyber_ciphertext: None,
        }));
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
        if !matches!(session.version, 3 | 4) {
            return Err(Error::Version);
        }
        let mut sender = session.sender.as_ref().ok_or(Error::MissingKey)?.clone();
        if !record.reservation.permits(sender.index) {
            return Err(Error::ReservationRequired);
        }
        let key = sender.step(self.crypto)?;
        let (cipher, mac, iv) = key.expanded(self.crypto)?;
        let ciphertext_len = super::crypto::cbc_ciphertext_len(plaintext.len())?;
        let mut output = SignalMessage::prefix(
            session.version as u8,
            &sender.ratchet,
            key.index(),
            session.previous_counter,
            ciphertext_len,
        );
        let prefix_len = output.len();
        self.crypto
            .encrypt_cbc_into(&cipher, &iv, plaintext, &mut output)?;
        if output.len().checked_sub(prefix_len) != Some(ciphertext_len) {
            return Err(Error::Crypto);
        }
        let authentication = self.crypto.hmac_sha256(
            &mac,
            &[
                &wire::tagged_key(&session.local_identity),
                &wire::tagged_key(&session.remote_identity),
                &output,
            ],
        )?;
        output.extend_from_slice(&authentication[..8]);
        // Match the decoder budget: exclude the version byte and trailing MAC.
        if output.len() - 9 > wire::MAX_MESSAGE_SIZE {
            return Err(Error::Encoding("protobuf size limit"));
        }
        if let Some(pending) = &session.pending_pre_key {
            if pending.kyber_pre_key_id.is_some() || pending.kyber_ciphertext.is_some() {
                return Err(Error::Version);
            }
            output = PreKeySignalMessage {
                registration: session.local_registration,
                pre_key_id: pending.pre_key_id,
                signed_pre_key_id: pending.signed_pre_key_id.ok_or(Error::MissingKey)? as u32,
                base_key: pending.base_key,
                identity: session.local_identity,
                message: output,
            }
            .encode_with_version(session.version as u8);
            if output.len() - 1 > wire::MAX_MESSAGE_SIZE {
                return Err(Error::Encoding("protobuf size limit"));
            }
        }
        record.current.as_mut().ok_or(Error::MissingKey)?.sender = Some(sender);
        Ok(output)
    }
    pub fn decrypt(&mut self, record: &mut SessionRecord, input: &[u8]) -> Result<Vec<u8>> {
        self.decrypt_selected(record, input, None)
    }
    pub(crate) fn decrypt_existing_pre_key(
        &mut self,
        record: &mut SessionRecord,
        input: &[u8],
        version: u32,
        base_key: [u8; 32],
    ) -> Result<Vec<u8>> {
        self.decrypt_selected(record, input, Some((version, base_key)))
    }
    fn decrypt_selected(
        &mut self,
        record: &mut SessionRecord,
        input: &[u8],
        selection: Option<(u32, [u8; 32])>,
    ) -> Result<Vec<u8>> {
        let message = SignalMessageView::decode(input)?;
        let matches = |state: &SessionState| {
            selection.is_none_or(|(version, base)| {
                state.version == version && state.alice_base_key == Some(base)
            })
        };
        let mut failure = Error::MissingKey;
        if let Some(current) = record.current.as_mut()
            && matches(current)
        {
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
                            record.reservation.rebase();
                        }
                        return Ok(plaintext);
                    }
                    Err(error) => failure = error,
                }
            }
            if selection.is_some() {
                return Err(failure);
            }
        }
        for index in 0..record.previous.len() {
            let mut candidate = record.previous[index].decode(self.crypto)?;
            if !matches(&candidate) {
                continue;
            }
            match self.decrypt_state(&mut candidate, &message, input) {
                Ok((plaintext, _)) => {
                    record.materialize_reservation(self.crypto)?;
                    record.previous.remove(index);
                    record.install(candidate);
                    return Ok(plaintext);
                }
                Err(error) => {
                    // A prekey identifies one session. Searching stale copies
                    // after its failure could accept an already consumed packet.
                    if selection.is_some() {
                        return Err(error);
                    }
                    if error == Error::Duplicate || failure == Error::MissingKey {
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
        message: &SignalMessageView<'_>,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        if !matches!(session.version, 3 | 4) || session.version != u32::from(message.bytes[0] >> 4)
        {
            return Err(Error::Version);
        }
        let limit = if session.local_identity == session.remote_identity {
            SELF_FUTURE_LIMIT
        } else {
            PEER_FUTURE_LIMIT
        };
        let plan = session.receivers[index].plan(self.crypto, message.counter, limit)?;
        let (cipher, mac, iv) = plan.message.expanded(self.crypto)?;
        authenticate(
            &self.crypto.hmac_sha256(
                &mac,
                &[
                    &wire::tagged_key(&session.remote_identity),
                    &wire::tagged_key(&session.local_identity),
                    &input[..input.len() - 8],
                ],
            )?[..8],
            &message.mac,
        )?;
        let plaintext = self.crypto.decrypt_cbc(&cipher, &iv, message.ciphertext)?;
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
        let wrapper = PreKeySignalMessageView::decode(input)?;
        if wrapper.identity != p.remote_identity
            || wrapper.base_key != p.remote_base_key
            || wrapper.registration != p.remote_registration
        {
            return Err(Error::IdentityChanged);
        }
        let mut existing = record.current.as_ref().is_some_and(|s| {
            s.version == u32::from(wrapper.bytes[0] >> 4)
                && s.alice_base_key == Some(wrapper.base_key)
                && s.remote_identity == wrapper.identity
        });
        if !existing {
            for archive in &record.previous {
                let s = archive.decode(self.crypto)?;
                if s.version == u32::from(wrapper.bytes[0] >> 4)
                    && s.alice_base_key == Some(wrapper.base_key)
                    && s.remote_identity == wrapper.identity
                {
                    existing = true;
                    break;
                }
            }
        }
        if existing {
            return self.decrypt_existing_pre_key(
                record,
                wrapper.message,
                u32::from(wrapper.bytes[0] >> 4),
                wrapper.base_key,
            );
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
        candidate.materialize_reservation(self.crypto)?;
        let mut state = self.initialize_responder(p)?;
        state.version = u32::from(wrapper.bytes[0] >> 4);
        candidate.install(state);
        let plaintext = self.decrypt_existing_pre_key(
            &mut candidate,
            wrapper.message,
            u32::from(wrapper.bytes[0] >> 4),
            wrapper.base_key,
        )?;
        *record = candidate;
        Ok(plaintext)
    }
    fn decrypt_state(
        &mut self,
        session: &mut SessionState,
        message: &SignalMessageView<'_>,
        input: &[u8],
    ) -> Result<(Vec<u8>, bool)> {
        if !matches!(session.version, 3 | 4) || session.version != u32::from(message.bytes[0] >> 4)
        {
            return Err(Error::Version);
        }
        let (index, receiving_root) = if let Some(index) = session
            .receivers
            .iter()
            .position(|c| c.ratchet == message.ratchet)
        {
            (index, None)
        } else {
            let old = session.sender.as_ref().ok_or(Error::MissingKey)?;
            let private = old.private.ok_or(Error::MissingKey)?;
            let (root, receiving) =
                root_step(self.crypto, &session.root, &private, &message.ratchet)?;
            session
                .receivers
                .push(Chain::new(message.ratchet, None, receiving));
            if session.receivers.len() > MAX_RECEIVER_CHAINS {
                session.receivers.remove(0);
            }
            (
                session.receivers.len() - 1,
                Some(zeroize::Zeroizing::new(root)),
            )
        };
        let limit = if session.local_identity == session.remote_identity {
            SELF_FUTURE_LIMIT
        } else {
            PEER_FUTURE_LIMIT
        };
        let advance = session.receivers[index].plan(self.crypto, message.counter, limit)?;
        let (cipher, mac, iv) = advance.message.expanded(self.crypto)?;
        authenticate(
            &self.crypto.hmac_sha256(
                &mac,
                &[
                    &wire::tagged_key(&session.remote_identity),
                    &wire::tagged_key(&session.local_identity),
                    &input[..input.len() - 8],
                ],
            )?[..8],
            &message.mac,
        )?;
        let ratcheted = receiving_root.is_some();
        if let Some(root) = receiving_root {
            // Failed archive candidates do not need a fresh reply key or a
            // second DH calculation. Authenticate before preparing either.
            let fresh = self.generate_key_pair()?;
            let (next_root, sending) =
                root_step(self.crypto, &root, &fresh.private, &message.ratchet)?;
            session.previous_counter = session
                .sender
                .as_ref()
                .ok_or(Error::MissingKey)?
                .index
                .saturating_sub(1);
            session.root = next_root;
            session.sender = Some(Chain::new(fresh.public, Some(fresh.private), sending));
        }
        let plaintext = self.crypto.decrypt_cbc(&cipher, &iv, message.ciphertext)?;
        advance.commit(&mut session.receivers[index]);
        session.pending_pre_key = None;
        Ok((plaintext, ratcheted))
    }
}
