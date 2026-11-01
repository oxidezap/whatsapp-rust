//! A/B Props (experiment config) IQ specification.
//!
//! Fetches server-side A/B testing properties and experiment configurations.
//!
//! ## Wire Format
//! ```xml
//! <!-- Request -->
//! <iq xmlns="abt" type="get" to="s.whatsapp.net" id="...">
//!   <props protocol="1" hash="..." refresh_id="..."/>
//! </iq>
//!
//! <!-- Response -->
//! <iq from="s.whatsapp.net" id="..." type="result">
//!   <props protocol="1" ab_key="..." hash="..." refresh="..." refresh_id="...">
//!     <prop config_code="123" config_value="value"/>
//!     <prop event_code="5138" sampling_weight="-1"/>
//!     <prop config_code="456" config_value="other"/>
//!     ...
//!   </props>
//! </iq>
//! ```
//!
//! Verified against WhatsApp Web JS (WASmaxOutAbPropsGetExperimentConfigRequest,
//! WASmaxInAbPropsConfigs).

use crate::iq::spec::{IqSpec, IqStreamSpec};
use crate::protocol::ProtocolNode;
use crate::request::InfoQuery;
use std::collections::HashSet;
use wacore_binary::CompactString;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Server};
use wacore_binary::{Node, NodeContent, NodeRef, NodeStream};

/// IQ namespace for A/B props.
pub const PROPS_NAMESPACE: &str = "abt";

use crate::iq::abprops;

/// Historical AB props retained for SDK callers even when the current Web
/// capture omits their definitions from the generated registry's `ALL` slices.
/// Retaining the historical values preserves callers and their
/// defaults; absence from a Web capture says nothing about server delivery.
pub mod stale {
    use crate::iq::abprops::{AbDefault, AbProp, AbPropType};

