//! Auto-generated AppState (syncd) action schemas (WhatsApp 2.3000.1040878135). DO NOT EDIT.
//!
//! Typed registry of syncd actions: collection, version, scope, value proto type,
//! enum fields, and the mutation-index parts. `const`/`&'static`, no deps.

#![allow(clippy::all)]

/// A syncd collection (mutation bucket / priority).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Collection {
    Regular,
    RegularLow,
    RegularHigh,
    CriticalBlock,
    CriticalUnblockLow,
}

impl Collection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Collection::Regular => "regular",
            Collection::RegularLow => "regular_low",
            Collection::RegularHigh => "regular_high",
            Collection::CriticalBlock => "critical_block",
            Collection::CriticalUnblockLow => "critical_unblock_low",
        }
    }
}

/// The index scope an action applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Account,
    Chat,
    ChatMessageRange,
    ChatOrContact,
    Message,
}

impl Scope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Scope::Account => "account",
            Scope::Chat => "chat",
            Scope::ChatMessageRange => "chatMessageRange",
            Scope::ChatOrContact => "chatOrContact",
            Scope::Message => "message",
        }
    }
}

/// One component of a mutation index key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexPart {
    /// Fixed wire name at position 0.
    Literal { value: &'static str },
    /// A WhatsApp JID (legacy-encoded).
    Jid { name: &'static str },
    /// '0' or '1' bool encoding.
    BoolString { name: &'static str },
    /// Participant slot: a JID, or '0' when fromMe/null.
    JidOrZero { name: &'static str },
    /// Stringified protobuf-enum integer; `proto_enum` is the dotted enum path.
    Enum {
        name: &'static str,
        proto_enum: &'static str,
    },
    /// Opaque identifier (msg/label/agent id, etc.).
    StringPart { name: &'static str },
    /// Unrecognized slot.
    Unknown { name: &'static str },
}

/// A syncd action schema.
#[derive(Debug, Clone, Copy)]
pub struct Schema {
    /// Registry key (e.g. "Agent").
    pub key: &'static str,
    /// On-wire action name (e.g. "deviceAgent").
    pub name: &'static str,
    /// Source WA Web module.
    pub module: &'static str,
    pub collection: Collection,
    pub version: u32,
    pub scope: Scope,
    /// Field on `SyncActionValue` carrying the payload.
    pub value_field: Option<&'static str>,
    /// Dotted protobuf type of the value (in `waproto`).
    pub value_proto_type: Option<&'static str>,
    /// `(field, dotted enum path)` for enum-typed value fields.
    pub value_enum_fields: &'static [(&'static str, &'static str)],
    /// Index position holding the chat JID, if any.
    pub chat_jid_index: Option<i64>,
    pub index_parts: &'static [IndexPart],
}

/// All syncd collections, in dependency order.
pub const COLLECTIONS: &[Collection] = &[
    Collection::Regular,
    Collection::RegularLow,
    Collection::RegularHigh,
    Collection::CriticalBlock,
    Collection::CriticalUnblockLow,
];

/// `AdsCtwaPerCustomerDataSharing` — module `WAWebCtwaPerCustomerDataSharingSync`.
pub const ADS_CTWA_PER_CUSTOMER_DATA_SHARING: Schema = Schema {
    key: "AdsCtwaPerCustomerDataSharing",
    name: "ctwaPerCustomerDataSharing",
    module: "WAWebCtwaPerCustomerDataSharingSync",
    collection: Collection::RegularHigh,
    version: 1,
    scope: Scope::Account,
    value_field: Some("ctwaPerCustomerDataSharingAction"),
    value_proto_type: Some("SyncActionValue.CtwaPerCustomerDataSharingAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "ctwaPerCustomerDataSharing",
        },
        IndexPart::StringPart { name: "accountLid" },
    ],
};

/// `Agent` — module `WAWebAgentSync`.
pub const AGENT: Schema = Schema {
    key: "Agent",
    name: "deviceAgent",
    module: "WAWebAgentSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Account,
    value_field: Some("agentAction"),
    value_proto_type: Some("SyncActionValue.AgentAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "deviceAgent",
        },
        IndexPart::StringPart { name: "agentId" },
    ],
};

/// `AiThreadDelete` — module `WAWebAiThreadDeleteSync`.
pub const AI_THREAD_DELETE: Schema = Schema {
    key: "AiThreadDelete",
    name: "ai_thread_delete",
    module: "WAWebAiThreadDeleteSync",
    collection: Collection::RegularHigh,
    version: 7,
    scope: Scope::Chat,
    value_field: None,
    value_proto_type: None,
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "ai_thread_delete",
        },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `AiThreadPin` — module `WAWebAiThreadPinSync`.
pub const AI_THREAD_PIN: Schema = Schema {
    key: "AiThreadPin",
    name: "thread_pin",
    module: "WAWebAiThreadPinSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Chat,
    value_field: Some("threadPinAction"),
    value_proto_type: Some("SyncActionValue.ThreadPinAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "thread_pin",
        },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `AiThreadRename` — module `WAWebAiThreadRenameSync`.
pub const AI_THREAD_RENAME: Schema = Schema {
    key: "AiThreadRename",
    name: "ai_thread_rename",
    module: "WAWebAiThreadRenameSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Chat,
    value_field: Some("aiThreadRenameAction"),
    value_proto_type: Some("SyncActionValue.AiThreadRenameAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "ai_thread_rename",
        },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `AndroidUnsupportedActions` — module `WAWebAndroidUnsupportedActionsSync`.
pub const ANDROID_UNSUPPORTED_ACTIONS: Schema = Schema {
    key: "AndroidUnsupportedActions",
    name: "android_unsupported_actions",
    module: "WAWebAndroidUnsupportedActionsSync",
    collection: Collection::RegularLow,
    version: 4,
    scope: Scope::Account,
    value_field: Some("androidUnsupportedActions"),
    value_proto_type: Some("SyncActionValue.AndroidUnsupportedActions"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "android_unsupported_actions",
    }],
};

/// `Archive` — module `WAWebArchiveChatSync`.
pub const ARCHIVE: Schema = Schema {
    key: "Archive",
    name: "archive",
    module: "WAWebArchiveChatSync",
    collection: Collection::RegularLow,
    version: 3,
    scope: Scope::ChatMessageRange,
    value_field: Some("archiveChatAction"),
    value_proto_type: Some("SyncActionValue.ArchiveChatAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "archive" },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `AvatarUpdated` — module `WAWebStickersAvatarUpdatedSyncAction`.
pub const AVATAR_UPDATED: Schema = Schema {
    key: "AvatarUpdated",
    name: "avatar_updated_action",
    module: "WAWebStickersAvatarUpdatedSyncAction",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Account,
    value_field: Some("avatarUpdatedAction"),
    value_proto_type: Some("SyncActionValue.AvatarUpdatedAction"),
    value_enum_fields: &[("eventType", "AvatarUpdatedAction.AvatarEventType")],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "avatar_updated_action",
    }],
};

/// `BizAiSettingsNudge` — module `WAWebBizAiSettingsNudgeSync`.
pub const BIZ_AI_SETTINGS_NUDGE: Schema = Schema {
    key: "BizAiSettingsNudge",
    name: "biz_ai_settings_nudge",
    module: "WAWebBizAiSettingsNudgeSync",
    collection: Collection::RegularHigh,
    version: 1,
    scope: Scope::Account,
    value_field: Some("bizAiSettingsNudgeAction"),
    value_proto_type: Some("SyncActionValue.BizAISettingsNudgeAction"),
    value_enum_fields: &[("category", "BizAISettingsNudgeAction.BizAISettingsCategory")],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "biz_ai_settings_nudge",
    }],
};

/// `BotWelcomeRequest` — module `WAWebBotWelcomeRequestSync`.
pub const BOT_WELCOME_REQUEST: Schema = Schema {
    key: "BotWelcomeRequest",
    name: "bot_welcome_request",
    module: "WAWebBotWelcomeRequestSync",
    collection: Collection::RegularLow,
    version: 2,
    scope: Scope::Chat,
    value_field: Some("botWelcomeRequestAction"),
    value_proto_type: Some("SyncActionValue.BotWelcomeRequestAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "bot_welcome_request",
        },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `BusinessBroadcastCampaign` — module `WAWebBroadcastCampaignSync`.
pub const BUSINESS_BROADCAST_CAMPAIGN: Schema = Schema {
    key: "BusinessBroadcastCampaign",
    name: "business_broadcast_campaign",
    module: "WAWebBroadcastCampaignSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("businessBroadcastCampaignAction"),
    value_proto_type: Some("SyncActionValue.BusinessBroadcastCampaignAction"),
    value_enum_fields: &[("status", "BusinessBroadcastCampaignStatus")],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "business_broadcast_campaign",
        },
        IndexPart::StringPart { name: "campaign" },
    ],
};

/// `BusinessBroadcastInsights` — module `WAWebBusinessBroadcastInsightsSync`.
pub const BUSINESS_BROADCAST_INSIGHTS: Schema = Schema {
    key: "BusinessBroadcastInsights",
    name: "business_broadcast_insights_sync",
    module: "WAWebBusinessBroadcastInsightsSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("businessBroadcastInsightsAction"),
    value_proto_type: Some("SyncActionValue.BusinessBroadcastInsightsAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "business_broadcast_insights_sync",
        },
        IndexPart::StringPart { name: "campaignId" },
    ],
};

