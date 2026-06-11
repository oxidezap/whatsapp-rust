// Compile-checks the README examples as doctests, so the advertised quick
// start can never silently rot.
#![doc = include_str!("../README.md")]
// Instrumenting large async fns (e.g. process_sync_task) wraps them in deep
// `Instrumented` future types; the default depth limit overflows when the
// `tracing` + `tracing-pii` paths combine. Raise it (compile-time only).
#![recursion_limit = "512"]

pub use wacore::appstate::schemas;
pub use wacore::client_profile::ClientProfile;
/// Optional metrics emission (the `metrics` feature). No-op when the feature is off.
pub use wacore::telemetry;
pub use wacore::{
    iq::privacy as privacy_settings, proto_helpers, sticker_pack, store::traits, webp,
};
pub use wacore_binary::CompactString;
pub use wacore_binary::OwnedNodeRef;
pub use wacore_binary::builder::NodeBuilder;
pub use wacore_binary::{Jid, Server};

// Whole-crate re-exports so a git consumer needs a single dependency:
// every `wacore::…`/`wacore_binary::…`/`waproto::…` path is reachable as
// `whatsapp_rust::wacore::…` (etc.) without declaring the sibling crates.
pub use wacore;
pub use wacore_binary;
pub use waproto;

pub mod cache;
// Available whenever the cache falls back to PortableCache: moka off, or wasm32
// (where moka can't build) even if `moka-cache` is enabled. Mirrors src/cache.rs.
#[cfg(any(not(feature = "moka-cache"), target_arch = "wasm32"))]
pub mod portable_cache;

pub mod cache_config;
pub use cache_config::{
    CacheConfig, CacheEntryConfig, CacheStores, MsgSecretPolicy, MsgSecretRetention,
    OriginalMessageResolver,
};
pub mod cache_store;
pub(crate) mod pending_device_sync;
pub(crate) mod sender_key_device_cache;
pub use cache_store::CacheStore;
pub mod http;
pub mod types;

pub mod client;
pub(crate) mod flush_scope;
pub use client::Client;
#[cfg(feature = "debug-diagnostics")]
pub use client::MemoryDiagnostics;
pub use client::NodeFilter;
pub mod download;
pub mod handlers;
pub use handlers::chatstate::ChatStateEvent;
pub mod handshake;
pub mod jid_utils;
pub mod keepalive;
pub mod mediaconn;
pub mod message;
pub(crate) mod msg_secret_buffer;
pub mod pair;
pub mod pair_code;
pub mod request;
#[cfg(feature = "tokio-runtime")]
pub mod runtime_impl;
#[cfg(feature = "tokio-runtime")]
pub use runtime_impl::TokioRuntime;
pub use wacore::runtime::Runtime;
pub mod send;
pub use send::{PinDuration, RevokeType, SendOptions, SendResult};
pub use wacore::send::StanzaType;
pub mod media;
pub mod session;
pub mod socket;
pub mod store;
pub mod transport;
pub mod upload;
pub use upload::UploadOptions;

pub mod pdo;
pub mod prekeys;
pub mod receipt;
pub mod retry;
pub mod unified_session;

pub mod appstate_sync;
pub mod history_sync;
pub mod usync;

pub mod features;
pub use features::{
    BatchGroupResult, Blocking, BlocklistEntry, ChatActions, ChatStateType, Chatstate, Comments,
    Community, CommunitySubgroup, Contacts, CreateCommunityOptions, CreateCommunityResult,
    CreateGroupResult, EncryptedEdit, EventCreationParams, EventResponseType, Events,
    GroupCreateOptions, GroupDescription, GroupJoinError, GroupMetadata, GroupParticipant,
    GroupParticipantOptions, GroupProfilePicture, GroupSubject, GroupType, Groups, GrowthLockInfo,
    InviteInfoError, IsOnWhatsAppResult, JoinGroupResult, Labels, LinkSubgroupsResult,
    MediaRetryResult, MediaReupload, MediaReuploadRequest, MemberAddMode, MemberLinkMode,
    MemberShareHistoryMode, MembershipApprovalMode, MembershipRequest, Mex, MexError,
    MexErrorExtensions, MexRequest, MexResponse, Newsletter, NewsletterMessage,
    NewsletterMessageType, NewsletterMetadata, NewsletterReactionCount, NewsletterRole,
    NewsletterState, NewsletterVerification, ParticipantChangeResponse, ParticipantType,
    PictureType, Presence, PresenceError, PresenceStatus, Profile, ProfilePicture, SecretEncKind,
    SecretEncrypted, SetProfilePictureResponse, Signal, Status, StatusPrivacySetting,
    StatusSendOptions, SyncActionMessageRange, TcToken, UnlinkSubgroupsResult, UserInfo,
    UsyncSubprotocolError, VerifiedName, group_type, message_key, message_range,
};

pub mod bot;
pub mod lid_pn_cache;
pub mod spam_report;
pub mod sync_task;
pub mod version;

/// One-import surface for the common bot path:
/// `use whatsapp_rust::prelude::*;`.
pub mod prelude {
    pub use crate::bot::{Bot, BotBuilder, BotHandle, MessageContext};
    pub use crate::client::Client;
    #[cfg(feature = "tokio-runtime")]
    pub use crate::runtime_impl::TokioRuntime;
    pub use crate::send::{SendOptions, SendResult};
    #[cfg(feature = "sqlite-storage")]
    pub use crate::store::SqliteStore;
    pub use crate::types::events::{Event, EventKind};
    pub use crate::types::message::MessageInfo;
    pub use crate::{Jid, Server};
    pub use wacore::proto_helpers::{MessageBuilderExt, MessageExt};
    /// The protobuf namespace (`wa::Message`, `wa::message::*`).
    pub use waproto::whatsapp as wa;
}

pub use spam_report::{SpamFlow, SpamReportRequest, SpamReportResult};

#[cfg(test)]
pub mod test_utils;
