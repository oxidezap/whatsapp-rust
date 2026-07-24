//! Compile-shaped proof that a downstream crate can build message literals and
//! implement the async traits using only this crate's re-exports (no direct
//! buffa/bytes/anyhow/async-trait/chrono dependency of its own).
#![cfg(test)]
// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use crate as whatsapp_rust;
use whatsapp_rust::waproto::whatsapp as wa;

#[test]
fn message_literals_build_from_reexports_only() {
    // Explicit MessageField path, as a consumer would write it.
    let explicit = wa::Message {
        extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
            text: Some("hi".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    // The From<T> route: no MessageField naming at all.
    let via_into = wa::Message {
        extended_text_message: wa::message::ExtendedTextMessage {
            text: Some("hi".into()),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    };
    assert_eq!(explicit, via_into);

    // Encode/decode through the re-exported Message trait.
    use whatsapp_rust::buffa::Message as _;
    let bytes = explicit.encode_to_vec();
    let back = wa::Message::decode_from_slice(&bytes).unwrap();
    assert_eq!(back, via_into);
}

// An implementable trait built purely from re-exports, the veloz shape.
struct NoopHook;

#[whatsapp_rust::async_trait]
impl whatsapp_rust::InboundDurabilityHook for NoopHook {
    async fn on_messages(
        &self,
        _client: std::sync::Arc<whatsapp_rust::Client>,
        _batch: &[whatsapp_rust::types::events::InboundMessage],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[test]
fn hook_impl_is_object_safe_and_constructible() {
    let hook: Box<dyn whatsapp_rust::InboundDurabilityHook> = Box::new(NoopHook);
    let _ = &hook;
}

// A RetryAdmission policy built purely from re-exports.
struct AdmitAll;

impl whatsapp_rust::RetryAdmission for AdmitAll {
    fn admit(
        &self,
        _chat: &whatsapp_rust::Jid,
        _requester: &whatsapp_rust::Jid,
        _retry_count: u8,
    ) -> bool {
        true
    }
}

#[test]
fn retry_admission_is_object_safe_and_constructible() {
    let policy: Box<dyn whatsapp_rust::RetryAdmission> = Box::new(AdmitAll);
    let _ = &policy;
}

#[test]
fn bytes_and_chrono_reexports_are_usable() {
    let b = bytes::Bytes::from_static(b"frame");
    assert_eq!(b.len(), 5);
    let _ts: chrono::DateTime<chrono::Utc> = wacore::time::now_utc();
}
