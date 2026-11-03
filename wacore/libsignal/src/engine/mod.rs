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
        let mut private = [0; 32];
        self.random.fill(&mut private)?;
        private[0] &= 248;
        private[31] = (private[31] & 127) | 64;
        Ok(KeyPair {
            public: self.crypto.public_key(&private)?,
            private,
        })
    }
}

#[cfg(test)]
mod tests;
