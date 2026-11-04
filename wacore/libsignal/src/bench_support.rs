// SPDX-License-Identifier: MIT
//! Feature-gated measurement ports for operations no longer in the public adapter API.
use crate::{
    engine::{self, Error, RustCrypto},
    protocol::{PublicKey, SessionState},
};

/// Consume an already cached key through the engine's actual plan/commit path.
pub fn take_cached_key(
    state: &mut SessionState,
    ratchet: &PublicKey,
    index: u32,
) -> Result<bool, Error> {
    let chain = state
        .inner
        .receivers
        .iter_mut()
        .find(|chain| chain.ratchet.as_slice() == ratchet.public_key_bytes())
        .ok_or(Error::MissingKey)?;
    if index >= chain.index {
        return Ok(false);
    }
    let plan = match chain.plan(&RustCrypto, index, 0) {
        Ok(plan) => plan,
        Err(Error::Duplicate) => return Ok(false),
        Err(error) => return Err(error),
    };
    std::hint::black_box(&plan.message);
    plan.commit(chain);
    Ok(true)
}

/// Exercise the same bounded insertion used by committed receive transitions.
pub fn insert_cached_key(
    state: &mut SessionState,
    ratchet: &PublicKey,
    index: u32,
    seed: [u8; 32],
) -> Result<(), Error> {
    let chain = state
        .inner
        .receivers
        .iter_mut()
        .find(|chain| chain.ratchet.as_slice() == ratchet.public_key_bytes())
        .ok_or(Error::MissingKey)?;
    chain.push_skipped(engine::MessageKey::Seed { index, seed });
    Ok(())
}
