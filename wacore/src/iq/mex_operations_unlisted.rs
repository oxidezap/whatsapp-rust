//! Historical public MEX contracts from whatspec 1a441f0329c941fcdb238490a6c604550d8a9939.
//! Their GraphQL definitions are absent from the next locked Web capture.
//! Retention preserves API and payload shapes, not a claim of current server support.

use serde::{Deserialize, Serialize};

/// `WAWebCreateLabyrinthBackupJobMutation` (mutation).
pub mod create_labyrinth_backup {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebCreateLabyrinthBackupJobMutation";
    pub const DOC_ID: &str = "27515507191403198";
    pub const OPERATION_KIND: &str = "mutation";
    pub const VARIABLE_KEYS: &[&str] = &["input"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaLabyrinthCreateBackup {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub backup_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub epoch_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mailbox_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub vd_device_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_labyrinth_create_backup: Option<WaLabyrinthCreateBackup>,
    }
}

/// `WAWebDebugLabyrinthInboxSnapshotQuery` (query).
pub mod debug_labyrinth_inbox_snapshot {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebDebugLabyrinthInboxSnapshotQuery";
    pub const DOC_ID: &str = "26544537655223129";
    pub const OPERATION_KIND: &str = "query";
    pub const VARIABLE_KEYS: &[&str] = &["params"];

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Params {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lower_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub num_msgs: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub num_threads: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub upper_timestamp: Option<i64>,
    }

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub params: Option<Params>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Item {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Messages {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_payload: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encryption_version: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ItemsWithMessages {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub item: Option<Item>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub messages: Option<Vec<Messages>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SnapshotThreadsWithMessages {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub items_with_messages: Option<Vec<ItemsWithMessages>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GetWaMailbox {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub snapshot_threads_with_messages: Option<SnapshotThreadsWithMessages>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub get_wa_mailbox: Option<GetWaMailbox>,
    }
}

/// `WAWebDebugLabyrinthRangeQuery` (query).
pub mod debug_labyrinth_range {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebDebugLabyrinthRangeQuery";
    pub const DOC_ID: &str = "27219778391054922";
    pub const OPERATION_KIND: &str = "query";
    pub const VARIABLE_KEYS: &[&str] = &["device_id", "message_count", "partial_thread_id"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub device_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub message_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub partial_thread_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_payload: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encryption_version: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PageInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_next_page: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_previous_page: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Messages {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_info: Option<PageInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GetWAMessagingViewerThreadByORF {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub messages: Option<Messages>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(rename = "get_WAMessagingViewerThreadByORF")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub get_wa_messaging_viewer_thread_by_orf: Option<GetWAMessagingViewerThreadByORF>,
    }
}

/// `EBMessageMetadataQueryQuery` (query).
pub mod eb_message_metadata_query {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "EBMessageMetadataQueryQuery";
    pub const DOC_ID: &str = "28525853583670706";
    pub const OPERATION_KIND: &str = "query";
    pub const VARIABLE_KEYS: &[&str] = &["data"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DeanonMessagesMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_admin_message: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub offline_threading_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sender_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sort_order_ms: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Mailbox {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub deanon_messages_metadata: Option<Vec<DeanonMessagesMetadata>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EncryptedBackup {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mailbox: Option<Mailbox>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Viewer {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_backup: Option<EncryptedBackup>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub viewer: Option<Viewer>,
    }
}

/// `WAWebRotateLabyrinthEpochJobMutation` (mutation).
pub mod rotate_labyrinth_epoch {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebRotateLabyrinthEpochJobMutation";
    pub const DOC_ID: &str = "27545094465170765";
    pub const OPERATION_KIND: &str = "mutation";
    pub const VARIABLE_KEYS: &[&str] = &["input"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaLabyrinthRotateEpoch {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub new_epoch_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_labyrinth_rotate_epoch: Option<WaLabyrinthRotateEpoch>,
    }
}

/// `WAWebTeamLinkCreateInvitationMutation` (mutation).
pub mod team_link_create_invitation {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebTeamLinkCreateInvitationMutation";
    pub const DOC_ID: &str = "27693700016951648";
    pub const OPERATION_KIND: &str = "mutation";
    pub const VARIABLE_KEYS: &[&str] = &["employeeName", "lid"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "employeeName")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub employee_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappTeamlinkCreateAgentInvitation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub employee_lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub employee_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expires_at: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invitation_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nonce_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_teamlink_create_agent_invitation:
            Option<WhatsappTeamlinkCreateAgentInvitation>,
    }
}

/// `WAWebTeamLinkListInvitationsQuery` (query).
pub mod team_link_list_invitations {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebTeamLinkListInvitationsQuery";
    pub const DOC_ID: &str = "27966540672965115";
    pub const OPERATION_KIND: &str = "query";
    pub const VARIABLE_KEYS: &[&str] = &[];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappTeamlinkListAgentInvitations {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub employee_lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub employee_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expires_at: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invitation_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nonce_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_teamlink_list_agent_invitations:
            Option<Vec<WhatsappTeamlinkListAgentInvitations>>,
    }
}

/// `WAWebTeamLinkRemoveInvitationMutation` (mutation).
pub mod team_link_remove_invitation {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebTeamLinkRemoveInvitationMutation";
    pub const DOC_ID: &str = "27015637738109068";
    pub const OPERATION_KIND: &str = "mutation";
    pub const VARIABLE_KEYS: &[&str] = &["lid"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappTeamlinkRemoveAgentInvitation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub removed: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub was_onboarded: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_teamlink_remove_agent_invitation:
            Option<WhatsappTeamlinkRemoveAgentInvitation>,
    }
}

/// `WAWebUploadLabyrinthMessagesJobMutation` (mutation).
pub mod upload_labyrinth_messages {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebUploadLabyrinthMessagesJobMutation";
    pub const DOC_ID: &str = "28023438937253549";
    pub const OPERATION_KIND: &str = "mutation";
    pub const VARIABLE_KEYS: &[&str] = &["input"];

    /// Variables for this operation, one field per variable the persisted document
    /// declares.
    ///
    /// Deliberately not `Default`: the server rejects a persisted query whose
    /// variables it cannot bind, so every variable is a decision a call site has to
    /// write out, and `..Default::default()` would make skipping one invisible.
    /// Passing `None` is still how a variable WhatsApp Web itself omits is omitted.
    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Results {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub offline_threading_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaLabyrinthUploadMessages {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub results: Option<Vec<Results>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_labyrinth_upload_messages: Option<WaLabyrinthUploadMessages>,
    }
}
