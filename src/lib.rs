// Compile-checks the README examples as doctests, so the advertised quick
// start can never silently rot.
#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]
// Instrumenting large async fns (e.g. process_sync_task) wraps them in deep
// `Instrumented` future types; the default depth limit overflows when the
// `tracing` + `tracing-pii` paths combine. Raise it (compile-time only).
#![recursion_limit = "512"]

// Test-only allocation instrumentation. Process totals remain available for
// diagnostics; scoped guards attribute allocations to the calling thread.
#[cfg(test)]
#[allow(clippy::disallowed_types)]
pub(crate) mod test_alloc {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    pub(crate) static ALLOCS: AtomicU64 = AtomicU64::new(0);
    /// Process-wide requested live bytes, for isolated diagnostic runs only.
    /// Concurrent frees can make a window's delta understate its retained bytes.
    pub(crate) static LIVE_BYTES: AtomicI64 = AtomicI64::new(0);

    #[derive(Default)]
    struct Measurement {
        live: AtomicI64,
        allocs: AtomicU64,
        max: AtomicUsize,
    }

    thread_local! {
        static ACTIVE: RefCell<Option<Arc<Measurement>>> = const { RefCell::new(None) };
        // Destructor-free: registry cleanup must still work during TLS teardown.
        static IN_REGISTRY: Cell<bool> = const { Cell::new(false) };
    }

    // A scope must be dropped on the thread whose ACTIVE entry it owns.
    struct Scope(Arc<Measurement>, std::marker::PhantomData<*const ()>);

    impl Scope {
        fn enter() -> Self {
            ACTIVE
                .with(|active| assert!(active.borrow().is_none(), "nested allocation measurement"));
            let state = Arc::new(Measurement::default());
            ACTIVE.with(|active| *active.borrow_mut() = Some(state.clone()));
            Self(state, std::marker::PhantomData)
        }
    }

    impl Drop for Scope {
        fn drop(&mut self) {
            ACTIVE.with(|active| active.borrow_mut().take());
        }
    }

    struct CountingAlloc;

    // Only scoped blocks are registered. BTreeMap's own allocations bypass the
    // registry via destructor-free TLS, avoiding allocator recursion/deadlock.
    // Entries keep the measurement alive after scope/thread exit, and removing
    // one on any thread debits only its owner. User layouts/pointers stay intact.
    static OWNERS: Mutex<BTreeMap<usize, Arc<Measurement>>> = Mutex::new(BTreeMap::new());
    static HAS_OWNED_BLOCKS: AtomicBool = AtomicBool::new(false);

    fn with_registry(op: impl FnOnce(&mut BTreeMap<usize, Arc<Measurement>>)) {
        if IN_REGISTRY.try_with(|active| active.replace(true)) != Ok(false) {
            return;
        }
        // No user code runs under this lock; never propagate poisoning through
        // GlobalAlloc, whose methods must not unwind.
        let mut owners = OWNERS.lock().unwrap_or_else(|error| error.into_inner());
        op(&mut owners);
        // Publish before releasing the lock or returning an allocated pointer.
        // Any thread that can free an owned block must observe its registration.
        HAS_OWNED_BLOCKS.store(!owners.is_empty(), Ordering::Release);
        drop(owners);
        IN_REGISTRY.with(|active| active.set(false));
    }

    unsafe impl GlobalAlloc for CountingAlloc {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let block = unsafe { System.alloc(layout) };
            if block.is_null() {
                return block;
            }
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            LIVE_BYTES.fetch_add(layout.size() as i64, Ordering::Relaxed);
            let owner = ACTIVE
                .try_with(|active| active.try_borrow().ok().and_then(|owner| owner.clone()))
                .ok()
                .flatten();
            if let Some(state) = owner {
                with_registry(|owners| {
                    state
                        .live
                        .fetch_add(layout.size() as i64, Ordering::Relaxed);
                    state.allocs.fetch_add(1, Ordering::Relaxed);
                    state.max.fetch_max(layout.size(), Ordering::Relaxed);
                    owners.insert(block.addr(), state);
                });
            }
            block
        }

