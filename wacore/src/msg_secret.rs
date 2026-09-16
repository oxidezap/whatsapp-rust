//! Retention policy for the per-message `messageSecret` store.
//!
//! This client is headless and keeps no message store, so it must persist a
//! standalone secret index. That index bloats: PR #665 seeds a secret for every
//! non-forwarded history-sync message and captures one for every live message
//! and every send. The secrets only unlock add-ons that reference a parent by
//! id (secret-encrypted edits, msmsg bot replies, poll/event edits), all of
//! which are bounded in time, so retaining a secret for every message forever
//! is pure waste.
//!
//! This module turns the three scattered decisions (capture / seed / prune)
//! into one [`MsgSecretPolicy`] and bounds retention by the *parent message's*
//! event time, per add-on kind, via `expires_at`.
//!
//! # The store contract
//!
//! A row is keyed by `(chat, sender, msg_id)` and carries a 32-byte `secret`,
//! an absolute `expires_at` deadline (`0` = never), and the parent's
//! `message_ts` (`0` = unknown). `created_at` was removed: it had no reader
//! once `expires_at` became the retention model, so it was pure write
//! amplification. The SQLite backend keeps its expiry index partial over
//! `expires_at <> 0`, so rows that can never expire cost nothing in it.
//!
//! # Pruning
//!
//! Retention fires from the keepalive's ~5-minute tick and once at startup
//! (`Client::run_startup_maintenance`), both through the store's write permit.
//! The startup pass is what reaps rows that expired while the process was
//! closed, and covers an app that opens the store without a connection. `Full`
//! is the only policy that does not prune.

use std::time::Duration;

use async_trait::async_trait;

use crate::proto_helpers::MessageExt;
use waproto::whatsapp as wa;

/// How the core manages `messageSecret` persistence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MsgSecretPolicy {
    /// Bounded default: capture live secrets, seed only the still-relevant
    /// slice of history, and prune by a per-kind event-time horizon.
    #[default]
    Managed,
    /// Pre-#665 behavior: only capture/seed secrets in bot contexts.
    BotOnly,
    /// Capture and seed everything, never prune (unbounded retention).
    Full,
    /// Persist nothing in core. Add-on decryption relies entirely on an
    /// app-supplied [`OriginalMessageResolver`]; without one, add-ons whose
    /// parent secret the core never saw simply will not decrypt.
    Disabled,
}

impl MsgSecretPolicy {
    /// Whether this policy writes secrets to the backing store at all.
    pub fn persists(self) -> bool {
        !matches!(self, MsgSecretPolicy::Disabled)
    }

    /// Whether capture/seed is restricted to bot contexts.
    pub fn bot_only(self) -> bool {
        matches!(self, MsgSecretPolicy::BotOnly)
    }

    /// Whether the periodic prune sweep should run. Everything except `Full`
    /// prunes: `Managed`/`BotOnly` reap their own expired rows, and `Disabled`
    /// still reaps legacy rows left by a prior policy (it just writes no new
    /// ones). Only `Full` keeps everything forever.
    pub fn prunes(self) -> bool {
        !matches!(self, MsgSecretPolicy::Full)
    }

    /// Whether rows written under this policy get a finite deadline. Only
    /// `Managed`/`BotOnly` do; `Full` writes never-expire rows and `Disabled`
    /// writes none.
    pub fn bounds_retention(self) -> bool {
        matches!(self, MsgSecretPolicy::Managed | MsgSecretPolicy::BotOnly)
    }
}

/// Per-add-on-kind retention horizons applied to the parent message's event
/// time. Defaults are derived from verified protocol limits, not guesses.
#[derive(Debug, Clone, Copy)]
pub struct MsgSecretRetention {
    /// Text / `MESSAGE_EDIT` parents. An edit is only valid when authored within
    /// 20 min of the parent — enforced on both send and receive against the
    /// edit's own timestamp (`editTs < parentTs + window`), not "now" — so a
    /// validly-authored edit still applies after an offline gap. WhatsApp's
    /// offline queue can deliver it up to ~30 days later, so a 30-day-offline
    /// receiver still needs the secret; hence a 30-day horizon for delivery.
    pub text: Duration,
    /// Poll / event parents (poll votes, `PollAddOption`, `EventEdit`,
    /// `PollEdit`). These add-ons have no sender-side time window, so the
    /// parent secret must outlive them generously.
    pub poll_event: Duration,
    /// Outbound msmsg bot context secrets.
    pub bot: Duration,
}