/// `BusinessBroadcastList` — module `WAWebBroadcastListSync`.
pub const BUSINESS_BROADCAST_LIST: Schema = Schema {
    key: "BusinessBroadcastList",
    name: "business_broadcast_list",
    module: "WAWebBroadcastListSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("businessBroadcastListAction"),
    value_proto_type: Some("SyncActionValue.BusinessBroadcastListAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "business_broadcast_list",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `CallLog` — module `WAWebCallLogSync`.
pub const CALL_LOG: Schema = Schema {
    key: "CallLog",
    name: "call_log",
    module: "WAWebCallLogSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("callLogAction"),
    value_proto_type: Some("SyncActionValue.CallLogAction"),
    value_enum_fields: &[
        ("callLogRecord.callResult", "CallLogRecord.CallResult"),
        ("callLogRecord.callType", "CallLogRecord.CallType"),
        (
            "callLogRecord.participants.callResult",
            "CallLogRecord.CallResult",
        ),
        ("callLogRecord.silenceReason", "CallLogRecord.SilenceReason"),
    ],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal { value: "call_log" }],
};

/// `ChatAssignment` — module `WAWebChatAssignmentSync`.
pub const CHAT_ASSIGNMENT: Schema = Schema {
    key: "ChatAssignment",
    name: "agentChatAssignment",
    module: "WAWebChatAssignmentSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Chat,
    value_field: Some("chatAssignment"),
    value_proto_type: Some("SyncActionValue.ChatAssignmentAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "agentChatAssignment",
        },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `ChatAssignmentOpenedStatus` — module `WAWebChatAssignmentOpenedStatusSync`.
pub const CHAT_ASSIGNMENT_OPENED_STATUS: Schema = Schema {
    key: "ChatAssignmentOpenedStatus",
    name: "agentChatAssignmentOpenedStatus",
    module: "WAWebChatAssignmentOpenedStatusSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Chat,
    value_field: Some("chatAssignmentOpenedStatus"),
    value_proto_type: Some("SyncActionValue.ChatAssignmentOpenedStatusAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "agentChatAssignmentOpenedStatus",
        },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart { name: "agentId" },
    ],
};

/// `ChatLockSettings` — module `WAWebChatLockSettingsSync`.
pub const CHAT_LOCK_SETTINGS: Schema = Schema {
    key: "ChatLockSettings",
    name: "setting_chatLock",
    module: "WAWebChatLockSettingsSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("chatLockSettings"),
    value_proto_type: Some("ChatLockSettings"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_chatLock",
    }],
};

