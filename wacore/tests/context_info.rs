use std::cell::Cell;
use std::collections::BTreeSet;
use wacore::proto_helpers::MessageExt;
use waproto::whatsapp as wa;

fn context() -> wa::ContextInfo {
    wa::ContextInfo {
        stanza_id: Some("quoted-message".into()),
        is_forwarded: Some(true),
        forwarding_score: Some(2),
        mentioned_jid: vec!["assistant@bot".into()],
        expiration: Some(3600),
        quoted_message: buffa::MessageField::some(wa::Message {
            conversation: Some("quoted text".repeat(1024)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

// Independently enumerate the direct ContextInfo fields in the current schema.
// Multiple Message fields share a payload type (poll versions and video/PTV).
fn carriers() -> Vec<(&'static str, wa::Message)> {
    let mut messages = Vec::new();
    macro_rules! add {
        ($($field:ident),+ $(,)?) => {
            $(
                let mut message = wa::Message::default();
                message.$field.get_or_insert_default().context_info =
                    buffa::MessageField::some(context());
                messages.push((stringify!($field), message));
            )+
        };
    }
    add!(
        image_message,
        contact_message,
        location_message,
        extended_text_message,
        document_message,
        audio_message,
        video_message,
        call,
        contacts_array_message,
        live_location_message,
        template_message,
        sticker_message,
        group_invite_message,
        template_button_reply_message,
        product_message,
        list_message,
        order_message,
        list_response_message,
        buttons_message,
        buttons_response_message,
        interactive_message,
        interactive_response_message,
        poll_creation_message,
        request_phone_number_message,
        poll_creation_message_v2,
        poll_creation_message_v3,
        ptv_message,
        message_history_bundle,
        event_message,
        newsletter_admin_invite_message,
        album_message,
        sticker_pack_message,
        poll_result_snapshot_message,
        rich_response_message,
        message_history_notice,
        poll_creation_message_v5,
        newsletter_follower_invite_message_v2,
        poll_result_snapshot_message_v3,
        poll_creation_message_v6,
        event_invite_message,
        split_payment_message,
        music_message,
    );
    messages
}

// A narrow check of the vendored proto's formatting, not runtime reflection.
// Only optional fields directly inside a named message are relevant here.
fn proto_direct_optional_fields<'a>(source: &'a str, name: &str) -> Vec<(&'a str, &'a str)> {
    let marker = format!("message {name} {{");
    let body = source.split_once(&marker).unwrap().1;
    let mut depth = 1;
    let mut fields = Vec::new();
    for line in body.lines() {
        if depth == 1
            && let Some(field) = line.trim().strip_prefix("optional ")
        {
            let mut tokens = field.split_whitespace();
            fields.push((tokens.next().unwrap(), tokens.next().unwrap()));
        }
        depth += line.matches('{').count();
        depth -= line.matches('}').count();
        if depth == 0 {
            return fields;
        }
    }
    panic!("unterminated proto message: {name}");
}

#[test]
fn independent_carrier_fixtures_match_the_schema() {
    let source = include_str!("../../waproto/src/whatsapp.proto");
    let message_scope = source.split_once("message Message {").unwrap().1;
    let schema: BTreeSet<_> = proto_direct_optional_fields(source, "Message")
        .into_iter()
        .filter(|(payload, _)| {
            let marker = format!("message {payload} {{");
            // Several ContextInfo subtypes reuse Message's payload names.
            // Resolve Message-local declarations before top-level ones.
            let scope = if message_scope.contains(&marker) {
                message_scope
            } else {
                source
            };
            scope.contains(&marker)
                && proto_direct_optional_fields(scope, payload)
                    .contains(&("ContextInfo", "contextInfo"))
        })
        .map(|(_, name)| name.to_ascii_lowercase())
        .collect();
    let fixtures: BTreeSet<_> = carriers()
        .into_iter()
        .map(|(name, _)| name.replace('_', "").to_ascii_lowercase())
        .collect();
    assert!(!schema.is_empty());
    assert_eq!(
        fixtures, schema,
        "update independent fixtures when schema carriers change"
    );
}

#[test]
fn all_direct_carriers_share_context_helpers() {
    assert_eq!(carriers().len(), 42);
    for (name, mut message) in carriers() {
        let borrowed = message.context_info().expect(name);
        assert_eq!(
            borrowed.stanza_id.as_deref(),
            Some("quoted-message"),
            "{name}"
        );
        assert!(message.is_forwarded(), "{name}");
        assert!(message.mentions_any_bot(), "{name}");
        assert_eq!(message.get_ephemeral_expiration(), Some(3600), "{name}");

        let quoted = message.prepare_for_quote();
        let ctx = quoted.context_info().expect(name);
        assert!(ctx.mentioned_jid.is_empty(), "{name}");
        assert!(ctx.quoted_message.is_unset(), "{name}");

        let forwarded = message.prepare_for_forward();
        let ctx = forwarded.context_info().expect(name);
        assert_eq!(ctx.is_forwarded, Some(true), "{name}");
        assert_eq!(ctx.forwarding_score, Some(3), "{name}");
        assert!(ctx.mentioned_jid.is_empty(), "{name}");
        assert!(ctx.quoted_message.is_unset(), "{name}");

        assert!(
            message.set_context_info(wa::ContextInfo::default()),
            "{name}"
        );
        assert!(!message.is_forwarded(), "{name}");
        assert!(!message.mentions_any_bot(), "{name}");
        assert_eq!(message.get_ephemeral_expiration(), None, "{name}");
        assert!(!message.set_ephemeral_expiration(0), "{name}");
        assert!(message.set_ephemeral_expiration(7200), "{name}");
        assert_eq!(
            message.context_info().unwrap().expiration,
            Some(7200),
            "{name}"
        );
    }
}

#[test]
fn context_is_borrowed_from_location_and_nested_wrappers() {
    let mut message = wa::Message::default();
    message
        .location_message
        .get_or_insert_default()
        .context_info = buffa::MessageField::some(context());
    let expected = message
        .location_message
        .as_option()
        .unwrap()
        .context_info
        .as_option()
        .unwrap();
    assert!(std::ptr::eq(message.context_info().unwrap(), expected));
    let expected_quote = expected.quoted_message.as_option().unwrap() as *const wa::Message;
    let wrapped = wa::Message {
        device_sent_message: buffa::MessageField::some(wa::message::DeviceSentMessage {
            message: buffa::MessageField::some(wa::Message {
                ephemeral_message: buffa::MessageField::some(wa::message::FutureProofMessage {
                    message: buffa::MessageField::some(message),
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    let base_context = wrapped
        .get_base_message()
        .location_message
        .as_option()
        .unwrap()
        .context_info
        .as_option()
        .unwrap();
    assert!(std::ptr::eq(wrapped.context_info().unwrap(), base_context));
    assert!(std::ptr::eq(
        wrapped
            .context_info()
            .unwrap()
            .quoted_message
            .as_option()
            .unwrap(),
        expected_quote
    ));
    assert_eq!(
        wrapped.is_forwarded(),
        wrapped.context_info().unwrap().is_forwarded.unwrap()
    );
    assert!(wrapped.mentions_any_bot());
    // Expiration reads and setters operate on the outer message, not its base.
    assert_eq!(wrapped.get_ephemeral_expiration(), None);
    let mut wrapped = wrapped;
    assert!(!wrapped.set_context_info(wa::ContextInfo::default()));
    assert!(!wrapped.set_ephemeral_expiration(7200));
    assert_eq!(wrapped.context_info().unwrap().expiration, Some(3600));
}

#[test]
fn context_absence_and_precedence_are_preserved() {
    let mut message = wa::Message::default();
    assert!(message.context_info().is_none());
    message.location_message.get_or_insert_default();
    assert!(message.context_info().is_none());
    message.message_context_info.get_or_insert_default();
    assert!(message.context_info().is_none());
    message.ephemeral_message.get_or_insert_default(); // no inner message
    message
        .location_message
        .get_or_insert_default()
        .context_info = buffa::MessageField::some(context());
    assert!(message.context_info().is_some());
    message.extended_text_message.get_or_insert_default(); // no context: skip this carrier
    assert_eq!(
        message.context_info().unwrap().stanza_id.as_deref(),
        Some("quoted-message")
    );
    message
        .extended_text_message
        .get_or_insert_default()
        .context_info = buffa::MessageField::some(wa::ContextInfo::default());
    assert!(!message.is_forwarded());
    assert!(!message.mentions_any_bot());
    assert!(message.context_info().unwrap().stanza_id.is_none());
    // The expiration reader skips missing/zero expiration, unlike the getter.
    assert_eq!(message.get_ephemeral_expiration(), Some(3600));
    message
        .extended_text_message
        .get_or_insert_default()
        .context_info
        .get_or_insert_default()
        .expiration = Some(0);
    assert_eq!(message.get_ephemeral_expiration(), Some(3600));
    message.ephemeral_message.get_or_insert_default().message =
        buffa::MessageField::some(wa::Message::default());
    // A present empty inner message wins over outer carriers.
    assert!(message.context_info().is_none());
    assert!(!message.is_forwarded());
}

// An existing downstream implementation supplies only the pre-existing methods.
// The new accessor must be provided by the trait, not a required host method.
struct MessageView(wa::Message, Cell<usize>);

impl MessageExt for MessageView {
    fn get_base_message(&self) -> &wa::Message {
        self.1.set(self.1.get() + 1);
        self.0.get_base_message()
    }
    fn into_base_message(self) -> wa::Message {
        self.0.into_base_message()
    }
    fn is_ephemeral(&self) -> bool {
        self.0.is_ephemeral()
    }
    fn is_view_once(&self) -> bool {
        self.0.is_view_once()
    }
    fn get_caption(&self) -> Option<&str> {
        self.0.get_caption()
    }
    fn text_content(&self) -> Option<&str> {
        self.0.text_content()
    }
    fn prepare_for_quote(&self) -> Box<wa::Message> {
        self.0.prepare_for_quote()
    }
    fn prepare_for_forward(&self) -> wa::Message {
        self.0.prepare_for_forward()
    }
    fn set_context_info(&mut self, context: wa::ContextInfo) -> bool {
        self.0.set_context_info(context)
    }
    fn get_ephemeral_expiration(&self) -> Option<u32> {
        self.0.get_ephemeral_expiration()
    }
    fn set_ephemeral_expiration(&mut self, expiration: u32) -> bool {
        self.0.set_ephemeral_expiration(expiration)
    }
    fn is_forwarded(&self) -> bool {
        self.0.is_forwarded()
    }
    fn mentions_any_bot(&self) -> bool {
        self.0.mentions_any_bot()
    }
}

#[test]
fn downstream_trait_implementation_inherits_borrowed_accessor() {
    let (_, message) = carriers().pop().unwrap();
    let view = MessageView(message, Cell::new(0));
    assert!(std::ptr::eq(
        view.context_info().unwrap(),
        view.0.context_info().unwrap()
    ));
}

#[test]
fn context_unwraps_only_once_for_late_or_absent_carriers() {
    let (_, late) = carriers().pop().unwrap();
    for message in [late, wa::Message::default()] {
        let view = MessageView(message, Cell::new(0));
        assert_eq!(
            view.context_info().is_some(),
            view.0.context_info().is_some()
        );
        assert_eq!(view.1.get(), 1);
    }
}

#[test]
fn context_uses_existing_base_wrapper_order() {
    // get_base_message does a single ordered pass, not arbitrary recursion.
    let mut inner = wa::Message::default();
    inner
        .poll_creation_message_v5
        .get_or_insert_default()
        .context_info = buffa::MessageField::some(context());
    let message = wa::Message {
        ephemeral_message: buffa::MessageField::some(wa::message::FutureProofMessage {
            message: buffa::MessageField::some(wa::Message {
                device_sent_message: buffa::MessageField::some(wa::message::DeviceSentMessage {
                    message: buffa::MessageField::some(inner),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    assert!(message.get_base_message().device_sent_message.is_set());
    assert!(message.context_info().is_none());
    assert!(!message.is_forwarded());
}
