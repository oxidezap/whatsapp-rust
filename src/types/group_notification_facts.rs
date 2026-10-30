//! Effective account facts for opting into source-qualified group parsing.

/// One coherent snapshot of the effective account gates used by group parsing.
/// Supply actual account values; protocol feature-flag defaults do not establish
/// that a gate is enabled for the current account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct GroupNotificationFacts {
    pub username_displayed: bool,
    pub history_after_join: bool,
    pub opus_enabled: bool,
}

/// Optional synchronous facts for the currently authenticated account.
///
/// The provider must belong to this client's account. `snapshot` must read a
/// coherent effective configuration without I/O. The country-code method must
/// be a pure lookup matching the official registry: return its normalized value,
/// or `None` for an unknown code. Parsing calls it only on source-gated paths.
/// Neither method should send messages or mutate protocol/persisted state.
/// Register before receiving notifications to use contextual parsing throughout
/// a connection. Without a provider the existing legacy parser remains active.
/// Leave it unregistered when effective gates are unavailable rather than
/// inventing values.
pub trait GroupNotificationFactsProvider: Send + Sync {
    fn snapshot(&self) -> GroupNotificationFacts;
    fn normalize_country_code(&self, code: &str) -> Option<String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("group notification facts provider is already registered")]
pub struct GroupNotificationFactsAlreadySet;
