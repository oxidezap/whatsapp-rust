// Compile-checks the README examples as doctests, so the advertised quick
// start can never silently rot.
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Instrumenting large async fns (e.g. process_sync_task) wraps them in deep
// `Instrumented` future types; the default depth limit overflows when the
// `tracing` + `tracing-pii` paths combine. Raise it (compile-time only).
#![recursion_limit = "512"]

// Process-wide allocation counter shared by empirical unit-test guards. It sees
// every thread, so measurements go through `min_allocs`, which retries until a
// window lands quiet rather than trusting any single one.
#[cfg(test)]
#[allow(clippy::disallowed_types)]
pub(crate) mod test_alloc {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::sync::atomic::{AtomicU64, Ordering};

    pub(crate) static ALLOCS: AtomicU64 = AtomicU64::new(0);

    struct CountingAlloc;

    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            unsafe { System.dealloc(ptr, layout) }
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    /// Smallest allocation delta observed while running `op`, retrying until it
    /// reaches `expected`.
    ///
    /// `ALLOCS` counts every allocation in the process, so a sibling test thread
    /// allocating inside the window inflates that window's delta. A fixed
    /// iteration count only hopes one of its windows lands quiet, which is a
    /// flake under a loaded CI runner; retrying until the delta reaches
    /// `expected` makes ambient traffic cost iterations instead of a false
    /// failure. A real regression never reaches `expected`, so the caller's
    /// assertion still fires — with the count actually observed.
    pub(crate) fn min_allocs<T>(expected: u64, mut op: impl FnMut() -> T) -> u64 {
        // Bounded so a genuine regression fails instead of spinning forever.
        // The happy path exits on its first quiet window, so a budget this
        // large is free unless something is actually wrong.
        const BUDGET: u32 = 100_000;

        let mut min = u64::MAX;
        for _ in 0..BUDGET {
            let before = ALLOCS.load(Ordering::Relaxed);
            let value = std::hint::black_box(op());
            let after = ALLOCS.load(Ordering::Relaxed);
            drop(value);
            min = min.min(after - before);
            if min <= expected {
                break;
            }
        }
        min
    }
}

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

// Third-party re-exports: these crates' types appear in the public API, so
// consumers must name them; a direct dependency would have to version-match
// this crate exactly.
pub use anyhow;
pub use async_channel;
pub use async_trait::async_trait;
pub use bytes;
pub use futures;
pub use serde;
pub use serde_json;
pub use wacore::chrono;
pub use waproto::buffa;

