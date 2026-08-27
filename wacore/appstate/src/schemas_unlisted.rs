//! Syncd action schemas the protocol carries but WA Web's action registry does
//! not build.
//!
//! [`schemas`](crate::schemas) is generated wholesale from the WA Web bundle, so
//! it can only declare actions that have a `*SyncdActionBase` module behind
//! them. An action name that survives in the protocol while the web client
//! stops offering the feature therefore disappears from that registry, and
//! hand-editing the generated file would lose it on the next regeneration.
//! Such schemas live here instead, each with the evidence that fixes its
//! collection, version and index shape.

use crate::schemas::{Collection, IndexPart, Schema, Scope};

/// `label_message` — associate a label with a single message.
///
/// Not in [`schemas::ALL`](crate::schemas::ALL): WA Web's live action table
/// (`WAWebSyncdConst.Actions`) lists only `label_edit`, `label_jid` and
/// `label_sublist`, so no module builds this mutation and the extractor has
/// nothing to emit. The name is still a first-class protocol action — WA Web's
/// protobuf action registry (`WAWebProtobufSyncAction.pb`) declares
/// `LABEL_MESSAGE_ACTION: "label_message"` alongside the chat association — and
/// the WhatsApp Business mobile clients emit it, which is why whatsmeow
/// (`appstate.BuildLabelMessage`) and Baileys (`addMessageLabel`) both send this
/// exact index.
///
/// Version 3 is `WAWebSyncdConst.LABEL_ASSOCIATION_SYNC_VERSION`, which covers
/// label associations as a family; both second opinions stamp the same value.
///
/// The trailing `fromMe`/`participant` slots are the message-key tail that every
/// message-scoped action carries (compare
/// [`STAR`](crate::schemas::STAR) and
/// [`DELETE_MESSAGE_FOR_ME`](crate::schemas::DELETE_MESSAGE_FOR_ME), both built
/// from WA Web's `constructMsgKeySegmentsFromMsgKey`). No source shows them
/// carrying anything but their defaults here, so callers should leave them at
/// `"0"`/`"0"`.
pub const LABEL_MESSAGE: Schema = Schema {
    key: "LabelMessage",
    name: "label_message",
    module: "WAWebProtobufSyncAction.pb",
    collection: Collection::Regular,
    version: 3,
    scope: Scope::Message,
    value_field: Some("labelAssociationAction"),
    value_proto_type: Some("SyncActionValue.LabelAssociationAction"),
    value_enum_fields: &[],
    chat_jid_index: Some(2),
    index_parts: &[
        IndexPart::Literal {
            value: "label_message",
        },
        IndexPart::StringPart { name: "labelId" },
        IndexPart::Jid { name: "chatJid" },
        IndexPart::StringPart { name: "id" },
        IndexPart::BoolString { name: "fromMe" },
        IndexPart::JidOrZero {
            name: "participant",
        },
    ],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schemas;

    #[test]
    fn label_message_is_absent_from_the_generated_registry() {
        // The reason this module exists. If a WA Web release brings the action
        // back, the regenerated registry will declare it and this schema should
        // be deleted rather than left to drift.
        assert!(
            schemas::ALL.iter().all(|s| s.name != LABEL_MESSAGE.name),
            "label_message is in the generated registry now; drop schemas_unlisted::LABEL_MESSAGE"
        );
    }

    #[test]
    fn label_message_matches_the_chat_association_family() {
        // Same collection, version and value field as `label_jid`; only the
        // index differs, by the message-key tail.
        assert_eq!(LABEL_MESSAGE.collection, schemas::LABEL_JID.collection);
        assert_eq!(LABEL_MESSAGE.version, schemas::LABEL_JID.version);
        assert_eq!(LABEL_MESSAGE.value_field, schemas::LABEL_JID.value_field);
        assert_eq!(
            LABEL_MESSAGE.value_proto_type,
            schemas::LABEL_JID.value_proto_type
        );
        assert_eq!(LABEL_MESSAGE.chat_jid_index, Some(2));
        assert_eq!(LABEL_MESSAGE.index_parts.len(), 6);
    }
}