impl Default for MsgSecretRetention {
    fn default() -> Self {
        Self {
            text: Duration::from_secs(30 * 86_400),
            poll_event: Duration::from_secs(90 * 86_400),
            bot: Duration::from_secs(30 * 86_400),
        }
    }
}

impl MsgSecretRetention {
    fn horizon_secs(&self, class: RetentionClass) -> u64 {
        match class {
            RetentionClass::Text => self.text.as_secs(),
            RetentionClass::PollEvent => self.poll_event.as_secs(),
            RetentionClass::Bot => self.bot.as_secs(),
        }
    }
}

/// Default `message_edit_window_duration_seconds` (WA Web AB prop, 20 min). An
/// edit is only valid when authored within this window of its parent — checked
/// against the edit's own timestamp (`editTs < parentTs + window`), not "now" —
/// so the receive path can drop a stale edit even after an offline delivery gap.
pub const EDIT_PROCESSING_WINDOW_SECS: i64 = 1200;

/// Retention class of a stored secret, fixed at write time by which call site
/// wrote it (the store cannot see the add-on kind at prune time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionClass {
    Text,
    PollEvent,
    Bot,
}

/// Whether `msg` belongs to a bot context for retention: the chat is a bot, or
/// the message invokes a bot (top-level botMetadata or a bot mention). A group
/// message that invokes a bot is a bot context even though the chat is a group,
/// so its secret is kept under `BotOnly` and the later bot reply can decrypt.
pub fn is_bot_context(chat_is_bot: bool, msg: &wa::Message) -> bool {
    chat_is_bot || invokes_bot(msg)
}

fn invokes_bot(msg: &wa::Message) -> bool {
    let has_bot_metadata = |m: &wa::Message| {
        m.message_context_info
            .as_option()
            .is_some_and(|c| c.bot_metadata.as_option().is_some())
    };
    // botMetadata sits on the top-level MessageContextInfo even when wrapped.
    has_bot_metadata(msg) || has_bot_metadata(msg.get_base_message()) || msg.mentions_any_bot()
}

fn message_is_poll_or_event(msg: &wa::Message) -> bool {
    let base = msg.get_base_message();
    base.poll_creation_message.as_option().is_some()
        || base.poll_creation_message_v2.as_option().is_some()
        || base.poll_creation_message_v3.as_option().is_some()
        || base.event_message.as_option().is_some()
}

/// Classify a message for retention. Bot context wins (bot horizon), then
/// poll/event (longer horizon), else text. Unwraps device-sent/ephemeral/etc.
/// wrappers first.
pub fn classify(msg: &wa::Message, chat_is_bot: bool) -> RetentionClass {
    classify_from_flags(
        is_bot_context(chat_is_bot, msg),
        message_is_poll_or_event(msg),
    )
}

/// Classify from precomputed flags. Used where only flags are available — the
/// history-sync seed has no full `wa::Message` in hand. Keeps the single
/// three-way rule in one place so the seed and live paths cannot drift.
pub fn classify_from_flags(bot_context: bool, poll_or_event: bool) -> RetentionClass {
    if bot_context {
        RetentionClass::Bot
    } else if poll_or_event {
        RetentionClass::PollEvent
    } else {
        RetentionClass::Text
    }
}

/// Compute the absolute `expires_at` deadline (unix seconds, `0` = never) for a
/// secret row.
///
/// `Full`/`Disabled` never set a deadline. `message_ts` is the parent's event
/// time; when unknown it falls back to `now` so an unknown-age secret still
/// expires a horizon from when we first saw it (bounded), rather than living
/// forever — but it is never dropped at write for lacking a timestamp.
///
/// `0` is the store's sentinel for "unknown", the same as `None`: the receive
/// path already reads a stored `0` that way when it checks the edit window. It
/// must be treated as unknown here too. A stanza whose `t` attribute is absent
/// parses to the Unix epoch, and taking that literally would mint a deadline in
/// 1970, so the row would be born already expired and the next sweep would drop
/// a secret captured moments ago.
pub fn expires_at(
    policy: MsgSecretPolicy,
    retention: &MsgSecretRetention,
    class: RetentionClass,
    message_ts: Option<u64>,
    now: i64,
) -> i64 {
    if !policy.bounds_retention() {
        return 0;
    }
    let base = message_ts
        .filter(|&ts| ts != 0)
        .and_then(|t| i64::try_from(t).ok())
        .unwrap_or(now);
    let horizon = i64::try_from(retention.horizon_secs(class)).unwrap_or(i64::MAX);
    base.saturating_add(horizon)
}

