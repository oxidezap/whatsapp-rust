// SPDX-License-Identifier: MIT
//! Sender-key group chains with signed ciphertext and bounded skipped seeds.
use crate::engine::{
    CryptoProvider, Engine, Error, RandomSource, Result,
    state::*,
    wire::{SenderKeyDistribution, SenderKeyMessage, SenderKeyMessageView},
};
use zeroize::Zeroize;
impl SenderKeyState {
    fn step<C: CryptoProvider>(&mut self, crypto: &C) -> Result<SenderMessageKey> {
        let next = self
            .iteration
            .checked_add(1)
            .ok_or(Error::CounterExhausted)?;
        let seed = crypto.hmac_sha256(&self.chain_key, &[1]);
        let key = SenderMessageKey {
            iteration: self.iteration,
            seed,
        };
        self.chain_key = crypto.hmac_sha256(&self.chain_key, &[2]);
        self.iteration = next;
        Ok(key)
    }
    fn take<C: CryptoProvider>(&mut self, crypto: &C, iteration: u32) -> Result<SenderMessageKey> {
        if iteration < self.iteration {
            return self
                .skipped
                .iter()
                .position(|key| key.iteration == iteration)
                .and_then(|i| self.skipped.remove(i))
                .ok_or(Error::Duplicate);
        }
        if iteration - self.iteration > PEER_FUTURE_LIMIT {
            return Err(Error::FutureMessage);
        }
        while self.iteration < iteration {
            let key = self.step(crypto)?;
            self.skipped.push_back(key);
            if self.skipped.len() > MAX_SKIPPED {
                self.skipped.pop_front();
            }
        }
        self.step(crypto)
    }
}
fn expand<C: CryptoProvider>(crypto: &C, seed: &[u8; 32]) -> Result<([u8; 32], [u8; 16])> {
    let mut expanded = [0; 48];
    crypto.hkdf_sha256(&[0; 32], seed, b"WhisperGroup", &mut expanded)?;
    let mut iv = [0; 16];
    let mut cipher = [0; 32];
    iv.copy_from_slice(&expanded[..16]);
    cipher.copy_from_slice(&expanded[16..]);
    expanded.zeroize();
    Ok((cipher, iv))
}
impl<C: CryptoProvider, R: RandomSource> Engine<'_, C, R> {
    /// Creates a new sender state only when no sending state already exists.
    pub fn create_sender_distribution(&mut self, record: &mut SenderKeyRecord) -> Result<Vec<u8>> {
        if record
            .states
            .first()
            .is_none_or(|state| state.signing_private.is_none())
        {
            let signing = self.generate_key_pair()?;
            let mut key = [0; 32];
            self.random.fill(&mut key)?;
            let mut id = [0; 4];
            self.random.fill(&mut id)?;
            let state = SenderKeyState {
                id: u32::from_le_bytes(id) & 0x7fff_ffff,
                iteration: 0,
                chain_key: key,
                signing_public: signing.public,
                signing_private: Some(signing.private),
                skipped: SharedDeque::new(),
            };
            record.states.insert(0, state);
            record.states.truncate(MAX_SENDER_STATES);
            record.reservation.reset_chain();
        }
        let state = record.states.first().ok_or(Error::MissingKey)?;
        Ok(SenderKeyDistribution {
            id: state.id,
            iteration: state.iteration,
            chain_key: state.chain_key,
            signing_key: state.signing_public,
        }
        .encode())
    }
    pub fn process_sender_distribution(
        &mut self,
        record: &mut SenderKeyRecord,
        input: &[u8],
    ) -> Result<()> {
        let distribution = SenderKeyDistribution::decode(input)?;
        // Preserve this client's established distribution policy: a repeated
        // authenticated signing identity cannot discard its consumed position
        // or skipped keys, even when the advertised iteration is newer.
        let retained = record
            .states
            .iter()
            .position(|state| {
                state.id == distribution.id && state.signing_public == distribution.signing_key
            })
            .map(|index| record.states.remove(index));
        record.states.retain(|state| state.id != distribution.id);
        let state = retained.unwrap_or_else(|| SenderKeyState {
            id: distribution.id,
            iteration: distribution.iteration,
            chain_key: distribution.chain_key,
            signing_public: distribution.signing_key,
            signing_private: None,
            skipped: SharedDeque::new(),
        });
        record.states.insert(0, state);
        record.states.truncate(MAX_SENDER_STATES);
        record.reservation.reset_chain();
        Ok(())
    }
    /// Prepared output must be published only after the caller persists the state.
    pub fn encrypt_sender_key(
        &mut self,
        record: &mut SenderKeyRecord,
        plaintext: &[u8],
    ) -> Result<Vec<u8>> {
        let mut state = record.states.first().ok_or(Error::MissingKey)?.clone();
        if !record.reservation.permits(state.iteration) {
            return Err(Error::ReservationRequired);
        }
        let private = state.signing_private.ok_or(Error::MissingKey)?;
        let key = state.step(self.crypto)?;
        let (cipher, iv) = expand(self.crypto, &key.seed)?;
        let mut message = SenderKeyMessage {
            id: state.id,
            iteration: key.iteration,
            ciphertext: self.crypto.encrypt_cbc(&cipher, &iv, plaintext)?,
            signature: [0; 64],
        };
        message.signature = self
            .crypto
            .sign(&private, &message.unsigned(), self.random)?;
        record.states[0] = state;
        Ok(message.encode())
    }
    pub fn decrypt_sender_key(
        &mut self,
        record: &mut SenderKeyRecord,
        input: &[u8],
    ) -> Result<Vec<u8>> {
        let message = SenderKeyMessageView::decode(input)?;
        let index = record
            .states
            .iter()
            .position(|state| state.id == message.id)
            .ok_or(Error::MissingKey)?;
        let mut state = record.states[index].clone();
        self.crypto.verify(
            &state.signing_public,
            &input[..input.len() - 64],
            &message.signature,
        )?;
        let key = state.take(self.crypto, message.iteration)?;
        let (cipher, iv) = expand(self.crypto, &key.seed)?;
        let plaintext = self.crypto.decrypt_cbc(&cipher, &iv, message.ciphertext)?;
        record.states[index] = state;
        Ok(plaintext)
    }
}
