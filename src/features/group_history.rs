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

/// Opaque in-memory retry state for a failed upload or incomplete share.
///
/// Before upload it retains the compressed, selected protobuf and audience so
/// upload can be retried without adding members again. Afterwards it retains
/// the exact encrypted-upload reference and message IDs so retransmission does
/// not repackage history or change its pairwise audience. Treat it as
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
    pub(crate) prepared_upload: Option<Arc<Vec<u8>>>,
    stage: GroupHistoryRetryStage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupHistoryRetryStage {
    Upload,
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
            prepared_upload: None,
            stage: GroupHistoryRetryStage::Bundle,
        }
    }

    pub(crate) fn upload(
        group: &wacore_binary::Jid,
        recipients: &[wacore_binary::Jid],
        compressed: Vec<u8>,
        notice_message: Arc<waproto::whatsapp::Message>,
        bundle_message_id: String,
        notice_message_id: String,
        message_count: usize,
    ) -> Self {
        Self {
            group: group.clone(),
            recipients: recipients.to_vec(),
            bundle_message: Arc::new(waproto::whatsapp::Message::default()),
            notice_message,
            bundle_message_id,
            notice_message_id,
            message_count,
            prepared_upload: Some(Arc::new(compressed)),
            stage: GroupHistoryRetryStage::Upload,
        }
    }

    pub(crate) fn prepared_upload(&self) -> Option<&[u8]> {
        self.prepared_upload.as_deref().map(Vec::as_slice)
    }

    pub(crate) fn fits_current_limits(&self, limits: GroupHistoryLimits, now: u64) -> bool {
        if self.stage == GroupHistoryRetryStage::Upload {
            return self
                .notice_message
                .message_history_notice
                .as_option()
                .and_then(|notice| notice.message_history_metadata.as_option())
                .is_some_and(|metadata| retained_history_fits_limits(metadata, limits, now));
        }
        group_history_bundle_fits_current_limits(&self.bundle_message, limits, now)
    }

    pub(crate) fn is_upload_stage(&self) -> bool {
        self.stage == GroupHistoryRetryStage::Upload
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

pub(crate) fn group_history_bundle_fits_current_limits(
    message: &waproto::whatsapp::Message,
    limits: GroupHistoryLimits,
    now: u64,
) -> bool {
    let Some(bundle) = message.message_history_bundle.as_option() else {
        return false;
    };
    let Some(metadata) = bundle.message_history_metadata.as_option() else {
        return false;
    };
    retained_history_fits_limits(metadata, limits, now)
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
        /// Resume the prepared share without re-adding participants.
        retry: GroupHistoryRetryToken,
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
    /// The server explicitly rejected a retryable request; retry under the
    /// same message ID after current policy has been verified again.
    BundleRetryableRejection {
        bundle_message_id: String,
        error: Option<String>,
        code: String,
        retry: GroupHistoryRetryToken,
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
    /// Retry only the notice; the bundle was already ACKed.
    NoticeRetryableRejection {
        bundle_message_id: String,
        notice_message_id: String,
        error: Option<String>,
        code: String,
        retry: GroupHistoryRetryToken,
    },
    /// The bundle was ACKed, but no trustworthy correlated notice ACK was observed.
    NoticeIndeterminate {
        bundle_message_id: String,
        notice_message_id: String,
        retry: GroupHistoryRetryToken,
    },
    /// Bundle and notice ACKs were correlated, with complete pairwise bundle
    /// fanout. The group-wide notice uses sender-key routing, so this proves
    /// server acceptance, not individual delivery or a read receipt.
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

fn is_shareable_history_text(content: &waproto::whatsapp::Message) -> bool {
    use waproto::whatsapp::{Message, MessageContextInfo};

    macro_rules! remaining_fields_are_empty {
        ($value:expr, $ty:ident, [$($allowed:ident),*], [$($field:ident => $empty:ident),* $(,)?]) => {{
            let $ty { $($allowed: _,)* $($field,)* } = $value;
            $($field.$empty())&&*
        }};
    }

    content
        .conversation
        .as_deref()
        .is_some_and(|text| !text.is_empty())
        && remaining_fields_are_empty!(
            content,
            Message,
            [conversation, message_context_info],
            [
                sender_key_distribution_message => is_unset,
                image_message => is_unset,
                contact_message => is_unset,
                location_message => is_unset,
                extended_text_message => is_unset,
                document_message => is_unset,
                audio_message => is_unset,
                video_message => is_unset,
                call => is_unset,
                chat => is_unset,
                protocol_message => is_unset,
                contacts_array_message => is_unset,
                highly_structured_message => is_unset,
                fast_ratchet_key_sender_key_distribution_message => is_unset,
                send_payment_message => is_unset,
                live_location_message => is_unset,
                request_payment_message => is_unset,
                decline_payment_request_message => is_unset,
                cancel_payment_request_message => is_unset,
                template_message => is_unset,
                sticker_message => is_unset,
                group_invite_message => is_unset,
                template_button_reply_message => is_unset,
                product_message => is_unset,
                device_sent_message => is_unset,
                list_message => is_unset,
                view_once_message => is_unset,
                order_message => is_unset,
                list_response_message => is_unset,
                ephemeral_message => is_unset,
                invoice_message => is_unset,
                buttons_message => is_unset,
                buttons_response_message => is_unset,
                payment_invite_message => is_unset,
                interactive_message => is_unset,
                reaction_message => is_unset,
                sticker_sync_rmr_message => is_unset,
                interactive_response_message => is_unset,
                poll_creation_message => is_unset,
                poll_update_message => is_unset,
                keep_in_chat_message => is_unset,
                document_with_caption_message => is_unset,
                request_phone_number_message => is_unset,
                view_once_message_v2 => is_unset,
                enc_reaction_message => is_unset,
                edited_message => is_unset,
                view_once_message_v2_extension => is_unset,
                poll_creation_message_v2 => is_unset,
                scheduled_call_creation_message => is_unset,
                group_mentioned_message => is_unset,
                pin_in_chat_message => is_unset,
                poll_creation_message_v3 => is_unset,
                scheduled_call_edit_message => is_unset,
                ptv_message => is_unset,
                bot_invoke_message => is_unset,
                call_log_messsage => is_unset,
                message_history_bundle => is_unset,
                enc_comment_message => is_unset,
                bcall_message => is_unset,
                lottie_sticker_message => is_unset,
                event_message => is_unset,
                enc_event_response_message => is_unset,
                comment_message => is_unset,
                newsletter_admin_invite_message => is_unset,
                placeholder_message => is_unset,
                secret_encrypted_message => is_unset,
                album_message => is_unset,
                event_cover_image => is_unset,
                sticker_pack_message => is_unset,
                status_mention_message => is_unset,
                poll_result_snapshot_message => is_unset,
                poll_creation_option_image_message => is_unset,
                associated_child_message => is_unset,
                group_status_mention_message => is_unset,
                poll_creation_message_v4 => is_unset,
                status_add_yours => is_unset,
                group_status_message => is_unset,
                rich_response_message => is_unset,
                status_notification_message => is_unset,
                limit_sharing_message => is_unset,
                bot_task_message => is_unset,
                question_message => is_unset,
                message_history_notice => is_unset,
                group_status_message_v2 => is_unset,
                bot_forwarded_message => is_unset,
                status_question_answer_message => is_unset,
                question_reply_message => is_unset,
                question_response_message => is_unset,
                status_quoted_message => is_unset,
                status_sticker_interaction_message => is_unset,
                poll_creation_message_v5 => is_unset,
                newsletter_follower_invite_message_v2 => is_unset,
                poll_result_snapshot_message_v3 => is_unset,
                newsletter_admin_profile_message => is_unset,
                newsletter_admin_profile_message_v2 => is_unset,
                spoiler_message => is_unset,
                poll_creation_message_v6 => is_unset,
                conditional_reveal_message => is_unset,
                poll_add_option_message => is_unset,
                event_invite_message => is_unset,
                group_root_key_share => is_unset,
                payment_reminder_message => is_unset,
                split_payment_message => is_unset,
                newsletter_admin_profile_status_message => is_unset,
                root_secret_distribute_message => is_unset,
                split_payment_update_message => is_unset,
                music_message => is_unset,
                status_link_preview_metadata => is_unset,
                bot_platform_registration_success_message => is_unset,
            ]
        )
        && content
            .message_context_info
            .as_option()
            .is_none_or(|context| {
                remaining_fields_are_empty!(
                    context,
                    MessageContextInfo,
                    [message_secret, reporting_token_version],
                    [
                        device_list_metadata => is_unset,
                        device_list_metadata_version => is_none,
                        padding_bytes => is_none,
                        message_add_on_duration_in_secs => is_none,
                        bot_message_secret => is_none,
                        bot_metadata => is_unset,
                        message_add_on_expiry_type => is_none,
                        message_association => is_unset,
                        capi_created_group => is_none,
                        support_payload => is_none,
                        limit_sharing => is_unset,
                        limit_sharing_v2 => is_unset,
                        thread_id => is_empty,
                        weblink_render_config => is_none,
                        tee_bot_metadata => is_none,
                        account_encryption_attestation => is_unset,
                        associated_primary_identity_key => is_none,
                    ]
                )
            })
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
    if limits.max_messages == 0 {
        return None;
    }
    // Only retain references to the newest permitted records; projecting every
    // eligible protobuf from an unbounded consumer archive wastes memory.
    let mut newest = std::collections::BinaryHeap::new();
    let mut seen_ids = std::collections::HashSet::new();
    for (index, message) in messages.iter().enumerate() {
        let Some(key) = message.key.as_option() else {
            continue;
        };
        let Some(id) = key.id.as_deref().filter(|id| !id.is_empty()) else {
            continue;
        };
        if key.remote_jid.as_deref() != Some(group.as_str()) || !message.message.is_set() {
            continue;
        }
        if !matches!(
            message.status,
            Some(
                waproto::whatsapp::web_message_info::Status::SERVER_ACK
                    | waproto::whatsapp::web_message_info::Status::DELIVERY_ACK
                    | waproto::whatsapp::web_message_info::Status::READ
                    | waproto::whatsapp::web_message_info::Status::PLAYED
            )
        ) {
            continue;
        }
        let Some(timestamp) = message.message_timestamp else {
            continue;
        };
        // Retry tokens retain an immutable compressed upload, so an ephemeral
        // record could expire after upload without a way to remove it. Until
        // expiry metadata is retained for every retransmission boundary,
        // deliberately exclude even not-yet-expired ephemeral records.
        if message.ephemeral_expiration_timestamp.is_some()
            || message.ephemeral_duration.is_some()
            || message.ephemeral_start_timestamp.is_some()
            || timestamp < window_start
            || timestamp > now
        {
            continue;
        }
        let Some(content) = message.message.as_option() else {
            continue;
        };
        if !is_shareable_history_text(content) || !seen_ids.insert(id) {
            continue;
        }
        let entry = std::cmp::Reverse((timestamp, index));
        if newest.len() < limits.max_messages {
            newest.push(entry);
        } else if newest.peek().is_some_and(|oldest| entry < *oldest) {
            newest.pop();
            newest.push(entry);
        }
    }
    let mut selected = newest.into_vec();
    selected.sort_by_key(|std::cmp::Reverse((timestamp, index))| (*timestamp, *index));
    let oldest_timestamp = selected.first()?.0.0;
    let messages = selected
        .into_iter()
        .map(|std::cmp::Reverse((timestamp, index))| {
            let source = &messages[index];
            let content = source.message.as_option()?;
            let context = content
                .message_context_info
                .as_option()
                .and_then(|context| {
                    context.message_secret.clone().map(|message_secret| {
                        buffa::MessageField::some(waproto::whatsapp::MessageContextInfo {
                            message_secret: Some(message_secret),
                            ..Default::default()
                        })
                    })
                })
                .unwrap_or_default();
            Some(waproto::whatsapp::WebMessageInfo {
                key: source.key.clone(),
                message: buffa::MessageField::some(waproto::whatsapp::Message {
                    conversation: content.conversation.clone(),
                    message_context_info: context,
                    ..Default::default()
                }),
                message_timestamp: Some(timestamp),
                status: source.status,
                participant: source.participant.clone(),
                ..Default::default()
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(SelectedGroupHistory {
        messages,
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

    let mut group_values = HashMap::new();
    for (code, value) in &group.experiment_props {
        if *code == 0
            || group_values
                .insert(*code, value.as_str())
                .is_some_and(|previous| previous != value.as_str())
        {
            return Err(GroupHistoryPolicyError::GroupPropsUnavailable);
        }
    }

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

        let upload_token = GroupHistoryRetryToken::upload(
            &group,
            std::slice::from_ref(&receiver),
            b"synthetic-compressed-history".to_vec(),
            Arc::new(wa::Message {
                message_history_notice: buffa::MessageField::some(
                    wa::message::MessageHistoryNotice {
                        message_history_metadata: buffa::MessageField::some(
                            wa::message::MessageHistoryMetadata {
                                message_count: Some(1),
                                oldest_message_timestamp_in_bundle: Some(900),
                                ..Default::default()
                            },
                        ),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            }),
            "SYNTHETIC-BUNDLE-ID".into(),
            "SYNTHETIC-NOTICE-ID".into(),
            1,
        );
        assert!(upload_token.is_upload_stage());
        assert_eq!(
            upload_token.prepared_upload(),
            Some(b"synthetic-compressed-history".as_slice())
        );
        assert_eq!(upload_token.recipients, vec![receiver]);
        assert!(upload_token.fits_current_limits(
            GroupHistoryLimits {
                max_messages: 1,
                time_window_seconds: 200
            },
            1000
        ));
        assert!(!upload_token.fits_current_limits(
            GroupHistoryLimits {
                max_messages: 1,
                time_window_seconds: 200
            },
            1101
        ));
        assert!(!format!("{upload_token:?}").contains("synthetic-compressed-history"));
    }

    #[test]
    fn retransmitted_bundle_must_fit_current_count_and_time_window() {
        use waproto::whatsapp as wa;

        let make_bundle = |count: Option<i64>, oldest: Option<i64>| wa::Message {
            message_history_bundle: buffa::MessageField::some(wa::message::MessageHistoryBundle {
                message_history_metadata: buffa::MessageField::some(
                    wa::message::MessageHistoryMetadata {
                        message_count: count,
                        oldest_message_timestamp_in_bundle: oldest,
                        ..Default::default()
                    },
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        let limits = GroupHistoryLimits {
            max_messages: 2,
            time_window_seconds: 200,
        };

        assert!(group_history_bundle_fits_current_limits(
            &make_bundle(Some(2), Some(900)),
            limits,
            1000,
        ));
        assert!(!group_history_bundle_fits_current_limits(
            &make_bundle(Some(3), Some(900)),
            limits,
            1000,
        ));
        assert!(!group_history_bundle_fits_current_limits(
            &make_bundle(Some(2), Some(799)),
            limits,
            1000,
        ));
        assert!(!group_history_bundle_fits_current_limits(
            &make_bundle(Some(2), None),
            limits,
            1000,
        ));
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
            status: Some(wa::web_message_info::Status::SERVER_ACK),
            ..Default::default()
        };
        let mut private = make_message("latest", &group, 950);
        private
            .message
            .as_option_mut()
            .unwrap()
            .message_context_info = buffa::MessageField::some(wa::MessageContextInfo {
            message_secret: Some(b"synthetic-secret".to_vec()),
            reporting_token_version: Some(1),
            ..Default::default()
        });
        private.starred = Some(true);
        private.labels = vec!["private-label".into()];
        private.message_secret = Some(b"private-secret".to_vec());
        private.message_add_ons.push(wa::MessageAddOn::default());
        let messages = vec![
            make_message("old", &group, 799),
            make_message("oldest-in-window", &group, 800),
            make_message("newer", &group, 900),
            private,
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
        assert_eq!(selected.messages[1].starred, None);
        assert!(selected.messages[1].labels.is_empty());
        assert_eq!(selected.messages[1].message_secret, None);
        assert!(selected.messages[1].message_add_ons.is_empty());
        assert_eq!(
            selected.messages[1]
                .message
                .as_option()
                .and_then(|message| message.message_context_info.as_option())
                .and_then(|context| context.message_secret.as_deref()),
            Some(b"synthetic-secret".as_slice())
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

    #[test]
    fn history_text_admission_checks_borrowed_payload_and_context() {
        use waproto::whatsapp as wa;

        let mut content = wa::Message {
            conversation: Some("synthetic text".into()),
            ..Default::default()
        };
        assert!(is_shareable_history_text(&content));
        for context in [
            wa::MessageContextInfo::default(),
            wa::MessageContextInfo {
                message_secret: Some(vec![7; 32]),
                reporting_token_version: Some(1),
                ..Default::default()
            },
        ] {
            content.message_context_info = buffa::MessageField::some(context);
            assert!(is_shareable_history_text(&content));
        }
        for context in [
            wa::MessageContextInfo {
                padding_bytes: Some(Vec::new()),
                ..Default::default()
            },
            wa::MessageContextInfo {
                support_payload: Some("private".into()),
                ..Default::default()
            },
            wa::MessageContextInfo {
                limit_sharing: buffa::MessageField::some(Default::default()),
                ..Default::default()
            },
        ] {
            content.message_context_info = buffa::MessageField::some(context);
            assert!(!is_shareable_history_text(&content));
        }
        content.message_context_info = buffa::MessageField::none();
        content.ephemeral_message = buffa::MessageField::some(Default::default());
        assert!(!is_shareable_history_text(&content));
        content.ephemeral_message = buffa::MessageField::some(wa::message::FutureProofMessage {
            message: buffa::MessageField::some(wa::Message {
                conversation: Some("private nested text".repeat(4096)),
                ..Default::default()
            }),
        });
        assert!(!is_shareable_history_text(&content));
        content.ephemeral_message = buffa::MessageField::none();
        content.conversation = Some(String::new());
        assert!(!is_shareable_history_text(&content));
    }

    #[test]
    fn selection_preserves_first_eligible_duplicate_and_newest_ties() {
        use waproto::whatsapp as wa;

        let group = wacore_binary::Jid::new("120363000000000001", wacore_binary::Server::Group);
        let make_message = |id: &str, timestamp| wa::WebMessageInfo {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(group.to_string()),
                id: Some(id.into()),
                ..Default::default()
            }),
            message: buffa::MessageField::some(wa::Message {
                conversation: Some(id.into()),
                ..Default::default()
            }),
            message_timestamp: Some(timestamp),
            status: Some(wa::web_message_info::Status::SERVER_ACK),
            ..Default::default()
        };
        let archive = [
            make_message("duplicate", 799),
            make_message("duplicate", 950),
            make_message("duplicate", 999),
            make_message("earlier-tie", 950),
            make_message("later-tie", 950),
        ];
        let selected = select_group_history_messages(
            &group,
            &archive,
            1000,
            GroupHistoryLimits {
                max_messages: 3,
                time_window_seconds: 200,
            },
        )
        .unwrap();
        assert_eq!(selected.oldest_timestamp, 950);
        assert_eq!(selected.messages.len(), 3);
        assert_eq!(selected.messages[0].message_timestamp, Some(950));
        assert_eq!(selected.messages[0].key, archive[1].key);
        let selected = select_group_history_messages(
            &group,
            &archive,
            1000,
            GroupHistoryLimits {
                max_messages: 1,
                time_window_seconds: 200,
            },
        )
        .unwrap();
        assert_eq!(selected.messages[0].key, archive[4].key);
    }

    #[test]
    fn selection_refuses_nested_bundles_unsent_messages_and_expired_content() {
        use waproto::whatsapp as wa;
        let group = wacore_binary::Jid::new("120363000000000001", wacore_binary::Server::Group);
        let make_message = |id: &str| wa::WebMessageInfo {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(group.to_string()),
                id: Some(id.into()),
                from_me: Some(true),
                ..Default::default()
            }),
            message: buffa::MessageField::some(wa::Message {
                conversation: Some("synthetic text".into()),
                ..Default::default()
            }),
            message_timestamp: Some(950),
            status: Some(wa::web_message_info::Status::SERVER_ACK),
            ..Default::default()
        };
        let mut nested = make_message("nested");
        nested.message = buffa::MessageField::some(wa::Message {
            conversation: Some("synthetic text".into()),
            message_history_bundle: buffa::MessageField::some(wa::message::MessageHistoryBundle {
                media_key: Some(b"SYNTHETIC-OTHER-KEY".to_vec()),
                ..Default::default()
            }),
            ..Default::default()
        });
        let mut pending = make_message("pending");
        pending.status = Some(wa::web_message_info::Status::PENDING);
        let mut failed = make_message("failed");
        failed.status = Some(wa::web_message_info::Status::ERROR);
        let mut expired = make_message("expired");
        expired.ephemeral_expiration_timestamp = Some(999);
        let mut expiring = make_message("expiring");
        expiring.ephemeral_expiration_timestamp = Some(1010);
        let mut timed = make_message("timed");
        timed.ephemeral_start_timestamp = Some(1000);
        timed.ephemeral_duration = Some(10);
        let mut incomplete = make_message("incomplete");
        incomplete.ephemeral_start_timestamp = Some(1000);
        let mut unknown = make_message("unknown-status");
        unknown.status = None;
        let selected = select_group_history_messages(
            &group,
            &[
                nested,
                pending,
                failed,
                expired,
                expiring,
                timed,
                incomplete,
                unknown,
                make_message("safe"),
            ],
            1000,
            GroupHistoryLimits {
                max_messages: 100,
                time_window_seconds: 200,
            },
        )
        .unwrap();
        assert_eq!(selected.messages.len(), 1);
        assert_eq!(
            selected.messages[0].key.as_option().unwrap().id.as_deref(),
            Some("safe")
        );
    }

    #[tokio::test]
    async fn group_history_policy_rejects_conflicting_duplicate_properties() {
        let account = account_snapshot(&[]).await;
        let code = abprops::group::GROUP_HISTORY_SEND_GROUP_LEVEL.code;
        let conflicting = group_props(&[(code, "0"), (code, "1")]);
        assert!(resolve_group_history_limits(&account, &conflicting).is_err());
        let identical = group_props(&[(code, "1"), (code, "1")]);
        assert!(resolve_group_history_limits(&account, &identical).is_ok());
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
