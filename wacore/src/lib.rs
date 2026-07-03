extern crate self as wacore;

pub use wacore_appstate as appstate;
// time::* returns chrono types; re-exported so consumers do not pin their own.
pub use chrono;
pub use wacore_noise as noise;

// Re-export derive macros
pub use wacore_derive::{EmptyNode, ProtocolNode, WireEnum};

pub mod adv;
pub mod appstate_sync;
pub mod bot_message;
pub mod client;
pub mod client_profile;
pub mod companion_reg;
pub mod download;
pub mod iq;
pub mod protocol;
pub use wacore_noise::framing;
pub mod handshake;
pub mod history_sync;
pub mod ib;
pub use wacore_libsignal as libsignal;
pub mod comment;
pub mod event;
pub mod media_retry;
pub mod message_edit;
pub mod message_processing;
pub mod messages;
pub mod msg_secret;
pub mod net;
pub mod pair;
pub mod pair_code;
pub mod poll;
pub mod prekeys;
pub mod proto_helpers;
pub mod reaction;
pub mod reporting_token;
pub mod request;
pub mod runtime;
pub mod secret_enc_addon;
pub mod send;
pub mod session;
pub mod shortcake;
pub mod stanza;
pub mod stats;
pub mod sticker_pack;

pub mod store;
pub mod sync_marker;
pub mod telemetry;
pub mod time;
pub mod types;
pub mod upload;
pub mod usync;
#[cfg(feature = "voip")]
pub mod voip;
pub mod webp;

pub mod version;
pub mod xml;
mod zip;
