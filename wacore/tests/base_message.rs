use wacore::proto_helpers::{MessageBuilderExt, MessageExt};
use wacore::send::stanza_type_from_message;
use waproto::whatsapp as wa;

#[test]
fn helpers_follow_classified_wrappers_and_edits() {
    macro_rules! check {
        ($($field:ident),+ $(,)?) => {
            $(
                let text = stringify!($field);
                let mut outer = wa::Message::default();
                outer.$field.get_or_insert_default().message =
                    buffa::MessageField::some(wa::Message::text(text));
                let borrowed = outer.text_content().unwrap();
                assert_eq!(borrowed, text);
                let allocation = borrowed.as_ptr();
                let owned = outer.into_base_message().conversation.unwrap();
                assert_eq!(owned, text);
                assert_eq!(owned.as_ptr(), allocation);
            )+
        };
    }
    check!(
        device_sent_message,
        ephemeral_message,
        view_once_message,
        view_once_message_v2,
        view_once_message_v2_extension,
        document_with_caption_message,
        group_mentioned_message,
        bot_invoke_message,
        associated_child_message,
        poll_creation_option_image_message,
        event_cover_image,
        group_status_message,
        group_status_message_v2,
        group_status_mention_message,
        status_add_yours,
        status_mention_message,
        question_message,
        question_reply_message,
        spoiler_message,
        lottie_sticker_message,
        limit_sharing_message,
        newsletter_admin_profile_message,
        newsletter_admin_profile_message_v2,
        poll_creation_message_v4,
        bot_forwarded_message,
        edited_message,
    );
}

#[test]
fn device_sent_and_edits_keep_their_distinct_precedence() {
    let mut image = wa::Message::default();
    image.image_message.get_or_insert_default().caption = Some("photo".into());
    let mut edit = wa::Message::default();
    edit.edited_message.get_or_insert_default().message = buffa::MessageField::some(image.clone());
    assert_eq!(stanza_type_from_message(&edit), "text");
    assert_eq!(edit.get_caption(), Some("photo"));
    assert_eq!(edit.clone().into_base_message(), image);

    let mut outer = wa::Message::default();
    outer.device_sent_message.get_or_insert_default().message = buffa::MessageField::some(edit);
    outer.ephemeral_message.get_or_insert_default();
    let mut poll = wa::Message::default();
    poll.poll_creation_message.get_or_insert_default();
    outer
        .group_mentioned_message
        .get_or_insert_default()
        .message = buffa::MessageField::some(poll);
    assert_eq!(stanza_type_from_message(&outer), "poll");
    assert_eq!(outer.get_base_message(), &image);
    assert_eq!(outer.into_base_message(), image);
}

#[test]
fn helpers_limit_repeated_wrappers_and_preserve_empty_ones() {
    fn wrap(inner: wa::Message) -> wa::Message {
        let mut outer = wa::Message::default();
        outer
            .associated_child_message
            .get_or_insert_default()
            .message = buffa::MessageField::some(inner);
        outer
    }
    let mut deep = wa::Message::text("leaf");
    for _ in 0..buffa::RECURSION_LIMIT {
        deep = wrap(deep);
    }
    assert_eq!(deep.text_content(), Some("leaf"));
    let deep = wrap(deep);
    assert_eq!(stanza_type_from_message(&deep), "text");
    let expected = wrap(wa::Message::text("leaf"));
    assert_eq!(deep.get_base_message(), &expected);
    assert_eq!(deep.into_base_message(), expected);

    let mut empty = wa::Message::text("fallback");
    empty.device_sent_message.get_or_insert_default().phash = Some("keep".into());
    empty.associated_child_message.get_or_insert_default();
    assert!(std::ptr::eq(empty.get_base_message(), &empty));
    assert_eq!(empty.clone().into_base_message(), empty);
}

#[test]
fn empty_wrappers_do_not_hide_later_bodies_or_follow_references() {
    let mut msg = wa::Message::default();
    msg.device_sent_message.get_or_insert_default();
    msg.ephemeral_message.get_or_insert_default();
    msg.edited_message.get_or_insert_default().message =
        buffa::MessageField::some(wa::Message::text("edit"));
    msg.associated_child_message.get_or_insert_default().message =
        buffa::MessageField::some(wa::Message::text("child"));
    assert_eq!(msg.text_content(), Some("child"));
    assert_eq!(msg.into_base_message(), wa::Message::text("child"));

    let mut opaque = wa::Message::default();
    opaque.bot_task_message.get_or_insert_default().message =
        buffa::MessageField::some(wa::Message::text("task"));
    let key = opaque
        .message_context_info
        .get_or_insert_default()
        .message_association
        .get_or_insert_default()
        .parent_message_key
        .get_or_insert_default();
    key.id = Some("parent".into());
    assert!(opaque.text_content().is_none());
    assert!(std::ptr::eq(opaque.get_base_message(), &opaque));
    assert_eq!(opaque.clone().into_base_message(), opaque);
}