/// `ClearChat` — module `WAWebClearChatSync`.
pub const CLEAR_CHAT: Schema = Schema {
    key: "ClearChat",
    name: "clearChat",
    module: "WAWebClearChatSync",
    collection: Collection::RegularHigh,
    version: 6,
    scope: Scope::ChatMessageRange,
    value_field: Some("clearChatAction"),
    value_proto_type: Some("SyncActionValue.ClearChatAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "clearChat" },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart {
            name: "deleteStarred",
        },
        IndexPart::StringPart {
            name: "deleteMedia",
        },
    ],
};

/// `Contact` — module `WAWebContactSync`.
pub const CONTACT: Schema = Schema {
    key: "Contact",
    name: "contact",
    module: "WAWebContactSync",
    collection: Collection::CriticalUnblockLow,
    version: 2,
    scope: Scope::Account,
    value_field: Some("contactAction"),
    value_proto_type: Some("SyncActionValue.ContactAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal { value: "contact" },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `CustomPaymentMethods` — module `WAWebCustomPaymentMethodsSync`.
pub const CUSTOM_PAYMENT_METHODS: Schema = Schema {
    key: "CustomPaymentMethods",
    name: "custom_payment_methods",
    module: "WAWebCustomPaymentMethodsSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("customPaymentMethodsAction"),
    value_proto_type: Some("SyncActionValue.CustomPaymentMethodsAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "custom_payment_methods",
    }],
};

/// `CustomerData` — module `WAWebCustomerDataSync`.
pub const CUSTOMER_DATA: Schema = Schema {
    key: "CustomerData",
    name: "customer_data",
    module: "WAWebCustomerDataSync",
    collection: Collection::RegularLow,
    version: 1,
    scope: Scope::Account,
    value_field: Some("customerDataAction"),
    value_proto_type: Some("SyncActionValue.CustomerDataAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "customer_data",
        },
        IndexPart::StringPart { name: "chatJid" },
    ],
};

/// `DeleteChat` — module `WAWebDeleteChatSync`.
pub const DELETE_CHAT: Schema = Schema {
    key: "DeleteChat",
    name: "deleteChat",
    module: "WAWebDeleteChatSync",
    collection: Collection::RegularHigh,
    version: 6,
    scope: Scope::ChatMessageRange,
    value_field: Some("deleteChatAction"),
    value_proto_type: Some("SyncActionValue.DeleteChatAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "deleteChat",
        },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart {
            name: "deleteMedia",
        },
    ],
};

