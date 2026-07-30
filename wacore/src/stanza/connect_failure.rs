//! `<failure>` stanza — the server's connect-failure / enforcement notice.
//!
//! Mirrors WA Web's `failureParser` in `WAWebHandleFailure`, which reads:
//! `reason` (required, 400..=599), `location`, `code`, `expire`, `message`,
//! `url`, `logout_message_header`, `logout_message_subtext` and
//! `logout_message_locale`.
//!
//! WA Web ignores every other attribute, but the server does not stop at that
//! set: an account lock also carries operator data for the native clients
//! (`appeal_token`, `violation_reason`, `vt`, …), delivered exactly once and
//! unrecoverable if dropped. That is why this parser is a *view* over the node
//! rather than a replacement for it — the caller keeps the whole stanza on the
//! event and the typed fields below are only the ones WA Web itself acts on.

use crate::types::events::ConnectFailureReason;
use std::borrow::Cow;
use wacore_binary::NodeRef;

/// The attributes WA Web's `failureParser` reads out of `<failure>`.
///
/// Borrows from the stanza; nothing is copied until the caller keeps a field.
#[derive(Debug, Clone, Default)]
pub struct ConnectFailureStanza<'a> {
    /// `reason`, the failure code. `None` only when the attribute is absent or
    /// unparseable — a code outside WA Web's accepted 400..=599 is kept as
    /// [`ConnectFailureReason::Unknown`], not discarded. WA Web can afford to
    /// drop such a stanza because it has a UI to fall back on; here the exact
    /// code is the only thing that would tell an operator what the server
    /// actually said, and whatsmeow keeps it for the same reason.
    pub reason: Option<ConnectFailureReason>,
    /// `location` — a region routing token (e.g. `"rva"`), never the cause.
    /// Diagnostic only; WA Web parses it and only ever logs it.
    pub location: Option<Cow<'a, str>>,
    /// `code` — the ban sub-reason, present on `reason=402`.
    pub code: Option<i32>,
    /// `expire` — how long the temporary ban lasts, in seconds. A duration, not
    /// a timestamp: WA Web's ban screen renders it as
    /// `moment.duration(expire, "seconds").humanize()` in "You'll be able to
    /// use WhatsApp again in {duration}".
    pub expire: Option<u64>,
    /// `message` — free-text detail, not localized.
    pub message: Option<Cow<'a, str>>,
    /// `url` — the link the ban screen's "Learn more" opens. WA Web falls back
    /// to the generic FAQ URL when the server sends none.
    pub url: Option<Cow<'a, str>>,
    /// `logout_message_header`.
    pub logout_message_header: Option<Cow<'a, str>>,
    /// `logout_message_subtext`.
    pub logout_message_subtext: Option<Cow<'a, str>>,
    /// `logout_message_locale` — the locale the header/subtext are written in.
    pub logout_message_locale: Option<Cow<'a, str>>,
}

impl<'a> ConnectFailureStanza<'a> {
    /// Parse `<failure>`'s attributes. Unparseable numerics are reported as
    /// absent rather than as a sentinel: the caller must not be able to confuse
    /// "server said 0" with "server said nothing".
    pub fn parse(node: &'a NodeRef<'_>) -> Self {
        let mut attrs = node.attrs();
        Self {
            reason: attrs
                .optional_u64("reason")
                .and_then(|r| i32::try_from(r).ok())
                .map(ConnectFailureReason::from),
            location: attrs.optional_string("location"),
            code: attrs
                .optional_u64("code")
                .and_then(|c| i32::try_from(c).ok()),
            expire: attrs.optional_u64("expire"),
            message: attrs.optional_string("message"),
            url: attrs.optional_string("url"),
            logout_message_header: attrs.optional_string("logout_message_header"),
            logout_message_subtext: attrs.optional_string("logout_message_subtext"),
            logout_message_locale: attrs.optional_string("logout_message_locale"),
        }
    }

