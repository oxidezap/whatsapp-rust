//! Auto-generated A/B-props registry (WhatsApp 2.3000.1042742319). DO NOT EDIT.
//!
//! One `pub mod` per WA Web registry, one `pub const` per flag (screaming-snake of
//! its key) with the numeric `code` sent in the `<props>` IQ, value type, and default;
//! reference only what you use. Each module's `ALL` lists its flags; the top-level
//! `ALL` lists every module's slice.

#![allow(clippy::all)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbPropType {
    Bool,
    Int,
    Float,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct AbProp {
    pub name: &'static str,
    pub code: u32,
    pub value_type: AbPropType,
    pub default: AbDefault,
}

/// `WAWebABPropsConfigs` — 2036 flags.
pub mod web {
    use super::{AbDefault, AbProp, AbPropType};

    pub const A2UI_SUPPORTED_ELEMENTS: AbProp = AbProp {
        name: "a2ui_supported_elements",
        code: 32276,
        value_type: AbPropType::Str,
        default: AbDefault::Str("info_card, list_card"),
    };
    pub const ACP_REMOVAL: AbProp = AbProp {
        name: "acp_removal",
        code: 25255,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ACP_REMOVAL_EPOCH_TIME: AbProp = AbProp {
        name: "acp_removal_epoch_time",
        code: 25993,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1782518400),
    };
    pub const ACS_USE_GRAPHQL_FOR_FORWARD_COUNTER: AbProp = AbProp {
        name: "acs_use_graphql_for_forward_counter",
        code: 29218,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ACS_USE_GRAPHQL_FOR_MIGRATION_TEST: AbProp = AbProp {
        name: "acs_use_graphql_for_migration_test",
        code: 29217,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ACS_USE_GRAPHQL_ISSUANCE: AbProp = AbProp {
        name: "acs_use_graphql_issuance",
        code: 27219,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ADD_MEMBER_SYSTEM_MESSAGE: AbProp = AbProp {
        name: "add_member_system_message",
        code: 4579,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ADD_TO_CALL_IN_CHAT_THREAD: AbProp = AbProp {
        name: "add_to_call_in_chat_thread",
        code: 11700,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const ADDON_INFRA_ENABLE_PERF_LOGGING: AbProp = AbProp {
        name: "addon_infra_enable_perf_logging",
        code: 7567,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ADMIN_ONLY_MENTION_EVERYONE_GROUP_SIZE: AbProp = AbProp {
        name: "admin_only_mention_everyone_group_size",
        code: 20354,
        value_type: AbPropType::Int,
        default: AbDefault::Int(33),
    };
    pub const ADMIN_REVOKE_RECEIVER: AbProp = AbProp {
        name: "admin_revoke_receiver",
        code: 1177,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ADV_ACCEPT_HOSTED_DEVICES: AbProp = AbProp {
        name: "adv_accept_hosted_devices",
        code: 6939,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ADV_V2_M4_M5: AbProp = AbProp {
        name: "adv_v2_m4_m5",
        code: 753,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ADVANCED_CHAT_PRIVACY_CONTENT_UPDATE_JULY_25: AbProp = AbProp {
        name: "advanced_chat_privacy_content_update_july_25",
        code: 18025,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AFTER_READ_FALLBACK_DURATION: AbProp = AbProp {
        name: "after_read_fallback_duration",
        code: 26225,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const AFTER_READ_RECEIVER_ENABLED: AbProp = AbProp {
        name: "after_read_receiver_enabled",
        code: 25649,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AFTER_READ_SENDING_ENABLED: AbProp = AbProp {
        name: "after_read_sending_enabled",
        code: 25648,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_3P_AGENT_CHAT_ENABLED: AbProp = AbProp {
        name: "ai_3p_agent_chat_enabled",
        code: 31063,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_3P_AGENT_LINK_ENABLED: AbProp = AbProp {
        name: "ai_3p_agent_link_enabled",
        code: 31064,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_ALL_LANGUAGES_ENABLED: AbProp = AbProp {
        name: "ai_all_languages_enabled",
        code: 16091,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_ASSET_REPLACEMENT_ENABLED: AbProp = AbProp {
        name: "ai_asset_replacement_enabled",
        code: 28265,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_BIZAI_2WAY_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "ai_bizai_2way_integration_enabled",
        code: 26613,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_BIZAI_2WAY_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED: AbProp = AbProp {
        name: "ai_bizai_2way_integration_history_sync_pre_chatd_enabled",
        code: 26614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_BOT_INTEGRATION_BOT_PROFILE: AbProp = AbProp {
        name: "ai_bot_integration_bot_profile",
        code: 25268,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_BOT_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "ai_bot_integration_enabled",
        code: 25119,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_BOT_INTEGRATION_HISTORY_SYNC_ENABLED: AbProp = AbProp {
        name: "ai_bot_integration_history_sync_enabled",
        code: 25269,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_BOT_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED: AbProp = AbProp {
        name: "ai_bot_integration_history_sync_pre_chatd_enabled",
        code: 25469,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_META_AI_BANNER_M2_ENABLED: AbProp = AbProp {
        name: "ai_chat_meta_ai_banner_m2_enabled",
        code: 18784,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_META_AI_GLASSES_BANNER_ENABLED: AbProp = AbProp {
        name: "ai_chat_meta_ai_glasses_banner_enabled",
        code: 20405,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_META_AI_HOME_DEFAULT_LANDING_ENABLED: AbProp = AbProp {
        name: "ai_chat_meta_ai_home_default_landing_enabled",
        code: 28033,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_META_AI_HOME_WEB_ENABLED: AbProp = AbProp {
        name: "ai_chat_meta_ai_home_web_enabled",
        code: 27817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_META_AI_NULL_STATE_WEB_ENABLED: AbProp = AbProp {
        name: "ai_chat_meta_ai_null_state_web_enabled",
        code: 32817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREAD_CAPABILITY_ENABLED: AbProp = AbProp {
        name: "ai_chat_thread_capability_enabled",
        code: 22038,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_FUZZY_SEARCH_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_fuzzy_search_enabled",
        code: 27199,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_HISTORICAL_MESSAGES_MIGRATION_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_historical_messages_migration_enabled",
        code: 22070,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_HISTORY_ICON_VARIANT: AbProp = AbProp {
        name: "ai_chat_threads_history_icon_variant",
        code: 27316,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_CHAT_THREADS_IMPLICIT_ROUTING_STRATEGY: AbProp = AbProp {
        name: "ai_chat_threads_implicit_routing_strategy",
        code: 27519,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_CHAT_THREADS_INFRA_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_infra_enabled",
        code: 20652,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_INFRA_WEB_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_infra_web_enabled",
        code: 26776,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_PIN_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_pin_enabled",
        code: 25517,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CHAT_THREADS_PIN_MAX_COUNT: AbProp = AbProp {
        name: "ai_chat_threads_pin_max_count",
        code: 25520,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const AI_CHAT_THREADS_WEB_ENABLED: AbProp = AbProp {
        name: "ai_chat_threads_web_enabled",
        code: 23169,
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
    pub const AI_CONTEXTUAL_WRITING_HELP_ENABLED: AbProp = AbProp {
        name: "ai_contextual_writing_help_enabled",
        code: 22488,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_CONTEXTUAL_WRITING_HELP_LANGUAGES_AND_TONES_CONFIG: AbProp = AbProp {
        name: "ai_contextual_writing_help_languages_and_tones_config",
        code: 22797,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const AI_CONTEXTUAL_WRITING_HELP_NUM_SUGGESTIONS: AbProp = AbProp {
        name: "ai_contextual_writing_help_num_suggestions",
        code: 22759,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const AI_CONTINUOUS_SESSION_TRANSPARENCY_NOTICE_ENABLED: AbProp = AbProp {
        name: "ai_continuous_session_transparency_notice_enabled",
        code: 21510,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_DYNAMIC_MODE_SELECTOR_ENABLED: AbProp = AbProp {
        name: "ai_dynamic_mode_selector_enabled",
        code: 25287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_DYNAMIC_MODE_SELECTOR_TTL_SECONDS: AbProp = AbProp {
        name: "ai_dynamic_mode_selector_ttl_seconds",
        code: 25797,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const AI_EXPERIMENT_GRAPHQL_CONFIG: AbProp = AbProp {
        name: "ai_experiment_graphql_config",
        code: 9601,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_FBID_MIGRATION_INVOKE_RECEIVE_ENABLED: AbProp = AbProp {
        name: "ai_fbid_migration_invoke_receive_enabled",
        code: 12795,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_FBID_MIGRATION_RECEIVE_ENABLED: AbProp = AbProp {
        name: "ai_fbid_migration_receive_enabled",
        code: 11660,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_FILE_UPLOAD_COUNT_LIMIT: AbProp = AbProp {
        name: "ai_file_upload_count_limit",
        code: 25093,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_FILE_UPLOAD_SIZE_LIMIT_MB: AbProp = AbProp {
        name: "ai_file_upload_size_limit_mb",
        code: 25524,
        value_type: AbPropType::Int,
        default: AbDefault::Int(40),
    };
    pub const AI_FILE_UPLOAD_SUPPORTED_FILE_TYPES: AbProp = AbProp {
        name: "ai_file_upload_supported_file_types",
        code: 25090,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_FORWARD_ATTRIBUTION_ENABLED: AbProp = AbProp {
        name: "ai_forward_attribution_enabled",
        code: 18286,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_FORWARD_FLOW_SURFACE_META_AI_AS_CONTACT_ENABLED: AbProp = AbProp {
        name: "ai_forward_flow_surface_meta_ai_as_contact_enabled",
        code: 13879,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GENAI_STRAW_HAT: AbProp = AbProp {
        name: "ai_genai_straw_hat",
        code: 28268,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GIZMO_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "ai_gizmo_integration_enabled",
        code: 28584,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_ADD_IN_CALL_AHGC_ENABLED: AbProp = AbProp {
        name: "ai_group_call_add_in_call_ahgc_enabled",
        code: 24654,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_ADD_IN_CALL_LGC_ENABLED: AbProp = AbProp {
        name: "ai_group_call_add_in_call_lgc_enabled",
        code: 31717,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_MAX_VERSION_BY_COUNTRY: AbProp = AbProp {
        name: "ai_group_call_max_version_by_country",
        code: 24656,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_GROUP_CALL_MAX_VERSION_BY_PLATFORM: AbProp = AbProp {
        name: "ai_group_call_max_version_by_platform",
        code: 24655,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_GROUP_CALL_META_AI_ANIMATION_VERSION: AbProp = AbProp {
        name: "ai_group_call_meta_ai_animation_version",
        code: 32245,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_GROUP_CALL_START_CALL_AHGC_ENABLED: AbProp = AbProp {
        name: "ai_group_call_start_call_ahgc_enabled",
        code: 31716,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_START_CALL_LGC_ENABLED: AbProp = AbProp {
        name: "ai_group_call_start_call_lgc_enabled",
        code: 31713,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_START_CALL_LOGGING_ENABLED: AbProp = AbProp {
        name: "ai_group_call_start_call_logging_enabled",
        code: 32527,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_CALL_START_CALL_NOTICE_ID: AbProp = AbProp {
        name: "ai_group_call_start_call_notice_id",
        code: 31736,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_GROUP_CALL_VERSION: AbProp = AbProp {
        name: "ai_group_call_version",
        code: 24652,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const AI_GROUP_PARTICIPATION_ADD_TEE_ENABLED: AbProp = AbProp {
        name: "ai_group_participation_add_tee_enabled",
        code: 22236,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_PARTICIPATION_ENABLED: AbProp = AbProp {
        name: "ai_group_participation_enabled",
        code: 22171,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_PARTICIPATION_SEND_ENABLED: AbProp = AbProp {
        name: "ai_group_participation_send_enabled",
        code: 22184,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_SEND_MENTIONED_PUSHNAME_ENABLED: AbProp = AbProp {
        name: "ai_group_send_mentioned_pushname_enabled",
        code: 24361,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUP_TEE_HISTORY_SHARE_ENABLED: AbProp = AbProp {
        name: "ai_group_tee_history_share_enabled",
        code: 28278,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_GROUPS_OPEN_ENABLED: AbProp = AbProp {
        name: "ai_groups_open_enabled",
        code: 22165,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_COMMANDS_ENABLED: AbProp = AbProp {
        name: "ai_hatch_commands_enabled",
        code: 27660,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_DOCUMENT_UPLOAD_SIZE_LIMIT_MB: AbProp = AbProp {
        name: "ai_hatch_document_upload_size_limit_mb",
        code: 27873,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20),
    };
    pub const AI_HATCH_ENCRYPTED_MEDIA_ENABLED: AbProp = AbProp {
        name: "ai_hatch_encrypted_media_enabled",
        code: 32496,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_FORWARDING_HTML_ENABLED: AbProp = AbProp {
        name: "ai_hatch_forwarding_html_enabled",
        code: 27876,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_INTEGRATION_BOT_PROFILE: AbProp = AbProp {
        name: "ai_hatch_integration_bot_profile",
        code: 26190,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_HATCH_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "ai_hatch_integration_enabled",
        code: 26189,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_INTEGRATION_HISTORY_SYNC_ENABLED: AbProp = AbProp {
        name: "ai_hatch_integration_history_sync_enabled",
        code: 26517,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED: AbProp = AbProp {
        name: "ai_hatch_integration_history_sync_pre_chatd_enabled",
        code: 26445,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_INTEGRATION_TAB_ENABLED: AbProp = AbProp {
        name: "ai_hatch_integration_tab_enabled",
        code: 27356,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_MEDIA_UPLOAD_COUNT_LIMIT: AbProp = AbProp {
        name: "ai_hatch_media_upload_count_limit",
        code: 27897,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const AI_HATCH_SECRET_ENCRYPTED_MESSAGE_ENABLED: AbProp = AbProp {
        name: "ai_hatch_secret_encrypted_message_enabled",
        code: 31040,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_VIDEO_AVATARS_ENABLED: AbProp = AbProp {
        name: "ai_hatch_video_avatars_enabled",
        code: 31494,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HATCH_VIDEO_UPLOAD_ENABLED: AbProp = AbProp {
        name: "ai_hatch_video_upload_enabled",
        code: 27470,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_HOME_BOT_PROFILE_SYNC_INTERVAL_SEC: AbProp = AbProp {
        name: "ai_home_bot_profile_sync_interval_sec",
        code: 11168,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const AI_IMAGINE_LOADING_INDICATOR_ENABLED: AbProp = AbProp {
        name: "ai_imagine_loading_indicator_enabled",
        code: 22795,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_LEARNING_CLEAR_CHAT_DISABLE_EMPTY_CHATS: AbProp = AbProp {
        name: "ai_learning_clear_chat_disable_empty_chats",
        code: 26745,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_MAIBA_WASS_MIGRATION_RECEIVING: AbProp = AbProp {
        name: "ai_maiba_wass_migration_receiving",
        code: 27083,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_MAIBA_WASS_MIGRATION_SENDING: AbProp = AbProp {
        name: "ai_maiba_wass_migration_sending",
        code: 27084,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_META_AI_PREKEY_CLEANUP_ENABLED: AbProp = AbProp {
        name: "ai_meta_ai_prekey_cleanup_enabled",
        code: 31941,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_METABOT_DOCUMENT_OCR_IMAGE_CONVERSION_ENABLED: AbProp = AbProp {
        name: "ai_metabot_document_ocr_image_conversion_enabled",
        code: 22301,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_METABOT_DOCUMENT_UPLOAD_ENABLED: AbProp = AbProp {
        name: "ai_metabot_document_upload_enabled",
        code: 17957,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_METABOT_DOCUMENT_UPLOAD_PAGE_COUNT_LIMIT: AbProp = AbProp {
        name: "ai_metabot_document_upload_page_count_limit",
        code: 19987,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100000),
    };
    pub const AI_METABOT_DOCUMENT_UPLOAD_SIZE_LIMIT_MB: AbProp = AbProp {
        name: "ai_metabot_document_upload_size_limit_mb",
        code: 19823,
        value_type: AbPropType::Int,
        default: AbDefault::Int(40),
    };
    pub const AI_METABOT_IMAGE_INPUT_LANGUAGES: AbProp = AbProp {
        name: "ai_metabot_image_input_languages",
        code: 9163,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_METABOT_SEND_IMAGE_LIMIT: AbProp = AbProp {
        name: "ai_metabot_send_image_limit",
        code: 8685,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const AI_MIGRATE_AWAY_FROM_INLINE_TOS_ENABLED: AbProp = AbProp {
        name: "ai_migrate_away_from_inline_tos_enabled",
        code: 18843,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_MODE_SELECTOR_ENABLED: AbProp = AbProp {
        name: "ai_mode_selector_enabled",
        code: 23885,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_MODE_SELECTOR_MEDIA_EDITOR_ENABLED: AbProp = AbProp {
        name: "ai_mode_selector_media_editor_enabled",
        code: 30986,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_PDFN_NUX_AI_GROUP_TEE_DISCOVER_NOTICE_ID: AbProp = AbProp {
        name: "ai_pdfn_nux_ai_group_tee_discover_notice_id",
        code: 26171,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20260212"),
    };
    pub const AI_PDFN_NUX_AI_SIDE_CHAT_NOTICE_ID: AbProp = AbProp {
        name: "ai_pdfn_nux_ai_side_chat_notice_id",
        code: 31542,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" 20260211"),
    };
    pub const AI_PDFN_TOS_INLINE_NOTICES: AbProp = AbProp {
        name: "ai_pdfn_tos_inline_notices",
        code: 13970,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_PDFN_TOS_INVOKE_NOTICE_ID: AbProp = AbProp {
        name: "ai_pdfn_tos_invoke_notice_id",
        code: 9483,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_PDFN_TOS_MASTER_NOTICE_ID: AbProp = AbProp {
        name: "ai_pdfn_tos_master_notice_id",
        code: 15295,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_PDFN_TOS_NON_BLOCKING_NOTICES: AbProp = AbProp {
        name: "ai_pdfn_tos_non_blocking_notices",
        code: 15280,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_PDFN_TOS_SHORTCUT_NOTICE_ID: AbProp = AbProp {
        name: "ai_pdfn_tos_shortcut_notice_id",
        code: 9482,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_PTT_MAIN_GATE_SUPPORTED_LANGUAGES: AbProp = AbProp {
        name: "ai_ptt_main_gate_supported_languages",
        code: 9694,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_REPLY_MESSAGE_CONTEXT_MAX_COUNT: AbProp = AbProp {
        name: "ai_reply_message_context_max_count",
        code: 22024,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20),
    };
    pub const AI_REPLY_MESSAGE_CONTEXT_TRIGGER_MIN_COUNT: AbProp = AbProp {
        name: "ai_reply_message_context_trigger_min_count",
        code: 22025,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const AI_REWRITE_ENABLED: AbProp = AbProp {
        name: "ai_rewrite_enabled",
        code: 14219,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_REWRITE_ENTRY_POINT_MIN_WORDS: AbProp = AbProp {
        name: "ai_rewrite_entry_point_min_words",
        code: 14923,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const AI_REWRITE_IN_EXPRESSION_TRAY_ENABLED: AbProp = AbProp {
        name: "ai_rewrite_in_expression_tray_enabled",
        code: 16510,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_REWRITE_LANGUAGES_AND_TONES_CONFIG: AbProp = AbProp {
        name: "ai_rewrite_languages_and_tones_config",
        code: 21139,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const AI_REWRITE_LOAD_MORE_ENABLED: AbProp = AbProp {
        name: "ai_rewrite_load_more_enabled",
        code: 20918,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_REWRITE_NUM_SUGGESTIONS: AbProp = AbProp {
        name: "ai_rewrite_num_suggestions",
        code: 14924,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const AI_REWRITE_STACK_UNDO_ENABLED: AbProp = AbProp {
        name: "ai_rewrite_stack_undo_enabled",
        code: 16943,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_REWRITE_SUPPORTED_LANGUAGES: AbProp = AbProp {
        name: "ai_rewrite_supported_languages",
        code: 14220,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const AI_REWRITE_TONE_MODIFIERS: AbProp = AbProp {
        name: "ai_rewrite_tone_modifiers",
        code: 14743,
        value_type: AbPropType::Str,
        default: AbDefault::Str("rephrase,professional,funny,supportive"),
    };
    pub const AI_RICH_RESPONSE_FORWARD_RECEIVING_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_forward_receiving_enabled",
        code: 16682,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_FORWARD_SENDING_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_forward_sending_enabled",
        code: 16681,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_FORWARDING_VERIFICATION_ENABLED_V1: AbProp = AbProp {
        name: "ai_rich_response_forwarding_verification_enabled_v1",
        code: 19590,
        value_type: AbPropType::Str,
        default: AbDefault::Str("\"none\""),
    };
    pub const AI_RICH_RESPONSE_GRID_IMAGE_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_grid_image_enabled",
        code: 13578,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_INLINE_LINKS_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_inline_links_enabled",
        code: 23819,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_MAIN_GATE_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_main_gate_enabled",
        code: 12539,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const AI_RICH_RESPONSE_POST_CITATIONS_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_post_citations_enabled",
        code: 22672,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_REASONING_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_reasoning_enabled",
        code: 15589,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_REMOVE_GROUPED_CITATIONS_COUNT: AbProp = AbProp {
        name: "ai_rich_response_remove_grouped_citations_count",
        code: 31010,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_SIDE_BY_SIDE_SURVEY_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_side_by_side_survey_enabled",
        code: 17408,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_TEE_FORWARD_SENDING_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_tee_forward_sending_enabled",
        code: 32683,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_TEE_FORWARDING_VERIFICATION_ENFORCEMENT_V1: AbProp = AbProp {
        name: "ai_rich_response_tee_forwarding_verification_enforcement_v1",
        code: 32551,
        value_type: AbPropType::Str,
        default: AbDefault::Str("none"),
    };
    pub const AI_RICH_RESPONSE_UNKNOWN_SENDER_PREVIEW_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_unknown_sender_preview_enabled",
        code: 27355,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_UNKNOWN_SENDER_VERIFICATION_MASKING_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_unknown_sender_verification_masking_enabled",
        code: 27635,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_UR_MEDIA_GRID_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_ur_media_grid_enabled",
        code: 18746,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_WEB_STRUCTURED_RESPONSE_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_web_structured_response_enabled",
        code: 14141,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_RICH_RESPONSE_ZEITGEIST_CAROUSEL_ENABLED: AbProp = AbProp {
        name: "ai_rich_response_zeitgeist_carousel_enabled",
        code: 22750,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_ASK_BUTTON_WEB_ENABLED: AbProp = AbProp {
        name: "ai_search_ask_button_web_enabled",
        code: 30604,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_BAR_2025_REDESIGN_ENABLED: AbProp = AbProp {
        name: "ai_search_bar_2025_redesign_enabled",
        code: 16208,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_EXPERIENCE_ENABLED: AbProp = AbProp {
        name: "ai_search_experience_enabled",
        code: 8025,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_EXPERIENCE_WEB_ENABLED: AbProp = AbProp {
        name: "ai_search_experience_web_enabled",
        code: 18740,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_MAX_NUM_SUGGESTIONS: AbProp = AbProp {
        name: "ai_search_max_num_suggestions",
        code: 8076,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const AI_SEARCH_META_AI_SEND_BUTTON_ENABLED: AbProp = AbProp {
        name: "ai_search_meta_ai_send_button_enabled",
        code: 20603,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const AI_SEARCH_NULL_STATE_CONVO_STARTER_SUGGESTIONS_UPDATE_INTERVAL: AbProp = AbProp {
        name: "ai_search_null_state_convo_starter_suggestions_update_interval",
        code: 17623,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const AI_SEARCH_NULL_STATE_ENABLED: AbProp = AbProp {
        name: "ai_search_null_state_enabled",
        code: 8026,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SEARCH_NULL_STATE_ROW_COUNT: AbProp = AbProp {
        name: "ai_search_null_state_row_count",
        code: 8407,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const AI_SEARCH_NULL_STATE_UPDATE_INTERVAL: AbProp = AbProp {
        name: "ai_search_null_state_update_interval",
        code: 8100,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const AI_SESSION_TRANSPARENCY_META_AI_ENABLED: AbProp = AbProp {
        name: "ai_session_transparency_meta_ai_enabled",
        code: 23188,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SIMPLIFIED_PROFILE_PAGE_ENABLED: AbProp = AbProp {
        name: "ai_simplified_profile_page_enabled",
        code: 17104,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SUBSCRIPTION_IMAGINE_INTENT_ENABLED: AbProp = AbProp {
        name: "ai_subscription_imagine_intent_enabled",
        code: 28585,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_SUBSCRIPTION_METERING_ENABLED: AbProp = AbProp {
        name: "ai_subscription_metering_enabled",
        code: 30960,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_TAB_UNREAD_BADGE_RECENCY_WINDOW_HOURS: AbProp = AbProp {
        name: "ai_tab_unread_badge_recency_window_hours",
        code: 29800,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const AI_UGC_HIDE_ENABLED: AbProp = AbProp {
        name: "ai_ugc_hide_enabled",
        code: 20041,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UGC_NOT_AN_EXPERT_ENABLED: AbProp = AbProp {
        name: "ai_ugc_not_an_expert_enabled",
        code: 17285,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UNIFIED_RESPONSE_FORWARDING_SENDER_WEB_TIMESTAMP: AbProp = AbProp {
        name: "ai_unified_response_forwarding_sender_web_timestamp",
        code: 32008,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1781582400),
    };
    pub const AI_UNIFIED_RESPONSE_IMAGINE_RECEIVER_WEB_ENABLED: AbProp = AbProp {
        name: "ai_unified_response_imagine_receiver_web_enabled",
        code: 24109,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UNIFIED_RESPONSE_QPL_LOGGING: AbProp = AbProp {
        name: "ai_unified_response_qpl_logging",
        code: 24484,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UNIFIED_RESPONSE_RECEIVER_WEB_ENABLED: AbProp = AbProp {
        name: "ai_unified_response_receiver_web_enabled",
        code: 23348,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UNIFIED_RESPONSE_RECEIVER_WEB_ENABLED_V2: AbProp = AbProp {
        name: "ai_unified_response_receiver_web_enabled_v2",
        code: 25929,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_UNIFIED_RESPONSE_RECEIVER_WEB_TIMESTAMP_V2: AbProp = AbProp {
        name: "ai_unified_response_receiver_web_timestamp_v2",
        code: 25930,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1772082000),
    };
    pub const AI_UNIFIED_RESPONSE_SENDER_WEB_ENABLED: AbProp = AbProp {
        name: "ai_unified_response_sender_web_enabled",
        code: 23347,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_VIDEO_UPLOAD_SIZE_LIMIT_MB: AbProp = AbProp {
        name: "ai_video_upload_size_limit_mb",
        code: 25523,
        value_type: AbPropType::Int,
        default: AbDefault::Int(40),
    };
    pub const AI_VIDEO_UPLOAD_SUPPORT_LANGUAGES: AbProp = AbProp {
        name: "ai_video_upload_support_languages",
        code: 28336,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const AI_VIDEO_UPLOAD_WEB_ENABLED: AbProp = AbProp {
        name: "ai_video_upload_web_enabled",
        code: 31107,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_VOICE_ENTRY_POINT_LOGGING_ENABLED: AbProp = AbProp {
        name: "ai_voice_entry_point_logging_enabled",
        code: 13247,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_VOICE_MULTIMODAL_COMPOSER_ENABLED: AbProp = AbProp {
        name: "ai_voice_multimodal_composer_enabled",
        code: 12692,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_WEB_ASK_META_AI_ENABLED: AbProp = AbProp {
        name: "ai_web_ask_meta_ai_enabled",
        code: 23725,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_WEB_FORWARD_FLOW_ENABLED: AbProp = AbProp {
        name: "ai_web_forward_flow_enabled",
        code: 19676,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_WEB_META_AI_IMAGE_INPUT_ENABLED: AbProp = AbProp {
        name: "ai_web_meta_ai_image_input_enabled",
        code: 20522,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AI_WEB_META_AI_PDF_DOCUMENT_INPUT_ENABLED: AbProp = AbProp {
        name: "ai_web_meta_ai_pdf_document_input_enabled",
        code: 20581,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AIGC_VERSION: AbProp = AbProp {
        name: "aigc_version",
        code: 23692,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const ALBUM_V2_FORWARD_AS_ALBUM_ENABLED: AbProp = AbProp {
        name: "album_v2_forward_as_album_enabled",
        code: 10725,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ALBUM_V2_ITEM_WITH_CAPTION_IN_ALBUM_RECEIVER_ENABLED: AbProp = AbProp {
        name: "album_v2_item_with_caption_in_album_receiver_enabled",
        code: 11943,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ALBUM_V2_MIN_ITEMS_TO_SEND_ALBUM_WITH_CAPTION: AbProp = AbProp {
        name: "album_v2_min_items_to_send_album_with_caption",
        code: 12538,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const ALBUM_V2_MIN_ITEMS_TO_SEND_AS_ALBUM_ENABLED: AbProp = AbProp {
        name: "album_v2_min_items_to_send_as_album_enabled",
        code: 10848,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const ALBUM_V2_RECEIVING_ENABLED: AbProp = AbProp {
        name: "album_v2_receiving_enabled",
        code: 8528,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ALBUM_V2_SENDER_ENABLED: AbProp = AbProp {
        name: "album_v2_sender_enabled",
        code: 8529,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ALLOW_BACKFILL_WITH_V0_TO_V1_PRIMARY_VERSION_TRANSITION: AbProp = AbProp {
        name: "allow_backfill_with_v0_to_v1_primary_version_transition",
        code: 32186,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_EMOJI_FINAL_SET_ENABLED: AbProp = AbProp {
        name: "animated_emoji_final_set_enabled",
        code: 9757,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_EMOJI_SET_1_ENABLED: AbProp = AbProp {
        name: "animated_emoji_set_1_enabled",
        code: 9758,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_EMOJI_USE_LAZY_PARSING: AbProp = AbProp {
        name: "animated_emoji_use_lazy_parsing",
        code: 29140,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_EMOJIS_ENABLED: AbProp = AbProp {
        name: "animated_emojis_enabled",
        code: 3575,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_RACE_MERCEDES_CAR_EMOJI_ENABLED: AbProp = AbProp {
        name: "animated_race_mercedes_car_emoji_enabled",
        code: 13490,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_SOCCER_BALL_PROD_ENABLED: AbProp = AbProp {
        name: "animated_soccer_ball_prod_enabled",
        code: 27751,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANIMATED_SOCCER_BALL_TEST_ENABLED: AbProp = AbProp {
        name: "animated_soccer_ball_test_enabled",
        code: 27750,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ANYONE_CAN_LINK_TO_GROUPS: AbProp = AbProp {
        name: "anyone_can_link_to_groups",
        code: 13268,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const APP_EXIT_REASON_VERSION: AbProp = AbProp {
        name: "app_exit_reason_version",
        code: 8147,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const APPOINTMENT_BOOKING_BLOKS_ENABLED: AbProp = AbProp {
        name: "appointment_booking_bloks_enabled",
        code: 28146,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ATTACH_TRANSPORT_RTX: AbProp = AbProp {
        name: "attach_transport_rtx",
        code: 16201,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AUDIO_LEVEL_SPEAKING_THRESHOLD: AbProp = AbProp {
        name: "audio_level_speaking_threshold",
        code: 1213,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const AURA_APP_THEMES_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_app_themes_benefit_active",
        code: 23273,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_APP_THEMES_ENABLED: AbProp = AbProp {
        name: "aura_app_themes_enabled",
        code: 23274,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_ENABLED: AbProp = AbProp {
        name: "aura_enabled",
        code: 23270,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_FOCUS_LISTS_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_focus_lists_benefit_active",
        code: 32724,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_FOCUS_LISTS_ENABLED: AbProp = AbProp {
        name: "aura_focus_lists_enabled",
        code: 32723,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_KILL_SWITCH: AbProp = AbProp {
        name: "aura_kill_switch",
        code: 28345,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_MEDIA_OFFLOAD_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_media_offload_benefit_active",
        code: 29308,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_MEDIA_OFFLOAD_ENABLED: AbProp = AbProp {
        name: "aura_media_offload_enabled",
        code: 29391,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_PINNED_CHATS_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_pinned_chats_benefit_active",
        code: 23278,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_PINNED_CHATS_ENABLED: AbProp = AbProp {
        name: "aura_pinned_chats_enabled",
        code: 23277,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_PINNED_CHATS_TARGETED_NUX_FORCE: AbProp = AbProp {
        name: "aura_pinned_chats_targeted_nux_force",
        code: 27135,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_PREMIUM_STICKERS_KILLSWITCH: AbProp = AbProp {
        name: "aura_premium_stickers_killswitch",
        code: 27946,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_RINGTONES_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_ringtones_benefit_active",
        code: 24050,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_RINGTONES_ENABLED: AbProp = AbProp {
        name: "aura_ringtones_enabled",
        code: 24047,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_SETTINGS_ROW_ENABLED: AbProp = AbProp {
        name: "aura_settings_row_enabled",
        code: 27210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_STATUS_SEARCH_ENABLED: AbProp = AbProp {
        name: "aura_status_search_enabled",
        code: 26346,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_STATUS_SEARCH_MAX_VIEWERS: AbProp = AbProp {
        name: "aura_status_search_max_viewers",
        code: 26545,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1000),
    };
    pub const AURA_STATUS_SEARCH_TIMEOUT_THRESHOLD: AbProp = AbProp {
        name: "aura_status_search_timeout_threshold",
        code: 26546,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const AURA_STICKERS_BENEFIT_ACTIVE: AbProp = AbProp {
        name: "aura_stickers_benefit_active",
        code: 24801,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_STICKERS_ENABLED: AbProp = AbProp {
        name: "aura_stickers_enabled",
        code: 24800,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_STICKERS_OVERLAY_ANIMATION_ENABLED: AbProp = AbProp {
        name: "aura_stickers_overlay_animation_enabled",
        code: 25210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AURA_STICKERS_PREVIEW_MAX_ANIMATION_COUNT: AbProp = AbProp {
        name: "aura_stickers_preview_max_animation_count",
        code: 26602,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const AURA_SUBSCRIPTION_SIMULATION_ENABLED: AbProp = AbProp {
        name: "aura_subscription_simulation_enabled",
        code: 26086,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AUTH_AGENT_SOFT_OFFBOARDING_ENABLED: AbProp = AbProp {
        name: "auth_agent_soft_offboarding_enabled",
        code: 28802,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AUTH_AGENTS_CONSUMER_EXP_ENABLED: AbProp = AbProp {
        name: "auth_agents_consumer_exp_enabled",
        code: 26492,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const AUTH_AGENTS_CONSUMER_OFFBOARDING_EXP_ENABLED: AbProp = AbProp {
        name: "auth_agents_consumer_offboarding_exp_enabled",
        code: 30360,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BACKFILL_SUPPORTS_COEX_COMPANION: AbProp = AbProp {
        name: "backfill_supports_coex_companion",
        code: 27975,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BANNED_SHOPS_UX_ENABLED: AbProp = AbProp {
        name: "banned_shops_ux_enabled",
        code: 957,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_1: AbProp = AbProp {
        name: "bb_chat_list_banner_1",
        code: 32208,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_10: AbProp = AbProp {
        name: "bb_chat_list_banner_10",
        code: 32217,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_2: AbProp = AbProp {
        name: "bb_chat_list_banner_2",
        code: 32209,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_3: AbProp = AbProp {
        name: "bb_chat_list_banner_3",
        code: 32210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_4: AbProp = AbProp {
        name: "bb_chat_list_banner_4",
        code: 32211,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_5: AbProp = AbProp {
        name: "bb_chat_list_banner_5",
        code: 32212,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_6: AbProp = AbProp {
        name: "bb_chat_list_banner_6",
        code: 32213,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_7: AbProp = AbProp {
        name: "bb_chat_list_banner_7",
        code: 32214,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_8: AbProp = AbProp {
        name: "bb_chat_list_banner_8",
        code: 32215,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_9: AbProp = AbProp {
        name: "bb_chat_list_banner_9",
        code: 32216,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_V1: AbProp = AbProp {
        name: "bb_chat_list_banner_v1",
        code: 32373,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_BANNER_V2: AbProp = AbProp {
        name: "bb_chat_list_banner_v2",
        code: 32374,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_1: AbProp = AbProp {
        name: "bb_chat_list_mab_1",
        code: 31965,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_10: AbProp = AbProp {
        name: "bb_chat_list_mab_10",
        code: 31966,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_2: AbProp = AbProp {
        name: "bb_chat_list_mab_2",
        code: 31961,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_3: AbProp = AbProp {
        name: "bb_chat_list_mab_3",
        code: 31960,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_4: AbProp = AbProp {
        name: "bb_chat_list_mab_4",
        code: 31964,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_5: AbProp = AbProp {
        name: "bb_chat_list_mab_5",
        code: 31962,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_6: AbProp = AbProp {
        name: "bb_chat_list_mab_6",
        code: 31967,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_7: AbProp = AbProp {
        name: "bb_chat_list_mab_7",
        code: 31969,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_8: AbProp = AbProp {
        name: "bb_chat_list_mab_8",
        code: 31963,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BB_CHAT_LIST_MAB_9: AbProp = AbProp {
        name: "bb_chat_list_mab_9",
        code: 31968,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_AGENT_3P_STORE_LINKS_ENABLED: AbProp = AbProp {
        name: "biz_ai_agent_3p_store_links_enabled",
        code: 24114,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const BIZ_AI_AGENT_THREAD_STATUS_HISTORY_SYNC_ENABLED: AbProp = AbProp {
        name: "biz_ai_agent_thread_status_history_sync_enabled",
        code: 20099,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_AUTO_SAVE_ENABLED: AbProp = AbProp {
        name: "biz_ai_auto_save_enabled",
        code: 13464,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_COACHING_ENABLED: AbProp = AbProp {
        name: "biz_ai_coaching_enabled",
        code: 13465,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_CONSUMER_TOS_NOTICE_IQ_WEB: AbProp = AbProp {
        name: "biz_ai_consumer_tos_notice_iq_web",
        code: 24754,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_CONSUMER_TOS_UPDATE_WEB: AbProp = AbProp {
        name: "biz_ai_consumer_tos_update_web",
        code: 23880,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_FAB_CONFIRM_MODAL_ENABLED: AbProp = AbProp {
        name: "biz_ai_fab_confirm_modal_enabled",
        code: 32846,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_IN_THREAD_UNMUTE_V2: AbProp = AbProp {
        name: "biz_ai_in_thread_unmute_v2",
        code: 15523,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_LARGE_SCREENS_GATE_FETCH_ENABLED: AbProp = AbProp {
        name: "biz_ai_large_screens_gate_fetch_enabled",
        code: 31880,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_PRIORITY_LIST_ENABLED: AbProp = AbProp {
        name: "biz_ai_priority_list_enabled",
        code: 16420,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_RESPONDING_LIST_ENABLED: AbProp = AbProp {
        name: "biz_ai_responding_list_enabled",
        code: 26670,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_SMB_AGENTS_AUTOMATIC_REPLY_ENABLED: AbProp = AbProp {
        name: "biz_ai_smb_agents_automatic_reply_enabled",
        code: 8505,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_TOOLS_SETTINGS: AbProp = AbProp {
        name: "biz_ai_tools_settings",
        code: 28552,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_TOOLS_SYNC: AbProp = AbProp {
        name: "biz_ai_tools_sync",
        code: 29383,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_TOS_VARIANT: AbProp = AbProp {
        name: "biz_ai_tos_variant",
        code: 20833,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const BIZ_AI_WEB_AI_HUB_TAP_CTA_SHOW_ALERT: AbProp = AbProp {
        name: "biz_ai_web_ai_hub_tap_cta_show_alert",
        code: 17093,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_WEB_BULK_THREAD_CONTROL_ENABLED: AbProp = AbProp {
        name: "biz_ai_web_bulk_thread_control_enabled",
        code: 32588,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_WEB_ONBOARDING_HANDOFF: AbProp = AbProp {
        name: "biz_ai_web_onboarding_handoff",
        code: 29298,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_AI_WEB_ONBOARDING_HANDOFF_KILLSWITCH: AbProp = AbProp {
        name: "biz_ai_web_onboarding_handoff_killswitch",
        code: 32263,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_VPV_DIMENSIONS_LOGGING_ENABLED: AbProp = AbProp {
        name: "biz_vpv_dimensions_logging_enabled",
        code: 30266,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BIZ_VPV_IMPRESSION_LOGGING_ENABLED: AbProp = AbProp {
        name: "biz_vpv_impression_logging_enabled",
        code: 25465,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLOCKLIST_SYSTEM_MSG_ON_FULL_REFETCH: AbProp = AbProp {
        name: "blocklist_system_msg_on_full_refetch",
        code: 28070,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLOKS_A2UI_STEPS_ENABLED: AbProp = AbProp {
        name: "bloks_a2ui_steps_enabled",
        code: 32251,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLUE_EDUCATION_ENABLED: AbProp = AbProp {
        name: "blue_education_enabled",
        code: 5295,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLUE_EDUCATION_V2_ENABLED: AbProp = AbProp {
        name: "blue_education_v2_enabled",
        code: 6127,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLUE_ENABLED: AbProp = AbProp {
        name: "blue_enabled",
        code: 5276,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLUE_PROFILE_LOCKED_UI_ENABLED: AbProp = AbProp {
        name: "blue_profile_locked_ui_enabled",
        code: 6337,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BLUE_STRINGS_ENABLED: AbProp = AbProp {
        name: "blue_strings_enabled",
        code: 5846,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_AVATAR_ENABLED: AbProp = AbProp {
        name: "bonsai_avatar_enabled",
        code: 4532,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_CAROUSEL_ENABLED: AbProp = AbProp {
        name: "bonsai_carousel_enabled",
        code: 5283,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_CAROUSEL_HQ_THUMBNAIL_ENABLED: AbProp = AbProp {
        name: "bonsai_carousel_hq_thumbnail_enabled",
        code: 6459,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_CAROUSEL_REELS_PROFILE_PHOTO_ENABLED: AbProp = AbProp {
        name: "bonsai_carousel_reels_profile_photo_enabled",
        code: 6458,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_CHAT_LIST_ENTRY_POINT_ENABLED: AbProp = AbProp {
        name: "bonsai_chat_list_entry_point_enabled",
        code: 6251,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_ENABLED: AbProp = AbProp {
        name: "bonsai_enabled",
        code: 4010,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_ENGLISH_ONLY: AbProp = AbProp {
        name: "bonsai_english_only",
        code: 5637,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_FP_UGC_SENDER: AbProp = AbProp {
        name: "bonsai_fp_ugc_sender",
        code: 9541,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_META_AI_SHORTCUT_TOS_ENABLED: AbProp = AbProp {
        name: "bonsai_meta_ai_shortcut_tos_enabled",
        code: 8004,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_PTT_ENABLED: AbProp = AbProp {
        name: "bonsai_ptt_enabled",
        code: 4416,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BONSAI_SUPPORTED_LANGUAGES: AbProp = AbProp {
        name: "bonsai_supported_languages",
        code: 7848,
        value_type: AbPropType::Str,
        default: AbDefault::Str("en"),
    };
    pub const BONSAI_TI_TIMEOUT_DURATION_MS: AbProp = AbProp {
        name: "bonsai_ti_timeout_duration_ms",
        code: 4736,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10000),
    };
    pub const BONSAI_UPDATE_INTERVAL: AbProp = AbProp {
        name: "bonsai_update_interval",
        code: 4417,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const BONSAI_WORD_STREAMING_ENABLED: AbProp = AbProp {
        name: "bonsai_word_streaming_enabled",
        code: 4974,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BOOKING_CONFIRMATION_ENABLED_WA_WEB: AbProp = AbProp {
        name: "booking_confirmation_enabled_wa_web",
        code: 23559,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BOT_3P_ENABLED: AbProp = AbProp {
        name: "bot_3p_enabled",
        code: 5587,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BOT_3P_STATUS: AbProp = AbProp {
        name: "bot_3p_status",
        code: 5985,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const BOT_PROFILE_SYNC_MIGRATION_ENABLED: AbProp = AbProp {
        name: "bot_profile_sync_migration_enabled",
        code: 17485,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_ENABLE_PAYMENT_LOGOS_ON_BUBBLE: AbProp = AbProp {
        name: "br_enable_payment_logos_on_bubble",
        code: 8160,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_PAYMENTS_HOME_DURATION_RULE_FOR_PUX_BANNER: AbProp = AbProp {
        name: "br_payments_home_duration_rule_for_pux_banner",
        code: 22249,
        value_type: AbPropType::Int,
        default: AbDefault::Int(604800),
    };
    pub const BR_PAYMENTS_PAYMENT_DETECTION_ENHANCEMENT: AbProp = AbProp {
        name: "br_payments_payment_detection_enhancement",
        code: 27309,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_PAYMENTS_PAYMENT_REQUEST_CTA: AbProp = AbProp {
        name: "br_payments_payment_request_cta",
        code: 25599,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_PAYMENTS_PIX_GROUPS_ENABLED: AbProp = AbProp {
        name: "br_payments_pix_groups_enabled",
        code: 21741,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_PIX_KEY_BUBBLE_CONTENT_UPDATE: AbProp = AbProp {
        name: "br_pix_key_bubble_content_update",
        code: 26033,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_SMB_PAYMENTSHOME_ENABLED: AbProp = AbProp {
        name: "br_smb_paymentshome_enabled",
        code: 23042,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BR_SMB_PIX_PAYMENT_REQUEST_VARIANT: AbProp = AbProp {
        name: "br_smb_pix_payment_request_variant",
        code: 24388,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const BRIGADING_PRIVACY_SETTING_ENABLED: AbProp = AbProp {
        name: "brigading_privacy_setting_enabled",
        code: 9876,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BROADCAST_TO_YOUR_FOLLOWERS_ENABLED: AbProp = AbProp {
        name: "broadcast_to_your_followers_enabled",
        code: 31580,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_ABPROPS_UPLOADED_ON_SUBMISSOIN: AbProp = AbProp {
        name: "bug_reporting_abprops_uploaded_on_submissoin",
        code: 24850,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_ASYNC_ATTACHMENTS_ENABLED: AbProp = AbProp {
        name: "bug_reporting_async_attachments_enabled",
        code: 23978,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_ATTACH_PATHFINDER_PRE_BUG_CREATION: AbProp = AbProp {
        name: "bug_reporting_attach_pathfinder_pre_bug_creation",
        code: 26311,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const BUG_REPORTING_ATTACH_VIEW_DUMP_PRE_BUG_CREATION: AbProp = AbProp {
        name: "bug_reporting_attach_view_dump_pre_bug_creation",
        code: 26307,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const BUG_REPORTING_NOT_SHIPPED_YET_ENABLED: AbProp = AbProp {
        name: "bug_reporting_not_shipped_yet_enabled",
        code: 29458,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_PRE_UPLOADED_ATTACHMENTS_ON_BUG_CREATION_ENABLED: AbProp = AbProp {
        name: "bug_reporting_pre_uploaded_attachments_on_bug_creation_enabled",
        code: 24422,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_RID_IN_FLYTRAP: AbProp = AbProp {
        name: "bug_reporting_rid_in_flytrap",
        code: 24421,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUG_REPORTING_USING_GRAPHQL: AbProp = AbProp {
        name: "bug_reporting_using_graphql",
        code: 24161,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUSINESS_BROADCAST_CAMPAIGN_SYNCD_ENABLED: AbProp = AbProp {
        name: "business_broadcast_campaign_syncd_enabled",
        code: 26426,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const BUSINESS_BROADCAST_INSIGHTS_CAMPAIGN_TTL_DAYS: AbProp = AbProp {
        name: "business_broadcast_insights_campaign_ttl_days",
        code: 27218,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const BUSINESS_BROADCAST_INSIGHTS_SYNC_PAST_X_DAYS: AbProp = AbProp {
        name: "business_broadcast_insights_sync_past_x_days",
        code: 27082,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const BUSINESS_BROADCASTS_SYNCD_WAM_LOGGING: AbProp = AbProp {
        name: "business_broadcasts_syncd_wam_logging",
        code: 28277,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUSINESS_TOOL_ENHANCED_LOGGING: AbProp = AbProp {
        name: "business_tool_enhanced_logging",
        code: 4427,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const BUYER_INITIATED_ORDER_REQUEST_VARIANT_ENABLED: AbProp = AbProp {
        name: "buyer_initiated_order_request_variant_enabled",
        code: 5114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALL_ADMIN_VERSION: AbProp = AbProp {
        name: "call_admin_version",
        code: 2912,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALL_INFO_OPTIMIZATIONS_1ON1: AbProp = AbProp {
        name: "call_info_optimizations_1on1",
        code: 31095,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALL_INFO_OPTIMIZATIONS_AHGC_CALL_LINK: AbProp = AbProp {
        name: "call_info_optimizations_ahgc_call_link",
        code: 31096,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALL_INFO_OPTIMIZATIONS_LGC: AbProp = AbProp {
        name: "call_info_optimizations_lgc",
        code: 31094,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALL_INFO_OPTIMIZATIONS_VERSION: AbProp = AbProp {
        name: "call_info_optimizations_version",
        code: 27483,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALL_INFO_USE_TYPED_JID: AbProp = AbProp {
        name: "call_info_use_typed_jid",
        code: 29027,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALL_OFFER_FAILED_SOFT_LANDING_SCREEN_VERSION: AbProp = AbProp {
        name: "call_offer_failed_soft_landing_screen_version",
        code: 10559,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALL_SCREEN_SHARE_DUAL_STREAM_APP_UPDATE_DIALOG_ENABLED: AbProp = AbProp {
        name: "call_screen_share_dual_stream_app_update_dialog_enabled",
        code: 31922,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CALLEE_ACCEPT_TIMEOUT_MS: AbProp = AbProp {
        name: "callee_accept_timeout_ms",
        code: 6007,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30000),
    };
    pub const CALLING_32P_VERSION: AbProp = AbProp {
        name: "calling_32p_version",
        code: 7709,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_AUDIO_SHARE_VERSION: AbProp = AbProp {
        name: "calling_audio_share_version",
        code: 6598,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_AV_SYNC_WEBRTC: AbProp = AbProp {
        name: "calling_av_sync_webrtc",
        code: 24599,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALLING_E2E_KEYGEN_VIA_SELF_LID: AbProp = AbProp {
        name: "calling_e2e_keygen_via_self_lid",
        code: 26411,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_LID_VERSION: AbProp = AbProp {
        name: "calling_lid_version",
        code: 3358,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_RUST_MIGRATION_BITMAP: AbProp = AbProp {
        name: "calling_rust_migration_bitmap",
        code: 17954,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_RUST_MIGRATION_INCOMING_ACK_STANZA_BITMAP: AbProp = AbProp {
        name: "calling_rust_migration_incoming_ack_stanza_bitmap",
        code: 28434,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_RUST_MIGRATION_INCOMING_STANZA_BITMAP: AbProp = AbProp {
        name: "calling_rust_migration_incoming_stanza_bitmap",
        code: 26876,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_SCREEN_SHARE_MILESTONE_VERSION: AbProp = AbProp {
        name: "calling_screen_share_milestone_version",
        code: 30350,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const CALLING_UX_LOGGING_BITMAP: AbProp = AbProp {
        name: "calling_ux_logging_bitmap",
        code: 8175,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_VOICEMAIL_ATTACHED_ICCE_ENABLED: AbProp = AbProp {
        name: "calling_voicemail_attached_icce_enabled",
        code: 30383,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CALLING_VOICEMAIL_ENABLED: AbProp = AbProp {
        name: "calling_voicemail_enabled",
        code: 17685,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALLING_VOICEMAIL_QUOTED_REPLIES_ENABLED: AbProp = AbProp {
        name: "calling_voicemail_quoted_replies_enabled",
        code: 30165,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CALLS_TAB_USERNAME_GLOBAL_SEARCH_ENABLED: AbProp = AbProp {
        name: "calls_tab_username_global_search_enabled",
        code: 17698,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CAMERA_ERROR_BANNERS_VERSION: AbProp = AbProp {
        name: "camera_error_banners_version",
        code: 10584,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CAMERA_HEALTH_CHECK_DELAY: AbProp = AbProp {
        name: "camera_health_check_delay",
        code: 8739,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5000),
    };
    pub const CAMERA_HEALTH_CHECK_PERIOD: AbProp = AbProp {
        name: "camera_health_check_period",
        code: 8740,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2000),
    };
    pub const CANONICAL_ENT_COMPANION_SERVER_CACHED_NONCE_ENABLED: AbProp = AbProp {
        name: "canonical_ent_companion_server_cached_nonce_enabled",
        code: 28399,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CAROUSEL_MESSAGE_CLIENT_ENABLED: AbProp = AbProp {
        name: "carousel_message_client_enabled",
        code: 4668,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CATALOG_CATEGORIES_ENABLED: AbProp = AbProp {
        name: "catalog_categories_enabled",
        code: 1514,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CCI_COMPLIANCE_CTWA: AbProp = AbProp {
        name: "cci_compliance_ctwa",
        code: 24983,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CCI_COMPLIANCE_CTWA_LEARN_MORE_HYPERLINK: AbProp = AbProp {
        name: "cci_compliance_ctwa_learn_more_hyperlink",
        code: 25366,
        value_type: AbPropType::Str,
        default: AbDefault::Str("https://faq.whatsapp.com/785493319976156/"),
    };
    pub const CCI_COMPLIANCE_MM: AbProp = AbProp {
        name: "cci_compliance_mm",
        code: 24853,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_ALBUM_V2_RECEIVING_ENABLED: AbProp = AbProp {
        name: "channel_album_v2_receiving_enabled",
        code: 13219,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_ALBUM_V2_SENDER_ENABLED: AbProp = AbProp {
        name: "channel_album_v2_sender_enabled",
        code: 13220,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_ENFORCEMENT_LOGGING_ENABLED: AbProp = AbProp {
        name: "channel_enforcement_logging_enabled",
        code: 20549,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_ENFORCEMENT_POLICY_EDUCATION_ENABLED: AbProp = AbProp {
        name: "channel_enforcement_policy_education_enabled",
        code: 23745,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_FORWARD_BOTTOM_BUTTON_ENABLED: AbProp = AbProp {
        name: "channel_forward_bottom_button_enabled",
        code: 9422,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_FORWARD_TO_CHAT_ENABLED: AbProp = AbProp {
        name: "channel_forward_to_chat_enabled",
        code: 4338,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_FORWARD_TO_CHAT_V2_MESSAGE_NAVIGATION_ENABLED: AbProp = AbProp {
        name: "channel_forward_to_chat_v2_message_navigation_enabled",
        code: 4682,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_OSA_REPORTING_ENABLED: AbProp = AbProp {
        name: "channel_osa_reporting_enabled",
        code: 12987,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_PHOTO_POLL_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channel_photo_poll_receiver_enabled",
        code: 11980,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_PHOTO_POLL_SENDER_ENABLED: AbProp = AbProp {
        name: "channel_photo_poll_sender_enabled",
        code: 11989,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_PLAYABLE_MESSAGE_VIEWS_DURATION_MILLISECONDS: AbProp = AbProp {
        name: "channel_playable_message_views_duration_milliseconds",
        code: 4722,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3000),
    };
    pub const CHANNEL_POLL_FORWARDING_ENABLED: AbProp = AbProp {
        name: "channel_poll_forwarding_enabled",
        code: 10412,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_PULL_MESSAGE_UPDATES_THRESHOLD_SECONDS: AbProp = AbProp {
        name: "channel_pull_message_updates_threshold_seconds",
        code: 4326,
        value_type: AbPropType::Int,
        default: AbDefault::Int(120),
    };
    pub const CHANNEL_REACTIONS_ENABLED: AbProp = AbProp {
        name: "channel_reactions_enabled",
        code: 4306,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_REACTIONS_SENDER_LIST_ENABLED: AbProp = AbProp {
        name: "channel_reactions_sender_list_enabled",
        code: 5185,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNEL_REACTIONS_SETTINGS_ENABLED: AbProp = AbProp {
        name: "channel_reactions_settings_enabled",
        code: 4887,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STATUS_CONSUMPTION: AbProp = AbProp {
        name: "channel_status_consumption",
        code: 23995,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STATUS_CREATION: AbProp = AbProp {
        name: "channel_status_creation",
        code: 23994,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STATUS_DEEPLINK_ENABLED: AbProp = AbProp {
        name: "channel_status_deeplink_enabled",
        code: 28500,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNEL_STATUS_FILL_GAP_PAGE_SIZE: AbProp = AbProp {
        name: "channel_status_fill_gap_page_size",
        code: 27777,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const CHANNEL_STATUS_FORWARDING_ENABLED: AbProp = AbProp {
        name: "channel_status_forwarding_enabled",
        code: 28479,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STATUS_HELP_ENABLED: AbProp = AbProp {
        name: "channel_status_help_enabled",
        code: 30999,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STATUS_RESHARING_ENABLED: AbProp = AbProp {
        name: "channel_status_resharing_enabled",
        code: 30155,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_STICKER_PACK_FORWARDING: AbProp = AbProp {
        name: "channel_sticker_pack_forwarding",
        code: 20212,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_SUPPORTED_MESSAGE_TYPES: AbProp = AbProp {
        name: "channel_supported_message_types",
        code: 3919,
        value_type: AbPropType::Str,
        default: AbDefault::Str("1, 2, 3, 5, 9, 10, 12, 15"),
    };
    pub const CHANNEL_TO_CHANNEL_FORWARDING_LOGGING_ENABLED: AbProp = AbProp {
        name: "channel_to_channel_forwarding_logging_enabled",
        code: 8227,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_US_NCII_REPORTING_ENABLED: AbProp = AbProp {
        name: "channel_us_ncii_reporting_enabled",
        code: 25818,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_VIEW_COUNTS_ENABLED: AbProp = AbProp {
        name: "channel_view_counts_enabled",
        code: 4721,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNEL_VIEWS_DURATION_MILLISECONDS: AbProp = AbProp {
        name: "channel_views_duration_milliseconds",
        code: 4648,
        value_type: AbPropType::Int,
        default: AbDefault::Int(250),
    };
    pub const CHANNEL_VIEWS_VPV_DEFINITION_ENABLED: AbProp = AbProp {
        name: "channel_views_vpv_definition_enabled",
        code: 23616,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNEL_WEB_EMBEDDING_ENABLED: AbProp = AbProp {
        name: "channel_web_embedding_enabled",
        code: 31664,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_INSIGHTS_GIZMOS_ENABLED: AbProp = AbProp {
        name: "channels_admin_insights_gizmos_enabled",
        code: 9641,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_NOTIFICATIONS_ENABLED: AbProp = AbProp {
        name: "channels_admin_notifications_enabled",
        code: 18560,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_NOTIFICATIONS_FORWARDS_ENABLED: AbProp = AbProp {
        name: "channels_admin_notifications_forwards_enabled",
        code: 32808,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_PROFILES_FORWARDING_TO_CHATS_ENABLED: AbProp = AbProp {
        name: "channels_admin_profiles_forwarding_to_chats_enabled",
        code: 23170,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_PROFILES_LIST_ENABLED: AbProp = AbProp {
        name: "channels_admin_profiles_list_enabled",
        code: 23174,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_PROFILES_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_admin_profiles_receiver_enabled",
        code: 22318,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_PROFILES_SENDER_ENABLED: AbProp = AbProp {
        name: "channels_admin_profiles_sender_enabled",
        code: 22316,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_REPLY_ENABLED: AbProp = AbProp {
        name: "channels_admin_reply_enabled",
        code: 7211,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ADMIN_REPLY_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_admin_reply_receiver_enabled",
        code: 7237,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ALBUM_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_album_receiver_enabled",
        code: 23809,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ALBUM_SENDER_ENABLED: AbProp = AbProp {
        name: "channels_album_sender_enabled",
        code: 23859,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_AUDIO_FILES_DISPLAY_WAVEFORM_ENABLED: AbProp = AbProp {
        name: "channels_audio_files_display_waveform_enabled",
        code: 6996,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_AUDIO_FILES_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_audio_files_receiver_enabled",
        code: 6506,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_AUDIO_FILES_SENDER_ENABLED: AbProp = AbProp {
        name: "channels_audio_files_sender_enabled",
        code: 6505,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_AUDIO_FILES_SENDER_WAVEFORM_ENABLED: AbProp = AbProp {
        name: "channels_audio_files_sender_waveform_enabled",
        code: 6943,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_CAPABILITIES_ENABLED: AbProp = AbProp {
        name: "channels_capabilities_enabled",
        code: 10328,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNELS_CONTEXT_CARD_INVITE_FOLLOWERS_ENABLED: AbProp = AbProp {
        name: "channels_context_card_invite_followers_enabled",
        code: 27449,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_CREATION_ENABLED: AbProp = AbProp {
        name: "channels_creation_enabled",
        code: 3878,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_CREATION_ENTRYPOINT_IN_DIRECTORY_ENABLED: AbProp = AbProp {
        name: "channels_creation_entrypoint_in_directory_enabled",
        code: 18613,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_CREATION_ENTRYPOINT_IN_UPDATES_TAB_ENABLED: AbProp = AbProp {
        name: "channels_creation_entrypoint_in_updates_tab_enabled",
        code: 18925,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_DIRECTORY_CATEGORIES_CACHE_REFRESH_INTERVAL_MS: AbProp = AbProp {
        name: "channels_directory_categories_cache_refresh_interval_ms",
        code: 8151,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400000),
    };
    pub const CHANNELS_DIRECTORY_CATEGORIES_ENABLED: AbProp = AbProp {
        name: "channels_directory_categories_enabled",
        code: 7685,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_DIRECTORY_CATEGORIES_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_directory_categories_logging_enabled",
        code: 10188,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_DIRECTORY_CATEGORY_TYPES: AbProp = AbProp {
        name: "channels_directory_category_types",
        code: 7734,
        value_type: AbPropType::Str,
        default: AbDefault::Str("3,7,6,4,1,5,2"),
    };
    pub const CHANNELS_DIRECTORY_ENABLED: AbProp = AbProp {
        name: "channels_directory_enabled",
        code: 3879,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_DIRECTORY_PAGE_SIZE: AbProp = AbProp {
        name: "channels_directory_page_size",
        code: 5853,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const CHANNELS_DIRECTORY_SEARCH_DEBOUNCE_MS: AbProp = AbProp {
        name: "channels_directory_search_debounce_ms",
        code: 5204,
        value_type: AbPropType::Int,
        default: AbDefault::Int(250),
    };
    pub const CHANNELS_DIRECTORY_V2_CACHE_REFRESH_INTERVAL_MS: AbProp = AbProp {
        name: "channels_directory_v2_cache_refresh_interval_ms",
        code: 5304,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1800000),
    };
    pub const CHANNELS_DIRECTORY_V2_FILTER_TYPES: AbProp = AbProp {
        name: "channels_directory_v2_filter_types",
        code: 5127,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_EMOJI_FORWARDED_ATTRIBUTION_UI_ENABLED: AbProp = AbProp {
        name: "channels_emoji_forwarded_attribution_ui_enabled",
        code: 17081,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_ENABLED: AbProp = AbProp {
        name: "channels_enabled",
        code: 3877,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_FETCH_AND_LOG_CAPABILITIES: AbProp = AbProp {
        name: "channels_fetch_and_log_capabilities",
        code: 10325,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNELS_FILTER_OUT_SUBSCRIBED_IN_DIRECTORY_NULL_STATE: AbProp = AbProp {
        name: "channels_filter_out_subscribed_in_directory_null_state",
        code: 5015,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_FOLLOWER_INVITE_CREATION_MODAL_ENABLED: AbProp = AbProp {
        name: "channels_follower_invite_creation_modal_enabled",
        code: 26120,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_FOLLOWERS_LIST_CACHE_REFRESH_MILLISECONDS: AbProp = AbProp {
        name: "channels_followers_list_cache_refresh_milliseconds",
        code: 5217,
        value_type: AbPropType::Int,
        default: AbDefault::Int(60000),
    };
    pub const CHANNELS_FORWARD_COUNTER_ON_STATUS_CARD_ENABLED: AbProp = AbProp {
        name: "channels_forward_counter_on_status_card_enabled",
        code: 26148,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_FORWARD_LOGGING_V2_ENABLED: AbProp = AbProp {
        name: "channels_forward_logging_v2_enabled",
        code: 5492,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_HIDE_NEWS_URL_PREVIEW: AbProp = AbProp {
        name: "channels_hide_news_url_preview",
        code: 5287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_IN_APP_POLICY_DETAIL_ENABLED: AbProp = AbProp {
        name: "channels_in_app_policy_detail_enabled",
        code: 29132,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_INVITE_CONTACTS_TO_FOLLOW_CONSUMER_ENABLED: AbProp = AbProp {
        name: "channels_invite_contacts_to_follow_consumer_enabled",
        code: 16790,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_INVITE_CONTACTS_TO_FOLLOW_PRODUCER_ENABLED: AbProp = AbProp {
        name: "channels_invite_contacts_to_follow_producer_enabled",
        code: 16789,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_INVITE_CONTACTS_TO_FOLLOW_RECEIVER_INVALID_MESSAGE_DROP_ENDABLED: AbProp =
        AbProp {
            name: "channels_invite_contacts_to_follow_receiver_invalid_message_drop_endabled",
            code: 22280,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(true),
        };
    pub const CHANNELS_INVITE_CONTACTS_TO_FOLLOW_RECEIVER_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_invite_contacts_to_follow_receiver_logging_enabled",
        code: 20836,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_INVITE_CONTACTS_TO_FOLLOW_SENDER_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_invite_contacts_to_follow_sender_logging_enabled",
        code: 20837,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_INVITE_LINK_PREVIEW_IMPROVEMENT_ENABLED: AbProp = AbProp {
        name: "channels_invite_link_preview_improvement_enabled",
        code: 22196,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_IS_MULTI_ADMIN_LID_MIGRATION_ENABLED: AbProp = AbProp {
        name: "channels_is_multi_admin_lid_migration_enabled",
        code: 16193,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_MAX_MESSAGES_BATCH_PULL: AbProp = AbProp {
        name: "channels_max_messages_batch_pull",
        code: 5494,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const CHANNELS_MESSAGE_PIN_ADMIN_ENABLED: AbProp = AbProp {
        name: "channels_message_pin_admin_enabled",
        code: 29516,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_MESSAGE_PIN_FOLLOWER_ENABLED: AbProp = AbProp {
        name: "channels_message_pin_follower_enabled",
        code: 29517,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_MULTI_ADMIN_MAX_ADMIN_COUNT: AbProp = AbProp {
        name: "channels_multi_admin_max_admin_count",
        code: 6461,
        value_type: AbPropType::Int,
        default: AbDefault::Int(16),
    };
    pub const CHANNELS_MUSIC_FORWARDING_DISABLED: AbProp = AbProp {
        name: "channels_music_forwarding_disabled",
        code: 22089,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_MUSIC_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_music_receiver_enabled",
        code: 20266,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_OPEN_QPL_IMPROVEMENTS_ENABLED: AbProp = AbProp {
        name: "channels_open_qpl_improvements_enabled",
        code: 15754,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_OPEN_QPL_USER_RID_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_open_qpl_user_rid_logging_enabled",
        code: 17712,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PHOTO_POLLS_GENAI_ENABLED: AbProp = AbProp {
        name: "channels_photo_polls_genai_enabled",
        code: 26392,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PINNING_NUDGE_ENABLED: AbProp = AbProp {
        name: "channels_pinning_nudge_enabled",
        code: 20551,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_POLL_RECEIVE_ENABLED: AbProp = AbProp {
        name: "channels_poll_receive_enabled",
        code: 6191,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_POLL_VOTER_LIST_ENABLED: AbProp = AbProp {
        name: "channels_poll_voter_list_enabled",
        code: 6382,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_POLL_VOTERS_DETAILS_CACHE_TTL_MS: AbProp = AbProp {
        name: "channels_poll_voters_details_cache_ttl_ms",
        code: 7920,
        value_type: AbPropType::Int,
        default: AbDefault::Int(300000),
    };
    pub const CHANNELS_POLL_VOTERS_SUMMARY_CACHE_TTL_MS: AbProp = AbProp {
        name: "channels_poll_voters_summary_cache_ttl_ms",
        code: 7919,
        value_type: AbPropType::Int,
        default: AbDefault::Int(120000),
    };
    pub const CHANNELS_PROACTIVE_MESSAGE_GAP_HANDLING_ENABLED: AbProp = AbProp {
        name: "channels_proactive_message_gap_handling_enabled",
        code: 5871,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PRODUCER_INSIGHTS_ENABLED: AbProp = AbProp {
        name: "channels_producer_insights_enabled",
        code: 8960,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PRODUCER_INSIGHTS_HIDE_DELTAS: AbProp = AbProp {
        name: "channels_producer_insights_hide_deltas",
        code: 9792,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNELS_PRODUCER_INSIGHTS_MIN_FOLLOWERS: AbProp = AbProp {
        name: "channels_producer_insights_min_followers",
        code: 9447,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const CHANNELS_PTT_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_ptt_logging_enabled",
        code: 6274,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CHANNELS_PTT_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_ptt_receiver_enabled",
        code: 5876,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PTV_FORWARDING_ENABLED: AbProp = AbProp {
        name: "channels_ptv_forwarding_enabled",
        code: 13776,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PTV_RECEIVING_ENABLED: AbProp = AbProp {
        name: "channels_ptv_receiving_enabled",
        code: 13559,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_PULSE_ON_UNREAD_BADGE_ENABLED: AbProp = AbProp {
        name: "channels_pulse_on_unread_badge_enabled",
        code: 28224,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QPL_IMPROVEMENTS_SUPPORTED_TYPES: AbProp = AbProp {
        name: "channels_qpl_improvements_supported_types",
        code: 19589,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QPL_LOGGING: AbProp = AbProp {
        name: "channels_qpl_logging",
        code: 7677,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTION_ADMIN_ENABLED: AbProp = AbProp {
        name: "channels_question_admin_enabled",
        code: 17426,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTION_ADMIN_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_admin_m2_enabled",
        code: 26910,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTION_FETCH_RESPONSES_PAGE_SIZE: AbProp = AbProp {
        name: "channels_question_fetch_responses_page_size",
        code: 18984,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const CHANNELS_QUESTION_FOLLOWER_ENABLED: AbProp = AbProp {
        name: "channels_question_follower_enabled",
        code: 17425,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTION_FOLLOWER_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_follower_m2_enabled",
        code: 26911,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_CHAT_M1_ENABLED: AbProp = AbProp {
        name: "channels_question_forward_message_types_chat_m1_enabled",
        code: 18988,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_CHAT_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_forward_message_types_chat_m2_enabled",
        code: 26925,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_STATUS_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_forward_message_types_status_m2_enabled",
        code: 26926,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_RECEIVER_MESSAGE_TYPES_M1_ENABLED: AbProp = AbProp {
        name: "channels_question_receiver_message_types_m1_enabled",
        code: 15246,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_RECEIVER_MESSAGE_TYPES_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_receiver_message_types_m2_enabled",
        code: 26932,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_REPLY_RECEIVER_MESSAGE_TYPES_M1_ENABLED: AbProp = AbProp {
        name: "channels_question_reply_receiver_message_types_m1_enabled",
        code: 18393,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_REPLY_RECEIVER_MESSAGE_TYPES_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_reply_receiver_message_types_m2_enabled",
        code: 26933,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_REPLY_SENDER_MESSAGE_TYPES_M1_ENABLED: AbProp = AbProp {
        name: "channels_question_reply_sender_message_types_m1_enabled",
        code: 18394,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_REPLY_SENDER_MESSAGE_TYPES_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_reply_sender_message_types_m2_enabled",
        code: 26931,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTION_RESPONSE_RATE_LIMIT_MAX_COUNT_IN_CLIENT_UI: AbProp = AbProp {
        name: "channels_question_response_rate_limit_max_count_in_client_ui",
        code: 19989,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const CHANNELS_QUESTION_SENDER_MESSAGE_TYPES_M1_ENABLED: AbProp = AbProp {
        name: "channels_question_sender_message_types_m1_enabled",
        code: 15418,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const CHANNELS_QUESTION_SENDER_MESSAGE_TYPES_M2_ENABLED: AbProp = AbProp {
        name: "channels_question_sender_message_types_m2_enabled",
        code: 26930,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_QUESTIONS_INTEGRITY_M1_ENABLED: AbProp = AbProp {
        name: "channels_questions_integrity_m1_enabled",
        code: 17600,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTIONS_RESPONSES_DRAWER_LOADING_SHIMMER_ENABLED: AbProp = AbProp {
        name: "channels_questions_responses_drawer_loading_shimmer_enabled",
        code: 29209,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTIONS_SEARCH_BACKTEST_ENABLED: AbProp = AbProp {
        name: "channels_questions_search_backtest_enabled",
        code: 31046,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUESTIONS_SEARCH_ENABLED: AbProp = AbProp {
        name: "channels_questions_search_enabled",
        code: 24004,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUICK_FORWARDING_BUTTON_MODE: AbProp = AbProp {
        name: "channels_quick_forwarding_button_mode",
        code: 7234,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_QUIZ_RECEIVING_ENABLED: AbProp = AbProp {
        name: "channels_quiz_receiving_enabled",
        code: 19778,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_QUIZ_SENDING_ENABLED: AbProp = AbProp {
        name: "channels_quiz_sending_enabled",
        code: 19777,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_REACTIONS_BOTTOMSHEET_TAP_TO_REACT_ENABLED: AbProp = AbProp {
        name: "channels_reactions_bottomsheet_tap_to_react_enabled",
        code: 7682,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_RECOMMENDED_V3_UI_LIMIT: AbProp = AbProp {
        name: "channels_recommended_v3_ui_limit",
        code: 8167,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_CHAT_M1_ENABLED: AbProp = AbProp {
        name: "channels_reply_forward_message_types_chat_m1_enabled",
        code: 19053,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_CHAT_M2_ENABLED: AbProp = AbProp {
        name: "channels_reply_forward_message_types_chat_m2_enabled",
        code: 26927,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_STATUS_M2_ENABLED: AbProp = AbProp {
        name: "channels_reply_forward_message_types_status_m2_enabled",
        code: 26924,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const CHANNELS_SEND_ALBUM_ENABLED: AbProp = AbProp {
        name: "channels_send_album_enabled",
        code: 5643,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_SEND_VIEW_RECEIPT_ENABLED: AbProp = AbProp {
        name: "channels_send_view_receipt_enabled",
        code: 4760,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_SGI_RECEIVER_ENABLED: AbProp = AbProp {
        name: "channels_sgi_receiver_enabled",
        code: 32801,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_SGI_SENDER_ENABLED: AbProp = AbProp {
        name: "channels_sgi_sender_enabled",
        code: 32802,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_SHARE_LINK_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_share_link_logging_enabled",
        code: 5491,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_STATUS_CONSUMPTION_ENTRYPOINTS: AbProp = AbProp {
        name: "channels_status_consumption_entrypoints",
        code: 27240,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_STATUS_UPDATES_CONSUMPTION_ENABLED: AbProp = AbProp {
        name: "channels_status_updates_consumption_enabled",
        code: 6444,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_STICKER_FORWARDED_ATTRIBUTION_UI_ENABLED: AbProp = AbProp {
        name: "channels_sticker_forwarded_attribution_ui_enabled",
        code: 16856,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_STICKER_PACK_FORWARDED_ATTRIBUTION_UI_ENABLED: AbProp = AbProp {
        name: "channels_sticker_pack_forwarded_attribution_ui_enabled",
        code: 16858,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_STICKER_PACK_RENDERING: AbProp = AbProp {
        name: "channels_sticker_pack_rendering",
        code: 20182,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_T_ENABLED: AbProp = AbProp {
        name: "channels_t_enabled",
        code: 25078,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_UK_OSA_ENABLED: AbProp = AbProp {
        name: "channels_uk_osa_enabled",
        code: 14249,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_UPDATES_TAB_SWIPE_ACTIONS_ENABLED: AbProp = AbProp {
        name: "channels_updates_tab_swipe_actions_enabled",
        code: 8653,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_VERIFIED_BADGE_IN_COMPACT_INBOX_ENABLED: AbProp = AbProp {
        name: "channels_verified_badge_in_compact_inbox_enabled",
        code: 8059,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_VIDEO_PLAY_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_video_play_logging_enabled",
        code: 16491,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_VIEW_COUNTS_SENDER_ADMIN_EXCLUSION_MODE: AbProp = AbProp {
        name: "channels_view_counts_sender_admin_exclusion_mode",
        code: 31729,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CHANNELS_VIEW_COUNTS_VPV_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_view_counts_vpv_logging_enabled",
        code: 12295,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_VISIBILITY_LOGGING_FULLSCREEN_MEDIA_ENABLED: AbProp = AbProp {
        name: "channels_visibility_logging_fullscreen_media_enabled",
        code: 28148,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHANNELS_VPV_LOGGING_ENABLED: AbProp = AbProp {
        name: "channels_vpv_logging_enabled",
        code: 9834,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHATLIST_FILTERS_V1: AbProp = AbProp {
        name: "chatlist_filters_v1",
        code: 1608,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHATLIST_PREVENT_AUTOREAD: AbProp = AbProp {
        name: "chatlist_prevent_autoread",
        code: 21156,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CHATLIST_SHOW_DRAFT_FOR_EMPTY_CHAT: AbProp = AbProp {
        name: "chatlist_show_draft_for_empty_chat",
        code: 19287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_CALLING_ENABLED: AbProp = AbProp {
        name: "coex_calling_enabled",
        code: 18047,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_CALLING_ENABLED_BUSINESS: AbProp = AbProp {
        name: "coex_calling_enabled_business",
        code: 23933,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_CALLING_PERMISSIONS_3P_ENABLED: AbProp = AbProp {
        name: "coex_calling_permissions_3p_enabled",
        code: 23464,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_EDIT_MSG_ENABLED: AbProp = AbProp {
        name: "coex_edit_msg_enabled",
        code: 19039,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_IICON_BACKFILL: AbProp = AbProp {
        name: "coex_iicon_backfill",
        code: 28349,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEX_REVOKE_MESSAGE_ENABLED: AbProp = AbProp {
        name: "coex_revoke_message_enabled",
        code: 19285,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEXV2_RECV_ENABLED: AbProp = AbProp {
        name: "coexv2_recv_enabled",
        code: 28110,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COEXV2_SEND_ENABLED: AbProp = AbProp {
        name: "coexv2_send_enabled",
        code: 27839,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COMMERCE_SANCTIONED: AbProp = AbProp {
        name: "commerce_sanctioned",
        code: 1319,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COMMUNITY_ADMIN_PROMOTION_ONE_TIME_PROMPT: AbProp = AbProp {
        name: "community_admin_promotion_one_time_prompt",
        code: 1864,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COMMUNITY_ANNOUNCEMENT_GROUP_SIZE_LIMIT: AbProp = AbProp {
        name: "community_announcement_group_size_limit",
        code: 2774,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5000),
    };
    pub const COMMUNITY_GENERAL_CHAT_UI_ENABLED: AbProp = AbProp {
        name: "community_general_chat_UI_enabled",
        code: 5021,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COMMUNITY_GENERAL_CHAT_CREATE_ENABLED: AbProp = AbProp {
        name: "community_general_chat_create_enabled",
        code: 5453,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CONSUMER_GRAPHQL_ENABLE_DOUBLE_LOG_FOR_SURVEY: AbProp = AbProp {
        name: "consumer_graphql_enable_double_log_for_survey",
        code: 28129,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CONSUMER_GRAPHQL_WEB_TO_FETCH_QP_SURFACE_IDS: AbProp = AbProp {
        name: "consumer_graphql_web_to_fetch_qp_surface_ids",
        code: 28159,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const CONSUMER_WEB_QP_GRAPHQL_TO_FETCH_QP_FREQUENCY_MINS: AbProp = AbProp {
        name: "consumer_web_qp_graphql_to_fetch_qp_frequency_mins",
        code: 28529,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1320),
    };
    pub const COUNTRY_CLIENT_GATING_ENABLED: AbProp = AbProp {
        name: "country_client_gating_enabled",
        code: 1105,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const COUPON_COPY_BUTTON_URL: AbProp = AbProp {
        name: "coupon_copy_button_url",
        code: 3631,
        value_type: AbPropType::Str,
        default: AbDefault::Str("https://www.whatsapp.com/coupon?code="),
    };
    pub const CREATE_GROUP_AND_ADD_MEMBER_OVERFLOW: AbProp = AbProp {
        name: "create_group_and_add_member_overflow",
        code: 15772,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CROSS_DEVICE_MESSAGE_EDITING: AbProp = AbProp {
        name: "cross_device_message_editing",
        code: 28340,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_1PD_LONGEST_CALL_ENABLED: AbProp = AbProp {
        name: "ctwa_1pd_longest_call_enabled",
        code: 32108,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_AGGREGATED_CALL_LOGGING_ALLOWED: AbProp = AbProp {
        name: "ctwa_3pd_aggregated_call_logging_allowed",
        code: 32379,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_AGGREGATED_CONVERSION_ENABLED: AbProp = AbProp {
        name: "ctwa_3pd_aggregated_conversion_enabled",
        code: 27640,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_DATA_SHARING_ADDITIONAL_LOGGING: AbProp = AbProp {
        name: "ctwa_3pd_data_sharing_additional_logging",
        code: 29333,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_DATA_SHARING_COOLDOWN_MAX_TIMES_SHOWN_FOR_OPTED_OUT: AbProp = AbProp {
        name: "ctwa_3pd_data_sharing_cooldown_max_times_shown_for_opted_out",
        code: 15686,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CTWA_3PD_DATA_SHARING_ON_THREAD_ENTRY: AbProp = AbProp {
        name: "ctwa_3pd_data_sharing_on_thread_entry",
        code: 13485,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_DATA_SHARING_TITLE_CHANGE: AbProp = AbProp {
        name: "ctwa_3pd_data_sharing_title_change",
        code: 29332,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_3PD_OPT_OUT_COUNTER_OPTIMIZATION_ENABLED: AbProp = AbProp {
        name: "ctwa_3pd_opt_out_counter_optimization_enabled",
        code: 24984,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_AD_ACCOUNT_NONCE_PUSH_WAIT_TIMEOUT_WEB: AbProp = AbProp {
        name: "ctwa_ad_account_nonce_push_wait_timeout_web",
        code: 8664,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20),
    };
    pub const CTWA_AD_ACCOUNT_NONCE_RETRIES_MAX_WEB: AbProp = AbProp {
        name: "ctwa_ad_account_nonce_retries_max_web",
        code: 8663,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CTWA_AD_ACCOUNT_TOKEN_STORAGE_KILL_SWITCH_WEB: AbProp = AbProp {
        name: "ctwa_ad_account_token_storage_kill_switch_web",
        code: 8166,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const CTWA_AD_CREATION_ENTRY_POINT_CATALOG_PRODUCT_WEB: AbProp = AbProp {
        name: "ctwa_ad_creation_entry_point_catalog_product_web",
        code: 9677,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_AD_CREATION_ENTRY_POINT_CATALOG_WEB: AbProp = AbProp {
        name: "ctwa_ad_creation_entry_point_catalog_web",
        code: 9596,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_AE_MODEL_META_DATA_ENABLED: AbProp = AbProp {
        name: "ctwa_ae_model_meta_data_enabled",
        code: 27515,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_AE_MODEL_META_DATA_SIGNAL_ENABLED: AbProp = AbProp {
        name: "ctwa_ae_model_meta_data_signal_enabled",
        code: 27516,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_BLOCK_IB_AR_FOR_WABAI: AbProp = AbProp {
        name: "ctwa_block_ib_ar_for_wabai",
        code: 26302,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_CONVERSION_CREATION_FROM_DELAY_ENABLED: AbProp = AbProp {
        name: "ctwa_conversion_creation_from_delay_enabled",
        code: 32777,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_CUSTOM_LABEL_ALGORITHM: AbProp = AbProp {
        name: "ctwa_custom_label_algorithm",
        code: 14887,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const CTWA_CUSTOM_LABEL_SIGNALS_ENABLED: AbProp = AbProp {
        name: "ctwa_custom_label_signals_enabled",
        code: 11205,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_DATA_MAX_LENGTH: AbProp = AbProp {
        name: "ctwa_data_max_length",
        code: 1841,
        value_type: AbPropType::Int,
        default: AbDefault::Int(768),
    };
    pub const CTWA_DOWNLOAD_3PD_SIGNALS: AbProp = AbProp {
        name: "ctwa_download_3pd_signals",
        code: 13385,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_ENABLE_BIZ_DATA_SHARING_AFTER_NUX_DISMISS: AbProp = AbProp {
        name: "ctwa_enable_biz_data_sharing_after_nux_dismiss",
        code: 13240,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_ENTRY_POINT_CONFIG_FETCH_THRESHHOLD: AbProp = AbProp {
        name: "ctwa_entry_point_config_fetch_threshhold",
        code: 6214,
        value_type: AbPropType::Int,
        default: AbDefault::Int(43200000),
    };
    pub const CTWA_FAVORITES_LIST_SENDS_SIGNALS: AbProp = AbProp {
        name: "ctwa_favorites_list_sends_signals",
        code: 29529,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_IMPORTANT_LABEL_SENDS_SIGNALS: AbProp = AbProp {
        name: "ctwa_important_label_sends_signals",
        code: 15271,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_LEAD_TAXONOMY: AbProp = AbProp {
        name: "ctwa_lead_taxonomy",
        code: 26531,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_LONG_TERM_HOLDOUT_CLIENT_SIDE_CHECK: AbProp = AbProp {
        name: "ctwa_long_term_holdout_client_side_check",
        code: 11000,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_LONG_TERM_HOLDOUT_CONTENT_ENABLED: AbProp = AbProp {
        name: "ctwa_long_term_holdout_content_enabled",
        code: 8015,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_LONGEST_CALL_DURATION: AbProp = AbProp {
        name: "ctwa_longest_call_duration",
        code: 32100,
        value_type: AbPropType::Int,
        default: AbDefault::Int(120),
    };
    pub const CTWA_MM_BIZ_AI_DISCLOSURE_UPDATE_ENABLED: AbProp = AbProp {
        name: "ctwa_mm_biz_ai_disclosure_update_enabled",
        code: 10379,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_NATIVE_ADS_CREATION_WEB_ENABLED: AbProp = AbProp {
        name: "ctwa_native_ads_creation_web_enabled",
        code: 18857,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_NATIVE_ADS_CREATION_WEB_HAWK_TOOL_ENABLED: AbProp = AbProp {
        name: "ctwa_native_ads_creation_web_hawk_tool_enabled",
        code: 20442,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_NATIVE_ADS_CREATION_WEB_TARGETING_MODAL_HAWK_TOOL_ENABLED: AbProp = AbProp {
        name: "ctwa_native_ads_creation_web_targeting_modal_hawk_tool_enabled",
        code: 20731,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_NATIVE_ADS_DETAILED_TARGETING: AbProp = AbProp {
        name: "ctwa_native_ads_detailed_targeting",
        code: 32487,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_NATIVE_ADS_INLINE_NOTICE_MODULES: AbProp = AbProp {
        name: "ctwa_native_ads_inline_notice_modules",
        code: 32701,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "AdsLWICTWAZeroOutcomeAdValidationModule,AdsLWICTWASimilarAdvertiserBudgetRecommendationValidationModule",
        ),
    };
    pub const CTWA_NATIVE_WEB_DRAFT_AD_ENABLED: AbProp = AbProp {
        name: "ctwa_native_web_draft_ad_enabled",
        code: 28989,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_PER_CUSTOMER_DATA_SHARING_CONTROLS_DO_NOT_SHOW_MSG_UNTIL_CHOSEN: AbProp =
        AbProp {
            name: "ctwa_per_customer_data_sharing_controls_do_not_show_msg_until_chosen",
            code: 19763,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const CTWA_SHOW_ADS_DATA_SHARING_AFTER_MESSAGE: AbProp = AbProp {
        name: "ctwa_show_ads_data_sharing_after_message",
        code: 13579,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_DATA_SHARING_CONSENT: AbProp = AbProp {
        name: "ctwa_smb_data_sharing_consent",
        code: 2934,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_DATA_SHARING_OPT_IN_COOL_OFF_PERIOD: AbProp = AbProp {
        name: "ctwa_smb_data_sharing_opt_in_cool_off_period",
        code: 3331,
        value_type: AbPropType::Int,
        default: AbDefault::Int(259200),
    };
    pub const CTWA_SMB_DATA_SHARING_SETTINGS_KILLSWITCH: AbProp = AbProp {
        name: "ctwa_smb_data_sharing_settings_killswitch",
        code: 5615,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_DETECTED_OUTCOME_LABELS_ENABLED: AbProp = AbProp {
        name: "ctwa_smb_detected_outcome_labels_enabled",
        code: 15307,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_DETECTED_OUTCOME_LABELS_MERGER_ENABLED: AbProp = AbProp {
        name: "ctwa_smb_detected_outcome_labels_merger_enabled",
        code: 15308,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_DETECTED_OUTCOME_LISTS_ENABLED: AbProp = AbProp {
        name: "ctwa_smb_detected_outcome_lists_enabled",
        code: 20220,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_LABEL_CHAT_HEADER_ENABLED_WEB: AbProp = AbProp {
        name: "ctwa_smb_label_chat_header_enabled_web",
        code: 25180,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_LISTS_DROPDOWN_APPLICATION_FIX_ENABLED: AbProp = AbProp {
        name: "ctwa_smb_lists_dropdown_application_fix_enabled",
        code: 30401,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SMB_MULTISELECT_ENABLED: AbProp = AbProp {
        name: "ctwa_smb_multiselect_enabled",
        code: 26719,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SUPPRESS_MESSAGE_VIA_AD_SPAM_WEB: AbProp = AbProp {
        name: "ctwa_suppress_message_via_ad_spam_web",
        code: 17580,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_SUPPRESS_MESSAGE_WITH_EXTERNAL_AD_REPLY_CONSUMER_DB_LEVEL_ENABLED: AbProp =
        AbProp {
            name: "ctwa_suppress_message_with_external_ad_reply_consumer_db_level_enabled",
            code: 21819,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const CTWA_TOS_FILTERING_ENABLED: AbProp = AbProp {
        name: "ctwa_tos_filtering_enabled",
        code: 976,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_CUSTOM_LABEL_SIGNALS_ENABLED: AbProp = AbProp {
        name: "ctwa_web_custom_label_signals_enabled",
        code: 19985,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_NATIVE_ADS_BUDGET_RECOMMENDATION_ENABLED: AbProp = AbProp {
        name: "ctwa_web_native_ads_budget_recommendation_enabled",
        code: 32511,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_NATIVE_ADS_MVP_QE1_ENABLED: AbProp = AbProp {
        name: "ctwa_web_native_ads_mvp_qe1_enabled",
        code: 24668,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_NATIVE_ADS_MVP_QE1_ENABLED_NO_EXPOSURE: AbProp = AbProp {
        name: "ctwa_web_native_ads_mvp_qe1_enabled_no_exposure",
        code: 24761,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_NATIVE_ADS_MVP_QE2_ENABLED: AbProp = AbProp {
        name: "ctwa_web_native_ads_mvp_qe2_enabled",
        code: 24669,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CTWA_WEB_NATIVE_ADS_SABR_ENABLED: AbProp = AbProp {
        name: "ctwa_web_native_ads_sabr_enabled",
        code: 32007,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CUSTOM_NOTIFICATION_TONES: AbProp = AbProp {
        name: "custom_notification_tones",
        code: 18884,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CUSTOM_RACING_EMOJI: AbProp = AbProp {
        name: "custom_racing_emoji",
        code: 7463,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const CUSTOM_RACING_EMOJI_FEB2025: AbProp = AbProp {
        name: "custom_racing_emoji_feb2025",
        code: 13322,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DATA_PRIVACY_PHASE_2_ENABLED: AbProp = AbProp {
        name: "data_privacy_phase_2_enabled",
        code: 6843,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DATA_PRIVACY_PHASE_2_NON_E2EE_ENABLED: AbProp = AbProp {
        name: "data_privacy_phase_2_non_e2ee_enabled",
        code: 7131,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DATA_SHARING_TRANSPARENCY_INDICATOR_DURATION: AbProp = AbProp {
        name: "data_sharing_transparency_indicator_duration",
        code: 5990,
        value_type: AbPropType::Int,
        default: AbDefault::Int(604800),
    };
    pub const DAU_FIX_DELAY_PRESENCE_ON_FOCUS: AbProp = AbProp {
        name: "dau_fix_delay_presence_on_focus",
        code: 18189,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DEFAULT_AUDIO_LIMIT_MB: AbProp = AbProp {
        name: "default_audio_limit_mb",
        code: 3657,
        value_type: AbPropType::Int,
        default: AbDefault::Int(16),
    };
    pub const DEFAULT_ENDPOINT_THREAD_POLL_TIMEOUT: AbProp = AbProp {
        name: "default_endpoint_thread_poll_timeout",
        code: 11129,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const DEFAULT_MEDIA_LIMIT_MB: AbProp = AbProp {
        name: "default_media_limit_mb",
        code: 3660,
        value_type: AbPropType::Int,
        default: AbDefault::Int(16),
    };
    pub const DEFAULT_STATUS_MEDIA_LIMIT_MB: AbProp = AbProp {
        name: "default_status_media_limit_mb",
        code: 3659,
        value_type: AbPropType::Int,
        default: AbDefault::Int(16),
    };
    pub const DEFAULT_VIDEO_LIMIT_MB: AbProp = AbProp {
        name: "default_video_limit_mb",
        code: 3185,
        value_type: AbPropType::Int,
        default: AbDefault::Int(16),
    };
    pub const DEFENSE_MODE_AVAILABLE: AbProp = AbProp {
        name: "defense_mode_available",
        code: 13874,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const DEFENSE_MODE_QUARANTINE: AbProp = AbProp {
        name: "defense_mode_quarantine",
        code: 24959,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DEFENSE_MODE_QUARANTINE_BULK_UNBLOCK_LIMIT: AbProp = AbProp {
        name: "defense_mode_quarantine_bulk_unblock_limit",
        code: 21921,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const DEFENSE_MODE_QUARANTINE_MESSAGE_EXPIRATION_WINDOW: AbProp = AbProp {
        name: "defense_mode_quarantine_message_expiration_window",
        code: 21918,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1210000),
    };
    pub const DESKTOP_UPSELL_INTRO_PANEL_ILLUSTRATION_VARIANT: AbProp = AbProp {
        name: "desktop_upsell_intro_panel_illustration_variant",
        code: 19518,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const DEV_PROP_BOOLEAN: AbProp = AbProp {
        name: "dev_prop_boolean",
        code: 1065,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DEV_PROP_FLOAT: AbProp = AbProp {
        name: "dev_prop_float",
        code: 1067,
        value_type: AbPropType::Float,
        default: AbDefault::Float(0.0),
    };
    pub const DEV_PROP_INT: AbProp = AbProp {
        name: "dev_prop_int",
        code: 1066,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const DEV_PROP_STRING: AbProp = AbProp {
        name: "dev_prop_string",
        code: 1064,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const DEVICE_SWITCHING_ENABLED: AbProp = AbProp {
        name: "device_switching_enabled",
        code: 3205,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DIALER_PAD_FOR_NEW_CHATS: AbProp = AbProp {
        name: "dialer_pad_for_new_chats",
        code: 18688,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DIRECT_CONNECTION_BUSINESS_NUMBERS: AbProp = AbProp {
        name: "direct_connection_business_numbers",
        code: 1846,
        value_type: AbPropType::Str,
        default: AbDefault::Str("16005554444,918591749310,917977079770"),
    };
    pub const DIRECTORY_CATEGORIES_DISPLAY_NEWSLETTERS_PER_CATEGORY_LIMIT: AbProp = AbProp {
        name: "directory_categories_display_newsletters_per_category_limit",
        code: 9312,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const DIRECTORY_CATEGORIES_NEWSLETTERS_PER_CATEGORY_LIMIT: AbProp = AbProp {
        name: "directory_categories_newsletters_per_category_limit",
        code: 7986,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const DISABLE_AUTO_DOWNLOAD: AbProp = AbProp {
        name: "disable_auto_download",
        code: 1838,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DISABLE_LIBAOM_REGISTRATION: AbProp = AbProp {
        name: "disable_libaom_registration",
        code: 23836,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DISABLE_RAISE_HAND_1ON1: AbProp = AbProp {
        name: "disable_raise_hand_1on1",
        code: 27177,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DISAPPEARING_MODE: AbProp = AbProp {
        name: "disappearing_mode",
        code: 536,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DISCLOSURE_FOR_THE_MARKETING_MESSAGE_BODY_LINKS_ENABLED: AbProp = AbProp {
        name: "disclosure_for_the_marketing_message_body_links_enabled",
        code: 12994,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DM_ADDITIONAL_DURATIONS: AbProp = AbProp {
        name: "dm_additional_durations",
        code: 3305,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DM_AFTER_READ_TIMER_SENDER_OPTIONS_SECONDS: AbProp = AbProp {
        name: "dm_after_read_timer_sender_options_seconds",
        code: 30176,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"timers\": [0, 300, 3600, 43200]}"),
    };
    pub const DM_INITIATOR_TRIGGER_DAILY_LOGS: AbProp = AbProp {
        name: "dm_initiator_trigger_daily_logs",
        code: 7402,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DM_INITIATOR_TRIGGER_GROUPS: AbProp = AbProp {
        name: "dm_initiator_trigger_groups",
        code: 7141,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DM_RECEIVER_AFTER_READ_ALLOW_VALUES: AbProp = AbProp {
        name: "dm_receiver_after_read_allow_values",
        code: 26218,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"timers\": [0, 900]}"),
    };
    pub const DM_RECEIVER_ALLOWED_VALUES: AbProp = AbProp {
        name: "dm_receiver_allowed_values",
        code: 19232,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"timers\": [0, 86400, 604800, 7776000]}"),
    };
    pub const DM_RELIABILITY_LOGGING: AbProp = AbProp {
        name: "dm_reliability_logging",
        code: 5580,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DM_UPDATED_SYSTEM_MESSAGE: AbProp = AbProp {
        name: "dm_updated_system_message",
        code: 1670,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DOWNLOAD_DOCUMENT_THUMB_MMS_ENABLED: AbProp = AbProp {
        name: "download_document_thumb_mms_enabled",
        code: 250,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DOWNLOAD_STATUS_THUMB_MMS_ENABLED: AbProp = AbProp {
        name: "download_status_thumb_mms_enabled",
        code: 249,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DROP_LAST_NAME: AbProp = AbProp {
        name: "drop_last_name",
        code: 726,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DSA_21_CHANNEL_REPORTING_ENABLED: AbProp = AbProp {
        name: "dsa_21_channel_reporting_enabled",
        code: 21073,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DSA_26_RECEIVER_ENABLED: AbProp = AbProp {
        name: "dsa_26_receiver_enabled",
        code: 22515,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DSA_26_SENDER_ENABLED: AbProp = AbProp {
        name: "dsa_26_sender_enabled",
        code: 22516,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DSA_CHANNELS_REPORT_UNLAWFUL_CONTENT_ENABLED: AbProp = AbProp {
        name: "dsa_channels_report_unlawful_content_enabled",
        code: 6145,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const DSA_INFORMATION_FOR_EU_ONLY_ENABLED: AbProp = AbProp {
        name: "dsa_information_for_eu_only_enabled",
        code: 7592,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EARLY_AUDIO_DRIVER_CAPTURE_AT_NATIVE: AbProp = AbProp {
        name: "early_audio_driver_capture_at_native",
        code: 13166,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EARLY_AUDIO_DRIVER_PRE_BUFFERING: AbProp = AbProp {
        name: "early_audio_driver_pre_buffering",
        code: 13168,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EARLY_BOT_CONNECT_EVENT_BITMAP: AbProp = AbProp {
        name: "early_bot_connect_event_bitmap",
        code: 14200,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const EDUCATIONAL_DIALOGS_BUTTON_ENABLED: AbProp = AbProp {
        name: "educational_dialogs_button_enabled",
        code: 14676,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ELEVATED_PUSH_NAMES_V2_M2_ENABLED: AbProp = AbProp {
        name: "elevated_push_names_v2_m2_enabled",
        code: 2904,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EMOJI_SEARCH_CLDR: AbProp = AbProp {
        name: "emoji_search_cldr",
        code: 13323,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EMPTY_UNREAD_FILTER_CTA_VARIANT: AbProp = AbProp {
        name: "empty_unread_filter_cta_variant",
        code: 22962,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const ENABLE_3P_CONTACTS_SHARE_HYBRID: AbProp = AbProp {
        name: "enable_3p_contacts_share_hybrid",
        code: 20849,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_AGM_FLOW_CTA: AbProp = AbProp {
        name: "enable_agm_flow_cta",
        code: 22006,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_AUDIO_DEVICE_ASYNC_START: AbProp = AbProp {
        name: "enable_audio_device_async_start",
        code: 13231,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_AUTO_ADD_CALL_LINK_CREATOR: AbProp = AbProp {
        name: "enable_auto_add_call_link_creator",
        code: 15184,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_AV_DOWNGRADE_1ON1: AbProp = AbProp {
        name: "enable_av_downgrade_1on1",
        code: 18165,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_AVATARS_ON_WEB_COMPANION: AbProp = AbProp {
        name: "enable_avatars_on_web_companion",
        code: 18081,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_BUSY_REASON_FS: AbProp = AbProp {
        name: "enable_busy_reason_fs",
        code: 9674,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CACHED_MEDIA_MANAGER: AbProp = AbProp {
        name: "enable_cached_media_manager",
        code: 4812,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_CALL_CONTROL_M5: AbProp = AbProp {
        name: "enable_call_control_m5",
        code: 8524,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALL_LINK_CALL_LOG_AGGREGATION: AbProp = AbProp {
        name: "enable_call_link_call_log_aggregation",
        code: 16523,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALL_LINKS_PUSH_NOTIFICATION: AbProp = AbProp {
        name: "enable_call_links_push_notification",
        code: 13679,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALL_RESULT_FIX_FOR_404_ACCEPT_NACK: AbProp = AbProp {
        name: "enable_call_result_fix_for_404_accept_nack",
        code: 10565,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALL_TRANSFER_NOTIFICATION: AbProp = AbProp {
        name: "enable_call_transfer_notification",
        code: 29242,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALLING_PHONE_NUMBER_PRIVACY: AbProp = AbProp {
        name: "enable_calling_phone_number_privacy",
        code: 17731,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CALLING_USERNAME: AbProp = AbProp {
        name: "enable_calling_username",
        code: 13359,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CHANNEL_VIDEO_SERVER_THUMBNAIL: AbProp = AbProp {
        name: "enable_channel_video_server_thumbnail",
        code: 11192,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CHAT_LIST_STICKER_EMOJIS: AbProp = AbProp {
        name: "enable_chat_list_sticker_emojis",
        code: 9069,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CHAT_PSA_AUTO_PLAY_VIDEOS: AbProp = AbProp {
        name: "enable_chat_psa_auto_play_videos",
        code: 3182,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CLEAR_FORMATTED_PREVIEW: AbProp = AbProp {
        name: "enable_clear_formatted_preview",
        code: 4659,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_COMMUNITY_SUSPEND_AND_APPEALS: AbProp = AbProp {
        name: "enable_community_suspend_and_appeals",
        code: 10539,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_COPY_PASTE_P2P: AbProp = AbProp {
        name: "enable_copy_paste_p2p",
        code: 27642,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_CTWA_ML_ENTRY_POINT_CONFIG: AbProp = AbProp {
        name: "enable_ctwa_ml_entry_point_config",
        code: 6216,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_DAYS_SINCE_RECEIVE_LOGGING: AbProp = AbProp {
        name: "enable_days_since_receive_logging",
        code: 3322,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_EARLY_AUDIO_DRIVER_START: AbProp = AbProp {
        name: "enable_early_audio_driver_start",
        code: 13807,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_EVENTS_V2_ADD_TO_CALENDAR: AbProp = AbProp {
        name: "enable_events_v2_add_to_calendar",
        code: 29417,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const ENABLE_EVENTS_V2_ENTRY_POINTS_CREATION: AbProp = AbProp {
        name: "enable_events_v2_entry_points_creation",
        code: 29361,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const ENABLE_EVENTS_V2_INVITE_MESSAGE_WITH_DATETIME: AbProp = AbProp {
        name: "enable_events_v2_invite_message_with_datetime",
        code: 32612,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_EVENTS_V2_ON_COMPANION: AbProp = AbProp {
        name: "enable_events_v2_on_companion",
        code: 30964,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_FMX_LOGGING: AbProp = AbProp {
        name: "enable_fmx_logging",
        code: 19893,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_FORCE_VOIP_LOGGING: AbProp = AbProp {
        name: "enable_force_voip_logging",
        code: 7300,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_FUTUREPROOF_GALAXY_FLOW_MESSAGE_FOR_BUSINESS_NUMBERS: AbProp = AbProp {
        name: "enable_futureproof_galaxy_flow_message_for_business_numbers",
        code: 22311,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const ENABLE_GRID_LAYOUT_TILE_UNIFICATION: AbProp = AbProp {
        name: "enable_grid_layout_tile_unification",
        code: 18066,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_GROUP_CREATE_OR_ADD_RATE_LIMITING_ERROR_UX: AbProp = AbProp {
        name: "enable_group_create_or_add_rate_limiting_error_ux",
        code: 12020,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_HYBRID_CALL_LINKS_CREATION: AbProp = AbProp {
        name: "enable_hybrid_call_links_creation",
        code: 15502,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_HYBRID_CALL_LINKS_JOIN: AbProp = AbProp {
        name: "enable_hybrid_call_links_join",
        code: 15501,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_HYBRID_VIDEO_TRANSCODING: AbProp = AbProp {
        name: "enable_hybrid_video_transcoding",
        code: 19895,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_HYBRID_VIDEO_TRANSCODING_FOR_VALID_MP4: AbProp = AbProp {
        name: "enable_hybrid_video_transcoding_for_valid_mp4",
        code: 20070,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_INIT_BWE_FOR_GROUP_CALL: AbProp = AbProp {
        name: "enable_init_bwe_for_group_call",
        code: 2601,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_JOIN_GROUP_CONTEXT_NON_AUTO_EXPOSE: AbProp = AbProp {
        name: "enable_join_group_context_non_auto_expose",
        code: 30282,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_LAZY_LOADING_OF_CALL_VIEW_ELEMENTS: AbProp = AbProp {
        name: "enable_lazy_loading_of_call_view_elements",
        code: 5053,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_LID_CALL_LINK: AbProp = AbProp {
        name: "enable_lid_call_link",
        code: 8180,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_LOGGING_QBM_INCOMING_MESSAGE: AbProp = AbProp {
        name: "enable_logging_qbm_incoming_message",
        code: 25149,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_MENTION_EVERYONE_RECEIVER_WEB: AbProp = AbProp {
        name: "enable_mention_everyone_receiver_web",
        code: 24843,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_MENTION_EVERYONE_SENDER_WEB: AbProp = AbProp {
        name: "enable_mention_everyone_sender_web",
        code: 24844,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_MENTION_EVERYONE_SYNCD_SENDER: AbProp = AbProp {
        name: "enable_mention_everyone_syncd_sender",
        code: 24244,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_MINIMIZE_INDIVIDUAL_MUTATION_WRITE: AbProp = AbProp {
        name: "enable_minimize_individual_mutation_write",
        code: 8910,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_ML_BWE_MODEL_DOWNLOAD: AbProp = AbProp {
        name: "enable_ml_bwe_model_download",
        code: 4349,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_NEW_ONGOING_CALL_CELL_UI: AbProp = AbProp {
        name: "enable_new_ongoing_call_cell_ui",
        code: 11426,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_NEW_USER_ACTION_STANZA_FOR_RAISE_HAND_SENDER: AbProp = AbProp {
        name: "enable_new_user_action_stanza_for_raise_hand_sender",
        code: 18489,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_OFFER_V2_UPGRADE: AbProp = AbProp {
        name: "enable_offer_v2_upgrade",
        code: 26435,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_ORBIT_SSO_BRIDGE: AbProp = AbProp {
        name: "enable_orbit_sso_bridge",
        code: 32299,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_ORDER_DETAILS_FOR_PAYMENT_KEY: AbProp = AbProp {
        name: "enable_order_details_for_payment_key",
        code: 27643,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_PEER_SNAPSHOT_RECOVERY: AbProp = AbProp {
        name: "enable_peer_snapshot_recovery",
        code: 16329,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_POLL_SETTINGS_LABEL_IMPROVED_LAYOUT: AbProp = AbProp {
        name: "enable_poll_settings_label_improved_layout",
        code: 32778,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_PRE_WARM_AUDIO_COMPONENT: AbProp = AbProp {
        name: "enable_pre_warm_audio_component",
        code: 15994,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_PRIVACY_TOKEN_WITH_TIMESTAMP: AbProp = AbProp {
        name: "enable_privacy_token_with_timestamp",
        code: 4992,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_PRODUCT_CAROUSEL_MESSAGE: AbProp = AbProp {
        name: "enable_product_carousel_message",
        code: 7177,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_RATE_APP_PROMPT: AbProp = AbProp {
        name: "enable_rate_app_prompt",
        code: 19894,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_RING_FOR_GC_ON_OFFER_EXPIRE: AbProp = AbProp {
        name: "enable_ring_for_gc_on_offer_expire",
        code: 10103,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SCHEDULED_CALLS_V2_ENTRY_POINTS_CREATION: AbProp = AbProp {
        name: "enable_scheduled_calls_v2_entry_points_creation",
        code: 29793,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const ENABLE_SETUP_ERROR_RESULT_CHECK: AbProp = AbProp {
        name: "enable_setup_error_result_check",
        code: 28689,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SHARING_FILES_FROM_WEB_WINDOWS_HYBRID: AbProp = AbProp {
        name: "enable_sharing_files_from_web_windows_hybrid",
        code: 21184,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SILENT_OFFER: AbProp = AbProp {
        name: "enable_silent_offer",
        code: 3235,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SOOX_MESSAGE_SENDING: AbProp = AbProp {
        name: "enable_soox_message_sending",
        code: 2832,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SPAM_REPORT_IQ_WITH_PRIVACY_TOKEN: AbProp = AbProp {
        name: "enable_spam_report_iq_with_privacy_token",
        code: 4991,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_STICKER_VERIFICATION_FOR_GIMMICK: AbProp = AbProp {
        name: "enable_sticker_verification_for_gimmick",
        code: 7886,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_SYNC_FOR_DRAFT_MESSAGES: AbProp = AbProp {
        name: "enable_sync_for_draft_messages",
        code: 29314,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SYNCD_COEX_V2: AbProp = AbProp {
        name: "enable_syncd_coex_v2",
        code: 31810,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_SYNCD_DEBUG_DATA_IN_PATCH: AbProp = AbProp {
        name: "enable_syncd_debug_data_in_patch",
        code: 6614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_TOOLTIP_FOR_MEDIA_HUB: AbProp = AbProp {
        name: "enable_tooltip_for_media_hub",
        code: 21535,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_TURN_ON_CALL_NOTIFICATION_REMINDERS: AbProp = AbProp {
        name: "enable_turn_on_call_notification_reminders",
        code: 5360,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UGC_VOICE_FS_LOGGING: AbProp = AbProp {
        name: "enable_ugc_voice_fs_logging",
        code: 14641,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UNIFIED_CALL_BUTTONS_IN_CHAT: AbProp = AbProp {
        name: "enable_unified_call_buttons_in_chat",
        code: 13497,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UPCOMING_SCHEDULE_CALL_EVENTS_IN_CALLS_TAB: AbProp = AbProp {
        name: "enable_upcoming_schedule_call_events_in_calls_tab",
        code: 15514,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UWP_DEVICE_SWITCH_BANNER: AbProp = AbProp {
        name: "enable_uwp_device_switch_banner",
        code: 10416,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UWP_SCREEN_SHARE_TEACHING_TIP: AbProp = AbProp {
        name: "enable_uwp_screen_share_teaching_tip",
        code: 6264,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_UWP_SHARE_ANY_WINDOW: AbProp = AbProp {
        name: "enable_uwp_share_any_window",
        code: 4801,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_UWP_SWAP_VIDEO_STREAM: AbProp = AbProp {
        name: "enable_uwp_swap_video_stream",
        code: 10241,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_VIDEO_METRICS_FIX: AbProp = AbProp {
        name: "enable_video_metrics_fix",
        code: 20520,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WAITING_ROOM_ADMIN_UI: AbProp = AbProp {
        name: "enable_waiting_room_admin_ui",
        code: 21676,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WAITING_ROOM_LOGGING: AbProp = AbProp {
        name: "enable_waiting_room_logging",
        code: 24991,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WAITING_ROOM_UI: AbProp = AbProp {
        name: "enable_waiting_room_ui",
        code: 19819,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WDS_CALLING_DROPDOWN: AbProp = AbProp {
        name: "enable_wds_calling_dropdown",
        code: 26974,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_CALLING: AbProp = AbProp {
        name: "enable_web_calling",
        code: 15461,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_CALLING_BETA_UPSELL: AbProp = AbProp {
        name: "enable_web_calling_beta_upsell",
        code: 24812,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_CALLING_NUX: AbProp = AbProp {
        name: "enable_web_calling_nux",
        code: 24504,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_GROUP_CALLING: AbProp = AbProp {
        name: "enable_web_group_calling",
        code: 20924,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_LOG_DOWNLOAD: AbProp = AbProp {
        name: "enable_web_log_download",
        code: 28226,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_ANR_OPTIMIZATIONS: AbProp = AbProp {
        name: "enable_web_voip_anr_optimizations",
        code: 27268,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_DYNAMIC_FPS_THROTTLE: AbProp = AbProp {
        name: "enable_web_voip_dynamic_fps_throttle",
        code: 25394,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_WEB_VOIP_EAGER_MIC_ACQUIRE: AbProp = AbProp {
        name: "enable_web_voip_eager_mic_acquire",
        code: 29836,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_P2P: AbProp = AbProp {
        name: "enable_web_voip_p2p",
        code: 25621,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_PLATFORM_AV_SYNC: AbProp = AbProp {
        name: "enable_web_voip_platform_av_sync",
        code: 25177,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_PROXY_AND_SCTP_WORKERS: AbProp = AbProp {
        name: "enable_web_voip_proxy_and_sctp_workers",
        code: 26012,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
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
    pub const ENABLE_WEB_VOIP_VIRTUAL_VIDEO_CAPTURE_DRIVER: AbProp = AbProp {
        name: "enable_web_voip_virtual_video_capture_driver",
        code: 26817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEB_VOIP_WEBTRANSPORT: AbProp = AbProp {
        name: "enable_web_voip_webtransport",
        code: 29764,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEBCODEC_REQUIRE_KEYFRAME: AbProp = AbProp {
        name: "enable_webcodec_require_keyframe",
        code: 29510,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const ENABLE_WEBCODEC_VIDEO_ENCODE: AbProp = AbProp {
        name: "enable_webcodec_video_encode",
        code: 26079,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEBRTC_VIDEO_JB: AbProp = AbProp {
        name: "enable_webrtc_video_jb",
        code: 27591,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WEFR_CLIENT_EXPO_PULSE: AbProp = AbProp {
        name: "enable_wefr_client_expo_pulse",
        code: 10230,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WINDOWS_HYBRID_JUMPLIST_CONTACTS: AbProp = AbProp {
        name: "enable_windows_hybrid_jumplist_contacts",
        code: 21057,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WINDOWS_JUMPLIST_HYBRID: AbProp = AbProp {
        name: "enable_windows_jumplist_hybrid",
        code: 20899,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WINDOWS_MOCKS_CAPTURE_DRIVERS: AbProp = AbProp {
        name: "enable_windows_mocks_capture_drivers",
        code: 31159,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENABLE_WINDOWS_XDR_CHAT_HANDOFF: AbProp = AbProp {
        name: "enable_windows_xdr_chat_handoff",
        code: 24783,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ENHANCED_MENTION_LIMIT: AbProp = AbProp {
        name: "enhanced_mention_limit",
        code: 25951,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const ENHANCED_MENTION_SUGGESTIONS_MIN_MENTION_CHAR_COUNT: AbProp = AbProp {
        name: "enhanced_mention_suggestions_min_mention_char_count",
        code: 28089,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const ENHANCED_MENTION_SUGGESTIONS_NON_GROUP_MEMBERS_ENABLED: AbProp = AbProp {
        name: "enhanced_mention_suggestions_non_group_members_enabled",
        code: 24852,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EPHEMERAL_SYNC_RESPONSE: AbProp = AbProp {
        name: "ephemeral_sync_response",
        code: 2714,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENT_DESCRIPTION_LENGTH_LIMIT: AbProp = AbProp {
        name: "event_description_length_limit",
        code: 6208,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2048),
    };
    pub const EVENT_NAME_LENGTH_LIMIT: AbProp = AbProp {
        name: "event_name_length_limit",
        code: 6207,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const EVENTS_CREATE: AbProp = AbProp {
        name: "events_create",
        code: 5562,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const EVENTS_CREATE_CAG_ENABLED: AbProp = AbProp {
        name: "events_create_cag_enabled",
        code: 9932,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENTS_EDIT_RECEIVE: AbProp = AbProp {
        name: "events_edit_receive",
        code: 7358,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENTS_M3_COVER_IMAGE_RECEIVE: AbProp = AbProp {
        name: "events_m3_cover_image_receive",
        code: 7511,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENTS_M3_COVER_IMAGE_SEND: AbProp = AbProp {
        name: "events_m3_cover_image_send",
        code: 7510,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENTS_V2_ENABLE_NOTIFICATIONS: AbProp = AbProp {
        name: "events_v2_enable_notifications",
        code: 31418,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVENTS_V2_HIDE_ADD_TO_CALENDAR_POST_START_WINDOW_SEC: AbProp = AbProp {
        name: "events_v2_hide_add_to_calendar_post_start_window_sec",
        code: 30826,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1800),
    };
    pub const EVENTS_V2_INVITATION_MESSAGE_VERSION: AbProp = AbProp {
        name: "events_v2_invitation_message_version",
        code: 26618,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const EVOLVE_ABOUT_M1_RECEIVER_ENABLED: AbProp = AbProp {
        name: "evolve_about_m1_receiver_enabled",
        code: 5839,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EVOLVE_ABOUT_M1_RECEIVER_FOR_NEW_SURFACES_ENABLED: AbProp = AbProp {
        name: "evolve_about_m1_receiver_for_new_surfaces_enabled",
        code: 6172,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EXPAND_FMX_MEX_SHOULD_USE_FMX_USE_CASE: AbProp = AbProp {
        name: "expand_fmx_mex_should_use_fmx_use_case",
        code: 27662,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EXTENSIONS_GEOBLOCKING_ENABLED: AbProp = AbProp {
        name: "extensions_geoblocking_enabled",
        code: 5333,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EXTENSIONS_USER_REPORT_STORE_MAX_DATA_EXCHANGES_PER_SESSION: AbProp = AbProp {
        name: "extensions_user_report_store_max_data_exchanges_per_session",
        code: 3211,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const EXTENSIONS_USER_REPORT_STORE_MAX_DATA_MAX_SESSIONS_PER_MESSAGE: AbProp = AbProp {
        name: "extensions_user_report_store_max_data_max_sessions_per_message",
        code: 3212,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const EXTERNAL_BETA_CAN_JOIN: AbProp = AbProp {
        name: "external_beta_can_join",
        code: 3081,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EXTERNAL_CTX_AUTHORISE_EXISTING_CHATS: AbProp = AbProp {
        name: "external_ctx_authorise_existing_chats",
        code: 12761,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const EXTERNAL_CTX_AUTHORISE_WA_CHAT: AbProp = AbProp {
        name: "external_ctx_authorise_wa_chat",
        code: 11655,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const EXTERNAL_CTX_FOA_LOGGING: AbProp = AbProp {
        name: "external_ctx_foa_logging",
        code: 13565,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const EXTERNAL_CTX_URL_PARAM_NAMES: AbProp = AbProp {
        name: "external_ctx_url_param_names",
        code: 12726,
        value_type: AbPropType::Str,
        default: AbDefault::Str("partnertoken"),
    };
    pub const FAVORITE_STICKER_SYNC_AFTER_PAIRING_ENABLED_WEB: AbProp = AbProp {
        name: "favorite_sticker_sync_after_pairing_enabled_web",
        code: 20815,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FAVORITES_LIMIT: AbProp = AbProp {
        name: "favorites_limit",
        code: 7267,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const FEATURE_KEY_STORE_INFRA_ENABLED: AbProp = AbProp {
        name: "feature_key_store_infra_enabled",
        code: 26829,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FETCH_QP_VIA_GRAPHQL_WEB_ENABLED: AbProp = AbProp {
        name: "fetch_qp_via_graphql_web_enabled",
        code: 28158,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FLATTENED_REACTIONS_COLLECTION: AbProp = AbProp {
        name: "flattened_reactions_collection",
        code: 4390,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FLOWS_TERMINATION_MESSAGE_V2_SENDING_ENABLED: AbProp = AbProp {
        name: "flows_termination_message_v2_sending_enabled",
        code: 9157,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FLOWS_WA_WEB: AbProp = AbProp {
        name: "flows_wa_web",
        code: 12520,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FLOWS_WA_WEB_AGM_CTA: AbProp = AbProp {
        name: "flows_wa_web_agm_cta",
        code: 24215,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FLOWS_WA_WEB_RESPONSES_DOWNLOAD: AbProp = AbProp {
        name: "flows_wa_web_responses_download",
        code: 24216,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FMX_CTWA_KILL_SWITCH: AbProp = AbProp {
        name: "fmx_ctwa_kill_switch",
        code: 6061,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FORWARDED_MESSAGE_USER_JOURNEY_LOGGING_ENABLED: AbProp = AbProp {
        name: "forwarded_message_user_journey_logging_enabled",
        code: 16055,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FOUR_REACTIONS_IN_BUBBLE_ENABLED: AbProp = AbProp {
        name: "four_reactions_in_bubble_enabled",
        code: 2378,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FT_VALIDATION_FAILURE_DROP_PLACEHOLDER: AbProp = AbProp {
        name: "ft_validation_failure_drop_placeholder",
        code: 13063,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FULLSCREEN_ANIMATION_FOR_KEYWORD: AbProp = AbProp {
        name: "fullscreen_animation_for_keyword",
        code: 2776,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FUNCTIONAL_CHATLIST_ENABLED: AbProp = AbProp {
        name: "functional_chatlist_enabled",
        code: 21799,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const FUTUREPROOF_ASSOCIATED_CHILD_ENABLED: AbProp = AbProp {
        name: "futureproof_associated_child_enabled",
        code: 11976,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GC_DEVICE_SWITCHING_KILLSWITCH: AbProp = AbProp {
        name: "gc_device_switching_killswitch",
        code: 26182,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GENAI_EARLY_AUDIO_PRE_BUF_SIZE: AbProp = AbProp {
        name: "genai_early_audio_pre_buf_size",
        code: 15306,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const GIF_MAX_PLAY_DURATION: AbProp = AbProp {
        name: "gif_max_play_duration",
        code: 3684,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const GIF_MAX_PLAY_LOOPS: AbProp = AbProp {
        name: "gif_max_play_loops",
        code: 3683,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const GIF_MIN_PLAY_LOOPS: AbProp = AbProp {
        name: "gif_min_play_loops",
        code: 3682,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const GIF_PROVIDER: AbProp = AbProp {
        name: "gif_provider",
        code: 14343,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const GIMMICK_PHASE_TWO_DATA_SUFFIX: AbProp = AbProp {
        name: "gimmick_phase_two_data_suffix",
        code: 6785,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const GIPHY_PMA_SHUTOFF_ENABLED: AbProp = AbProp {
        name: "giphy_pma_shutoff_enabled",
        code: 27942,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GRAPHQL_GET_PRODUCT_LIST: AbProp = AbProp {
        name: "graphql_get_product_list",
        code: 8800,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GRAPHQL_LOCALE_REMAPPING: AbProp = AbProp {
        name: "graphql_locale_remapping",
        code: 2014,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const GROUP_CALL_MAX_PARTICIPANTS: AbProp = AbProp {
        name: "group_call_max_participants",
        code: 4190,
        value_type: AbPropType::Int,
        default: AbDefault::Int(32),
    };
    pub const GROUP_CALLING_WAVE_RECEIVING_ENABLED: AbProp = AbProp {
        name: "group_calling_wave_receiving_enabled",
        code: 29161,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_CALLING_WAVE_SENDING_ENABLED: AbProp = AbProp {
        name: "group_calling_wave_sending_enabled",
        code: 29247,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_CATCH_UP: AbProp = AbProp {
        name: "group_catch_up",
        code: 559,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_CREATE_ADD_USING_LID_JIDS: AbProp = AbProp {
        name: "group_create_add_using_lid_jids",
        code: 16192,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_DESCRIPTION_LENGTH: AbProp = AbProp {
        name: "group_description_length",
        code: 14778,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2048),
    };
    pub const GROUP_FROM_GROUP: AbProp = AbProp {
        name: "group_from_group",
        code: 24024,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_AFTER_JOIN_PREREQUISITES: AbProp = AbProp {
        name: "group_history_after_join_prerequisites",
        code: 28787,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_BUMP_MESSAGE_ID: AbProp = AbProp {
        name: "group_history_bump_message_id",
        code: 16346,
        value_type: AbPropType::Int,
        default: AbDefault::Int(200),
    };
    pub const GROUP_HISTORY_BUNDLE_TIME_LIMIT_RECEIVER_ENFORCEMENT_SECS: AbProp = AbProp {
        name: "group_history_bundle_time_limit_receiver_enforcement_secs",
        code: 25910,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1209600),
    };
    pub const GROUP_HISTORY_MESSAGE_COUNT_LIMIT: AbProp = AbProp {
        name: "group_history_message_count_limit",
        code: 18405,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const GROUP_HISTORY_MESSAGE_COUNT_RECEIVER_UPPER_LIMIT: AbProp = AbProp {
        name: "group_history_message_count_receiver_upper_limit",
        code: 19811,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const GROUP_HISTORY_MESSAGES_TIME_LIMIT_RECEIVER_ENFORCEMENT_SECS: AbProp = AbProp {
        name: "group_history_messages_time_limit_receiver_enforcement_secs",
        code: 21313,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1209600),
    };
    pub const GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS: AbProp = AbProp {
        name: "group_history_messages_time_limit_secs",
        code: 18406,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1209600),
    };
    pub const GROUP_HISTORY_NEW_USER_THRESHOLD_RECEIVER_ENFORCEMENT_SECS: AbProp = AbProp {
        name: "group_history_new_user_threshold_receiver_enforcement_secs",
        code: 30345,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2592000),
    };
    pub const GROUP_HISTORY_NEW_USER_THRESHOLD_SECS: AbProp = AbProp {
        name: "group_history_new_user_threshold_secs",
        code: 30333,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2592000),
    };
    pub const GROUP_HISTORY_NOTICE_RECEIVE: AbProp = AbProp {
        name: "group_history_notice_receive",
        code: 15722,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_OUT_OF_WINDOW_PIN_SENDER: AbProp = AbProp {
        name: "group_history_out_of_window_pin_sender",
        code: 26037,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_OUT_OF_WINDOW_PINS_RECEIVER: AbProp = AbProp {
        name: "group_history_out_of_window_pins_receiver",
        code: 26039,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_RECEIVE: AbProp = AbProp {
        name: "group_history_receive",
        code: 15311,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_RECEIVER_DEDUP: AbProp = AbProp {
        name: "group_history_receiver_dedup",
        code: 30462,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_RECEIVER_FLOATING_BANNER: AbProp = AbProp {
        name: "group_history_receiver_floating_banner",
        code: 21568,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_REPORTING: AbProp = AbProp {
        name: "group_history_reporting",
        code: 22329,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const GROUP_HISTORY_SEND: AbProp = AbProp {
        name: "group_history_send",
        code: 15313,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SEND_AFTER_JOIN: AbProp = AbProp {
        name: "group_history_send_after_join",
        code: 26451,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTING_DECOUPLE_ENABLED: AbProp = AbProp {
        name: "group_history_setting_decouple_enabled",
        code: 29973,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTINGS: AbProp = AbProp {
        name: "group_history_settings",
        code: 21261,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTINGS_QUERY: AbProp = AbProp {
        name: "group_history_settings_query",
        code: 22230,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTINGS_TOGGLE_UI: AbProp = AbProp {
        name: "group_history_settings_toggle_ui",
        code: 21481,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SUPPORT_HISTORY_SYNC_RECEIVER_PRE_CHAT: AbProp = AbProp {
        name: "group_history_support_history_sync_receiver_pre_chat",
        code: 20658,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_JOIN_REQUEST_CAN_SEND_OPTIONAL_MESSAGE: AbProp = AbProp {
        name: "group_join_request_can_send_optional_message",
        code: 3384,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_JOIN_REQUEST_CAN_VIEW_OPTIONAL_MESSAGE: AbProp = AbProp {
        name: "group_join_request_can_view_optional_message",
        code: 3383,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_JOIN_REQUEST_M2_BANNER_ON_CONVERSATION: AbProp = AbProp {
        name: "group_join_request_m2_banner_on_conversation",
        code: 2449,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MAX_SUBJECT: AbProp = AbProp {
        name: "group_max_subject",
        code: 14801,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const GROUP_MEMBER_UPDATES_HIDE_IN_THREAD_ENABLED: AbProp = AbProp {
        name: "group_member_updates_hide_in_thread_enabled",
        code: 24584,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MEMBER_UPDATES_PAST_PARTICIPANT_MIGRATION_ENABLED: AbProp = AbProp {
        name: "group_member_updates_past_participant_migration_enabled",
        code: 31614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MEMBER_UPDATES_USERNAME_DESCRIPTION_ENABLED: AbProp = AbProp {
        name: "group_member_updates_username_description_enabled",
        code: 28087,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MEMBER_UPDATES_USERNAMES_DB_ENABLED: AbProp = AbProp {
        name: "group_member_updates_usernames_db_enabled",
        code: 24586,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MEMBER_UPDATES_USERNAMES_ENABLED: AbProp = AbProp {
        name: "group_member_updates_usernames_enabled",
        code: 24617,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_MEMBER_UPDATES_USERNAMES_UI_ENABLED: AbProp = AbProp {
        name: "group_member_updates_usernames_ui_enabled",
        code: 24585,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_SIZE_BYPASSING_SAMPLING: AbProp = AbProp {
        name: "group_size_bypassing_sampling",
        code: 1861,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100000),
    };
    pub const GROUP_SIZE_LIMIT: AbProp = AbProp {
        name: "group_size_limit",
        code: 1304,
        value_type: AbPropType::Int,
        default: AbDefault::Int(257),
    };
    pub const GROUP_STATUS_RECEIVER_ENABLED: AbProp = AbProp {
        name: "group_status_receiver_enabled",
        code: 13956,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_SUSPEND_APPEAL_INCLUDE_ENTITY_ID_ENABLED: AbProp = AbProp {
        name: "group_suspend_appeal_include_entity_id_enabled",
        code: 2057,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_SUSPEND_V2_ENABLED: AbProp = AbProp {
        name: "group_suspend_v2_enabled",
        code: 3180,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_SUSPENSION_APPEALS_REDESIGN_ENABLED: AbProp = AbProp {
        name: "group_suspension_appeals_redesign_enabled",
        code: 26276,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_SUSPENSION_APPEALS_REDESIGN_VARIANT_ENABLE: AbProp = AbProp {
        name: "group_suspension_appeals_redesign_variant_enable",
        code: 28376,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_USERNAME_UPDATES_AS_MEMBER_UPDATES_ENABLED: AbProp = AbProp {
        name: "group_username_updates_as_member_updates_enabled",
        code: 24477,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HAND_RAISE_RECEIVER_ENABLED: AbProp = AbProp {
        name: "hand_raise_receiver_enabled",
        code: 13540,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HARMFUL_FILE_DIALOG_LOGGING: AbProp = AbProp {
        name: "harmful_file_dialog_logging",
        code: 15020,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HASH_IDENTITY_KEYS_FOR_QR_CODE_DEVICE_VERIFICATION: AbProp = AbProp {
        name: "hash_identity_keys_for_qr_code_device_verification",
        code: 9211,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HATCH_PAIRING_FROM_COMPANION_ENABLED: AbProp = AbProp {
        name: "hatch_pairing_from_companion_enabled",
        code: 32497,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HD_VIDEO_DEFINITION_MAX_EDGE: AbProp = AbProp {
        name: "hd_video_definition_max_edge",
        code: 4172,
        value_type: AbPropType::Int,
        default: AbDefault::Int(864),
    };
    pub const HD_VIDEO_DEFINITION_MIN_EDGE: AbProp = AbProp {
        name: "hd_video_definition_min_edge",
        code: 4171,
        value_type: AbPropType::Int,
        default: AbDefault::Int(720),
    };
    pub const HD_VIDEO_DEFINITION_MIN_EDGE_WITH_MAX_EDGE: AbProp = AbProp {
        name: "hd_video_definition_min_edge_with_max_edge",
        code: 4175,
        value_type: AbPropType::Int,
        default: AbDefault::Int(480),
    };
    pub const HEARTBEAT_INTERVAL_S: AbProp = AbProp {
        name: "heartbeat_interval_s",
        code: 1430,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const HIDE_AUTO_QUOTES_ON_WEB: AbProp = AbProp {
        name: "hide_auto_quotes_on_web",
        code: 20892,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HIDE_SILENT_SYSTEM_MESSAGE_ENABLED: AbProp = AbProp {
        name: "hide_silent_system_message_enabled",
        code: 24268,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HISTORY_SYNC_ON_DEMAND: AbProp = AbProp {
        name: "history_sync_on_demand",
        code: 3337,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HISTORY_SYNC_ON_DEMAND_COMPANION: AbProp = AbProp {
        name: "history_sync_on_demand_companion",
        code: 17198,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HISTORY_SYNC_ON_DEMAND_COOLDOWN_SEC: AbProp = AbProp {
        name: "history_sync_on_demand_cooldown_sec",
        code: 4365,
        value_type: AbPropType::Int,
        default: AbDefault::Int(7200),
    };
    pub const HISTORY_SYNC_ON_DEMAND_FAILURE_LIMIT: AbProp = AbProp {
        name: "history_sync_on_demand_failure_limit",
        code: 4364,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const HISTORY_SYNC_ON_DEMAND_MESSAGE_COUNT: AbProp = AbProp {
        name: "history_sync_on_demand_message_count",
        code: 3811,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const HISTORY_SYNC_ON_DEMAND_TIME_BOUNDARY_DAYS_DESKTOPS: AbProp = AbProp {
        name: "history_sync_on_demand_time_boundary_days_desktops",
        code: 18391,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1095),
    };
    pub const HISTORY_SYNC_ON_DEMAND_TIMEOUT_MS: AbProp = AbProp {
        name: "history_sync_on_demand_timeout_ms",
        code: 3882,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10000),
    };
    pub const HISTORY_SYNC_ON_DEMAND_WITH_ANDROID_BETA: AbProp = AbProp {
        name: "history_sync_on_demand_with_android_beta",
        code: 4135,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HOSTED_MESSAGE_FLAG_ENABLED: AbProp = AbProp {
        name: "hosted_message_flag_enabled",
        code: 27979,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HSM_TAG_IN_HISTORY_SYNC_DESERIALIZATION_ENABLED: AbProp = AbProp {
        name: "hsm_tag_in_history_sync_deserialization_enabled",
        code: 25804,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HYBRID_EDUCATIONAL_DIALOG_START_AT: AbProp = AbProp {
        name: "hybrid_educational_dialog_start_at",
        code: 14675,
        value_type: AbPropType::Str,
        default: AbDefault::Str(" "),
    };
    pub const HYBRID_EDUCATIONAL_DIALOGS_ENABLED: AbProp = AbProp {
        name: "hybrid_educational_dialogs_enabled",
        code: 14674,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HYBRID_FLYTRAP_FEEDBACK_ENABLED: AbProp = AbProp {
        name: "hybrid_flytrap_feedback_enabled",
        code: 19495,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HYBRID_FONT_SIZE_DROPDOWN: AbProp = AbProp {
        name: "hybrid_font_size_dropdown",
        code: 17637,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HYBRID_INCREMENTAL_ZOOMING_SIMPLE_ENABLED: AbProp = AbProp {
        name: "hybrid_incremental_zooming_simple_enabled",
        code: 18080,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const HYBRID_NUX_BETA_50_ENABLED: AbProp = AbProp {
        name: "hybrid_nux_beta_50_enabled",
        code: 17717,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IGNORE_JOINABLE_TERMINATE_ON_EXPIRED_OFFER: AbProp = AbProp {
        name: "ignore_joinable_terminate_on_expired_offer",
        code: 11519,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IGNORE_ONE_TO_ONE_TERMINATE_IN_GROUP_CALL: AbProp = AbProp {
        name: "ignore_one_to_one_terminate_in_group_call",
        code: 10273,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IM_BLOKS_WIDGET_ENABLE: AbProp = AbProp {
        name: "im_bloks_widget_enable",
        code: 25071,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IM_NFM_MULTI_STEP_FORM_KILLSWITCH: AbProp = AbProp {
        name: "im_nfm_multi_step_form_killswitch",
        code: 28891,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IMP_SEND_SIGNAL_POST_CONNECT_DELAY: AbProp = AbProp {
        name: "imp_send_signal_post_connect_delay",
        code: 23323,
        value_type: AbPropType::Int,
        default: AbDefault::Int(500),
    };
    pub const IMP_SEND_SIGNAL_POST_CONNECT_WEBC_ENABLED: AbProp = AbProp {
        name: "imp_send_signal_post_connect_webc_enabled",
        code: 23322,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IMPROVE_GROUP_REPORTING: AbProp = AbProp {
        name: "improve_group_reporting",
        code: 26114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IMPROVE_SUBGROUP_ACTIVATION_SUBGROUP_POLL_INTERVAL: AbProp = AbProp {
        name: "improve_subgroup_activation_subgroup_poll_interval",
        code: 8542,
        value_type: AbPropType::Int,
        default: AbDefault::Int(43200),
    };
    pub const IN_APP_BUG_REPORTING_DESCRIPTION_GOOD_QUALITY_CHARS: AbProp = AbProp {
        name: "in_app_bug_reporting_description_good_quality_chars",
        code: 22361,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const IN_APP_BUG_REPORTING_SHOW_QUALITY_HINTS_V1: AbProp = AbProp {
        name: "in_app_bug_reporting_show_quality_hints_v1",
        code: 22363,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IN_APP_COMMS_MANAGE_ADS_WEB_BANNER_CAMPAIGN_ENABLED: AbProp = AbProp {
        name: "in_app_comms_manage_ads_web_banner_campaign_enabled",
        code: 4542,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IN_APP_SUPPORT_CAPI_NUMBER_PREFIXES: AbProp = AbProp {
        name: "in_app_support_capi_number_prefixes",
        code: 4799,
        value_type: AbPropType::Str,
        default: AbDefault::Str("155178684"),
    };
    pub const IN_APP_SUPPORT_V2_NUMBER_PREFIXES: AbProp = AbProp {
        name: "in_app_support_v2_number_prefixes",
        code: 1031,
        value_type: AbPropType::Str,
        default: AbDefault::Str("15517868"),
    };
    pub const INAPP_SIGNUP_AGM_CTA_EXPERIMENT: AbProp = AbProp {
        name: "inapp_signup_agm_cta_experiment",
        code: 27860,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const INAPP_SIGNUP_CONFIRMATION_MESSAGE_ENABLED: AbProp = AbProp {
        name: "inapp_signup_confirmation_message_enabled",
        code: 26390,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INAPP_SIGNUP_M1_LOGGING_ENABLED: AbProp = AbProp {
        name: "inapp_signup_m1_logging_enabled",
        code: 28142,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INAPP_SIGNUP_QPL_LOGGING_ENABLED: AbProp = AbProp {
        name: "inapp_signup_qpl_logging_enabled",
        code: 28806,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INAPP_SIGNUP_WEB_CTA_LOGGING_ENABLED: AbProp = AbProp {
        name: "inapp_signup_web_cta_logging_enabled",
        code: 30498,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_CUSTOM_SMB_ENABLED: AbProp = AbProp {
        name: "inbox_filters_custom_smb_enabled",
        code: 7637,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_ENABLED: AbProp = AbProp {
        name: "inbox_filters_enabled",
        code: 5171,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_HAPTIC_FEEDBACK_ENABLED: AbProp = AbProp {
        name: "inbox_filters_haptic_feedback_enabled",
        code: 6052,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_READ_UNREAD_LOGGING_ENABLED: AbProp = AbProp {
        name: "inbox_filters_read_unread_logging_enabled",
        code: 6967,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_RESET_TIMEOUT: AbProp = AbProp {
        name: "inbox_filters_reset_timeout",
        code: 5765,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1800),
    };
    pub const INBOX_FILTERS_SMB_ENABLED: AbProp = AbProp {
        name: "inbox_filters_smb_enabled",
        code: 7108,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INBOX_FILTERS_SUPPRESS_CONTACT_FILTER: AbProp = AbProp {
        name: "inbox_filters_suppress_contact_filter",
        code: 7769,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
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
    pub const INTEGRITY_CHECKPOINTS_ENABLED: AbProp = AbProp {
        name: "integrity_checkpoints_enabled",
        code: 26961,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INTERACTIVE_BLOKS_WIDGET_WEB_ENABLED: AbProp = AbProp {
        name: "interactive_bloks_widget_web_enabled",
        code: 26685,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INTERACTIVE_MESSAGE_NATIVE_FLOW_KILLSWITCH: AbProp = AbProp {
        name: "interactive_message_native_flow_killswitch",
        code: 1133,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INTERACTIVE_RESPONSE_MESSAGE_KILLSWITCH: AbProp = AbProp {
        name: "interactive_response_message_killswitch",
        code: 1435,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INTERACTIVE_RESPONSE_MESSAGE_NATIVE_FLOW_KILLSWITCH: AbProp = AbProp {
        name: "interactive_response_message_native_flow_killswitch",
        code: 1436,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INTERNAL_GROUP_INDICATOR: AbProp = AbProp {
        name: "internal_group_indicator",
        code: 18109,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const INVITE_DEACTIVATED_USER_WEB: AbProp = AbProp {
        name: "invite_deactivated_user_web",
        code: 31516,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_AI_MODE_SELECTOR_VISIBLE: AbProp = AbProp {
        name: "is_ai_mode_selector_visible",
        code: 24489,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_EXPAND_FMX_ACCOUNT_AGE_BOLDED_NON_AUTO_EXPOSE: AbProp = AbProp {
        name: "is_expand_fmx_account_age_bolded_non_auto_expose",
        code: 26549,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_EXPAND_FMX_ACCOUNT_AGE_UI_ENABLED: AbProp = AbProp {
        name: "is_expand_fmx_account_age_ui_enabled",
        code: 26548,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_EXPAND_FMX_ENABLED_NON_AUTO_EXPOSE: AbProp = AbProp {
        name: "is_expand_fmx_enabled_non_auto_expose",
        code: 26551,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_EXPAND_FMX_MEX_ENABLED: AbProp = AbProp {
        name: "is_expand_fmx_mex_enabled",
        code: 26550,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_INDIVIDUAL_SUSPICIOUS_FMX_ENABLED: AbProp = AbProp {
        name: "is_individual_suspicious_fmx_enabled",
        code: 26191,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_INTERNAL_TESTER: AbProp = AbProp {
        name: "is_internal_tester",
        code: 2945,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_META_EMPLOYEE_OR_INTERNAL_TESTER: AbProp = AbProp {
        name: "is_meta_employee_or_internal_tester",
        code: 1777,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_PART_OF_GSC_EXPERIMENT: AbProp = AbProp {
        name: "is_part_of_gsc_experiment",
        code: 14279,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_PMX_FUNNEL_METRICS_LOGGING_ENABLED: AbProp = AbProp {
        name: "is_pmx_funnel_metrics_logging_enabled",
        code: 6816,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_PMX_HASHED_MSG_KEY_LOGGING_ENABLED: AbProp = AbProp {
        name: "is_pmx_hashed_msg_key_logging_enabled",
        code: 6837,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_SPOILER_RICH_FORMAT_ENABLED: AbProp = AbProp {
        name: "is_spoiler_rich_format_enabled",
        code: 22221,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const IS_SPOILER_RICH_FORMAT_SENDER_ENABLED: AbProp = AbProp {
        name: "is_spoiler_rich_format_sender_enabled",
        code: 24210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const JOINABLE_CLIENT_POLL_INTERVAL_MIN: AbProp = AbProp {
        name: "joinable_client_poll_interval_min",
        code: 522,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const KALEIDOSCOPE_THUMBNAIL_VALIDATION: AbProp = AbProp {
        name: "kaleidoscope_thumbnail_validation",
        code: 18114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const KEEP_IN_CHAT_UNDO_DURATION_LIMIT: AbProp = AbProp {
        name: "keep_in_chat_undo_duration_limit",
        code: 1698,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2592000),
    };
    pub const KILL_SWITCH_CTWA_ML_ENTRY_POINT_CONFIG: AbProp = AbProp {
        name: "kill_switch_ctwa_ml_entry_point_config",
        code: 6215,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
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
    pub const KS_USE_COMPONENT_MODEL: AbProp = AbProp {
        name: "ks_use_component_model",
        code: 26966,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LARGE_SCREENS_NEW_CHAT_BUTTON_VARIANTS: AbProp = AbProp {
        name: "large_screens_new_chat_button_variants",
        code: 26788,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const LAZY_SYSTEM_MESSAGE_INSERTION_ENABLED: AbProp = AbProp {
        name: "lazy_system_message_insertion_enabled",
        code: 9077,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_GROUP_CREATION_ADDRESSING_MODE_OVERRIDE: AbProp = AbProp {
        name: "lid_group_creation_addressing_mode_override",
        code: 12985,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_GROUP_MIGRATION_NON_MEMBER_IQ: AbProp = AbProp {
        name: "lid_group_migration_non_member_iq",
        code: 16104,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_MIGRATION_FOR_BIZ_PROFILE_ENABLED: AbProp = AbProp {
        name: "lid_migration_for_biz_profile_enabled",
        code: 12000,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_MIGRATION_FOR_VNAME_ENABLED: AbProp = AbProp {
        name: "lid_migration_for_vname_enabled",
        code: 11049,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_MIGRATION_NOTIFICATIONS_ENABLED: AbProp = AbProp {
        name: "lid_migration_notifications_enabled",
        code: 8785,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_ONE_ON_ONE_MIGRATION_COMPATIBLE: AbProp = AbProp {
        name: "lid_one_on_one_migration_compatible",
        code: 13161,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const LID_ONE_ON_ONE_MIGRATION_ENABLED: AbProp = AbProp {
        name: "lid_one_on_one_migration_enabled",
        code: 9435,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_ONE_ON_ONE_MIGRATION_LOG_OUT_ON_MISMATCH: AbProp = AbProp {
        name: "lid_one_on_one_migration_log_out_on_mismatch",
        code: 11050,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const LID_ONE_ON_ONE_MIGRATION_PEER_SYNC_TIMEOUT_IN_SECONDS: AbProp = AbProp {
        name: "lid_one_on_one_migration_peer_sync_timeout_in_seconds",
        code: 13936,
        value_type: AbPropType::Int,
        default: AbDefault::Int(300),
    };
    pub const LID_ONE_TO_ONE_MIGRATION_EVENT_RESPONSE_FORCE_PN_JID: AbProp = AbProp {
        name: "lid_one_to_one_migration_event_response_force_pn_jid",
        code: 15791,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_PN_USERNAME_MAPPING_LOGGING_ENABLED: AbProp = AbProp {
        name: "lid_pn_username_mapping_logging_enabled",
        code: 31266,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_STATUS_NON_SOAKED_CLIENT_SUPPORT_ENABLED: AbProp = AbProp {
        name: "lid_status_non_soaked_client_support_enabled",
        code: 19696,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const LID_STATUS_SEND_ENABLED: AbProp = AbProp {
        name: "lid_status_send_enabled",
        code: 6791,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LID_TRUSTED_TOKEN_ISSUE_TO_LID: AbProp = AbProp {
        name: "lid_trusted_token_issue_to_lid",
        code: 14303,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LIGHTWEIGHT_GROUP_CREATION: AbProp = AbProp {
        name: "lightweight_group_creation",
        code: 27819,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LIMIT_SHARING_ENABLED_FOR_1ON1_CHAT: AbProp = AbProp {
        name: "limit_sharing_enabled_for_1on1_chat",
        code: 15127,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LIMIT_SHARING_PROTOCOL_MESSAGE_RECEIVER_ENABLED: AbProp = AbProp {
        name: "limit_sharing_protocol_message_receiver_enabled",
        code: 15129,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LIMIT_SHARING_UPDATE_ENABLED_WEB: AbProp = AbProp {
        name: "limit_sharing_update_enabled_web",
        code: 16376,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LINK_PREVIEW_WAIT_TIME: AbProp = AbProp {
        name: "link_preview_wait_time",
        code: 2566,
        value_type: AbPropType::Int,
        default: AbDefault::Int(7),
    };
    pub const LISTS_CHAT_LIST_ROW_PILL_ENABLED: AbProp = AbProp {
        name: "lists_chat_list_row_pill_enabled",
        code: 24133,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LISTS_SMB_ENABLED: AbProp = AbProp {
        name: "lists_smb_enabled",
        code: 18229,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LISTS_SMB_WEB_ENABLED: AbProp = AbProp {
        name: "lists_smb_web_enabled",
        code: 24732,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LISTS_SMB_WEB_M2_ENABLED: AbProp = AbProp {
        name: "lists_smb_web_m2_enabled",
        code: 31380,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LOBBY_TIMEOUT_MIN: AbProp = AbProp {
        name: "lobby_timeout_min",
        code: 1565,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const LOG_CLOCK_SKEW: AbProp = AbProp {
        name: "log_clock_skew",
        code: 1190,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const LOW_CACHE_HIT_RATE_MEDIA_TYPES: AbProp = AbProp {
        name: "low_cache_hit_rate_media_types",
        code: 4836,
        value_type: AbPropType::Str,
        default: AbDefault::Str("ptt,audio,document,ppic"),
    };
    pub const LTHASH_CHECK_HOURS: AbProp = AbProp {
        name: "lthash_check_hours",
        code: 1104,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const M2_AUDIENCE_DYNAMIC_RULES: AbProp = AbProp {
        name: "m2_audience_dynamic_rules",
        code: 28099,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MARK_AS_VERIFIED_ENABLED: AbProp = AbProp {
        name: "mark_as_verified_enabled",
        code: 29343,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MAX_GROUP_SIZE_FOR_LONG_RINGTONE: AbProp = AbProp {
        name: "max_group_size_for_long_ringtone",
        code: 4710,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const MAX_NUM_PARTICIPANTS_FOR_SS: AbProp = AbProp {
        name: "max_num_participants_for_ss",
        code: 3694,
        value_type: AbPropType::Int,
        default: AbDefault::Int(8),
    };
    pub const MAXIMUM_GROUP_SIZE_FOR_RCAT: AbProp = AbProp {
        name: "maximum_group_size_for_rcat",
        code: 2915,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const MAY_HAVE_MESSAGES_ENABLED: AbProp = AbProp {
        name: "may_have_messages_enabled",
        code: 25303,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MC_ENABLED: AbProp = AbProp {
        name: "mc_enabled",
        code: 32843,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MD_APP_STATE_GATE_D34336913: AbProp = AbProp {
        name: "md_app_state_gate_D34336913",
        code: 1379,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MD_ICDC_HASH_LENGTH: AbProp = AbProp {
        name: "md_icdc_hash_length",
        code: 310,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const MD_OFFLINE_V2_M2_ENABLED: AbProp = AbProp {
        name: "md_offline_v2_m2_enabled",
        code: 1517,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const MD_SYNCD_BUNDLE_LOGGING: AbProp = AbProp {
        name: "md_syncd_bundle_logging",
        code: 27126,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"allowlist\": []}"),
    };
    pub const MD_SYNCD_MUTATION_LOGGING: AbProp = AbProp {
        name: "md_syncd_mutation_logging",
        code: 27124,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"allowlist\": []}"),
    };
    pub const MD_SYNCD_MUTATION_SUMMARY_LOGGING: AbProp = AbProp {
        name: "md_syncd_mutation_summary_logging",
        code: 27125,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"allowlist\": []}"),
    };
    pub const MEDIA_FORCE_TRANSCODE_ON_ELST: AbProp = AbProp {
        name: "media_force_transcode_on_elst",
        code: 30235,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEDIA_HUB_HISTORY_MAX_DAYS: AbProp = AbProp {
        name: "media_hub_history_max_days",
        code: 22518,
        value_type: AbPropType::Int,
        default: AbDefault::Int(14),
    };
    pub const MEDIA_LARGE_FILE_AWARENESS_POPUP_FILE_SIZE_IN_MB: AbProp = AbProp {
        name: "media_large_file_awareness_popup_file_size_in_MB",
        code: 3115,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2048),
    };
    pub const MEDIA_PICKER_SELECT_LIMIT: AbProp = AbProp {
        name: "media_picker_select_limit",
        code: 2614,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const MEDIA_PICKER_SELECT_LIMIT_NEW: AbProp = AbProp {
        name: "media_picker_select_limit_new",
        code: 2693,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const MEDIA_VIEWER_ACCELERATED_PLAYBACK_ENABLED: AbProp = AbProp {
        name: "media_viewer_accelerated_playback_enabled",
        code: 12813,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEMBER_NAME_TAG_DB_ENABLED: AbProp = AbProp {
        name: "member_name_tag_db_enabled",
        code: 16551,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MEMBER_NAME_TAG_RECEIVER_ENABLED: AbProp = AbProp {
        name: "member_name_tag_receiver_enabled",
        code: 13523,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEMBER_NAME_TAG_SENDER_ENABLED: AbProp = AbProp {
        name: "member_name_tag_sender_enabled",
        code: 13524,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEMBER_NAME_TAG_WEB_RECEIVER_ENABLED: AbProp = AbProp {
        name: "member_name_tag_web_receiver_enabled",
        code: 22655,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEMBER_NAME_TAG_WEB_SENDER_ENABLED: AbProp = AbProp {
        name: "member_name_tag_web_sender_enabled",
        code: 22654,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MESSAGE_ASSOCIATION_INFRA_ENABLED: AbProp = AbProp {
        name: "message_association_infra_enabled",
        code: 8783,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MESSAGE_CAPPING_UPSELL_VERSION: AbProp = AbProp {
        name: "message_capping_upsell_version",
        code: 19781,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const MESSAGE_COUNT_LOGGING_MD_ENABLED: AbProp = AbProp {
        name: "message_count_logging_md_enabled",
        code: 1135,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MESSAGE_EDIT_CLIENT_ENTRY_POINT_LIMIT_SECONDS: AbProp = AbProp {
        name: "message_edit_client_entry_point_limit_seconds",
        code: 3272,
        value_type: AbPropType::Int,
        default: AbDefault::Int(900),
    };
    pub const MESSAGE_EDIT_TO_MESSAGE_SECRET_RECEIVER_ENABLED: AbProp = AbProp {
        name: "message_edit_to_message_secret_receiver_enabled",
        code: 17811,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MESSAGE_EDIT_TO_MESSAGE_SECRET_SENDER_ENABLED: AbProp = AbProp {
        name: "message_edit_to_message_secret_sender_enabled",
        code: 16057,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MESSAGE_EDIT_WINDOW_DURATION_SECONDS: AbProp = AbProp {
        name: "message_edit_window_duration_seconds",
        code: 2983,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1200),
    };
    pub const MESSAGE_KEYS_ASYNC_CHUNK_SIZE: AbProp = AbProp {
        name: "message_keys_async_chunk_size",
        code: 22815,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const MESSAGE_PARTIAL_SELECTION_M2: AbProp = AbProp {
        name: "message_partial_selection_m2",
        code: 32142,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const META_AI_IN_APP_SURVEY_ENABLED: AbProp = AbProp {
        name: "meta_ai_in_app_survey_enabled",
        code: 17956,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const META_CATALOG_LINKING_M2_ENABLED: AbProp = AbProp {
        name: "meta_catalog_linking_m2_enabled",
        code: 11029,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const META_VERIFIED_BADGE_EDUCATION_VAI_CONTENT: AbProp = AbProp {
        name: "meta_verified_badge_education_vai_content",
        code: 7976,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEX_GET_PRIVACY_CONTACT_LIST_ENABLED: AbProp = AbProp {
        name: "mex_get_privacy_contact_list_enabled",
        code: 23874,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEX_GET_PRIVACY_SETTINGS_MODE: AbProp = AbProp {
        name: "mex_get_privacy_settings_mode",
        code: 23463,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const MEX_PHASE3_ENABLED: AbProp = AbProp {
        name: "mex_phase3_enabled",
        code: 2249,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEX_PHASE3_STATUS_FLAGS: AbProp = AbProp {
        name: "mex_phase3_status_flags",
        code: 2250,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const MEX_USYNC_ABOUT_STATUS: AbProp = AbProp {
        name: "mex_usync_about_status",
        code: 9524,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MEX_USYNC_USERNAME_QUERY: AbProp = AbProp {
        name: "mex_usync_username_query",
        code: 8421,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ML_MODEL_DOWNLOAD_SKIP_HASH_CHECK: AbProp = AbProp {
        name: "ml_model_download_skip_hash_check",
        code: 11454,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MM_1PD_POST_DC_DEPTH_LIMIT: AbProp = AbProp {
        name: "mm_1pd_post_dc_depth_limit",
        code: 26281,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const MM_1PD_POST_DC_NEW_SCHEMA_ENABLED: AbProp = AbProp {
        name: "mm_1pd_post_dc_new_schema_enabled",
        code: 26280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_1PD_POST_DC_OLD_SCHEMA_DISABLED: AbProp = AbProp {
        name: "mm_1pd_post_dc_old_schema_disabled",
        code: 26282,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_DATA_SHARING_DISCLOSURE_ENABLED: AbProp = AbProp {
        name: "mm_data_sharing_disclosure_enabled",
        code: 5869,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_DATA_SHARING_DISCLOSURE_ENABLED_ADDITIONAL_TRANSPARENCY_LARGE_SCREENS: AbProp =
        AbProp {
            name: "mm_data_sharing_disclosure_enabled_additional_transparency_large_screens",
            code: 25421,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const MM_DATA_SHARING_DISCLOSURE_ENABLED_COMPANION_HISTORY_SYNC: AbProp = AbProp {
        name: "mm_data_sharing_disclosure_enabled_companion_history_sync",
        code: 21288,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_DATA_SHARING_DISCLOSURE_ON_CHAT_OPEN_ENABLED: AbProp = AbProp {
        name: "mm_data_sharing_disclosure_on_chat_open_enabled",
        code: 17630,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_DISCLOSURE_HANDLE_TOS_FAILURES_ENABLED: AbProp = AbProp {
        name: "mm_disclosure_handle_tos_failures_enabled",
        code: 28572,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_DISCLOSURE_LEARN_MORE_ARTICLE_ID: AbProp = AbProp {
        name: "mm_disclosure_learn_more_article_id",
        code: 25021,
        value_type: AbPropType::Str,
        default: AbDefault::Str("263784176043634"),
    };
    pub const MM_MESSAGE_LEVEL_FEEDBACK_ENABLED: AbProp = AbProp {
        name: "mm_message_level_feedback_enabled",
        code: 10011,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_MESSAGE_LEVEL_FEEDBACK_NOT_INTERESTED_MENU_ENABLED: AbProp = AbProp {
        name: "mm_message_level_feedback_not_interested_menu_enabled",
        code: 10668,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPT_OUT_ENABLED: AbProp = AbProp {
        name: "mm_opt_out_enabled",
        code: 11241,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPT_OUT_FMX_STOP_FOR_HIGH_TRUST: AbProp = AbProp {
        name: "mm_opt_out_fmx_stop_for_high_trust",
        code: 12172,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPT_OUT_LID_MIGRATION_ENABLED: AbProp = AbProp {
        name: "mm_opt_out_lid_migration_enabled",
        code: 16952,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPTIMIZED_DELIVERY_APP_CTA_ENABLED: AbProp = AbProp {
        name: "mm_optimized_delivery_app_cta_enabled",
        code: 22776,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPTIMIZED_DELIVERY_ARCHIVE_SIGNAL_SHARING_ENABLED: AbProp = AbProp {
        name: "mm_optimized_delivery_archive_signal_sharing_enabled",
        code: 28558,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPTIMIZED_DELIVERY_REPLACING_SHIMMED_LINKS_ENABLED: AbProp = AbProp {
        name: "mm_optimized_delivery_replacing_shimmed_links_enabled",
        code: 21782,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_OPTIMIZED_DELIVERY_TOKEN_FALLBACK_DISABLED: AbProp = AbProp {
        name: "mm_optimized_delivery_token_fallback_disabled",
        code: 29002,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MM_OPTIMIZED_DELIVERY_UNIQUE_TOKEN_PER_MESSAGE_ID_ENABLED: AbProp = AbProp {
        name: "mm_optimized_delivery_unique_token_per_message_id_enabled",
        code: 29037,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MM_SIGNAL_SHARING_COLLECTION_WINDOW_LOGGING_ENABLED: AbProp = AbProp {
        name: "mm_signal_sharing_collection_window_logging_enabled",
        code: 18126,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_SIGNAL_SHARING_VERIFICATION_NEW_SIGNAL_TYPE_ORIGIN: AbProp = AbProp {
        name: "mm_signal_sharing_verification_new_signal_type_origin",
        code: 26784,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_SIGNAL_SHARING_VERIFICATION_SYSTEM_LID_ENABLED: AbProp = AbProp {
        name: "mm_signal_sharing_verification_system_lid_enabled",
        code: 16727,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const MM_TAP_TARGET_BLOKS_CLIENT_HYDRATION_ENABLED: AbProp = AbProp {
        name: "mm_tap_target_bloks_client_hydration_enabled",
        code: 28473,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_TEMPLATE_MESSAGE_TELEMETRY_IS_FIRST_MM_ENABLED: AbProp = AbProp {
        name: "mm_template_message_telemetry_is_first_mm_enabled",
        code: 32482,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_USER_CONTROLS_ENTRY_POINTS_UPDATE_M1_ICON: AbProp = AbProp {
        name: "mm_user_controls_entry_points_update_m1_icon",
        code: 20388,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_USER_CONTROLS_ENTRY_POINTS_UPDATE_M1_MENU: AbProp = AbProp {
        name: "mm_user_controls_entry_points_update_m1_menu",
        code: 20381,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MM_USER_CONTROLS_EXCEPTION_NUMBER_PREFIXES: AbProp = AbProp {
        name: "mm_user_controls_exception_number_prefixes",
        code: 13999,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const MM_USER_CONTROLS_EXPOSURE: AbProp = AbProp {
        name: "mm_user_controls_exposure",
        code: 13510,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MMS_VCACHE_AGGREGATION_ENABLED: AbProp = AbProp {
        name: "mms_vcache_aggregation_enabled",
        code: 2134,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const MUSIC_OHAI_PROXY_URL: AbProp = AbProp {
        name: "music_ohai_proxy_url",
        code: 10975,
        value_type: AbPropType::Str,
        default: AbDefault::Str("https://meta-ohttp-relay-prod.fastly-edge.com/"),
    };
    pub const NATIVE_CONTACT_COMPANION_CHANGE_ENABLED: AbProp = AbProp {
        name: "native_contact_companion_change_enabled",
        code: 7301,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NATIVE_CONTACT_COMPANION_NUX_LEARN_MORE_ARTICLE_ID: AbProp = AbProp {
        name: "native_contact_companion_nux_learn_more_article_id",
        code: 11644,
        value_type: AbPropType::Str,
        default: AbDefault::Str("1191526044909364"),
    };
    pub const NATIVE_FLOW_RESPONSE_MESSAGE_PARAMS_JSON_MAX_SIZE: AbProp = AbProp {
        name: "native_flow_response_message_params_json_max_size",
        code: 32367,
        value_type: AbPropType::Int,
        default: AbDefault::Int(262144),
    };
    pub const NATIVE_LIB_SANDBOXING_ENABLE_LIBWEBP: AbProp = AbProp {
        name: "native_lib_sandboxing_enable_libwebp",
        code: 26414,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEW_CHAT_MSG_CAPPING_FIRST_WARNING_THRESHOLD_PERCENTAGE: AbProp = AbProp {
        name: "new_chat_msg_capping_first_warning_threshold_percentage",
        code: 18967,
        value_type: AbPropType::Int,
        default: AbDefault::Int(50),
    };
    pub const NEW_END_CALL_SURVEY_POP_UP_USER_INTERVAL_S: AbProp = AbProp {
        name: "new_end_call_survey_pop_up_user_interval_s",
        code: 2553,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const NEWSLETTER_ADMIN_INVITE_NUX_ID: AbProp = AbProp {
        name: "newsletter_admin_invite_nux_id",
        code: 15256,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610220"),
    };
    pub const NEWSLETTER_ADMIN_INVITE_TOS_ID: AbProp = AbProp {
        name: "newsletter_admin_invite_tos_id",
        code: 6498,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610101"),
    };
    pub const NEWSLETTER_ADMIN_INVITE_TOS_ID_SMB_WEB: AbProp = AbProp {
        name: "newsletter_admin_invite_tos_id_smb_web",
        code: 6536,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610104"),
    };
    pub const NEWSLETTER_CREATION_NUX_ID: AbProp = AbProp {
        name: "newsletter_creation_nux_id",
        code: 3835,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20601218"),
    };
    pub const NEWSLETTER_CREATION_TOS_ID: AbProp = AbProp {
        name: "newsletter_creation_tos_id",
        code: 3834,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20601217"),
    };
    pub const NEWSLETTER_CREATION_TOS_ID_SMB_WEB: AbProp = AbProp {
        name: "newsletter_creation_tos_id_smb_web",
        code: 5598,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20601217"),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_BUMP_FORWARDS_TO_SELF: AbProp = AbProp {
        name: "newsletter_forward_counter_bump_forwards_to_self",
        code: 22204,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_BUMP_OWN_CHANNEL_UPDATES_FOWARDS: AbProp = AbProp {
        name: "newsletter_forward_counter_bump_own_channel_updates_fowards",
        code: 22203,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_BUMP_SECOND_ORDER_FORWARDS: AbProp = AbProp {
        name: "newsletter_forward_counter_bump_second_order_forwards",
        code: 22205,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_INFRA_ENABLED: AbProp = AbProp {
        name: "newsletter_forward_counter_infra_enabled",
        code: 19889,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_MAX_SEND_AFTER_RANDOM_TIME: AbProp = AbProp {
        name: "newsletter_forward_counter_max_send_after_random_time",
        code: 22206,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3600),
    };
    pub const NEWSLETTER_FORWARD_COUNTER_UI_ENABLED: AbProp = AbProp {
        name: "newsletter_forward_counter_ui_enabled",
        code: 19888,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const NEWSLETTER_NUX_NOTICE_ID: AbProp = AbProp {
        name: "newsletter_nux_notice_id",
        code: 15255,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610210"),
    };
    pub const NEWSLETTER_RCAT_FIELD_GENERATING_ENABLED: AbProp = AbProp {
        name: "newsletter_rcat_field_generating_enabled",
        code: 19303,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEWSLETTER_STATUS_CREATION_ENABLED: AbProp = AbProp {
        name: "newsletter_status_creation_enabled",
        code: 26669,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEWSLETTER_STATUS_RECEIVER_ENABLED: AbProp = AbProp {
        name: "newsletter_status_receiver_enabled",
        code: 26217,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NEWSLETTER_TOS_NOTICE_ID: AbProp = AbProp {
        name: "newsletter_tos_notice_id",
        code: 3810,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20601216"),
    };
    pub const NEWSLETTER_TOS_NOTICE_ID_SMB_WEB: AbProp = AbProp {
        name: "newsletter_tos_notice_id_smb_web",
        code: 5597,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20601216"),
    };
    pub const NEWSLETTERS_VIDEO_PLAYBACK_WABBA_LOGGING_ENABLED: AbProp = AbProp {
        name: "newsletters_video_playback_wabba_logging_enabled",
        code: 13954,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NO_LARGE_EMOJI_REGEX: AbProp = AbProp {
        name: "no_large_emoji_regex",
        code: 29172,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const NOISE_PQ_MODE: AbProp = AbProp {
        name: "noise_pq_mode",
        code: 20161,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const NON_WA_CONTACT_INVITE_CTA_ENABLED: AbProp = AbProp {
        name: "non_wa_contact_invite_cta_enabled",
        code: 27217,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const NOTIFICATION_HIGHLIGHT_GROUP_SIZE_THRESHOLD: AbProp = AbProp {
        name: "notification_highlight_group_size_threshold",
        code: 11891,
        value_type: AbPropType::Int,
        default: AbDefault::Int(130),
    };
    pub const NUM_DAYS_BEFORE_DEVICE_EXPIRY_CHECK: AbProp = AbProp {
        name: "num_days_before_device_expiry_check",
        code: 731,
        value_type: AbPropType::Int,
        default: AbDefault::Int(7),
    };
    pub const NUM_DAYS_KEY_INDEX_LIST_EXPIRATION: AbProp = AbProp {
        name: "num_days_key_index_list_expiration",
        code: 730,
        value_type: AbPropType::Int,
        default: AbDefault::Int(35),
    };
    pub const OHAI_REQUEST_KB_SIZE: AbProp = AbProp {
        name: "ohai_request_kb_size",
        code: 12248,
        value_type: AbPropType::Float,
        default: AbDefault::Float(20.0),
    };
    pub const OPTIMIZED_DELIVERY_BLOCK_AND_REPORT_ENTRY_POINTS_ALLOWLIST_WEB: AbProp = AbProp {
        name: "optimized_delivery_block_and_report_entry_points_allowlist_web",
        code: 18736,
        value_type: AbPropType::Str,
        default: AbDefault::Str("4,10,12,13,14,15,17,18,24,31,32,33,34,35,36,39,40,45"),
    };
    pub const OPTIMIZED_DELIVERY_MULTIPLE_COLLECTION_WINDOWS_ENABLED: AbProp = AbProp {
        name: "optimized_delivery_multiple_collection_windows_enabled",
        code: 14588,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_CONFIG: AbProp = AbProp {
        name: "optimized_delivery_signal_collection_config",
        code: 10302,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_ENABLED: AbProp = AbProp {
        name: "optimized_delivery_signal_collection_enabled",
        code: 9348,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_ON_COMPANIONS_ENABLED: AbProp = AbProp {
        name: "optimized_delivery_signal_collection_on_companions_enabled",
        code: 15884,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OPTIMIZED_DELIVERY_TOKENS_STORAGE_CONFIG: AbProp = AbProp {
        name: "optimized_delivery_tokens_storage_config",
        code: 10303,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const OPUS_ADMIN: AbProp = AbProp {
        name: "opus_admin",
        code: 30454,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OPUS_ENABLED: AbProp = AbProp {
        name: "opus_enabled",
        code: 27278,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OPUS_T: AbProp = AbProp {
        name: "opus_t",
        code: 27803,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2147483647),
    };
    pub const OPUS_TIME: AbProp = AbProp {
        name: "opus_time",
        code: 27277,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1784516400),
    };
    pub const ORDER_DETAILS_CUSTOM_ITEM_ENABLED: AbProp = AbProp {
        name: "order_details_custom_item_enabled",
        code: 1176,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_DETAILS_FROM_CART_ENABLED: AbProp = AbProp {
        name: "order_details_from_cart_enabled",
        code: 1107,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_DETAILS_FROM_CATALOG_ENABLED: AbProp = AbProp {
        name: "order_details_from_catalog_enabled",
        code: 1212,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_DETAILS_PAYMENT_INSTRUCTIONS_SYNC_ENABLED: AbProp = AbProp {
        name: "order_details_payment_instructions_sync_enabled",
        code: 6670,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_DETAILS_QUICK_PAY: AbProp = AbProp {
        name: "order_details_quick_pay",
        code: 1600,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"allowed_product_type\":\"none\"}"),
    };
    pub const ORDER_DETAILS_TOTAL_MAXIMUM_VALUE: AbProp = AbProp {
        name: "order_details_total_maximum_value",
        code: 1684,
        value_type: AbPropType::Float,
        default: AbDefault::Float(500000000.0),
    };
    pub const ORDER_DETAILS_TOTAL_ORDER_MINIMUM_VALUE: AbProp = AbProp {
        name: "order_details_total_order_minimum_value",
        code: 1719,
        value_type: AbPropType::Float,
        default: AbDefault::Float(1.0),
    };
    pub const ORDER_MANAGEMENT_ENABLED: AbProp = AbProp {
        name: "order_management_enabled",
        code: 1188,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_MESSAGES_EPHEMERAL_EXCEPTION_ENABLED: AbProp = AbProp {
        name: "order_messages_ephemeral_exception_enabled",
        code: 3240,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDER_STATUSES_REVAMP_M1_ENABLED: AbProp = AbProp {
        name: "order_statuses_revamp_m1_enabled",
        code: 5770,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const ORDERS_EXPANSION_RECEIVER_COUNTRIES_ALLOWED: AbProp = AbProp {
        name: "orders_expansion_receiver_countries_allowed",
        code: 3690,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const ORIGINAL_QUALITY_IMAGE_MIN_EDGE: AbProp = AbProp {
        name: "original_quality_image_min_edge",
        code: 3068,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2560),
    };
    pub const OTP_LID_MIGRATION_ENABLED: AbProp = AbProp {
        name: "otp_lid_migration_enabled",
        code: 12553,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const OUT_CONTACT_INVITES_ENABLED: AbProp = AbProp {
        name: "out_contact_invites_enabled",
        code: 28170,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const OUT_OF_SYNC_DISAPPEARING_MESSAGES_LOGGING: AbProp = AbProp {
        name: "out_of_sync_disappearing_messages_logging",
        code: 2561,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2B_CALLING_AVAILABILITY_EXPERIMENT_ENABLED: AbProp = AbProp {
        name: "p2b_calling_availability_experiment_enabled",
        code: 31098,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2M_EXTERNAL_PAYMENTS_LINK_ENABLED: AbProp = AbProp {
        name: "p2m_external_payments_link_enabled",
        code: 4295,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2P_PILLS_ALLOWLIST: AbProp = AbProp {
        name: "p2p_pills_allowlist",
        code: 29554,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "[{ \"business_id\": \"34666845417\", \"pills\": [\"CHAT\", \"PROFILE\", \"BOOK_APPOINTMENT\", \"CATALOG\", \"BESTSELLERS\", \"OFFERS\", \"ABOUT_US\"] }]",
        ),
    };
    pub const P2P_PILLS_ALLOWLIST_ENTRIES: AbProp = AbProp {
        name: "p2p_pills_allowlist_entries",
        code: 29708,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "{ \"entries\": [{ \"business_id\": \"34666845417\", \"pills\": [\"CHAT\", \"PROFILE\", \"ABOUT_US\"] }]}",
        ),
    };
    pub const P2P_PILLS_AUTO_SEND_MESSAGES: AbProp = AbProp {
        name: "p2p_pills_auto_send_messages",
        code: 30208,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2P_PILLS_ENABLED: AbProp = AbProp {
        name: "p2p_pills_enabled",
        code: 27959,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2P_PILLS_ENABLED_FOR_INELIGIBLE_CONTACTS: AbProp = AbProp {
        name: "p2p_pills_enabled_for_ineligible_contacts",
        code: 29715,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2P_PILLS_ENTRIES: AbProp = AbProp {
        name: "p2p_pills_entries",
        code: 31469,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "{\"enabled_for\": {\"sender\": true,\"receiver\": true},\"enabled_on\": {\"contact_card\": true,\"p2p_link\": true,\"phone_number\": true,\"username\": true}}",
        ),
    };
    pub const P2P_PILLS_ENTRIES_ENABLED: AbProp = AbProp {
        name: "p2p_pills_entries_enabled",
        code: 31471,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "{\"enabled_for\": {\"sender\": true,\"receiver\": true},\"enabled_on\": {\"contact_card\": true,\"p2p_link\": true,\"phone_number\": true,\"username\": true}}",
        ),
    };
    pub const P2P_PILLS_GRAPHQL_ENABLED: AbProp = AbProp {
        name: "p2p_pills_graphql_enabled",
        code: 30629,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const P2P_PILLS_MAX_WAIT_ON_CONTACT_CARD_SEND: AbProp = AbProp {
        name: "p2p_pills_max_wait_on_contact_card_send",
        code: 30943,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const P2P_PILLS_NEW_BUSINESS_METADATA_ENABLED: AbProp = AbProp {
        name: "p2p_pills_new_business_metadata_enabled",
        code: 30578,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAA_SUPPORT_FOR_DISABLED_EPEHEMERALITY: AbProp = AbProp {
        name: "paa_support_for_disabled_epehemerality",
        code: 21235,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_ADMINS_LIMIT: AbProp = AbProp {
        name: "parent_group_admins_limit",
        code: 1655,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20),
    };
    pub const PARENT_GROUP_ALLOW_MEMBER_SUGGEST_EXISTING_M3_RECEIVER: AbProp = AbProp {
        name: "parent_group_allow_member_suggest_existing_m3_receiver",
        code: 5078,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_ALLOW_MEMBER_SUGGEST_EXISTING_M3_SENDER: AbProp = AbProp {
        name: "parent_group_allow_member_suggest_existing_m3_sender",
        code: 5077,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_ANNOUNCEMENT_COMMENTS_HISTORY_SYNC_RECEIVER_ENABLED: AbProp = AbProp {
        name: "parent_group_announcement_comments_history_sync_receiver_enabled",
        code: 5813,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_CREATE_PRIVACY: AbProp = AbProp {
        name: "parent_group_create_privacy",
        code: 2356,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_LINK_LIMIT: AbProp = AbProp {
        name: "parent_group_link_limit",
        code: 1238,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const PARENT_GROUP_LINK_LIMIT_COMMUNITY_CREATION: AbProp = AbProp {
        name: "parent_group_link_limit_community_creation",
        code: 1990,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const PARENT_GROUP_MIN_PARTICIPANTS_FOR_GROUP_ENTRY_POINT: AbProp = AbProp {
        name: "parent_group_min_participants_for_group_entry_point",
        code: 2382,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20),
    };
    pub const PARENT_GROUP_SUBGROUP_FILTER: AbProp = AbProp {
        name: "parent_group_subgroup_filter",
        code: 3147,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARENT_GROUP_VIEW_ENABLED: AbProp = AbProp {
        name: "parent_group_view_enabled",
        code: 982,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PARENT_GROUP_VIEW_ENABLED_FOR_SMB_ON_WEB: AbProp = AbProp {
        name: "parent_group_view_enabled_for_smb_on_web",
        code: 2205,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PARSE_ENCRYPTED_DSM_MSG_FIX: AbProp = AbProp {
        name: "parse_encrypted_dsm_msg_fix",
        code: 26772,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_BR_HOLDOUT: AbProp = AbProp {
        name: "payment_br_holdout",
        code: 14358,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_LINK_TRACE_ID_LOGGING_ENABLED: AbProp = AbProp {
        name: "payment_link_trace_id_logging_enabled",
        code: 19440,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_LINKS_TRUST_SIGNALS_METATAG_ENABLED: AbProp = AbProp {
        name: "payment_links_trust_signals_metatag_enabled",
        code: 16866,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_LINKS_TRUST_SIGNALS_METATAG_PSP_LIST: AbProp = AbProp {
        name: "payment_links_trust_signals_metatag_psp_list",
        code: 17162,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"psp\":[\"mercadopago\"]} "),
    };
    pub const PAYMENT_LINKS_TRUST_SIGNALS_OTHER_METATAG_KILL_SWITCH_ENABLED: AbProp = AbProp {
        name: "payment_links_trust_signals_other_metatag_kill_switch_enabled",
        code: 24662,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_LINKS_TRUST_SIGNALS_OTHER_METATAGS_ENABLED: AbProp = AbProp {
        name: "payment_links_trust_signals_other_metatags_enabled",
        code: 17355,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENT_SUPPORT_LIDS: AbProp = AbProp {
        name: "payment_support_lids",
        code: 14333,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "116664750354676,128385682505839,46635358933114,26521959944357,200206125658243,179985503506636,187797998674170,228746200088715,117914552262794,10158134550607",
        ),
    };
    pub const PAYMENTS_BR_CONTENT_OPTIMIZATION_VARIANT: AbProp = AbProp {
        name: "payments_br_content_optimization_variant",
        code: 4248,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const PAYMENTS_BR_COPY_PIX_CODE_API_MERCHANT_ENABLED: AbProp = AbProp {
        name: "payments_br_copy_pix_code_api_merchant_enabled",
        code: 9017,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_FORCE_COPY_PIX_CTA_ENABLED: AbProp = AbProp {
        name: "payments_br_force_copy_pix_cta_enabled",
        code: 8953,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_MERCHANT_PSP_ACCOUNT_STATUS_SYNC: AbProp = AbProp {
        name: "payments_br_merchant_psp_account_status_sync",
        code: 9076,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_BOLETO_ENABLED: AbProp = AbProp {
        name: "payments_br_p2m_boleto_enabled",
        code: 11671,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_BUYER_LOGGING_PHASE_2: AbProp = AbProp {
        name: "payments_br_p2m_buyer_logging_phase_2",
        code: 29803,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_COMPLETED_PAYMENT_INTENT_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_completed_payment_intent_buyer_logging",
        code: 27095,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_COPY_BOLETO_CODE_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_copy_boleto_code_buyer_logging",
        code: 27096,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_ORDER_DETAILS_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_order_details_buyer_logging",
        code: 27008,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_PAY_NOW_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_pay_now_buyer_logging",
        code: 27092,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_PIX_COPY_CODE_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_pix_copy_code_buyer_logging",
        code: 27028,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_PIX_COPY_KEY_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_pix_copy_key_buyer_logging",
        code: 27026,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_PIX_IN_GROUPS_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_pix_in_groups_buyer_logging",
        code: 27029,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_PIX_MORE_WAYS_TO_PAY_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_pix_more_ways_to_pay_buyer_logging",
        code: 27094,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2M_VIEW_ORDER_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2m_view_order_buyer_logging",
        code: 27093,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2P_PIX_COPY_CODE_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2p_pix_copy_code_buyer_logging",
        code: 27114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_P2P_PIX_COPY_KEY_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_p2p_pix_copy_key_buyer_logging",
        code: 26847,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_PAYMENT_LINKS_BUYER_LOGGING: AbProp = AbProp {
        name: "payments_br_payment_links_buyer_logging",
        code: 27027,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_PIX_ON_WEB: AbProp = AbProp {
        name: "payments_br_pix_on_web",
        code: 16156,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_PIX_PHASE_1_SELLER_SYNC_ENABLED: AbProp = AbProp {
        name: "payments_br_pix_phase_1_seller_sync_enabled",
        code: 7024,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_PIX_QUICK_REPLY_ENABLED: AbProp = AbProp {
        name: "payments_br_pix_quick_reply_enabled",
        code: 7857,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_BR_PIX_WEB_ATTACHMENT_TRAY: AbProp = AbProp {
        name: "payments_br_pix_web_attachment_tray",
        code: 19276,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_LINK_TO_LITE_CONSUMER_ENABLED: AbProp = AbProp {
        name: "payments_link_to_lite_consumer_enabled",
        code: 3051,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_MERCHANT_GLOBAL_ORDERS_VALUE_PROPS_BANNER_ENABLED: AbProp = AbProp {
        name: "payments_merchant_global_orders_value_props_banner_enabled",
        code: 3744,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_BUBBLE_COUNTRIES: AbProp = AbProp {
        name: "payments_upr_bubble_countries",
        code: 29342,
        value_type: AbPropType::Str,
        default: AbDefault::Str("MX"),
    };
    pub const PAYMENTS_UPR_CUSTOM_PAYMENT_METHODS_SYNC_COUNTRIES: AbProp = AbProp {
        name: "payments_upr_custom_payment_methods_sync_countries",
        code: 30647,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const PAYMENTS_UPR_EGYPT_ENABLED: AbProp = AbProp {
        name: "payments_upr_egypt_enabled",
        code: 31870,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_HONGKONG_ENABLED: AbProp = AbProp {
        name: "payments_upr_hongkong_enabled",
        code: 31868,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_ID_ENABLED: AbProp = AbProp {
        name: "payments_upr_id_enabled",
        code: 32170,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_MEXICO_WALLET_ENABLED: AbProp = AbProp {
        name: "payments_upr_mexico_wallet_enabled",
        code: 32043,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_MULTIPLE_KEY_COPY_ENABLED: AbProp = AbProp {
        name: "payments_upr_multiple_key_copy_enabled",
        code: 32124,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_MX_ENABLED: AbProp = AbProp {
        name: "payments_upr_mx_enabled",
        code: 32169,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_SEND_KEY_FROM_WEB: AbProp = AbProp {
        name: "payments_upr_send_key_from_web",
        code: 32826,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const PAYMENTS_UPR_TAIWAN_ENABLED: AbProp = AbProp {
        name: "payments_upr_taiwan_enabled",
        code: 31869,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_TURKEY_ENABLED: AbProp = AbProp {
        name: "payments_upr_turkey_enabled",
        code: 31848,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PAYMENTS_UPR_UAE_ENABLED: AbProp = AbProp {
        name: "payments_upr_uae_enabled",
        code: 31860,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PEER_MESSAGE_LID_MIGRATION_OUTGOING: AbProp = AbProp {
        name: "peer_message_lid_migration_outgoing",
        code: 24184,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PENDING_GROUP_REQUESTS_PERSISTENT_BANNER: AbProp = AbProp {
        name: "pending_group_requests_persistent_banner",
        code: 20545,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PER_CUSTOMER_DATA_SHARING_CONTROLS_ELIGIBLE: AbProp = AbProp {
        name: "per_customer_data_sharing_controls_eligible",
        code: 13383,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PHONE_NUMBER_SHARING_FLOW: AbProp = AbProp {
        name: "phone_number_sharing_flow",
        code: 15653,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PINNED_MESSAGES_INFINITE_RECEIVER_ENABLED: AbProp = AbProp {
        name: "pinned_messages_infinite_receiver_enabled",
        code: 31886,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PINNED_MESSAGES_INFINITE_SENDER_ENABLED: AbProp = AbProp {
        name: "pinned_messages_infinite_sender_enabled",
        code: 31887,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PINNED_MESSAGES_M0: AbProp = AbProp {
        name: "pinned_messages_m0",
        code: 3138,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PINNED_MESSAGES_M1_RECEIVER: AbProp = AbProp {
        name: "pinned_messages_m1_receiver",
        code: 3139,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PINNED_MESSAGES_M1_SENDER: AbProp = AbProp {
        name: "pinned_messages_m1_sender",
        code: 3140,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PINNED_MESSAGES_M2: AbProp = AbProp {
        name: "pinned_messages_m2",
        code: 3141,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PINNED_MESSAGES_M2_IMAGE_THUMBNAIL: AbProp = AbProp {
        name: "pinned_messages_m2_image_thumbnail",
        code: 7467,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PINNED_MESSAGES_M2_PIN_MAX: AbProp = AbProp {
        name: "pinned_messages_m2_pin_max",
        code: 3732,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const PINNED_MESSAGES_SENDER_SHORT_EXPIRY_DURATIONS_ENABLED: AbProp = AbProp {
        name: "pinned_messages_sender_short_expiry_durations_enabled",
        code: 4432,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PIX_ONBOARDING_NEW_CONTENT_ENABLED: AbProp = AbProp {
        name: "pix_onboarding_new_content_enabled",
        code: 23953,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PIX_PAYMENT_REQUEST_UPDATE_STATUS_ENABLED: AbProp = AbProp {
        name: "pix_payment_request_update_status_enabled",
        code: 27006,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PLACEHOLDER_MESSAGE_KEY_HASH_LOGGING: AbProp = AbProp {
        name: "placeholder_message_key_hash_logging",
        code: 2639,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PLACEHOLDER_MESSAGE_RESEND: AbProp = AbProp {
        name: "placeholder_message_resend",
        code: 3579,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PLACEHOLDER_MESSAGE_RESEND_MAXIMUM_DAYS_LIMIT: AbProp = AbProp {
        name: "placeholder_message_resend_maximum_days_limit",
        code: 3639,
        value_type: AbPropType::Int,
        default: AbDefault::Int(14),
    };
    pub const PNH_CAG_DISABLE_POLLS_GROUP_SIZE: AbProp = AbProp {
        name: "pnh_cag_disable_polls_group_size",
        code: 5056,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10000),
    };
    pub const PNH_CAG_DISABLE_REACTIONS_GROUP_SIZE: AbProp = AbProp {
        name: "pnh_cag_disable_reactions_group_size",
        code: 4495,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10000),
    };
    pub const PNH_HISTORY_SYNC_FORCE_GENERAL: AbProp = AbProp {
        name: "pnh_history_sync_force_general",
        code: 28664,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PNH_PN_FOR_LID_CHAT_SYNC: AbProp = AbProp {
        name: "pnh_pn_for_lid_chat_sync",
        code: 3062,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PNH_THREAD_PROMOTION_TO_GENERAL_LID: AbProp = AbProp {
        name: "pnh_thread_promotion_to_general_lid",
        code: 16632,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_ADD_OPTION_ENABLED: AbProp = AbProp {
        name: "poll_add_option_enabled",
        code: 24517,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_ADD_OPTION_RECEIVING_ENABLED: AbProp = AbProp {
        name: "poll_add_option_receiving_enabled",
        code: 25758,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const POLL_CREATION_CAG_ENABLED: AbProp = AbProp {
        name: "poll_creation_cag_enabled",
        code: 2738,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_CREATOR_EDIT_ENABLED: AbProp = AbProp {
        name: "poll_creator_edit_enabled",
        code: 24887,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_CREATOR_EDIT_RECEIVING_VERSION: AbProp = AbProp {
        name: "poll_creator_edit_receiving_version",
        code: 24886,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const POLL_END_TIME_ENABLED: AbProp = AbProp {
        name: "poll_end_time_enabled",
        code: 24405,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_END_TIME_RECEIVING_ENABLED: AbProp = AbProp {
        name: "poll_end_time_receiving_enabled",
        code: 24884,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_HIDE_VOTERS_ENABLED: AbProp = AbProp {
        name: "poll_hide_voters_enabled",
        code: 24518,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_HIDE_VOTERS_RECEIVING_ENABLED: AbProp = AbProp {
        name: "poll_hide_voters_receiving_enabled",
        code: 24885,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const POLL_NAME_LENGTH: AbProp = AbProp {
        name: "poll_name_length",
        code: 1406,
        value_type: AbPropType::Int,
        default: AbDefault::Int(255),
    };
    pub const POLL_OPTION_COUNT: AbProp = AbProp {
        name: "poll_option_count",
        code: 1408,
        value_type: AbPropType::Int,
        default: AbDefault::Int(12),
    };
    pub const POLL_OPTION_LENGTH: AbProp = AbProp {
        name: "poll_option_length",
        code: 1407,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const POLL_RECEIVING_CAG_ENABLED: AbProp = AbProp {
        name: "poll_receiving_cag_enabled",
        code: 2737,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_RESULT_SNAPSHOT_POLLTYPE_ENVELOPE_ENABLED: AbProp = AbProp {
        name: "poll_result_snapshot_polltype_envelope_enabled",
        code: 12258,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_TC_RECEIVING_ENABLED: AbProp = AbProp {
        name: "poll_tc_receiving_enabled",
        code: 31592,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_TC_SENDING_ENABLED: AbProp = AbProp {
        name: "poll_tc_sending_enabled",
        code: 31593,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PREMIUM_BLUE_ENABLED: AbProp = AbProp {
        name: "premium_blue_enabled",
        code: 5318,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PREMIUM_BROADCAST_SMB_CAPPING_ENABLED: AbProp = AbProp {
        name: "premium_broadcast_smb_capping_enabled",
        code: 13808,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PREMIUM_MSG_BB_CAMPAIGN_SYNC_ENABLED: AbProp = AbProp {
        name: "premium_msg_bb_campaign_sync_enabled",
        code: 29650,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_SCREEN_ENABLED: AbProp = AbProp {
        name: "privacy_screen_enabled",
        code: 26820,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_SETTINGS_ABOUT_LID_MIGRATION_ENABLE: AbProp = AbProp {
        name: "privacy_settings_about_lid_migration_enable",
        code: 16195,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_SETTINGS_GROUP_ADD_LID_MIGRATION_ENABLE: AbProp = AbProp {
        name: "privacy_settings_group_add_lid_migration_enable",
        code: 16274,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_SETTINGS_PRESENCE_LID_MIGRATION_ENABLE: AbProp = AbProp {
        name: "privacy_settings_presence_lid_migration_enable",
        code: 16275,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_SETTINGS_PROFILE_LID_MIGRATION_ENABLE: AbProp = AbProp {
        name: "privacy_settings_profile_lid_migration_enable",
        code: 16161,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TIPS_GROUPS_BUILD: AbProp = AbProp {
        name: "privacy_tips_groups_build",
        code: 3995,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TIPS_KILLSWITCH: AbProp = AbProp {
        name: "privacy_tips_killswitch",
        code: 4314,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TIPS_PROFILE_BUILD: AbProp = AbProp {
        name: "privacy_tips_profile_build",
        code: 3998,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES: AbProp = AbProp {
        name: "privacy_token_sending_on_all_1_on_1_messages",
        code: 10518,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TOKEN_SENDING_ON_GROUP_CREATE: AbProp = AbProp {
        name: "privacy_token_sending_on_group_create",
        code: 11261,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVACY_TOKEN_SENDING_ON_GROUP_PARTICIPANT_ADD: AbProp = AbProp {
        name: "privacy_token_sending_on_group_participant_add",
        code: 11262,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVATE_MESSAGING_UK_OSA_ENABLED: AbProp = AbProp {
        name: "private_messaging_uk_osa_enabled",
        code: 14250,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PRIVATE_OSA_REPORTING_ENABLED: AbProp = AbProp {
        name: "private_osa_reporting_enabled",
        code: 12990,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PROFILE_PICTURE_DEEPLINK_ENABLED: AbProp = AbProp {
        name: "profile_picture_deeplink_enabled",
        code: 7634,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_IQ: AbProp = AbProp {
        name: "profile_scraping_privacy_token_in_about_iq",
        code: 9668,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_USYNC: AbProp = AbProp {
        name: "profile_scraping_privacy_token_in_about_usync",
        code: 20798,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PTT_USER_JOURNEY_LOGGING_WAM_ENABLED: AbProp = AbProp {
        name: "ptt_user_journey_logging_wam_enabled",
        code: 8630,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PTV_AUTOPLAY_ENABLED: AbProp = AbProp {
        name: "ptv_autoplay_enabled",
        code: 3482,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const PTV_AUTOPLAY_LOOP_LIMIT: AbProp = AbProp {
        name: "ptv_autoplay_loop_limit",
        code: 3483,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const PTV_MAX_DURATION_SECONDS: AbProp = AbProp {
        name: "ptv_max_duration_seconds",
        code: 3356,
        value_type: AbPropType::Int,
        default: AbDefault::Int(60),
    };
    pub const PTV_QUOTED_REPLIES_CUTOUT_ENABLED: AbProp = AbProp {
        name: "ptv_quoted_replies_cutout_enabled",
        code: 30384,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PUBLIC_BUG_REPORTING_SIDEBAR: AbProp = AbProp {
        name: "public_bug_reporting_sidebar",
        code: 19124,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const PUSHNAME_BLOCKLIST_STARTING_WITH_AT: AbProp = AbProp {
        name: "pushname_blocklist_starting_with_at",
        code: 18097,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const QP_BANNER_STICKER_ANIMATION_ENABLED: AbProp = AbProp {
        name: "qp_banner_sticker_animation_enabled",
        code: 31213,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const QP_CAMPAIGN_CLIENT_ENABLED: AbProp = AbProp {
        name: "qp_campaign_client_enabled",
        code: 3536,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const QUICK_PROMOTION_BANNER_CLIENT_ENABLED: AbProp = AbProp {
        name: "quick_promotion_banner_client_enabled",
        code: 3712,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const QUOTED_MESSAGE_USER_JOURNEY_LOGGING_ENABLED: AbProp = AbProp {
        name: "quoted_message_user_journey_logging_enabled",
        code: 15694,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RASTERIZE_TEXT_STATUS_PIXEL_WIDTH: AbProp = AbProp {
        name: "rasterize_text_status_pixel_width",
        code: 13460,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1080),
    };
    pub const REACTION_USER_JOURNEY_LOGGING_ENABLED: AbProp = AbProp {
        name: "reaction_user_journey_logging_enabled",
        code: 10438,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REACTIONS_ALIGNMENT_FOR_TRANSPARENT_MESSAGES_ENABLED: AbProp = AbProp {
        name: "reactions_alignment_for_transparent_messages_enabled",
        code: 16792,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REACTIONS_CHAT_PREVIEW: AbProp = AbProp {
        name: "reactions_chat_preview",
        code: 1605,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REACTIONS_RECEIVER_ENABLED: AbProp = AbProp {
        name: "reactions_receiver_enabled",
        code: 13542,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RECEIPT_MODE_BITMASK_ENABLED: AbProp = AbProp {
        name: "receipt_mode_bitmask_enabled",
        code: 30084,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RECOMMENDED_CHANNELS_BACKGROUND_REFRESH: AbProp = AbProp {
        name: "recommended_channels_background_refresh",
        code: 4309,
        value_type: AbPropType::Int,
        default: AbDefault::Int(14400000),
    };
    pub const RELAX_INTEGRITY_CONSTRAINTS_FOR_BB_WA_TENURED_ACCOUNTS: AbProp = AbProp {
        name: "relax_integrity_constraints_for_bb_wa_tenured_accounts",
        code: 28516,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REMOVE_DEVICE_PN_DEPENDENCIES: AbProp = AbProp {
        name: "remove_device_pn_dependencies",
        code: 27791,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REMOVE_PN_DEPENDENCIES: AbProp = AbProp {
        name: "remove_pn_dependencies",
        code: 26888,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RENDER_UPDATED_DISCLOSURE: AbProp = AbProp {
        name: "render_updated_disclosure",
        code: 14407,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REPORT_BLOCK_IMPROVEMENTS_FOR_GROUPS_ENABLED: AbProp = AbProp {
        name: "report_block_improvements_for_groups_enabled",
        code: 8327,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REPORT_CALL_REPLAYER_ID: AbProp = AbProp {
        name: "report_call_replayer_id",
        code: 1834,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REPORT_TO_ADMIN_ENABLED: AbProp = AbProp {
        name: "report_to_admin_enabled",
        code: 3696,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REPORT_TO_ADMIN_KILL_SWITCH: AbProp = AbProp {
        name: "report_to_admin_kill_switch",
        code: 3695,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const REUSE_CACHED_CERTS_FOR_DATA_CHANNEL: AbProp = AbProp {
        name: "reuse_cached_certs_for_data_channel",
        code: 12913,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RICH_ORDER_STATUS_WA_WEB: AbProp = AbProp {
        name: "rich_order_status_wa_web",
        code: 16534,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RNR_DAYS_COOLDOWN: AbProp = AbProp {
        name: "rnr_days_cooldown",
        code: 18703,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100000),
    };
    pub const RNR_MIN_DAYS_USER_ACTIVE: AbProp = AbProp {
        name: "rnr_min_days_user_active",
        code: 18702,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const ROW_BUYER_ORDER_REVAMP_M0_ENABLED: AbProp = AbProp {
        name: "row_buyer_order_revamp_m0_enabled",
        code: 4893,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RT_CLEAN_REPORTING_TAG: AbProp = AbProp {
        name: "rt_clean_reporting_tag",
        code: 6723,
        value_type: AbPropType::Int,
        default: AbDefault::Int(31),
    };
    pub const RT_CLEAN_REPORTING_TOKEN: AbProp = AbProp {
        name: "rt_clean_reporting_token",
        code: 9567,
        value_type: AbPropType::Int,
        default: AbDefault::Int(31),
    };
    pub const RT_EDIT_RECEIVE: AbProp = AbProp {
        name: "rt_edit_receive",
        code: 15016,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_GHS_RECEIVER_ENABLED: AbProp = AbProp {
        name: "rt_ghs_receiver_enabled",
        code: 24742,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RT_GHS_SENDER_ENABLED: AbProp = AbProp {
        name: "rt_ghs_sender_enabled",
        code: 24741,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RT_RECEIVE_REPORTING_TAG: AbProp = AbProp {
        name: "rt_receive_reporting_tag",
        code: 5718,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_RECEIVER_DUAL_ENCRYPTED_MSG_ENABLED: AbProp = AbProp {
        name: "rt_receiver_dual_encrypted_msg_enabled",
        code: 15258,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_REPORT_TOKEN_FROM_INCLUSION_LIST: AbProp = AbProp {
        name: "rt_report_token_from_inclusion_list",
        code: 9818,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RT_SENDER_DUAL_ENCRYPTED_MSG_ENABLED: AbProp = AbProp {
        name: "rt_sender_dual_encrypted_msg_enabled",
        code: 12623,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_SENDER_REPORTING_TOKEN_VERSION: AbProp = AbProp {
        name: "rt_sender_reporting_token_version",
        code: 8860,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const RT_SWAPPED_FALLBACK_VALIDATION: AbProp = AbProp {
        name: "rt_swapped_fallback_validation",
        code: 21718,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_SYNC_REPORTING_TAG: AbProp = AbProp {
        name: "rt_sync_reporting_tag",
        code: 6578,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const RT_WEB_DELAY_PROCESSING: AbProp = AbProp {
        name: "rt_web_delay_processing",
        code: 15181,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SAGA_COPY: AbProp = AbProp {
        name: "saga_copy",
        code: 7044,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SAGA_ENABLED: AbProp = AbProp {
        name: "saga_enabled",
        code: 5626,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SAGA_MESSAGE_FEEDBACK_USING_CANONICAL_ENT: AbProp = AbProp {
        name: "saga_message_feedback_using_canonical_ent",
        code: 23328,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SAGA_PROTOBUF_AI_STARDUST_WEB: AbProp = AbProp {
        name: "saga_protobuf_ai_stardust_web",
        code: 11756,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SAGA_PROTOBUF_SHOW_SYSMSG_WEB: AbProp = AbProp {
        name: "saga_protobuf_show_sysmsg_web",
        code: 11832,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SAGA_V1_CAROUSEL: AbProp = AbProp {
        name: "saga_v1_carousel",
        code: 10609,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SAGA_V1_ENABLED: AbProp = AbProp {
        name: "saga_v1_enabled",
        code: 9942,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SAGA_V1_NUX_ENABLED: AbProp = AbProp {
        name: "saga_v1_nux_enabled",
        code: 9944,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SAGA_V1_REENGAGEMENT_ENABLED: AbProp = AbProp {
        name: "saga_v1_reengagement_enabled",
        code: 9924,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SCHEDULE_CALL_SHOW_JOIN_BUTTON_TIME_INTERVAL_MINS: AbProp = AbProp {
        name: "schedule_call_show_join_button_time_interval_mins",
        code: 16253,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const SCHEDULE_CALL_SHOW_UPCOMING_BANNER_TIME_INTERVAL_MINS: AbProp = AbProp {
        name: "schedule_call_show_upcoming_banner_time_interval_mins",
        code: 16254,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1440),
    };
    pub const SCHEDULED_MESSAGES_PHOTO_VIDEO_SENDER_ENABLED: AbProp = AbProp {
        name: "scheduled_messages_photo_video_sender_enabled",
        code: 32553,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SCHEDULED_MESSAGES_RECEIVER_ENABLED: AbProp = AbProp {
        name: "scheduled_messages_receiver_enabled",
        code: 24610,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SCHEDULED_MESSAGES_SENDER_ENABLED: AbProp = AbProp {
        name: "scheduled_messages_sender_enabled",
        code: 23845,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SCHEDULED_MESSAGES_WINDOW_DURATION_MAX_SECONDS: AbProp = AbProp {
        name: "scheduled_messages_window_duration_max_seconds",
        code: 26347,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1209600),
    };
    pub const SCHEDULED_MESSAGES_WINDOW_DURATION_MIN_SECONDS: AbProp = AbProp {
        name: "scheduled_messages_window_duration_min_seconds",
        code: 26348,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600),
    };
    pub const SEARCH_THE_WEB_DESIGN_EXPERIMENT_V1: AbProp = AbProp {
        name: "search_the_web_design_experiment_v1",
        code: 15423,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEARCH_THE_WEB_DIALOG_REDESIGN: AbProp = AbProp {
        name: "search_the_web_dialog_redesign",
        code: 8171,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEARCH_THE_WEB_IMAGE_SEARCH: AbProp = AbProp {
        name: "search_the_web_image_search",
        code: 9547,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEARCH_THE_WEB_TEXT_SEARCH: AbProp = AbProp {
        name: "search_the_web_text_search",
        code: 9548,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEARCH_THE_WEB_URL_OFFER: AbProp = AbProp {
        name: "search_the_web_url_offer",
        code: 8473,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEARCH_USER_JOURNEY_LOGGING_WAM_ENABLED: AbProp = AbProp {
        name: "search_user_journey_logging_wam_enabled",
        code: 14682,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SECURITY_FIXES_BITMAP: AbProp = AbProp {
        name: "security_fixes_bitmap",
        code: 3094,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const SELLER_ORDERS_MANAGEMENT_REVAMP: AbProp = AbProp {
        name: "seller_orders_management_revamp",
        code: 5190,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEND_CAG_MEMBER_REVOKES_AS_GDM: AbProp = AbProp {
        name: "send_cag_member_revokes_as_GDM",
        code: 3069,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SEND_EXTENDED_NACK_ENABLED: AbProp = AbProp {
        name: "send_extended_nack_enabled",
        code: 3280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SEND_INVALID_PROTOBUF_NACK_FAILURE_REASON: AbProp = AbProp {
        name: "send_invalid_protobuf_nack_failure_reason",
        code: 9174,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SERVER_DRIVEN_COPY_M2: AbProp = AbProp {
        name: "server_driven_copy_m2",
        code: 30492,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SERVICE_IMPROVEMENT_OPT_OUT_FLAG: AbProp = AbProp {
        name: "service_improvement_opt_out_flag",
        code: 3664,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SETTINGS_SYNC_ENABLED: AbProp = AbProp {
        name: "settings_sync_enabled",
        code: 22692,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SFU_SECONDARY_REMOTE_BWE_IMPL: AbProp = AbProp {
        name: "sfu_secondary_remote_bwe_impl",
        code: 11472,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const SHARE_OWN_PN_SYNC: AbProp = AbProp {
        name: "share_own_pn_sync",
        code: 3070,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SHARE_PHONE_NUMBER_ON_CART_SEND_TO_DIRECT_CONNECTION_BIZ_ENABLED: AbProp = AbProp {
        name: "share_phone_number_on_cart_send_to_direct_connection_biz_enabled",
        code: 1867,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SHIMMED_LINKS_IN_THE_MARKETING_MESSAGE_BODY_ENABLED: AbProp = AbProp {
        name: "shimmed_links_in_the_marketing_message_body_enabled",
        code: 12995,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_ASSERTION_TIMEOUT_SECONDS: AbProp = AbProp {
        name: "shortcake_companion_prologue__passkeys__assertion_timeout_seconds",
        code: 30661,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600),
    };
    pub const SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_ENABLED: AbProp = AbProp {
        name: "shortcake_companion_prologue__passkeys__enabled",
        code: 29206,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_HANDOFF_ENABLED: AbProp = AbProp {
        name: "shortcake_companion_prologue__passkeys__handoff_enabled",
        code: 29204,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_REQUEST_OPTIONS_TTL_SECONDS: AbProp = AbProp {
        name: "shortcake_companion_prologue__passkeys__request_options_ttl_seconds",
        code: 30662,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600),
    };
    pub const SHOW_INTEGRITY_SCREENSHARING_FRICTION_UI: AbProp = AbProp {
        name: "show_integrity_screensharing_friction_ui",
        code: 16411,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SILENT_GROUP_USERNAME_ACTIVITIES_ENABLED: AbProp = AbProp {
        name: "silent_group_username_activities_enabled",
        code: 24269,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SIMILAR_CHANNELS_IN_CHANNEL_DETAILS_ENABLED: AbProp = AbProp {
        name: "similar_channels_in_channel_details_enabled",
        code: 7473,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SIMILAR_CHANNELS_IN_THREAD_ON_FOLLOW_ENABLED: AbProp = AbProp {
        name: "similar_channels_in_thread_on_follow_enabled",
        code: 7472,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SIMILAR_CHANNELS_MAX_LIMIT: AbProp = AbProp {
        name: "similar_channels_max_limit",
        code: 7559,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const SIMILAR_CHANNELS_MIN_LIMIT: AbProp = AbProp {
        name: "similar_channels_min_limit",
        code: 7560,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const SINGLE_E2EE_SESSION_MIGRATION_STATE_INCOMING: AbProp = AbProp {
        name: "single_e2ee_session_migration_state_incoming",
        code: 7821,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const SINGLE_E2EE_SESSION_MIGRATION_STATE_OUTGOING: AbProp = AbProp {
        name: "single_e2ee_session_migration_state_outgoing",
        code: 7820,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const SINGLE_EMOJI_LOGGING_ENABLED: AbProp = AbProp {
        name: "single_emoji_logging_enabled",
        code: 9669,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMART_FILTERS_ENABLED: AbProp = AbProp {
        name: "smart_filters_enabled",
        code: 1015,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMART_FILTERS_ENABLED_CONSUMER: AbProp = AbProp {
        name: "smart_filters_enabled_consumer",
        code: 1287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_AGENT_CHAT_LIST_INDICATOR_ENABLED: AbProp = AbProp {
        name: "smb_agent_chat_list_indicator_enabled",
        code: 10455,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_AGENT_THREAD_CONTROL_NOTIFICATION_ENABLED: AbProp = AbProp {
        name: "smb_agent_thread_control_notification_enabled",
        code: 10456,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_AI_AGENTS_WEB_CHAT_ASSIGNMENT_INTEROP_ENABLED: AbProp = AbProp {
        name: "smb_ai_agents_web_chat_assignment_interop_enabled",
        code: 13387,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_AUTH_AGENTS_FEATURE_CONTROL_ENABLED: AbProp = AbProp {
        name: "smb_auth_agents_feature_control_enabled",
        code: 27585,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BB_WEB_AUDIENCE_EXPRESSION_SYNC_READ: AbProp = AbProp {
        name: "smb_bb_web_audience_expression_sync_read",
        code: 26894,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SMB_BILLING_ENABLED: AbProp = AbProp {
        name: "smb_billing_enabled",
        code: 1583,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BIZ_AI_LISTS_PILLS: AbProp = AbProp {
        name: "smb_biz_ai_lists_pills",
        code: 28470,
        value_type: AbPropType::Str,
        default: AbDefault::Str("None"),
    };
    pub const SMB_BIZ_PROFILE_CUSTOM_URL: AbProp = AbProp {
        name: "smb_biz_profile_custom_url",
        code: 2582,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SMB_BUSINESS_BROADCAST_IMPORT_CONTACT: AbProp = AbProp {
        name: "smb_business_broadcast_import_contact",
        code: 17433,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_MULTI_AUDIENCE_SEND_WEB: AbProp = AbProp {
        name: "smb_business_broadcast_multi_audience_send_web",
        code: 25206,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_PRO_ENABLED: AbProp = AbProp {
        name: "smb_business_broadcast_pro_enabled",
        code: 29033,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_SEND_WEB: AbProp = AbProp {
        name: "smb_business_broadcast_send_web",
        code: 21508,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_SEND_WEB_NO_EXP: AbProp = AbProp {
        name: "smb_business_broadcast_send_web_no_exp",
        code: 28138,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_SEND_WEB_SMBA: AbProp = AbProp {
        name: "smb_business_broadcast_send_web_smba",
        code: 27486,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_BUSINESS_BROADCAST_SEND_WEB_SMBA_NO_EXP: AbProp = AbProp {
        name: "smb_business_broadcast_send_web_smba_no_exp",
        code: 28139,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CATALOG_GRAPHQL_GET_PUBLIC_KEY: AbProp = AbProp {
        name: "smb_catalog_graphql_get_public_key",
        code: 11690,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CATALOG_GRAPHQL_VERIFY_POSTCODE: AbProp = AbProp {
        name: "smb_catalog_graphql_verify_postcode",
        code: 11624,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CATKIT_QUERY_VERSION: AbProp = AbProp {
        name: "smb_catkit_query_version",
        code: 1229,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const SMB_COLLECTIONS_ENABLED: AbProp = AbProp {
        name: "smb_collections_enabled",
        code: 451,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CORE_BIZ_PROFILE_PREVIEW: AbProp = AbProp {
        name: "smb_core_biz_profile_preview",
        code: 26441,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CORE_BIZ_PROFILE_UX_REFRESHED: AbProp = AbProp {
        name: "smb_core_biz_profile_ux_refreshed",
        code: 19929,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CORE_BIZ_PROFILE_UX_REFRESHED_V2: AbProp = AbProp {
        name: "smb_core_biz_profile_ux_refreshed_v2",
        code: 22561,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CORE_REC_CARD: AbProp = AbProp {
        name: "smb_core_rec_card",
        code: 27568,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CTWA_BILLING_ENABLED: AbProp = AbProp {
        name: "smb_ctwa_billing_enabled",
        code: 2158,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_CTWA_IREV_LONG_TERM_HOLDOUT_DUMMY_ENABLED: AbProp = AbProp {
        name: "smb_ctwa_irev_long_term_holdout_dummy_enabled",
        code: 31959,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_DO_LABEL_LOCALIZE_BACKFILL_ENABLED_CODE: AbProp = AbProp {
        name: "smb_do_label_localize_backfill_enabled_code",
        code: 30352,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_DO_LABEL_LOCALIZE_ON_CREATE_ENABLED_CODE: AbProp = AbProp {
        name: "smb_do_label_localize_on_create_enabled_code",
        code: 30344,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_ECOMMERCE_COMPLIANCE_INDIA_M4: AbProp = AbProp {
        name: "smb_ecommerce_compliance_india_m4",
        code: 1003,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_ECOMMERCE_COMPLIANCE_INDIA_M4_5: AbProp = AbProp {
        name: "smb_ecommerce_compliance_india_m4_5",
        code: 1192,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_GRAPHQL_TO_FETCH_QP_ENABLED: AbProp = AbProp {
        name: "smb_graphql_to_fetch_qp_enabled",
        code: 7645,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_GRAPHQL_TO_FETCH_QP_FREQUENCY_MINS: AbProp = AbProp {
        name: "smb_graphql_to_fetch_qp_frequency_mins",
        code: 7646,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1320),
    };
    pub const SMB_GRAPHQL_TO_FETCH_QP_SURFACE_IDS: AbProp = AbProp {
        name: "smb_graphql_to_fetch_qp_surface_ids",
        code: 7647,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const SMB_GRAPHQL_TOKEN_RECOVERY_DURING_ACCOUNT_RECOVERY_ENABLED: AbProp = AbProp {
        name: "smb_graphql_token_recovery_during_account_recovery_enabled",
        code: 9197,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_HIDE_UNSUPPORTED_CURRENCY_PRICE: AbProp = AbProp {
        name: "smb_hide_unsupported_currency_price",
        code: 1203,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_LABEL_SYNC_CRITICAL_EVENT_LOGGING: AbProp = AbProp {
        name: "smb_label_sync_critical_event_logging",
        code: 24311,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_LABELS_CTWA_DATA_SHARING: AbProp = AbProp {
        name: "smb_labels_ctwa_data_sharing",
        code: 5009,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_CHATS_REORDER_ON_CHAT_ASSIGNMENT_ENABLED: AbProp =
        AbProp {
            name: "smb_md_agent_chat_assignment_chats_reorder_on_chat_assignment_enabled",
            code: 2787,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_CHATS_REORDER_ON_CHAT_UNASSIGNMENT_ENABLED: AbProp =
        AbProp {
            name: "smb_md_agent_chat_assignment_chats_reorder_on_chat_unassignment_enabled",
            code: 2788,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_ENABLED: AbProp = AbProp {
        name: "smb_md_agent_chat_assignment_enabled",
        code: 1798,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_NOTIFICATIONS_ENABLED: AbProp = AbProp {
        name: "smb_md_agent_chat_assignment_notifications_enabled",
        code: 2908,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_NUX_IMPRESSIONS: AbProp = AbProp {
        name: "smb_md_agent_chat_assignment_nux_impressions",
        code: 2207,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const SMB_MD_AGENT_CHAT_ASSIGNMENT_SYSTEM_MESSAGES_LOGGING_V2_ENABLED: AbProp = AbProp {
        name: "smb_md_agent_chat_assignment_system_messages_logging_v2_enabled",
        code: 2709,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_META_VERIFIED_CONTEXT_CARD: AbProp = AbProp {
        name: "smb_meta_verified_context_card",
        code: 8313,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_MULTI_DEVICE_AGENTS_LOGGING_V2_ENABLED: AbProp = AbProp {
        name: "smb_multi_device_agents_logging_V2_enabled",
        code: 1897,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_MULTI_DEVICE_MESSAGE_ATTRIBUTION_ENABLED: AbProp = AbProp {
        name: "smb_multi_device_message_attribution_enabled",
        code: 1981,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_NOTES_CONTENT_MAX_LIMIT: AbProp = AbProp {
        name: "smb_notes_content_max_limit",
        code: 10272,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5000),
    };
    pub const SMB_NOTES_V1_ENABLED: AbProp = AbProp {
        name: "smb_notes_v1_enabled",
        code: 7710,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PAYMENT_LINKS_CTA_BUTTON_KILL_SWITCH: AbProp = AbProp {
        name: "smb_payment_links_cta_button_kill_switch",
        code: 14967,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PAYMENT_LINKS_CTA_PSP_LIST: AbProp = AbProp {
        name: "smb_payment_links_cta_psp_list",
        code: 14998,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const SMB_PAYMENT_LINKS_CTA_VARIANT: AbProp = AbProp {
        name: "smb_payment_links_cta_variant",
        code: 14957,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2),
    };
    pub const SMB_PAYMENT_LINKS_LOGGING_ENABLED: AbProp = AbProp {
        name: "smb_payment_links_logging_enabled",
        code: 9213,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PAYMENT_LINKS_SELLER_LOGGING_ENABLED: AbProp = AbProp {
        name: "smb_payment_links_seller_logging_enabled",
        code: 10389,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PAYMENT_LINKS_URL_REGEX_LIST: AbProp = AbProp {
        name: "smb_payment_links_url_regex_list",
        code: 8969,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{}"),
    };
    pub const SMB_PAYMENT_REQUEST_STATUS_UPDATE: AbProp = AbProp {
        name: "smb_payment_request_status_update",
        code: 27077,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PHASE_OUT_NOT_A_BUSINESS_V2: AbProp = AbProp {
        name: "smb_phase_out_not_a_business_V2",
        code: 1771,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PREMIUM_MESSAGES_CLICK_LOGGING_ENABLED: AbProp = AbProp {
        name: "smb_premium_messages_click_logging_enabled",
        code: 4657,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PREMIUM_MESSAGES_URL_CTA_ALERT_DIALOG_ENABLED: AbProp = AbProp {
        name: "smb_premium_messages_url_cta_alert_dialog_enabled",
        code: 5044,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SMB_PRODUCT_COUNTRY_OF_ORIGIN_M1: AbProp = AbProp {
        name: "smb_product_country_of_origin_m1",
        code: 13415,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_PROJECT_WALDO_SET_PRICE_TIER_BIZ_PROFILE_ENABLED: AbProp = AbProp {
        name: "smb_project_waldo_set_price_tier_biz_profile_enabled",
        code: 3467,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_QP_CONVERSION_TRACKING_INFRA: AbProp = AbProp {
        name: "smb_qp_conversion_tracking_infra",
        code: 26331,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_QP_EMERGENCY_FORCE_FETCH_NONCE: AbProp = AbProp {
        name: "smb_qp_emergency_force_fetch_nonce",
        code: 27115,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const SMB_QP_WEB_DEBUG_RECUNIT: AbProp = AbProp {
        name: "smb_qp_web_debug_recunit",
        code: 31009,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_RAMBUTAN_ENABLED: AbProp = AbProp {
        name: "smb_rambutan_enabled",
        code: 3124,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_TEMP_COVER_PHOTO_PRIVACY_MESSAGING: AbProp = AbProp {
        name: "smb_temp_cover_photo_privacy_messaging",
        code: 1913,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_TOS_QP_CHATLIST_BANNER: AbProp = AbProp {
        name: "smb_tos_qp_chatlist_banner",
        code: 32396,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WALDO_SERVICE_OFFERINGS_SELECTION_ENABLED: AbProp = AbProp {
        name: "smb_waldo_service_offerings_selection_enabled",
        code: 3285,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_BB_HOME_QP_SURFACE_ENABLED: AbProp = AbProp {
        name: "smb_web_bb_home_qp_surface_enabled",
        code: 32613,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CATEGORY_SEARCH_VIA_GRAPH_ENABLED: AbProp = AbProp {
        name: "smb_web_category_search_via_graph_enabled",
        code: 28519,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CUSTOMER_MANAGEMENT_ENABLED: AbProp = AbProp {
        name: "smb_web_customer_management_enabled",
        code: 26165,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CUSTOMER_MANAGER_BULK_EDIT_ENABLED: AbProp = AbProp {
        name: "smb_web_customer_manager_bulk_edit_enabled",
        code: 32550,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CUSTOMER_MANAGER_DATE_RANGE_FILTER_ENABLED: AbProp = AbProp {
        name: "smb_web_customer_manager_date_range_filter_enabled",
        code: 32096,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CUSTOMER_MANAGER_DOB_FILTER_ENABLED: AbProp = AbProp {
        name: "smb_web_customer_manager_dob_filter_enabled",
        code: 32229,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_CUSTOMER_MANAGER_EXPORT_ENABLED: AbProp = AbProp {
        name: "smb_web_customer_manager_export_enabled",
        code: 32287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_ENABLE_FB_LINKING: AbProp = AbProp {
        name: "smb_web_enable_fb_linking",
        code: 30112,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMB_WEB_SHOW_QUICK_REPLY_OPTION_IN_COMPOSER: AbProp = AbProp {
        name: "smb_web_show_quick_reply_option_in_composer",
        code: 31700,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBA_BB_GENAI_COMPOSER_MIN_WORDS: AbProp = AbProp {
        name: "smba_bb_genai_composer_min_words",
        code: 21447,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_CUSTOM_USER_PROMPT_ENABLED: AbProp = AbProp {
        name: "smba_business_broadcast_genai_custom_user_prompt_enabled",
        code: 20464,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_MASTER_ABPROP: AbProp = AbProp {
        name: "smba_business_broadcast_genai_master_abprop",
        code: 22384,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_SHARE_MESSAGE_HISTORY: AbProp = AbProp {
        name: "smba_business_broadcast_genai_share_message_history",
        code: 20926,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_TEXT: AbProp = AbProp {
        name: "smba_business_broadcast_genai_text",
        code: 17743,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_TEXT_MAX_TRIES: AbProp = AbProp {
        name: "smba_business_broadcast_genai_text_max_tries",
        code: 20946,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const SMBA_BUSINESS_BROADCAST_GENAI_TEXT_MODEL: AbProp = AbProp {
        name: "smba_business_broadcast_genai_text_model",
        code: 20929,
        value_type: AbPropType::Str,
        default: AbDefault::Str("LLAMA"),
    };
    pub const SMBA_BUSINESS_BROADCAST_RECIPIENT_LIMIT: AbProp = AbProp {
        name: "smba_business_broadcast_recipient_limit",
        code: 17937,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const SMBA_PREMIUM_MESSAGES_LEAVING_WA_CONTENT: AbProp = AbProp {
        name: "smba_premium_messages_leaving_wa_content",
        code: 6693,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SMBI_PREMIUM_BROADCAST_MAX_RECIPIENT_LIMIT: AbProp = AbProp {
        name: "smbi_premium_broadcast_max_recipient_limit",
        code: 23857,
        value_type: AbPropType::Int,
        default: AbDefault::Int(256),
    };
    pub const SMBW_BUSINESS_BROADCAST_DUPLICATE_ENABLED: AbProp = AbProp {
        name: "smbw_business_broadcast_duplicate_enabled",
        code: 29021,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMBW_BUSINESS_BROADCAST_SMART_COLUMN_DETECTION_ENABLED: AbProp = AbProp {
        name: "smbw_business_broadcast_smart_column_detection_enabled",
        code: 27999,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMOOTHIE_PERFORMANCE_CSS_DOM: AbProp = AbProp {
        name: "smoothie_performance_css_dom",
        code: 18995,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SMOOTHIE_PERFORMANCE_MSG_SEND: AbProp = AbProp {
        name: "smoothie_performance_msg_send",
        code: 17942,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SMOOTHIE_PERFORMANCE_RESIZE_FOLLOWUP: AbProp = AbProp {
        name: "smoothie_performance_resize_followup",
        code: 18992,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SNAPL_NEWSLETTER_LOGGING_ENCRYPTED_RID_ENABLED: AbProp = AbProp {
        name: "snapl_newsletter_logging_encrypted_rid_enabled",
        code: 32239,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SNAPL_NEWSLETTER_LOGGING_MEDIA_ID_PLACEHOLDER_STRING: AbProp = AbProp {
        name: "snapl_newsletter_logging_media_id_placeholder_string",
        code: 14064,
        value_type: AbPropType::Str,
        default: AbDefault::Str("-1"),
    };
    pub const SNAPSHOT_RECOVERY_MAX_MUTATIONS_COUNT_ALLOWED: AbProp = AbProp {
        name: "snapshot_recovery_max_mutations_count_allowed",
        code: 18786,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2000),
    };
    pub const SOCCER_BALL_REACTION_FULL_ANIMATION_ENABLED: AbProp = AbProp {
        name: "soccer_ball_reaction_full_animation_enabled",
        code: 27834,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SOCCER_REACTION_IN_TRAY_ENABLED: AbProp = AbProp {
        name: "soccer_reaction_in_tray_enabled",
        code: 27833,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_ALLOW_FORWARDING_TO_STATUS_ON_WEB: AbProp = AbProp {
        name: "status_allow_forwarding_to_status_on_web",
        code: 17071,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_E2EE_RECV_OVER_STATUS_STANZA: AbProp = AbProp {
        name: "status_e2ee_recv_over_status_stanza",
        code: 27622,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_E2EE_SEND_OVER_STATUS_STANZA: AbProp = AbProp {
        name: "status_e2ee_send_over_status_stanza",
        code: 27620,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_FUTURE_PROOFING: AbProp = AbProp {
        name: "status_future_proofing",
        code: 9522,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_INFRA_1_1_SESSION_SPLIT: AbProp = AbProp {
        name: "status_infra_1_1_session_split",
        code: 25034,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const STATUS_LIKES_FIFA_LOTTIE_FULL_SCREEN_ANIMATION_ENABLED: AbProp = AbProp {
        name: "status_likes_fifa_lottie_full_screen_animation_enabled",
        code: 27054,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_LIKES_SENDING_ENABLED: AbProp = AbProp {
        name: "status_likes_sending_enabled",
        code: 31665,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_MENTIONS_GROUP_MENTION_RECEIVER: AbProp = AbProp {
        name: "status_mentions_group_mention_receiver",
        code: 12254,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_MENTIONS_RECEIVER: AbProp = AbProp {
        name: "status_mentions_receiver",
        code: 7869,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_PLAYER_AVATAR_STATUS_CREATION_ENTRYPOINT: AbProp = AbProp {
        name: "status_player_avatar_status_creation_entrypoint",
        code: 30912,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_POG_ID_ROTATION_WINDOW_DAYS: AbProp = AbProp {
        name: "status_pog_id_rotation_window_days",
        code: 18297,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const STATUS_POSTER_SIDE_GATING_ENABLED: AbProp = AbProp {
        name: "status_poster_side_gating_enabled",
        code: 8742,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_REACTION_EMOJIS: AbProp = AbProp {
        name: "status_reaction_emojis",
        code: 1852,
        value_type: AbPropType::Str,
        default: AbDefault::Str("[128525, 128514, 128558, 128546, 128591, 128079, 127881, 128175]"),
    };
    pub const STATUS_SAVE_TO_CAMERA_ROLL_ENABLED: AbProp = AbProp {
        name: "status_save_to_camera_roll_enabled",
        code: 13280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STATUS_VIDEO_MAX_DURATION: AbProp = AbProp {
        name: "status_video_max_duration",
        code: 175,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const STATUS_WEB_RANKING: AbProp = AbProp {
        name: "status_web_ranking",
        code: 31666,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STICKER_STORE_TESTING_ENABLED: AbProp = AbProp {
        name: "sticker_store_testing_enabled",
        code: 25639,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STICKERS_EMOJI_TAGGING_ENABLED: AbProp = AbProp {
        name: "stickers_emoji_tagging_enabled",
        code: 26465,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const STICKY_CHAT_PROFILE_PICTURE_ENABLED: AbProp = AbProp {
        name: "sticky_chat_profile_picture_enabled",
        code: 13692,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SUGGESTED_AUDIENCES_WA_WEB: AbProp = AbProp {
        name: "suggested_audiences_wa_web",
        code: 26207,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SUPPORT_CONTACT_FORM_USING_GRAPHQL: AbProp = AbProp {
        name: "support_contact_form_using_graphql",
        code: 26001,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SUPPORT_LIDS: AbProp = AbProp {
        name: "support_lids",
        code: 14317,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "4200746488034,30563255730192,70334669676777,19349129719984,66065505775654,133814269518032,243799792062487,7323238039569,269290422947912,261718412386336,4351103873168,12391299473616,92410801582180,277730033709185,36090878648473,79882365190287,94274800595104,117794058317863,115784047153172,179250745360524,7301780005088,166653589463190,94249030815912,198964645236955,198427807899653,23656948363422,255735573270728,106670109786240,130932396826763,18855208456329",
        ),
    };
    pub const SUPPORT_MESSAGE_FEEDBACK_ENABLED: AbProp = AbProp {
        name: "support_message_feedback_enabled",
        code: 7080,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SUPPORTS_KEEP_IN_CHAT_IN_CAG: AbProp = AbProp {
        name: "supports_keep_in_chat_in_cag",
        code: 2844,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const SYNCD_ADDITIONAL_MUTATIONS_COUNT: AbProp = AbProp {
        name: "syncd_additional_mutations_count",
        code: 2777,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const SYNCD_INLINE_MUTATIONS_MAX_COUNT: AbProp = AbProp {
        name: "syncd_inline_mutations_max_count",
        code: 14494,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const SYNCD_KEY_MAX_USE_DAYS: AbProp = AbProp {
        name: "syncd_key_max_use_days",
        code: 14488,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const SYNCD_LTHASH_CONSISTENCY_CHECK_ON_SNAPSHOT_MAC_MISMATCH: AbProp = AbProp {
        name: "syncd_lthash_consistency_check_on_snapshot_mac_mismatch",
        code: 1783,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SYNCD_MUTATION_AND_BUNDLE_LOGGING: AbProp = AbProp {
        name: "syncd_mutation_and_bundle_logging",
        code: 11821,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"allowlist\": []}"),
    };
    pub const SYNCD_PATCH_PROTOBUF_MAX_SIZE: AbProp = AbProp {
        name: "syncd_patch_protobuf_max_size",
        code: 14495,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const SYNCD_PERIODIC_SYNC_DAYS: AbProp = AbProp {
        name: "syncd_periodic_sync_days",
        code: 1400,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const SYNCD_SENTINEL_TIMEOUT_SECONDS: AbProp = AbProp {
        name: "syncd_sentinel_timeout_seconds",
        code: 14485,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const SYNCD_USE_INDEX_FOR_LTHASH_LOOKUP: AbProp = AbProp {
        name: "syncd_use_index_for_lthash_lookup",
        code: 28144,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const SYNCD_WAIT_FOR_KEY_TIMEOUT_DAYS: AbProp = AbProp {
        name: "syncd_wait_for_key_timeout_days",
        code: 14492,
        value_type: AbPropType::Int,
        default: AbDefault::Int(7),
    };
    pub const SYNCED_MESSAGE_KEYS_PROCESSING_TYPE: AbProp = AbProp {
        name: "synced_message_keys_processing_type",
        code: 22825,
        value_type: AbPropType::Str,
        default: AbDefault::Str("control"),
    };
    pub const SYSTEM_MSG_NUMBERS_FB_BRANDED: AbProp = AbProp {
        name: "system_msg_numbers_fb_branded",
        code: 1035,
        value_type: AbPropType::Str,
        default: AbDefault::Str(
            "16325551023,16505434800,16503130062,16507885324,16508620604,16504228206,447710173736,16315551023,16505361212,16508129150,16315555102,16315558723,16505212669,16507885280,19032707825,0",
        ),
    };
    pub const SYSTEM_MSG_NUMBERS_FB_INC: AbProp = AbProp {
        name: "system_msg_numbers_fb_inc",
        code: 1036,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const SYSTEM_MSG_TEXT_STYLING: AbProp = AbProp {
        name: "system_msg_text_styling",
        code: 6246,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TAPPABLE_LINKS_IN_POLL_OPTION_ENABLED: AbProp = AbProp {
        name: "tappable_links_in_poll_option_enabled",
        code: 26062,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TCTOKEN_DURATION: AbProp = AbProp {
        name: "tctoken_duration",
        code: 865,
        value_type: AbPropType::Int,
        default: AbDefault::Int(604800),
    };
    pub const TCTOKEN_DURATION_SENDER: AbProp = AbProp {
        name: "tctoken_duration_sender",
        code: 996,
        value_type: AbPropType::Int,
        default: AbDefault::Int(604800),
    };
    pub const TCTOKEN_NUM_BUCKETS: AbProp = AbProp {
        name: "tctoken_num_buckets",
        code: 909,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const TCTOKEN_NUM_BUCKETS_SENDER: AbProp = AbProp {
        name: "tctoken_num_buckets_sender",
        code: 997,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const TEXT_STATUS_TTL_SECONDS_ALLOWLIST: AbProp = AbProp {
        name: "text_status_ttl_seconds_allowlist",
        code: 6153,
        value_type: AbPropType::Str,
        default: AbDefault::Str("1800,3600,7200,14400,28800,86400"),
    };
    pub const TEXT_USER_JOURNEY_LOGGING_WAM_ENABLED: AbProp = AbProp {
        name: "text_user_journey_logging_wam_enabled",
        code: 8627,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TIMEOUT_MEX_CALL_EXPAND_FMX_TRUST_SIGNALS: AbProp = AbProp {
        name: "timeout_mex_call_expand_fmx_trust_signals",
        code: 27862,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600),
    };
    pub const TOP_LEVEL_MESSAGE_SECRET_CHECK: AbProp = AbProp {
        name: "top_level_message_secret_check",
        code: 23796,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TOS_3_CLIENT_GATING_ENABLED: AbProp = AbProp {
        name: "tos_3_client_gating_enabled",
        code: 791,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TOS_CLIENT_STATE_FETCH_ENABLED: AbProp = AbProp {
        name: "tos_client_state_fetch_enabled",
        code: 877,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TOS_CLIENT_STATE_FETCH_ITERATION: AbProp = AbProp {
        name: "tos_client_state_fetch_iteration",
        code: 908,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const TRANSCODE_AND_REPAIR_VIDEOS: AbProp = AbProp {
        name: "transcode_and_repair_videos",
        code: 26027,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const TS_SESSION_DURATION_MS: AbProp = AbProp {
        name: "ts_session_duration_ms",
        code: 3860,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600000),
    };
    pub const TS_SURFACE_KILLSWITCH: AbProp = AbProp {
        name: "ts_surface_killswitch",
        code: 4929,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const UGC_ENABLED: AbProp = AbProp {
        name: "ugc_enabled",
        code: 3011,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UGC_PARTICIPANT_LIMIT: AbProp = AbProp {
        name: "ugc_participant_limit",
        code: 4118,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const UNIFIED_CALLING_ENTRY_POINT_DESKTOP_TYPE: AbProp = AbProp {
        name: "unified_calling_entry_point_desktop_type",
        code: 21591,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const UNIFIED_OTP_COPY_CODE_URL: AbProp = AbProp {
        name: "unified_otp_copy_code_url",
        code: 3827,
        value_type: AbPropType::Str,
        default: AbDefault::Str("https://www.whatsapp.com/otp/copy/"),
    };
    pub const UNIFIED_OTP_RETRIEVER_URL: AbProp = AbProp {
        name: "unified_otp_retriever_url",
        code: 3828,
        value_type: AbPropType::Str,
        default: AbDefault::Str("https://www.whatsapp.com/otp/code"),
    };
    pub const UNIFIED_PIN_ADDON_TABLE_ENABLED: AbProp = AbProp {
        name: "unified_pin_addon_table_enabled",
        code: 8356,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFIED_POLL_VOTE_ADDON_INFRA_ENABLED: AbProp = AbProp {
        name: "unified_poll_vote_addon_infra_enabled",
        code: 6046,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFIED_RESPONSE_AI_CONTENT_SEARCH_ENABLED: AbProp = AbProp {
        name: "unified_response_ai_content_search_enabled",
        code: 30000,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFIED_RESPONSE_AI_SPORTS_WIDGET_ENABLED: AbProp = AbProp {
        name: "unified_response_ai_sports_widget_enabled",
        code: 31780,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFIED_RESPONSE_MARKDOWN_LINKS_ENABLED: AbProp = AbProp {
        name: "unified_response_markdown_links_enabled",
        code: 30330,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFIED_SESSION_LOG_CALL_EVENT: AbProp = AbProp {
        name: "unified_session_log_call_event",
        code: 8582,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UNIFY_END_CALL_EVENTS: AbProp = AbProp {
        name: "unify_end_call_events",
        code: 2856,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UPDATED_HARMFUL_DOCUMENT_DIALOG: AbProp = AbProp {
        name: "updated_harmful_document_dialog",
        code: 15022,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UPDATES_PRIVACY_NOTICE_ROLLOUT_DATE: AbProp = AbProp {
        name: "updates_privacy_notice_rollout_date",
        code: 14387,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1742310000),
    };
    pub const UPDATES_QUICK_PROMOTION_BANNER_ENABLED: AbProp = AbProp {
        name: "updates_quick_promotion_banner_enabled",
        code: 13997,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UPLOAD_DOCUMENT_THUMB_MMS_ENABLED: AbProp = AbProp {
        name: "upload_document_thumb_mms_enabled",
        code: 247,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USE_CACHED_APP_SETTINGS_FROM_GLOBAL_CTX: AbProp = AbProp {
        name: "use_cached_app_settings_from_global_ctx",
        code: 13428,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const USE_CUSTOM_SOCCER_BALL_FOR_REACTION_ENABLED: AbProp = AbProp {
        name: "use_custom_soccer_ball_for_reaction_enabled",
        code: 27807,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USE_PER_CHAT_WALLPAPER: AbProp = AbProp {
        name: "use_per_chat_wallpaper",
        code: 9756,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USE_SIGNED_SHIMMED_URL_LINK: AbProp = AbProp {
        name: "use_signed_shimmed_url_link",
        code: 11977,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_1ON1_SYS_MSG_CREATION_UPSELL_ENABLED: AbProp = AbProp {
        name: "username_1on1_sys_msg_creation_upsell_enabled",
        code: 27359,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const USERNAME_ACTIVATION_QP: AbProp = AbProp {
        name: "username_activation_qp",
        code: 32809,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_ADOPTION_AND_ENGAGEMENT_MONITORING_ENABLED: AbProp = AbProp {
        name: "username_adoption_and_engagement_monitoring_enabled",
        code: 15493,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_ANTISCRAPING_SEND_CACHED_UN: AbProp = AbProp {
        name: "username_antiscraping_send_cached_un",
        code: 31261,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_API_RATE_LIMIT_ENABLED: AbProp = AbProp {
        name: "username_api_rate_limit_enabled",
        code: 28678,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CHANNELS_PN_PRIVACY_ENABLED: AbProp = AbProp {
        name: "username_channels_pn_privacy_enabled",
        code: 23795,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CHECK_DEBOUNCE_IN_MS: AbProp = AbProp {
        name: "username_check_debounce_in_ms",
        code: 18975,
        value_type: AbPropType::Int,
        default: AbDefault::Int(600),
    };
    pub const USERNAME_CONTACT_CARD_DEDUPE_ICONS: AbProp = AbProp {
        name: "username_contact_card_dedupe_icons",
        code: 32614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CONTACT_DISPLAY: AbProp = AbProp {
        name: "username_contact_display",
        code: 4746,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CONTACT_PRIVACY_SETTING_ALLOW_UNCONTACT_SET_ENABLE: AbProp = AbProp {
        name: "username_contact_privacy_setting_allow_uncontact_set_enable",
        code: 20993,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CONTACT_SYNCD_SUPPORT_ENABLE: AbProp = AbProp {
        name: "username_contact_syncd_support_enable",
        code: 17614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CONTACT_UI_VCARD: AbProp = AbProp {
        name: "username_contact_ui_vcard",
        code: 18204,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CONTACT_USYNC_LID_BASED: AbProp = AbProp {
        name: "username_contact_usync_lid_based",
        code: 14565,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CREATION: AbProp = AbProp {
        name: "username_creation",
        code: 4745,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_CREATION_RESERVATION_PP_DISCLOSURE_ENABLED: AbProp = AbProp {
        name: "username_creation_reservation_pp_disclosure_enabled",
        code: 32098,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_ENABLED_ON_COMPANION: AbProp = AbProp {
        name: "username_enabled_on_companion",
        code: 23817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_ENGAGEMENT_NETWORK_IMPACT_LOGGING: AbProp = AbProp {
        name: "username_engagement_network_impact_logging",
        code: 11794,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_EXPOSED_LOGGING_ENABLED: AbProp = AbProp {
        name: "username_exposed_logging_enabled",
        code: 25353,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_GLOBAL_SEARCH_ENABLED: AbProp = AbProp {
        name: "username_global_search_enabled",
        code: 18251,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_GROUP_MUTATION_ENABLED: AbProp = AbProp {
        name: "username_group_mutation_enabled",
        code: 16148,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_KEY_REDESIGN_ENABLED: AbProp = AbProp {
        name: "username_key_redesign_enabled",
        code: 29026,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_KEY_UPSELL_MAX_CHARACTERS: AbProp = AbProp {
        name: "username_key_upsell_max_characters",
        code: 25790,
        value_type: AbPropType::Int,
        default: AbDefault::Int(8),
    };
    pub const USERNAME_KEY_UPSELL_MAX_NUMBERS: AbProp = AbProp {
        name: "username_key_upsell_max_numbers",
        code: 25789,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const USERNAME_KEY_UPSELL_MODE: AbProp = AbProp {
        name: "username_key_upsell_mode",
        code: 26220,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const USERNAME_LID_MIGRATION_CALLING: AbProp = AbProp {
        name: "username_lid_migration_calling",
        code: 21890,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_MAX_LENGTH: AbProp = AbProp {
        name: "username_max_length",
        code: 20459,
        value_type: AbPropType::Int,
        default: AbDefault::Int(35),
    };
    pub const USERNAME_MEX_ACCOUNT_SYNC_ENABLED: AbProp = AbProp {
        name: "username_mex_account_sync_enabled",
        code: 8763,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_MIN_LENGTH: AbProp = AbProp {
        name: "username_min_length",
        code: 20494,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const USERNAME_NUMERIC_CODE_V4: AbProp = AbProp {
        name: "username_numeric_code_v4",
        code: 14286,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const USERNAME_SEARCH: AbProp = AbProp {
        name: "username_search",
        code: 15956,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_SECURITY_CODE_GENERATION: AbProp = AbProp {
        name: "username_security_code_generation",
        code: 7468,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const USERNAME_SUGGESTIONS_ENABLED: AbProp = AbProp {
        name: "username_suggestions_enabled",
        code: 21984,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UTILITY_ORDER_STATUS_LOGGING_ENABLED: AbProp = AbProp {
        name: "utility_order_status_logging_enabled",
        code: 19059,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UTILITY_ORDER_VIEW_MBS_ENABLED: AbProp = AbProp {
        name: "utility_order_view_mbs_enabled",
        code: 31282,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UTILITY_PAYMENT_REMINDER_M1_ENABLED: AbProp = AbProp {
        name: "utility_payment_reminder_m1_enabled",
        code: 22434,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UTM_TRACKING_ENABLED: AbProp = AbProp {
        name: "utm_tracking_enabled",
        code: 2895,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const UTM_TRACKING_EXPIRATION_HOURS: AbProp = AbProp {
        name: "utm_tracking_expiration_hours",
        code: 2896,
        value_type: AbPropType::Int,
        default: AbDefault::Int(24),
    };
    pub const UWP_VOIP_INCOMING_CALL_NOTIFICATION_VERSION: AbProp = AbProp {
        name: "uwp_voip_incoming_call_notification_version",
        code: 7541,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const VERIFIED_BADGE_IN_CHATS_LIST_ENABLED: AbProp = AbProp {
        name: "verified_badge_in_chats_list_enabled",
        code: 9292,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VID_PORT_ENABLE_CAPTURE_FPS_MEDIAN_FILTER: AbProp = AbProp {
        name: "vid_port_enable_capture_fps_median_filter",
        code: 29214,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VID_PORT_FRM_BUF_MUTEX_FIXES: AbProp = AbProp {
        name: "vid_port_frm_buf_mutex_fixes",
        code: 22525,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VID_STREAM_PAUSE_RESUME_JB_RESET_THRESHOLD_MS: AbProp = AbProp {
        name: "vid_stream_pause_resume_jb_reset_threshold_ms",
        code: 2642,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const VIDEO_STREAM_BUFFERING_UI_ENABLED: AbProp = AbProp {
        name: "video_stream_buffering_ui_enabled",
        code: 2167,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VIEW_REPLIES_ENTRY_POINT: AbProp = AbProp {
        name: "view_replies_entry_point",
        code: 19860,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const VIEW_REPLIES_INFRA_ENABLED: AbProp = AbProp {
        name: "view_replies_infra_enabled",
        code: 14199,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VIEW_REPLIES_IS_COMPOSER_ENABLED: AbProp = AbProp {
        name: "view_replies_is_composer_enabled",
        code: 20817,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const VIEW_REPLIES_WITH_THREADID_ENABLED: AbProp = AbProp {
        name: "view_replies_with_threadid_enabled",
        code: 16998,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VISIBLE_MESSAGE_DROP_PLACEHOLDER_ENABLED_INTERNAL_ONLY: AbProp = AbProp {
        name: "visible_message_drop_placeholder_enabled_internal_only",
        code: 7287,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VOICE_AI_CONVERSATION_STARTER_LATENCY_TRACKING: AbProp = AbProp {
        name: "voice_ai_conversation_starter_latency_tracking",
        code: 19624,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const VOICE_CHAT_COMPANION_EXPERIENCE_VERSION: AbProp = AbProp {
        name: "voice_chat_companion_experience_version",
        code: 17052,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const VOICEMAIL_NUDGE_DURATION_MS: AbProp = AbProp {
        name: "voicemail_nudge_duration_ms",
        code: 18339,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4000),
    };
    pub const VOIP_CALL_COORDINATOR_VERSION: AbProp = AbProp {
        name: "voip_call_coordinator_version",
        code: 9502,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const VOIP_ENABLE_WEBRTC_STATS_POLLING: AbProp = AbProp {
        name: "voip_enable_webrtc_stats_polling",
        code: 26744,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const VOIP_STACK_INCOMING_MESSAGE_OWNERSHIP_TRANSFER: AbProp = AbProp {
        name: "voip_stack_incoming_message_ownership_transfer",
        code: 16481,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_ASTERIA_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED: AbProp = AbProp {
        name: "wa_asteria_eligibility_subscription_status_check_enabled",
        code: 26399,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_ASTERIA_ENABLED: AbProp = AbProp {
        name: "wa_asteria_enabled",
        code: 26234,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_ASTERIA_META_AI_SETTINGS_TAB_ENTRYPOINT_ENABLED: AbProp = AbProp {
        name: "wa_asteria_meta_ai_settings_tab_entrypoint_enabled",
        code: 27118,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_ASTERIA_ROLLOUT_ENABLED: AbProp = AbProp {
        name: "wa_asteria_rollout_enabled",
        code: 26996,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_AUTH_AGENT_OFFBOARDING_ENABLED: AbProp = AbProp {
        name: "wa_auth_agent_offboarding_enabled",
        code: 29923,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CAPPING_LOCAL_DATA_LOGIC_UPDATE: AbProp = AbProp {
        name: "wa_capping_local_data_logic_update",
        code: 21348,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CATALOG_GRAPHQL_USE_LID_ENABLED: AbProp = AbProp {
        name: "wa_catalog_graphql_use_lid_enabled",
        code: 30797,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CONSUMER_ENTRY_POINT_ENABLED: AbProp = AbProp {
        name: "wa_consumer_entry_point_enabled",
        code: 24380,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CONSUMER_NOVA_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED: AbProp = AbProp {
        name: "wa_consumer_nova_eligibility_subscription_status_check_enabled",
        code: 25388,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CONSUMER_NOVA_ENTRY_POINT_SETTINGS_ENABLED: AbProp = AbProp {
        name: "wa_consumer_nova_entry_point_settings_enabled",
        code: 24495,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CONSUMER_NOVA_SETTINGS_GREEN_DOT_ENABLED: AbProp = AbProp {
        name: "wa_consumer_nova_settings_green_dot_enabled",
        code: 24955,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CONSUMER_NOVA_SUBSCRIPTION_NOTIFICATIONS_ENABLED: AbProp = AbProp {
        name: "wa_consumer_nova_subscription_notifications_enabled",
        code: 27068,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_LOG_USER_JOURNEY_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_log_user_journey_enabled",
        code: 1681,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_ENABLE_CONTINUOUS_DURATION: AbProp = AbProp {
        name: "wa_ctwa_web_enable_continuous_duration",
        code: 31426,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_ENTRYPOINT_HOME_HEADER_DROPDOWN_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_web_entrypoint_home_header_dropdown_enabled",
        code: 3095,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_ENTRYPOINT_HOME_HEADER_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_web_entrypoint_home_header_enabled",
        code: 3058,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_ENTRYPOINT_MANAGE_ADS_HOME_HEADER_DROPDOWN_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_web_entrypoint_manage_ads_home_header_dropdown_enabled",
        code: 3376,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_FETCH_LINKED_ACCOUNTS_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_web_fetch_linked_accounts_enabled",
        code: 3294,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_HIDE_AD_CONTEXT_IF_SOFT_DISMISSED_IN_PRIMARY: AbProp = AbProp {
        name: "wa_ctwa_web_hide_ad_context_if_soft_dismissed_in_primary",
        code: 9729,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_CTWA_WEB_THREAD_AD_ATTRIBUTION_ENABLED: AbProp = AbProp {
        name: "wa_ctwa_web_thread_ad_attribution_enabled",
        code: 2898,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_ENABLED: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_capping_enabled",
        code: 20865,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_FETCH_TTL_SECONDS: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_capping_fetch_ttl_seconds",
        code: 20649,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3600),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_LIMIT: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_capping_limit",
        code: 17845,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_MV_GET_SUBSCRIPTION_V2: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_capping_mv_get_subscription_v2",
        code: 20667,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_FCI_STALENESS_TTL_IN_SECONDS: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_fci_staleness_ttl_in_seconds",
        code: 21410,
        value_type: AbPropType::Int,
        default: AbDefault::Int(120),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_MSG_LATEST_RAMPUP_DATE: AbProp = AbProp {
        name: "wa_individual_new_chat_msg_latest_rampup_date",
        code: 20601,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_INDIVIDUAL_NEW_CHAT_THREAD_CAPPING_LIMIT: AbProp = AbProp {
        name: "wa_individual_new_chat_thread_capping_limit",
        code: 29369,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_MEDIA_IMAGE_UPLOAD_CACHE: AbProp = AbProp {
        name: "wa_media_image_upload_cache",
        code: 22784,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_META_ONE_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED: AbProp = AbProp {
        name: "wa_meta_one_eligibility_subscription_status_check_enabled",
        code: 28613,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_META_ONE_ENABLED: AbProp = AbProp {
        name: "wa_meta_one_enabled",
        code: 28611,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_META_ONE_LAUNCH_FREE_TRIAL_ENABLED: AbProp = AbProp {
        name: "wa_meta_one_launch_free_trial_enabled",
        code: 29290,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_META_ONE_ROLLOUT_ENABLED: AbProp = AbProp {
        name: "wa_meta_one_rollout_enabled",
        code: 28612,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_META_ONE_SUBSCRIPTION_NOTIFICATIONS_ENABLED: AbProp = AbProp {
        name: "wa_meta_one_subscription_notifications_enabled",
        code: 29866,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_NCT_TOKEN_HISTORY_SYNC_ENABLED: AbProp = AbProp {
        name: "wa_nct_token_history_sync_enabled",
        code: 25189,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_NCT_TOKEN_SALT_CREATION_ENABLED: AbProp = AbProp {
        name: "wa_nct_token_salt_creation_enabled",
        code: 24915,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_NCT_TOKEN_SEND_ENABLED: AbProp = AbProp {
        name: "wa_nct_token_send_enabled",
        code: 24941,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_NCT_TOKEN_SYNCD_ENABLED: AbProp = AbProp {
        name: "wa_nct_token_syncd_enabled",
        code: 25253,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_OHAI_NEW_VIP_HEADER_ENABLED: AbProp = AbProp {
        name: "wa_ohai_new_vip_header_enabled",
        code: 31340,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_PAYMENTS_SMB_ENABLED: AbProp = AbProp {
        name: "wa_payments_smb_enabled",
        code: 27173,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_PAYMENTS_SMB_LABELS_CONVENTION_ENABLED: AbProp = AbProp {
        name: "wa_payments_smb_labels_convention_enabled",
        code: 27172,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_QP_EXPOSURE_LOG_VIA_GRAPHQL_ENABLED: AbProp = AbProp {
        name: "wa_qp_exposure_log_via_graphql_enabled",
        code: 31560,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_SMB_BIZ_PROFILE_GOOGLE_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "wa_smb_biz_profile_google_integration_enabled",
        code: 29007,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_SMB_FORWARD_BB_WEB_ENABLED: AbProp = AbProp {
        name: "wa_smb_forward_bb_web_enabled",
        code: 30028,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_SMB_WEB_LISTS_QUICK_REPLIES_ENABLED: AbProp = AbProp {
        name: "wa_smb_web_lists_quick_replies_enabled",
        code: 31061,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ADAPTIVE_LAYOUT_ENABLED: AbProp = AbProp {
        name: "wa_web_adaptive_layout_enabled",
        code: 30140,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_AGM_SIGNUP_ENABLED: AbProp = AbProp {
        name: "wa_web_agm_signup_enabled",
        code: 26467,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ANR_PUSHNAME_CHECK_ENABLED: AbProp = AbProp {
        name: "wa_web_anr_pushname_check_enabled",
        code: 32065,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ANYONE_CAN_LINK_M2: AbProp = AbProp {
        name: "wa_web_anyone_can_link_m2",
        code: 24432,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ANYONE_CAN_LINK_M2_FLOOD_LIMIT: AbProp = AbProp {
        name: "wa_web_anyone_can_link_m2_flood_limit",
        code: 25009,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const WA_WEB_APP_LOCK_UPSELL: AbProp = AbProp {
        name: "wa_web_app_lock_upsell",
        code: 20064,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ATTACH_ICON_VARIANT: AbProp = AbProp {
        name: "wa_web_attach_icon_variant",
        code: 26386,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_BASE_VIDEO_COMET_VIDEO_PLAYER_ENABLED: AbProp = AbProp {
        name: "wa_web_base_video_comet_video_player_enabled",
        code: 25660,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_BROADCAST_COLLECTION_BASED_CAMPAIGNS_ENABLED: AbProp = AbProp {
        name: "wa_web_biz_broadcast_collection_based_campaigns_enabled",
        code: 31682,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_BROADCASTS_CATALOG_ATTACHMENT: AbProp = AbProp {
        name: "wa_web_biz_broadcasts_catalog_attachment",
        code: 28471,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_BROADCASTS_CONTEXTUAL_ENTRYPOINTS: AbProp = AbProp {
        name: "wa_web_biz_broadcasts_contextual_entrypoints",
        code: 30270,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_PROFILE_GOOGLE_INTEGRATION_ENABLED: AbProp = AbProp {
        name: "wa_web_biz_profile_google_integration_enabled",
        code: 31246,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_PROFILE_GRAPHQL_MIGRATION: AbProp = AbProp {
        name: "wa_web_biz_profile_graphql_migration",
        code: 25846,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_PROFILE_GRAPHQL_MIGRATION_BYPASS_LID_CHECK_DOGFOODING: AbProp = AbProp {
        name: "wa_web_biz_profile_graphql_migration_bypass_lid_check_dogfooding",
        code: 29965,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BIZ_PROFILE_PRELOAD: AbProp = AbProp {
        name: "wa_web_biz_profile_preload",
        code: 31842,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BLOCKED_PARTICIPANT_CALL_WARNING: AbProp = AbProp {
        name: "wa_web_blocked_participant_call_warning",
        code: 29039,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BLOCKED_PARTICIPANT_CHAT_WARNING: AbProp = AbProp {
        name: "wa_web_blocked_participant_chat_warning",
        code: 29038,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BOT_ORPHAN_LOGIC_ENABLED: AbProp = AbProp {
        name: "wa_web_bot_orphan_logic_enabled",
        code: 29753,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BOT_TOS_CHECK_REFINIEMENT: AbProp = AbProp {
        name: "wa_web_bot_tos_check_refiniement",
        code: 28897,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_BROADCAST_DISAPPEARING_MESSAGES_FIX: AbProp = AbProp {
        name: "wa_web_broadcast_disappearing_messages_fix",
        code: 31499,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CALLING_DEEP_LINK_ERROR: AbProp = AbProp {
        name: "wa_web_calling_deep_link_error",
        code: 10051,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WA_WEB_CANONICAL_REG_RELOAD_ENABLED: AbProp = AbProp {
        name: "wa_web_canonical_reg_reload_enabled",
        code: 29472,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CANONICAL_WAM_FALCO_BUFFER_ENABLED: AbProp = AbProp {
        name: "wa_web_canonical_wam_falco_buffer_enabled",
        code: 30212,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CANONICAL_WAM_FALCO_BUFFER_SIZE: AbProp = AbProp {
        name: "wa_web_canonical_wam_falco_buffer_size",
        code: 30219,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2000),
    };
    pub const WA_WEB_CHANGE_LIST_WDS_SUBMENU: AbProp = AbProp {
        name: "wa_web_change_list_wds_submenu",
        code: 27123,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHANNELS_COMET_VIDEO_PLAYER_ENABLED_V2: AbProp = AbProp {
        name: "wa_web_channels_comet_video_player_enabled_v2",
        code: 24541,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHAT_OPEN_OPTIMIZATIONS: AbProp = AbProp {
        name: "wa_web_chat_open_optimizations",
        code: 31399,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHAT_SEARCH_ENTRYPOINT: AbProp = AbProp {
        name: "wa_web_chat_search_entrypoint",
        code: 25609,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHAT_THEMES: AbProp = AbProp {
        name: "wa_web_chat_themes",
        code: 26629,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHAT_THEMES_LOGGING: AbProp = AbProp {
        name: "wa_web_chat_themes_logging",
        code: 29457,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHAT_THEMES_SOLID_WALLPAPER_SYNC_ENCODE: AbProp = AbProp {
        name: "wa_web_chat_themes_solid_wallpaper_sync_encode",
        code: 32878,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHATLIST_RENDER_CHAT_OPEN: AbProp = AbProp {
        name: "wa_web_chatlist_render_chat_open",
        code: 27947,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CLEAR_SELECTED_CHATS_ENABLED: AbProp = AbProp {
        name: "wa_web_clear_selected_chats_enabled",
        code: 20626,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_COMET_VIDEO_PLAYER_SNAPL: AbProp = AbProp {
        name: "wa_web_comet_video_player_snapl",
        code: 25065,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WA_WEB_COMPOSER_HEIGHT_INCREASE_ENABLED: AbProp = AbProp {
        name: "wa_web_composer_height_increase_enabled",
        code: 27441,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CONSOLE_LOG_LEVEL: AbProp = AbProp {
        name: "wa_web_console_log_level",
        code: 16806,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_DISTANCE_THRESHOLD: AbProp = AbProp {
        name: "wa_web_contact_and_chat_fuzzy_search_distance_threshold",
        code: 26731,
        value_type: AbPropType::Float,
        default: AbDefault::Float(0.30000001192092896),
    };
    pub const WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_ENABLED: AbProp = AbProp {
        name: "wa_web_contact_and_chat_fuzzy_search_enabled",
        code: 26728,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_SIMILARITY_OPTIMIZATION_ENABLED: AbProp =
        AbProp {
            name: "wa_web_contact_and_chat_fuzzy_search_similarity_optimization_enabled",
            code: 26729,
            value_type: AbPropType::Bool,
            default: AbDefault::Bool(false),
        };
    pub const WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_TIMEOUT_THRESHOLD: AbProp = AbProp {
        name: "wa_web_contact_and_chat_fuzzy_search_timeout_threshold",
        code: 26733,
        value_type: AbPropType::Float,
        default: AbDefault::Float(5.0),
    };
    pub const WA_WEB_CONTACT_SEARCH_TOKENIZED_ENABLED: AbProp = AbProp {
        name: "wa_web_contact_search_tokenized_enabled",
        code: 24773,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CONTEXT_CARD_VERTICAL_BUTTONS: AbProp = AbProp {
        name: "wa_web_context_card_vertical_buttons",
        code: 31178,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_COPY_LINK_URL_ENABLED: AbProp = AbProp {
        name: "wa_web_copy_link_url_enabled",
        code: 25820,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CREATE_GROUP_IN_FILTER: AbProp = AbProp {
        name: "wa_web_create_group_in_filter",
        code: 22617,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_DEBUG_COLOR_CODE_RETRY_MESSAGES: AbProp = AbProp {
        name: "wa_web_debug_color_code_retry_messages",
        code: 16138,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_DEFAULT_PROFILE_PICS: AbProp = AbProp {
        name: "wa_web_default_profile_pics",
        code: 25455,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_DISABLE_PREFETCH_LOADABLES: AbProp = AbProp {
        name: "wa_web_disable_prefetch_loadables",
        code: 21917,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_DISCUSS_PRIVATELY: AbProp = AbProp {
        name: "wa_web_discuss_privately",
        code: 26815,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_DOWNLOAD_MIMETYPE_CHECK_BLOCK_ENABLED: AbProp = AbProp {
        name: "wa_web_download_mimetype_check_block_enabled",
        code: 26555,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_EDIT_BEFORE_FORWARDING_TO_STATUS: AbProp = AbProp {
        name: "wa_web_edit_before_forwarding_to_status",
        code: 27616,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_CHAT_THREAD_AND_INFO_STATUS_RING: AbProp = AbProp {
        name: "wa_web_enable_chat_thread_and_info_status_ring",
        code: 30026,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_FOLLOW_UP_REPLY_ICON: AbProp = AbProp {
        name: "wa_web_enable_follow_up_reply_icon",
        code: 24429,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_GRANULAR_NOTIFICATIONS: AbProp = AbProp {
        name: "wa_web_enable_granular_notifications",
        code: 21909,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_MENTION_MESSAGE: AbProp = AbProp {
        name: "wa_web_enable_mention_message",
        code: 27714,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_STATUS_HQ_THUMBNAIL: AbProp = AbProp {
        name: "wa_web_enable_status_hq_thumbnail",
        code: 25079,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ENABLE_SYNCD_KEY_PERSISTENCE_ONLY_AFTER_SERVER_ACK: AbProp = AbProp {
        name: "wa_web_enable_syncd_key_persistence_only_after_server_ack",
        code: 27069,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_EXPANSION_COUNTRIES_BONSAI_ENABLED: AbProp = AbProp {
        name: "wa_web_expansion_countries_bonsai_enabled",
        code: 29543,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_EXPORT_CHAT: AbProp = AbProp {
        name: "wa_web_export_chat",
        code: 26201,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FALCO_CLEAR_LOCAL_STORAGE_QUEUE_ENABLED: AbProp = AbProp {
        name: "wa_web_falco_clear_local_storage_queue_enabled",
        code: 18835,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FALCO_CONSOLE_LOGGER: AbProp = AbProp {
        name: "wa_web_falco_console_logger",
        code: 28054,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FAVICON_BADGING_ENABLED: AbProp = AbProp {
        name: "wa_web_favicon_badging_enabled",
        code: 22924,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FAVICONS_UPDATE_M1: AbProp = AbProp {
        name: "wa_web_favicons_update_m1",
        code: 14260,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FEATURE_PARITY_SMALL_WINS: AbProp = AbProp {
        name: "wa_web_feature_parity_small_wins",
        code: 26481,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FMX_AGM_ENABLED: AbProp = AbProp {
        name: "wa_web_fmx_agm_enabled",
        code: 13597,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FOCUS_MANAGEMENT_FOR_STATUS_AUDIENCE: AbProp = AbProp {
        name: "wa_web_focus_management_for_status_audience",
        code: 27719,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FORWARD_TO_SMALL_GROUPS: AbProp = AbProp {
        name: "wa_web_forward_to_small_groups",
        code: 27157,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_FREQUENT_REACTIONS_REACTS_AGO_THRESHOLD: AbProp = AbProp {
        name: "wa_web_frequent_reactions_reacts_ago_threshold",
        code: 27712,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
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
    pub const WA_WEB_GLOBAL_SEARCH_PREFIX_BASED: AbProp = AbProp {
        name: "wa_web_global_search_prefix_based",
        code: 24559,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_GROUP_DISCARD_DIALOG_CONTACT_THRESHOLD: AbProp = AbProp {
        name: "wa_web_group_discard_dialog_contact_threshold",
        code: 25682,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const WA_WEB_GROUP_INFO_NOTIFICATION_ROW: AbProp = AbProp {
        name: "wa_web_group_info_notification_row",
        code: 25292,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_GROUPS_IN_COMMON_MULTI_CONTACT: AbProp = AbProp {
        name: "wa_web_groups_in_common_multi_contact",
        code: 25808,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_GROWTH_EMPTY_STATE_UPSELL_VARIANT_M1: AbProp = AbProp {
        name: "wa_web_growth_empty_state_upsell_variant_m1",
        code: 15557,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const WA_WEB_HIGHLIGHT_ME_MENTION: AbProp = AbProp {
        name: "wa_web_highlight_me_mention",
        code: 25408,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_HIGHLIGHT_ME_MENTION_GROUPSIZE_THRESHOLD: AbProp = AbProp {
        name: "wa_web_highlight_me_mention_groupsize_threshold",
        code: 25836,
        value_type: AbPropType::Int,
        default: AbDefault::Int(130),
    };
    pub const WA_WEB_HISTORY_SYNC_DYNAMIC_THROTTLING: AbProp = AbProp {
        name: "wa_web_history_sync_dynamic_throttling",
        code: 19110,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WA_WEB_HORIZONTAL_LINK_PREVIEWS: AbProp = AbProp {
        name: "wa_web_horizontal_link_previews",
        code: 24425,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_HQ_IMAGE_THUMBNAIL_IN_CHAT_SCANS: AbProp = AbProp {
        name: "wa_web_hq_image_thumbnail_in_chat_scans",
        code: 27512,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_HYBRID_CONTEXT_MENU_REACTIONS_ENABLED: AbProp = AbProp {
        name: "wa_web_hybrid_context_menu_reactions_enabled",
        code: 17650,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_HYBRID_SIMPLE_CHAT_CONVERSATION_CONTEXT_MENU_ENABLED: AbProp = AbProp {
        name: "wa_web_hybrid_simple_chat_conversation_context_menu_enabled",
        code: 17479,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_IMAGINE_UR_ENABLED: AbProp = AbProp {
        name: "wa_web_imagine_ur_enabled",
        code: 25331,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_IMPORTANT_MSG_NOTIFICATION: AbProp = AbProp {
        name: "wa_web_important_msg_notification",
        code: 27614,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_INVITE_LINK_PAGE_ENHANCEMENTS: AbProp = AbProp {
        name: "wa_web_invite_link_page_enhancements",
        code: 31210,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_JUMP_TO_CART: AbProp = AbProp {
        name: "wa_web_jump_to_cart",
        code: 27939,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_LARGE_GROUP_PRESENCE_ENABLED: AbProp = AbProp {
        name: "wa_web_large_group_presence_enabled",
        code: 29279,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_LISTS_FULL_WIDTH_FILTERS: AbProp = AbProp {
        name: "wa_web_lists_full_width_filters",
        code: 25805,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_LISTS_M1_ENABLED: AbProp = AbProp {
        name: "wa_web_lists_m1_enabled",
        code: 22090,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_LISTS_M2_ENABLED: AbProp = AbProp {
        name: "wa_web_lists_m2_enabled",
        code: 22086,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_LOADER_BUTTON_UIX_IMPROVEMENT: AbProp = AbProp {
        name: "wa_web_loader_button_uix_improvement",
        code: 27768,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_MATCH_PRIMARY_ICONS: AbProp = AbProp {
        name: "wa_web_match_primary_icons",
        code: 29293,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_ME_TAB: AbProp = AbProp {
        name: "wa_web_me_tab",
        code: 24944,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_MEDIA_UPLOAD_RETRY_RETRIES_COUNT: AbProp = AbProp {
        name: "wa_web_media_upload_retry_retries_count",
        code: 27782,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_MENTION_SEARCH: AbProp = AbProp {
        name: "wa_web_mention_search",
        code: 28455,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_MULTI_PPL_TYPING_INDICATOR_FOR_CHATLIST_GROUPS_VARIANT: AbProp = AbProp {
        name: "wa_web_multi_ppl_typing_indicator_for_chatlist_groups_variant",
        code: 24560,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_NOTIFICATIONS_MODAL: AbProp = AbProp {
        name: "wa_web_notifications_modal",
        code: 32228,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_NOTIFICATIONS_MODAL_VARIANTS: AbProp = AbProp {
        name: "wa_web_notifications_modal_variants",
        code: 32277,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_NOTIFY_FOR: AbProp = AbProp {
        name: "wa_web_notify_for",
        code: 25544,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_PATHFINDER_UNSAMPLING_CONFIG: AbProp = AbProp {
        name: "wa_web_pathfinder_unsampling_config",
        code: 32631,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"schema_version\":1,\"session_flag_rules\":[]}"),
    };
    pub const WA_WEB_PRE_CHAT_DEVICE_ID_TEST: AbProp = AbProp {
        name: "wa_web_pre_chat_device_id_test",
        code: 26553,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_PRELOAD_CONVERSATION_CHAT_OPEN: AbProp = AbProp {
        name: "wa_web_preload_conversation_chat_open",
        code: 25937,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_PTT_LOADER_BUTTON_UIX_IMPROVEMENT: AbProp = AbProp {
        name: "wa_web_ptt_loader_button_uix_improvement",
        code: 32418,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_PUSH_NAME_IN_GLOBAL_SEARCH_NON_CONTACTS_ENABLED: AbProp = AbProp {
        name: "wa_web_push_name_in_global_search_non_contacts_enabled",
        code: 28506,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_QUICK_REACTIONS: AbProp = AbProp {
        name: "wa_web_quick_reactions",
        code: 28621,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_REACTIONS_2: AbProp = AbProp {
        name: "wa_web_reactions_2",
        code: 22469,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_REACTIONS_MOTION_V2_ENABLED: AbProp = AbProp {
        name: "wa_web_reactions_motion_v2_enabled",
        code: 26102,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_RECONNECT_ANR: AbProp = AbProp {
        name: "wa_web_reconnect_anr",
        code: 31467,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_REDUCE_CASCADING_UPDATES_CHAT_OPEN: AbProp = AbProp {
        name: "wa_web_reduce_cascading_updates_chat_open",
        code: 25006,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_REDUCE_FORCED_LAYOUT_CHAT_OPEN: AbProp = AbProp {
        name: "wa_web_reduce_forced_layout_chat_open",
        code: 24526,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_RESHARE_POSTER_SIDE_ENABLED: AbProp = AbProp {
        name: "wa_web_reshare_poster_side_enabled",
        code: 28732,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_RICH_RESPONSE_REPLYING_ENABLED: AbProp = AbProp {
        name: "wa_web_rich_response_replying_enabled",
        code: 30493,
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
    pub const WA_WEB_SEARCH_EMPTY_STATE_M1: AbProp = AbProp {
        name: "wa_web_search_empty_state_m1",
        code: 25310,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SELECT_ALL_CHATS_ENABLED: AbProp = AbProp {
        name: "wa_web_select_all_chats_enabled",
        code: 30040,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SELF_PROFILE_PHOTO_FIX_ENABLED: AbProp = AbProp {
        name: "wa_web_self_profile_photo_fix_enabled",
        code: 24945,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SHARE_CONTENT_UJ: AbProp = AbProp {
        name: "wa_web_share_content_uj",
        code: 22813,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SHOW_HD_PHOTO: AbProp = AbProp {
        name: "wa_web_show_hd_photo",
        code: 26610,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SHOW_STATUS_RING_FOR_NO_UNREAD: AbProp = AbProp {
        name: "wa_web_show_status_ring_for_no_unread",
        code: 22567,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_SMALL_GROUP_PRESENCE_ENABLED: AbProp = AbProp {
        name: "wa_web_small_group_presence_enabled",
        code: 29280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_STARRED_MSGS_SEARCH: AbProp = AbProp {
        name: "wa_web_starred_msgs_search",
        code: 27353,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_STATUS_COMET_VIDEO_PLAYER_ENABLED: AbProp = AbProp {
        name: "wa_web_status_comet_video_player_enabled",
        code: 24791,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_STATUS_FIRST_UPLOAD_FIX_ENABLED: AbProp = AbProp {
        name: "wa_web_status_first_upload_fix_enabled",
        code: 25015,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_STATUS_QUESTION_STICKER_REPLY_ENABLED: AbProp = AbProp {
        name: "wa_web_status_question_sticker_reply_enabled",
        code: 30495,
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
    pub const WA_WEB_STATUS_RESHARER_FLOW_ENABLED: AbProp = AbProp {
        name: "wa_web_status_resharer_flow_enabled",
        code: 28812,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_STATUS_VIEWER_SIDE_POSTER_IDENTIFIERS_ENABLED: AbProp = AbProp {
        name: "wa_web_status_viewer_side_poster_identifiers_enabled",
        code: 25151,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_UR_BLOKS_ENABLED: AbProp = AbProp {
        name: "wa_web_ur_bloks_enabled",
        code: 25332,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_UR_IMAGINE_VIDEO_ENABLED: AbProp = AbProp {
        name: "wa_web_ur_imagine_video_enabled",
        code: 25329,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_VELOCITY_ANIMATE_MIGRATION_ENABLED: AbProp = AbProp {
        name: "wa_web_velocity_animate_migration_enabled",
        code: 31784,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_VIDEO_COMET_VIDEO_PLAYER_ENABLED: AbProp = AbProp {
        name: "wa_web_video_comet_video_player_enabled",
        code: 24905,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_VOIP_ADAPTIVE_GRID_PAGE_SIZE: AbProp = AbProp {
        name: "wa_web_voip_adaptive_grid_page_size",
        code: 28909,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_VOIP_STACK_LOG_LEVEL: AbProp = AbProp {
        name: "wa_web_voip_stack_log_level",
        code: 30261,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const WA_WEB_WAE_QPL_ENABLED: AbProp = AbProp {
        name: "wa_web_wae_qpl_enabled",
        code: 21742,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WA_WEB_WAM_FALCO_CRITICAL_EVENT_IDS: AbProp = AbProp {
        name: "wa_web_wam_falco_critical_event_ids",
        code: 32632,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WA_WEB_WAM_FALCO_FLUSH_INTERVAL_MS: AbProp = AbProp {
        name: "wa_web_wam_falco_flush_interval_ms",
        code: 32393,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3000),
    };
    pub const WA_WEB_WAM_FALCO_LOGGING_ENABLED: AbProp = AbProp {
        name: "wa_web_wam_falco_logging_enabled",
        code: 26200,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_WAM_FALCO_MODE: AbProp = AbProp {
        name: "wa_web_wam_falco_mode",
        code: 25306,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEB_WAM_FALCO_SHADOW_EVENT_IDS: AbProp = AbProp {
        name: "wa_web_wam_falco_shadow_event_ids",
        code: 25309,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WA_WEB_XB_BUBBLE_ENABLED: AbProp = AbProp {
        name: "wa_web_xb_bubble_enabled",
        code: 32818,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_EDIT_PDF_IN_WHATSAPP_ENABLED: AbProp = AbProp {
        name: "wa_webtp_edit_pdf_in_whatsapp_enabled",
        code: 26279,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_PDF_RENDERER_MODE_NO_EXPOSURE: AbProp = AbProp {
        name: "wa_webtp_pdf_renderer_mode_no_exposure",
        code: 27941,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WA_WEBTP_PDF_SHARER_CONSENT_COPY_V2: AbProp = AbProp {
        name: "wa_webtp_pdf_sharer_consent_copy_v2",
        code: 30771,
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
    pub const WA_WEBTP_THUMBNAIL_RENDERER_TIMEOUT_MS: AbProp = AbProp {
        name: "wa_webtp_thumbnail_renderer_timeout_ms",
        code: 27148,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3000),
    };
    pub const WA_WEBTP_USE_ASYNC_PDF_SEND: AbProp = AbProp {
        name: "wa_webtp_use_async_pdf_send",
        code: 30214,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_USE_PDF_ANNOTATIONS: AbProp = AbProp {
        name: "wa_webtp_use_pdf_annotations",
        code: 32144,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_USE_PDF_EDITOR: AbProp = AbProp {
        name: "wa_webtp_use_pdf_editor",
        code: 23498,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_USE_PDF_RENDERER: AbProp = AbProp {
        name: "wa_webtp_use_pdf_renderer",
        code: 20607,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEBTP_USE_THUMBNAIL_RENDERER: AbProp = AbProp {
        name: "wa_webtp_use_thumbnail_renderer",
        code: 20555,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WIN_PDF_RENDERING_ENABLED: AbProp = AbProp {
        name: "wa_win_pdf_rendering_enabled",
        code: 29548,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WABAI_CONSENT_COOLDOWN: AbProp = AbProp {
        name: "wabai_consent_cooldown",
        code: 5746,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const WABAI_CONSENT_REQUIRED: AbProp = AbProp {
        name: "wabai_consent_required",
        code: 5747,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WABAI_MESSAGE_FEEDBACK_ENABLED: AbProp = AbProp {
        name: "wabai_message_feedback_enabled",
        code: 5215,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WABAI_MESSAGE_RENDERING_ENABLED: AbProp = AbProp {
        name: "wabai_message_rendering_enabled",
        code: 4873,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WABBA_RECEIVER_ENABLED: AbProp = AbProp {
        name: "wabba_receiver_enabled",
        code: 10970,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WABBA_SAVE_TO_CAMERA_ROLL_ENABLED: AbProp = AbProp {
        name: "wabba_save_to_camera_roll_enabled",
        code: 13114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAE_METADATA_INTEGRITY_TIMEOUT_MINUTES: AbProp = AbProp {
        name: "wae_metadata_integrity_timeout_minutes",
        code: 4849,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const WAM_DISABLE_ABKEY_ATTRIBUTE: AbProp = AbProp {
        name: "wam_disable_abkey_attribute",
        code: 12390,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAM_DISABLE_EXPOKEY_ATTRIBUTE: AbProp = AbProp {
        name: "wam_disable_expokey_attribute",
        code: 12391,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_AGM_ENABLED: AbProp = AbProp {
        name: "wamo_agm_enabled",
        code: 15714,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_PRIVACY_TOS_LINKED_HIGHLIGHTED_NOTICE_ID: AbProp = AbProp {
        name: "wamo_privacy_tos_linked_highlighted_notice_id",
        code: 14985,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610204"),
    };
    pub const WAMO_PRIVACY_TOS_SHOW_CHANNELS_NUX_ENABLED: AbProp = AbProp {
        name: "wamo_privacy_tos_show_channels_nux_enabled",
        code: 15254,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WAMO_PRIVACY_TOS_UNLINKED_HIGHLIGHTED_NOTICE_ID: AbProp = AbProp {
        name: "wamo_privacy_tos_unlinked_highlighted_notice_id",
        code: 14987,
        value_type: AbPropType::Str,
        default: AbDefault::Str("20610203"),
    };
    pub const WAMO_SUB_ADMIN_ENABLED_V2: AbProp = AbProp {
        name: "wamo_sub_admin_enabled_v2",
        code: 11020,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_SUB_CONSUMER_ENABLED_V2: AbProp = AbProp {
        name: "wamo_sub_consumer_enabled_v2",
        code: 11021,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_SUB_LOGGING_ENABLED_V2: AbProp = AbProp {
        name: "wamo_sub_logging_enabled_v2",
        code: 11017,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_SUB_MESSAGES_SUPPORTED: AbProp = AbProp {
        name: "wamo_sub_messages_supported",
        code: 11062,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAMO_SUB_PROCESS_MESSAGE_KILL_SWITCH: AbProp = AbProp {
        name: "wamo_sub_process_message_kill_switch",
        code: 12722,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WAVOIP_ENABLE_ML_NAMESPACE_V2: AbProp = AbProp {
        name: "wavoip_enable_ml_namespace_v2",
        code: 26947,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAVOIP_LEGACY_ML_QPL_EXP_TAG: AbProp = AbProp {
        name: "wavoip_legacy_ml_qpl_exp_tag",
        code: 30561,
        value_type: AbPropType::Str,
        default: AbDefault::Str("none"),
    };
    pub const WAVOIP_ML_BWE_CONG_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_cong_model_download_versions",
        code: 21732,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_CONG_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_cong_model_download_versions_v2",
        code: 27991,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_GC_HD_TARGET_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_gc_hd_target_model_download_versions",
        code: 21822,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_GC_HD_TARGET_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_gc_hd_target_model_download_versions_v2",
        code: 28021,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_GC_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_gc_undershoot_model_download_versions",
        code: 21821,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_GC_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_gc_undershoot_model_download_versions_v2",
        code: 28019,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_HD_TARGET_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_hd_target_model_download_versions",
        code: 21738,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_HD_TARGET_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_hd_target_model_download_versions_v2",
        code: 27990,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_PLC_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_plc_model_download_versions",
        code: 5228,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_PLC_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_plc_model_download_versions_v2",
        code: 27998,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_QUICKHD_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_quickhd_model_download_versions",
        code: 27109,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_RL_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_rl_model_download_versions",
        code: 21733,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_TR_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_tr_model_download_versions",
        code: 21734,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_TR_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_bwe_tr_model_download_versions_v2",
        code: 27996,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_BWE_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_bwe_undershoot_model_download_versions",
        code: 5231,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_MEDIA_AUTOMOS_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_media_automos_model_download_versions",
        code: 21731,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_MEDIA_NS_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_media_ns_model_download_versions",
        code: 21737,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_MEDIA_VMOS_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_media_vmos_model_download_versions",
        code: 21736,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_MEDIA_VSR_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_media_vsr_model_download_versions",
        code: 21735,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_NADL_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_nadl_model_download_versions",
        code: 24174,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_NADL_MODEL_DOWNLOAD_VERSIONS_V2: AbProp = AbProp {
        name: "wavoip_ml_nadl_model_download_versions_v2",
        code: 28015,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_QPL_EXP_TAG: AbProp = AbProp {
        name: "wavoip_ml_qpl_exp_tag",
        code: 30539,
        value_type: AbPropType::Str,
        default: AbDefault::Str("none"),
    };
    pub const WAVOIP_ML_TEMP_MODEL_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_temp_model_download_versions",
        code: 21815,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAVOIP_ML_TRANSPORT_DOWNLOAD_VERSIONS: AbProp = AbProp {
        name: "wavoip_ml_transport_download_versions",
        code: 24173,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WAWEB_CHATINFO_REFRESH: AbProp = AbProp {
        name: "waweb_chatinfo_refresh",
        code: 23018,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAWEB_CROSSPOSTING_ATTRIBUTIONS: AbProp = AbProp {
        name: "waweb_crossposting_attributions",
        code: 26138,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAWEB_ENABLE_LEGACY_IMAGE_ZOOM: AbProp = AbProp {
        name: "waweb_enable_legacy_image_zoom",
        code: 27239,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WAWEB_STATUS_CLOSE_FRIENDS_VIEWER_SIDE_ENABLED: AbProp = AbProp {
        name: "waweb_status_close_friends_viewer_side_enabled",
        code: 26659,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_RADIUS_AND_CASING: AbProp = AbProp {
        name: "wds_radius_and_casing",
        code: 3350,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_ACTION_TILE_REFRESH: AbProp = AbProp {
        name: "wds_web_action_tile_refresh",
        code: 28564,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_BADGE: AbProp = AbProp {
        name: "wds_web_badge",
        code: 27856,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_CHIP: AbProp = AbProp {
        name: "wds_web_chip",
        code: 20970,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_COMPOSER_TOOLBAR_V2: AbProp = AbProp {
        name: "wds_web_composer_toolbar_v2",
        code: 26773,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_DIALOG: AbProp = AbProp {
        name: "wds_web_dialog",
        code: 28557,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_EXPRESSIONS_PANEL: AbProp = AbProp {
        name: "wds_web_expressions_panel",
        code: 25144,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_MENU_REACTION_DETAIL_PANEL_V2: AbProp = AbProp {
        name: "wds_web_menu_reaction_detail_panel_v2",
        code: 30694,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_PROFILE_PHOTO: AbProp = AbProp {
        name: "wds_web_profile_photo",
        code: 27954,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_RICH_TEXT_FIELD: AbProp = AbProp {
        name: "wds_web_rich_text_field",
        code: 27264,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_SUBMENUS: AbProp = AbProp {
        name: "wds_web_submenus",
        code: 25351,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_TEXT_LAYOUT: AbProp = AbProp {
        name: "wds_web_text_layout",
        code: 31789,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WDS_WEB_TOAST: AbProp = AbProp {
        name: "wds_web_toast",
        code: 23486,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_BLOCK_CATALOG_CREATION_ECOMMERCE_COMPLIANCE_INDIA: AbProp = AbProp {
        name: "web_abprop_block_catalog_creation_ecommerce_compliance_india",
        code: 894,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_BUSINESS_PROFILE_REFRESH_LINKED_ACCOUNT_ENABLED: AbProp = AbProp {
        name: "web_abprop_business_profile_refresh_linked_account_enabled",
        code: 764,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_BUSINESS_PROFILE_REFRESH_LINKED_ACCOUNTS_KILLSWITCH: AbProp = AbProp {
        name: "web_abprop_business_profile_refresh_linked_accounts_killswitch",
        code: 1351,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_COLLECTIONS_NUX_BANNER: AbProp = AbProp {
        name: "web_abprop_collections_nux_banner",
        code: 741,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_CORE_WAM_RUNTIME: AbProp = AbProp {
        name: "web_abprop_core_wam_runtime",
        code: 1753,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_DIRECT_CONNECTION_MD: AbProp = AbProp {
        name: "web_abprop_direct_connection_md",
        code: 869,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_DROP_FULL_HISTORY_SYNC: AbProp = AbProp {
        name: "web_abprop_drop_full_history_sync",
        code: 600,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_MEDIA_LINKS_DOCS_SEARCH: AbProp = AbProp {
        name: "web_abprop_media_links_docs_search",
        code: 2063,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ABPROP_SCREEN_LOCK_ENABLED: AbProp = AbProp {
        name: "web_abprop_screen_lock_enabled",
        code: 1680,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ADD_CONTACT: AbProp = AbProp {
        name: "web_add_contact",
        code: 26892,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WEB_ADV_LOGOUT_ON_SELF_DEVICE_LIST_EXPIRED: AbProp = AbProp {
        name: "web_adv_logout_on_self_device_list_expired",
        code: 11011,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_AI_GROUP_OPEN_SUPPORT: AbProp = AbProp {
        name: "web_ai_group_open_support",
        code: 23530,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_ASYNC_CONTACTS_RESTORE_FROM_DB_ENABLED: AbProp = AbProp {
        name: "web_anr_async_contacts_restore_from_db_enabled",
        code: 27775,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_ASYNC_MEDIA_DECRYPTION_ENABLED: AbProp = AbProp {
        name: "web_anr_async_media_decryption_enabled",
        code: 23200,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_ASYNC_MSG_SEND_HANDLER: AbProp = AbProp {
        name: "web_anr_async_msg_send_handler",
        code: 27249,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_ASYNC_NATIVE_APP_STATE_BRIDGE_ENABLED: AbProp = AbProp {
        name: "web_anr_async_native_app_state_bridge_enabled",
        code: 29551,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_ASYNC_SQLITE_BRIDGE_OPERATIONS: AbProp = AbProp {
        name: "web_anr_async_sqlite_bridge_operations",
        code: 29460,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_BATCH_AND_QUEUE_BULK_CONTACTS_DB_WRITES_ENABLED: AbProp = AbProp {
        name: "web_anr_batch_and_queue_bulk_contacts_db_writes_enabled",
        code: 25413,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_BATCH_PROFILE_PICTURE_BRIDGE_OPERATIONS: AbProp = AbProp {
        name: "web_anr_batch_profile_picture_bridge_operations",
        code: 29122,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_DISABLE_MEMORY_LOGGING: AbProp = AbProp {
        name: "web_anr_disable_memory_logging",
        code: 31047,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_FILE_SIZE_THRESHOLD_TO_USE_WORKER_MB: AbProp = AbProp {
        name: "web_anr_file_size_threshold_to_use_worker_mb",
        code: 22930,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_ANR_GROUP_METADATA_YIELD: AbProp = AbProp {
        name: "web_anr_group_metadata_yield",
        code: 29294,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_MEDIA_CHUNK_ENC_DELAY_ENABLED: AbProp = AbProp {
        name: "web_anr_media_chunk_enc_delay_enabled",
        code: 22931,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_NOOP_GC_ENABLED: AbProp = AbProp {
        name: "web_anr_noop_gc_enabled",
        code: 25915,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_OPTIMIZED_INITIAL_CONTACTS_SYNC_ENABLED: AbProp = AbProp {
        name: "web_anr_optimized_initial_contacts_sync_enabled",
        code: 30227,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_PRUNE_CMC: AbProp = AbProp {
        name: "web_anr_prune_cmc",
        code: 29060,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_SKIP_UNUSED_CONTACTS_DB_UPDATES_ENABLED: AbProp = AbProp {
        name: "web_anr_skip_unused_contacts_db_updates_enabled",
        code: 30043,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_SPINNER_GPU_ANIMATION: AbProp = AbProp {
        name: "web_anr_spinner_gpu_animation",
        code: 29405,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ANR_THROTTLE_SIGNAL_SNAPSHOT_ENABLED: AbProp = AbProp {
        name: "web_anr_throttle_signal_snapshot_enabled",
        code: 28890,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ATTACH_MENU_ADD_DRAWING_ENABLED: AbProp = AbProp {
        name: "web_attach_menu_add_drawing_enabled",
        code: 24384,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_AUTODOWNLOAD_STICKERS: AbProp = AbProp {
        name: "web_autodownload_stickers",
        code: 7422,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BACKGROUND_SYNC_V2: AbProp = AbProp {
        name: "web_background_sync_v2",
        code: 8782,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BB_GENAI_COMPOSER_MIN_WORDS: AbProp = AbProp {
        name: "web_bb_genai_composer_min_words",
        code: 32054,
        value_type: AbPropType::Int,
        default: AbDefault::Int(4),
    };
    pub const WEB_BIZ_PROFILE_OPTIONS: AbProp = AbProp {
        name: "web_biz_profile_options",
        code: 14881,
        value_type: AbPropType::Int,
        default: AbDefault::Int(116),
    };
    pub const WEB_BIZ_QUALITY_TELEMETRY_ENABLED: AbProp = AbProp {
        name: "web_biz_quality_telemetry_enabled",
        code: 27855,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_CLICKS_ENABLED: AbProp = AbProp {
        name: "web_biz_quality_telemetry_message_clicks_enabled",
        code: 27854,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_LEVEL_ACTIONS_ENABLED: AbProp = AbProp {
        name: "web_biz_quality_telemetry_message_level_actions_enabled",
        code: 28590,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_READS_ENABLED: AbProp = AbProp {
        name: "web_biz_quality_telemetry_message_reads_enabled",
        code: 28574,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BIZ_SIMPLE_SIGNAL_ENABLED: AbProp = AbProp {
        name: "web_biz_simple_signal_enabled",
        code: 28573,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_BIZ_SIMPLE_SIGNAL_GROUP_ENABLED: AbProp = AbProp {
        name: "web_biz_simple_signal_group_enabled",
        code: 28679,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BOT_PROFILE_GQL_MIGRATION_ENABLED: AbProp = AbProp {
        name: "web_bot_profile_gql_migration_enabled",
        code: 28941,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BOT_PROFILE_PIC_GQL_MIGRATION_ENABLED: AbProp = AbProp {
        name: "web_bot_profile_pic_gql_migration_enabled",
        code: 30597,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BROWSER_MIN_STORAGE_QUOTA: AbProp = AbProp {
        name: "web_browser_min_storage_quota",
        code: 3135,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const WEB_BROWSER_QUOTA_THRESHOLD: AbProp = AbProp {
        name: "web_browser_quota_threshold",
        code: 3134,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const WEB_BUG_REPORTING_REQUEST_PEER_LOG_ENABLED: AbProp = AbProp {
        name: "web_bug_reporting_request_peer_log_enabled",
        code: 30485,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BULK_ADD_CONTACTS_ENABLED: AbProp = AbProp {
        name: "web_bulk_add_contacts_enabled",
        code: 24875,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_CUSTOM_USER_PROMPT_ENABLED: AbProp = AbProp {
        name: "web_business_broadcast_genai_custom_user_prompt_enabled",
        code: 32052,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_TEXT: AbProp = AbProp {
        name: "web_business_broadcast_genai_text",
        code: 32050,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_TEXT_LANGUAGES: AbProp = AbProp {
        name: "web_business_broadcast_genai_text_languages",
        code: 32117,
        value_type: AbPropType::Str,
        default: AbDefault::Str("en,es"),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_TEXT_MAX_TRIES: AbProp = AbProp {
        name: "web_business_broadcast_genai_text_max_tries",
        code: 32053,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_TEXT_MODEL: AbProp = AbProp {
        name: "web_business_broadcast_genai_text_model",
        code: 32051,
        value_type: AbPropType::Str,
        default: AbDefault::Str("LLAMA"),
    };
    pub const WEB_BUSINESS_BROADCAST_GENAI_TEXT_NO_EXP: AbProp = AbProp {
        name: "web_business_broadcast_genai_text_no_exp",
        code: 32055,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_BUSINESS_TOOLS_DRAWER_ENABLED: AbProp = AbProp {
        name: "web_business_tools_drawer_enabled",
        code: 6803,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CACHE_OPEN_FAILED_RELOAD_FLOW_ENABLED: AbProp = AbProp {
        name: "web_cache_open_failed_reload_flow_enabled",
        code: 22155,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CALENDAR_MESSAGE_DENSITY_ENABLED: AbProp = AbProp {
        name: "web_calendar_message_density_enabled",
        code: 25823,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CALLING_AUTO_POPOUT_VIDEO: AbProp = AbProp {
        name: "web_calling_auto_popout_video",
        code: 28046,
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
    pub const WEB_CALLING_OFFLINE_RESUME_ORDERING: AbProp = AbProp {
        name: "web_calling_offline_resume_ordering",
        code: 29564,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CALLING_PERF_OPTIMIZATIONS_BITMASK: AbProp = AbProp {
        name: "web_calling_perf_optimizations_bitmask",
        code: 22186,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const WEB_CALLING_SPEAKER_STRIP_RESIZE_ENABLED: AbProp = AbProp {
        name: "web_calling_speaker_strip_resize_enabled",
        code: 30928,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CALLS_TAB_EMPTY_STATE_BUTTONS: AbProp = AbProp {
        name: "web_calls_tab_empty_state_buttons",
        code: 17724,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CATALOG_RECOVERY_FLOW_ENABLED: AbProp = AbProp {
        name: "web_catalog_recovery_flow_enabled",
        code: 14294,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CATALOG_VIEWING_VARIANTS_ENABLED: AbProp = AbProp {
        name: "web_catalog_viewing_variants_enabled",
        code: 15534,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHANNEL_STATUS_LIKES_SENDING_ENABLED: AbProp = AbProp {
        name: "web_channel_status_likes_sending_enabled",
        code: 32428,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHANNEL_VIDEO_SERVER_TRANSCODE_UPLOAD: AbProp = AbProp {
        name: "web_channel_video_server_transcode_upload",
        code: 19920,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHAT_INFO_ACTION_BUTTONS_REFRESH: AbProp = AbProp {
        name: "web_chat_info_action_buttons_refresh",
        code: 14664,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHAT_THEME_DRAWER_TITLE: AbProp = AbProp {
        name: "web_chat_theme_drawer_title",
        code: 28157,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHATPSA_FORWARDING: AbProp = AbProp {
        name: "web_chatpsa_forwarding",
        code: 23695,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CHATS_CONTENT_VISIBILITY: AbProp = AbProp {
        name: "web_chats_content_visibility",
        code: 31259,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_COEX_SIMPLE_SIGNAL_ENABLED: AbProp = AbProp {
        name: "web_coex_simple_signal_enabled",
        code: 30577,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_COMMS_SOCKET_RECONNECT_ENABLED: AbProp = AbProp {
        name: "web_comms_socket_reconnect_enabled",
        code: 7854,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_COMMUNITIES_GENERAL_CHAT_V_2: AbProp = AbProp {
        name: "web_communities_general_chat_v_2",
        code: 8580,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CONFIGURABLE_QUICK_ACTIONS_M1: AbProp = AbProp {
        name: "web_configurable_quick_actions_m1",
        code: 29874,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CONFIGURABLE_QUICK_ACTIONS_M1_CHANNELS: AbProp = AbProp {
        name: "web_configurable_quick_actions_m1_channels",
        code: 31781,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CONFIGURABLE_QUICK_ACTIONS_M1_COMMUNITIES: AbProp = AbProp {
        name: "web_configurable_quick_actions_m1_communities",
        code: 31782,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CONTACT_COLLECTION_LOCALE_LISTENER: AbProp = AbProp {
        name: "web_contact_collection_locale_listener",
        code: 31103,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CONTACT_SORT_LETTERS_FIRST: AbProp = AbProp {
        name: "web_contact_sort_letters_first",
        code: 28962,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const WEB_CONVERSATION_CLEANUP_TEMP_COLLECTION: AbProp = AbProp {
        name: "web_conversation_cleanup_temp_collection",
        code: 30829,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_CROSSPOST_SETTINGS_SYNC: AbProp = AbProp {
        name: "web_crosspost_settings_sync",
        code: 26296,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_DATE_MARKER_CALENDAR_ENABLED: AbProp = AbProp {
        name: "web_date_marker_calendar_enabled",
        code: 25811,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DEPRECATE_MMS4_HASH_BASED_DOWNLOAD: AbProp = AbProp {
        name: "web_deprecate_mms4_hash_based_download",
        code: 3152,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DESIGN_REFRESH: AbProp = AbProp {
        name: "web_design_refresh",
        code: 6665,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DEXIE_HOOKS_SUPPORT_ENABLED: AbProp = AbProp {
        name: "web_dexie_hooks_support_enabled",
        code: 12831,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DISABLE_COMPOSE_BOX_FOR_DEPRECATED_CHATS: AbProp = AbProp {
        name: "web_disable_compose_box_for_deprecated_chats",
        code: 30753,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DISABLE_LOGS_LOW_END_DEVICE: AbProp = AbProp {
        name: "web_disable_logs_low_end_device",
        code: 18660,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DISABLE_SW_ON_SAFARI_PWA: AbProp = AbProp {
        name: "web_disable_sw_on_safari_pwa",
        code: 7281,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DISPLAY_LID_CONTACTS: AbProp = AbProp {
        name: "web_display_lid_contacts",
        code: 24280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_DRAWER_DESCRIPTOR_ENABLED: AbProp = AbProp {
        name: "web_drawer_descriptor_enabled",
        code: 27677,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_E2E_BACKFILL_EXPIRE_TIME: AbProp = AbProp {
        name: "web_e2e_backfill_expire_time",
        code: 3234,
        value_type: AbPropType::Int,
        default: AbDefault::Int(5),
    };
    pub const WEB_ENABLE_BIZ_CATALOG_VIEW_PS_LOGGING: AbProp = AbProp {
        name: "web_enable_biz_catalog_view_ps_logging",
        code: 2056,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WEB_ENABLE_CAMERA_CAPTURE_REFRESH: AbProp = AbProp {
        name: "web_enable_camera_capture_refresh",
        code: 28316,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ENABLE_IMPROVED_BULK_MERGE: AbProp = AbProp {
        name: "web_enable_improved_bulk_merge",
        code: 19854,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ENABLE_PROFILE_PIC_THUMB_DB_CACHING: AbProp = AbProp {
        name: "web_enable_profile_pic_thumb_db_caching",
        code: 2018,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_EVICT_THUMBNAIL_HQ_ON_INACTIVE: AbProp = AbProp {
        name: "web_evict_thumbnail_hq_on_inactive",
        code: 32702,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_EVOLVE_ABOUT_SEND_ENABLED: AbProp = AbProp {
        name: "web_evolve_about_send_enabled",
        code: 5347,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_FIX_DUPLICATED_LIDS_HISTORY_SYNC: AbProp = AbProp {
        name: "web_fix_duplicated_lids_history_sync",
        code: 19994,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_FORCE_LID_CHATS_IN_HISTORY: AbProp = AbProp {
        name: "web_force_lid_chats_in_history",
        code: 24343,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WEB_FREQUENTLY_CONTACTED_ENABLED: AbProp = AbProp {
        name: "web_frequently_contacted_enabled",
        code: 29063,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const WEB_GET_MSG_EXIST_OPTMISE: AbProp = AbProp {
        name: "web_get_msg_exist_optmise",
        code: 29880,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_GETTERS_LRU_CACHE_SIZE_LIMIT: AbProp = AbProp {
        name: "web_getters_lru_cache_size_limit",
        code: 30796,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_GROUP_BULK_ADD_CONTACT: AbProp = AbProp {
        name: "web_group_bulk_add_contact",
        code: 30417,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_GROUP_EXPERIMENTATION_ENABLE: AbProp = AbProp {
        name: "web_group_experimentation_enable",
        code: 25414,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_GROUP_HOVER_CARD_VARIANT: AbProp = AbProp {
        name: "web_group_hover_card_variant",
        code: 30260,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_GROUP_PROFILE_EDITOR: AbProp = AbProp {
        name: "web_group_profile_editor",
        code: 1745,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WEB_GUEST_CALLING_REPRESENTATION_ENABLED: AbProp = AbProp {
        name: "web_guest_calling_representation_enabled",
        code: 31533,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_HISTORY_SYNC_ALLOW_DUPLICATE_IN_BULK_ERROR: AbProp = AbProp {
        name: "web_history_sync_allow_duplicate_in_bulk_error",
        code: 10842,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_HISTORY_SYNC_WORKER_ENABLED: AbProp = AbProp {
        name: "web_history_sync_worker_enabled",
        code: 24147,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_HYBRID_APPLY_LATEST_DB_SCHEMA_OPTIMIZATION_ENABLED: AbProp = AbProp {
        name: "web_hybrid_apply_latest_db_schema_optimization_enabled",
        code: 23595,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_IMAGE_MAX_EDGE: AbProp = AbProp {
        name: "web_image_max_edge",
        code: 3042,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1600),
    };
    pub const WEB_IMAGE_MAX_HD_EDGE: AbProp = AbProp {
        name: "web_image_max_hd_edge",
        code: 3204,
        value_type: AbPropType::Int,
        default: AbDefault::Int(2560),
    };
    pub const WEB_INIT_CHAT_BATCH_SIZE: AbProp = AbProp {
        name: "web_init_chat_batch_size",
        code: 1171,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const WEB_INIT_CHAT_MAX_UNREAD_MESSAGE_COUNT: AbProp = AbProp {
        name: "web_init_chat_max_unread_message_count",
        code: 1172,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_INTERN_DOGFOODING_UPSELL_CONTENT: AbProp = AbProp {
        name: "web_intern_dogfooding_upsell_content",
        code: 6860,
        value_type: AbPropType::Str,
        default: AbDefault::Str(""),
    };
    pub const WEB_INTERN_DOGFOODING_UPSELL_ENABLED: AbProp = AbProp {
        name: "web_intern_dogfooding_upsell_enabled",
        code: 6858,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_INTERN_DOGFOODING_UPSELL_SNOOZE_DURATION: AbProp = AbProp {
        name: "web_intern_dogfooding_upsell_snooze_duration",
        code: 6859,
        value_type: AbPropType::Int,
        default: AbDefault::Int(86400),
    };
    pub const WEB_INTERNAL_IN_APP_BUG_REPORTING_ENABLE: AbProp = AbProp {
        name: "web_internal_in_app_bug_reporting_enable",
        code: 4681,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_IP_TOKEN_ENABLED: AbProp = AbProp {
        name: "web_ip_token_enabled",
        code: 20043,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_JPEG_QUALITY: AbProp = AbProp {
        name: "web_jpeg_quality",
        code: 6619,
        value_type: AbPropType::Int,
        default: AbDefault::Int(92),
    };
    pub const WEB_LARGER_LINK_PREVIEWS: AbProp = AbProp {
        name: "web_larger_link_previews",
        code: 8172,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_LINK_PREVIEW_SYNC_ENABLED: AbProp = AbProp {
        name: "web_link_preview_sync_enabled",
        code: 2156,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_LOG_CAPACITY_OVERRIDE: AbProp = AbProp {
        name: "web_log_capacity_override",
        code: 24363,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_LOGOUT_UNMIGRATED_COMPANION: AbProp = AbProp {
        name: "web_logout_unmigrated_companion",
        code: 31151,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_LOW_END_DEVICE_LEVEL: AbProp = AbProp {
        name: "web_low_end_device_level",
        code: 18747,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_MAC_BETA_UPSELL: AbProp = AbProp {
        name: "web_mac_beta_upsell",
        code: 16223,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MATERIAL_REFRESH: AbProp = AbProp {
        name: "web_material_refresh",
        code: 6332,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MAX_CONTACTS_TO_SHOW_COMMON_GROUPS: AbProp = AbProp {
        name: "web_max_contacts_to_show_common_groups",
        code: 2264,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const WEB_MAX_FOUND_COMMON_GROUPS_DISPLAYED: AbProp = AbProp {
        name: "web_max_found_common_groups_displayed",
        code: 2268,
        value_type: AbPropType::Int,
        default: AbDefault::Int(15),
    };
    pub const WEB_MEDIA_COMPUTE_IN_WORKER_ENABLED: AbProp = AbProp {
        name: "web_media_compute_in_worker_enabled",
        code: 25641,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MEDIA_ENCRYPT_UPLOAD_IN_WORKER_ENABLED: AbProp = AbProp {
        name: "web_media_encrypt_upload_in_worker_enabled",
        code: 31721,
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
    pub const WEB_MESSAGE_CUSTOM_ARIA_LABEL: AbProp = AbProp {
        name: "web_message_custom_aria_label",
        code: 2280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MESSAGE_LIST_A11Y_REDESIGN: AbProp = AbProp {
        name: "web_message_list_a11y_redesign",
        code: 2016,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WEB_MESSAGE_PLUGIN_FRONTEND_REGISTRATION_ENABLED: AbProp = AbProp {
        name: "web_message_plugin_frontend_registration_enabled",
        code: 2793,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MESSAGE_PROCESSING_CACHE_SIZE: AbProp = AbProp {
        name: "web_message_processing_cache_size",
        code: 3728,
        value_type: AbPropType::Int,
        default: AbDefault::Int(400),
    };
    pub const WEB_MESSAGES_CONTENT_VISIBILITY: AbProp = AbProp {
        name: "web_messages_content_visibility",
        code: 31260,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MOVE_MESSAGE_SECRET_TOP_LEVEL_ENABLED: AbProp = AbProp {
        name: "web_move_message_secret_top_level_enabled",
        code: 29492,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MSG_INFRA_REMOVE_DEVICES_ON_406_ERROR_ENABLED: AbProp = AbProp {
        name: "web_msg_infra_remove_devices_on_406_error_enabled",
        code: 27463,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_MULTI_SKIN_TONED_EMOJI_PICKER: AbProp = AbProp {
        name: "web_multi_skin_toned_emoji_picker",
        code: 1850,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_NATIVE_FETCH_MEDIA_DOWNLOAD: AbProp = AbProp {
        name: "web_native_fetch_media_download",
        code: 3031,
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
    pub const WEB_NEW_EVENT_EMITTER: AbProp = AbProp {
        name: "web_new_event_emitter",
        code: 31127,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_NEW_WDS_ICONS: AbProp = AbProp {
        name: "web_new_wds_icons",
        code: 31128,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_NON_BLOCKING_OFFLINE_RESUME_MAX_MESSAGE_COUNT: AbProp = AbProp {
        name: "web_non_blocking_offline_resume_max_message_count",
        code: 2508,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1000),
    };
    pub const WEB_NONCRITICAL_HISTORY_SYNC_MESSAGE_PROCESSING_BREAK_ITERATION: AbProp = AbProp {
        name: "web_noncritical_history_sync_message_processing_break_iteration",
        code: 5106,
        value_type: AbPropType::Int,
        default: AbDefault::Int(100),
    };
    pub const WEB_NOTIFICATIONS_BANNER_NEW_LOGIC_ENABLED: AbProp = AbProp {
        name: "web_notifications_banner_new_logic_enabled",
        code: 19399,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_NOTIFICATIONS_BANNER_VARIANT: AbProp = AbProp {
        name: "web_notifications_banner_variant",
        code: 19168,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_OFFLINE_DYNAMIC_BATCH_CONFIG: AbProp = AbProp {
        name: "web_offline_dynamic_batch_config",
        code: 5297,
        value_type: AbPropType::Str,
        default: AbDefault::Str("{\"version\": \"progressive\", \"multiplier\": 0.25}"),
    };
    pub const WEB_OFFLINE_DYNAMIC_BATCH_SIZE_ENABLED: AbProp = AbProp {
        name: "web_offline_dynamic_batch_size_enabled",
        code: 5271,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OFFLINE_MESSAGE_PROCESSOR_TIMEOUT_SECONDS: AbProp = AbProp {
        name: "web_offline_message_processor_timeout_seconds",
        code: 8406,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_OFFLINE_RESUME_QPL_ENABLED: AbProp = AbProp {
        name: "web_offline_resume_qpl_enabled",
        code: 1773,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OFFLINE_RESUME_WAIT_FOR_PING_TIMEOUT_SECONDS: AbProp = AbProp {
        name: "web_offline_resume_wait_for_ping_timeout_seconds",
        code: 16956,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const WEB_OPTIMIZED_AVATARS: AbProp = AbProp {
        name: "web_optimized_avatars",
        code: 31257,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OPTIMIZED_COMPOSITING_LAYERS: AbProp = AbProp {
        name: "web_optimized_compositing_layers",
        code: 32280,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OPTIMIZED_EVENT_HANDLERS: AbProp = AbProp {
        name: "web_optimized_event_handlers",
        code: 31129,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OPTIMIZED_MESSAGE_TAILS: AbProp = AbProp {
        name: "web_optimized_message_tails",
        code: 31258,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OPTIMIZED_PILLS: AbProp = AbProp {
        name: "web_optimized_pills",
        code: 31130,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_ORIGINAL_PHOTO_QUALITY_UPLOAD_ENABLED: AbProp = AbProp {
        name: "web_original_photo_quality_upload_enabled",
        code: 3136,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_OTP_COPY_CODE_DISABLED: AbProp = AbProp {
        name: "web_otp_copy_code_disabled",
        code: 4330,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PATHFINDER_LOGGING: AbProp = AbProp {
        name: "web_pathfinder_logging",
        code: 27628,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_PDF_THUMBNAIL_SIZE_IN_BYTES: AbProp = AbProp {
        name: "web_pdf_thumbnail_size_in_bytes",
        code: 16834,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1300),
    };
    pub const WEB_PENDING_MESSAGE_CACHE_ENABLED: AbProp = AbProp {
        name: "web_pending_message_cache_enabled",
        code: 8353,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PHONE_NUMBER_GLOBAL_SEARCH: AbProp = AbProp {
        name: "web_phone_number_global_search",
        code: 22603,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PNLESS_STANZAS: AbProp = AbProp {
        name: "web_pnless_stanzas",
        code: 26211,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PRELOAD_CHAT_MESSAGES: AbProp = AbProp {
        name: "web_preload_chat_messages",
        code: 5079,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PREMIUM_MESSAGES_INTERACTIVITY_RENDERING_ENABLED: AbProp = AbProp {
        name: "web_premium_messages_interactivity_rendering_enabled",
        code: 4596,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PTT_RENDER_THROTTLING: AbProp = AbProp {
        name: "web_ptt_render_throttling",
        code: 31126,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PTT_STREAMER_UPLOAD: AbProp = AbProp {
        name: "web_ptt_streamer_upload",
        code: 1902,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PTT_TRANSCRIPTION_BUTTON_ENABLED: AbProp = AbProp {
        name: "web_ptt_transcription_button_enabled",
        code: 32799,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PTT_TRANSCRIPTION_ENABLED: AbProp = AbProp {
        name: "web_ptt_transcription_enabled",
        code: 32798,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PTT_TRANSCRIPTION_MAX_DURATION_SECONDS: AbProp = AbProp {
        name: "web_ptt_transcription_max_duration_seconds",
        code: 32800,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_PWA_BACKGROUND_SYNC: AbProp = AbProp {
        name: "web_pwa_background_sync",
        code: 6656,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_PWA_BACKGROUND_SYNC_MIN_INTERVAL_HOURS: AbProp = AbProp {
        name: "web_pwa_background_sync_min_interval_hours",
        code: 6706,
        value_type: AbPropType::Int,
        default: AbDefault::Int(24),
    };
    pub const WEB_QP_BB_RE_ENGAGEMENT_PAST_29_DAYS: AbProp = AbProp {
        name: "web_qp_bb_re_engagement_past_29_days",
        code: 30570,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_QP_SMB_BB_PMF_TEST_HIGH_ENGAGEMENT_USER: AbProp = AbProp {
        name: "web_qp_smb_bb_pmf_test_high_engagement_user",
        code: 30569,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_QP_SMB_BB_RECENT_MESSAGE_SEND: AbProp = AbProp {
        name: "web_qp_smb_bb_recent_message_send",
        code: 30568,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_RATING_AND_REVIEW_CONTEXTUAL_PROMPT_ENABLED: AbProp = AbProp {
        name: "web_rating_and_review_contextual_prompt_enabled",
        code: 18737,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_RATING_AND_REVIEW_ENABLED: AbProp = AbProp {
        name: "web_rating_and_review_enabled",
        code: 17540,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_READ_SELF_WATERMARK_PROCESSING: AbProp = AbProp {
        name: "web_read_self_watermark_processing",
        code: 30736,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_READ_SELF_WATERMARK_RECEIVE_STORE_TS: AbProp = AbProp {
        name: "web_read_self_watermark_receive_store_ts",
        code: 29396,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_READ_SELF_WATERMARK_SEND_STORE_TS: AbProp = AbProp {
        name: "web_read_self_watermark_send_store_ts",
        code: 29546,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_RECENT_SYNC_CHUNK_DOWNLOAD_OPTIMIZATION: AbProp = AbProp {
        name: "web_recent_sync_chunk_download_optimization",
        code: 7356,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_REMOVE_MESSAGE_SECRET_FROM_QUOTED_ENABLED: AbProp = AbProp {
        name: "web_remove_message_secret_from_quoted_enabled",
        code: 29491,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_REQUEST_MISSING_KEYS_FOR_REMOVES: AbProp = AbProp {
        name: "web_request_missing_keys_for_removes",
        code: 24838,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_RESUME_OPTIMIZED_READ_RECEIPT_SEND_INTERVAL: AbProp = AbProp {
        name: "web_resume_optimized_read_receipt_send_interval",
        code: 5502,
        value_type: AbPropType::Int,
        default: AbDefault::Int(500),
    };
    pub const WEB_SCREEN_LOCK_MAX_RETRIES: AbProp = AbProp {
        name: "web_screen_lock_max_retries",
        code: 2622,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const WEB_SEARCH_RESULTS_TYPE_DATE_FILTERS: AbProp = AbProp {
        name: "web_search_results_type_date_filters",
        code: 32787,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SEND_HID_FAILED_DECRYPT_IN_RECEIPTS_ENABLED: AbProp = AbProp {
        name: "web_send_hid_failed_decrypt_in_receipts_enabled",
        code: 31113,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SEND_INVISIBLE_MSG_MAX_GROUP_SIZE: AbProp = AbProp {
        name: "web_send_invisible_msg_max_group_size",
        code: 1945,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1024),
    };
    pub const WEB_SEND_INVISIBLE_MSG_MIN_GROUP_SIZE: AbProp = AbProp {
        name: "web_send_invisible_msg_min_group_size",
        code: 1100,
        value_type: AbPropType::Int,
        default: AbDefault::Int(128),
    };
    pub const WEB_SEND_ORPHAN_IN_RECEIPTS_ENABLED: AbProp = AbProp {
        name: "web_send_orphan_in_receipts_enabled",
        code: 31114,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SHOP_STOREFRONT_MESSAGE: AbProp = AbProp {
        name: "web_shop_storefront_message",
        code: 1053,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SHOW_TO_HIDE_ENABLED: AbProp = AbProp {
        name: "web_show_to_hide_enabled",
        code: 27958,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SIGNAL_FUTURE_MESSAGES_MAX: AbProp = AbProp {
        name: "web_signal_future_messages_max",
        code: 12509,
        value_type: AbPropType::Int,
        default: AbDefault::Int(20000),
    };
    pub const WEB_SOCKET_PARALLEL_CONNECTION_ENABLED: AbProp = AbProp {
        name: "web_socket_parallel_connection_enabled",
        code: 8019,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STATUS_CROSSPOSTING_ENABLED: AbProp = AbProp {
        name: "web_status_crossposting_enabled",
        code: 21501,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STATUS_LIKES_SEND_V2_ENABLED: AbProp = AbProp {
        name: "web_status_likes_send_v2_enabled",
        code: 26470,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STATUS_PSA: AbProp = AbProp {
        name: "web_status_psa",
        code: 873,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STATUS_RANKING: AbProp = AbProp {
        name: "web_status_ranking",
        code: 31683,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STATUS_RANKING_ENABLED: AbProp = AbProp {
        name: "web_status_ranking_enabled",
        code: 31684,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STICKER_SUGGESTIONS_ENABLE: AbProp = AbProp {
        name: "web_sticker_suggestions_enable",
        code: 4726,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STICKY_HD_PHOTO_SETTING_ENABLED: AbProp = AbProp {
        name: "web_sticky_hd_photo_setting_enabled",
        code: 8115,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STORE_QUOTA_MANAGER_ENABLED: AbProp = AbProp {
        name: "web_store_quota_manager_enabled",
        code: 3133,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_STREAMING_DOCUMENT_ENCRYPT_MIN_BYTES: AbProp = AbProp {
        name: "web_streaming_document_encrypt_min_bytes",
        code: 31864,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_SYNCD_FATAL_FIELDS_FROM_L1104589_PRV2: AbProp = AbProp {
        name: "web_syncd_fatal_fields_from_L1104589PRV2",
        code: 1808,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_SYNCD_MAX_MUTATIONS_TO_PROCESS_DURING_RESUME: AbProp = AbProp {
        name: "web_syncd_max_mutations_to_process_during_resume",
        code: 1513,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1000),
    };
    pub const WEB_TC_TOKEN_DB_READ_ENABLED: AbProp = AbProp {
        name: "web_tc_token_db_read_enabled",
        code: 5110,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_TEST_ABPROP_DELETE_ME: AbProp = AbProp {
        name: "web_test_abprop_delete_me",
        code: 27274,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_THREAD_LOADING_INFRA_ENABLED: AbProp = AbProp {
        name: "web_thread_loading_infra_enabled",
        code: 26192,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_THREADS_INFRA_ENABLED: AbProp = AbProp {
        name: "web_threads_infra_enabled",
        code: 21062,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WEB_TOP_LEVEL_MESSAGE_SECRET_ENFORCEMENT_ENABLED: AbProp = AbProp {
        name: "web_top_level_message_secret_enforcement_enabled",
        code: 32231,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_UI_REFRESH_M1: AbProp = AbProp {
        name: "web_ui_refresh_m1",
        code: 12993,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_USE_KALEIDOSCOPE_MEDIA_CHECK_ENABLED: AbProp = AbProp {
        name: "web_use_kaleidoscope_media_check_enabled",
        code: 20375,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_VIEW_MODE_USAGE_ENABLED: AbProp = AbProp {
        name: "web_view_mode_usage_enabled",
        code: 10856,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_VOIP_ADAPTIVE_SCTP_PREWARM: AbProp = AbProp {
        name: "web_voip_adaptive_sctp_prewarm",
        code: 32804,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_VOIP_AUDIO_CAPTURE_IMPL: AbProp = AbProp {
        name: "web_voip_audio_capture_impl",
        code: 21688,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_VOIP_AUDIO_PLAYBACK_IMPL: AbProp = AbProp {
        name: "web_voip_audio_playback_impl",
        code: 21689,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_VOIP_AV_SYNC_DEBUG_OVERLAY: AbProp = AbProp {
        name: "web_voip_av_sync_debug_overlay",
        code: 31481,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_VOIP_CAPTURE_VIDEO_ROTATION_TYPE: AbProp = AbProp {
        name: "web_voip_capture_video_rotation_type",
        code: 27973,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_VOIP_DYNAMIC_THREAD_PREALLOCATE_COUNT: AbProp = AbProp {
        name: "web_voip_dynamic_thread_preallocate_count",
        code: 23789,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_VOIP_LOAD_WASM_VARIANT: AbProp = AbProp {
        name: "web_voip_load_wasm_variant",
        code: 23045,
        value_type: AbPropType::Str,
        default: AbDefault::Str("prod-nonlab"),
    };
    pub const WEB_VOIP_LOW_RESOURCE_DEVICE: AbProp = AbProp {
        name: "web_voip_low_resource_device",
        code: 28203,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_VOIP_SCTP_WORKER_SAFARI_EXP: AbProp = AbProp {
        name: "web_voip_sctp_worker_safari_exp",
        code: 27695,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1),
    };
    pub const WEB_VOIP_VIDEO_CAPTURE_IMPL: AbProp = AbProp {
        name: "web_voip_video_capture_impl",
        code: 21350,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_VOIP_VIDEO_LOW_CAP_HEIGHT: AbProp = AbProp {
        name: "web_voip_video_low_cap_height",
        code: 28042,
        value_type: AbPropType::Int,
        default: AbDefault::Int(270),
    };
    pub const WEB_VOIP_VIDEO_LOW_CAP_WIDTH: AbProp = AbProp {
        name: "web_voip_video_low_cap_width",
        code: 28041,
        value_type: AbPropType::Int,
        default: AbDefault::Int(480),
    };
    pub const WEB_VOIP_VIDEO_MID_CAP_HEIGHT: AbProp = AbProp {
        name: "web_voip_video_mid_cap_height",
        code: 28044,
        value_type: AbPropType::Int,
        default: AbDefault::Int(360),
    };
    pub const WEB_VOIP_VIDEO_MID_CAP_WIDTH: AbProp = AbProp {
        name: "web_voip_video_mid_cap_width",
        code: 28043,
        value_type: AbPropType::Int,
        default: AbDefault::Int(640),
    };
    pub const WEB_VOIP_VIDEO_RENDERER: AbProp = AbProp {
        name: "web_voip_video_renderer",
        code: 20573,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WEB_WAFFLE: AbProp = AbProp {
        name: "web_waffle",
        code: 14300,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WAM_MAX_BUFFER_UPLOAD_SIZE_BYTES: AbProp = AbProp {
        name: "web_wam_max_buffer_upload_size_bytes",
        code: 9501,
        value_type: AbPropType::Int,
        default: AbDefault::Int(64000),
    };
    pub const WEB_WHATS_NEW_AUTO_MODAL: AbProp = AbProp {
        name: "web_whats_new_auto_modal",
        code: 29621,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WHATS_NEW_AUTO_MODAL_SHORT_COOLDOWN: AbProp = AbProp {
        name: "web_whats_new_auto_modal_short_cooldown",
        code: 29622,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WHATS_NEW_BANNER: AbProp = AbProp {
        name: "web_whats_new_banner",
        code: 29619,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WHATS_NEW_BANNER_SHORT_COOLDOWN: AbProp = AbProp {
        name: "web_whats_new_banner_short_cooldown",
        code: 29620,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WHATS_NEW_BANNER_SHORT_COOLDOWN_V2: AbProp = AbProp {
        name: "web_whats_new_banner_short_cooldown_v2",
        code: 29709,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WHATS_NEW_CAROUSEL: AbProp = AbProp {
        name: "web_whats_new_carousel",
        code: 29618,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WINDOWS_CALLING_32P_VERSION: AbProp = AbProp {
        name: "web_windows_calling_32p_version",
        code: 31845,
        value_type: AbPropType::Int,
        default: AbDefault::Int(3),
    };
    pub const WEB_WORKER_ADV_PROCESSING_ENABLED: AbProp = AbProp {
        name: "web_worker_adv_processing_enabled",
        code: 24924,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_WORKER_PREKEY_PROCESSING_ENABLED: AbProp = AbProp {
        name: "web_worker_prekey_processing_enabled",
        code: 26133,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEBC_PAGE_LOAD_EARLY_COMMIT_ENABLED: AbProp = AbProp {
        name: "webc_page_load_early_commit_enabled",
        code: 8458,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEBVIEW2_DISABLE_GPU_ACCELERATION: AbProp = AbProp {
        name: "webview2_disable_gpu_acceleration",
        code: 18262,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEBVIEW2_DISABLE_GPU_ACCELERATION_MEMORY_THRESHOLD_MB: AbProp = AbProp {
        name: "webview2_disable_gpu_acceleration_memory_threshold_mb",
        code: 23073,
        value_type: AbPropType::Int,
        default: AbDefault::Int(-1),
    };
    pub const WEBVIEW2_ENABLE_OFFLINE_SUPPORT: AbProp = AbProp {
        name: "webview2_enable_offline_support",
        code: 21793,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WHATSAPP_VPV_LOGGING_ENABLED: AbProp = AbProp {
        name: "whatsapp_vpv_logging_enabled",
        code: 9833,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WIN_CALL_LOG_SEND_OUTGOING_SYNCD_MUTATIONS: AbProp = AbProp {
        name: "win_call_log_send_outgoing_syncd_mutations",
        code: 5308,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WIN_ENABLE_SS_BUTTON_AUDIO: AbProp = AbProp {
        name: "win_enable_ss_button_audio",
        code: 9633,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WIN_HYBRID_BT_ENABLED: AbProp = AbProp {
        name: "win_hybrid_bt_enabled",
        code: 30041,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WIN_HYBRID_FORCE_PERSISTENT_STORAGE_PERMISSION: AbProp = AbProp {
        name: "win_hybrid_force_persistent_storage_permission",
        code: 20260,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WIN_HYBRID_VOIP_ANR_OPTIMIZATIONS: AbProp = AbProp {
        name: "win_hybrid_voip_anr_optimizations",
        code: 22616,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WIN_NETWORK_STATE_WATCHDOG_INTERVAL: AbProp = AbProp {
        name: "win_network_state_watchdog_interval",
        code: 7737,
        value_type: AbPropType::Int,
        default: AbDefault::Int(30),
    };
    pub const WINDOWS_CONTACTS_INITIAL_SYNC_DELAY: AbProp = AbProp {
        name: "windows_contacts_initial_sync_delay",
        code: 24883,
        value_type: AbPropType::Int,
        default: AbDefault::Int(10),
    };
    pub const WINDOWS_CONTACTS_SYNC_INTERVAL: AbProp = AbProp {
        name: "windows_contacts_sync_interval",
        code: 24882,
        value_type: AbPropType::Int,
        default: AbDefault::Int(60),
    };
    pub const WINDOWS_GRACEFUL_DEGRADATION_VERSION: AbProp = AbProp {
        name: "windows_graceful_degradation_version",
        code: 8454,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WINDOWS_SS_CAPTURE_DRIVER_TYPE: AbProp = AbProp {
        name: "windows_ss_capture_driver_type",
        code: 10434,
        value_type: AbPropType::Int,
        default: AbDefault::Int(0),
    };
    pub const WINRT_RENDERER: AbProp = AbProp {
        name: "winrt_renderer",
        code: 10966,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(true),
    };
    pub const WMI_ASYNC_AWAIT_PREP: AbProp = AbProp {
        name: "wmi_async_await_prep",
        code: 29197,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WMI_JM_TO_TS_M1: AbProp = AbProp {
        name: "wmi_jm_to_ts_m1",
        code: 32880,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WMI_TASK_SCHEDULER_SECOND_STEP: AbProp = AbProp {
        name: "wmi_task_scheduler_second_step",
        code: 30276,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WMI_WORKER_SCHEDULER_WEB: AbProp = AbProp {
        name: "wmi_worker_scheduler_web",
        code: 27237,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const XPLAT_ATTACHMENT_FORMAT_CHECK_V2: AbProp = AbProp {
        name: "xplat_attachment_format_check_v2",
        code: 8082,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const YOUTUBE_INLINE_PLAYBACK_KILLSWITCH: AbProp = AbProp {
        name: "youtube_inline_playback_killswitch",
        code: 3522,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    /// All 2036 flags in this registry, sorted by name.
    pub const ALL: &[AbProp] = &[
        A2UI_SUPPORTED_ELEMENTS,
        ACP_REMOVAL,
        ACP_REMOVAL_EPOCH_TIME,
        ACS_USE_GRAPHQL_FOR_FORWARD_COUNTER,
        ACS_USE_GRAPHQL_FOR_MIGRATION_TEST,
        ACS_USE_GRAPHQL_ISSUANCE,
        ADD_MEMBER_SYSTEM_MESSAGE,
        ADD_TO_CALL_IN_CHAT_THREAD,
        ADDON_INFRA_ENABLE_PERF_LOGGING,
        ADMIN_ONLY_MENTION_EVERYONE_GROUP_SIZE,
        ADMIN_REVOKE_RECEIVER,
        ADV_ACCEPT_HOSTED_DEVICES,
        ADV_V2_M4_M5,
        ADVANCED_CHAT_PRIVACY_CONTENT_UPDATE_JULY_25,
        AFTER_READ_FALLBACK_DURATION,
        AFTER_READ_RECEIVER_ENABLED,
        AFTER_READ_SENDING_ENABLED,
        AI_3P_AGENT_CHAT_ENABLED,
        AI_3P_AGENT_LINK_ENABLED,
        AI_ALL_LANGUAGES_ENABLED,
        AI_ASSET_REPLACEMENT_ENABLED,
        AI_BIZAI_2WAY_INTEGRATION_ENABLED,
        AI_BIZAI_2WAY_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED,
        AI_BOT_INTEGRATION_BOT_PROFILE,
        AI_BOT_INTEGRATION_ENABLED,
        AI_BOT_INTEGRATION_HISTORY_SYNC_ENABLED,
        AI_BOT_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED,
        AI_CHAT_META_AI_BANNER_M2_ENABLED,
        AI_CHAT_META_AI_GLASSES_BANNER_ENABLED,
        AI_CHAT_META_AI_HOME_DEFAULT_LANDING_ENABLED,
        AI_CHAT_META_AI_HOME_WEB_ENABLED,
        AI_CHAT_META_AI_NULL_STATE_WEB_ENABLED,
        AI_CHAT_THREAD_CAPABILITY_ENABLED,
        AI_CHAT_THREADS_FUZZY_SEARCH_ENABLED,
        AI_CHAT_THREADS_HISTORICAL_MESSAGES_MIGRATION_ENABLED,
        AI_CHAT_THREADS_HISTORY_ICON_VARIANT,
        AI_CHAT_THREADS_IMPLICIT_ROUTING_STRATEGY,
        AI_CHAT_THREADS_INFRA_ENABLED,
        AI_CHAT_THREADS_INFRA_WEB_ENABLED,
        AI_CHAT_THREADS_PIN_ENABLED,
        AI_CHAT_THREADS_PIN_MAX_COUNT,
        AI_CHAT_THREADS_WEB_ENABLED,
        AI_CHAT_THREADS_WEB_KILLSWITCH_ENABLED,
        AI_CHAT_THREADS_WEB_MSGS_LOAD_LIMIT,
        AI_CONTEXTUAL_WRITING_HELP_ENABLED,
        AI_CONTEXTUAL_WRITING_HELP_LANGUAGES_AND_TONES_CONFIG,
        AI_CONTEXTUAL_WRITING_HELP_NUM_SUGGESTIONS,
        AI_CONTINUOUS_SESSION_TRANSPARENCY_NOTICE_ENABLED,
        AI_DYNAMIC_MODE_SELECTOR_ENABLED,
        AI_DYNAMIC_MODE_SELECTOR_TTL_SECONDS,
        AI_EXPERIMENT_GRAPHQL_CONFIG,
        AI_FBID_MIGRATION_INVOKE_RECEIVE_ENABLED,
        AI_FBID_MIGRATION_RECEIVE_ENABLED,
        AI_FILE_UPLOAD_COUNT_LIMIT,
        AI_FILE_UPLOAD_SIZE_LIMIT_MB,
        AI_FILE_UPLOAD_SUPPORTED_FILE_TYPES,
        AI_FORWARD_ATTRIBUTION_ENABLED,
        AI_FORWARD_FLOW_SURFACE_META_AI_AS_CONTACT_ENABLED,
        AI_GENAI_STRAW_HAT,
        AI_GIZMO_INTEGRATION_ENABLED,
        AI_GROUP_CALL_ADD_IN_CALL_AHGC_ENABLED,
        AI_GROUP_CALL_ADD_IN_CALL_LGC_ENABLED,
        AI_GROUP_CALL_MAX_VERSION_BY_COUNTRY,
        AI_GROUP_CALL_MAX_VERSION_BY_PLATFORM,
        AI_GROUP_CALL_META_AI_ANIMATION_VERSION,
        AI_GROUP_CALL_START_CALL_AHGC_ENABLED,
        AI_GROUP_CALL_START_CALL_LGC_ENABLED,
        AI_GROUP_CALL_START_CALL_LOGGING_ENABLED,
        AI_GROUP_CALL_START_CALL_NOTICE_ID,
        AI_GROUP_CALL_VERSION,
        AI_GROUP_PARTICIPATION_ADD_TEE_ENABLED,
        AI_GROUP_PARTICIPATION_ENABLED,
        AI_GROUP_PARTICIPATION_SEND_ENABLED,
        AI_GROUP_SEND_MENTIONED_PUSHNAME_ENABLED,
        AI_GROUP_TEE_HISTORY_SHARE_ENABLED,
        AI_GROUPS_OPEN_ENABLED,
        AI_HATCH_COMMANDS_ENABLED,
        AI_HATCH_DOCUMENT_UPLOAD_SIZE_LIMIT_MB,
        AI_HATCH_ENCRYPTED_MEDIA_ENABLED,
        AI_HATCH_FORWARDING_HTML_ENABLED,
        AI_HATCH_INTEGRATION_BOT_PROFILE,
        AI_HATCH_INTEGRATION_ENABLED,
        AI_HATCH_INTEGRATION_HISTORY_SYNC_ENABLED,
        AI_HATCH_INTEGRATION_HISTORY_SYNC_PRE_CHATD_ENABLED,
        AI_HATCH_INTEGRATION_TAB_ENABLED,
        AI_HATCH_MEDIA_UPLOAD_COUNT_LIMIT,
        AI_HATCH_SECRET_ENCRYPTED_MESSAGE_ENABLED,
        AI_HATCH_VIDEO_AVATARS_ENABLED,
        AI_HATCH_VIDEO_UPLOAD_ENABLED,
        AI_HOME_BOT_PROFILE_SYNC_INTERVAL_SEC,
        AI_IMAGINE_LOADING_INDICATOR_ENABLED,
        AI_LEARNING_CLEAR_CHAT_DISABLE_EMPTY_CHATS,
        AI_MAIBA_WASS_MIGRATION_RECEIVING,
        AI_MAIBA_WASS_MIGRATION_SENDING,
        AI_META_AI_PREKEY_CLEANUP_ENABLED,
        AI_METABOT_DOCUMENT_OCR_IMAGE_CONVERSION_ENABLED,
        AI_METABOT_DOCUMENT_UPLOAD_ENABLED,
        AI_METABOT_DOCUMENT_UPLOAD_PAGE_COUNT_LIMIT,
        AI_METABOT_DOCUMENT_UPLOAD_SIZE_LIMIT_MB,
        AI_METABOT_IMAGE_INPUT_LANGUAGES,
        AI_METABOT_SEND_IMAGE_LIMIT,
        AI_MIGRATE_AWAY_FROM_INLINE_TOS_ENABLED,
        AI_MODE_SELECTOR_ENABLED,
        AI_MODE_SELECTOR_MEDIA_EDITOR_ENABLED,
        AI_PDFN_NUX_AI_GROUP_TEE_DISCOVER_NOTICE_ID,
        AI_PDFN_NUX_AI_SIDE_CHAT_NOTICE_ID,
        AI_PDFN_TOS_INLINE_NOTICES,
        AI_PDFN_TOS_INVOKE_NOTICE_ID,
        AI_PDFN_TOS_MASTER_NOTICE_ID,
        AI_PDFN_TOS_NON_BLOCKING_NOTICES,
        AI_PDFN_TOS_SHORTCUT_NOTICE_ID,
        AI_PTT_MAIN_GATE_SUPPORTED_LANGUAGES,
        AI_REPLY_MESSAGE_CONTEXT_MAX_COUNT,
        AI_REPLY_MESSAGE_CONTEXT_TRIGGER_MIN_COUNT,
        AI_REWRITE_ENABLED,
        AI_REWRITE_ENTRY_POINT_MIN_WORDS,
        AI_REWRITE_IN_EXPRESSION_TRAY_ENABLED,
        AI_REWRITE_LANGUAGES_AND_TONES_CONFIG,
        AI_REWRITE_LOAD_MORE_ENABLED,
        AI_REWRITE_NUM_SUGGESTIONS,
        AI_REWRITE_STACK_UNDO_ENABLED,
        AI_REWRITE_SUPPORTED_LANGUAGES,
        AI_REWRITE_TONE_MODIFIERS,
        AI_RICH_RESPONSE_FORWARD_RECEIVING_ENABLED,
        AI_RICH_RESPONSE_FORWARD_SENDING_ENABLED,
        AI_RICH_RESPONSE_FORWARDING_VERIFICATION_ENABLED_V1,
        AI_RICH_RESPONSE_GRID_IMAGE_ENABLED,
        AI_RICH_RESPONSE_INLINE_LINKS_ENABLED,
        AI_RICH_RESPONSE_MAIN_GATE_ENABLED,
        AI_RICH_RESPONSE_POST_CITATIONS_ENABLED,
        AI_RICH_RESPONSE_REASONING_ENABLED,
        AI_RICH_RESPONSE_REMOVE_GROUPED_CITATIONS_COUNT,
        AI_RICH_RESPONSE_SIDE_BY_SIDE_SURVEY_ENABLED,
        AI_RICH_RESPONSE_TEE_FORWARD_SENDING_ENABLED,
        AI_RICH_RESPONSE_TEE_FORWARDING_VERIFICATION_ENFORCEMENT_V1,
        AI_RICH_RESPONSE_UNKNOWN_SENDER_PREVIEW_ENABLED,
        AI_RICH_RESPONSE_UNKNOWN_SENDER_VERIFICATION_MASKING_ENABLED,
        AI_RICH_RESPONSE_UR_MEDIA_GRID_ENABLED,
        AI_RICH_RESPONSE_WEB_STRUCTURED_RESPONSE_ENABLED,
        AI_RICH_RESPONSE_ZEITGEIST_CAROUSEL_ENABLED,
        AI_SEARCH_ASK_BUTTON_WEB_ENABLED,
        AI_SEARCH_BAR_2025_REDESIGN_ENABLED,
        AI_SEARCH_EXPERIENCE_ENABLED,
        AI_SEARCH_EXPERIENCE_WEB_ENABLED,
        AI_SEARCH_MAX_NUM_SUGGESTIONS,
        AI_SEARCH_META_AI_SEND_BUTTON_ENABLED,
        AI_SEARCH_NULL_STATE_CONVO_STARTER_SUGGESTIONS_UPDATE_INTERVAL,
        AI_SEARCH_NULL_STATE_ENABLED,
        AI_SEARCH_NULL_STATE_ROW_COUNT,
        AI_SEARCH_NULL_STATE_UPDATE_INTERVAL,
        AI_SESSION_TRANSPARENCY_META_AI_ENABLED,
        AI_SIMPLIFIED_PROFILE_PAGE_ENABLED,
        AI_SUBSCRIPTION_IMAGINE_INTENT_ENABLED,
        AI_SUBSCRIPTION_METERING_ENABLED,
        AI_TAB_UNREAD_BADGE_RECENCY_WINDOW_HOURS,
        AI_UGC_HIDE_ENABLED,
        AI_UGC_NOT_AN_EXPERT_ENABLED,
        AI_UNIFIED_RESPONSE_FORWARDING_SENDER_WEB_TIMESTAMP,
        AI_UNIFIED_RESPONSE_IMAGINE_RECEIVER_WEB_ENABLED,
        AI_UNIFIED_RESPONSE_QPL_LOGGING,
        AI_UNIFIED_RESPONSE_RECEIVER_WEB_ENABLED,
        AI_UNIFIED_RESPONSE_RECEIVER_WEB_ENABLED_V2,
        AI_UNIFIED_RESPONSE_RECEIVER_WEB_TIMESTAMP_V2,
        AI_UNIFIED_RESPONSE_SENDER_WEB_ENABLED,
        AI_VIDEO_UPLOAD_SIZE_LIMIT_MB,
        AI_VIDEO_UPLOAD_SUPPORT_LANGUAGES,
        AI_VIDEO_UPLOAD_WEB_ENABLED,
        AI_VOICE_ENTRY_POINT_LOGGING_ENABLED,
        AI_VOICE_MULTIMODAL_COMPOSER_ENABLED,
        AI_WEB_ASK_META_AI_ENABLED,
        AI_WEB_FORWARD_FLOW_ENABLED,
        AI_WEB_META_AI_IMAGE_INPUT_ENABLED,
        AI_WEB_META_AI_PDF_DOCUMENT_INPUT_ENABLED,
        AIGC_VERSION,
        ALBUM_V2_FORWARD_AS_ALBUM_ENABLED,
        ALBUM_V2_ITEM_WITH_CAPTION_IN_ALBUM_RECEIVER_ENABLED,
        ALBUM_V2_MIN_ITEMS_TO_SEND_ALBUM_WITH_CAPTION,
        ALBUM_V2_MIN_ITEMS_TO_SEND_AS_ALBUM_ENABLED,
        ALBUM_V2_RECEIVING_ENABLED,
        ALBUM_V2_SENDER_ENABLED,
        ALLOW_BACKFILL_WITH_V0_TO_V1_PRIMARY_VERSION_TRANSITION,
        ANIMATED_EMOJI_FINAL_SET_ENABLED,
        ANIMATED_EMOJI_SET_1_ENABLED,
        ANIMATED_EMOJI_USE_LAZY_PARSING,
        ANIMATED_EMOJIS_ENABLED,
        ANIMATED_RACE_MERCEDES_CAR_EMOJI_ENABLED,
        ANIMATED_SOCCER_BALL_PROD_ENABLED,
        ANIMATED_SOCCER_BALL_TEST_ENABLED,
        ANYONE_CAN_LINK_TO_GROUPS,
        APP_EXIT_REASON_VERSION,
        APPOINTMENT_BOOKING_BLOKS_ENABLED,
        ATTACH_TRANSPORT_RTX,
        AUDIO_LEVEL_SPEAKING_THRESHOLD,
        AURA_APP_THEMES_BENEFIT_ACTIVE,
        AURA_APP_THEMES_ENABLED,
        AURA_ENABLED,
        AURA_FOCUS_LISTS_BENEFIT_ACTIVE,
        AURA_FOCUS_LISTS_ENABLED,
        AURA_KILL_SWITCH,
        AURA_MEDIA_OFFLOAD_BENEFIT_ACTIVE,
        AURA_MEDIA_OFFLOAD_ENABLED,
        AURA_PINNED_CHATS_BENEFIT_ACTIVE,
        AURA_PINNED_CHATS_ENABLED,
        AURA_PINNED_CHATS_TARGETED_NUX_FORCE,
        AURA_PREMIUM_STICKERS_KILLSWITCH,
        AURA_RINGTONES_BENEFIT_ACTIVE,
        AURA_RINGTONES_ENABLED,
        AURA_SETTINGS_ROW_ENABLED,
        AURA_STATUS_SEARCH_ENABLED,
        AURA_STATUS_SEARCH_MAX_VIEWERS,
        AURA_STATUS_SEARCH_TIMEOUT_THRESHOLD,
        AURA_STICKERS_BENEFIT_ACTIVE,
        AURA_STICKERS_ENABLED,
        AURA_STICKERS_OVERLAY_ANIMATION_ENABLED,
        AURA_STICKERS_PREVIEW_MAX_ANIMATION_COUNT,
        AURA_SUBSCRIPTION_SIMULATION_ENABLED,
        AUTH_AGENT_SOFT_OFFBOARDING_ENABLED,
        AUTH_AGENTS_CONSUMER_EXP_ENABLED,
        AUTH_AGENTS_CONSUMER_OFFBOARDING_EXP_ENABLED,
        BACKFILL_SUPPORTS_COEX_COMPANION,
        BANNED_SHOPS_UX_ENABLED,
        BB_CHAT_LIST_BANNER_1,
        BB_CHAT_LIST_BANNER_10,
        BB_CHAT_LIST_BANNER_2,
        BB_CHAT_LIST_BANNER_3,
        BB_CHAT_LIST_BANNER_4,
        BB_CHAT_LIST_BANNER_5,
        BB_CHAT_LIST_BANNER_6,
        BB_CHAT_LIST_BANNER_7,
        BB_CHAT_LIST_BANNER_8,
        BB_CHAT_LIST_BANNER_9,
        BB_CHAT_LIST_BANNER_V1,
        BB_CHAT_LIST_BANNER_V2,
        BB_CHAT_LIST_MAB_1,
        BB_CHAT_LIST_MAB_10,
        BB_CHAT_LIST_MAB_2,
        BB_CHAT_LIST_MAB_3,
        BB_CHAT_LIST_MAB_4,
        BB_CHAT_LIST_MAB_5,
        BB_CHAT_LIST_MAB_6,
        BB_CHAT_LIST_MAB_7,
        BB_CHAT_LIST_MAB_8,
        BB_CHAT_LIST_MAB_9,
        BIZ_AI_AGENT_3P_STORE_LINKS_ENABLED,
        BIZ_AI_AGENT_THREAD_STATUS_HISTORY_SYNC_ENABLED,
        BIZ_AI_AUTO_SAVE_ENABLED,
        BIZ_AI_COACHING_ENABLED,
        BIZ_AI_CONSUMER_TOS_NOTICE_IQ_WEB,
        BIZ_AI_CONSUMER_TOS_UPDATE_WEB,
        BIZ_AI_FAB_CONFIRM_MODAL_ENABLED,
        BIZ_AI_IN_THREAD_UNMUTE_V2,
        BIZ_AI_LARGE_SCREENS_GATE_FETCH_ENABLED,
        BIZ_AI_PRIORITY_LIST_ENABLED,
        BIZ_AI_RESPONDING_LIST_ENABLED,
        BIZ_AI_SMB_AGENTS_AUTOMATIC_REPLY_ENABLED,
        BIZ_AI_TOOLS_SETTINGS,
        BIZ_AI_TOOLS_SYNC,
        BIZ_AI_TOS_VARIANT,
        BIZ_AI_WEB_AI_HUB_TAP_CTA_SHOW_ALERT,
        BIZ_AI_WEB_BULK_THREAD_CONTROL_ENABLED,
        BIZ_AI_WEB_ONBOARDING_HANDOFF,
        BIZ_AI_WEB_ONBOARDING_HANDOFF_KILLSWITCH,
        BIZ_VPV_DIMENSIONS_LOGGING_ENABLED,
        BIZ_VPV_IMPRESSION_LOGGING_ENABLED,
        BLOCKLIST_SYSTEM_MSG_ON_FULL_REFETCH,
        BLOKS_A2UI_STEPS_ENABLED,
        BLUE_EDUCATION_ENABLED,
        BLUE_EDUCATION_V2_ENABLED,
        BLUE_ENABLED,
        BLUE_PROFILE_LOCKED_UI_ENABLED,
        BLUE_STRINGS_ENABLED,
        BONSAI_AVATAR_ENABLED,
        BONSAI_CAROUSEL_ENABLED,
        BONSAI_CAROUSEL_HQ_THUMBNAIL_ENABLED,
        BONSAI_CAROUSEL_REELS_PROFILE_PHOTO_ENABLED,
        BONSAI_CHAT_LIST_ENTRY_POINT_ENABLED,
        BONSAI_ENABLED,
        BONSAI_ENGLISH_ONLY,
        BONSAI_FP_UGC_SENDER,
        BONSAI_META_AI_SHORTCUT_TOS_ENABLED,
        BONSAI_PTT_ENABLED,
        BONSAI_SUPPORTED_LANGUAGES,
        BONSAI_TI_TIMEOUT_DURATION_MS,
        BONSAI_UPDATE_INTERVAL,
        BONSAI_WORD_STREAMING_ENABLED,
        BOOKING_CONFIRMATION_ENABLED_WA_WEB,
        BOT_3P_ENABLED,
        BOT_3P_STATUS,
        BOT_PROFILE_SYNC_MIGRATION_ENABLED,
        BR_ENABLE_PAYMENT_LOGOS_ON_BUBBLE,
        BR_PAYMENTS_HOME_DURATION_RULE_FOR_PUX_BANNER,
        BR_PAYMENTS_PAYMENT_DETECTION_ENHANCEMENT,
        BR_PAYMENTS_PAYMENT_REQUEST_CTA,
        BR_PAYMENTS_PIX_GROUPS_ENABLED,
        BR_PIX_KEY_BUBBLE_CONTENT_UPDATE,
        BR_SMB_PAYMENTSHOME_ENABLED,
        BR_SMB_PIX_PAYMENT_REQUEST_VARIANT,
        BRIGADING_PRIVACY_SETTING_ENABLED,
        BROADCAST_TO_YOUR_FOLLOWERS_ENABLED,
        BUG_REPORTING_ABPROPS_UPLOADED_ON_SUBMISSOIN,
        BUG_REPORTING_ASYNC_ATTACHMENTS_ENABLED,
        BUG_REPORTING_ATTACH_PATHFINDER_PRE_BUG_CREATION,
        BUG_REPORTING_ATTACH_VIEW_DUMP_PRE_BUG_CREATION,
        BUG_REPORTING_NOT_SHIPPED_YET_ENABLED,
        BUG_REPORTING_PRE_UPLOADED_ATTACHMENTS_ON_BUG_CREATION_ENABLED,
        BUG_REPORTING_RID_IN_FLYTRAP,
        BUG_REPORTING_USING_GRAPHQL,
        BUSINESS_BROADCAST_CAMPAIGN_SYNCD_ENABLED,
        BUSINESS_BROADCAST_INSIGHTS_CAMPAIGN_TTL_DAYS,
        BUSINESS_BROADCAST_INSIGHTS_SYNC_PAST_X_DAYS,
        BUSINESS_BROADCASTS_SYNCD_WAM_LOGGING,
        BUSINESS_TOOL_ENHANCED_LOGGING,
        BUYER_INITIATED_ORDER_REQUEST_VARIANT_ENABLED,
        CALL_ADMIN_VERSION,
        CALL_INFO_OPTIMIZATIONS_1ON1,
        CALL_INFO_OPTIMIZATIONS_AHGC_CALL_LINK,
        CALL_INFO_OPTIMIZATIONS_LGC,
        CALL_INFO_OPTIMIZATIONS_VERSION,
        CALL_INFO_USE_TYPED_JID,
        CALL_OFFER_FAILED_SOFT_LANDING_SCREEN_VERSION,
        CALL_SCREEN_SHARE_DUAL_STREAM_APP_UPDATE_DIALOG_ENABLED,
        CALLEE_ACCEPT_TIMEOUT_MS,
        CALLING_32P_VERSION,
        CALLING_AUDIO_SHARE_VERSION,
        CALLING_AV_SYNC_WEBRTC,
        CALLING_E2E_KEYGEN_VIA_SELF_LID,
        CALLING_LID_VERSION,
        CALLING_RUST_MIGRATION_BITMAP,
        CALLING_RUST_MIGRATION_INCOMING_ACK_STANZA_BITMAP,
        CALLING_RUST_MIGRATION_INCOMING_STANZA_BITMAP,
        CALLING_SCREEN_SHARE_MILESTONE_VERSION,
        CALLING_UX_LOGGING_BITMAP,
        CALLING_VOICEMAIL_ATTACHED_ICCE_ENABLED,
        CALLING_VOICEMAIL_ENABLED,
        CALLING_VOICEMAIL_QUOTED_REPLIES_ENABLED,
        CALLS_TAB_USERNAME_GLOBAL_SEARCH_ENABLED,
        CAMERA_ERROR_BANNERS_VERSION,
        CAMERA_HEALTH_CHECK_DELAY,
        CAMERA_HEALTH_CHECK_PERIOD,
        CANONICAL_ENT_COMPANION_SERVER_CACHED_NONCE_ENABLED,
        CAROUSEL_MESSAGE_CLIENT_ENABLED,
        CATALOG_CATEGORIES_ENABLED,
        CCI_COMPLIANCE_CTWA,
        CCI_COMPLIANCE_CTWA_LEARN_MORE_HYPERLINK,
        CCI_COMPLIANCE_MM,
        CHANNEL_ALBUM_V2_RECEIVING_ENABLED,
        CHANNEL_ALBUM_V2_SENDER_ENABLED,
        CHANNEL_ENFORCEMENT_LOGGING_ENABLED,
        CHANNEL_ENFORCEMENT_POLICY_EDUCATION_ENABLED,
        CHANNEL_FORWARD_BOTTOM_BUTTON_ENABLED,
        CHANNEL_FORWARD_TO_CHAT_ENABLED,
        CHANNEL_FORWARD_TO_CHAT_V2_MESSAGE_NAVIGATION_ENABLED,
        CHANNEL_OSA_REPORTING_ENABLED,
        CHANNEL_PHOTO_POLL_RECEIVER_ENABLED,
        CHANNEL_PHOTO_POLL_SENDER_ENABLED,
        CHANNEL_PLAYABLE_MESSAGE_VIEWS_DURATION_MILLISECONDS,
        CHANNEL_POLL_FORWARDING_ENABLED,
        CHANNEL_PULL_MESSAGE_UPDATES_THRESHOLD_SECONDS,
        CHANNEL_REACTIONS_ENABLED,
        CHANNEL_REACTIONS_SENDER_LIST_ENABLED,
        CHANNEL_REACTIONS_SETTINGS_ENABLED,
        CHANNEL_STATUS_CONSUMPTION,
        CHANNEL_STATUS_CREATION,
        CHANNEL_STATUS_DEEPLINK_ENABLED,
        CHANNEL_STATUS_FILL_GAP_PAGE_SIZE,
        CHANNEL_STATUS_FORWARDING_ENABLED,
        CHANNEL_STATUS_HELP_ENABLED,
        CHANNEL_STATUS_RESHARING_ENABLED,
        CHANNEL_STICKER_PACK_FORWARDING,
        CHANNEL_SUPPORTED_MESSAGE_TYPES,
        CHANNEL_TO_CHANNEL_FORWARDING_LOGGING_ENABLED,
        CHANNEL_US_NCII_REPORTING_ENABLED,
        CHANNEL_VIEW_COUNTS_ENABLED,
        CHANNEL_VIEWS_DURATION_MILLISECONDS,
        CHANNEL_VIEWS_VPV_DEFINITION_ENABLED,
        CHANNEL_WEB_EMBEDDING_ENABLED,
        CHANNELS_ADMIN_INSIGHTS_GIZMOS_ENABLED,
        CHANNELS_ADMIN_NOTIFICATIONS_ENABLED,
        CHANNELS_ADMIN_NOTIFICATIONS_FORWARDS_ENABLED,
        CHANNELS_ADMIN_PROFILES_FORWARDING_TO_CHATS_ENABLED,
        CHANNELS_ADMIN_PROFILES_LIST_ENABLED,
        CHANNELS_ADMIN_PROFILES_RECEIVER_ENABLED,
        CHANNELS_ADMIN_PROFILES_SENDER_ENABLED,
        CHANNELS_ADMIN_REPLY_ENABLED,
        CHANNELS_ADMIN_REPLY_RECEIVER_ENABLED,
        CHANNELS_ALBUM_RECEIVER_ENABLED,
        CHANNELS_ALBUM_SENDER_ENABLED,
        CHANNELS_AUDIO_FILES_DISPLAY_WAVEFORM_ENABLED,
        CHANNELS_AUDIO_FILES_RECEIVER_ENABLED,
        CHANNELS_AUDIO_FILES_SENDER_ENABLED,
        CHANNELS_AUDIO_FILES_SENDER_WAVEFORM_ENABLED,
        CHANNELS_CAPABILITIES_ENABLED,
        CHANNELS_CONTEXT_CARD_INVITE_FOLLOWERS_ENABLED,
        CHANNELS_CREATION_ENABLED,
        CHANNELS_CREATION_ENTRYPOINT_IN_DIRECTORY_ENABLED,
        CHANNELS_CREATION_ENTRYPOINT_IN_UPDATES_TAB_ENABLED,
        CHANNELS_DIRECTORY_CATEGORIES_CACHE_REFRESH_INTERVAL_MS,
        CHANNELS_DIRECTORY_CATEGORIES_ENABLED,
        CHANNELS_DIRECTORY_CATEGORIES_LOGGING_ENABLED,
        CHANNELS_DIRECTORY_CATEGORY_TYPES,
        CHANNELS_DIRECTORY_ENABLED,
        CHANNELS_DIRECTORY_PAGE_SIZE,
        CHANNELS_DIRECTORY_SEARCH_DEBOUNCE_MS,
        CHANNELS_DIRECTORY_V2_CACHE_REFRESH_INTERVAL_MS,
        CHANNELS_DIRECTORY_V2_FILTER_TYPES,
        CHANNELS_EMOJI_FORWARDED_ATTRIBUTION_UI_ENABLED,
        CHANNELS_ENABLED,
        CHANNELS_FETCH_AND_LOG_CAPABILITIES,
        CHANNELS_FILTER_OUT_SUBSCRIBED_IN_DIRECTORY_NULL_STATE,
        CHANNELS_FOLLOWER_INVITE_CREATION_MODAL_ENABLED,
        CHANNELS_FOLLOWERS_LIST_CACHE_REFRESH_MILLISECONDS,
        CHANNELS_FORWARD_COUNTER_ON_STATUS_CARD_ENABLED,
        CHANNELS_FORWARD_LOGGING_V2_ENABLED,
        CHANNELS_HIDE_NEWS_URL_PREVIEW,
        CHANNELS_IN_APP_POLICY_DETAIL_ENABLED,
        CHANNELS_INVITE_CONTACTS_TO_FOLLOW_CONSUMER_ENABLED,
        CHANNELS_INVITE_CONTACTS_TO_FOLLOW_PRODUCER_ENABLED,
        CHANNELS_INVITE_CONTACTS_TO_FOLLOW_RECEIVER_INVALID_MESSAGE_DROP_ENDABLED,
        CHANNELS_INVITE_CONTACTS_TO_FOLLOW_RECEIVER_LOGGING_ENABLED,
        CHANNELS_INVITE_CONTACTS_TO_FOLLOW_SENDER_LOGGING_ENABLED,
        CHANNELS_INVITE_LINK_PREVIEW_IMPROVEMENT_ENABLED,
        CHANNELS_IS_MULTI_ADMIN_LID_MIGRATION_ENABLED,
        CHANNELS_MAX_MESSAGES_BATCH_PULL,
        CHANNELS_MESSAGE_PIN_ADMIN_ENABLED,
        CHANNELS_MESSAGE_PIN_FOLLOWER_ENABLED,
        CHANNELS_MULTI_ADMIN_MAX_ADMIN_COUNT,
        CHANNELS_MUSIC_FORWARDING_DISABLED,
        CHANNELS_MUSIC_RECEIVER_ENABLED,
        CHANNELS_OPEN_QPL_IMPROVEMENTS_ENABLED,
        CHANNELS_OPEN_QPL_USER_RID_LOGGING_ENABLED,
        CHANNELS_PHOTO_POLLS_GENAI_ENABLED,
        CHANNELS_PINNING_NUDGE_ENABLED,
        CHANNELS_POLL_RECEIVE_ENABLED,
        CHANNELS_POLL_VOTER_LIST_ENABLED,
        CHANNELS_POLL_VOTERS_DETAILS_CACHE_TTL_MS,
        CHANNELS_POLL_VOTERS_SUMMARY_CACHE_TTL_MS,
        CHANNELS_PROACTIVE_MESSAGE_GAP_HANDLING_ENABLED,
        CHANNELS_PRODUCER_INSIGHTS_ENABLED,
        CHANNELS_PRODUCER_INSIGHTS_HIDE_DELTAS,
        CHANNELS_PRODUCER_INSIGHTS_MIN_FOLLOWERS,
        CHANNELS_PTT_LOGGING_ENABLED,
        CHANNELS_PTT_RECEIVER_ENABLED,
        CHANNELS_PTV_FORWARDING_ENABLED,
        CHANNELS_PTV_RECEIVING_ENABLED,
        CHANNELS_PULSE_ON_UNREAD_BADGE_ENABLED,
        CHANNELS_QPL_IMPROVEMENTS_SUPPORTED_TYPES,
        CHANNELS_QPL_LOGGING,
        CHANNELS_QUESTION_ADMIN_ENABLED,
        CHANNELS_QUESTION_ADMIN_M2_ENABLED,
        CHANNELS_QUESTION_FETCH_RESPONSES_PAGE_SIZE,
        CHANNELS_QUESTION_FOLLOWER_ENABLED,
        CHANNELS_QUESTION_FOLLOWER_M2_ENABLED,
        CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_CHAT_M1_ENABLED,
        CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_CHAT_M2_ENABLED,
        CHANNELS_QUESTION_FORWARD_MESSAGE_TYPES_STATUS_M2_ENABLED,
        CHANNELS_QUESTION_RECEIVER_MESSAGE_TYPES_M1_ENABLED,
        CHANNELS_QUESTION_RECEIVER_MESSAGE_TYPES_M2_ENABLED,
        CHANNELS_QUESTION_REPLY_RECEIVER_MESSAGE_TYPES_M1_ENABLED,
        CHANNELS_QUESTION_REPLY_RECEIVER_MESSAGE_TYPES_M2_ENABLED,
        CHANNELS_QUESTION_REPLY_SENDER_MESSAGE_TYPES_M1_ENABLED,
        CHANNELS_QUESTION_REPLY_SENDER_MESSAGE_TYPES_M2_ENABLED,
        CHANNELS_QUESTION_RESPONSE_RATE_LIMIT_MAX_COUNT_IN_CLIENT_UI,
        CHANNELS_QUESTION_SENDER_MESSAGE_TYPES_M1_ENABLED,
        CHANNELS_QUESTION_SENDER_MESSAGE_TYPES_M2_ENABLED,
        CHANNELS_QUESTIONS_INTEGRITY_M1_ENABLED,
        CHANNELS_QUESTIONS_RESPONSES_DRAWER_LOADING_SHIMMER_ENABLED,
        CHANNELS_QUESTIONS_SEARCH_BACKTEST_ENABLED,
        CHANNELS_QUESTIONS_SEARCH_ENABLED,
        CHANNELS_QUICK_FORWARDING_BUTTON_MODE,
        CHANNELS_QUIZ_RECEIVING_ENABLED,
        CHANNELS_QUIZ_SENDING_ENABLED,
        CHANNELS_REACTIONS_BOTTOMSHEET_TAP_TO_REACT_ENABLED,
        CHANNELS_RECOMMENDED_V3_UI_LIMIT,
        CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_CHAT_M1_ENABLED,
        CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_CHAT_M2_ENABLED,
        CHANNELS_REPLY_FORWARD_MESSAGE_TYPES_STATUS_M2_ENABLED,
        CHANNELS_SEND_ALBUM_ENABLED,
        CHANNELS_SEND_VIEW_RECEIPT_ENABLED,
        CHANNELS_SGI_RECEIVER_ENABLED,
        CHANNELS_SGI_SENDER_ENABLED,
        CHANNELS_SHARE_LINK_LOGGING_ENABLED,
        CHANNELS_STATUS_CONSUMPTION_ENTRYPOINTS,
        CHANNELS_STATUS_UPDATES_CONSUMPTION_ENABLED,
        CHANNELS_STICKER_FORWARDED_ATTRIBUTION_UI_ENABLED,
        CHANNELS_STICKER_PACK_FORWARDED_ATTRIBUTION_UI_ENABLED,
        CHANNELS_STICKER_PACK_RENDERING,
        CHANNELS_T_ENABLED,
        CHANNELS_UK_OSA_ENABLED,
        CHANNELS_UPDATES_TAB_SWIPE_ACTIONS_ENABLED,
        CHANNELS_VERIFIED_BADGE_IN_COMPACT_INBOX_ENABLED,
        CHANNELS_VIDEO_PLAY_LOGGING_ENABLED,
        CHANNELS_VIEW_COUNTS_SENDER_ADMIN_EXCLUSION_MODE,
        CHANNELS_VIEW_COUNTS_VPV_LOGGING_ENABLED,
        CHANNELS_VISIBILITY_LOGGING_FULLSCREEN_MEDIA_ENABLED,
        CHANNELS_VPV_LOGGING_ENABLED,
        CHATLIST_FILTERS_V1,
        CHATLIST_PREVENT_AUTOREAD,
        CHATLIST_SHOW_DRAFT_FOR_EMPTY_CHAT,
        COEX_CALLING_ENABLED,
        COEX_CALLING_ENABLED_BUSINESS,
        COEX_CALLING_PERMISSIONS_3P_ENABLED,
        COEX_EDIT_MSG_ENABLED,
        COEX_IICON_BACKFILL,
        COEX_REVOKE_MESSAGE_ENABLED,
        COEXV2_RECV_ENABLED,
        COEXV2_SEND_ENABLED,
        COMMERCE_SANCTIONED,
        COMMUNITY_ADMIN_PROMOTION_ONE_TIME_PROMPT,
        COMMUNITY_ANNOUNCEMENT_GROUP_SIZE_LIMIT,
        COMMUNITY_GENERAL_CHAT_UI_ENABLED,
        COMMUNITY_GENERAL_CHAT_CREATE_ENABLED,
        CONSUMER_GRAPHQL_ENABLE_DOUBLE_LOG_FOR_SURVEY,
        CONSUMER_GRAPHQL_WEB_TO_FETCH_QP_SURFACE_IDS,
        CONSUMER_WEB_QP_GRAPHQL_TO_FETCH_QP_FREQUENCY_MINS,
        COUNTRY_CLIENT_GATING_ENABLED,
        COUPON_COPY_BUTTON_URL,
        CREATE_GROUP_AND_ADD_MEMBER_OVERFLOW,
        CROSS_DEVICE_MESSAGE_EDITING,
        CTWA_1PD_LONGEST_CALL_ENABLED,
        CTWA_3PD_AGGREGATED_CALL_LOGGING_ALLOWED,
        CTWA_3PD_AGGREGATED_CONVERSION_ENABLED,
        CTWA_3PD_DATA_SHARING_ADDITIONAL_LOGGING,
        CTWA_3PD_DATA_SHARING_COOLDOWN_MAX_TIMES_SHOWN_FOR_OPTED_OUT,
        CTWA_3PD_DATA_SHARING_ON_THREAD_ENTRY,
        CTWA_3PD_DATA_SHARING_TITLE_CHANGE,
        CTWA_3PD_OPT_OUT_COUNTER_OPTIMIZATION_ENABLED,
        CTWA_AD_ACCOUNT_NONCE_PUSH_WAIT_TIMEOUT_WEB,
        CTWA_AD_ACCOUNT_NONCE_RETRIES_MAX_WEB,
        CTWA_AD_ACCOUNT_TOKEN_STORAGE_KILL_SWITCH_WEB,
        CTWA_AD_CREATION_ENTRY_POINT_CATALOG_PRODUCT_WEB,
        CTWA_AD_CREATION_ENTRY_POINT_CATALOG_WEB,
        CTWA_AE_MODEL_META_DATA_ENABLED,
        CTWA_AE_MODEL_META_DATA_SIGNAL_ENABLED,
        CTWA_BLOCK_IB_AR_FOR_WABAI,
        CTWA_CONVERSION_CREATION_FROM_DELAY_ENABLED,
        CTWA_CUSTOM_LABEL_ALGORITHM,
        CTWA_CUSTOM_LABEL_SIGNALS_ENABLED,
        CTWA_DATA_MAX_LENGTH,
        CTWA_DOWNLOAD_3PD_SIGNALS,
        CTWA_ENABLE_BIZ_DATA_SHARING_AFTER_NUX_DISMISS,
        CTWA_ENTRY_POINT_CONFIG_FETCH_THRESHHOLD,
        CTWA_FAVORITES_LIST_SENDS_SIGNALS,
        CTWA_IMPORTANT_LABEL_SENDS_SIGNALS,
        CTWA_LEAD_TAXONOMY,
        CTWA_LONG_TERM_HOLDOUT_CLIENT_SIDE_CHECK,
        CTWA_LONG_TERM_HOLDOUT_CONTENT_ENABLED,
        CTWA_LONGEST_CALL_DURATION,
        CTWA_MM_BIZ_AI_DISCLOSURE_UPDATE_ENABLED,
        CTWA_NATIVE_ADS_CREATION_WEB_ENABLED,
        CTWA_NATIVE_ADS_CREATION_WEB_HAWK_TOOL_ENABLED,
        CTWA_NATIVE_ADS_CREATION_WEB_TARGETING_MODAL_HAWK_TOOL_ENABLED,
        CTWA_NATIVE_ADS_DETAILED_TARGETING,
        CTWA_NATIVE_ADS_INLINE_NOTICE_MODULES,
        CTWA_NATIVE_WEB_DRAFT_AD_ENABLED,
        CTWA_PER_CUSTOMER_DATA_SHARING_CONTROLS_DO_NOT_SHOW_MSG_UNTIL_CHOSEN,
        CTWA_SHOW_ADS_DATA_SHARING_AFTER_MESSAGE,
        CTWA_SMB_DATA_SHARING_CONSENT,
        CTWA_SMB_DATA_SHARING_OPT_IN_COOL_OFF_PERIOD,
        CTWA_SMB_DATA_SHARING_SETTINGS_KILLSWITCH,
        CTWA_SMB_DETECTED_OUTCOME_LABELS_ENABLED,
        CTWA_SMB_DETECTED_OUTCOME_LABELS_MERGER_ENABLED,
        CTWA_SMB_DETECTED_OUTCOME_LISTS_ENABLED,
        CTWA_SMB_LABEL_CHAT_HEADER_ENABLED_WEB,
        CTWA_SMB_LISTS_DROPDOWN_APPLICATION_FIX_ENABLED,
        CTWA_SMB_MULTISELECT_ENABLED,
        CTWA_SUPPRESS_MESSAGE_VIA_AD_SPAM_WEB,
        CTWA_SUPPRESS_MESSAGE_WITH_EXTERNAL_AD_REPLY_CONSUMER_DB_LEVEL_ENABLED,
        CTWA_TOS_FILTERING_ENABLED,
        CTWA_WEB_CUSTOM_LABEL_SIGNALS_ENABLED,
        CTWA_WEB_NATIVE_ADS_BUDGET_RECOMMENDATION_ENABLED,
        CTWA_WEB_NATIVE_ADS_MVP_QE1_ENABLED,
        CTWA_WEB_NATIVE_ADS_MVP_QE1_ENABLED_NO_EXPOSURE,
        CTWA_WEB_NATIVE_ADS_MVP_QE2_ENABLED,
        CTWA_WEB_NATIVE_ADS_SABR_ENABLED,
        CUSTOM_NOTIFICATION_TONES,
        CUSTOM_RACING_EMOJI,
        CUSTOM_RACING_EMOJI_FEB2025,
        DATA_PRIVACY_PHASE_2_ENABLED,
        DATA_PRIVACY_PHASE_2_NON_E2EE_ENABLED,
        DATA_SHARING_TRANSPARENCY_INDICATOR_DURATION,
        DAU_FIX_DELAY_PRESENCE_ON_FOCUS,
        DEFAULT_AUDIO_LIMIT_MB,
        DEFAULT_ENDPOINT_THREAD_POLL_TIMEOUT,
        DEFAULT_MEDIA_LIMIT_MB,
        DEFAULT_STATUS_MEDIA_LIMIT_MB,
        DEFAULT_VIDEO_LIMIT_MB,
        DEFENSE_MODE_AVAILABLE,
        DEFENSE_MODE_QUARANTINE,
        DEFENSE_MODE_QUARANTINE_BULK_UNBLOCK_LIMIT,
        DEFENSE_MODE_QUARANTINE_MESSAGE_EXPIRATION_WINDOW,
        DESKTOP_UPSELL_INTRO_PANEL_ILLUSTRATION_VARIANT,
        DEV_PROP_BOOLEAN,
        DEV_PROP_FLOAT,
        DEV_PROP_INT,
        DEV_PROP_STRING,
        DEVICE_SWITCHING_ENABLED,
        DIALER_PAD_FOR_NEW_CHATS,
        DIRECT_CONNECTION_BUSINESS_NUMBERS,
        DIRECTORY_CATEGORIES_DISPLAY_NEWSLETTERS_PER_CATEGORY_LIMIT,
        DIRECTORY_CATEGORIES_NEWSLETTERS_PER_CATEGORY_LIMIT,
        DISABLE_AUTO_DOWNLOAD,
        DISABLE_LIBAOM_REGISTRATION,
        DISABLE_RAISE_HAND_1ON1,
        DISAPPEARING_MODE,
        DISCLOSURE_FOR_THE_MARKETING_MESSAGE_BODY_LINKS_ENABLED,
        DM_ADDITIONAL_DURATIONS,
        DM_AFTER_READ_TIMER_SENDER_OPTIONS_SECONDS,
        DM_INITIATOR_TRIGGER_DAILY_LOGS,
        DM_INITIATOR_TRIGGER_GROUPS,
        DM_RECEIVER_AFTER_READ_ALLOW_VALUES,
        DM_RECEIVER_ALLOWED_VALUES,
        DM_RELIABILITY_LOGGING,
        DM_UPDATED_SYSTEM_MESSAGE,
        DOWNLOAD_DOCUMENT_THUMB_MMS_ENABLED,
        DOWNLOAD_STATUS_THUMB_MMS_ENABLED,
        DROP_LAST_NAME,
        DSA_21_CHANNEL_REPORTING_ENABLED,
        DSA_26_RECEIVER_ENABLED,
        DSA_26_SENDER_ENABLED,
        DSA_CHANNELS_REPORT_UNLAWFUL_CONTENT_ENABLED,
        DSA_INFORMATION_FOR_EU_ONLY_ENABLED,
        EARLY_AUDIO_DRIVER_CAPTURE_AT_NATIVE,
        EARLY_AUDIO_DRIVER_PRE_BUFFERING,
        EARLY_BOT_CONNECT_EVENT_BITMAP,
        EDUCATIONAL_DIALOGS_BUTTON_ENABLED,
        ELEVATED_PUSH_NAMES_V2_M2_ENABLED,
        EMOJI_SEARCH_CLDR,
        EMPTY_UNREAD_FILTER_CTA_VARIANT,
        ENABLE_3P_CONTACTS_SHARE_HYBRID,
        ENABLE_AGM_FLOW_CTA,
        ENABLE_AUDIO_DEVICE_ASYNC_START,
        ENABLE_AUTO_ADD_CALL_LINK_CREATOR,
        ENABLE_AV_DOWNGRADE_1ON1,
        ENABLE_AVATARS_ON_WEB_COMPANION,
        ENABLE_BUSY_REASON_FS,
        ENABLE_CACHED_MEDIA_MANAGER,
        ENABLE_CALL_CONTROL_M5,
        ENABLE_CALL_LINK_CALL_LOG_AGGREGATION,
        ENABLE_CALL_LINKS_PUSH_NOTIFICATION,
        ENABLE_CALL_RESULT_FIX_FOR_404_ACCEPT_NACK,
        ENABLE_CALL_TRANSFER_NOTIFICATION,
        ENABLE_CALLING_PHONE_NUMBER_PRIVACY,
        ENABLE_CALLING_USERNAME,
        ENABLE_CHANNEL_VIDEO_SERVER_THUMBNAIL,
        ENABLE_CHAT_LIST_STICKER_EMOJIS,
        ENABLE_CHAT_PSA_AUTO_PLAY_VIDEOS,
        ENABLE_CLEAR_FORMATTED_PREVIEW,
        ENABLE_COMMUNITY_SUSPEND_AND_APPEALS,
        ENABLE_COPY_PASTE_P2P,
        ENABLE_CTWA_ML_ENTRY_POINT_CONFIG,
        ENABLE_DAYS_SINCE_RECEIVE_LOGGING,
        ENABLE_EARLY_AUDIO_DRIVER_START,
        ENABLE_EVENTS_V2_ADD_TO_CALENDAR,
        ENABLE_EVENTS_V2_ENTRY_POINTS_CREATION,
        ENABLE_EVENTS_V2_INVITE_MESSAGE_WITH_DATETIME,
        ENABLE_EVENTS_V2_ON_COMPANION,
        ENABLE_FMX_LOGGING,
        ENABLE_FORCE_VOIP_LOGGING,
        ENABLE_FUTUREPROOF_GALAXY_FLOW_MESSAGE_FOR_BUSINESS_NUMBERS,
        ENABLE_GRID_LAYOUT_TILE_UNIFICATION,
        ENABLE_GROUP_CREATE_OR_ADD_RATE_LIMITING_ERROR_UX,
        ENABLE_HYBRID_CALL_LINKS_CREATION,
        ENABLE_HYBRID_CALL_LINKS_JOIN,
        ENABLE_HYBRID_VIDEO_TRANSCODING,
        ENABLE_HYBRID_VIDEO_TRANSCODING_FOR_VALID_MP4,
        ENABLE_INIT_BWE_FOR_GROUP_CALL,
        ENABLE_JOIN_GROUP_CONTEXT_NON_AUTO_EXPOSE,
        ENABLE_LAZY_LOADING_OF_CALL_VIEW_ELEMENTS,
        ENABLE_LID_CALL_LINK,
        ENABLE_LOGGING_QBM_INCOMING_MESSAGE,
        ENABLE_MENTION_EVERYONE_RECEIVER_WEB,
        ENABLE_MENTION_EVERYONE_SENDER_WEB,
        ENABLE_MENTION_EVERYONE_SYNCD_SENDER,
        ENABLE_MINIMIZE_INDIVIDUAL_MUTATION_WRITE,
        ENABLE_ML_BWE_MODEL_DOWNLOAD,
        ENABLE_NEW_ONGOING_CALL_CELL_UI,
        ENABLE_NEW_USER_ACTION_STANZA_FOR_RAISE_HAND_SENDER,
        ENABLE_OFFER_V2_UPGRADE,
        ENABLE_ORBIT_SSO_BRIDGE,
        ENABLE_ORDER_DETAILS_FOR_PAYMENT_KEY,
        ENABLE_PEER_SNAPSHOT_RECOVERY,
        ENABLE_POLL_SETTINGS_LABEL_IMPROVED_LAYOUT,
        ENABLE_PRE_WARM_AUDIO_COMPONENT,
        ENABLE_PRIVACY_TOKEN_WITH_TIMESTAMP,
        ENABLE_PRODUCT_CAROUSEL_MESSAGE,
        ENABLE_RATE_APP_PROMPT,
        ENABLE_RING_FOR_GC_ON_OFFER_EXPIRE,
        ENABLE_SCHEDULED_CALLS_V2_ENTRY_POINTS_CREATION,
        ENABLE_SETUP_ERROR_RESULT_CHECK,
        ENABLE_SHARING_FILES_FROM_WEB_WINDOWS_HYBRID,
        ENABLE_SILENT_OFFER,
        ENABLE_SOOX_MESSAGE_SENDING,
        ENABLE_SPAM_REPORT_IQ_WITH_PRIVACY_TOKEN,
        ENABLE_STICKER_VERIFICATION_FOR_GIMMICK,
        ENABLE_SYNC_FOR_DRAFT_MESSAGES,
        ENABLE_SYNCD_COEX_V2,
        ENABLE_SYNCD_DEBUG_DATA_IN_PATCH,
        ENABLE_TOOLTIP_FOR_MEDIA_HUB,
        ENABLE_TURN_ON_CALL_NOTIFICATION_REMINDERS,
        ENABLE_UGC_VOICE_FS_LOGGING,
        ENABLE_UNIFIED_CALL_BUTTONS_IN_CHAT,
        ENABLE_UPCOMING_SCHEDULE_CALL_EVENTS_IN_CALLS_TAB,
        ENABLE_UWP_DEVICE_SWITCH_BANNER,
        ENABLE_UWP_SCREEN_SHARE_TEACHING_TIP,
        ENABLE_UWP_SHARE_ANY_WINDOW,
        ENABLE_UWP_SWAP_VIDEO_STREAM,
        ENABLE_VIDEO_METRICS_FIX,
        ENABLE_WAITING_ROOM_ADMIN_UI,
        ENABLE_WAITING_ROOM_LOGGING,
        ENABLE_WAITING_ROOM_UI,
        ENABLE_WDS_CALLING_DROPDOWN,
        ENABLE_WEB_CALLING,
        ENABLE_WEB_CALLING_BETA_UPSELL,
        ENABLE_WEB_CALLING_NUX,
        ENABLE_WEB_GROUP_CALLING,
        ENABLE_WEB_LOG_DOWNLOAD,
        ENABLE_WEB_VOIP_ANR_OPTIMIZATIONS,
        ENABLE_WEB_VOIP_DYNAMIC_FPS_THROTTLE,
        ENABLE_WEB_VOIP_EAGER_MIC_ACQUIRE,
        ENABLE_WEB_VOIP_P2P,
        ENABLE_WEB_VOIP_PLATFORM_AV_SYNC,
        ENABLE_WEB_VOIP_PROXY_AND_SCTP_WORKERS,
        ENABLE_WEB_VOIP_VIDEO_RESOLUTION_CAP,
        ENABLE_WEB_VOIP_VIRTUAL_AUDIO_CAPTURE_DRIVER,
        ENABLE_WEB_VOIP_VIRTUAL_VIDEO_CAPTURE_DRIVER,
        ENABLE_WEB_VOIP_WEBTRANSPORT,
        ENABLE_WEBCODEC_REQUIRE_KEYFRAME,
        ENABLE_WEBCODEC_VIDEO_ENCODE,
        ENABLE_WEBRTC_VIDEO_JB,
        ENABLE_WEFR_CLIENT_EXPO_PULSE,
        ENABLE_WINDOWS_HYBRID_JUMPLIST_CONTACTS,
        ENABLE_WINDOWS_JUMPLIST_HYBRID,
        ENABLE_WINDOWS_MOCKS_CAPTURE_DRIVERS,
        ENABLE_WINDOWS_XDR_CHAT_HANDOFF,
        ENHANCED_MENTION_LIMIT,
        ENHANCED_MENTION_SUGGESTIONS_MIN_MENTION_CHAR_COUNT,
        ENHANCED_MENTION_SUGGESTIONS_NON_GROUP_MEMBERS_ENABLED,
        EPHEMERAL_SYNC_RESPONSE,
        EVENT_DESCRIPTION_LENGTH_LIMIT,
        EVENT_NAME_LENGTH_LIMIT,
        EVENTS_CREATE,
        EVENTS_CREATE_CAG_ENABLED,
        EVENTS_EDIT_RECEIVE,
        EVENTS_M3_COVER_IMAGE_RECEIVE,
        EVENTS_M3_COVER_IMAGE_SEND,
        EVENTS_V2_ENABLE_NOTIFICATIONS,
        EVENTS_V2_HIDE_ADD_TO_CALENDAR_POST_START_WINDOW_SEC,
        EVENTS_V2_INVITATION_MESSAGE_VERSION,
        EVOLVE_ABOUT_M1_RECEIVER_ENABLED,
        EVOLVE_ABOUT_M1_RECEIVER_FOR_NEW_SURFACES_ENABLED,
        EXPAND_FMX_MEX_SHOULD_USE_FMX_USE_CASE,
        EXTENSIONS_GEOBLOCKING_ENABLED,
        EXTENSIONS_USER_REPORT_STORE_MAX_DATA_EXCHANGES_PER_SESSION,
        EXTENSIONS_USER_REPORT_STORE_MAX_DATA_MAX_SESSIONS_PER_MESSAGE,
        EXTERNAL_BETA_CAN_JOIN,
        EXTERNAL_CTX_AUTHORISE_EXISTING_CHATS,
        EXTERNAL_CTX_AUTHORISE_WA_CHAT,
        EXTERNAL_CTX_FOA_LOGGING,
        EXTERNAL_CTX_URL_PARAM_NAMES,
        FAVORITE_STICKER_SYNC_AFTER_PAIRING_ENABLED_WEB,
        FAVORITES_LIMIT,
        FEATURE_KEY_STORE_INFRA_ENABLED,
        FETCH_QP_VIA_GRAPHQL_WEB_ENABLED,
        FLATTENED_REACTIONS_COLLECTION,
        FLOWS_TERMINATION_MESSAGE_V2_SENDING_ENABLED,
        FLOWS_WA_WEB,
        FLOWS_WA_WEB_AGM_CTA,
        FLOWS_WA_WEB_RESPONSES_DOWNLOAD,
        FMX_CTWA_KILL_SWITCH,
        FORWARDED_MESSAGE_USER_JOURNEY_LOGGING_ENABLED,
        FOUR_REACTIONS_IN_BUBBLE_ENABLED,
        FT_VALIDATION_FAILURE_DROP_PLACEHOLDER,
        FULLSCREEN_ANIMATION_FOR_KEYWORD,
        FUNCTIONAL_CHATLIST_ENABLED,
        FUTUREPROOF_ASSOCIATED_CHILD_ENABLED,
        GC_DEVICE_SWITCHING_KILLSWITCH,
        GENAI_EARLY_AUDIO_PRE_BUF_SIZE,
        GIF_MAX_PLAY_DURATION,
        GIF_MAX_PLAY_LOOPS,
        GIF_MIN_PLAY_LOOPS,
        GIF_PROVIDER,
        GIMMICK_PHASE_TWO_DATA_SUFFIX,
        GIPHY_PMA_SHUTOFF_ENABLED,
        GRAPHQL_GET_PRODUCT_LIST,
        GRAPHQL_LOCALE_REMAPPING,
        GROUP_CALL_MAX_PARTICIPANTS,
        GROUP_CALLING_WAVE_RECEIVING_ENABLED,
        GROUP_CALLING_WAVE_SENDING_ENABLED,
        GROUP_CATCH_UP,
        GROUP_CREATE_ADD_USING_LID_JIDS,
        GROUP_DESCRIPTION_LENGTH,
        GROUP_FROM_GROUP,
        GROUP_HISTORY_AFTER_JOIN_PREREQUISITES,
        GROUP_HISTORY_BUMP_MESSAGE_ID,
        GROUP_HISTORY_BUNDLE_TIME_LIMIT_RECEIVER_ENFORCEMENT_SECS,
        GROUP_HISTORY_MESSAGE_COUNT_LIMIT,
        GROUP_HISTORY_MESSAGE_COUNT_RECEIVER_UPPER_LIMIT,
        GROUP_HISTORY_MESSAGES_TIME_LIMIT_RECEIVER_ENFORCEMENT_SECS,
        GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS,
        GROUP_HISTORY_NEW_USER_THRESHOLD_RECEIVER_ENFORCEMENT_SECS,
        GROUP_HISTORY_NEW_USER_THRESHOLD_SECS,
        GROUP_HISTORY_NOTICE_RECEIVE,
        GROUP_HISTORY_OUT_OF_WINDOW_PIN_SENDER,
        GROUP_HISTORY_OUT_OF_WINDOW_PINS_RECEIVER,
        GROUP_HISTORY_RECEIVE,
        GROUP_HISTORY_RECEIVER_DEDUP,
        GROUP_HISTORY_RECEIVER_FLOATING_BANNER,
        GROUP_HISTORY_REPORTING,
        GROUP_HISTORY_SEND,
        GROUP_HISTORY_SEND_AFTER_JOIN,
        GROUP_HISTORY_SETTING_DECOUPLE_ENABLED,
        GROUP_HISTORY_SETTINGS,
        GROUP_HISTORY_SETTINGS_QUERY,
        GROUP_HISTORY_SETTINGS_TOGGLE_UI,
        GROUP_HISTORY_SUPPORT_HISTORY_SYNC_RECEIVER_PRE_CHAT,
        GROUP_JOIN_REQUEST_CAN_SEND_OPTIONAL_MESSAGE,
        GROUP_JOIN_REQUEST_CAN_VIEW_OPTIONAL_MESSAGE,
        GROUP_JOIN_REQUEST_M2_BANNER_ON_CONVERSATION,
        GROUP_MAX_SUBJECT,
        GROUP_MEMBER_UPDATES_HIDE_IN_THREAD_ENABLED,
        GROUP_MEMBER_UPDATES_PAST_PARTICIPANT_MIGRATION_ENABLED,
        GROUP_MEMBER_UPDATES_USERNAME_DESCRIPTION_ENABLED,
        GROUP_MEMBER_UPDATES_USERNAMES_DB_ENABLED,
        GROUP_MEMBER_UPDATES_USERNAMES_ENABLED,
        GROUP_MEMBER_UPDATES_USERNAMES_UI_ENABLED,
        GROUP_SIZE_BYPASSING_SAMPLING,
        GROUP_SIZE_LIMIT,
        GROUP_STATUS_RECEIVER_ENABLED,
        GROUP_SUSPEND_APPEAL_INCLUDE_ENTITY_ID_ENABLED,
        GROUP_SUSPEND_V2_ENABLED,
        GROUP_SUSPENSION_APPEALS_REDESIGN_ENABLED,
        GROUP_SUSPENSION_APPEALS_REDESIGN_VARIANT_ENABLE,
        GROUP_USERNAME_UPDATES_AS_MEMBER_UPDATES_ENABLED,
        HAND_RAISE_RECEIVER_ENABLED,
        HARMFUL_FILE_DIALOG_LOGGING,
        HASH_IDENTITY_KEYS_FOR_QR_CODE_DEVICE_VERIFICATION,
        HATCH_PAIRING_FROM_COMPANION_ENABLED,
        HD_VIDEO_DEFINITION_MAX_EDGE,
        HD_VIDEO_DEFINITION_MIN_EDGE,
        HD_VIDEO_DEFINITION_MIN_EDGE_WITH_MAX_EDGE,
        HEARTBEAT_INTERVAL_S,
        HIDE_AUTO_QUOTES_ON_WEB,
        HIDE_SILENT_SYSTEM_MESSAGE_ENABLED,
        HISTORY_SYNC_ON_DEMAND,
        HISTORY_SYNC_ON_DEMAND_COMPANION,
        HISTORY_SYNC_ON_DEMAND_COOLDOWN_SEC,
        HISTORY_SYNC_ON_DEMAND_FAILURE_LIMIT,
        HISTORY_SYNC_ON_DEMAND_MESSAGE_COUNT,
        HISTORY_SYNC_ON_DEMAND_TIME_BOUNDARY_DAYS_DESKTOPS,
        HISTORY_SYNC_ON_DEMAND_TIMEOUT_MS,
        HISTORY_SYNC_ON_DEMAND_WITH_ANDROID_BETA,
        HOSTED_MESSAGE_FLAG_ENABLED,
        HSM_TAG_IN_HISTORY_SYNC_DESERIALIZATION_ENABLED,
        HYBRID_EDUCATIONAL_DIALOG_START_AT,
        HYBRID_EDUCATIONAL_DIALOGS_ENABLED,
        HYBRID_FLYTRAP_FEEDBACK_ENABLED,
        HYBRID_FONT_SIZE_DROPDOWN,
        HYBRID_INCREMENTAL_ZOOMING_SIMPLE_ENABLED,
        HYBRID_NUX_BETA_50_ENABLED,
        IGNORE_JOINABLE_TERMINATE_ON_EXPIRED_OFFER,
        IGNORE_ONE_TO_ONE_TERMINATE_IN_GROUP_CALL,
        IM_BLOKS_WIDGET_ENABLE,
        IM_NFM_MULTI_STEP_FORM_KILLSWITCH,
        IMP_SEND_SIGNAL_POST_CONNECT_DELAY,
        IMP_SEND_SIGNAL_POST_CONNECT_WEBC_ENABLED,
        IMPROVE_GROUP_REPORTING,
        IMPROVE_SUBGROUP_ACTIVATION_SUBGROUP_POLL_INTERVAL,
        IN_APP_BUG_REPORTING_DESCRIPTION_GOOD_QUALITY_CHARS,
        IN_APP_BUG_REPORTING_SHOW_QUALITY_HINTS_V1,
        IN_APP_COMMS_MANAGE_ADS_WEB_BANNER_CAMPAIGN_ENABLED,
        IN_APP_SUPPORT_CAPI_NUMBER_PREFIXES,
        IN_APP_SUPPORT_V2_NUMBER_PREFIXES,
        INAPP_SIGNUP_AGM_CTA_EXPERIMENT,
        INAPP_SIGNUP_CONFIRMATION_MESSAGE_ENABLED,
        INAPP_SIGNUP_M1_LOGGING_ENABLED,
        INAPP_SIGNUP_QPL_LOGGING_ENABLED,
        INAPP_SIGNUP_WEB_CTA_LOGGING_ENABLED,
        INBOX_FILTERS_CUSTOM_SMB_ENABLED,
        INBOX_FILTERS_ENABLED,
        INBOX_FILTERS_HAPTIC_FEEDBACK_ENABLED,
        INBOX_FILTERS_READ_UNREAD_LOGGING_ENABLED,
        INBOX_FILTERS_RESET_TIMEOUT,
        INBOX_FILTERS_SMB_ENABLED,
        INBOX_FILTERS_SUPPRESS_CONTACT_FILTER,
        INFO_DRAWER_REFRESH,
        INTEGRITY_CHECKPOINTS_DEFAULT_ENABLED,
        INTEGRITY_CHECKPOINTS_ENABLED,
        INTERACTIVE_BLOKS_WIDGET_WEB_ENABLED,
        INTERACTIVE_MESSAGE_NATIVE_FLOW_KILLSWITCH,
        INTERACTIVE_RESPONSE_MESSAGE_KILLSWITCH,
        INTERACTIVE_RESPONSE_MESSAGE_NATIVE_FLOW_KILLSWITCH,
        INTERNAL_GROUP_INDICATOR,
        INVITE_DEACTIVATED_USER_WEB,
        IS_AI_MODE_SELECTOR_VISIBLE,
        IS_EXPAND_FMX_ACCOUNT_AGE_BOLDED_NON_AUTO_EXPOSE,
        IS_EXPAND_FMX_ACCOUNT_AGE_UI_ENABLED,
        IS_EXPAND_FMX_ENABLED_NON_AUTO_EXPOSE,
        IS_EXPAND_FMX_MEX_ENABLED,
        IS_INDIVIDUAL_SUSPICIOUS_FMX_ENABLED,
        IS_INTERNAL_TESTER,
        IS_META_EMPLOYEE_OR_INTERNAL_TESTER,
        IS_PART_OF_GSC_EXPERIMENT,
        IS_PMX_FUNNEL_METRICS_LOGGING_ENABLED,
        IS_PMX_HASHED_MSG_KEY_LOGGING_ENABLED,
        IS_SPOILER_RICH_FORMAT_ENABLED,
        IS_SPOILER_RICH_FORMAT_SENDER_ENABLED,
        JOINABLE_CLIENT_POLL_INTERVAL_MIN,
        KALEIDOSCOPE_THUMBNAIL_VALIDATION,
        KEEP_IN_CHAT_UNDO_DURATION_LIMIT,
        KILL_SWITCH_CTWA_ML_ENTRY_POINT_CONFIG,
        KMP_SYNCD_ENGINE_CRYPTO_ENABLED,
        KMP_SYNCD_ENGINE_OUTGOING_PROCESSOR_ENABLED,
        KS_USE_COMPONENT_MODEL,
        LARGE_SCREENS_NEW_CHAT_BUTTON_VARIANTS,
        LAZY_SYSTEM_MESSAGE_INSERTION_ENABLED,
        LID_GROUP_CREATION_ADDRESSING_MODE_OVERRIDE,
        LID_GROUP_MIGRATION_NON_MEMBER_IQ,
        LID_MIGRATION_FOR_BIZ_PROFILE_ENABLED,
        LID_MIGRATION_FOR_VNAME_ENABLED,
        LID_MIGRATION_NOTIFICATIONS_ENABLED,
        LID_ONE_ON_ONE_MIGRATION_COMPATIBLE,
        LID_ONE_ON_ONE_MIGRATION_ENABLED,
        LID_ONE_ON_ONE_MIGRATION_LOG_OUT_ON_MISMATCH,
        LID_ONE_ON_ONE_MIGRATION_PEER_SYNC_TIMEOUT_IN_SECONDS,
        LID_ONE_TO_ONE_MIGRATION_EVENT_RESPONSE_FORCE_PN_JID,
        LID_PN_USERNAME_MAPPING_LOGGING_ENABLED,
        LID_STATUS_NON_SOAKED_CLIENT_SUPPORT_ENABLED,
        LID_STATUS_SEND_ENABLED,
        LID_TRUSTED_TOKEN_ISSUE_TO_LID,
        LIGHTWEIGHT_GROUP_CREATION,
        LIMIT_SHARING_ENABLED_FOR_1ON1_CHAT,
        LIMIT_SHARING_PROTOCOL_MESSAGE_RECEIVER_ENABLED,
        LIMIT_SHARING_UPDATE_ENABLED_WEB,
        LINK_PREVIEW_WAIT_TIME,
        LISTS_CHAT_LIST_ROW_PILL_ENABLED,
        LISTS_SMB_ENABLED,
        LISTS_SMB_WEB_ENABLED,
        LISTS_SMB_WEB_M2_ENABLED,
        LOBBY_TIMEOUT_MIN,
        LOG_CLOCK_SKEW,
        LOW_CACHE_HIT_RATE_MEDIA_TYPES,
        LTHASH_CHECK_HOURS,
        M2_AUDIENCE_DYNAMIC_RULES,
        MARK_AS_VERIFIED_ENABLED,
        MAX_GROUP_SIZE_FOR_LONG_RINGTONE,
        MAX_NUM_PARTICIPANTS_FOR_SS,
        MAXIMUM_GROUP_SIZE_FOR_RCAT,
        MAY_HAVE_MESSAGES_ENABLED,
        MC_ENABLED,
        MD_APP_STATE_GATE_D34336913,
        MD_ICDC_HASH_LENGTH,
        MD_OFFLINE_V2_M2_ENABLED,
        MD_SYNCD_BUNDLE_LOGGING,
        MD_SYNCD_MUTATION_LOGGING,
        MD_SYNCD_MUTATION_SUMMARY_LOGGING,
        MEDIA_FORCE_TRANSCODE_ON_ELST,
        MEDIA_HUB_HISTORY_MAX_DAYS,
        MEDIA_LARGE_FILE_AWARENESS_POPUP_FILE_SIZE_IN_MB,
        MEDIA_PICKER_SELECT_LIMIT,
        MEDIA_PICKER_SELECT_LIMIT_NEW,
        MEDIA_VIEWER_ACCELERATED_PLAYBACK_ENABLED,
        MEMBER_NAME_TAG_DB_ENABLED,
        MEMBER_NAME_TAG_RECEIVER_ENABLED,
        MEMBER_NAME_TAG_SENDER_ENABLED,
        MEMBER_NAME_TAG_WEB_RECEIVER_ENABLED,
        MEMBER_NAME_TAG_WEB_SENDER_ENABLED,
        MESSAGE_ASSOCIATION_INFRA_ENABLED,
        MESSAGE_CAPPING_UPSELL_VERSION,
        MESSAGE_COUNT_LOGGING_MD_ENABLED,
        MESSAGE_EDIT_CLIENT_ENTRY_POINT_LIMIT_SECONDS,
        MESSAGE_EDIT_TO_MESSAGE_SECRET_RECEIVER_ENABLED,
        MESSAGE_EDIT_TO_MESSAGE_SECRET_SENDER_ENABLED,
        MESSAGE_EDIT_WINDOW_DURATION_SECONDS,
        MESSAGE_KEYS_ASYNC_CHUNK_SIZE,
        MESSAGE_PARTIAL_SELECTION_M2,
        META_AI_IN_APP_SURVEY_ENABLED,
        META_CATALOG_LINKING_M2_ENABLED,
        META_VERIFIED_BADGE_EDUCATION_VAI_CONTENT,
        MEX_GET_PRIVACY_CONTACT_LIST_ENABLED,
        MEX_GET_PRIVACY_SETTINGS_MODE,
        MEX_PHASE3_ENABLED,
        MEX_PHASE3_STATUS_FLAGS,
        MEX_USYNC_ABOUT_STATUS,
        MEX_USYNC_USERNAME_QUERY,
        ML_MODEL_DOWNLOAD_SKIP_HASH_CHECK,
        MM_1PD_POST_DC_DEPTH_LIMIT,
        MM_1PD_POST_DC_NEW_SCHEMA_ENABLED,
        MM_1PD_POST_DC_OLD_SCHEMA_DISABLED,
        MM_DATA_SHARING_DISCLOSURE_ENABLED,
        MM_DATA_SHARING_DISCLOSURE_ENABLED_ADDITIONAL_TRANSPARENCY_LARGE_SCREENS,
        MM_DATA_SHARING_DISCLOSURE_ENABLED_COMPANION_HISTORY_SYNC,
        MM_DATA_SHARING_DISCLOSURE_ON_CHAT_OPEN_ENABLED,
        MM_DISCLOSURE_HANDLE_TOS_FAILURES_ENABLED,
        MM_DISCLOSURE_LEARN_MORE_ARTICLE_ID,
        MM_MESSAGE_LEVEL_FEEDBACK_ENABLED,
        MM_MESSAGE_LEVEL_FEEDBACK_NOT_INTERESTED_MENU_ENABLED,
        MM_OPT_OUT_ENABLED,
        MM_OPT_OUT_FMX_STOP_FOR_HIGH_TRUST,
        MM_OPT_OUT_LID_MIGRATION_ENABLED,
        MM_OPTIMIZED_DELIVERY_APP_CTA_ENABLED,
        MM_OPTIMIZED_DELIVERY_ARCHIVE_SIGNAL_SHARING_ENABLED,
        MM_OPTIMIZED_DELIVERY_REPLACING_SHIMMED_LINKS_ENABLED,
        MM_OPTIMIZED_DELIVERY_TOKEN_FALLBACK_DISABLED,
        MM_OPTIMIZED_DELIVERY_UNIQUE_TOKEN_PER_MESSAGE_ID_ENABLED,
        MM_SIGNAL_SHARING_COLLECTION_WINDOW_LOGGING_ENABLED,
        MM_SIGNAL_SHARING_VERIFICATION_NEW_SIGNAL_TYPE_ORIGIN,
        MM_SIGNAL_SHARING_VERIFICATION_SYSTEM_LID_ENABLED,
        MM_TAP_TARGET_BLOKS_CLIENT_HYDRATION_ENABLED,
        MM_TEMPLATE_MESSAGE_TELEMETRY_IS_FIRST_MM_ENABLED,
        MM_USER_CONTROLS_ENTRY_POINTS_UPDATE_M1_ICON,
        MM_USER_CONTROLS_ENTRY_POINTS_UPDATE_M1_MENU,
        MM_USER_CONTROLS_EXCEPTION_NUMBER_PREFIXES,
        MM_USER_CONTROLS_EXPOSURE,
        MMS_VCACHE_AGGREGATION_ENABLED,
        MUSIC_OHAI_PROXY_URL,
        NATIVE_CONTACT_COMPANION_CHANGE_ENABLED,
        NATIVE_CONTACT_COMPANION_NUX_LEARN_MORE_ARTICLE_ID,
        NATIVE_FLOW_RESPONSE_MESSAGE_PARAMS_JSON_MAX_SIZE,
        NATIVE_LIB_SANDBOXING_ENABLE_LIBWEBP,
        NEW_CHAT_MSG_CAPPING_FIRST_WARNING_THRESHOLD_PERCENTAGE,
        NEW_END_CALL_SURVEY_POP_UP_USER_INTERVAL_S,
        NEWSLETTER_ADMIN_INVITE_NUX_ID,
        NEWSLETTER_ADMIN_INVITE_TOS_ID,
        NEWSLETTER_ADMIN_INVITE_TOS_ID_SMB_WEB,
        NEWSLETTER_CREATION_NUX_ID,
        NEWSLETTER_CREATION_TOS_ID,
        NEWSLETTER_CREATION_TOS_ID_SMB_WEB,
        NEWSLETTER_FORWARD_COUNTER_BUMP_FORWARDS_TO_SELF,
        NEWSLETTER_FORWARD_COUNTER_BUMP_OWN_CHANNEL_UPDATES_FOWARDS,
        NEWSLETTER_FORWARD_COUNTER_BUMP_SECOND_ORDER_FORWARDS,
        NEWSLETTER_FORWARD_COUNTER_INFRA_ENABLED,
        NEWSLETTER_FORWARD_COUNTER_MAX_SEND_AFTER_RANDOM_TIME,
        NEWSLETTER_FORWARD_COUNTER_UI_ENABLED,
        NEWSLETTER_NUX_NOTICE_ID,
        NEWSLETTER_RCAT_FIELD_GENERATING_ENABLED,
        NEWSLETTER_STATUS_CREATION_ENABLED,
        NEWSLETTER_STATUS_RECEIVER_ENABLED,
        NEWSLETTER_TOS_NOTICE_ID,
        NEWSLETTER_TOS_NOTICE_ID_SMB_WEB,
        NEWSLETTERS_VIDEO_PLAYBACK_WABBA_LOGGING_ENABLED,
        NO_LARGE_EMOJI_REGEX,
        NOISE_PQ_MODE,
        NON_WA_CONTACT_INVITE_CTA_ENABLED,
        NOTIFICATION_HIGHLIGHT_GROUP_SIZE_THRESHOLD,
        NUM_DAYS_BEFORE_DEVICE_EXPIRY_CHECK,
        NUM_DAYS_KEY_INDEX_LIST_EXPIRATION,
        OHAI_REQUEST_KB_SIZE,
        OPTIMIZED_DELIVERY_BLOCK_AND_REPORT_ENTRY_POINTS_ALLOWLIST_WEB,
        OPTIMIZED_DELIVERY_MULTIPLE_COLLECTION_WINDOWS_ENABLED,
        OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_CONFIG,
        OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_ENABLED,
        OPTIMIZED_DELIVERY_SIGNAL_COLLECTION_ON_COMPANIONS_ENABLED,
        OPTIMIZED_DELIVERY_TOKENS_STORAGE_CONFIG,
        OPUS_ADMIN,
        OPUS_ENABLED,
        OPUS_T,
        OPUS_TIME,
        ORDER_DETAILS_CUSTOM_ITEM_ENABLED,
        ORDER_DETAILS_FROM_CART_ENABLED,
        ORDER_DETAILS_FROM_CATALOG_ENABLED,
        ORDER_DETAILS_PAYMENT_INSTRUCTIONS_SYNC_ENABLED,
        ORDER_DETAILS_QUICK_PAY,
        ORDER_DETAILS_TOTAL_MAXIMUM_VALUE,
        ORDER_DETAILS_TOTAL_ORDER_MINIMUM_VALUE,
        ORDER_MANAGEMENT_ENABLED,
        ORDER_MESSAGES_EPHEMERAL_EXCEPTION_ENABLED,
        ORDER_STATUSES_REVAMP_M1_ENABLED,
        ORDERS_EXPANSION_RECEIVER_COUNTRIES_ALLOWED,
        ORIGINAL_QUALITY_IMAGE_MIN_EDGE,
        OTP_LID_MIGRATION_ENABLED,
        OUT_CONTACT_INVITES_ENABLED,
        OUT_OF_SYNC_DISAPPEARING_MESSAGES_LOGGING,
        P2B_CALLING_AVAILABILITY_EXPERIMENT_ENABLED,
        P2M_EXTERNAL_PAYMENTS_LINK_ENABLED,
        P2P_PILLS_ALLOWLIST,
        P2P_PILLS_ALLOWLIST_ENTRIES,
        P2P_PILLS_AUTO_SEND_MESSAGES,
        P2P_PILLS_ENABLED,
        P2P_PILLS_ENABLED_FOR_INELIGIBLE_CONTACTS,
        P2P_PILLS_ENTRIES,
        P2P_PILLS_ENTRIES_ENABLED,
        P2P_PILLS_GRAPHQL_ENABLED,
        P2P_PILLS_MAX_WAIT_ON_CONTACT_CARD_SEND,
        P2P_PILLS_NEW_BUSINESS_METADATA_ENABLED,
        PAA_SUPPORT_FOR_DISABLED_EPEHEMERALITY,
        PARENT_GROUP_ADMINS_LIMIT,
        PARENT_GROUP_ALLOW_MEMBER_SUGGEST_EXISTING_M3_RECEIVER,
        PARENT_GROUP_ALLOW_MEMBER_SUGGEST_EXISTING_M3_SENDER,
        PARENT_GROUP_ANNOUNCEMENT_COMMENTS_HISTORY_SYNC_RECEIVER_ENABLED,
        PARENT_GROUP_CREATE_PRIVACY,
        PARENT_GROUP_LINK_LIMIT,
        PARENT_GROUP_LINK_LIMIT_COMMUNITY_CREATION,
        PARENT_GROUP_MIN_PARTICIPANTS_FOR_GROUP_ENTRY_POINT,
        PARENT_GROUP_SUBGROUP_FILTER,
        PARENT_GROUP_VIEW_ENABLED,
        PARENT_GROUP_VIEW_ENABLED_FOR_SMB_ON_WEB,
        PARSE_ENCRYPTED_DSM_MSG_FIX,
        PAYMENT_BR_HOLDOUT,
        PAYMENT_LINK_TRACE_ID_LOGGING_ENABLED,
        PAYMENT_LINKS_TRUST_SIGNALS_METATAG_ENABLED,
        PAYMENT_LINKS_TRUST_SIGNALS_METATAG_PSP_LIST,
        PAYMENT_LINKS_TRUST_SIGNALS_OTHER_METATAG_KILL_SWITCH_ENABLED,
        PAYMENT_LINKS_TRUST_SIGNALS_OTHER_METATAGS_ENABLED,
        PAYMENT_SUPPORT_LIDS,
        PAYMENTS_BR_CONTENT_OPTIMIZATION_VARIANT,
        PAYMENTS_BR_COPY_PIX_CODE_API_MERCHANT_ENABLED,
        PAYMENTS_BR_FORCE_COPY_PIX_CTA_ENABLED,
        PAYMENTS_BR_MERCHANT_PSP_ACCOUNT_STATUS_SYNC,
        PAYMENTS_BR_P2M_BOLETO_ENABLED,
        PAYMENTS_BR_P2M_BUYER_LOGGING_PHASE_2,
        PAYMENTS_BR_P2M_COMPLETED_PAYMENT_INTENT_BUYER_LOGGING,
        PAYMENTS_BR_P2M_COPY_BOLETO_CODE_BUYER_LOGGING,
        PAYMENTS_BR_P2M_ORDER_DETAILS_BUYER_LOGGING,
        PAYMENTS_BR_P2M_PAY_NOW_BUYER_LOGGING,
        PAYMENTS_BR_P2M_PIX_COPY_CODE_BUYER_LOGGING,
        PAYMENTS_BR_P2M_PIX_COPY_KEY_BUYER_LOGGING,
        PAYMENTS_BR_P2M_PIX_IN_GROUPS_BUYER_LOGGING,
        PAYMENTS_BR_P2M_PIX_MORE_WAYS_TO_PAY_BUYER_LOGGING,
        PAYMENTS_BR_P2M_VIEW_ORDER_BUYER_LOGGING,
        PAYMENTS_BR_P2P_PIX_COPY_CODE_BUYER_LOGGING,
        PAYMENTS_BR_P2P_PIX_COPY_KEY_BUYER_LOGGING,
        PAYMENTS_BR_PAYMENT_LINKS_BUYER_LOGGING,
        PAYMENTS_BR_PIX_ON_WEB,
        PAYMENTS_BR_PIX_PHASE_1_SELLER_SYNC_ENABLED,
        PAYMENTS_BR_PIX_QUICK_REPLY_ENABLED,
        PAYMENTS_BR_PIX_WEB_ATTACHMENT_TRAY,
        PAYMENTS_LINK_TO_LITE_CONSUMER_ENABLED,
        PAYMENTS_MERCHANT_GLOBAL_ORDERS_VALUE_PROPS_BANNER_ENABLED,
        PAYMENTS_UPR_BUBBLE_COUNTRIES,
        PAYMENTS_UPR_CUSTOM_PAYMENT_METHODS_SYNC_COUNTRIES,
        PAYMENTS_UPR_EGYPT_ENABLED,
        PAYMENTS_UPR_HONGKONG_ENABLED,
        PAYMENTS_UPR_ID_ENABLED,
        PAYMENTS_UPR_MEXICO_WALLET_ENABLED,
        PAYMENTS_UPR_MULTIPLE_KEY_COPY_ENABLED,
        PAYMENTS_UPR_MX_ENABLED,
        PAYMENTS_UPR_SEND_KEY_FROM_WEB,
        PAYMENTS_UPR_TAIWAN_ENABLED,
        PAYMENTS_UPR_TURKEY_ENABLED,
        PAYMENTS_UPR_UAE_ENABLED,
        PEER_MESSAGE_LID_MIGRATION_OUTGOING,
        PENDING_GROUP_REQUESTS_PERSISTENT_BANNER,
        PER_CUSTOMER_DATA_SHARING_CONTROLS_ELIGIBLE,
        PHONE_NUMBER_SHARING_FLOW,
        PINNED_MESSAGES_INFINITE_RECEIVER_ENABLED,
        PINNED_MESSAGES_INFINITE_SENDER_ENABLED,
        PINNED_MESSAGES_M0,
        PINNED_MESSAGES_M1_RECEIVER,
        PINNED_MESSAGES_M1_SENDER,
        PINNED_MESSAGES_M2,
        PINNED_MESSAGES_M2_IMAGE_THUMBNAIL,
        PINNED_MESSAGES_M2_PIN_MAX,
        PINNED_MESSAGES_SENDER_SHORT_EXPIRY_DURATIONS_ENABLED,
        PIX_ONBOARDING_NEW_CONTENT_ENABLED,
        PIX_PAYMENT_REQUEST_UPDATE_STATUS_ENABLED,
        PLACEHOLDER_MESSAGE_KEY_HASH_LOGGING,
        PLACEHOLDER_MESSAGE_RESEND,
        PLACEHOLDER_MESSAGE_RESEND_MAXIMUM_DAYS_LIMIT,
        PNH_CAG_DISABLE_POLLS_GROUP_SIZE,
        PNH_CAG_DISABLE_REACTIONS_GROUP_SIZE,
        PNH_HISTORY_SYNC_FORCE_GENERAL,
        PNH_PN_FOR_LID_CHAT_SYNC,
        PNH_THREAD_PROMOTION_TO_GENERAL_LID,
        POLL_ADD_OPTION_ENABLED,
        POLL_ADD_OPTION_RECEIVING_ENABLED,
        POLL_CREATION_CAG_ENABLED,
        POLL_CREATOR_EDIT_ENABLED,
        POLL_CREATOR_EDIT_RECEIVING_VERSION,
        POLL_END_TIME_ENABLED,
        POLL_END_TIME_RECEIVING_ENABLED,
        POLL_HIDE_VOTERS_ENABLED,
        POLL_HIDE_VOTERS_RECEIVING_ENABLED,
        POLL_NAME_LENGTH,
        POLL_OPTION_COUNT,
        POLL_OPTION_LENGTH,
        POLL_RECEIVING_CAG_ENABLED,
        POLL_RESULT_SNAPSHOT_POLLTYPE_ENVELOPE_ENABLED,
        POLL_TC_RECEIVING_ENABLED,
        POLL_TC_SENDING_ENABLED,
        PREMIUM_BLUE_ENABLED,
        PREMIUM_BROADCAST_SMB_CAPPING_ENABLED,
        PREMIUM_MSG_BB_CAMPAIGN_SYNC_ENABLED,
        PRIVACY_SCREEN_ENABLED,
        PRIVACY_SETTINGS_ABOUT_LID_MIGRATION_ENABLE,
        PRIVACY_SETTINGS_GROUP_ADD_LID_MIGRATION_ENABLE,
        PRIVACY_SETTINGS_PRESENCE_LID_MIGRATION_ENABLE,
        PRIVACY_SETTINGS_PROFILE_LID_MIGRATION_ENABLE,
        PRIVACY_TIPS_GROUPS_BUILD,
        PRIVACY_TIPS_KILLSWITCH,
        PRIVACY_TIPS_PROFILE_BUILD,
        PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES,
        PRIVACY_TOKEN_SENDING_ON_GROUP_CREATE,
        PRIVACY_TOKEN_SENDING_ON_GROUP_PARTICIPANT_ADD,
        PRIVATE_MESSAGING_UK_OSA_ENABLED,
        PRIVATE_OSA_REPORTING_ENABLED,
        PROFILE_PICTURE_DEEPLINK_ENABLED,
        PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_IQ,
        PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_USYNC,
        PTT_USER_JOURNEY_LOGGING_WAM_ENABLED,
        PTV_AUTOPLAY_ENABLED,
        PTV_AUTOPLAY_LOOP_LIMIT,
        PTV_MAX_DURATION_SECONDS,
        PTV_QUOTED_REPLIES_CUTOUT_ENABLED,
        PUBLIC_BUG_REPORTING_SIDEBAR,
        PUSHNAME_BLOCKLIST_STARTING_WITH_AT,
        QP_BANNER_STICKER_ANIMATION_ENABLED,
        QP_CAMPAIGN_CLIENT_ENABLED,
        QUICK_PROMOTION_BANNER_CLIENT_ENABLED,
        QUOTED_MESSAGE_USER_JOURNEY_LOGGING_ENABLED,
        RASTERIZE_TEXT_STATUS_PIXEL_WIDTH,
        REACTION_USER_JOURNEY_LOGGING_ENABLED,
        REACTIONS_ALIGNMENT_FOR_TRANSPARENT_MESSAGES_ENABLED,
        REACTIONS_CHAT_PREVIEW,
        REACTIONS_RECEIVER_ENABLED,
        RECEIPT_MODE_BITMASK_ENABLED,
        RECOMMENDED_CHANNELS_BACKGROUND_REFRESH,
        RELAX_INTEGRITY_CONSTRAINTS_FOR_BB_WA_TENURED_ACCOUNTS,
        REMOVE_DEVICE_PN_DEPENDENCIES,
        REMOVE_PN_DEPENDENCIES,
        RENDER_UPDATED_DISCLOSURE,
        REPORT_BLOCK_IMPROVEMENTS_FOR_GROUPS_ENABLED,
        REPORT_CALL_REPLAYER_ID,
        REPORT_TO_ADMIN_ENABLED,
        REPORT_TO_ADMIN_KILL_SWITCH,
        REUSE_CACHED_CERTS_FOR_DATA_CHANNEL,
        RICH_ORDER_STATUS_WA_WEB,
        RNR_DAYS_COOLDOWN,
        RNR_MIN_DAYS_USER_ACTIVE,
        ROW_BUYER_ORDER_REVAMP_M0_ENABLED,
        RT_CLEAN_REPORTING_TAG,
        RT_CLEAN_REPORTING_TOKEN,
        RT_EDIT_RECEIVE,
        RT_GHS_RECEIVER_ENABLED,
        RT_GHS_SENDER_ENABLED,
        RT_RECEIVE_REPORTING_TAG,
        RT_RECEIVER_DUAL_ENCRYPTED_MSG_ENABLED,
        RT_REPORT_TOKEN_FROM_INCLUSION_LIST,
        RT_SENDER_DUAL_ENCRYPTED_MSG_ENABLED,
        RT_SENDER_REPORTING_TOKEN_VERSION,
        RT_SWAPPED_FALLBACK_VALIDATION,
        RT_SYNC_REPORTING_TAG,
        RT_WEB_DELAY_PROCESSING,
        SAGA_COPY,
        SAGA_ENABLED,
        SAGA_MESSAGE_FEEDBACK_USING_CANONICAL_ENT,
        SAGA_PROTOBUF_AI_STARDUST_WEB,
        SAGA_PROTOBUF_SHOW_SYSMSG_WEB,
        SAGA_V1_CAROUSEL,
        SAGA_V1_ENABLED,
        SAGA_V1_NUX_ENABLED,
        SAGA_V1_REENGAGEMENT_ENABLED,
        SCHEDULE_CALL_SHOW_JOIN_BUTTON_TIME_INTERVAL_MINS,
        SCHEDULE_CALL_SHOW_UPCOMING_BANNER_TIME_INTERVAL_MINS,
        SCHEDULED_MESSAGES_PHOTO_VIDEO_SENDER_ENABLED,
        SCHEDULED_MESSAGES_RECEIVER_ENABLED,
        SCHEDULED_MESSAGES_SENDER_ENABLED,
        SCHEDULED_MESSAGES_WINDOW_DURATION_MAX_SECONDS,
        SCHEDULED_MESSAGES_WINDOW_DURATION_MIN_SECONDS,
        SEARCH_THE_WEB_DESIGN_EXPERIMENT_V1,
        SEARCH_THE_WEB_DIALOG_REDESIGN,
        SEARCH_THE_WEB_IMAGE_SEARCH,
        SEARCH_THE_WEB_TEXT_SEARCH,
        SEARCH_THE_WEB_URL_OFFER,
        SEARCH_USER_JOURNEY_LOGGING_WAM_ENABLED,
        SECURITY_FIXES_BITMAP,
        SELLER_ORDERS_MANAGEMENT_REVAMP,
        SEND_CAG_MEMBER_REVOKES_AS_GDM,
        SEND_EXTENDED_NACK_ENABLED,
        SEND_INVALID_PROTOBUF_NACK_FAILURE_REASON,
        SERVER_DRIVEN_COPY_M2,
        SERVICE_IMPROVEMENT_OPT_OUT_FLAG,
        SETTINGS_SYNC_ENABLED,
        SFU_SECONDARY_REMOTE_BWE_IMPL,
        SHARE_OWN_PN_SYNC,
        SHARE_PHONE_NUMBER_ON_CART_SEND_TO_DIRECT_CONNECTION_BIZ_ENABLED,
        SHIMMED_LINKS_IN_THE_MARKETING_MESSAGE_BODY_ENABLED,
        SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_ASSERTION_TIMEOUT_SECONDS,
        SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_ENABLED,
        SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_HANDOFF_ENABLED,
        SHORTCAKE_COMPANION_PROLOGUE_PASSKEYS_REQUEST_OPTIONS_TTL_SECONDS,
        SHOW_INTEGRITY_SCREENSHARING_FRICTION_UI,
        SILENT_GROUP_USERNAME_ACTIVITIES_ENABLED,
        SIMILAR_CHANNELS_IN_CHANNEL_DETAILS_ENABLED,
        SIMILAR_CHANNELS_IN_THREAD_ON_FOLLOW_ENABLED,
        SIMILAR_CHANNELS_MAX_LIMIT,
        SIMILAR_CHANNELS_MIN_LIMIT,
        SINGLE_E2EE_SESSION_MIGRATION_STATE_INCOMING,
        SINGLE_E2EE_SESSION_MIGRATION_STATE_OUTGOING,
        SINGLE_EMOJI_LOGGING_ENABLED,
        SMART_FILTERS_ENABLED,
        SMART_FILTERS_ENABLED_CONSUMER,
        SMB_AGENT_CHAT_LIST_INDICATOR_ENABLED,
        SMB_AGENT_THREAD_CONTROL_NOTIFICATION_ENABLED,
        SMB_AI_AGENTS_WEB_CHAT_ASSIGNMENT_INTEROP_ENABLED,
        SMB_AUTH_AGENTS_FEATURE_CONTROL_ENABLED,
        SMB_BB_WEB_AUDIENCE_EXPRESSION_SYNC_READ,
        SMB_BILLING_ENABLED,
        SMB_BIZ_AI_LISTS_PILLS,
        SMB_BIZ_PROFILE_CUSTOM_URL,
        SMB_BUSINESS_BROADCAST_IMPORT_CONTACT,
        SMB_BUSINESS_BROADCAST_MULTI_AUDIENCE_SEND_WEB,
        SMB_BUSINESS_BROADCAST_PRO_ENABLED,
        SMB_BUSINESS_BROADCAST_SEND_WEB,
        SMB_BUSINESS_BROADCAST_SEND_WEB_NO_EXP,
        SMB_BUSINESS_BROADCAST_SEND_WEB_SMBA,
        SMB_BUSINESS_BROADCAST_SEND_WEB_SMBA_NO_EXP,
        SMB_CATALOG_GRAPHQL_GET_PUBLIC_KEY,
        SMB_CATALOG_GRAPHQL_VERIFY_POSTCODE,
        SMB_CATKIT_QUERY_VERSION,
        SMB_COLLECTIONS_ENABLED,
        SMB_CORE_BIZ_PROFILE_PREVIEW,
        SMB_CORE_BIZ_PROFILE_UX_REFRESHED,
        SMB_CORE_BIZ_PROFILE_UX_REFRESHED_V2,
        SMB_CORE_REC_CARD,
        SMB_CTWA_BILLING_ENABLED,
        SMB_CTWA_IREV_LONG_TERM_HOLDOUT_DUMMY_ENABLED,
        SMB_DO_LABEL_LOCALIZE_BACKFILL_ENABLED_CODE,
        SMB_DO_LABEL_LOCALIZE_ON_CREATE_ENABLED_CODE,
        SMB_ECOMMERCE_COMPLIANCE_INDIA_M4,
        SMB_ECOMMERCE_COMPLIANCE_INDIA_M4_5,
        SMB_GRAPHQL_TO_FETCH_QP_ENABLED,
        SMB_GRAPHQL_TO_FETCH_QP_FREQUENCY_MINS,
        SMB_GRAPHQL_TO_FETCH_QP_SURFACE_IDS,
        SMB_GRAPHQL_TOKEN_RECOVERY_DURING_ACCOUNT_RECOVERY_ENABLED,
        SMB_HIDE_UNSUPPORTED_CURRENCY_PRICE,
        SMB_LABEL_SYNC_CRITICAL_EVENT_LOGGING,
        SMB_LABELS_CTWA_DATA_SHARING,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_CHATS_REORDER_ON_CHAT_ASSIGNMENT_ENABLED,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_CHATS_REORDER_ON_CHAT_UNASSIGNMENT_ENABLED,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_ENABLED,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_NOTIFICATIONS_ENABLED,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_NUX_IMPRESSIONS,
        SMB_MD_AGENT_CHAT_ASSIGNMENT_SYSTEM_MESSAGES_LOGGING_V2_ENABLED,
        SMB_META_VERIFIED_CONTEXT_CARD,
        SMB_MULTI_DEVICE_AGENTS_LOGGING_V2_ENABLED,
        SMB_MULTI_DEVICE_MESSAGE_ATTRIBUTION_ENABLED,
        SMB_NOTES_CONTENT_MAX_LIMIT,
        SMB_NOTES_V1_ENABLED,
        SMB_PAYMENT_LINKS_CTA_BUTTON_KILL_SWITCH,
        SMB_PAYMENT_LINKS_CTA_PSP_LIST,
        SMB_PAYMENT_LINKS_CTA_VARIANT,
        SMB_PAYMENT_LINKS_LOGGING_ENABLED,
        SMB_PAYMENT_LINKS_SELLER_LOGGING_ENABLED,
        SMB_PAYMENT_LINKS_URL_REGEX_LIST,
        SMB_PAYMENT_REQUEST_STATUS_UPDATE,
        SMB_PHASE_OUT_NOT_A_BUSINESS_V2,
        SMB_PREMIUM_MESSAGES_CLICK_LOGGING_ENABLED,
        SMB_PREMIUM_MESSAGES_URL_CTA_ALERT_DIALOG_ENABLED,
        SMB_PRODUCT_COUNTRY_OF_ORIGIN_M1,
        SMB_PROJECT_WALDO_SET_PRICE_TIER_BIZ_PROFILE_ENABLED,
        SMB_QP_CONVERSION_TRACKING_INFRA,
        SMB_QP_EMERGENCY_FORCE_FETCH_NONCE,
        SMB_QP_WEB_DEBUG_RECUNIT,
        SMB_RAMBUTAN_ENABLED,
        SMB_TEMP_COVER_PHOTO_PRIVACY_MESSAGING,
        SMB_TOS_QP_CHATLIST_BANNER,
        SMB_WALDO_SERVICE_OFFERINGS_SELECTION_ENABLED,
        SMB_WEB_BB_HOME_QP_SURFACE_ENABLED,
        SMB_WEB_CATEGORY_SEARCH_VIA_GRAPH_ENABLED,
        SMB_WEB_CUSTOMER_MANAGEMENT_ENABLED,
        SMB_WEB_CUSTOMER_MANAGER_BULK_EDIT_ENABLED,
        SMB_WEB_CUSTOMER_MANAGER_DATE_RANGE_FILTER_ENABLED,
        SMB_WEB_CUSTOMER_MANAGER_DOB_FILTER_ENABLED,
        SMB_WEB_CUSTOMER_MANAGER_EXPORT_ENABLED,
        SMB_WEB_ENABLE_FB_LINKING,
        SMB_WEB_SHOW_QUICK_REPLY_OPTION_IN_COMPOSER,
        SMBA_BB_GENAI_COMPOSER_MIN_WORDS,
        SMBA_BUSINESS_BROADCAST_GENAI_CUSTOM_USER_PROMPT_ENABLED,
        SMBA_BUSINESS_BROADCAST_GENAI_MASTER_ABPROP,
        SMBA_BUSINESS_BROADCAST_GENAI_SHARE_MESSAGE_HISTORY,
        SMBA_BUSINESS_BROADCAST_GENAI_TEXT,
        SMBA_BUSINESS_BROADCAST_GENAI_TEXT_MAX_TRIES,
        SMBA_BUSINESS_BROADCAST_GENAI_TEXT_MODEL,
        SMBA_BUSINESS_BROADCAST_RECIPIENT_LIMIT,
        SMBA_PREMIUM_MESSAGES_LEAVING_WA_CONTENT,
        SMBI_PREMIUM_BROADCAST_MAX_RECIPIENT_LIMIT,
        SMBW_BUSINESS_BROADCAST_DUPLICATE_ENABLED,
        SMBW_BUSINESS_BROADCAST_SMART_COLUMN_DETECTION_ENABLED,
        SMOOTHIE_PERFORMANCE_CSS_DOM,
        SMOOTHIE_PERFORMANCE_MSG_SEND,
        SMOOTHIE_PERFORMANCE_RESIZE_FOLLOWUP,
        SNAPL_NEWSLETTER_LOGGING_ENCRYPTED_RID_ENABLED,
        SNAPL_NEWSLETTER_LOGGING_MEDIA_ID_PLACEHOLDER_STRING,
        SNAPSHOT_RECOVERY_MAX_MUTATIONS_COUNT_ALLOWED,
        SOCCER_BALL_REACTION_FULL_ANIMATION_ENABLED,
        SOCCER_REACTION_IN_TRAY_ENABLED,
        STATUS_ALLOW_FORWARDING_TO_STATUS_ON_WEB,
        STATUS_E2EE_RECV_OVER_STATUS_STANZA,
        STATUS_E2EE_SEND_OVER_STATUS_STANZA,
        STATUS_FUTURE_PROOFING,
        STATUS_INFRA_1_1_SESSION_SPLIT,
        STATUS_LIKES_FIFA_LOTTIE_FULL_SCREEN_ANIMATION_ENABLED,
        STATUS_LIKES_SENDING_ENABLED,
        STATUS_MENTIONS_GROUP_MENTION_RECEIVER,
        STATUS_MENTIONS_RECEIVER,
        STATUS_PLAYER_AVATAR_STATUS_CREATION_ENTRYPOINT,
        STATUS_POG_ID_ROTATION_WINDOW_DAYS,
        STATUS_POSTER_SIDE_GATING_ENABLED,
        STATUS_REACTION_EMOJIS,
        STATUS_SAVE_TO_CAMERA_ROLL_ENABLED,
        STATUS_VIDEO_MAX_DURATION,
        STATUS_WEB_RANKING,
        STICKER_STORE_TESTING_ENABLED,
        STICKERS_EMOJI_TAGGING_ENABLED,
        STICKY_CHAT_PROFILE_PICTURE_ENABLED,
        SUGGESTED_AUDIENCES_WA_WEB,
        SUPPORT_CONTACT_FORM_USING_GRAPHQL,
        SUPPORT_LIDS,
        SUPPORT_MESSAGE_FEEDBACK_ENABLED,
        SUPPORTS_KEEP_IN_CHAT_IN_CAG,
        SYNCD_ADDITIONAL_MUTATIONS_COUNT,
        SYNCD_INLINE_MUTATIONS_MAX_COUNT,
        SYNCD_KEY_MAX_USE_DAYS,
        SYNCD_LTHASH_CONSISTENCY_CHECK_ON_SNAPSHOT_MAC_MISMATCH,
        SYNCD_MUTATION_AND_BUNDLE_LOGGING,
        SYNCD_PATCH_PROTOBUF_MAX_SIZE,
        SYNCD_PERIODIC_SYNC_DAYS,
        SYNCD_SENTINEL_TIMEOUT_SECONDS,
        SYNCD_USE_INDEX_FOR_LTHASH_LOOKUP,
        SYNCD_WAIT_FOR_KEY_TIMEOUT_DAYS,
        SYNCED_MESSAGE_KEYS_PROCESSING_TYPE,
        SYSTEM_MSG_NUMBERS_FB_BRANDED,
        SYSTEM_MSG_NUMBERS_FB_INC,
        SYSTEM_MSG_TEXT_STYLING,
        TAPPABLE_LINKS_IN_POLL_OPTION_ENABLED,
        TCTOKEN_DURATION,
        TCTOKEN_DURATION_SENDER,
        TCTOKEN_NUM_BUCKETS,
        TCTOKEN_NUM_BUCKETS_SENDER,
        TEXT_STATUS_TTL_SECONDS_ALLOWLIST,
        TEXT_USER_JOURNEY_LOGGING_WAM_ENABLED,
        TIMEOUT_MEX_CALL_EXPAND_FMX_TRUST_SIGNALS,
        TOP_LEVEL_MESSAGE_SECRET_CHECK,
        TOS_3_CLIENT_GATING_ENABLED,
        TOS_CLIENT_STATE_FETCH_ENABLED,
        TOS_CLIENT_STATE_FETCH_ITERATION,
        TRANSCODE_AND_REPAIR_VIDEOS,
        TS_SESSION_DURATION_MS,
        TS_SURFACE_KILLSWITCH,
        UGC_ENABLED,
        UGC_PARTICIPANT_LIMIT,
        UNIFIED_CALLING_ENTRY_POINT_DESKTOP_TYPE,
        UNIFIED_OTP_COPY_CODE_URL,
        UNIFIED_OTP_RETRIEVER_URL,
        UNIFIED_PIN_ADDON_TABLE_ENABLED,
        UNIFIED_POLL_VOTE_ADDON_INFRA_ENABLED,
        UNIFIED_RESPONSE_AI_CONTENT_SEARCH_ENABLED,
        UNIFIED_RESPONSE_AI_SPORTS_WIDGET_ENABLED,
        UNIFIED_RESPONSE_MARKDOWN_LINKS_ENABLED,
        UNIFIED_SESSION_LOG_CALL_EVENT,
        UNIFY_END_CALL_EVENTS,
        UPDATED_HARMFUL_DOCUMENT_DIALOG,
        UPDATES_PRIVACY_NOTICE_ROLLOUT_DATE,
        UPDATES_QUICK_PROMOTION_BANNER_ENABLED,
        UPLOAD_DOCUMENT_THUMB_MMS_ENABLED,
        USE_CACHED_APP_SETTINGS_FROM_GLOBAL_CTX,
        USE_CUSTOM_SOCCER_BALL_FOR_REACTION_ENABLED,
        USE_PER_CHAT_WALLPAPER,
        USE_SIGNED_SHIMMED_URL_LINK,
        USERNAME_1ON1_SYS_MSG_CREATION_UPSELL_ENABLED,
        USERNAME_ACTIVATION_QP,
        USERNAME_ADOPTION_AND_ENGAGEMENT_MONITORING_ENABLED,
        USERNAME_ANTISCRAPING_SEND_CACHED_UN,
        USERNAME_API_RATE_LIMIT_ENABLED,
        USERNAME_CHANNELS_PN_PRIVACY_ENABLED,
        USERNAME_CHECK_DEBOUNCE_IN_MS,
        USERNAME_CONTACT_CARD_DEDUPE_ICONS,
        USERNAME_CONTACT_DISPLAY,
        USERNAME_CONTACT_PRIVACY_SETTING_ALLOW_UNCONTACT_SET_ENABLE,
        USERNAME_CONTACT_SYNCD_SUPPORT_ENABLE,
        USERNAME_CONTACT_UI_VCARD,
        USERNAME_CONTACT_USYNC_LID_BASED,
        USERNAME_CREATION,
        USERNAME_CREATION_RESERVATION_PP_DISCLOSURE_ENABLED,
        USERNAME_ENABLED_ON_COMPANION,
        USERNAME_ENGAGEMENT_NETWORK_IMPACT_LOGGING,
        USERNAME_EXPOSED_LOGGING_ENABLED,
        USERNAME_GLOBAL_SEARCH_ENABLED,
        USERNAME_GROUP_MUTATION_ENABLED,
        USERNAME_KEY_REDESIGN_ENABLED,
        USERNAME_KEY_UPSELL_MAX_CHARACTERS,
        USERNAME_KEY_UPSELL_MAX_NUMBERS,
        USERNAME_KEY_UPSELL_MODE,
        USERNAME_LID_MIGRATION_CALLING,
        USERNAME_MAX_LENGTH,
        USERNAME_MEX_ACCOUNT_SYNC_ENABLED,
        USERNAME_MIN_LENGTH,
        USERNAME_NUMERIC_CODE_V4,
        USERNAME_SEARCH,
        USERNAME_SECURITY_CODE_GENERATION,
        USERNAME_SUGGESTIONS_ENABLED,
        UTILITY_ORDER_STATUS_LOGGING_ENABLED,
        UTILITY_ORDER_VIEW_MBS_ENABLED,
        UTILITY_PAYMENT_REMINDER_M1_ENABLED,
        UTM_TRACKING_ENABLED,
        UTM_TRACKING_EXPIRATION_HOURS,
        UWP_VOIP_INCOMING_CALL_NOTIFICATION_VERSION,
        VERIFIED_BADGE_IN_CHATS_LIST_ENABLED,
        VID_PORT_ENABLE_CAPTURE_FPS_MEDIAN_FILTER,
        VID_PORT_FRM_BUF_MUTEX_FIXES,
        VID_STREAM_PAUSE_RESUME_JB_RESET_THRESHOLD_MS,
        VIDEO_STREAM_BUFFERING_UI_ENABLED,
        VIEW_REPLIES_ENTRY_POINT,
        VIEW_REPLIES_INFRA_ENABLED,
        VIEW_REPLIES_IS_COMPOSER_ENABLED,
        VIEW_REPLIES_WITH_THREADID_ENABLED,
        VISIBLE_MESSAGE_DROP_PLACEHOLDER_ENABLED_INTERNAL_ONLY,
        VOICE_AI_CONVERSATION_STARTER_LATENCY_TRACKING,
        VOICE_CHAT_COMPANION_EXPERIENCE_VERSION,
        VOICEMAIL_NUDGE_DURATION_MS,
        VOIP_CALL_COORDINATOR_VERSION,
        VOIP_ENABLE_WEBRTC_STATS_POLLING,
        VOIP_STACK_INCOMING_MESSAGE_OWNERSHIP_TRANSFER,
        WA_ASTERIA_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED,
        WA_ASTERIA_ENABLED,
        WA_ASTERIA_META_AI_SETTINGS_TAB_ENTRYPOINT_ENABLED,
        WA_ASTERIA_ROLLOUT_ENABLED,
        WA_AUTH_AGENT_OFFBOARDING_ENABLED,
        WA_CAPPING_LOCAL_DATA_LOGIC_UPDATE,
        WA_CATALOG_GRAPHQL_USE_LID_ENABLED,
        WA_CONSUMER_ENTRY_POINT_ENABLED,
        WA_CONSUMER_NOVA_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED,
        WA_CONSUMER_NOVA_ENTRY_POINT_SETTINGS_ENABLED,
        WA_CONSUMER_NOVA_SETTINGS_GREEN_DOT_ENABLED,
        WA_CONSUMER_NOVA_SUBSCRIPTION_NOTIFICATIONS_ENABLED,
        WA_CTWA_LOG_USER_JOURNEY_ENABLED,
        WA_CTWA_WEB_ENABLE_CONTINUOUS_DURATION,
        WA_CTWA_WEB_ENTRYPOINT_HOME_HEADER_DROPDOWN_ENABLED,
        WA_CTWA_WEB_ENTRYPOINT_HOME_HEADER_ENABLED,
        WA_CTWA_WEB_ENTRYPOINT_MANAGE_ADS_HOME_HEADER_DROPDOWN_ENABLED,
        WA_CTWA_WEB_FETCH_LINKED_ACCOUNTS_ENABLED,
        WA_CTWA_WEB_HIDE_AD_CONTEXT_IF_SOFT_DISMISSED_IN_PRIMARY,
        WA_CTWA_WEB_THREAD_AD_ATTRIBUTION_ENABLED,
        WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_ENABLED,
        WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_FETCH_TTL_SECONDS,
        WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_LIMIT,
        WA_INDIVIDUAL_NEW_CHAT_MSG_CAPPING_MV_GET_SUBSCRIPTION_V2,
        WA_INDIVIDUAL_NEW_CHAT_MSG_FCI_STALENESS_TTL_IN_SECONDS,
        WA_INDIVIDUAL_NEW_CHAT_MSG_LATEST_RAMPUP_DATE,
        WA_INDIVIDUAL_NEW_CHAT_THREAD_CAPPING_LIMIT,
        WA_MEDIA_IMAGE_UPLOAD_CACHE,
        WA_META_ONE_ELIGIBILITY_SUBSCRIPTION_STATUS_CHECK_ENABLED,
        WA_META_ONE_ENABLED,
        WA_META_ONE_LAUNCH_FREE_TRIAL_ENABLED,
        WA_META_ONE_ROLLOUT_ENABLED,
        WA_META_ONE_SUBSCRIPTION_NOTIFICATIONS_ENABLED,
        WA_NCT_TOKEN_HISTORY_SYNC_ENABLED,
        WA_NCT_TOKEN_SALT_CREATION_ENABLED,
        WA_NCT_TOKEN_SEND_ENABLED,
        WA_NCT_TOKEN_SYNCD_ENABLED,
        WA_OHAI_NEW_VIP_HEADER_ENABLED,
        WA_PAYMENTS_SMB_ENABLED,
        WA_PAYMENTS_SMB_LABELS_CONVENTION_ENABLED,
        WA_QP_EXPOSURE_LOG_VIA_GRAPHQL_ENABLED,
        WA_SMB_BIZ_PROFILE_GOOGLE_INTEGRATION_ENABLED,
        WA_SMB_FORWARD_BB_WEB_ENABLED,
        WA_SMB_WEB_LISTS_QUICK_REPLIES_ENABLED,
        WA_WEB_ADAPTIVE_LAYOUT_ENABLED,
        WA_WEB_AGM_SIGNUP_ENABLED,
        WA_WEB_ANR_PUSHNAME_CHECK_ENABLED,
        WA_WEB_ANYONE_CAN_LINK_M2,
        WA_WEB_ANYONE_CAN_LINK_M2_FLOOD_LIMIT,
        WA_WEB_APP_LOCK_UPSELL,
        WA_WEB_ATTACH_ICON_VARIANT,
        WA_WEB_BASE_VIDEO_COMET_VIDEO_PLAYER_ENABLED,
        WA_WEB_BIZ_BROADCAST_COLLECTION_BASED_CAMPAIGNS_ENABLED,
        WA_WEB_BIZ_BROADCASTS_CATALOG_ATTACHMENT,
        WA_WEB_BIZ_BROADCASTS_CONTEXTUAL_ENTRYPOINTS,
        WA_WEB_BIZ_PROFILE_GOOGLE_INTEGRATION_ENABLED,
        WA_WEB_BIZ_PROFILE_GRAPHQL_MIGRATION,
        WA_WEB_BIZ_PROFILE_GRAPHQL_MIGRATION_BYPASS_LID_CHECK_DOGFOODING,
        WA_WEB_BIZ_PROFILE_PRELOAD,
        WA_WEB_BLOCKED_PARTICIPANT_CALL_WARNING,
        WA_WEB_BLOCKED_PARTICIPANT_CHAT_WARNING,
        WA_WEB_BOT_ORPHAN_LOGIC_ENABLED,
        WA_WEB_BOT_TOS_CHECK_REFINIEMENT,
        WA_WEB_BROADCAST_DISAPPEARING_MESSAGES_FIX,
        WA_WEB_CALLING_DEEP_LINK_ERROR,
        WA_WEB_CANONICAL_REG_RELOAD_ENABLED,
        WA_WEB_CANONICAL_WAM_FALCO_BUFFER_ENABLED,
        WA_WEB_CANONICAL_WAM_FALCO_BUFFER_SIZE,
        WA_WEB_CHANGE_LIST_WDS_SUBMENU,
        WA_WEB_CHANNELS_COMET_VIDEO_PLAYER_ENABLED_V2,
        WA_WEB_CHAT_OPEN_OPTIMIZATIONS,
        WA_WEB_CHAT_SEARCH_ENTRYPOINT,
        WA_WEB_CHAT_THEMES,
        WA_WEB_CHAT_THEMES_LOGGING,
        WA_WEB_CHAT_THEMES_SOLID_WALLPAPER_SYNC_ENCODE,
        WA_WEB_CHATLIST_RENDER_CHAT_OPEN,
        WA_WEB_CLEAR_SELECTED_CHATS_ENABLED,
        WA_WEB_COMET_VIDEO_PLAYER_SNAPL,
        WA_WEB_COMPOSER_HEIGHT_INCREASE_ENABLED,
        WA_WEB_CONSOLE_LOG_LEVEL,
        WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_DISTANCE_THRESHOLD,
        WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_ENABLED,
        WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_SIMILARITY_OPTIMIZATION_ENABLED,
        WA_WEB_CONTACT_AND_CHAT_FUZZY_SEARCH_TIMEOUT_THRESHOLD,
        WA_WEB_CONTACT_SEARCH_TOKENIZED_ENABLED,
        WA_WEB_CONTEXT_CARD_VERTICAL_BUTTONS,
        WA_WEB_COPY_LINK_URL_ENABLED,
        WA_WEB_CREATE_GROUP_IN_FILTER,
        WA_WEB_DEBUG_COLOR_CODE_RETRY_MESSAGES,
        WA_WEB_DEFAULT_PROFILE_PICS,
        WA_WEB_DISABLE_PREFETCH_LOADABLES,
        WA_WEB_DISCUSS_PRIVATELY,
        WA_WEB_DOWNLOAD_MIMETYPE_CHECK_BLOCK_ENABLED,
        WA_WEB_EDIT_BEFORE_FORWARDING_TO_STATUS,
        WA_WEB_ENABLE_CHAT_THREAD_AND_INFO_STATUS_RING,
        WA_WEB_ENABLE_FOLLOW_UP_REPLY_ICON,
        WA_WEB_ENABLE_GRANULAR_NOTIFICATIONS,
        WA_WEB_ENABLE_MENTION_MESSAGE,
        WA_WEB_ENABLE_STATUS_HQ_THUMBNAIL,
        WA_WEB_ENABLE_SYNCD_KEY_PERSISTENCE_ONLY_AFTER_SERVER_ACK,
        WA_WEB_EXPANSION_COUNTRIES_BONSAI_ENABLED,
        WA_WEB_EXPORT_CHAT,
        WA_WEB_FALCO_CLEAR_LOCAL_STORAGE_QUEUE_ENABLED,
        WA_WEB_FALCO_CONSOLE_LOGGER,
        WA_WEB_FAVICON_BADGING_ENABLED,
        WA_WEB_FAVICONS_UPDATE_M1,
        WA_WEB_FEATURE_PARITY_SMALL_WINS,
        WA_WEB_FMX_AGM_ENABLED,
        WA_WEB_FOCUS_MANAGEMENT_FOR_STATUS_AUDIENCE,
        WA_WEB_FORWARD_TO_SMALL_GROUPS,
        WA_WEB_FREQUENT_REACTIONS_REACTS_AGO_THRESHOLD,
        WA_WEB_FREQUENT_REACTIONS_STORE_ENABLED,
        WA_WEB_FREQUENT_REACTIONS_WEIGHT_REDUCER,
        WA_WEB_GLOBAL_SEARCH_PREFIX_BASED,
        WA_WEB_GROUP_DISCARD_DIALOG_CONTACT_THRESHOLD,
        WA_WEB_GROUP_INFO_NOTIFICATION_ROW,
        WA_WEB_GROUPS_IN_COMMON_MULTI_CONTACT,
        WA_WEB_GROWTH_EMPTY_STATE_UPSELL_VARIANT_M1,
        WA_WEB_HIGHLIGHT_ME_MENTION,
        WA_WEB_HIGHLIGHT_ME_MENTION_GROUPSIZE_THRESHOLD,
        WA_WEB_HISTORY_SYNC_DYNAMIC_THROTTLING,
        WA_WEB_HORIZONTAL_LINK_PREVIEWS,
        WA_WEB_HQ_IMAGE_THUMBNAIL_IN_CHAT_SCANS,
        WA_WEB_HYBRID_CONTEXT_MENU_REACTIONS_ENABLED,
        WA_WEB_HYBRID_SIMPLE_CHAT_CONVERSATION_CONTEXT_MENU_ENABLED,
        WA_WEB_IMAGINE_UR_ENABLED,
        WA_WEB_IMPORTANT_MSG_NOTIFICATION,
        WA_WEB_INVITE_LINK_PAGE_ENHANCEMENTS,
        WA_WEB_JUMP_TO_CART,
        WA_WEB_LARGE_GROUP_PRESENCE_ENABLED,
        WA_WEB_LISTS_FULL_WIDTH_FILTERS,
        WA_WEB_LISTS_M1_ENABLED,
        WA_WEB_LISTS_M2_ENABLED,
        WA_WEB_LOADER_BUTTON_UIX_IMPROVEMENT,
        WA_WEB_MATCH_PRIMARY_ICONS,
        WA_WEB_ME_TAB,
        WA_WEB_MEDIA_UPLOAD_RETRY_RETRIES_COUNT,
        WA_WEB_MENTION_SEARCH,
        WA_WEB_MULTI_PPL_TYPING_INDICATOR_FOR_CHATLIST_GROUPS_VARIANT,
        WA_WEB_NOTIFICATIONS_MODAL,
        WA_WEB_NOTIFICATIONS_MODAL_VARIANTS,
        WA_WEB_NOTIFY_FOR,
        WA_WEB_PATHFINDER_UNSAMPLING_CONFIG,
        WA_WEB_PRE_CHAT_DEVICE_ID_TEST,
        WA_WEB_PRELOAD_CONVERSATION_CHAT_OPEN,
        WA_WEB_PTT_LOADER_BUTTON_UIX_IMPROVEMENT,
        WA_WEB_PUSH_NAME_IN_GLOBAL_SEARCH_NON_CONTACTS_ENABLED,
        WA_WEB_QUICK_REACTIONS,
        WA_WEB_REACTIONS_2,
        WA_WEB_REACTIONS_MOTION_V2_ENABLED,
        WA_WEB_RECONNECT_ANR,
        WA_WEB_REDUCE_CASCADING_UPDATES_CHAT_OPEN,
        WA_WEB_REDUCE_FORCED_LAYOUT_CHAT_OPEN,
        WA_WEB_RESHARE_POSTER_SIDE_ENABLED,
        WA_WEB_RICH_RESPONSE_REPLYING_ENABLED,
        WA_WEB_SCROLLABLE_REACTION_TRAY_ENABLED,
        WA_WEB_SEARCH_EMOJI_PICKER,
        WA_WEB_SEARCH_EMPTY_STATE_M1,
        WA_WEB_SELECT_ALL_CHATS_ENABLED,
        WA_WEB_SELF_PROFILE_PHOTO_FIX_ENABLED,
        WA_WEB_SHARE_CONTENT_UJ,
        WA_WEB_SHOW_HD_PHOTO,
        WA_WEB_SHOW_STATUS_RING_FOR_NO_UNREAD,
        WA_WEB_SMALL_GROUP_PRESENCE_ENABLED,
        WA_WEB_STARRED_MSGS_SEARCH,
        WA_WEB_STATUS_COMET_VIDEO_PLAYER_ENABLED,
        WA_WEB_STATUS_FIRST_UPLOAD_FIX_ENABLED,
        WA_WEB_STATUS_QUESTION_STICKER_REPLY_ENABLED,
        WA_WEB_STATUS_REACTION_STICKER_REPLY_ENABLED,
        WA_WEB_STATUS_RESHARE_ATTRIBUTION_ENABLED,
        WA_WEB_STATUS_RESHARER_FLOW_ENABLED,
        WA_WEB_STATUS_VIEWER_SIDE_POSTER_IDENTIFIERS_ENABLED,
        WA_WEB_UR_BLOKS_ENABLED,
        WA_WEB_UR_IMAGINE_VIDEO_ENABLED,
        WA_WEB_VELOCITY_ANIMATE_MIGRATION_ENABLED,
        WA_WEB_VIDEO_COMET_VIDEO_PLAYER_ENABLED,
        WA_WEB_VOIP_ADAPTIVE_GRID_PAGE_SIZE,
        WA_WEB_VOIP_STACK_LOG_LEVEL,
        WA_WEB_WAE_QPL_ENABLED,
        WA_WEB_WAM_FALCO_CRITICAL_EVENT_IDS,
        WA_WEB_WAM_FALCO_FLUSH_INTERVAL_MS,
        WA_WEB_WAM_FALCO_LOGGING_ENABLED,
        WA_WEB_WAM_FALCO_MODE,
        WA_WEB_WAM_FALCO_SHADOW_EVENT_IDS,
        WA_WEB_XB_BUBBLE_ENABLED,
        WA_WEBTP_EDIT_PDF_IN_WHATSAPP_ENABLED,
        WA_WEBTP_PDF_RENDERER_MODE_NO_EXPOSURE,
        WA_WEBTP_PDF_SHARER_CONSENT_COPY_V2,
        WA_WEBTP_PRELOAD_THUMBNAIL_RENDERER_NO_EXPOSURE,
        WA_WEBTP_THUMBNAIL_RENDERER_MODE,
        WA_WEBTP_THUMBNAIL_RENDERER_TIMEOUT_MS,
        WA_WEBTP_USE_ASYNC_PDF_SEND,
        WA_WEBTP_USE_PDF_ANNOTATIONS,
        WA_WEBTP_USE_PDF_EDITOR,
        WA_WEBTP_USE_PDF_RENDERER,
        WA_WEBTP_USE_THUMBNAIL_RENDERER,
        WA_WIN_PDF_RENDERING_ENABLED,
        WABAI_CONSENT_COOLDOWN,
        WABAI_CONSENT_REQUIRED,
        WABAI_MESSAGE_FEEDBACK_ENABLED,
        WABAI_MESSAGE_RENDERING_ENABLED,
        WABBA_RECEIVER_ENABLED,
        WABBA_SAVE_TO_CAMERA_ROLL_ENABLED,
        WAE_METADATA_INTEGRITY_TIMEOUT_MINUTES,
        WAM_DISABLE_ABKEY_ATTRIBUTE,
        WAM_DISABLE_EXPOKEY_ATTRIBUTE,
        WAMO_AGM_ENABLED,
        WAMO_PRIVACY_TOS_LINKED_HIGHLIGHTED_NOTICE_ID,
        WAMO_PRIVACY_TOS_SHOW_CHANNELS_NUX_ENABLED,
        WAMO_PRIVACY_TOS_UNLINKED_HIGHLIGHTED_NOTICE_ID,
        WAMO_SUB_ADMIN_ENABLED_V2,
        WAMO_SUB_CONSUMER_ENABLED_V2,
        WAMO_SUB_LOGGING_ENABLED_V2,
        WAMO_SUB_MESSAGES_SUPPORTED,
        WAMO_SUB_PROCESS_MESSAGE_KILL_SWITCH,
        WAVOIP_ENABLE_ML_NAMESPACE_V2,
        WAVOIP_LEGACY_ML_QPL_EXP_TAG,
        WAVOIP_ML_BWE_CONG_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_CONG_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_GC_HD_TARGET_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_GC_HD_TARGET_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_GC_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_GC_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_HD_TARGET_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_HD_TARGET_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_PLC_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_PLC_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_QUICKHD_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_RL_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_TR_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_BWE_TR_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_BWE_UNDERSHOOT_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_MEDIA_AUTOMOS_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_MEDIA_NS_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_MEDIA_VMOS_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_MEDIA_VSR_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_NADL_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_NADL_MODEL_DOWNLOAD_VERSIONS_V2,
        WAVOIP_ML_QPL_EXP_TAG,
        WAVOIP_ML_TEMP_MODEL_DOWNLOAD_VERSIONS,
        WAVOIP_ML_TRANSPORT_DOWNLOAD_VERSIONS,
        WAWEB_CHATINFO_REFRESH,
        WAWEB_CROSSPOSTING_ATTRIBUTIONS,
        WAWEB_ENABLE_LEGACY_IMAGE_ZOOM,
        WAWEB_STATUS_CLOSE_FRIENDS_VIEWER_SIDE_ENABLED,
        WDS_RADIUS_AND_CASING,
        WDS_WEB_ACTION_TILE_REFRESH,
        WDS_WEB_BADGE,
        WDS_WEB_CHIP,
        WDS_WEB_COMPOSER_TOOLBAR_V2,
        WDS_WEB_DIALOG,
        WDS_WEB_EXPRESSIONS_PANEL,
        WDS_WEB_MENU_REACTION_DETAIL_PANEL_V2,
        WDS_WEB_PROFILE_PHOTO,
        WDS_WEB_RICH_TEXT_FIELD,
        WDS_WEB_SUBMENUS,
        WDS_WEB_TEXT_LAYOUT,
        WDS_WEB_TOAST,
        WEB_ABPROP_BLOCK_CATALOG_CREATION_ECOMMERCE_COMPLIANCE_INDIA,
        WEB_ABPROP_BUSINESS_PROFILE_REFRESH_LINKED_ACCOUNT_ENABLED,
        WEB_ABPROP_BUSINESS_PROFILE_REFRESH_LINKED_ACCOUNTS_KILLSWITCH,
        WEB_ABPROP_COLLECTIONS_NUX_BANNER,
        WEB_ABPROP_CORE_WAM_RUNTIME,
        WEB_ABPROP_DIRECT_CONNECTION_MD,
        WEB_ABPROP_DROP_FULL_HISTORY_SYNC,
        WEB_ABPROP_MEDIA_LINKS_DOCS_SEARCH,
        WEB_ABPROP_SCREEN_LOCK_ENABLED,
        WEB_ADD_CONTACT,
        WEB_ADV_LOGOUT_ON_SELF_DEVICE_LIST_EXPIRED,
        WEB_AI_GROUP_OPEN_SUPPORT,
        WEB_ANR_ASYNC_CONTACTS_RESTORE_FROM_DB_ENABLED,
        WEB_ANR_ASYNC_MEDIA_DECRYPTION_ENABLED,
        WEB_ANR_ASYNC_MSG_SEND_HANDLER,
        WEB_ANR_ASYNC_NATIVE_APP_STATE_BRIDGE_ENABLED,
        WEB_ANR_ASYNC_SQLITE_BRIDGE_OPERATIONS,
        WEB_ANR_BATCH_AND_QUEUE_BULK_CONTACTS_DB_WRITES_ENABLED,
        WEB_ANR_BATCH_PROFILE_PICTURE_BRIDGE_OPERATIONS,
        WEB_ANR_DISABLE_MEMORY_LOGGING,
        WEB_ANR_FILE_SIZE_THRESHOLD_TO_USE_WORKER_MB,
        WEB_ANR_GROUP_METADATA_YIELD,
        WEB_ANR_MEDIA_CHUNK_ENC_DELAY_ENABLED,
        WEB_ANR_NOOP_GC_ENABLED,
        WEB_ANR_OPTIMIZED_INITIAL_CONTACTS_SYNC_ENABLED,
        WEB_ANR_PRUNE_CMC,
        WEB_ANR_SKIP_UNUSED_CONTACTS_DB_UPDATES_ENABLED,
        WEB_ANR_SPINNER_GPU_ANIMATION,
        WEB_ANR_THROTTLE_SIGNAL_SNAPSHOT_ENABLED,
        WEB_ATTACH_MENU_ADD_DRAWING_ENABLED,
        WEB_AUTODOWNLOAD_STICKERS,
        WEB_BACKGROUND_SYNC_V2,
        WEB_BB_GENAI_COMPOSER_MIN_WORDS,
        WEB_BIZ_PROFILE_OPTIONS,
        WEB_BIZ_QUALITY_TELEMETRY_ENABLED,
        WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_CLICKS_ENABLED,
        WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_LEVEL_ACTIONS_ENABLED,
        WEB_BIZ_QUALITY_TELEMETRY_MESSAGE_READS_ENABLED,
        WEB_BIZ_SIMPLE_SIGNAL_ENABLED,
        WEB_BIZ_SIMPLE_SIGNAL_GROUP_ENABLED,
        WEB_BOT_PROFILE_GQL_MIGRATION_ENABLED,
        WEB_BOT_PROFILE_PIC_GQL_MIGRATION_ENABLED,
        WEB_BROWSER_MIN_STORAGE_QUOTA,
        WEB_BROWSER_QUOTA_THRESHOLD,
        WEB_BUG_REPORTING_REQUEST_PEER_LOG_ENABLED,
        WEB_BULK_ADD_CONTACTS_ENABLED,
        WEB_BUSINESS_BROADCAST_GENAI_CUSTOM_USER_PROMPT_ENABLED,
        WEB_BUSINESS_BROADCAST_GENAI_TEXT,
        WEB_BUSINESS_BROADCAST_GENAI_TEXT_LANGUAGES,
        WEB_BUSINESS_BROADCAST_GENAI_TEXT_MAX_TRIES,
        WEB_BUSINESS_BROADCAST_GENAI_TEXT_MODEL,
        WEB_BUSINESS_BROADCAST_GENAI_TEXT_NO_EXP,
        WEB_BUSINESS_TOOLS_DRAWER_ENABLED,
        WEB_CACHE_OPEN_FAILED_RELOAD_FLOW_ENABLED,
        WEB_CALENDAR_MESSAGE_DENSITY_ENABLED,
        WEB_CALLING_AUTO_POPOUT_VIDEO,
        WEB_CALLING_ENABLE_ON_WINDOWS,
        WEB_CALLING_FULL_SCREEN_TOGGLE_ENABLED,
        WEB_CALLING_OFFLINE_RESUME_ORDERING,
        WEB_CALLING_PERF_OPTIMIZATIONS_BITMASK,
        WEB_CALLING_SPEAKER_STRIP_RESIZE_ENABLED,
        WEB_CALLS_TAB_EMPTY_STATE_BUTTONS,
        WEB_CATALOG_RECOVERY_FLOW_ENABLED,
        WEB_CATALOG_VIEWING_VARIANTS_ENABLED,
        WEB_CHANNEL_STATUS_LIKES_SENDING_ENABLED,
        WEB_CHANNEL_VIDEO_SERVER_TRANSCODE_UPLOAD,
        WEB_CHAT_INFO_ACTION_BUTTONS_REFRESH,
        WEB_CHAT_THEME_DRAWER_TITLE,
        WEB_CHATPSA_FORWARDING,
        WEB_CHATS_CONTENT_VISIBILITY,
        WEB_COEX_SIMPLE_SIGNAL_ENABLED,
        WEB_COMMS_SOCKET_RECONNECT_ENABLED,
        WEB_COMMUNITIES_GENERAL_CHAT_V_2,
        WEB_CONFIGURABLE_QUICK_ACTIONS_M1,
        WEB_CONFIGURABLE_QUICK_ACTIONS_M1_CHANNELS,
        WEB_CONFIGURABLE_QUICK_ACTIONS_M1_COMMUNITIES,
        WEB_CONTACT_COLLECTION_LOCALE_LISTENER,
        WEB_CONTACT_SORT_LETTERS_FIRST,
        WEB_CONVERSATION_CLEANUP_TEMP_COLLECTION,
        WEB_CROSSPOST_SETTINGS_SYNC,
        WEB_DATE_MARKER_CALENDAR_ENABLED,
        WEB_DEPRECATE_MMS4_HASH_BASED_DOWNLOAD,
        WEB_DESIGN_REFRESH,
        WEB_DEXIE_HOOKS_SUPPORT_ENABLED,
        WEB_DISABLE_COMPOSE_BOX_FOR_DEPRECATED_CHATS,
        WEB_DISABLE_LOGS_LOW_END_DEVICE,
        WEB_DISABLE_SW_ON_SAFARI_PWA,
        WEB_DISPLAY_LID_CONTACTS,
        WEB_DRAWER_DESCRIPTOR_ENABLED,
        WEB_E2E_BACKFILL_EXPIRE_TIME,
        WEB_ENABLE_BIZ_CATALOG_VIEW_PS_LOGGING,
        WEB_ENABLE_CAMERA_CAPTURE_REFRESH,
        WEB_ENABLE_IMPROVED_BULK_MERGE,
        WEB_ENABLE_PROFILE_PIC_THUMB_DB_CACHING,
        WEB_EVICT_THUMBNAIL_HQ_ON_INACTIVE,
        WEB_EVOLVE_ABOUT_SEND_ENABLED,
        WEB_FIX_DUPLICATED_LIDS_HISTORY_SYNC,
        WEB_FORCE_LID_CHATS_IN_HISTORY,
        WEB_FREQUENTLY_CONTACTED_ENABLED,
        WEB_GET_MSG_EXIST_OPTMISE,
        WEB_GETTERS_LRU_CACHE_SIZE_LIMIT,
        WEB_GROUP_BULK_ADD_CONTACT,
        WEB_GROUP_EXPERIMENTATION_ENABLE,
        WEB_GROUP_HOVER_CARD_VARIANT,
        WEB_GROUP_PROFILE_EDITOR,
        WEB_GUEST_CALLING_REPRESENTATION_ENABLED,
        WEB_HISTORY_SYNC_ALLOW_DUPLICATE_IN_BULK_ERROR,
        WEB_HISTORY_SYNC_WORKER_ENABLED,
        WEB_HYBRID_APPLY_LATEST_DB_SCHEMA_OPTIMIZATION_ENABLED,
        WEB_IMAGE_MAX_EDGE,
        WEB_IMAGE_MAX_HD_EDGE,
        WEB_INIT_CHAT_BATCH_SIZE,
        WEB_INIT_CHAT_MAX_UNREAD_MESSAGE_COUNT,
        WEB_INTERN_DOGFOODING_UPSELL_CONTENT,
        WEB_INTERN_DOGFOODING_UPSELL_ENABLED,
        WEB_INTERN_DOGFOODING_UPSELL_SNOOZE_DURATION,
        WEB_INTERNAL_IN_APP_BUG_REPORTING_ENABLE,
        WEB_IP_TOKEN_ENABLED,
        WEB_JPEG_QUALITY,
        WEB_LARGER_LINK_PREVIEWS,
        WEB_LINK_PREVIEW_SYNC_ENABLED,
        WEB_LOG_CAPACITY_OVERRIDE,
        WEB_LOGOUT_UNMIGRATED_COMPANION,
        WEB_LOW_END_DEVICE_LEVEL,
        WEB_MAC_BETA_UPSELL,
        WEB_MATERIAL_REFRESH,
        WEB_MAX_CONTACTS_TO_SHOW_COMMON_GROUPS,
        WEB_MAX_FOUND_COMMON_GROUPS_DISPLAYED,
        WEB_MEDIA_COMPUTE_IN_WORKER_ENABLED,
        WEB_MEDIA_ENCRYPT_UPLOAD_IN_WORKER_ENABLED,
        WEB_MEDIA_WORKER_SPLIT_ENABLED,
        WEB_MEMORY_REDUCTION,
        WEB_MENU_SHARE_GROUP,
        WEB_MESSAGE_CUSTOM_ARIA_LABEL,
        WEB_MESSAGE_LIST_A11Y_REDESIGN,
        WEB_MESSAGE_PLUGIN_FRONTEND_REGISTRATION_ENABLED,
        WEB_MESSAGE_PROCESSING_CACHE_SIZE,
        WEB_MESSAGES_CONTENT_VISIBILITY,
        WEB_MOVE_MESSAGE_SECRET_TOP_LEVEL_ENABLED,
        WEB_MSG_INFRA_REMOVE_DEVICES_ON_406_ERROR_ENABLED,
        WEB_MULTI_SKIN_TONED_EMOJI_PICKER,
        WEB_NATIVE_FETCH_MEDIA_DOWNLOAD,
        WEB_NAVIGATION_BAR_UPDATES_TAB,
        WEB_NEW_CHAT_FLOW_REFRESH_VARIANT,
        WEB_NEW_EVENT_EMITTER,
        WEB_NEW_WDS_ICONS,
        WEB_NON_BLOCKING_OFFLINE_RESUME_MAX_MESSAGE_COUNT,
        WEB_NONCRITICAL_HISTORY_SYNC_MESSAGE_PROCESSING_BREAK_ITERATION,
        WEB_NOTIFICATIONS_BANNER_NEW_LOGIC_ENABLED,
        WEB_NOTIFICATIONS_BANNER_VARIANT,
        WEB_OFFLINE_DYNAMIC_BATCH_CONFIG,
        WEB_OFFLINE_DYNAMIC_BATCH_SIZE_ENABLED,
        WEB_OFFLINE_MESSAGE_PROCESSOR_TIMEOUT_SECONDS,
        WEB_OFFLINE_RESUME_QPL_ENABLED,
        WEB_OFFLINE_RESUME_WAIT_FOR_PING_TIMEOUT_SECONDS,
        WEB_OPTIMIZED_AVATARS,
        WEB_OPTIMIZED_COMPOSITING_LAYERS,
        WEB_OPTIMIZED_EVENT_HANDLERS,
        WEB_OPTIMIZED_MESSAGE_TAILS,
        WEB_OPTIMIZED_PILLS,
        WEB_ORIGINAL_PHOTO_QUALITY_UPLOAD_ENABLED,
        WEB_OTP_COPY_CODE_DISABLED,
        WEB_PATHFINDER_LOGGING,
        WEB_PDF_THUMBNAIL_SIZE_IN_BYTES,
        WEB_PENDING_MESSAGE_CACHE_ENABLED,
        WEB_PHONE_NUMBER_GLOBAL_SEARCH,
        WEB_PNLESS_STANZAS,
        WEB_PRELOAD_CHAT_MESSAGES,
        WEB_PREMIUM_MESSAGES_INTERACTIVITY_RENDERING_ENABLED,
        WEB_PTT_RENDER_THROTTLING,
        WEB_PTT_STREAMER_UPLOAD,
        WEB_PTT_TRANSCRIPTION_BUTTON_ENABLED,
        WEB_PTT_TRANSCRIPTION_ENABLED,
        WEB_PTT_TRANSCRIPTION_MAX_DURATION_SECONDS,
        WEB_PWA_BACKGROUND_SYNC,
        WEB_PWA_BACKGROUND_SYNC_MIN_INTERVAL_HOURS,
        WEB_QP_BB_RE_ENGAGEMENT_PAST_29_DAYS,
        WEB_QP_SMB_BB_PMF_TEST_HIGH_ENGAGEMENT_USER,
        WEB_QP_SMB_BB_RECENT_MESSAGE_SEND,
        WEB_RATING_AND_REVIEW_CONTEXTUAL_PROMPT_ENABLED,
        WEB_RATING_AND_REVIEW_ENABLED,
        WEB_READ_SELF_WATERMARK_PROCESSING,
        WEB_READ_SELF_WATERMARK_RECEIVE_STORE_TS,
        WEB_READ_SELF_WATERMARK_SEND_STORE_TS,
        WEB_RECENT_SYNC_CHUNK_DOWNLOAD_OPTIMIZATION,
        WEB_REMOVE_MESSAGE_SECRET_FROM_QUOTED_ENABLED,
        WEB_REQUEST_MISSING_KEYS_FOR_REMOVES,
        WEB_RESUME_OPTIMIZED_READ_RECEIPT_SEND_INTERVAL,
        WEB_SCREEN_LOCK_MAX_RETRIES,
        WEB_SEARCH_RESULTS_TYPE_DATE_FILTERS,
        WEB_SEND_HID_FAILED_DECRYPT_IN_RECEIPTS_ENABLED,
        WEB_SEND_INVISIBLE_MSG_MAX_GROUP_SIZE,
        WEB_SEND_INVISIBLE_MSG_MIN_GROUP_SIZE,
        WEB_SEND_ORPHAN_IN_RECEIPTS_ENABLED,
        WEB_SHOP_STOREFRONT_MESSAGE,
        WEB_SHOW_TO_HIDE_ENABLED,
        WEB_SIGNAL_FUTURE_MESSAGES_MAX,
        WEB_SOCKET_PARALLEL_CONNECTION_ENABLED,
        WEB_STATUS_CROSSPOSTING_ENABLED,
        WEB_STATUS_LIKES_SEND_V2_ENABLED,
        WEB_STATUS_PSA,
        WEB_STATUS_RANKING,
        WEB_STATUS_RANKING_ENABLED,
        WEB_STICKER_SUGGESTIONS_ENABLE,
        WEB_STICKY_HD_PHOTO_SETTING_ENABLED,
        WEB_STORE_QUOTA_MANAGER_ENABLED,
        WEB_STREAMING_DOCUMENT_ENCRYPT_MIN_BYTES,
        WEB_SYNCD_FATAL_FIELDS_FROM_L1104589_PRV2,
        WEB_SYNCD_MAX_MUTATIONS_TO_PROCESS_DURING_RESUME,
        WEB_TC_TOKEN_DB_READ_ENABLED,
        WEB_TEST_ABPROP_DELETE_ME,
        WEB_THREAD_LOADING_INFRA_ENABLED,
        WEB_THREADS_INFRA_ENABLED,
        WEB_TOP_LEVEL_MESSAGE_SECRET_ENFORCEMENT_ENABLED,
        WEB_UI_REFRESH_M1,
        WEB_USE_KALEIDOSCOPE_MEDIA_CHECK_ENABLED,
        WEB_VIEW_MODE_USAGE_ENABLED,
        WEB_VOIP_ADAPTIVE_SCTP_PREWARM,
        WEB_VOIP_AUDIO_CAPTURE_IMPL,
        WEB_VOIP_AUDIO_PLAYBACK_IMPL,
        WEB_VOIP_AV_SYNC_DEBUG_OVERLAY,
        WEB_VOIP_CAPTURE_VIDEO_ROTATION_TYPE,
        WEB_VOIP_DYNAMIC_THREAD_PREALLOCATE_COUNT,
        WEB_VOIP_LOAD_WASM_VARIANT,
        WEB_VOIP_LOW_RESOURCE_DEVICE,
        WEB_VOIP_SCTP_WORKER_SAFARI_EXP,
        WEB_VOIP_VIDEO_CAPTURE_IMPL,
        WEB_VOIP_VIDEO_LOW_CAP_HEIGHT,
        WEB_VOIP_VIDEO_LOW_CAP_WIDTH,
        WEB_VOIP_VIDEO_MID_CAP_HEIGHT,
        WEB_VOIP_VIDEO_MID_CAP_WIDTH,
        WEB_VOIP_VIDEO_RENDERER,
        WEB_WAFFLE,
        WEB_WAM_MAX_BUFFER_UPLOAD_SIZE_BYTES,
        WEB_WHATS_NEW_AUTO_MODAL,
        WEB_WHATS_NEW_AUTO_MODAL_SHORT_COOLDOWN,
        WEB_WHATS_NEW_BANNER,
        WEB_WHATS_NEW_BANNER_SHORT_COOLDOWN,
        WEB_WHATS_NEW_BANNER_SHORT_COOLDOWN_V2,
        WEB_WHATS_NEW_CAROUSEL,
        WEB_WINDOWS_CALLING_32P_VERSION,
        WEB_WORKER_ADV_PROCESSING_ENABLED,
        WEB_WORKER_PREKEY_PROCESSING_ENABLED,
        WEBC_PAGE_LOAD_EARLY_COMMIT_ENABLED,
        WEBVIEW2_DISABLE_GPU_ACCELERATION,
        WEBVIEW2_DISABLE_GPU_ACCELERATION_MEMORY_THRESHOLD_MB,
        WEBVIEW2_ENABLE_OFFLINE_SUPPORT,
        WHATSAPP_VPV_LOGGING_ENABLED,
        WIN_CALL_LOG_SEND_OUTGOING_SYNCD_MUTATIONS,
        WIN_ENABLE_SS_BUTTON_AUDIO,
        WIN_HYBRID_BT_ENABLED,
        WIN_HYBRID_FORCE_PERSISTENT_STORAGE_PERMISSION,
        WIN_HYBRID_VOIP_ANR_OPTIMIZATIONS,
        WIN_NETWORK_STATE_WATCHDOG_INTERVAL,
        WINDOWS_CONTACTS_INITIAL_SYNC_DELAY,
        WINDOWS_CONTACTS_SYNC_INTERVAL,
        WINDOWS_GRACEFUL_DEGRADATION_VERSION,
        WINDOWS_SS_CAPTURE_DRIVER_TYPE,
        WINRT_RENDERER,
        WMI_ASYNC_AWAIT_PREP,
        WMI_JM_TO_TS_M1,
        WMI_TASK_SCHEDULER_SECOND_STEP,
        WMI_WORKER_SCHEDULER_WEB,
        XPLAT_ATTACHMENT_FORMAT_CHECK_V2,
        YOUTUBE_INLINE_PLAYBACK_KILLSWITCH,
    ];
}

/// `WAWebGroupABPropsConfigs` — 14 flags.
pub mod group {
    use super::{AbDefault, AbProp, AbPropType};

    pub const AI_GROUP_TEE_HISTORY_SHARE_GROUP_LEVEL_ENABLED: AbProp = AbProp {
        name: "ai_group_tee_history_share_group_level_enabled",
        code: 32501,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_messages_time_limit_secs_group_level",
        code: 26270,
        value_type: AbPropType::Int,
        default: AbDefault::Int(1209600),
    };
    pub const GROUP_HISTORY_OUT_OF_WINDOW_PIN_SENDER_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_out_of_window_pin_sender_group_level",
        code: 26269,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SEND_AFTER_JOIN_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_send_after_join_group_level",
        code: 30905,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SEND_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_send_group_level",
        code: 23245,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTING_DECOUPLE_ENABLED_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_setting_decouple_enabled_group_level",
        code: 30906,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const GROUP_HISTORY_SETTINGS_TOGGLE_UI_GROUP_LEVEL: AbProp = AbProp {
        name: "group_history_settings_toggle_ui_group_level",
        code: 23246,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_ADD_OPTION_ENABLED_GROUP_LEVEL: AbProp = AbProp {
        name: "poll_add_option_enabled_group_level",
        code: 28357,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_CREATOR_EDIT_ENABLED_GROUP_LEVEL: AbProp = AbProp {
        name: "poll_creator_edit_enabled_group_level",
        code: 28358,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_END_TIME_ENABLED_GROUP_LEVEL: AbProp = AbProp {
        name: "poll_end_time_enabled_group_level",
        code: 27009,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const POLL_HIDE_VOTERS_ENABLED_GROUP_LEVEL: AbProp = AbProp {
        name: "poll_hide_voters_enabled_group_level",
        code: 27025,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const RT_GHS_SENDER_GROUP_LEVEL_ENABLED: AbProp = AbProp {
        name: "rt_ghs_sender_group_level_enabled",
        code: 30590,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WA_WEB_CHANNELS_COMET_VIDEO_PLAYER_ENABLED: AbProp = AbProp {
        name: "wa_web_channels_comet_video_player_enabled",
        code: 24037,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };
    pub const WEB_TEST_USE_CASE_CLIENT_GROUP: AbProp = AbProp {
        name: "web_test_use_case_client_group",
        code: 25322,
        value_type: AbPropType::Bool,
        default: AbDefault::Bool(false),
    };

    /// All 14 flags in this registry, sorted by name.
    pub const ALL: &[AbProp] = &[
        AI_GROUP_TEE_HISTORY_SHARE_GROUP_LEVEL_ENABLED,
        GROUP_HISTORY_MESSAGES_TIME_LIMIT_SECS_GROUP_LEVEL,
        GROUP_HISTORY_OUT_OF_WINDOW_PIN_SENDER_GROUP_LEVEL,
        GROUP_HISTORY_SEND_AFTER_JOIN_GROUP_LEVEL,
        GROUP_HISTORY_SEND_GROUP_LEVEL,
        GROUP_HISTORY_SETTING_DECOUPLE_ENABLED_GROUP_LEVEL,
        GROUP_HISTORY_SETTINGS_TOGGLE_UI_GROUP_LEVEL,
        POLL_ADD_OPTION_ENABLED_GROUP_LEVEL,
        POLL_CREATOR_EDIT_ENABLED_GROUP_LEVEL,
        POLL_END_TIME_ENABLED_GROUP_LEVEL,
        POLL_HIDE_VOTERS_ENABLED_GROUP_LEVEL,
        RT_GHS_SENDER_GROUP_LEVEL_ENABLED,
        WA_WEB_CHANNELS_COMET_VIDEO_PLAYER_ENABLED,
        WEB_TEST_USE_CASE_CLIENT_GROUP,
    ];
}

/// Every registry's `ALL`, for whole-catalog iteration:
/// `ALL.iter().flat_map(|r| r.iter())`.
pub const ALL: &[&[AbProp]] = &[web::ALL, group::ALL];
