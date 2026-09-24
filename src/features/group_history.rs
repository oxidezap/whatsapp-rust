use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use thiserror::Error;
use wacore::{
    iq::groups::MemberShareHistoryMode,
    iq::{abprops, props::GroupPropsResponse},
    store::ab_props::AbPropsSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupHistorySkipReason {
    NoOptedInSuccessfulRecipients,
    SenderNotAuthorized,
    UnsupportedGroup,
    AccountPropsUnavailable,
    GroupMetadataUnavailable,
    GroupPropsUnavailable,
    InvalidProperties,
    SharingDisabled,
    NoEligibleMessages,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GroupHistoryLimits {
    pub max_messages: usize,
    pub time_window_seconds: u64,
}

/// Opaque in-memory retry state for an indeterminate or partial share.
///
/// It retains the exact encrypted-upload reference and message IDs so retrying
/// does not repackage history or change its pairwise audience. Treat it as
/// sensitive; its `Debug` output deliberately omits message keys and contents.
/// Pass it to [`crate::features::Groups::retry_group_history`] rather than
/// constructing or inspecting it. It is not a persistence format.
#[derive(Clone)]
pub struct GroupHistoryRetryToken {
    pub(crate) group: wacore_binary::Jid,
    pub(crate) recipients: Vec<wacore_binary::Jid>,
    pub(crate) bundle_message: Arc<waproto::whatsapp::Message>,
    pub(crate) notice_message: Arc<waproto::whatsapp::Message>,
    pub(crate) bundle_message_id: String,
    pub(crate) notice_message_id: String,
    pub(crate) message_count: usize,
    stage: GroupHistoryRetryStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupHistoryRetryStage {
    Bundle,
    Notice,
}

impl GroupHistoryRetryToken {
    pub(crate) fn bundle(
        group: &wacore_binary::Jid,
        recipients: &[wacore_binary::Jid],
        bundle_message: Arc<waproto::whatsapp::Message>,
        notice_message: Arc<waproto::whatsapp::Message>,
        bundle_message_id: String,
        notice_message_id: String,
        message_count: usize,
    ) -> Self {
        Self {
            group: group.clone(),
            recipients: recipients.to_vec(),
            bundle_message,
            notice_message,
            bundle_message_id,
            notice_message_id,
            message_count,
            stage: GroupHistoryRetryStage::Bundle,
        }
    }

    pub(crate) fn fits_current_limits(&self, limits: GroupHistoryLimits, now: u64) -> bool {
        let Some(bundle) = self.bundle_message.message_history_bundle.as_option() else {
            return false;
        };
        let Some(metadata) = bundle.message_history_metadata.as_option() else {
            return false;
        };
        retained_history_fits_limits(metadata, limits, now)
    }

    pub(crate) fn is_notice_stage(&self) -> bool {
        self.stage == GroupHistoryRetryStage::Notice
    }

    pub(crate) fn for_notice(&self) -> Self {
        let mut retry = self.clone();
        retry.stage = GroupHistoryRetryStage::Notice;
        retry
    }
}

fn retained_history_fits_limits(
    metadata: &waproto::whatsapp::message::MessageHistoryMetadata,
    limits: GroupHistoryLimits,
    now: u64,
) -> bool {
    let count = metadata
        .message_count
        .and_then(|value| usize::try_from(value).ok());
    let oldest = metadata
        .oldest_message_timestamp_in_bundle
        .and_then(|value| u64::try_from(value).ok());
    count.is_some_and(|count| count > 0 && count <= limits.max_messages)
        && oldest.is_some_and(|oldest| {
            oldest >= now.saturating_sub(limits.time_window_seconds) && oldest <= now
        })
}

impl PartialEq for GroupHistoryRetryToken {
    fn eq(&self, other: &Self) -> bool {
        self.group == other.group
            && self.recipients == other.recipients
            && self.bundle_message_id == other.bundle_message_id
            && self.notice_message_id == other.notice_message_id
            && self.message_count == other.message_count
            && self.stage == other.stage
    }
}

impl Eq for GroupHistoryRetryToken {}

impl fmt::Debug for GroupHistoryRetryToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupHistoryRetryToken")
            .field("stage", &self.stage)
            .field("message_count", &self.message_count)
            .field("recipient_count", &self.recipients.len())
            .field("media_keys", &"<redacted>")
            .finish()
    }
}

/// The add operation's history-sharing result, independent of participant-add
/// success. Indeterminate and partial outcomes carry exact retry state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupHistoryShareOutcome {
    NotRequested,
    Skipped(GroupHistorySkipReason),
    PreparationFailed {
        error: String,
    },
    UploadFailed {
        error: String,
    },
    /// A local/typed failure occurred before the bundle stanza was sent.
    BundleNotSent {
        bundle_message_id: String,
        error: String,
        retry: GroupHistoryRetryToken,
    },
    /// The correlated server ACK explicitly rejected the bundle.
    BundleRejected {
        bundle_message_id: String,
        error: Option<String>,
        code: Option<String>,
    },
    /// No trustworthy correlated ACK was observed; the server may have accepted it.
    BundleIndeterminate {
        bundle_message_id: String,
        retry: GroupHistoryRetryToken,
    },
    /// Server ACKed the stanza, but local encryption missed one or more devices.
    /// This does not claim recipient receipt; the retry token reuses the same ID.
    BundlePartialFanout {
        bundle_message_id: String,
        /// Devices that produced encrypted fanout nodes.
        encrypted_devices: usize,
        /// Opted-in receivers plus own companion devices; excludes this device.
        addressed_devices: usize,
        retry: GroupHistoryRetryToken,
    },
    /// The bundle was ACKed, but a local/typed failure prevented sending notice.
    NoticeNotSent {
        bundle_message_id: String,
        notice_message_id: String,
        error: String,
        retry: GroupHistoryRetryToken,
    },
    /// The bundle was ACKed, but the correlated notice ACK explicitly rejected.
    NoticeRejected {
        bundle_message_id: String,
        notice_message_id: String,
        error: Option<String>,
        code: Option<String>,
    },
    /// The bundle was ACKed, but no trustworthy correlated notice ACK was observed.
    NoticeIndeterminate {
        bundle_message_id: String,
        notice_message_id: String,
        retry: GroupHistoryRetryToken,
    },
    /// The notice was ACKed, but local encryption missed one or more devices.
    /// This does not claim recipient receipt; the retry token reuses the same ID.
    NoticePartialFanout {
        bundle_message_id: String,
        notice_message_id: String,
        /// Devices that produced encrypted fanout nodes.
        encrypted_devices: usize,
        /// Opted-in receivers plus own companion devices; excludes this device.
        addressed_devices: usize,
        retry: GroupHistoryRetryToken,
    },
    /// Bundle and notice ACKs were correlated, with complete local device fanout.
    /// This is server acceptance, not a read or receipt confirmation.
    Shared {
        bundle_message_id: String,
        notice_message_id: String,
        message_count: usize,
    },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum GroupHistoryPolicyError {
    #[error("account AB properties have not been applied in this connection")]
    AccountPropsUnavailable,
    #[error("group AB properties are not a complete, versioned response")]
    GroupPropsUnavailable,
    #[error("account AB property {0} has an invalid value")]
    InvalidAccountProp(&'static str),
    #[error("group AB property {0} has an invalid value")]
    InvalidGroupProp(&'static str),
    #[error("history sharing is disabled by both account and group AB properties")]
    SharingDisabled,
    #[error("history AB property {0} has an invalid or unsupported limit")]
    InvalidLimit(&'static str),
}

/// Mirror WA Web's `canCurrentUserShareHistory`: admins and superadmins can
/// always share, while ordinary members require the explicit all-member mode.
pub(crate) fn can_current_user_share_history(
    is_admin: bool,
    is_super_admin: bool,
    mode: Option<MemberShareHistoryMode>,
) -> bool {
    is_admin || is_super_admin || mode == Some(MemberShareHistoryMode::AllMemberShare)
}

pub(crate) struct SelectedGroupHistory {
    pub messages: Vec<waproto::whatsapp::WebMessageInfo>,
    pub oldest_timestamp: u64,
}

/// Keep only well-formed messages from this exact group which fit both
/// effective AB-prop bounds. The consumer's storage remains authoritative; no
/// message is loaded or persisted by this selection step.
pub(crate) fn select_group_history_messages(
    group: &wacore_binary::Jid,
    messages: &[waproto::whatsapp::WebMessageInfo],
    now: u64,
    limits: GroupHistoryLimits,
) -> Option<SelectedGroupHistory> {
    let window_start = now.saturating_sub(limits.time_window_seconds);
    let group = group.to_string();
    let mut selected = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for message in messages {
        let Some(key) = message.key.as_option() else {
            continue;
        };
        let Some(id) = key.id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        if key.remote_jid.as_deref() != Some(group.as_str()) || !message.message.is_set() {
            continue;
        }
        let Some(timestamp) = message.message_timestamp else {
            continue;
        };
        if timestamp < window_start || timestamp > now || !seen_ids.insert(id.to_owned()) {
            continue;
        }
        selected.push((timestamp, message.clone()));
    }
    selected.sort_by_key(|(timestamp, _)| *timestamp);
    if limits.max_messages == 0 || selected.is_empty() {
        return None;
    }
    if selected.len() > limits.max_messages {
        selected.drain(..selected.len() - limits.max_messages);
    }
    let oldest_timestamp = selected.first()?.0;
    Some(SelectedGroupHistory {
        messages: selected.into_iter().map(|(_, message)| message).collect(),
        oldest_timestamp,
    })
}

/// Resolve the effective WA Web sender gate and history bounds from the exact
/// AB-prop inputs WA Web uses. A missing group entry uses its registry default
/// only after a successful, versioned group-props response; an absent response
/// is unknown and must never authorize a send.
pub(crate) fn resolve_group_history_limits(
    account: &AbPropsSnapshot,
    group: &GroupPropsResponse,
) -> Result<GroupHistoryLimits, GroupHistoryPolicyError> {
    if !account.applied_in_generation() {
        return Err(GroupHistoryPolicyError::AccountPropsUnavailable);
    }
    if group.hash.as_deref().is_none_or(str::is_empty) {
        return Err(GroupHistoryPolicyError::GroupPropsUnavailable);
    }

    let group_values = group
        .experiment_props
        .iter()
        .map(|(code, value)| (*code, value.as_str()))
        .collect::<HashMap<_, _>>();

    let account_enabled = account.get_bool(abprops::web::GROUP_HISTORY_SEND).ok_or(
        GroupHistoryPolicyError::InvalidAccountProp("group_history_send"),
    )?;
    let group_enabled = group_bool(
        &group_values,
        abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL,
    )?;
    if !account_enabled && !group_enabled {
        return Err(GroupHistoryPolicyError::SharingDisabled);
    }

    let account_window = account
        .get_int(abprops::web::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS)
        .ok_or(GroupHistoryPolicyError::InvalidLimit(
            "group_history_messages_time_limit_secs",
        ))?;
    let default_window = match abprops::web::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS.default {
        abprops::AbDefault::Int(value) => value,
        _ => unreachable!("generated account history window has integer default"),
    };
    let effective_window = if account_window != default_window {
        account_window
    } else {
        group_int(
            &group_values,
            abprops::group::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS_GROUP_LEVEL,
        )?
    };
    let time_window_seconds = u64::try_from(effective_window)
        .ok()
        .filter(|v| *v > 0)
        .ok_or(GroupHistoryPolicyError::InvalidLimit(
            "group_history_messages_time_limit_secs_group_level",
        ))?;

    let max_messages = account
        .get_int(abprops::web::GROUP_HISTORY_MESSAGE_COUNT_LIMIT)
        .and_then(|count| usize::try_from(count).ok())
        .ok_or(GroupHistoryPolicyError::InvalidLimit(
            "group_history_message_count_limit",
        ))?;

    Ok(GroupHistoryLimits {
        max_messages,
        time_window_seconds,
    })
}

fn group_bool(
    values: &HashMap<u32, &str>,
    prop: abprops::AbProp,
) -> Result<bool, GroupHistoryPolicyError> {
    let default = match prop.default {
        abprops::AbDefault::Bool(value) => value,
        _ => unreachable!("generated group history sender flag has boolean default"),
    };
    let Some(value) = values.get(&prop.code) else {
        return Ok(default);
    };
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "enabled" => Ok(true),
        "0" | "false" | "disabled" => Ok(false),
        _ => Err(GroupHistoryPolicyError::InvalidGroupProp(prop.name)),
    }
}

fn group_int(
    values: &HashMap<u32, &str>,
    prop: abprops::AbProp,
) -> Result<i64, GroupHistoryPolicyError> {
    let default = match prop.default {
        abprops::AbDefault::Int(value) => value,
        _ => unreachable!("generated group history limit has integer default"),
    };
    values.get(&prop.code).map_or(Ok(default), |value| {
        value
            .parse()
            .map_err(|_| GroupHistoryPolicyError::InvalidGroupProp(prop.name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::store::ab_props::AbPropsCache;
    use wacore_binary::CompactString;

    #[test]
    fn group_history_retry_checks_current_count_and_window() {
        let metadata = waproto::whatsapp::message::MessageHistoryMetadata {
            message_count: Some(100),
            oldest_message_timestamp_in_bundle: Some(800),
            ..Default::default()
        };
        let limits = GroupHistoryLimits {
            max_messages: 100,
            time_window_seconds: 200,
        };
        assert!(retained_history_fits_limits(&metadata, limits, 1000));
        assert!(!retained_history_fits_limits(
            &metadata,
            GroupHistoryLimits {
                max_messages: 0,
                ..limits
            },
            1000
        ));
        assert!(!retained_history_fits_limits(
            &metadata,
            GroupHistoryLimits {
                time_window_seconds: 199,
                ..limits
            },
            1000
        ));
        assert!(!retained_history_fits_limits(&metadata, limits, 1001));
        assert!(!retained_history_fits_limits(
            &Default::default(),
            limits,
            1000
        ));
    }

    async fn account_snapshot(props: &[(u32, &str)]) -> AbPropsSnapshot {
        let cache = AbPropsCache::new();
        cache.begin_generation(1).await;
        cache
            .apply_props(
                false,
                props
                    .iter()
                    .map(|(code, value)| (*code, CompactString::from(*value))),
            )
            .await;
        cache.snapshot().await
    }

    fn group_props(props: &[(u32, &str)]) -> GroupPropsResponse {
        GroupPropsResponse {
            hash: Some("synthetic-group-hash".into()),
            experiment_props: props
                .iter()
                .map(|(code, value)| (*code, CompactString::from(*value)))
                .collect(),
        }
    }

    #[test]
    fn retry_token_preserves_stage_without_exposing_media_keys_in_debug() {
        use waproto::whatsapp as wa;

        let group = wacore_binary::Jid::new("120363000000000001", wacore_binary::Server::Group);
        let receiver = wacore_binary::Jid::pn("111111111111");
        let bundle_message = Arc::new(wa::Message {
            message_history_bundle: buffa::MessageField::some(wa::message::MessageHistoryBundle {
                media_key: Some(b"SYNTHETIC-MEDIA-KEY".to_vec()),
                ..Default::default()
            }),
            ..Default::default()
        });
        let token = GroupHistoryRetryToken::bundle(
            &group,
            std::slice::from_ref(&receiver),
            Arc::clone(&bundle_message),
            Arc::new(wa::Message::default()),
            "SYNTHETIC-BUNDLE-ID".into(),
            "SYNTHETIC-NOTICE-ID".into(),
            1,
        );
        let notice_retry = token.for_notice();

        assert!(!token.is_notice_stage());
        assert!(notice_retry.is_notice_stage());
        assert_ne!(token, notice_retry);
        let debug = format!("{token:?}");
        assert!(!debug.contains("SYNTHETIC-MEDIA-KEY"));
        assert!(!debug.contains("SYNTHETIC-BUNDLE-ID"));
    }

    #[test]
    fn history_permission_matches_wa_web_role_and_member_mode_gate() {
        assert!(can_current_user_share_history(true, false, None));
        assert!(can_current_user_share_history(false, true, None));
        assert!(can_current_user_share_history(
            false,
            false,
            Some(MemberShareHistoryMode::AllMemberShare)
        ));
        assert!(!can_current_user_share_history(false, false, None));
        assert!(!can_current_user_share_history(
            false,
            false,
            Some(MemberShareHistoryMode::AdminShare)
        ));
    }

    #[test]
    fn message_selection_enforces_group_time_and_count_bounds() {
        use waproto::whatsapp as wa;

        let group = wacore_binary::Jid::new("120363000000000001", wacore_binary::Server::Group);
        let other_group =
            wacore_binary::Jid::new("120363000000000002", wacore_binary::Server::Group);
        let make_message = |id: &str, remote: &wacore_binary::Jid, timestamp| wa::WebMessageInfo {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(remote.to_string()),
                id: Some(id.into()),
                ..Default::default()
            }),
            message: buffa::MessageField::some(wa::Message {
                conversation: Some("synthetic test message".into()),
                ..Default::default()
            }),
            message_timestamp: Some(timestamp),
            ..Default::default()
        };
        let messages = vec![
            make_message("old", &group, 799),
            make_message("oldest-in-window", &group, 800),
            make_message("newer", &group, 900),
            make_message("latest", &group, 950),
            make_message("future", &group, 1001),
            make_message("other-group", &other_group, 999),
        ];

        let selected = select_group_history_messages(
            &group,
            &messages,
            1000,
            GroupHistoryLimits {
                max_messages: 2,
                time_window_seconds: 200,
            },
        )
        .unwrap();
        assert_eq!(selected.oldest_timestamp, 900);
        assert_eq!(selected.messages.len(), 2);
        assert_eq!(
            selected.messages[0]
                .key
                .as_option()
                .and_then(|key| key.id.as_deref()),
            Some("newer")
        );
        assert_eq!(
            selected.messages[1]
                .key
                .as_option()
                .and_then(|key| key.id.as_deref()),
            Some("latest")
        );
        assert!(
            select_group_history_messages(
                &group,
                &messages,
                1000,
                GroupHistoryLimits {
                    max_messages: 0,
                    time_window_seconds: 200,
                },
            )
            .is_none()
        );
    }

    #[tokio::test]
    async fn account_sender_gate_and_nondefault_account_window_override_group_values() {
        let account = account_snapshot(&[
            (abprops::web::GROUP_HISTORY_SEND.code, "1"),
            (
                abprops::web::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS.code,
                "3600",
            ),
            (abprops::web::GROUP_HISTORY_MESSAGE_COUNT_LIMIT.code, "37"),
        ])
        .await;
        let group = group_props(&[
            (abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code, "0"),
            (
                abprops::group::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS_GROUP_LEVEL.code,
                "7200",
            ),
        ]);

        assert_eq!(
            resolve_group_history_limits(&account, &group),
            Ok(GroupHistoryLimits {
                max_messages: 37,
                time_window_seconds: 3600,
            })
        );
    }

    #[tokio::test]
    async fn group_sender_override_and_group_specific_window_are_used() {
        let account = account_snapshot(&[]).await;
        let group = group_props(&[
            (abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code, "true"),
            (
                abprops::group::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS_GROUP_LEVEL.code,
                "86400",
            ),
        ]);

        assert_eq!(
            resolve_group_history_limits(&account, &group),
            Ok(GroupHistoryLimits {
                max_messages: 100,
                time_window_seconds: 86400,
            })
        );
    }

    #[tokio::test]
    async fn disabled_unknown_and_malformed_authorization_fail_closed() {
        let account = account_snapshot(&[]).await;
        let group = group_props(&[]);
        assert_eq!(
            resolve_group_history_limits(&account, &group),
            Err(GroupHistoryPolicyError::SharingDisabled)
        );
        assert_eq!(
            resolve_group_history_limits(
                &account,
                &GroupPropsResponse {
                    hash: None,
                    experiment_props: Vec::new(),
                }
            ),
            Err(GroupHistoryPolicyError::GroupPropsUnavailable)
        );
        assert_eq!(
            resolve_group_history_limits(
                &account,
                &group_props(&[(abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code, "maybe")])
            ),
            Err(GroupHistoryPolicyError::InvalidGroupProp(
                "group_history_send_group_level"
            ))
        );

        let malformed_account =
            account_snapshot(&[(abprops::web::GROUP_HISTORY_SEND.code, "maybe")]).await;
        assert_eq!(
            resolve_group_history_limits(
                &malformed_account,
                &group_props(&[(abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code, "1")])
            ),
            Err(GroupHistoryPolicyError::InvalidAccountProp(
                "group_history_send"
            ))
        );
    }

    #[tokio::test]
    async fn account_props_not_applied_in_current_generation_fail_closed() {
        let cache = AbPropsCache::new();
        let account = cache.snapshot().await;
        let group = group_props(&[(abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code, "1")]);
        assert_eq!(
            resolve_group_history_limits(&account, &group),
            Err(GroupHistoryPolicyError::AccountPropsUnavailable)
        );
    }
}