    /// The server-supplied logout text, when it sent any.
    ///
    /// WA Web builds this object only when a header or subtext is present, and
    /// then renders it only if `logout_message_locale` equals the client's
    /// current locale — the locale is what makes the text safe to show, so it
    /// travels with it instead of being dropped.
    pub fn logout_message(&self) -> Option<crate::types::events::LogoutMessage> {
        if self.logout_message_header.is_none() && self.logout_message_subtext.is_none() {
            return None;
        }
        Some(
            crate::types::events::LogoutMessage::builder()
                .maybe_header(self.logout_message_header.as_deref().map(str::to_owned))
                .maybe_subtext(self.logout_message_subtext.as_deref().map(str::to_owned))
                .maybe_locale(self.logout_message_locale.as_deref().map(str::to_owned))
                .build(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn parses_every_attribute_wa_web_reads() {
        let node = NodeBuilder::new("failure")
            .attr("reason", "402")
            .attr("location", "frc")
            .attr("code", "101")
            // A duration, not a deadline: 6 hours of ban.
            .attr("expire", "21600")
            .attr("message", "banned")
            .attr("url", "https://example.invalid/appeal")
            .attr("logout_message_header", "Conta desconectada")
            .attr("logout_message_subtext", "Verifique o app no celular")
            .attr("logout_message_locale", "pt_BR")
            .build();

        let node_ref = node.as_node_ref();
        let parsed = ConnectFailureStanza::parse(&node_ref);

        assert_eq!(parsed.reason, Some(ConnectFailureReason::TempBanned));
        assert_eq!(parsed.location.as_deref(), Some("frc"));
        assert_eq!(parsed.code, Some(101));
        assert_eq!(parsed.expire, Some(21_600));
        assert_eq!(parsed.message.as_deref(), Some("banned"));
        assert_eq!(
            parsed.url.as_deref(),
            Some("https://example.invalid/appeal")
        );

        let msg = parsed.logout_message().expect("header/subtext present");
        assert_eq!(msg.header.as_deref(), Some("Conta desconectada"));
        assert_eq!(msg.subtext.as_deref(), Some("Verifique o app no celular"));
        assert_eq!(msg.locale.as_deref(), Some("pt_BR"));
    }

    /// Absence must stay distinguishable from zero — a temporary ban with no
    /// `expire` is not a ban that expired at the epoch.
    #[test]
    fn absent_attributes_stay_absent() {
        let node = NodeBuilder::new("failure").attr("reason", "401").build();
        let node_ref = node.as_node_ref();
        let parsed = ConnectFailureStanza::parse(&node_ref);

        assert_eq!(parsed.reason, Some(ConnectFailureReason::LoggedOut));
        assert_eq!(parsed.code, None);
        assert_eq!(parsed.expire, None);
        assert!(parsed.logout_message().is_none());
    }

    /// A `<failure>` with no `reason` is reported as such; the client, not the
    /// parser, decides what an unattributed failure means.
    #[test]
    fn missing_reason_is_none_not_zero() {
        let node = NodeBuilder::new("failure").attr("location", "rva").build();
        let node_ref = node.as_node_ref();
        assert_eq!(ConnectFailureStanza::parse(&node_ref).reason, None);
    }

    /// A code outside WA Web's 400..=599 is kept, not dropped. Filtering it to
    /// `None` would collapse into the same `Unknown(0)` an absent attribute
    /// produces, and the exact code is the only description of what the server
    /// refused with — the one thing this whole module exists to preserve.
    #[test]
    fn out_of_range_reason_is_preserved_as_unknown() {
        let node = NodeBuilder::new("failure").attr("reason", "600").build();
        let node_ref = node.as_node_ref();
        assert_eq!(
            ConnectFailureStanza::parse(&node_ref).reason,
            Some(ConnectFailureReason::Unknown(600))
        );
    }

    /// The locale alone is not a message: WA Web only builds the object when a
    /// header or subtext exists.
    #[test]
    fn locale_alone_is_not_a_logout_message() {
        let node = NodeBuilder::new("failure")
            .attr("reason", "403")
            .attr("logout_message_locale", "en")
            .build();
        let node_ref = node.as_node_ref();
        assert!(
            ConnectFailureStanza::parse(&node_ref)
                .logout_message()
                .is_none()
        );
    }
}
