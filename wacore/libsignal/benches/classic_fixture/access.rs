// SPDX-License-Identifier: MIT
//! The comparison driver replaces only this API port for the immutable baseline.
use wacore_libsignal::protocol::{
    PublicKey, SenderKeyCryptoContext, SenderKeyRecord, SessionState,
};
pub(super) type CryptoContext = SenderKeyCryptoContext;
pub(super) fn capture_crypto(record: &mut SenderKeyRecord) -> CryptoContext {
    record.prepare_crypto_context()
}
pub(super) fn restore_crypto(record: &mut SenderKeyRecord, context: &CryptoContext) {
    record.restore_crypto_context(context);
}
pub(super) fn take(state: &mut SessionState, ratchet: &PublicKey, index: u32) -> bool {
    wacore_libsignal::bench_support::take_cached_key(state, ratchet, index).unwrap()
}
pub(super) fn insert(state: &mut SessionState, ratchet: &PublicKey, index: u32, seed: [u8; 32]) {
    wacore_libsignal::bench_support::insert_cached_key(state, ratchet, index, seed).unwrap();
}
pub(super) fn same_identity(state: &SessionState) -> bool {
    state.remote_identity_key().unwrap().as_ref() == Some(&state.local_identity_key().unwrap())
}
