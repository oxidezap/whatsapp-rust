pub const OPEN_ENUMS: &[&str] = &[
    ".whatsapp.SyncdMutation.SyncdOperation",
    ".whatsapp.SyncActionValue.StatusPrivacyAction.StatusDistributionMode",
];

pub const FEATURE_ENUM_SERDE_FIELDS: &[&str] = &[
    ".whatsapp.SyncdMutation.operation",
    ".whatsapp.SyncActionValue.StatusPrivacyAction.mode",
    ".whatsapp.SyncActionValue.StatusPrivacyAction.modes",
];

pub const SHARED_BYTES_MESSAGES: &[&str] = &[
    ".whatsapp.SessionStructure.Chain.ChainKey",
    ".whatsapp.SessionStructure.Chain.MessageKey",
    ".whatsapp.SenderKeyStateStructure.SenderChainKey",
    ".whatsapp.SenderKeyStateStructure.SenderMessageKey",
    ".whatsapp.SenderKeyStateStructure.SenderSigningKey",
];