/// `DeleteMessageForMe` — module `WAWebDeleteMessageForMeSync`.
pub const DELETE_MESSAGE_FOR_ME: Schema = Schema {
    key: "DeleteMessageForMe",
    name: "deleteMessageForMe",
    module: "WAWebDeleteMessageForMeSync",
    collection: Collection::RegularHigh,
    version: 3,
    scope: Scope::Message,
    value_field: Some("deleteMessageForMeAction"),
    value_proto_type: Some("SyncActionValue.DeleteMessageForMeAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "deleteMessageForMe",
        },
        IndexPart::Jid { name: "remote" },
        IndexPart::StringPart { name: "id" },
        IndexPart::BoolString { name: "fromMe" },
        IndexPart::JidOrZero {
            name: "participant",
        },
    ],
};

/// `DetectedOutcomeStatus` — module `WAWebDetectedOutcomesStatusSync`.
pub const DETECTED_OUTCOME_STATUS: Schema = Schema {
    key: "DetectedOutcomeStatus",
    name: "detected_outcomes_status_action",
    module: "WAWebDetectedOutcomesStatusSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("detectedOutcomesStatusAction"),
    value_proto_type: Some("SyncActionValue.DetectedOutcomesStatusAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "detected_outcomes_status_action",
    }],
};

/// `DeviceCapabilities` — module `WAWebDeviceCapabilitiesSync`.
pub const DEVICE_CAPABILITIES: Schema = Schema {
    key: "DeviceCapabilities",
    name: "device_capabilities",
    module: "WAWebDeviceCapabilitiesSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("deviceCapabilities"),
    value_proto_type: Some("DeviceCapabilities"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "device_capabilities",
    }],
};

/// `DisableLinkPreviews` — module `WAWebDisableLinkPreviewsSync`.
pub const DISABLE_LINK_PREVIEWS: Schema = Schema {
    key: "DisableLinkPreviews",
    name: "setting_disableLinkPreviews",
    module: "WAWebDisableLinkPreviewsSync",
    collection: Collection::Regular,
    version: 8,
    scope: Scope::Account,
    value_field: Some("privacySettingDisableLinkPreviewsAction"),
    value_proto_type: Some("SyncActionValue.PrivacySettingDisableLinkPreviewsAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_disableLinkPreviews",
    }],
};

/// `ExternalWebBeta` — module `WAWebExternalWebBetaSync`.
pub const EXTERNAL_WEB_BETA: Schema = Schema {
    key: "ExternalWebBeta",
    name: "external_web_beta",
    module: "WAWebExternalWebBetaSync",
    collection: Collection::Regular,
    version: 3,
    scope: Scope::Account,
    value_field: Some("externalWebBetaAction"),
    value_proto_type: Some("SyncActionValue.ExternalWebBetaAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "external_web_beta",
    }],
};

/// `FavoriteSticker` — module `WAWebStickersFavoriteSyncAction`.
pub const FAVORITE_STICKER: Schema = Schema {
    key: "FavoriteSticker",
    name: "favoriteSticker",
    module: "WAWebStickersFavoriteSyncAction",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("stickerAction"),
    value_proto_type: Some("SyncActionValue.StickerAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "favoriteSticker",
        },
        IndexPart::StringPart { name: "filehash" },
    ],
};

/// `Favorites` — module `WAWebFavoritesSync`.
pub const FAVORITES: Schema = Schema {
    key: "Favorites",
    name: "favorites",
    module: "WAWebFavoritesSync",
    collection: Collection::RegularHigh,
    version: 1,
    scope: Scope::Account,
    value_field: Some("favoritesAction"),
    value_proto_type: Some("SyncActionValue.FavoritesAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal { value: "favorites" }],
};

/// `InteractiveMessageAction` — module `WAWebInteractiveMessageSync`.
pub const INTERACTIVE_MESSAGE_ACTION: Schema = Schema {
    key: "InteractiveMessageAction",
    name: "interactive_message_action",
    module: "WAWebInteractiveMessageSync",
    collection: Collection::RegularLow,
    version: 1,
    scope: Scope::Message,
    value_field: Some("interactiveMessageAction"),
    value_proto_type: Some("SyncActionValue.InteractiveMessageAction"),
    value_enum_fields: &[(
        "type",
        "InteractiveMessageAction.InteractiveMessageActionMode",
    )],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "interactive_message_action",
        },
        IndexPart::Jid { name: "remote" },
        IndexPart::StringPart { name: "id" },
        IndexPart::BoolString { name: "fromMe" },
        IndexPart::JidOrZero {
            name: "participant",
        },
        IndexPart::StringPart { name: "arg5" },
    ],
};

