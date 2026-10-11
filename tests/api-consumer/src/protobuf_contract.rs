//! Source compatibility of generated protobufs from a separate crate.
use whatsapp_rust::waproto::whatsapp as wa;

pub fn owned(text: &str) -> wa::Message {
    wa::Message::default().with_conversation(text)
}
pub fn view(text: &str) -> wa::MessageView<'_> {
    let mut message = wa::MessageView::default();
    message.conversation = Some(text);
    message
}
pub fn encryption_type(value: wa::ADVEncryptionType) -> bool {
    match value {
        wa::ADVEncryptionType::E2EE => true,
        _ => false,
    }
}
pub fn has_operation(value: wa::biz_ai_metadata_sync::Operation) -> bool {
    match value {
        wa::biz_ai_metadata_sync::Operation::ServerEvent(_) => true,
        _ => false,
    }
}
pub fn has_view_operation(value: wa::biz_ai_metadata_sync::OperationView<'_>) -> bool {
    match value {
        wa::biz_ai_metadata_sync::OperationView::ServerEvent(_) => true,
        _ => false,
    }
}
#[test]
fn defaults_assignments_and_setters_work_externally() {
    assert_eq!(
        owned("synthetic").conversation.as_deref(),
        Some("synthetic")
    );
    assert_eq!(view("synthetic").conversation, Some("synthetic"));
    let wa::Message { conversation, .. } = owned("synthetic");
    assert_eq!(conversation.as_deref(), Some("synthetic"));
}
#[cfg(feature = "proto-owned-literal")]
pub fn forbidden_owned_literal() {
    let _ = wa::Message {
        ..Default::default()
    };
}
#[cfg(feature = "proto-view-literal")]
pub fn forbidden_view_literal() {
    let _ = wa::MessageView {
        ..Default::default()
    };
}
#[cfg(feature = "proto-exhaustive-enum")]
pub fn forbidden_enum(value: wa::ADVEncryptionType) {
    match value {
        wa::ADVEncryptionType::E2EE
        | wa::ADVEncryptionType::HOSTED
        | wa::ADVEncryptionType::NON_E2EE => {}
    }
}
#[cfg(feature = "proto-exhaustive-oneof")]
pub fn forbidden_oneof(value: wa::biz_ai_metadata_sync::Operation) {
    match value {
        wa::biz_ai_metadata_sync::Operation::ServerEvent(_) => {}
    }
}
#[cfg(feature = "proto-exhaustive-view-oneof")]
pub fn forbidden_view_oneof(value: wa::biz_ai_metadata_sync::OperationView<'_>) {
    match value {
        wa::biz_ai_metadata_sync::OperationView::ServerEvent(_) => {}
    }
}
