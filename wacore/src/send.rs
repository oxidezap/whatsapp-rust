use crate::client::context::{GroupInfo, SendContextResolver};
use crate::libsignal::protocol::{
    CiphertextMessage, IdentityChange, ProtocolAddress, SENDERKEY_MESSAGE_CURRENT_VERSION,
    SenderKeyMessage, SenderKeyStore, SignalProtocolError, UsePQRatchet, message_encrypt,
    process_prekey_bundle,
};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::messages::MessageUtils;
use crate::reporting_token::{
    build_reporting_node, generate_reporting_token, prepare_message_with_context,
};
use crate::runtime::{AbortHandle, Runtime};
use crate::types::jid::JidExt;
use crate::types::jid::make_sender_key_name;
use crate::types::message::PeerMessageOptions;
use anyhow::{Result, anyhow, bail};
use futures::stream::{FuturesUnordered, StreamExt};
use prost::Message as ProtoMessage;
use rand::{CryptoRng, Rng};
use std::collections::HashSet;
use std::future::Future;
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, JidExt as _};
use wacore_libsignal::crypto::aes_256_cbc_encrypt_into;
use waproto::whatsapp as wa;

/// Wire-format constants (MsgCreateDeviceStanza.js).
pub(crate) mod stanza {
    pub const ENC_VERSION: &str = "2";
    pub const MSG_TYPE_TEXT: &str = "text";
    pub const MSG_TYPE_MEDIA: &str = "media";
    pub const MSG_TYPE_REACTION: &str = "reaction";
    pub const MSG_TYPE_POLL: &str = "poll";
    pub const MSG_TYPE_EVENT: &str = "event";
    pub const ENC_TYPE_MSG: &str = "msg";
    pub const ENC_TYPE_PKMSG: &str = "pkmsg";
    pub const ENC_TYPE_SKMSG: &str = "skmsg";
    pub const MSG_TYPE_PAY: &str = "pay";
}

/// Type-safe `<message type="...">` value for the send-time override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StanzaType {
    Text,
    Media,
    Reaction,
    Poll,
    Event,
    Pay,
}

impl StanzaType {
    pub fn as_wire(self) -> &'static str {
        match self {
            Self::Text => stanza::MSG_TYPE_TEXT,
            Self::Media => stanza::MSG_TYPE_MEDIA,
            Self::Reaction => stanza::MSG_TYPE_REACTION,
            Self::Poll => stanza::MSG_TYPE_POLL,
            Self::Event => stanza::MSG_TYPE_EVENT,
            Self::Pay => stanza::MSG_TYPE_PAY,
        }
    }
}

mod classify;
mod dm;
mod encrypt;
mod group;
mod peer;
mod status;

pub use classify::*;
#[cfg(test)]
pub(crate) use dm::partition_dm_devices;
pub(crate) use dm::pkmsg_would_be_emitted;
pub use dm::*;
pub use encrypt::*;
pub use group::*;
pub use peer::*;
pub use status::*;

#[cfg(test)]
mod tests;