/// `LabelEdit` — module `WAWebLabelSync`.
pub const LABEL_EDIT: Schema = Schema {
    key: "LabelEdit",
    name: "label_edit",
    module: "WAWebLabelSync",
    collection: Collection::Regular,
    version: 3,
    scope: Scope::Account,
    value_field: Some("labelEditAction"),
    value_proto_type: Some("SyncActionValue.LabelEditAction"),
    value_enum_fields: &[("type", "LabelEditAction.ListType")],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "label_edit",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `LabelJid` — module `WAWebLabelJidSync`.
pub const LABEL_JID: Schema = Schema {
    key: "LabelJid",
    name: "label_jid",
    module: "WAWebLabelJidSync",
    collection: Collection::Regular,
    version: 3,
    scope: Scope::ChatOrContact,
    value_field: Some("labelAssociationAction"),
    value_proto_type: Some("SyncActionValue.LabelAssociationAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(2),
    index_parts: &[
        IndexPart::Literal { value: "label_jid" },
        IndexPart::StringPart { name: "labelId" },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `LabelReordering` — module `WAWebLabelReorderingSync`.
pub const LABEL_REORDERING: Schema = Schema {
    key: "LabelReordering",
    name: "label_reordering",
    module: "WAWebLabelReorderingSync",
    collection: Collection::Regular,
    version: 3,
    scope: Scope::Account,
    value_field: Some("labelReorderingAction"),
    value_proto_type: Some("SyncActionValue.LabelReorderingAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "label_reordering",
    }],
};

/// `LidContact` — module `WAWebLidContactSync`.
pub const LID_CONTACT: Schema = Schema {
    key: "LidContact",
    name: "lid_contact",
    module: "WAWebLidContactSync",
    collection: Collection::CriticalUnblockLow,
    version: 1,
    scope: Scope::Account,
    value_field: Some("lidContactAction"),
    value_proto_type: Some("SyncActionValue.LidContactAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "lid_contact",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `LocaleSetting` — module `WAWebLocaleSettingSync`.
pub const LOCALE_SETTING: Schema = Schema {
    key: "LocaleSetting",
    name: "setting_locale",
    module: "WAWebLocaleSettingSync",
    collection: Collection::CriticalBlock,
    version: 3,
    scope: Scope::Account,
    value_field: Some("localeSetting"),
    value_proto_type: Some("SyncActionValue.LocaleSetting"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_locale",
    }],
};

/// `LockChat` — module `WAWebLockChatSync`.
pub const LOCK_CHAT: Schema = Schema {
    key: "LockChat",
    name: "lock",
    module: "WAWebLockChatSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Chat,
    value_field: Some("lockChatAction"),
    value_proto_type: Some("SyncActionValue.LockChatAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "lock" },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `MarkChatAsRead` — module `WAWebMarkChatAsReadSync`.
pub const MARK_CHAT_AS_READ: Schema = Schema {
    key: "MarkChatAsRead",
    name: "markChatAsRead",
    module: "WAWebMarkChatAsReadSync",
    collection: Collection::RegularLow,
    version: 3,
    scope: Scope::ChatMessageRange,
    value_field: Some("markChatAsReadAction"),
    value_proto_type: Some("SyncActionValue.MarkChatAsReadAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal {
            value: "markChatAsRead",
        },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `MarketingMessage` — module `WAWebPremiumMessageSync`.
pub const MARKETING_MESSAGE: Schema = Schema {
    key: "MarketingMessage",
    name: "marketingMessage",
    module: "WAWebPremiumMessageSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Account,
    value_field: Some("marketingMessageAction"),
    value_proto_type: Some("SyncActionValue.MarketingMessageAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "marketingMessage",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `MarketingMessageBroadcast` — module `WAWebPremiumMessageBroadcastSync`.
pub const MARKETING_MESSAGE_BROADCAST: Schema = Schema {
    key: "MarketingMessageBroadcast",
    name: "marketingMessageBroadcast",
    module: "WAWebPremiumMessageBroadcastSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Account,
    value_field: None,
    value_proto_type: None,
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "marketingMessageBroadcast",
        },
        IndexPart::StringPart {
            name: "premiumMessageId",
        },
        IndexPart::StringPart { name: "messageId" },
    ],
};

/// `MerchantPaymentPartner` — module `WAWebMerchantPaymentPartnerSync`.
pub const MERCHANT_PAYMENT_PARTNER: Schema = Schema {
    key: "MerchantPaymentPartner",
    name: "merchant_payment_partner",
    module: "WAWebMerchantPaymentPartnerSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: None,
    value_proto_type: None,
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "merchant_payment_partner",
    }],
};

/// `Mute` — module `WAWebMuteChatSync`.
pub const MUTE: Schema = Schema {
    key: "Mute",
    name: "mute",
    module: "WAWebMuteChatSync",
    collection: Collection::RegularHigh,
    version: 2,
    scope: Scope::Chat,
    value_field: Some("muteAction"),
    value_proto_type: Some("SyncActionValue.MuteAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "mute" },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `NctSaltSync` — module `WAWebNctSaltSync`.
pub const NCT_SALT_SYNC: Schema = Schema {
    key: "NctSaltSync",
    name: "nct_salt_sync",
    module: "WAWebNctSaltSync",
    collection: Collection::RegularHigh,
    version: 1,
    scope: Scope::Account,
    value_field: Some("nctSaltSyncAction"),
    value_proto_type: Some("SyncActionValue.NctSaltSyncAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "nct_salt_sync",
    }],
};

/// `NoteEdit` — module `WAWebNoteSync`.
pub const NOTE_EDIT: Schema = Schema {
    key: "NoteEdit",
    name: "note_edit",
    module: "WAWebNoteSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("noteEditAction"),
    value_proto_type: Some("SyncActionValue.NoteEditAction"),
    value_enum_fields: &[("type", "NoteEditAction.NoteType")],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal { value: "note_edit" },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `Nux` — module `WAWebNuxSync`.
pub const NUX: Schema = Schema {
    key: "Nux",
    name: "nux",
    module: "WAWebNuxSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("nuxAction"),
    value_proto_type: Some("SyncActionValue.NuxAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal { value: "nux" },
        IndexPart::StringPart { name: "nuxKey" },
    ],
};

/// `OutContact` — module `WAWebOutContactSync`.
pub const OUT_CONTACT: Schema = Schema {
    key: "OutContact",
    name: "out_contact",
    module: "WAWebOutContactSync",
    collection: Collection::RegularLow,
    version: 1,
    scope: Scope::Account,
    value_field: Some("outContactAction"),
    value_proto_type: Some("SyncActionValue.OutContactAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "out_contact",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `PaymentInfo` — module `WAWebPaymentInfoSync`.
pub const PAYMENT_INFO: Schema = Schema {
    key: "PaymentInfo",
    name: "payment_info",
    module: "WAWebPaymentInfoSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("paymentInfoAction"),
    value_proto_type: Some("SyncActionValue.PaymentInfoAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "payment_info",
    }],
};

/// `PaymentTos` — module `WAWebPaymentTosSync`.
pub const PAYMENT_TOS: Schema = Schema {
    key: "PaymentTos",
    name: "payment_tos",
    module: "WAWebPaymentTosSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("paymentTosAction"),
    value_proto_type: Some("SyncActionValue.PaymentTosAction"),
    value_enum_fields: &[("paymentNotice", "PaymentTosAction.PaymentNotice")],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "payment_tos",
    }],
};

/// `Pin` — module `WAWebPinChatSync`.
pub const PIN: Schema = Schema {
    key: "Pin",
    name: "pin_v1",
    module: "WAWebPinChatSync",
    collection: Collection::RegularLow,
    version: 5,
    scope: Scope::Chat,
    value_field: Some("pinAction"),
    value_proto_type: Some("SyncActionValue.PinAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "pin_v1" },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `PnForLidChat` — module `WAWebPnForLidChatSync`.
pub const PN_FOR_LID_CHAT: Schema = Schema {
    key: "PnForLidChat",
    name: "pnForLidChat",
    module: "WAWebPnForLidChatSync",
    collection: Collection::Regular,
    version: 8,
    scope: Scope::Account,
    value_field: Some("pnForLidChatAction"),
    value_proto_type: Some("SyncActionValue.PnForLidChatAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "pnForLidChat",
        },
        IndexPart::StringPart { name: "lid" },
    ],
};

/// `PrimaryFeature` — module `WAWebPrimaryFeatureSync`.
pub const PRIMARY_FEATURE: Schema = Schema {
    key: "PrimaryFeature",
    name: "primary_feature",
    module: "WAWebPrimaryFeatureSync",
    collection: Collection::Regular,
    version: 7,
    scope: Scope::Account,
    value_field: Some("primaryFeature"),
    value_proto_type: Some("SyncActionValue.PrimaryFeature"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "primary_feature",
    }],
};

/// `PrimaryVersion` — module `WAWebPrimaryVersionSync`.
pub const PRIMARY_VERSION: Schema = Schema {
    key: "PrimaryVersion",
    name: "primary_version",
    module: "WAWebPrimaryVersionSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("primaryVersionAction"),
    value_proto_type: Some("SyncActionValue.PrimaryVersionAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "primary_version",
        },
        IndexPart::StringPart { name: "key1" },
    ],
};

/// `QuickReply` — module `WAWebQuickRepliesSync`.
pub const QUICK_REPLY: Schema = Schema {
    key: "QuickReply",
    name: "quick_reply",
    module: "WAWebQuickRepliesSync",
    collection: Collection::Regular,
    version: 2,
    scope: Scope::Account,
    value_field: Some("quickReplyAction"),
    value_proto_type: Some("SyncActionValue.QuickReplyAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "quick_reply",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `RemoveRecentSticker` — module `WAWebStickersRemoveRecentSyncAction`.
pub const REMOVE_RECENT_STICKER: Schema = Schema {
    key: "RemoveRecentSticker",
    name: "removeRecentSticker",
    module: "WAWebStickersRemoveRecentSyncAction",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("removeRecentStickerAction"),
    value_proto_type: Some("SyncActionValue.RemoveRecentStickerAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "removeRecentSticker",
        },
        IndexPart::StringPart { name: "filehash" },
    ],
};

/// `Sentinel` — module `WAWebSentinelMutationSync`.
pub const SENTINEL: Schema = Schema {
    key: "Sentinel",
    name: "sentinel",
    module: "WAWebSentinelMutationSync",
    collection: Collection::RegularLow,
    version: 3,
    scope: Scope::Account,
    value_field: Some("keyExpiration"),
    value_proto_type: Some("SyncActionValue.KeyExpiration"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal { value: "sentinel" }],
};

/// `SettingPushName` — module `WAWebPushNameSync`.
pub const SETTING_PUSH_NAME: Schema = Schema {
    key: "SettingPushName",
    name: "setting_pushName",
    module: "WAWebPushNameSync",
    collection: Collection::CriticalBlock,
    version: 1,
    scope: Scope::Account,
    value_field: Some("pushNameSetting"),
    value_proto_type: Some("SyncActionValue.PushNameSetting"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_pushName",
    }],
};

/// `SettingsSync` — module `WAWebSettingsSync`.
pub const SETTINGS_SYNC: Schema = Schema {
    key: "SettingsSync",
    name: "settings_sync",
    module: "WAWebSettingsSync",
    collection: Collection::RegularLow,
    version: 1,
    scope: Scope::Account,
    value_field: Some("settingsSyncAction"),
    value_proto_type: Some("SyncActionValue.SettingsSyncAction"),
    value_enum_fields: &[
        (
            "bannerNotificationDisplayMode",
            "SettingsSyncAction.DisplayMode",
        ),
        (
            "mediaUploadQuality",
            "SettingsSyncAction.MediaQualitySetting",
        ),
        (
            "unreadCounterBadgeDisplayMode",
            "SettingsSyncAction.DisplayMode",
        ),
    ],
    chat_jid_index: Some(3),
    index_parts: &[
        IndexPart::Literal {
            value: "settings_sync",
        },
        IndexPart::Enum {
            name: "settingPlatform",
            proto_enum: "SettingsSyncAction.SettingPlatform",
        },
        IndexPart::Enum {
            name: "settingKey",
            proto_enum: "SettingsSyncAction.SettingKey",
        },
        IndexPart::Jid { name: "chatJid" },
    ],
};

/// `ShareOwnPn` — module `WAWebShareOwnPnSync`.
pub const SHARE_OWN_PN: Schema = Schema {
    key: "ShareOwnPn",
    name: "shareOwnPn",
    module: "WAWebShareOwnPnSync",
    collection: Collection::Regular,
    version: 8,
    scope: Scope::Account,
    value_field: None,
    value_proto_type: None,
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "shareOwnPn",
        },
        IndexPart::StringPart { name: "lid" },
    ],
};

/// `Star` — module `WAWebStarMessageSync`.
pub const STAR: Schema = Schema {
    key: "Star",
    name: "star",
    module: "WAWebStarMessageSync",
    collection: Collection::RegularHigh,
    version: 2,
    scope: Scope::Message,
    value_field: Some("starAction"),
    value_proto_type: Some("SyncActionValue.StarAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(1),
    index_parts: &[
        IndexPart::Literal { value: "star" },
        IndexPart::Jid { name: "remote" },
        IndexPart::StringPart { name: "id" },
        IndexPart::BoolString { name: "fromMe" },
        IndexPart::JidOrZero {
            name: "participant",
        },
    ],
};

/// `StatusPrivacy` — module `WAWebStatusPrivacySettingSync`.
pub const STATUS_PRIVACY: Schema = Schema {
    key: "StatusPrivacy",
    name: "status_privacy",
    module: "WAWebStatusPrivacySettingSync",
    collection: Collection::RegularHigh,
    version: 7,
    scope: Scope::Account,
    value_field: Some("statusPrivacy"),
    value_proto_type: Some("SyncActionValue.StatusPrivacyAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "status_privacy",
    }],
};

/// `SubscriptionsSyncV2` — module `WAWebSubscriptionsSyncV2Sync`.
pub const SUBSCRIPTIONS_SYNC_V2: Schema = Schema {
    key: "SubscriptionsSyncV2",
    name: "subscriptions_sync_v2",
    module: "WAWebSubscriptionsSyncV2Sync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("subscriptionsSyncV2Action"),
    value_proto_type: Some("SyncActionValue.SubscriptionsSyncV2Action"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "subscriptions_sync_v2",
    }],
};

/// `TimeFormat` — module `WAWebTimeFormatSync`.
pub const TIME_FORMAT: Schema = Schema {
    key: "TimeFormat",
    name: "time_format",
    module: "WAWebTimeFormatSync",
    collection: Collection::RegularLow,
    version: 7,
    scope: Scope::Account,
    value_field: Some("timeFormatAction"),
    value_proto_type: Some("SyncActionValue.TimeFormatAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "time_format",
    }],
};

/// `UnarchiveChatsSetting` — module `WAWebArchiveSettingSync`.
pub const UNARCHIVE_CHATS_SETTING: Schema = Schema {
    key: "UnarchiveChatsSetting",
    name: "setting_unarchiveChats",
    module: "WAWebArchiveSettingSync",
    collection: Collection::RegularLow,
    version: 4,
    scope: Scope::Account,
    value_field: Some("unarchiveChatsSetting"),
    value_proto_type: Some("SyncActionValue.UnarchiveChatsSetting"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_unarchiveChats",
    }],
};