/// Whether a history-sync secret with parent event time `message_ts` is still
/// within its retention horizon at `now` and is worth seeding. Records with no
/// timestamp are kept (we cannot prove they are too old); `0` is the same
/// "unknown" sentinel [`expires_at`] reads, not Unix time zero, so a history
/// record carrying it is seeded rather than rejected as decades old. Only
/// `Managed`/`BotOnly` filter; `Full` seeds everything and `Disabled` seeds
/// nothing (decided by the caller, not here).
pub fn within_seed_horizon(
    retention: &MsgSecretRetention,
    class: RetentionClass,
    message_ts: Option<u64>,
    now: i64,
) -> bool {
    let Some(ts) = message_ts
        .filter(|&t| t != 0)
        .and_then(|t| i64::try_from(t).ok())
    else {
        return true;
    };
    let horizon = i64::try_from(retention.horizon_secs(class)).unwrap_or(i64::MAX);
    ts.saturating_add(horizon) > now
}

/// App-supplied fallback returning a parent message's 32-byte `messageSecret`
/// on a store miss, keyed by the non-AD `(chat, sender, msg_id)`.
///
/// Lets an app that keeps its own message store own secret retention entirely
/// (the secret rides on the stored message and is read back on demand), and is
/// what makes the [`MsgSecretPolicy::Disabled`] tier able to decrypt add-ons
/// whose parent the core never persisted. Consulted only after the in-core
/// store and LID/PN alternate lookups miss.
///
/// The call is run under a bounded timeout (configurable, default 5s) because
/// it executes inside the per-chat receive lane; an implementation that blocks
/// past the bound is treated as a miss, so keep it fast or internally bounded.
///
/// The `Send + Sync` bound is dropped on wasm32 (single-threaded), so a
/// resolver there may be backed by `!Send` JS handles; native builds keep it
/// because the resolver is shared across the multi-threaded receive lanes.
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait OriginalMessageResolver: Send + Sync {
    async fn resolve_msg_secret(&self, chat: &str, sender: &str, msg_id: &str) -> Option<[u8; 32]>;

    /// Metadata-carrying variant of [`Self::resolve_msg_secret`].
    ///
    /// An app that stores its own messages also knows the parent's event time,
    /// and returning it lets the receive path enforce the same
    /// edit-processing window it applies to a row that came from the in-core
    /// store (`editTs < message_ts + EDIT_PROCESSING_WINDOW_SECS`). Without it
    /// the window cannot be checked and the core stays permissive, which
    /// accepts a stale edit that WhatsApp Web would drop — accepting a
    /// superseded edit's text is a correctness gap, not just a cosmetic one.
    ///
    /// Defaults to the plain secret with an unknown timestamp, so an existing
    /// implementation keeps working and simply opts out of the window check.
    /// Override this (not the trait) to supply the parent time.
    async fn resolve_msg_secret_with_metadata(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Option<ResolvedMessageSecret> {
        self.resolve_msg_secret(chat, sender, msg_id)
            .await
            .map(ResolvedMessageSecret::without_timestamp)
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait OriginalMessageResolver {
    async fn resolve_msg_secret(&self, chat: &str, sender: &str, msg_id: &str) -> Option<[u8; 32]>;

    /// See the native trait's [`OriginalMessageResolver::resolve_msg_secret_with_metadata`].
    async fn resolve_msg_secret_with_metadata(
        &self,
        chat: &str,
        sender: &str,
        msg_id: &str,
    ) -> Option<ResolvedMessageSecret> {
        self.resolve_msg_secret(chat, sender, msg_id)
            .await
            .map(ResolvedMessageSecret::without_timestamp)
    }
}

/// A parent `messageSecret` recovered by an [`OriginalMessageResolver`],
/// together with whatever the app knows about the parent's event time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedMessageSecret {
    /// The 32-byte `messageSecret` of the parent message.
    pub secret: [u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
    /// Parent message event time (unix seconds), or `None` when the app does
    /// not track it. `Some` enables the edit-processing window check; `None`
    /// leaves it unenforced, the historical behaviour.
    pub message_ts: Option<i64>,
}

impl ResolvedMessageSecret {
    /// A secret with no known parent timestamp: the edit window is not checked.
    pub const fn without_timestamp(
        secret: [u8; crate::reporting_token::MESSAGE_SECRET_SIZE],
    ) -> Self {
        Self {
            secret,
            message_ts: None,
        }
    }

    /// The parent timestamp as the store reports it: `0` when unknown, so the
    /// receive path can share one check for rows from either source.
    pub const fn message_ts_or_zero(&self) -> i64 {
        match self.message_ts {
            Some(ts) => ts,
            None => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    /// The metadata variant defaults to the plain secret with no timestamp, so
    /// a resolver that implements only `resolve_msg_secret` keeps working and
    /// opts out of the edit-window check. This is the compatibility contract.
    #[tokio::test]
    async fn legacy_resolver_defaults_to_no_timestamp() {
        struct Legacy;
        #[async_trait]
        impl OriginalMessageResolver for Legacy {
            async fn resolve_msg_secret(&self, _: &str, _: &str, _: &str) -> Option<[u8; 32]> {
                Some([0x11; 32])
            }
        }

        let got = Legacy
            .resolve_msg_secret_with_metadata("c", "s", "m")
            .await
            .expect("the legacy secret must surface");
        assert_eq!(got.secret, [0x11; 32]);
        assert_eq!(got.message_ts, None);
        assert_eq!(
            got.message_ts_or_zero(),
            0,
            "an absent timestamp reads as unknown for the shared window check"
        );
    }

    /// A resolver that knows the parent time reports it, and a miss stays a miss.
    #[tokio::test]
    async fn metadata_resolver_reports_the_parent_time() {
        struct WithTs;
        #[async_trait]
        impl OriginalMessageResolver for WithTs {
            async fn resolve_msg_secret(&self, _: &str, _: &str, _: &str) -> Option<[u8; 32]> {
                None
            }
            async fn resolve_msg_secret_with_metadata(
                &self,
                _: &str,
                _: &str,
                msg_id: &str,
            ) -> Option<ResolvedMessageSecret> {
                (msg_id == "hit").then_some(ResolvedMessageSecret {
                    secret: [0x22; 32],
                    message_ts: Some(1_700_000_000),
                })
            }
        }

        let hit = WithTs
            .resolve_msg_secret_with_metadata("c", "s", "hit")
            .await
            .expect("hit");
        assert_eq!(hit.secret, [0x22; 32]);
        assert_eq!(hit.message_ts_or_zero(), 1_700_000_000);
        assert!(
            WithTs
                .resolve_msg_secret_with_metadata("c", "s", "miss")
                .await
                .is_none()
        );
    }

    #[test]
    fn full_and_disabled_never_expire() {
        let r = MsgSecretRetention::default();
        for policy in [MsgSecretPolicy::Full, MsgSecretPolicy::Disabled] {
            assert_eq!(
                expires_at(policy, &r, RetentionClass::Text, Some(1_000), 2_000),
                0,
                "{policy:?} must not set a deadline"
            );
        }
    }

    #[test]
    fn managed_text_expires_30d_after_message_time() {
        let r = MsgSecretRetention::default();
        let msg_ts = 1_000_000u64;
        let got = expires_at(
            MsgSecretPolicy::Managed,
            &r,
            RetentionClass::Text,
            Some(msg_ts),
            5_000_000,
        );
        assert_eq!(got, msg_ts as i64 + 30 * DAY);
    }

    #[test]
    fn poll_event_horizon_is_longer_than_text() {
        let r = MsgSecretRetention::default();
        let now = 10_000_000i64;
        let text = expires_at(
            MsgSecretPolicy::Managed,
            &r,
            RetentionClass::Text,
            Some(1_000),
            now,
        );
        let poll = expires_at(
            MsgSecretPolicy::Managed,
            &r,
            RetentionClass::PollEvent,
            Some(1_000),
            now,
        );
        assert!(poll > text, "poll/event must outlive text secrets");
        assert_eq!(poll - text, (90 - 30) * DAY);
    }

    #[test]
    fn unknown_timestamp_expires_a_horizon_from_now_not_forever() {
        let r = MsgSecretRetention::default();
        let now = 5_000_000i64;
        let got = expires_at(
            MsgSecretPolicy::Managed,
            &r,
            RetentionClass::Text,
            None,
            now,
        );
        assert_eq!(got, now + 30 * DAY, "unknown age is bounded, never 0");
    }

    /// `0` is the store's "unknown" sentinel, not a real 1970 event time. A
    /// stanza with no `t` attribute parses to the epoch, and taking it literally
    /// would mint a deadline in 1970: the row would be born expired and the next
    /// sweep would drop a secret captured moments earlier.
    #[test]
    fn a_zero_timestamp_is_unknown_not_the_epoch() {
        let r = MsgSecretRetention::default();
        let now = 1_800_000_000i64;
        let got = expires_at(
            MsgSecretPolicy::Managed,
            &r,
            RetentionClass::Text,
            Some(0),
            now,
        );
        assert_eq!(
            got,
            now + 30 * DAY,
            "Some(0) must read as unknown and expire a horizon from now"
        );
    }

    #[test]
    fn seed_horizon_drops_old_text_keeps_recent_and_unknown() {
        let r = MsgSecretRetention::default();
        let now = 100 * DAY;
        // 40 days old text is past the 30-day text horizon.
        assert!(!within_seed_horizon(
            &r,
            RetentionClass::Text,
            Some((60 * DAY) as u64),
            now
        ));
        // 10 days old text is still within.
        assert!(within_seed_horizon(
            &r,
            RetentionClass::Text,
            Some((90 * DAY) as u64),
            now
        ));
        // 40 days old poll is still within the 90-day poll horizon.
        assert!(within_seed_horizon(
            &r,
            RetentionClass::PollEvent,
            Some((60 * DAY) as u64),
            now
        ));
        // Unknown age is conservatively kept.
        assert!(within_seed_horizon(&r, RetentionClass::Text, None, now));
        // `Some(0)` is the same "unknown" sentinel `expires_at` reads, so it is
        // kept too. Reading it as Unix time zero would reject the record as
        // decades old and drop a secret the write path would have kept.
        assert!(within_seed_horizon(&r, RetentionClass::Text, Some(0), now));
    }

    #[test]
    fn classify_from_flags_precedence() {
        assert_eq!(classify_from_flags(true, true), RetentionClass::Bot);
        assert_eq!(classify_from_flags(true, false), RetentionClass::Bot);
        assert_eq!(classify_from_flags(false, true), RetentionClass::PollEvent);
        assert_eq!(classify_from_flags(false, false), RetentionClass::Text);
    }

    #[test]
    fn is_bot_context_detects_chat_and_invocation() {
        // Chat is a bot.
        assert!(is_bot_context(true, &wa::Message::default()));
        // bot_metadata on a non-bot (e.g. group) chat is still a bot context.
        let prompt = wa::Message {
            message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
                bot_metadata: buffa::MessageField::some(wa::BotMetadata::default()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(is_bot_context(false, &prompt));
        // A plain message in a non-bot chat is not a bot context.
        let plain = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        assert!(!is_bot_context(false, &plain));
    }

    #[test]
    fn policy_predicates() {
        assert!(MsgSecretPolicy::Managed.persists());
        assert!(MsgSecretPolicy::Managed.prunes());
        assert!(MsgSecretPolicy::BotOnly.prunes());
        assert!(!MsgSecretPolicy::Full.prunes());
        // Disabled writes nothing but still reaps legacy rows.
        assert!(MsgSecretPolicy::Disabled.prunes());
        assert!(!MsgSecretPolicy::Disabled.persists());
        assert!(MsgSecretPolicy::BotOnly.bot_only());
    }
}
