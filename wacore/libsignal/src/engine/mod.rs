// SPDX-License-Identifier: MIT
//! Synchronous, storage-independent Signal session transitions.
//!
//! Protocol references: <https://signal.org/docs/specifications/x3dh/>,
//! <https://signal.org/docs/specifications/doubleratchet/> and
//! <https://signal.org/docs/specifications/xeddsa/>. Storage field numbers are
//! the data schema in `waproto/signal-storage.snapshot`.
//! Callers serialize access and persist returned state before publishing ciphertext.
#![forbid(unsafe_code)]

pub mod codec;
pub mod crypto;
mod encode;
pub mod pairwise;
pub mod rustcrypto;
pub use rustcrypto::RustCrypto;
pub mod sender;
pub mod state;
pub mod wire;
pub use crypto::{CryptoProvider, RandomSource};
pub use pairwise::{InitiatorParameters, ResponderParameters};
pub use state::*;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("invalid encoding: {0}")]
    Encoding(&'static str),
    #[error("unsupported protocol version")]
    Version,
    #[error("cryptographic operation failed")]
    Crypto,
    #[error("authentication failed")]
    Authentication,
    #[error("invalid ciphertext framing or padding")]
    InvalidCiphertext,
    #[error("message key already consumed or unavailable")]
    Duplicate,
    #[error("receiver chain is closed")]
    ClosedChain,
    #[error("message counter exceeds the permitted future gap")]
    FutureMessage,
    #[error("counter exhausted")]
    CounterExhausted,
    #[error("required session or key is missing")]
    MissingKey,
    #[error("identity key changed")]
    IdentityChanged,
    #[error("durable counter reservation is required")]
    ReservationRequired,
}
pub type Result<T> = core::result::Result<T, Error>;

/// No clock, storage, network, or process randomness is accessed by the engine.
pub struct Engine<'a, C: CryptoProvider, R: RandomSource> {
    pub crypto: &'a C,
    pub random: &'a mut R,
}
impl<'a, C: CryptoProvider, R: RandomSource> Engine<'a, C, R> {
    pub fn new(crypto: &'a C, random: &'a mut R) -> Self {
        Self { crypto, random }
    }
    pub fn generate_key_pair(&mut self) -> Result<KeyPair> {
        let private = self.generate_private_key()?;
        Ok(KeyPair {
            public: self.crypto.public_key(&private)?,
            private,
        })
    }
    /// Generate a ratchet key pair and run the root KDF step with its
    /// agreement against `remote`. Returns the key pair, next root and chain.
    pub(crate) fn fresh_root_step(
        &mut self,
        root: &[u8; 32],
        remote: &[u8; 32],
    ) -> Result<(KeyPair, [u8; 32], [u8; 32])> {
        let private = self.generate_private_key()?;
        let (public, shared) = self.crypto.public_key_and_agreement(&private, remote)?;
        let fresh = KeyPair { public, private };
        let shared = zeroize::Zeroizing::new(shared);
        let (next_root, chain) = crypto::root_step_from_shared(self.crypto, root, &shared)?;
        Ok((fresh, next_root, chain))
    }
    fn generate_private_key(&mut self) -> Result<[u8; 32]> {
        let mut private = [0; 32];
        self.random.fill(&mut private)?;
        private[0] &= 248;
        private[31] = (private[31] & 127) | 64;
        Ok(private)
    }
}

#[cfg(test)]
mod tests;