/// `UserStatusMute` — module `WAWebUserStatusMuteSync`.
pub const USER_STATUS_MUTE: Schema = Schema {
    key: "UserStatusMute",
    name: "userStatusMute",
    module: "WAWebUserStatusMuteSync",
    collection: Collection::RegularHigh,
    version: 7,
    scope: Scope::Account,
    value_field: Some("userStatusMuteAction"),
    value_proto_type: Some("SyncActionValue.UserStatusMuteAction"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[
        IndexPart::Literal {
            value: "userStatusMute",
        },
        IndexPart::StringPart { name: "id" },
    ],
};

/// `VoipRelayAllCalls` — module `WAWebVoipRelayAllCallsSettingSync`.
pub const VOIP_RELAY_ALL_CALLS: Schema = Schema {
    key: "VoipRelayAllCalls",
    name: "setting_relayAllCalls",
    module: "WAWebVoipRelayAllCallsSettingSync",
    collection: Collection::Regular,
    version: 1,
    scope: Scope::Account,
    value_field: Some("privacySettingRelayAllCalls"),
    value_proto_type: Some("SyncActionValue.PrivacySettingRelayAllCalls"),
    value_enum_fields: &[],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "setting_relayAllCalls",
    }],
};

/// `WaffleAccountLinkState` — module `WAWebWaffleAccountLinkStateSync`.
pub const WAFFLE_ACCOUNT_LINK_STATE: Schema = Schema {
    key: "WaffleAccountLinkState",
    name: "waffle_account_link_state",
    module: "WAWebWaffleAccountLinkStateSync",
    collection: Collection::RegularHigh,
    version: 1,
    scope: Scope::Account,
    value_field: Some("waffleAccountLinkStateAction"),
    value_proto_type: Some("SyncActionValue.WaffleAccountLinkStateAction"),
    value_enum_fields: &[("linkState", "WaffleAccountLinkStateAction.AccountLinkState")],
    chat_jid_index: None,
    index_parts: &[IndexPart::Literal {
        value: "waffle_account_link_state",
    }],
};