pub mod cache;
pub use cache::Freshness;
pub mod portable_cache;
pub(crate) mod resend_rate_limiter;

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
/// Shared base error for transport/connection concerns; the per-domain error
/// types embed it.
pub use client::ClientError;
pub use client::NodeFilter;
pub use client::{
    AllocSnapshot, CollectionStats, HttpResourceReport, MemoryReport, ResourceReport,
    StatsSnapshot, StorageResourceReport, TransportResourceReport,
};
pub use client::{CallError, Voip};
pub use client::{Client, ClientBuild, ClientBuilder, ClientBuilderError, RawNodeLease};
#[cfg(feature = "client-lifecycle")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
pub use client::{ClientLifecycle, ConnectionScope, ConnectionScopeState};
pub use client::{ConnectError, ConnectStage, SignalMaintenanceError};
pub use types::durability_hook::InboundDurabilityHook;
pub use types::retry_admission::RetryAdmission;
pub mod download;
pub mod error;
pub use error::{ErrorChainExt, ServerRejection, Sources};
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
pub mod passkey;
#[cfg(feature = "plugins")]
#[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
pub mod plugins;
#[cfg(feature = "plugins")]
#[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
pub use plugins::{
    ClientPlugin, PluginCapabilities, PluginCapability, PluginConnectionScope,
    PluginConnectionTasks, PluginContext, PluginCoreEventSubscription, PluginCoreEvents,
    PluginEventEndpointConfig, PluginEventEndpointStats, PluginEventEnvelope, PluginEventOverflow,
    PluginEventPayloadEncoding, PluginEventPublishError, PluginEventPublishReport,
    PluginEventPublisherStats, PluginEventReceiveError, PluginEventRouteError, PluginEventRouter,
    PluginEventRouterStats, PluginEventSelector, PluginEventSubscribeError,
    PluginEventSubscription, PluginEventTopic, PluginEventTryReceiveError, PluginEvents,
    PluginFuture, PluginHealth, PluginHostConfig, PluginHostStats, PluginIq, PluginIqError,
    PluginManifest, PluginMessaging, PluginMessagingError, PluginPlanError, PluginResourceError,
    PluginState, PluginStats, PluginTasks, UntypedClientPlugin,
};
pub mod request;
pub(crate) mod signal_flush;
pub use request::IqError;
#[cfg(feature = "tokio-runtime")]
pub mod runtime_impl;
#[cfg(feature = "tokio-runtime")]
pub use runtime_impl::TokioRuntime;
pub use wacore::runtime::Runtime;
pub mod send;
pub use send::{EditOptions, PinDuration, RevokeType, SendError, SendOptions, SendResult};
pub use wacore::send::StanzaType;
pub mod media;
pub mod session;
pub mod socket;
pub mod store;
pub mod transport;
pub mod upload;
#[cfg(feature = "voip-runtime")]
pub mod voip;
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
    AppStateError, BatchGroupResult, Blocking, BlockingError, BlocklistEntry, ChatActions,
    ChatStateError, ChatStateType, Chatstate, Comments, Community, CommunityError,
    CommunitySubgroup, ContactError, Contacts, CreateCommunityOptions, CreateCommunityResult,
    CreateGroupResult, EncType, EncryptedEdit, EventCreationParams, EventResponseType, Events,
    GroupAppealStatus, GroupCreateOptions, GroupDescription, GroupEphemeralSettings, GroupError,
    GroupJoinError, GroupMetadata, GroupParticipant, GroupParticipantDetails,
    GroupParticipantOptions, GroupProfilePicture, GroupSubject, GroupType, Groups, GrowthLockInfo,
    InviteInfoError, IsOnWhatsAppResult, JoinGroupResult, Labels, LinkSubgroupsResult,
    MediaRetryResult, MediaReupload, MediaReuploadError, MediaReuploadRequest, MemberAddMode,
    MemberLinkMode, MemberShareHistoryMode, MembershipApprovalMode, MembershipRequest,
    MessageEditError, MessageRetransmission, Mex, MexError, MexErrorExtensions, MexGraphQLError,
    MexRequest, MexResponse, NackReason, Newsletter, NewsletterError, NewsletterMessage,
    NewsletterMessageType, NewsletterMetadata, NewsletterReactionCount, NewsletterRole,
    NewsletterState, NewsletterVerification, ParticipantChangeResponse, ParticipantType,
    PictureType, PollError, PollOptionResult, PollVoteCiphertext, Polls, Presence, PresenceError,
    PresenceStatus, PreviousDescription, Profile, ProfileError, ProfilePicture, ReachoutTimelock,
    RetryReason, RetryRequestError, RetryRequestOptions, RetryRequestOutcome, SecretEncKind,
    SecretEncrypted, SetProfilePictureResponse, Signal, SignalError, SignalSessionInfo,
    SignalSessionMigration, StanzaRejection, StanzaResponseError, Status, StatusPrivacySetting,
    StatusSendOptions, SyncActionMessageRange, TcToken, TcTokenError, UnlinkSubgroupsResult,
    UserInfo, UsyncSubprotocolError, VerifiedName, group_type, message_key, message_range,
};

pub mod bot;
pub mod lid_pn_cache;
#[cfg(feature = "signal")]
pub mod shutdown;
#[cfg(feature = "signal")]
pub use shutdown::shutdown_signal;
pub mod spam_report;
pub mod sync_task;
pub mod version;

/// One-import surface for the common bot path:
/// `use whatsapp_rust::prelude::*;`.
pub mod prelude {
    pub use crate::bot::{Bot, BotBuilder, BotHandle, EventDelivery, MessageContext};
    pub use crate::client::{Client, ClientBuilder, ClientBuilderError, ClientError, RawNodeLease};
    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
    pub use crate::client::{ClientLifecycle, ConnectionScope, ConnectionScopeState};
    pub use crate::client::{ConnectError, ConnectStage};
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub use crate::plugins::{
        ClientPlugin, PluginCapability, PluginConnectionScope, PluginContext,
        PluginCoreEventSubscription, PluginEventEndpointConfig, PluginEventOverflow,
        PluginEventPayloadEncoding, PluginEventRouter, PluginEventSelector,
        PluginEventSubscription, PluginEventTopic, PluginEvents, PluginFuture, PluginHostConfig,
        PluginManifest, UntypedClientPlugin,
    };
    pub use crate::request::IqError;
    #[cfg(feature = "tokio-runtime")]
    pub use crate::runtime_impl::TokioRuntime;
    pub use crate::send::{EditOptions, SendError, SendOptions, SendResult};
    #[cfg(feature = "signal")]
    pub use crate::shutdown::shutdown_signal;
    #[cfg(feature = "sqlite-storage")]
    pub use crate::store::SqliteStore;
    pub use crate::types::events::{
        BatchOrigin, ChannelEventHandler, Event, EventHandler, EventInterest, EventKind,
        InboundMessage, MessageBatch, Subscription,
    };
    pub use crate::types::message::MessageInfo;
    pub use crate::{Jid, Server};
    pub use wacore::proto_helpers::{MessageBuilderExt, MessageExt};
    /// Optional sub-message wrapper in `wa::Message` literals.
    pub use waproto::buffa::MessageField;
    /// The protobuf namespace (`wa::Message`, `wa::message::*`).
    pub use waproto::whatsapp as wa;
}

pub use spam_report::{SpamFlow, SpamReportRequest, SpamReportResult};

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod reexports_test;