    // Public constants retained from whatspec 1a441f0, absent from the new capture.
    // No claim about server delivery follows from that absence.
    pub const AI_3P_AGENT_LINK_ENABLED: AbProp = AbProp {
        name: "ai_3p_agent_link_enabled",
        code: 31064,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const LISTS_SMB_WEB_ENABLED: AbProp = AbProp {
        name: "lists_smb_web_enabled",
        code: 24732,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const SCHEDULED_COMPANION_CONTACT_REFRESH_DAYS: AbProp = AbProp {
        name: "scheduled_companion_contact_refresh_days",
        code: 34960,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const SCHEDULED_COMPANION_CONTACT_REFRESH_HOURS: AbProp = AbProp {
        name: "scheduled_companion_contact_refresh_hours",
        code: 35018,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const SMOOTHIE_PERFORMANCE_MSG_SEND: AbProp = AbProp {
        name: "smoothie_performance_msg_send",
        code: 17942,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };

    pub const UPDATED_HARMFUL_DOCUMENT_DIALOG: AbProp = AbProp {
        name: "updated_harmful_document_dialog",
        code: 15022,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const PRIVACY_TOKEN_ONLY_CHECK_LID: AbProp = AbProp {
        name: "privacy_token_only_check_lid",
        code: 15_491,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    /// Gates tctoken inclusion in profile picture IQ requests.
    pub const PROFILE_PIC_PRIVACY_TOKEN: AbProp = AbProp {
        name: "profile_pic_privacy_token",
        code: 9_666,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    // Public API retained from whatspec c581c20 (2.3000.1047483476).
    // Keep these outside the current capture registry and its ALL slices.
    pub const AI_CHAT_THREADS_INFRA_WEB_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_infra_web_enabled",
        code: 26776,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const AI_CHAT_THREADS_WEB_KILLSWITCH_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_web_killswitch_enabled",
        code: 26806,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const AI_CHAT_THREADS_WEB_MSGS_LOAD_LIMIT: AbProp = AbProp {
        name: "ai_chat_threads_web_msgs_load_limit",
        code: 23694,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };

    pub const AI_META_AI_THREAD_CREATION_ENABLED: AbProp = AbProp {
        name: "ai_meta_ai_thread_creation_enabled",
        code: 34964,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const AUTH_AGENT_SOFT_OFFBOARDING_ENABLED: AbProp = AbProp {
        name: "auth_agent_soft_offboarding_enabled",
        code: 28802,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const BUG_REPORTING_NOT_SHIPPED_YET_ENABLED: AbProp = AbProp {
        name: "bug_reporting_not_shipped_yet_enabled",
        code: 29458,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const CHANNELS_PULSE_ON_UNREAD_BADGE_ENABLED: AbProp = AbProp {
        name: "channels_pulse_on_unread_badge_enabled",
        code: 28224,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const CHANNELS_STICKERS_QP_ENABLED: AbProp = AbProp {
        name: "channels_stickers_qp_enabled",
        code: 36632,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const CTWA_LEAD_TAXONOMY: AbProp = AbProp {
        name: "ctwa_lead_taxonomy",
        code: 26531,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const CTWA_NATIVE_WEB_DRAFT_AD_ENABLED: AbProp = AbProp {
        name: "ctwa_native_web_draft_ad_enabled",
        code: 28989,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_METAVERIFIED_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_metaverified_biz",
        code: 35819,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_SUSPICIOUS_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_suspicious_biz",
        code: 35818,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_TIER0_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_tier0_biz",
        code: 35814,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_TIER1_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_tier1_biz",
        code: 35813,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_TIER2_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_tier2_biz",
        code: 35817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_TIER3_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_tier3_biz",
        code: 35816,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_UNKNOWN_TIERED_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_unknown_tiered_biz",
        code: 35821,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_FMX_REDESIGNED_BCC_FOR_UNTIERED_BIZ: AbProp = AbProp {
        name: "enable_fmx_redesigned_bcc_for_untiered_biz",
        code: 35820,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const ENABLE_WEB_VOIP_VIDEO_RESOLUTION_CAP: AbProp = AbProp {
        name: "enable_web_voip_video_resolution_cap",
        code: 25899,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };

    pub const ENABLE_WEB_VOIP_VIRTUAL_AUDIO_CAPTURE_DRIVER: AbProp = AbProp {
        name: "enable_web_voip_virtual_audio_capture_driver",
        code: 26838,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const EXPAND_FMX_MEX_SHOULD_USE_FMX_USE_CASE: AbProp = AbProp {
        name: "expand_fmx_mex_should_use_fmx_use_case",
        code: 27662,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const GROUP_FROM_GROUP_MAX_MISSING_PRIVACY_TOKENS: AbProp = AbProp {
        name: "group_from_group_max_missing_privacy_tokens",
        code: 34957,
        value_type: AbPropType::Int,
        default: AbDefault::Int(32),
    };

    pub const INFO_DRAWER_REFRESH: AbProp = AbProp {
        name: "info_drawer_refresh",
        code: 29210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const INTEGRITY_CHECKPOINTS_DEFAULT_ENABLED: AbProp = AbProp {
        name: "integrity_checkpoints_default_enabled",
        code: 27663,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };

    pub const INTERACTIVE_BLOKS_WIDGET_WEB_ENABLED: AbProp = AbProp {
        name: "interactive_bloks_widget_web_enabled",
        code: 26685,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const INTERACTIVE_RESPONSE_MESSAGE_NATIVE_FLOW_KILLSWITCH: AbProp = AbProp {
        name: "interactive_response_message_native_flow_killswitch",
        code: 1436,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const KMP_SYNCD_ENGINE_CRYPTO_ENABLED: AbProp = AbProp {
        name: "kmp_syncd_engine_crypto_enabled",
        code: 15909,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const KMP_SYNCD_ENGINE_OUTGOING_PROCESSOR_ENABLED: AbProp = AbProp {
        name: "kmp_syncd_engine_outgoing_processor_enabled",
        code: 18234,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const LARGE_SCREENS_NEW_CHAT_BUTTON_VARIANTS: AbProp = AbProp {
        name: "large_screens_new_chat_button_variants",
        code: 26788,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const NEWSLETTER_STATUS_CREATION_ENABLED: AbProp = AbProp {
        name: "newsletter_status_creation_enabled",
        code: 26669,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const RNR_MIN_DAYS_USER_ACTIVE: AbProp = AbProp {
        name: "rnr_min_days_user_active",
        code: 18702,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };

    pub const RT_RECEIVER_V3_VALIDATION_ENABLED: AbProp = AbProp {
        name: "rt_receiver_v3_validation_enabled",
        code: 36371,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };

    pub const SMB_BIZ_AI_SUGGESTED_REPLY_PREFETCH: AbProp = AbProp {
        name: "smb_biz_ai_suggested_reply_prefetch",
        code: 35509,
        value_type: AbPropType::Str,
        default: AbDefault::Str("NONE"),
    };

    pub const SMB_WEB_ENABLE_FB_LINKING: AbProp = AbProp {
        name: "smb_web_enable_fb_linking",
        code: 30112,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const STATUS_WEB_RANKING: AbProp = AbProp {
        name: "status_web_ranking",
        code: 31666,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_SMB_WEB_LISTS_QUICK_REPLIES_ENABLED: AbProp = AbProp {
        name: "wa_smb_web_lists_quick_replies_enabled",
        code: 31061,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_ATTACH_ICON_VARIANT: AbProp = AbProp {
        name: "wa_web_attach_icon_variant",
        code: 26386,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const WA_WEB_BIZ_PROFILE_GRAPHQL_MIGRATION_BYPASS_LID_CHECK_DOGFOODING: AbProp = AbProp {
        name: "wa_web_biz_profile_graphql_migration_bypass_lid_check_dogfooding",
        code: 29965,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_BLOCKED_PARTICIPANT_CALL_WARNING: AbProp = AbProp {
        name: "wa_web_blocked_participant_call_warning",
        code: 29039,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_BROADCAST_DISAPPEARING_MESSAGES_FIX: AbProp = AbProp {
        name: "wa_web_broadcast_disappearing_messages_fix",
        code: 31499,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_CANONICAL_REG_RELOAD_ENABLED: AbProp = AbProp {
        name: "wa_web_canonical_reg_reload_enabled",
        code: 29472,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_CHANGE_LIST_WDS_SUBMENU: AbProp = AbProp {
        name: "wa_web_change_list_wds_submenu",
        code: 27123,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_CONTEXT_CARD_VERTICAL_BUTTONS: AbProp = AbProp {
        name: "wa_web_context_card_vertical_buttons",
        code: 31178,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_FAVICON_BADGING_ENABLED: AbProp = AbProp {
        name: "wa_web_favicon_badging_enabled",
        code: 22924,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_FOCUS_MANAGEMENT_FOR_STATUS_AUDIENCE: AbProp = AbProp {
        name: "wa_web_focus_management_for_status_audience",
        code: 27719,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_FREQUENT_REACTIONS_STORE_ENABLED: AbProp = AbProp {
        name: "wa_web_frequent_reactions_store_enabled",
        code: 27710,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_FREQUENT_REACTIONS_WEIGHT_REDUCER: AbProp = AbProp {
        name: "wa_web_frequent_reactions_weight_reducer",
        code: 27711,
        value_type: AbPropType::Int,
        default: AbDefault::Int(90),
    };

    pub const WA_WEB_INVITE_LINK_PAGE_ENHANCEMENTS: AbProp = AbProp {
        name: "wa_web_invite_link_page_enhancements",
        code: 31210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_NOTIFY_FOR: AbProp = AbProp {
        name: "wa_web_notify_for",
        code: 25544,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_SCROLLABLE_REACTION_TRAY_ENABLED: AbProp = AbProp {
        name: "wa_web_scrollable_reaction_tray_enabled",
        code: 27709,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_SEARCH_EMOJI_PICKER: AbProp = AbProp {
        name: "wa_web_search_emoji_picker",
        code: 27857,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_SELECT_ALL_CHATS_ENABLED: AbProp = AbProp {
        name: "wa_web_select_all_chats_enabled",
        code: 30040,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_STARRED_MSGS_SEARCH: AbProp = AbProp {
        name: "wa_web_starred_msgs_search",
        code: 27353,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_STATUS_REACTION_STICKER_REPLY_ENABLED: AbProp = AbProp {
        name: "wa_web_status_reaction_sticker_reply_enabled",
        code: 30494,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEB_STATUS_RESHARE_ATTRIBUTION_ENABLED: AbProp = AbProp {
        name: "wa_web_status_reshare_attribution_enabled",
        code: 28813,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEBTP_PRELOAD_THUMBNAIL_RENDERER_NO_EXPOSURE: AbProp = AbProp {
        name: "wa_webtp_preload_thumbnail_renderer_no_exposure",
        code: 27534,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WA_WEBTP_THUMBNAIL_RENDERER_MODE: AbProp = AbProp {
        name: "wa_webtp_thumbnail_renderer_mode",
        code: 27535,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const WAMO_AGM_ENABLED: AbProp = AbProp {
        name: "wamo_agm_enabled",
        code: 15714,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WAMO_PRIVACY_TOS_SHOW_CHANNELS_NUX_ENABLED: AbProp = AbProp {
        name: "wamo_privacy_tos_show_channels_nux_enabled",
        code: 15254,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };

    pub const WDS_WEB_BADGE: AbProp = AbProp {
        name: "wds_web_badge",
        code: 27856,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WDS_WEB_MENU_REACTION_DETAIL_PANEL_V2: AbProp = AbProp {
        name: "wds_web_menu_reaction_detail_panel_v2",
        code: 30694,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_ATTACH_MENU_ADD_DRAWING_ENABLED: AbProp = AbProp {
        name: "web_attach_menu_add_drawing_enabled",
        code: 24384,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_BIZ_SIMPLE_SIGNAL_GROUP_ENABLED: AbProp = AbProp {
        name: "web_biz_simple_signal_group_enabled",
        code: 28679,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_BOT_PROFILE_PIC_GQL_MIGRATION_ENABLED: AbProp = AbProp {
        name: "web_bot_profile_pic_gql_migration_enabled",
        code: 30597,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_BULK_ADD_CONTACTS_ENABLED: AbProp = AbProp {
        name: "web_bulk_add_contacts_enabled",
        code: 24875,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_CALLING_ENABLE_ON_WINDOWS: AbProp = AbProp {
        name: "web_calling_enable_on_windows",
        code: 26259,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_CALLING_FULL_SCREEN_TOGGLE_ENABLED: AbProp = AbProp {
        name: "web_calling_full_screen_toggle_enabled",
        code: 28830,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_CHAT_THEME_DRAWER_TITLE: AbProp = AbProp {
        name: "web_chat_theme_drawer_title",
        code: 28157,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_ENABLE_CAMERA_CAPTURE_REFRESH: AbProp = AbProp {
        name: "web_enable_camera_capture_refresh",
        code: 28316,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_LOGOUT_UNMIGRATED_COMPANION: AbProp = AbProp {
        name: "web_logout_unmigrated_companion",
        code: 31151,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_MEDIA_WORKER_SPLIT_ENABLED: AbProp = AbProp {
        name: "web_media_worker_split_enabled",
        code: 27753,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_MEMORY_REDUCTION: AbProp = AbProp {
        name: "web_memory_reduction",
        code: 30394,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_MENU_SHARE_GROUP: AbProp = AbProp {
        name: "web_menu_share_group",
        code: 26850,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_MSG_INFRA_REMOVE_DEVICES_ON_406_ERROR_ENABLED: AbProp = AbProp {
        name: "web_msg_infra_remove_devices_on_406_error_enabled",
        code: 27463,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_NAVIGATION_BAR_UPDATES_TAB: AbProp = AbProp {
        name: "web_navigation_bar_updates_tab",
        code: 21250,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_NEW_CHAT_FLOW_REFRESH_VARIANT: AbProp = AbProp {
        name: "web_new_chat_flow_refresh_variant",
        code: 12276,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };

    pub const WEB_OTP_COPY_CODE_DISABLED: AbProp = AbProp {
        name: "web_otp_copy_code_disabled",
        code: 4330,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_READ_SELF_WATERMARK_PROCESSING: AbProp = AbProp {
        name: "web_read_self_watermark_processing",
        code: 30736,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_TEST_ABPROP_DELETE_ME: AbProp = AbProp {
        name: "web_test_abprop_delete_me",
        code: 27274,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const WEB_WAM_MAX_BUFFER_UPLOAD_SIZE_BYTES: AbProp = AbProp {
        name: "web_wam_max_buffer_upload_size_bytes",
        code: 9501,
        value_type: AbPropType::Int,
        default: AbDefault::Int(64000),
    };

    pub const WEB_WHATS_NEW_BANNER_SHORT_COOLDOWN: AbProp = AbProp {
        name: "web_whats_new_banner_short_cooldown",
        code: 29620,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    pub const GROUP_CALL_MAX_PARTICIPANTS: AbProp = AbProp {
        name: "group_call_max_participants",
        code: 4190,
        value_type: AbPropType::Int,
        default: AbDefault::Int(32),
    };
}

/// AB props this client reads. Seeds the cache interest set so the other ~2090
/// server props are discarded on `apply_props`. Add an entry when you start
/// reading a new flag from [`crate::iq::abprops`]. Only these consts (and their
/// name strings) materialize in the binary; the rest of the vendored registry is
/// unreferenced and emitted nowhere.
pub const WATCHED: &[abprops::AbProp] = &[
    abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED,
    abprops::web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES,
    abprops::web::PRIVACY_TOKEN_SENDING_ON_GROUP_CREATE,
    abprops::web::PRIVACY_TOKEN_SENDING_ON_GROUP_PARTICIPANT_ADD,
    abprops::web::LID_TRUSTED_TOKEN_ISSUE_TO_LID,
    abprops::web::TCTOKEN_DURATION,
    abprops::web::TCTOKEN_DURATION_SENDER,
    abprops::web::TCTOKEN_NUM_BUCKETS,
    abprops::web::TCTOKEN_NUM_BUCKETS_SENDER,
    abprops::web::WA_NCT_TOKEN_SEND_ENABLED,
    abprops::web::RECEIPT_MODE_BITMASK_ENABLED,
    abprops::web::WEB_SEND_HID_FAILED_DECRYPT_IN_RECEIPTS_ENABLED,
    abprops::web::WEB_E2E_BACKFILL_EXPIRE_TIME,
    abprops::web::ENABLE_SPAM_REPORT_IQ_WITH_PRIVACY_TOKEN,
    abprops::web::PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_USYNC,
    abprops::web::ENABLE_PEER_SNAPSHOT_RECOVERY,
    abprops::web::SNAPSHOT_RECOVERY_MAX_MUTATIONS_COUNT_ALLOWED,
    abprops::web::USERNAME_CONTACT_DISPLAY,
    abprops::web::ENABLE_CALLING_PHONE_NUMBER_PRIVACY,
    abprops::web::GROUP_HISTORY_MESSAGE_COUNT_LIMIT,
    abprops::web::GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS,
    abprops::web::GROUP_HISTORY_SEND,
    stale::PRIVACY_TOKEN_ONLY_CHECK_LID,
    stale::PROFILE_PIC_PRIVACY_TOKEN,
];

/// Protocol version for props requests.
pub const PROPS_PROTOCOL_VERSION: &str = "1";

/// A/B experiment property returned from the server.
#[derive(Debug, Clone)]
pub struct AbProp {
    /// The config code (property identifier).
    pub config_code: u32,
    /// The config value. CompactString inlines values <=24 bytes (covers most props).
    pub config_value: CompactString,
    /// Optional experiment exposure key.
    pub config_expo_key: Option<u32>,
}

impl ProtocolNode for AbProp {
    fn tag(&self) -> &'static str {
        "prop"
    }

    fn into_node(self) -> Node {
        let mut builder = NodeBuilder::new("prop")
            .attr("config_code", self.config_code)
            .attr("config_value", &*self.config_value);

        if let Some(expo_key) = self.config_expo_key {
            builder = builder.attr("config_expo_key", expo_key);
        }

        builder.build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self, anyhow::Error> {
        use crate::iq::node::optional_attr;

        if node.tag != "prop" {
            return Err(anyhow::anyhow!("expected <prop>, got <{}>", node.tag));
        }

        let config_code: u32 = optional_attr(node, "config_code")
            .ok_or_else(|| anyhow::anyhow!("missing config_code in prop"))?
            .parse()?;
        if config_code == 0 {
            return Err(anyhow::anyhow!("config_code must be >= 1"));
        }
        let config_value = optional_attr(node, "config_value")
            .ok_or_else(|| anyhow::anyhow!("missing config_value in prop"))?;
        let config_value = CompactString::from(config_value.as_ref());
        let config_expo_key = optional_attr(node, "config_expo_key").and_then(|s| s.parse().ok());

        Ok(Self {
            config_code,
            config_value,
            config_expo_key,
        })
    }
}

/// A/B sampling property returned from the server.
#[derive(Debug, Clone)]
pub struct SamplingProp {
    /// The event code (sampling identifier).
    pub event_code: u32,
    /// The sampling weight (typically -10000..=10000).
    pub sampling_weight: i32,
}

impl ProtocolNode for SamplingProp {
    fn tag(&self) -> &'static str {
        "prop"
    }

    fn into_node(self) -> Node {
        NodeBuilder::new("prop")
            .attr("event_code", self.event_code)
            .attr("sampling_weight", self.sampling_weight)
            .build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self, anyhow::Error> {
        use crate::iq::node::optional_attr;

        if node.tag != "prop" {
            return Err(anyhow::anyhow!("expected <prop>, got <{}>", node.tag));
        }

        let event_code: u32 = optional_attr(node, "event_code")
            .ok_or_else(|| anyhow::anyhow!("missing event_code in prop"))?
            .parse()?;
        if event_code == 0 {
            return Err(anyhow::anyhow!("event_code must be >= 1"));
        }

        let sampling_weight: i32 = optional_attr(node, "sampling_weight")
            .ok_or_else(|| anyhow::anyhow!("missing sampling_weight in prop"))?
            .parse()?;
        if !(-10000..=10000).contains(&sampling_weight) {
            return Err(anyhow::anyhow!(
                "sampling_weight out of range (-10000..=10000): {}",
                sampling_weight
            ));
        }

        Ok(Self {
            event_code,
            sampling_weight,
        })
    }
}

/// A/B config entry, which can be an experiment or sampling config.
#[derive(Debug, Clone)]
pub enum AbPropConfig {
    Experiment(AbProp),
    Sampling(SamplingProp),
}

impl ProtocolNode for AbPropConfig {
    fn tag(&self) -> &'static str {
        "prop"
    }

    fn into_node(self) -> Node {
        match self {
            Self::Experiment(prop) => prop.into_node(),
            Self::Sampling(prop) => prop.into_node(),
        }
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self, anyhow::Error> {
        use crate::iq::node::optional_attr;

        if node.tag != "prop" {
            return Err(anyhow::anyhow!("expected <prop>, got <{}>", node.tag));
        }

        // Check discriminating attribute to avoid double-parse allocations
        let has_config = optional_attr(node, "config_code").is_some();
        let has_event = optional_attr(node, "event_code").is_some();

        if has_config && has_event {
            Err(anyhow::anyhow!(
                "prop has both config_code and event_code (attrs: {:?})",
                node.attrs
            ))
        } else if has_config {
            Ok(Self::Experiment(AbProp::try_from_node_ref(node)?))
        } else if has_event {
            Ok(Self::Sampling(SamplingProp::try_from_node_ref(node)?))
        } else {
            Err(anyhow::anyhow!(
                "prop has neither config_code nor event_code (attrs: {:?})",
                node.attrs
            ))
        }
    }
}

/// Response from props query.
#[derive(Debug, Clone, Default)]
pub struct PropsResponse {
    /// A/B key for this configuration set.
    pub ab_key: Option<String>,
    /// Hash of the current configuration.
    pub hash: Option<String>,
    /// Refresh interval in seconds.
    pub refresh: Option<u32>,
    /// Refresh ID for delta updates.
    pub refresh_id: Option<u32>,
    /// Whether this is a delta update.
    pub delta_update: bool,
    /// Experiment prop code-value pairs (lightweight, no enum wrapper).
    /// Sampling props are skipped during parsing.
    pub experiment_props: Vec<(u32, CompactString)>,
}

impl ProtocolNode for PropsResponse {
    fn tag(&self) -> &'static str {
        "props"
    }

    fn into_node(self) -> Node {
        let mut builder = NodeBuilder::new("props").attr("protocol", PROPS_PROTOCOL_VERSION);

        if let Some(ref ab_key) = self.ab_key {
            builder = builder.attr("ab_key", ab_key);
        }
        if let Some(ref hash) = self.hash {
            builder = builder.attr("hash", hash);
        }
        if let Some(refresh) = self.refresh {
            builder = builder.attr("refresh", refresh);
        }
        if let Some(refresh_id) = self.refresh_id {
            builder = builder.attr("refresh_id", refresh_id);
        }
        builder = builder.attr("delta_update", self.delta_update);

        // Round-trip with try_from_node_ref. config_expo_key is dropped on
        // both sides; extend the tuple type before adding it back.
        let prop_nodes: Vec<Node> = self
            .experiment_props
            .into_iter()
            .map(|(code, value)| {
                NodeBuilder::new("prop")
                    .attr("config_code", code)
                    .attr("config_value", &*value)
                    .build()
            })
            .collect();

        builder.children(prop_nodes).build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self, anyhow::Error> {
        use crate::iq::node::optional_attr;

        if node.tag != "props" {
            return Err(anyhow::anyhow!("expected <props>, got <{}>", node.tag));
        }

        let ab_key = optional_attr(node, "ab_key").map(|s| s.into_owned());
        let hash = optional_attr(node, "hash").map(|s| s.into_owned());
        let refresh = optional_attr(node, "refresh").and_then(|s| s.parse().ok());
        let refresh_id = optional_attr(node, "refresh_id").and_then(|s| s.parse().ok());
        let delta_update = optional_attr(node, "delta_update")
            .map(|s| s == "true")
            .unwrap_or(false);

        // Parse experiment props as lightweight (code, value) tuples.
        // Sampling props (missing config_code or config_value) are skipped.
        let mut experiment_props = Vec::new();
        for child in node.get_children_by_tag("prop") {
            if let Some(code_str) = optional_attr(child, "config_code")
                && let Ok(code) = code_str.parse::<u32>()
                && code > 0
                && let Some(value) = optional_attr(child, "config_value")
            {
                experiment_props.push((code, CompactString::from(value.as_ref())));
            }
        }

        Ok(Self {
            ab_key,
            hash,
            refresh,
            refresh_id,
            delta_update,
            experiment_props,
        })
    }
}

/// Fetches A/B testing properties from the server.
#[derive(Debug, Clone, Default)]
pub struct PropsSpec {
    /// Optional hash from previous props fetch (for delta updates).
    pub hash: Option<String>,
    /// Optional refresh ID (for emergency push updates).
    pub refresh_id: Option<u32>,
    /// Codes to keep when the response is consumed as a stream; `None` keeps
    /// every prop. See [`Self::retaining`].
    pub retain: Option<HashSet<u32>>,
}

impl PropsSpec {
    /// Create a new props spec without hash or refresh_id.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a props spec with a hash for delta updates.
    pub fn with_hash(hash: impl Into<String>) -> Self {
        Self {
            hash: Some(hash.into()),
            ..Self::default()
        }
    }

    /// Create a props spec with a refresh_id for emergency push responses.
    pub fn with_refresh_id(refresh_id: u32) -> Self {
        Self {
            refresh_id: Some(refresh_id),
            ..Self::default()
        }
    }

    /// Keep only these codes in the response.
    ///
    /// A full response carries the whole catalog, a few thousand props, of
    /// which a client reads a couple of dozen. Filtering while the stream is
    /// walked means the response never exists in memory as more than one
    /// `<prop>` at a time plus what is kept. A response decoded as a tree is
    /// filtered the same way, so the spec answers alike whichever way its
    /// response arrived.
    pub fn retaining(mut self, codes: impl IntoIterator<Item = u32>) -> Self {
        self.retain = Some(codes.into_iter().collect());
        self
    }

    fn keeps(&self, code: u32) -> bool {
        self.retain.as_ref().is_none_or(|keep| keep.contains(&code))
    }
}

impl IqStreamSpec for PropsSpec {
    fn consume_response(
        &self,
        stream: &mut NodeStream<'_>,
    ) -> Result<Self::Response, anyhow::Error> {
        use crate::iq::node::optional_attr;

        // Descend into <props>, skipping any sibling in front of it.
        let mut response = loop {
            let Some(head) = stream.open()? else {
                return Err(anyhow::anyhow!("<props> child not found"));
            };
            if head.tag != "props" {
                stream.close()?;
                continue;
            }
            break PropsResponse {
                ab_key: head.attr_str("ab_key").map(|s| s.into_owned()),
                hash: head.attr_str("hash").map(|s| s.into_owned()),
                refresh: head.attr_str("refresh").and_then(|s| s.parse().ok()),
                refresh_id: head.attr_str("refresh_id").and_then(|s| s.parse().ok()),
                delta_update: head
                    .attr_str("delta_update")
                    .map(|s| s == "true")
                    .unwrap_or(false),
                experiment_props: Vec::new(),
            };
        };

        // The same selection `PropsResponse::try_from_node_ref` makes, one
        // child at a time: experiment props only, sampling props skipped.
        while let Some(child) = stream.next_child()? {
            if child.tag == "prop"
                && let Some(code_str) = optional_attr(&child, "config_code")
                && let Ok(code) = code_str.parse::<u32>()
                && code > 0
                && self.keeps(code)
                && let Some(value) = optional_attr(&child, "config_value")
            {
                response
                    .experiment_props
                    .push((code, CompactString::from(value.as_ref())));
            }
        }
        stream.close()?;
        Ok(response)
    }
}

impl IqSpec for PropsSpec {
    type Response = PropsResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        let mut builder = NodeBuilder::new("props").attr("protocol", PROPS_PROTOCOL_VERSION);

        if let Some(ref hash) = self.hash {
            builder = builder.attr("hash", hash.as_str());
        }

        if let Some(refresh_id) = self.refresh_id {
            builder = builder.attr("refresh_id", refresh_id);
        }

        InfoQuery::get(
            PROPS_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![builder.build()])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        use crate::iq::node::required_child;

        let props_node = required_child(response, "props")?;
        let mut parsed = PropsResponse::try_from_node_ref(props_node)?;
        if self.retain.is_some() {
            parsed
                .experiment_props
                .retain(|(code, _)| self.keeps(*code));
        }
        Ok(parsed)
    }
}

/// Full group-scoped AB-property response. Unlike account props, these are
/// keyed by a group JID and need a separate `abt` request per group.
#[derive(Debug, Clone, Default)]
pub struct GroupPropsResponse {
    /// Present when the server returned a versioned group configuration set.
    pub hash: Option<String>,
    /// Experiment values only; sampling configurations do not gate features.
    pub experiment_props: Vec<(u32, CompactString)>,
}

/// Fetch one group's full AB-property set without a request-side hash.
/// The response hash lets callers verify that the server returned a versioned
/// configuration rather than treating an unavailable configuration as defaults.
#[derive(Debug, Clone)]
pub struct GroupPropsSpec {
    group: Jid,
}

impl GroupPropsSpec {
    pub fn new(group: &Jid) -> Self {
        Self {
            group: group.clone(),
        }
    }
}

impl IqSpec for GroupPropsSpec {
    type Response = GroupPropsResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        let props = NodeBuilder::new("props").attr("group", &self.group);
        InfoQuery::get(
            PROPS_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![props.build()])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        use crate::iq::node::{optional_attr, required_child};

        let props = required_child(response, "props")?;
        let mut parsed = GroupPropsResponse {
            hash: optional_attr(props, "hash").map(|value| value.into_owned()),
            experiment_props: Vec::new(),
        };
        let mut values = std::collections::HashMap::new();
        for child in props.get_children_by_tag("prop") {
            let code = optional_attr(child, "config_code");
            let value = optional_attr(child, "config_value");
            let (code, value) = match (code, value) {
                (None, None) => continue,
                (Some(code), Some(value)) => (code.parse::<u32>()?, value),
                _ => anyhow::bail!("incomplete group experiment property"),
            };
            anyhow::ensure!(code > 0, "invalid group experiment code");
            let value = CompactString::from(value.as_ref());
            if let Some(previous) = values.insert(code, value.clone()) {
                anyhow::ensure!(previous == value, "conflicting group experiment values");
            } else {
                parsed.experiment_props.push((code, value));
            }
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_props_request_uses_group_and_optional_hash_attributes() {
        let group = Jid::new("120363000000000001", Server::Group);
        let group_string = group.to_string();
        let spec = GroupPropsSpec::new(&group);
        let iq = spec.build_iq();
        assert_eq!(iq.namespace, PROPS_NAMESPACE);
        assert_eq!(iq.query_type, crate::request::InfoQueryType::Get);

        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("expected <props> child");
        };
        assert!(
            nodes[0]
                .attrs
                .get("group")
                .is_some_and(|value| value == group_string.as_str())
        );
        assert!(nodes[0].attrs.get("hash").is_none());
    }

    #[test]
    fn group_props_response_keeps_experiments_and_ignores_sampling() {
        let spec = GroupPropsSpec::new(&Jid::new("120363000000000001", Server::Group));
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("props")
                .attr("hash", "group-hash")
                .children([
                    NodeBuilder::new("prop")
                        .attr("config_code", 23245)
                        .attr("config_value", "1")
                        .build(),
                    NodeBuilder::new("prop")
                        .attr("event_code", 5138)
                        .attr("sampling_weight", -1)
                        .build(),
                ])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.hash.as_deref(), Some("group-hash"));
        assert_eq!(result.experiment_props.len(), 1);
        assert_eq!(result.experiment_props[0].0, 23245);
        assert_eq!(result.experiment_props[0].1.as_str(), "1");
    }

    #[test]
    fn group_props_rejects_malformed_experiments() {
        let spec = GroupPropsSpec::new(&Jid::new("120363000000000001", Server::Group));
        for (code, value) in [
            (Some("26270"), None),
            (None, Some("1")),
            (Some("invalid"), Some("1")),
            (Some("0"), Some("1")),
        ] {
            let mut prop = NodeBuilder::new("prop");
            if let Some(code) = code {
                prop = prop.attr("config_code", code);
            }
            if let Some(value) = value {
                prop = prop.attr("config_value", value);
            }
            let response = NodeBuilder::new("iq")
                .children([NodeBuilder::new("props")
                    .attr("hash", "synthetic")
                    .children([prop.build()])
                    .build()])
                .build();
            assert!(spec.parse_response(&response.as_node_ref()).is_err());
        }
        for (second_value, valid) in [("1", true), ("0", false)] {
            let response = NodeBuilder::new("iq")
                .children([NodeBuilder::new("props")
                    .attr("hash", "synthetic")
                    .children(["1", second_value].map(|value| {
                        NodeBuilder::new("prop")
                            .attr("config_code", "26270")
                            .attr("config_value", value)
                            .build()
                    }))
                    .build()])
                .build();
            assert_eq!(spec.parse_response(&response.as_node_ref()).is_ok(), valid);
        }
    }

    #[test]
    fn test_props_spec_build_iq_no_params() {
        let spec = PropsSpec::new();
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, PROPS_NAMESPACE);
        assert_eq!(iq.query_type, crate::request::InfoQueryType::Get);

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].tag, "props");
            assert!(nodes[0].attrs.get("protocol").is_some_and(|v| v == "1"));
            assert!(nodes[0].attrs.get("hash").is_none());
            assert!(nodes[0].attrs.get("refresh_id").is_none());
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_props_spec_build_iq_with_hash() {
        let spec = PropsSpec::with_hash("abc123");
        let iq = spec.build_iq();

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert!(nodes[0].attrs.get("hash").is_some_and(|v| v == "abc123"));
            assert!(nodes[0].attrs.get("refresh_id").is_none());
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_props_spec_build_iq_with_refresh_id() {
        let spec = PropsSpec::with_refresh_id(42);
        let iq = spec.build_iq();

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert!(nodes[0].attrs.get("hash").is_none());
            assert!(nodes[0].attrs.get("refresh_id").is_some_and(|v| v == "42"));
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_props_spec_parse_response() {
        let spec = PropsSpec::new();
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("props")
                .attr("protocol", "1")
                .attr("ab_key", "test_key")
                .attr("hash", "abcdef")
                .attr("refresh", "3600")
                .attr("refresh_id", "123")
                .children([
                    NodeBuilder::new("prop")
                        .attr("config_code", "100")
                        .attr("config_value", "enabled")
                        .build(),
                    NodeBuilder::new("prop")
                        .attr("event_code", "5138")
                        .attr("sampling_weight", "-1")
                        .build(),
                    NodeBuilder::new("prop")
                        .attr("config_code", "200")
                        .attr("config_value", "disabled")
                        .attr("config_expo_key", "5")
                        .build(),
                ])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.ab_key, Some("test_key".to_string()));
        assert_eq!(result.hash, Some("abcdef".to_string()));
        assert_eq!(result.refresh, Some(3600));
        assert_eq!(result.refresh_id, Some(123));
        assert!(!result.delta_update);
        // 2 of 3 props are experiments (sampling props are skipped)
        assert_eq!(result.experiment_props.len(), 2);
    }

    #[test]
    fn test_props_spec_parse_response_delta_update() {
        let spec = PropsSpec::new();
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("props")
                .attr("protocol", "1")
                .attr("delta_update", "true")
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert!(result.delta_update);
    }

    #[test]
    fn test_ab_prop_protocol_node_round_trip() {
        let prop = AbProp {
            config_code: 123,
            config_value: "test_value".into(),
            config_expo_key: Some(456),
        };

        let node = prop.clone().into_node();
        let parsed = AbProp::try_from_node(&node).unwrap();

        assert_eq!(parsed.config_code, prop.config_code);
        assert_eq!(parsed.config_value, prop.config_value);
        assert_eq!(parsed.config_expo_key, prop.config_expo_key);
    }

    #[test]
    fn test_ab_prop_protocol_node_no_expo_key() {
        let prop = AbProp {
            config_code: 789,
            config_value: "another_value".into(),
            config_expo_key: None,
        };

        let node = prop.clone().into_node();
        let parsed = AbProp::try_from_node(&node).unwrap();

        assert_eq!(parsed.config_code, prop.config_code);
        assert_eq!(parsed.config_value, prop.config_value);
        assert_eq!(parsed.config_expo_key, None);
    }

    #[test]
    fn test_props_response_protocol_node_round_trip() {
        let response = PropsResponse {
            ab_key: Some("test_ab_key".to_string()),
            hash: Some("hash123".to_string()),
            refresh: Some(7200),
            refresh_id: Some(42),
            delta_update: true,
            experiment_props: vec![
                (100, CompactString::from("value1")),
                (200, CompactString::from("value2")),
            ],
        };

        let node = response.clone().into_node();
        let parsed = PropsResponse::try_from_node(&node).unwrap();

        assert_eq!(parsed.ab_key, response.ab_key);
        assert_eq!(parsed.hash, response.hash);
        assert_eq!(parsed.refresh, response.refresh);
        assert_eq!(parsed.refresh_id, response.refresh_id);
        assert_eq!(parsed.delta_update, response.delta_update);
        assert_eq!(parsed.experiment_props, response.experiment_props);
    }

    /// `<props>` must carry one `<prop config_code config_value/>` per
    /// experiment, matching `WASmaxInAbPropsExperimentConfigMixin`.
    #[test]
    fn test_props_response_into_node_emits_wa_web_compliant_prop_children() {
        let response = PropsResponse {
            ab_key: None,
            hash: None,
            refresh: None,
            refresh_id: None,
            delta_update: false,
            experiment_props: vec![
                (11_262, CompactString::from("1")),
                (11_103, CompactString::from("0")),
            ],
        };

        let node = response.into_node();

        let children = match node.content {
            Some(NodeContent::Nodes(c)) => c,
            other => panic!("<props> must have Node children, got {other:?}"),
        };
        assert_eq!(
            children.len(),
            2,
            "expected one <prop> per experiment_props entry"
        );

        let pairs: Vec<(String, String)> = children
            .iter()
            .map(|n| {
                assert_eq!(n.tag, "prop");
                let code = n
                    .attrs
                    .get("config_code")
                    .map(|v| v.to_string())
                    .expect("missing config_code");
                let value = n
                    .attrs
                    .get("config_value")
                    .map(|v| v.to_string())
                    .expect("missing config_value");
                (code, value)
            })
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("11262".to_string(), "1".to_string()),
                ("11103".to_string(), "0".to_string()),
            ],
            "code/value pairs must be preserved with their original mapping"
        );
    }

    #[test]
    fn test_props_response_protocol_node_minimal() {
        let response = PropsResponse {
            ab_key: None,
            hash: None,
            refresh: None,
            refresh_id: None,
            delta_update: false,
            experiment_props: vec![],
        };

        let node = response.clone().into_node();
        let parsed = PropsResponse::try_from_node(&node).unwrap();

        assert_eq!(parsed.ab_key, None);
        assert_eq!(parsed.hash, None);
        assert!(!parsed.delta_update);
        assert_eq!(parsed.experiment_props.len(), 0);
    }

    #[test]
    fn test_sampling_prop_protocol_node_round_trip() {
        let prop = SamplingProp {
            event_code: 5138,
            sampling_weight: -1,
        };

        let node = prop.clone().into_node();
        let parsed = SamplingProp::try_from_node(&node).unwrap();

        assert_eq!(parsed.event_code, prop.event_code);
        assert_eq!(parsed.sampling_weight, prop.sampling_weight);
    }

    /// The streamed consumer and the tree parser are two readings of one wire
    /// format and must agree, with and without a retained-code filter, on a
    /// response that also carries the sampling props and a leading sibling
    /// the client has no use for.
    #[test]
    fn streamed_consume_matches_tree_parse() {
        let props: Vec<Node> = (1..=300u32)
            .flat_map(|i| {
                let mut nodes = vec![
                    NodeBuilder::new("prop")
                        .attr("config_code", i)
                        .attr("config_value", format!("v{i}"))
                        .build(),
                ];
                if i % 10 == 0 {
                    nodes.push(
                        NodeBuilder::new("prop")
                            .attr("event_code", i)
                            .attr("sampling_weight", 100)
                            .build(),
                    );
                }
                nodes
            })
            .collect();
        let props_node = NodeBuilder::new("props")
            .attr("protocol", "1")
            .attr("ab_key", "key")
            .attr("hash", "h1")
            .attr("refresh", 3600u32)
            .attr("refresh_id", 5u32)
            .attr("delta_update", "false")
            .children(props)
            .build();
        let iq = NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", "1")
            .children(vec![NodeBuilder::new("unrelated").build(), props_node])
            .build();
        let packed = wacore_binary::marshal::marshal(&iq).unwrap();

        for spec in [PropsSpec::new(), PropsSpec::new().retaining([3, 30, 999])] {
            let owned = wacore_binary::OwnedNodeRef::new(packed[1..].to_vec()).unwrap();
            let tree = spec.parse_response(owned.get()).unwrap();

            let mut stream = NodeStream::from_packed(&packed).unwrap();
            stream.open().unwrap().expect("root");
            let streamed = spec.consume_response(&mut stream).unwrap();
            stream.finish().unwrap();

            assert_eq!(streamed.ab_key, tree.ab_key);
            assert_eq!(streamed.hash, tree.hash);
            assert_eq!(streamed.refresh, tree.refresh);
            assert_eq!(streamed.refresh_id, tree.refresh_id);
            assert_eq!(streamed.delta_update, tree.delta_update);
            assert_eq!(streamed.experiment_props, tree.experiment_props);
            if spec.retain.is_some() {
                assert_eq!(streamed.experiment_props.len(), 2);
            } else {
                assert_eq!(streamed.experiment_props.len(), 300);
            }
        }
    }
}