/// Every action schema, keyed-sorted.
pub const ALL: &[Schema] = &[
    ADS_CTWA_PER_CUSTOMER_DATA_SHARING,
    AGENT,
    AI_THREAD_DELETE,
    AI_THREAD_PIN,
    AI_THREAD_RENAME,
    ANDROID_UNSUPPORTED_ACTIONS,
    ARCHIVE,
    AVATAR_UPDATED,
    BIZ_AI_SETTINGS_NUDGE,
    BOT_WELCOME_REQUEST,
    BUSINESS_BROADCAST_CAMPAIGN,
    BUSINESS_BROADCAST_INSIGHTS,
    BUSINESS_BROADCAST_LIST,
    CALL_LOG,
    CHAT_ASSIGNMENT,
    CHAT_ASSIGNMENT_OPENED_STATUS,
    CHAT_LOCK_SETTINGS,
    CLEAR_CHAT,
    CONTACT,
    CUSTOM_PAYMENT_METHODS,
    CUSTOMER_DATA,
    DELETE_CHAT,
    DELETE_MESSAGE_FOR_ME,
    DETECTED_OUTCOME_STATUS,
    DEVICE_CAPABILITIES,
    DISABLE_LINK_PREVIEWS,
    EXTERNAL_WEB_BETA,
    FAVORITE_STICKER,
    FAVORITES,
    INTERACTIVE_MESSAGE_ACTION,
    LABEL_EDIT,
    LABEL_JID,
    LABEL_REORDERING,
    LID_CONTACT,
    LOCALE_SETTING,
    LOCK_CHAT,
    MARK_CHAT_AS_READ,
    MARKETING_MESSAGE,
    MARKETING_MESSAGE_BROADCAST,
    MERCHANT_PAYMENT_PARTNER,
    MUTE,
    NCT_SALT_SYNC,
    NOTE_EDIT,
    NUX,
    OUT_CONTACT,
    PAYMENT_INFO,
    PAYMENT_TOS,
    PIN,
    PN_FOR_LID_CHAT,
    PRIMARY_FEATURE,
    PRIMARY_VERSION,
    QUICK_REPLY,
    REMOVE_RECENT_STICKER,
    SENTINEL,
    SETTING_PUSH_NAME,
    SETTINGS_SYNC,
    SHARE_OWN_PN,
    STAR,
    STATUS_PRIVACY,
    SUBSCRIPTIONS_SYNC_V2,
    TIME_FORMAT,
    UNARCHIVE_CHATS_SETTING,
    USER_STATUS_MUTE,
    VOIP_RELAY_ALL_CALLS,
    WAFFLE_ACCOUNT_LINK_STATE,
];

/// Look up a schema by its action key (the registry key, e.g. `"Agent"`).
pub fn by_name(key: &str) -> Option<&'static Schema> {
    ALL.iter().find(|s| s.key == key)
}
