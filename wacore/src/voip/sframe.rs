//! SFrame key derivation and internal media session implementation.
//!
//! Key derivation helpers remain public. Session internals are exposed only with
//! `bench-internals` for repository benchmarks.

mod implementation;

pub use implementation::{
    KDF_LABEL_E2E_SFRAME, derive_e2e_sframe_key_for_participant, format_sframe_participant_id,
    sframe_info_label,
};

#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub use implementation::{SframeIn, SframeSession};

#[cfg(not(feature = "bench-internals"))]
pub(crate) use implementation::{SframeIn, SframeSession};
