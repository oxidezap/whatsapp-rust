//! Run the pre-rewrite behavioral contracts against the production adapter.
//! Project-authored fixtures are shared with the unpublished legacy oracle.
extern crate wacore_signal_runtime as wacore_libsignal;
#[path = "fixtures/counter_lease.rs"]
mod counter_lease;
#[path = "fixtures/session_divergence.rs"]
mod session_divergence;

#[path = "fixtures/support/signed_prekey.rs"]
mod constructed_signed_prekey;
