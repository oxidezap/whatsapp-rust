//! External public-API contracts. Each profile compiles separately without defaults.

pub mod whatspec_compat;

#[cfg(feature = "core")]
pub mod derive_contract;

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
pub mod client_options;

#[cfg(feature = "sdk")]
pub mod encapsulation;

#[cfg(feature = "sdk")]
pub mod events;

#[cfg(feature = "sdk")]
pub mod creation;

#[cfg(feature = "sdk")]
pub mod download;

#[cfg(feature = "sdk")]
pub mod mex;

#[cfg(feature = "sdk")]
pub mod pictures;

#[cfg(feature = "sdk")]
pub mod requests;

#[cfg(feature = "voip-control")]
pub mod voip_control;

#[cfg(feature = "sdk")]
pub mod groups_lookup;

#[cfg(feature = "sdk")]
pub mod lifecycle;

#[cfg(feature = "core")]
pub mod groups;

#[cfg(feature = "voip-control")]
pub mod peer_video;

#[cfg(feature = "sdk")]
pub mod media_cache;

#[cfg(feature = "sdk")]
pub mod actions;

#[cfg(feature = "sdk")]
pub mod community;

#[cfg(feature = "sdk")]
pub mod status;
