//! What the sync store hooks promise, and what happens when a store declines
//! them.
//!
//! `#[async_trait]` turns every store method into a `Pin<Box<dyn Future>>`, so a
//! store that already knows the answer still allocates to hand it back. The
//! hooks exist to skip that box on the hot path, which only holds if the
//! resolver actually consults them, and only stays safe if declining the hook
//! falls through to the async answer rather than to a default.

use super::traits::*;
use crate::protocol::{IdentityKey, IdentityKeyPair, ProtocolAddress, SignalProtocolError};
use futures::executor::block_on;

fn test_address() -> ProtocolAddress {
    ProtocolAddress::new("5511987650001@s.whatsapp.net".to_string(), 1.into())
}

fn test_identity() -> IdentityKey {
    *IdentityKeyPair::generate(&mut rand::rng()).identity_key()
}

/// A store that answers both hooks synchronously, like the real adapter does on
/// a cache hit.
struct HookedStore;

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl IdentityKeyStore for HookedStore {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
        unimplemented!("not reached: the hooks answer first")
    }
    async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
        unimplemented!("not reached: the hooks answer first")
    }
    async fn save_identity(
        &mut self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
    ) -> Result<IdentityChange, SignalProtocolError> {
        panic!("the async path must not run when the hook answers")
    }
    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Result<bool, SignalProtocolError> {
        panic!("the async path must not run when the hook answers")
    }
    async fn get_identity(
        &self,
        _address: &ProtocolAddress,
    ) -> Result<Option<IdentityKey>, SignalProtocolError> {
        Ok(None)
    }
    fn try_is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Option<Result<bool, SignalProtocolError>> {
        Some(Ok(true))
    }
    fn try_save_identity(
        &mut self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
    ) -> Option<Result<IdentityChange, SignalProtocolError>> {
        Some(Ok(IdentityChange::NewOrUnchanged))
    }
}

/// A store that implements neither hook: the default must decline, and the
/// resolver must fall through to the async method instead of inventing an
/// answer. `is_trusted_identity` returning false is the case that matters, since
/// a resolver that defaulted to "trusted" would be a security hole rather than a
/// performance bug.
struct PlainStore {
    trusted: bool,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl IdentityKeyStore for PlainStore {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
        unimplemented!("not exercised")
    }
    async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
        unimplemented!("not exercised")
    }
    async fn save_identity(
        &mut self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
    ) -> Result<IdentityChange, SignalProtocolError> {
        Ok(IdentityChange::ReplacedExisting)
    }
    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity: &IdentityKey,
        _direction: Direction,
    ) -> Result<bool, SignalProtocolError> {
        Ok(self.trusted)
    }
    async fn get_identity(
        &self,
        _address: &ProtocolAddress,
    ) -> Result<Option<IdentityKey>, SignalProtocolError> {
        Ok(None)
    }
}

#[test]
fn a_store_without_hooks_falls_through_to_its_async_answer() {
    let mut store = PlainStore { trusted: false };
    let address = test_address();
    let identity = test_identity();

    assert!(
        store
            .try_is_trusted_identity(&address, &identity, Direction::Sending)
            .is_none(),
        "the default hook must decline rather than guess"
    );

    let trusted = block_on(is_trusted_identity(
        &store,
        &address,
        &identity,
        Direction::Sending,
    ))
    .expect("resolves");
    assert!(
        !trusted,
        "declining the hook must surface the store's real answer, not a default"
    );

    let change = block_on(save_identity(&mut store, &address, &identity)).expect("resolves");
    assert!(matches!(change, IdentityChange::ReplacedExisting));
}

#[test]
fn the_hook_result_is_what_the_caller_sees() {
    let mut store = HookedStore;
    let address = test_address();
    let identity = test_identity();

    assert!(
        block_on(is_trusted_identity(
            &store,
            &address,
            &identity,
            Direction::Receiving
        ))
        .expect("resolves"),
        "the hook's answer must reach the caller unchanged"
    );
    // Both stores panic in their async bodies, so reaching here at all proves
    // the async path was never taken.
    assert!(matches!(
        block_on(save_identity(&mut store, &address, &identity)).expect("resolves"),
        IdentityChange::NewOrUnchanged
    ));
}