        unsafe fn dealloc(&self, block: *mut u8, layout: Layout) {
            if HAS_OWNED_BLOCKS.load(Ordering::Acquire) {
                with_registry(|owners| {
                    if let Some(owner) = owners.remove(&block.addr()) {
                        owner
                            .live
                            .fetch_sub(layout.size() as i64, Ordering::Relaxed);
                    }
                });
            }
            LIVE_BYTES.fetch_sub(layout.size() as i64, Ordering::Relaxed);
            unsafe { System.dealloc(block, layout) };
        }

        unsafe fn realloc(&self, block: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let Ok(new_layout) = Layout::from_size_align(new_size, layout.align()) else {
                return ptr::null_mut();
            };
            // Attribute the replacement to the current scope, even if the old
            // block predates it. On failure the old block and owner stay intact.
            let replacement = unsafe { self.alloc(new_layout) };
            if !replacement.is_null() {
                unsafe {
                    ptr::copy_nonoverlapping(block, replacement, layout.size().min(new_size));
                    self.dealloc(block, layout);
                }
            }
            replacement
        }
    }

    #[global_allocator]
    static GLOBAL: CountingAlloc = CountingAlloc;

    /// Smallest process-wide allocation count over a synchronous call to `op`.
    /// Retries up to 100,000 times to exclude ambient allocations. Other threads
    /// can inflate this count but frees cannot reduce it. This counts successful
    /// allocations/reallocations, not retained bytes or allocator overhead.
    pub(crate) fn min_allocs<T>(expected: u64, mut op: impl FnMut() -> T) -> u64 {
        let mut min = u64::MAX;
        for _ in 0..100_000 {
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

    fn live_sample<T>(op: impl FnOnce() -> T) -> ((i64, u64), T) {
        let scope = Scope::enter();
        let held = std::hint::black_box(op());
        let sample = (
            scope.0.live.load(Ordering::Relaxed),
            scope.0.allocs.load(Ordering::Relaxed),
        );
        drop(scope);
        (sample, held)
    }

    /// Requested bytes still live and successful allocation/reallocation count
    /// owned by a synchronous call to `op` on this thread, before its return value
    /// is dropped. Frees on any thread debit only the block's originating scope;
    /// preexisting blocks cannot reduce this measurement. Realloc assigns the
    /// entire replacement block to the scope executing realloc.
    ///
    /// Other threads' allocations and work deferred past the call are excluded.
    /// This is not process heap/RSS or a measurement of spawned async tasks.
    /// Retries up to 10,000 times for warm-up; returns one actual sample, ordered
    /// by allocation count then bytes, if none meets both limits. No nesting.
    pub(crate) fn min_live<T>(expected: (i64, u64), mut op: impl FnMut() -> T) -> (i64, u64) {
        let mut best = (i64::MAX, u64::MAX);
        for _ in 0..10_000 {
            let (sample, held) = live_sample(&mut op);
            drop(held);
            if sample.0 <= expected.0 && sample.1 <= expected.1 {
                return sample;
            }
            if (sample.1, sample.0) < (best.1, best.0) {
                best = sample;
            }
        }
        best
    }

    /// Smallest largest requested block allocated/reallocated by a synchronous
    /// call to `op` on this thread, including blocks freed before return. Other
    /// threads cannot reset or inflate its maximum. Excludes spawned work and
    /// allocator overhead. Retries up to 10,000 times for warm-up. No nesting.
    pub(crate) fn min_max_block<T>(expected: usize, mut op: impl FnMut() -> T) -> usize {
        let mut min = usize::MAX;
        for _ in 0..10_000 {
            let scope = Scope::enter();
            let value = std::hint::black_box(op());
            let observed = scope.0.max.load(Ordering::Relaxed);
            drop(scope);
            drop(value);
            min = min.min(observed);
            if min <= expected {
                break;
            }
        }
        min
    }

    #[test]
    fn unrelated_frees_cannot_hide_retained_bytes() {
        let phase = AtomicUsize::new(0);
        std::thread::scope(|threads| {
            threads.spawn(|| {
                let preexisting = std::hint::black_box(vec![0u8; 4096]);
                phase.store(1, Ordering::Release);
                while phase.load(Ordering::Acquire) != 2 {
                    std::thread::yield_now();
                }
                drop(preexisting);
                phase.store(3, Ordering::Release);
            });
            while phase.load(Ordering::Acquire) != 1 {
                std::thread::yield_now();
            }
            // The handshake itself must not allocate: channel parking can
            // initialize thread-local state inside the measurement window.
            let (sample, retained) = live_sample(|| {
                let retained = std::hint::black_box(vec![0u8; 2048]);
                phase.store(2, Ordering::Release);
                while phase.load(Ordering::Acquire) != 3 {
                    std::thread::yield_now();
                }
                retained
            });
            drop(retained);
            assert_eq!(sample, (2048, 1));
            assert!(sample.0 > 1024);
        });
    }

    #[test]
    fn min_live_retries_until_both_limits_hold_in_one_sample() {
        let mut attempts = 0;
        let sample = min_live((1024, 1), || {
            attempts += 1;
            match attempts {
                1 => Some(vec![0u8; 2048]), // Only the allocation count fits.
                2 => {
                    let transient = std::hint::black_box((vec![0u8; 32], vec![0u8; 32]));
                    drop(transient); // Only the retained-byte count fits.
                    None
                }
                3 => Some(vec![0u8; 1024]),
                _ => panic!("the third sample meets both limits"),
            }
        });
        assert_eq!(attempts, 3);
        assert_eq!(sample, (1024, 1));
    }

    #[test]
    fn concurrent_measurements_keep_their_maxima() {
        use std::sync::Barrier;
        let allocated = Barrier::new(2);
        let measured = Barrier::new(2);
        std::thread::scope(|threads| {
            let first = threads.spawn(|| {
                min_max_block(4096, || {
                    let block = std::hint::black_box(vec![0u8; 4096]);
                    drop(block);
                    allocated.wait();
                    measured.wait();
                })
            });
            allocated.wait();
            let second = min_max_block(128, || std::hint::black_box(vec![0u8; 128]));
            measured.wait();
            assert_eq!(first.join().unwrap(), 4096);
            assert_eq!(second, 128);
        });
    }

    #[test]
    fn owned_blocks_can_be_freed_on_another_thread() {
        let scope = Scope::enter();
        let value = std::hint::black_box(vec![0u8; 2048]);
        let state = scope.0.clone();
        drop(scope);
        assert_eq!(state.live.load(Ordering::Relaxed), 2048);
        std::thread::spawn(move || drop(value)).join().unwrap();
        assert_eq!(state.live.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn preexisting_frees_and_reallocations_have_distinct_ownership() {
        let mut old = Some(vec![0u8; 4096]);
        assert_eq!(min_live((0, 0), || drop(old.take())), (0, 0));
        let mut old = Some(vec![0u8; 4096]);
        let sample = min_live((8192, 1), || {
            let mut value = old.take().unwrap();
            value.reserve_exact(4096);
            value
        });
        assert_eq!(sample, (8192, 1));
        assert_eq!(min_live((0, 1), || drop(vec![0u8; 2048])), (0, 1));
    }

    #[test]
    fn scope_is_restored_after_unwind_and_thread_exit() {
        let result = std::panic::catch_unwind(|| {
            let _scope = Scope::enter();
            panic!("test scope cleanup");
        });
        assert!(result.is_err());
        assert_eq!(min_live((32, 1), || vec![0u8; 32]), (32, 1));
        struct AllocateOnExit;
        impl Drop for AllocateOnExit {
            fn drop(&mut self) {
                drop(std::hint::black_box(vec![0u8; 128]));
            }
        }
        thread_local! {
            static ON_EXIT: AllocateOnExit = const { AllocateOnExit };
        }
        let value = std::thread::spawn(|| {
            ON_EXIT.with(|_| ());
            let _scope = Scope::enter();
            vec![0u8; 64]
        })
        .join()
        .unwrap();
        drop(value);
    }

    #[test]
    fn allocator_preserves_alignment_zeroing_and_realloc_contents() {
        unsafe {
            let layout = Layout::from_size_align(256, 4096).unwrap();
            let block = std::alloc::alloc_zeroed(layout);
            assert!(!block.is_null());
            assert_eq!(block as usize % 4096, 0);
            assert!(
                std::slice::from_raw_parts(block, 256)
                    .iter()
                    .all(|&b| b == 0)
            );
            block.write_bytes(42, 256);
            let grown = std::alloc::realloc(block, layout, 8192);
            assert!(!grown.is_null());
            assert_eq!(grown as usize % 4096, 0);
            assert!(
                std::slice::from_raw_parts(grown, 256)
                    .iter()
                    .all(|&b| b == 42)
            );
            std::alloc::dealloc(grown, Layout::from_size_align(8192, 4096).unwrap());
        }
    }
}

/// The app-state collections, named as they appear on the wire. Part of the
/// public surface because [`Client::resync_app_state`] takes them.
///
/// [`WAPatchName::Unknown`] is not one of them: it is what parsing an
/// unrecognised collection name yields, so the server has nothing under that
/// name. `resync_app_state` rejects a request naming it.
pub use wacore::appstate::patch_decode::WAPatchName;
pub use wacore::appstate::schemas;
pub use wacore::client_profile::ClientProfile;
pub use wacore::store::traits::StoredMessageSecret;
/// Optional metrics emission (the `metrics` feature). No-op when the feature is off.
pub use wacore::telemetry;
pub use wacore::types::message_ref::{
    MessageId, MessageRef, MessageRefError, NewsletterMessageRef, ServerMessageId, StanzaId,
};
pub use wacore::types::message_secret::{InvalidMessageSecret, MessageSecret};
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
pub use client::interceptor::{Interception, InterceptorHandle, StanzaInterceptor};
pub use client::{
    AllocSnapshot, CollectionStats, HttpResourceReport, MemoryReport, ResourceReport,
    StatsSnapshot, StorageResourceReport, SubsystemCollection, SubsystemMemory,
    TransportResourceReport,
};
pub use client::{CallError, Voip};
pub use client::{
    Client, ClientBuild, ClientBuilder, ClientBuilderError, ClientOptions, Connection,
    DecryptedPayloadLease, EncDecryptFailedLease, RawNodeLease, SentFrameLease,
};
#[cfg(feature = "client-lifecycle")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
pub use client::{ClientLifecycle, ConnectionScope, ConnectionScopeState};
pub use client::{
    ConflictKind, ConnectError, ConnectStage, DeregistrationOutcome, DeregistrationSkipReason,
    DrainOutcome, LogoutReport, ProtocolTerminalReason, Reachability, RunCompletionReason,
    SecretFlushReport, ShutdownReport, SignalMaintenanceError,
};
pub use client::{NodeFilter, NodeWaiter};
pub use types::connect_admission::ConnectAdmission;
pub use types::durability_hook::InboundDurabilityHook;
pub use types::history_sync_admission::{
    HistorySyncAdmission, HistorySyncDecision, HistorySyncMetadata,
};
pub use types::retry_admission::RetryAdmission;
pub mod download;
pub mod error;
pub use error::{ErrorChainExt, ServerRejection, Sources};
pub mod handlers;
pub use handlers::chatstate::ChatStateEvent;
pub mod handshake;
pub mod jid_utils;
// Worker modules contain only inherent Client operations, not host types.
// Those operations remain public through Client rather than an empty module.
mod keepalive;
pub mod mediaconn;
mod message;
pub(crate) mod msg_secret_buffer;
pub mod pair;
pub mod pair_code;
#[cfg(feature = "passkey")]
#[cfg_attr(docsrs, doc(cfg(feature = "passkey")))]
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
    PluginFuture, PluginHealth, PluginHostConfig, PluginHostStats, PluginInterceptorRegistration,
    PluginIq, PluginIqError, PluginManifest, PluginMessaging, PluginMessagingError,
    PluginPlanError, PluginResourceError, PluginStanzaInterception, PluginState, PluginStats,
    PluginTasks, UntypedClientPlugin,
};
pub mod request;
pub(crate) mod signal_flush;
pub use request::{IqError, RejectionStanza};
#[cfg(feature = "tokio-runtime")]
pub mod runtime_impl;
// The module is the feature's; the type inside it is the target's (`tokio::spawn` needs threads).
// Re-exporting on the feature alone made `--features tokio-runtime` on wasm32 an unresolved import
// rather than a runtime the target simply does not have.
#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
pub use runtime_impl::TokioRuntime;
pub use wacore::runtime::Runtime;
pub mod send;
pub use send::{
    EditOptions, EditRequest, PinDuration, RevokeType, SendError, SendOptions, SendRequest,
    SendResult,
};
pub use wacore::send::StanzaType;
pub mod media;
pub mod session;
pub mod socket;
pub mod store;
pub mod transport;
pub mod upload;
#[cfg(feature = "voip-control")]
pub mod voip;
#[cfg(feature = "voip-control")]
pub mod voip_control;
pub use upload::UploadOptions;

pub mod pdo;
pub mod prekeys;
mod receipt;
mod retry;
pub(crate) mod unified_session;

pub mod appstate_sync;
mod history_sync;
pub mod usync;

/// Declared syncd action names for log gating (generated, no `Schema` records).
pub(crate) mod appstate_known_verbs;

pub mod features;
pub use features::PushNameOutcome;
pub use features::{
    AppStateError, AppStateResyncMode, AppStateResyncReport, AppStateSettings,
    BUSINESS_PROFILE_MAX_WEBSITES, Blocking, BlockingError, BlocklistEntry, BotDefault, BotList,
    BotListEntry, BotListSection, BotListVersion, BotSectionDisplayType, BotSectionType, BotTheme,
    BotThemeMode, Bots, Business, BusinessCategory, BusinessError, BusinessHourMode, BusinessHours,
    BusinessHoursConfig, BusinessHoursUpdate, BusinessProfile, BusinessProfileUpdate,
    BusinessProfileUpdateError, CappingMvStatus, CappingOteStatus, CappingStatus, Catalog,
    CatalogOptions, ChatActions, ChatActivity, ChatStateError, Chatstate, Collection,
    CollectionOptions, Collections, Comments, Community, CommunityConfigurationStep,
    CommunityError, CommunitySubgroup, ContactError, Contacts, CoverPhotoUpload,
    CreateCommunityOptions, CreateCommunityResult, CreateGroupResult, CreateSubgroupOptions,
    DayOfWeek, EncType, EncryptedEdit, EventCreationParams, EventResponseType, Events,
    GroupAppealStatus, GroupCreateOptions, GroupDescription, GroupEphemeralSettings, GroupError,
    GroupHierarchy, GroupHistoryAddResult, GroupHistoryRetryToken, GroupHistoryShareOutcome,
    GroupHistorySkipReason, GroupJoinError, GroupLookupResult, GroupMessageReporter, GroupMetadata,
    GroupMetadataResult, GroupOverview, GroupOverviewResult, GroupParticipant,
    GroupParticipantDetails, GroupParticipantOptions, GroupPictureEntry, GroupProfilePicture,
    GroupProfilePictureOutcome, GroupSubject, Groups, GrowthLockInfo, HistorySharePreparation,
    ImporterAddress, InviteInfoError, IsOnWhatsAppResult, JoinGroupResult, Labels,
    LinkSubgroupOptions, LinkSubgroupsResult, MediaRetryResult, MediaReupload, MediaReuploadError,
    MediaReuploadPhase, MediaReuploadRequest, MemberAddMode, MemberLinkMode,
    MemberShareHistoryMode, MembershipApprovalMode, MembershipRequest, MessageEditError,
    MessageRetransmission, Mex, MexError, MexErrorExtensions, MexFatalError, MexGraphQLError,
    MexRequest, MexResponse, NackReason, NewChatMessageCapping, Newsletter, NewsletterAdminInfo,
    NewsletterAdminProfile, NewsletterError, NewsletterFollower, NewsletterMediaType,
    NewsletterMessage, NewsletterMessageAssociationType, NewsletterMessageType, NewsletterMetadata,
    NewsletterMyAddOns, NewsletterMyPollVote, NewsletterMyReaction, NewsletterPollVote,
    NewsletterQuestionType, NewsletterReactionCount, NewsletterRole, NewsletterState,
    NewsletterVerification, Order, OrderPriceDetails, OrderProduct, OwnUsername,
    ParticipantChangeResponse, ParticipantType, PictureType, Pictures, PollError, PollOptionResult,
    PollVoteCiphertext, Polls, PreparedGroupHistoryShare, Presence, PresenceError, PresencePolicy,
    PresenceStatus, PreviousDescription, Price, Product, ProductAvailability, ProductImage,
    ProductVideo, Profile, ProfileError, ProfilePicture, ProfilePictureLookup, QuickReplies,
    ReachoutTimelock, ReportedGroupMessage, ReportedGroupMessages, RetryReason, RetryRequestError,
    RetryRequestOptions, RetryRequestOutcome, SalePrice, SecretEncKind, SecretEncrypted,
    SetProfilePictureResponse, Signal, SignalError, SignalSessionInfo, SignalSessionMigration,
    StanzaRejection, StanzaResponseError, Status, StatusPrivacySetting, StatusSendOptions,
    SubgroupFailure, SubgroupKind, SubgroupVisibility, SyncActionMessageRange, TcToken,
    TcTokenError, USERNAME_MAX_LENGTH, USERNAME_MIN_LENGTH, UnlinkSubgroupsResult, UserInfo,
    UsernameLookup, UsernameLookupError, UsernameLookupUser, UsyncSubprotocolError,
    VariantProperty, VerifiedName, message_range,
};
pub use features::{CreatedEvent, CreatedPoll, EventRef, MexDoc, MexOperation, PollRef};

pub use features::{ProfilePictureRequest, ProfilePictureTarget, ProfilePictureType};

pub mod bot;
pub use bot::{
    BotRunOutcome, BotShutdownReport, CallbackEventHandler, EventDelivery, EventDeliveryStats,
};
pub use store::StoreRelease;
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
    pub use crate::bot::{
        BotRunOutcome, BotShutdownReport, CallbackEventHandler, EventDeliveryStats,
    };
    pub use crate::client::{
        Client, ClientBuilder, ClientBuilderError, ClientError, ClientOptions, Connection,
        DecryptedPayloadLease, EncDecryptFailedLease, RawNodeLease, SentFrameLease,
    };
    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
    pub use crate::client::{ClientLifecycle, ConnectionScope, ConnectionScopeState};
    pub use crate::client::{
        ConflictKind, ConnectError, ConnectStage, DeregistrationOutcome, DeregistrationSkipReason,
        DrainOutcome, LogoutReport, ProtocolTerminalReason, RunCompletionReason, SecretFlushReport,
        ShutdownReport,
    };
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub use crate::plugins::{
        ClientPlugin, PluginCapability, PluginConnectionScope, PluginContext,
        PluginCoreEventSubscription, PluginEventEndpointConfig, PluginEventOverflow,
        PluginEventPayloadEncoding, PluginEventRouter, PluginEventSelector,
        PluginEventSubscription, PluginEventTopic, PluginEvents, PluginFuture, PluginHostConfig,
        PluginInterceptorRegistration, PluginManifest, PluginStanzaInterception,
        UntypedClientPlugin,
    };
    pub use crate::request::{IqError, RejectionStanza};
    #[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
    pub use crate::runtime_impl::TokioRuntime;
    pub use crate::send::{
        EditOptions, EditRequest, SendError, SendOptions, SendRequest, SendResult,
    };
    #[cfg(feature = "signal")]
    pub use crate::shutdown::shutdown_signal;
    pub use crate::store::StoreRelease;
    #[cfg(feature = "sqlite-storage")]
    pub use crate::store::{SqliteStore, StoredDeviceSummary};
    pub use crate::types::events::{
        BatchOrigin, ChannelEventHandler, ChannelEventStats, Event, EventHandler, EventInterest,
        EventKind, InboundMessage, MessageBatch, Subscription,
    };
    pub use crate::types::message::MessageInfo;
    pub use crate::{
        CreatedEvent, CreatedPoll, EventRef, InvalidMessageSecret, Jid, MessageSecret, PollRef,
        Server, StoredMessageSecret,
    };
    pub use crate::{
        MessageId, MessageRef, MessageRefError, NewsletterMessageRef, ServerMessageId, StanzaId,
    };
    pub use wacore::proto_helpers::{MessageBuilderExt, MessageExt};
    /// Optional sub-message wrapper in `wa::Message` literals.
    pub use waproto::buffa::MessageField;
    /// The protobuf namespace (`wa::Message`, `wa::message::*`).
    pub use waproto::whatsapp as wa;
}

pub use spam_report::{SpamFlow, SpamReportRequest, SpamReportResult};

/// Offline fixture the client-level benchmarks build on. Not a public API:
/// gated behind the non-default `bench-harness` feature and hidden from docs,
/// so an ordinary build carries neither the module nor its sink transport.
#[cfg(feature = "bench-harness")]
#[doc(hidden)]
pub mod bench_support;

#[cfg(test)]
pub mod test_utils;

#[cfg(all(not(target_arch = "wasm32"), any(test, feature = "test-support")))]
#[doc(hidden)]
pub mod test_support;

#[cfg(test)]
mod reexports_test;
