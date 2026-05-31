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
//! event time, per add-on kind, via [`expires_at`].

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

    /// Whether the periodic prune sweep should run. `Full` keeps everything;
    /// `Disabled` writes nothing to prune.
    pub fn prunes(self) -> bool {
        matches!(self, MsgSecretPolicy::Managed | MsgSecretPolicy::BotOnly)
    }
}

/// Per-add-on-kind retention horizons applied to the parent message's event
/// time. Defaults are derived from verified protocol limits, not guesses.
#[derive(Debug, Clone, Copy)]
pub struct MsgSecretRetention {
    /// Text / `MESSAGE_EDIT` parents. An edit is sender-gated to 20 min of the
    /// parent, but WhatsApp's offline queue can deliver that edit up to ~30
    /// days later, so a 30-day-offline receiver still needs the secret.
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

/// Retention class of a stored secret, fixed at write time by which call site
/// wrote it (the store cannot see the add-on kind at prune time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionClass {
    Text,
    PollEvent,
    Bot,
}

/// Classify a message for retention. Bot-chat secrets always take the bot
/// horizon; otherwise poll/event parents get the longer horizon and everything
/// else is text. Unwraps device-sent/ephemeral/etc. wrappers first.
pub fn classify(msg: &wa::Message, is_bot_chat: bool) -> RetentionClass {
    if is_bot_chat {
        return RetentionClass::Bot;
    }
    let base = msg.get_base_message();
    if base.poll_creation_message.is_some()
        || base.poll_creation_message_v2.is_some()
        || base.poll_creation_message_v3.is_some()
        || base.event_message.is_some()
    {
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
pub fn expires_at(
    policy: MsgSecretPolicy,
    retention: &MsgSecretRetention,
    class: RetentionClass,
    message_ts: Option<u64>,
    now: i64,
) -> i64 {
    if !policy.prunes() {
        return 0;
    }
    let base = message_ts
        .and_then(|t| i64::try_from(t).ok())
        .unwrap_or(now);
    let horizon = i64::try_from(retention.horizon_secs(class)).unwrap_or(i64::MAX);
    base.saturating_add(horizon)
}

/// Whether a history-sync secret with parent event time `message_ts` is still
/// within its retention horizon at `now` and is worth seeding. Records with no
/// timestamp are kept (we cannot prove they are too old). Only `Managed`/
/// `BotOnly` filter; `Full` seeds everything and `Disabled` seeds nothing
/// (decided by the caller, not here).
pub fn within_seed_horizon(
    retention: &MsgSecretRetention,
    class: RetentionClass,
    message_ts: Option<u64>,
    now: i64,
) -> bool {
    let Some(ts) = message_ts.and_then(|t| i64::try_from(t).ok()) else {
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
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait OriginalMessageResolver: Send + Sync {
    async fn resolve_msg_secret(&self, chat: &str, sender: &str, msg_id: &str) -> Option<[u8; 32]>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

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
    }

    #[test]
    fn policy_predicates() {
        assert!(MsgSecretPolicy::Managed.persists());
        assert!(MsgSecretPolicy::Managed.prunes());
        assert!(!MsgSecretPolicy::Full.prunes());
        assert!(!MsgSecretPolicy::Disabled.persists());
        assert!(MsgSecretPolicy::BotOnly.bot_only());
    }
}
